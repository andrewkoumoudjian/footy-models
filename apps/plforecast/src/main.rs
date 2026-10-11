#![forbid(unsafe_code)]
//! Reusable matchweek forecast with causal refits and June-2026 FM26 squad priors.
use footy_match::{OutcomeCalibrationSample,OutcomeCalibrator,TeamGoalModel,TeamGoalsObservation};
use serde_json::{json,Value};
use std::{collections::BTreeMap,env,fs};

#[derive(Clone,Copy,Default)]
struct Fm {attack:f64,defense:f64,coverage:f64}

fn metrics(samples:&[OutcomeCalibrationSample])->Value {
    if samples.is_empty(){return Value::Null;}
    let (mut brier,mut loss)=(0.0,0.0);
    for s in samples {
        brier+=(0..3).map(|i|(s.probabilities[i]-if i==s.actual {1.0}else{0.0}).powi(2)).sum::<f64>();
        loss-=s.probabilities[s.actual].max(1e-12).ln();
    }
    json!({"n":samples.len(),"brier":brier/samples.len() as f64,"logloss":loss/samples.len() as f64})
}

fn run()->Result<(),String>{
    let args:Vec<String>=env::args().skip(1).collect();
    let mut opts=BTreeMap::new();let mut it=args.iter();
    while let Some(key)=it.next(){opts.insert(key.as_str(),it.next().ok_or(format!("missing {key}"))?.as_str());}
    let get=|key:&str|->Result<&str,String>{opts.get(key).copied().ok_or(format!("missing {key}"))};
    let cutoff:i32=get("--as-of-day")?.parse::<i32>().map_err(|e|e.to_string())?;
    let contents=fs::read_to_string(get("--results")?).map_err(|e|e.to_string())?;
    let mut premier=Vec::new();let mut championship=Vec::new();
    for line in contents.lines().skip(1).filter(|x|!x.trim().is_empty()){
        let v:Vec<_>=line.split('\t').collect();if v.len()!=6{return Err(format!("bad results row: {line}"));}
        let row=TeamGoalsObservation{day:v[0].parse::<i32>().map_err(|e|e.to_string())?,home:v[2].into(),away:v[3].into(),
            home_goals:v[4].parse::<u8>().map_err(|e|e.to_string())?,away_goals:v[5].parse::<u8>().map_err(|e|e.to_string())?};
        if row.day>cutoff{return Err("future result in training".into());}
        if v[1]=="E0"{premier.push(row)}else if v[1]=="E1"{championship.push(row)}else{return Err("unknown division".into());}
    }
    premier.sort_by_key(|r|r.day);championship.sort_by_key(|r|r.day);
    let mut fm=BTreeMap::<String,Fm>::new();
    let contents=fs::read_to_string(get("--fm")?).map_err(|e|e.to_string())?;
    for line in contents.lines().skip(1).filter(|x|!x.trim().is_empty()){
        let v:Vec<_>=line.split('\t').collect();if v.len()!=4{return Err(format!("bad FM row: {line}"));}
        fm.insert(v[0].into(),Fm{attack:v[1].parse::<f64>().map_err(|e|e.to_string())?,
            defense:v[2].parse::<f64>().map_err(|e|e.to_string())?,
            coverage:v[3].parse::<f64>().map_err(|e|e.to_string())?});
    }
    // Prospective validation/calibration: learn post-hoc H/D/A correction only
    // on preceding season, evaluate it on 2026/27. FM snapshot never used in
    // pre-June-2026 training or calibration; freeze its coefficient before testing.
    let mut calibration=Vec::new();let mut eval_raw=Vec::new();let mut eval_fm=Vec::new();
    let mut eval_raw_cal=Vec::new();let mut eval_fm_cal=Vec::new();let mut eval_days=Vec::new();
    let backtest_start=cutoff-470;
    let train_eval_start=cutoff-75; // only current PL season for FM26 snapshot and season evaluation
    let mut previous_day=None;
    let mut daily_model:Option<TeamGoalModel>=None;
    for game in premier.iter().filter(|g|g.day>=backtest_start&&g.day<=cutoff){
        if previous_day!=Some(game.day){
            let daily_champ:Vec<_>=championship.iter().filter(|g|g.day<game.day).cloned().collect();
            daily_model=TeamGoalModel::fit(&premier,&daily_champ,game.day);
            previous_day=Some(game.day);
        }
        let Some(model)=&daily_model else{continue};
        let forecast=model.forecast(&game.home,&game.away);
        let actual=if game.home_goals>game.away_goals {0}else if game.home_goals==game.away_goals {1}else{2};
        let sample=OutcomeCalibrationSample{probabilities:forecast.outcomes,actual};
        if game.day<train_eval_start {calibration.push(sample);}
        else {
            eval_raw.push(sample);eval_days.push(game.day);
            let a=fm.get(&game.home).copied().unwrap_or_default();let b=fm.get(&game.away).copied().unwrap_or_default();
            let prediction=model.forecast_with_fm(&game.home,&game.away,a.attack,a.defense,b.attack,b.defense,0.16);
            eval_fm.push(OutcomeCalibrationSample{probabilities:prediction.outcomes,actual});
        }
    }
    if calibration.len()<70{return Err(format!("too few calibration samples {}",calibration.len()));}
    if eval_raw.len()<30{return Err(format!("too few recent holdout samples {}",eval_raw.len()));}
    let calibrator=OutcomeCalibrator::fit(&calibration).ok_or("calibrator failed")?;
    for s in &eval_raw {eval_raw_cal.push(OutcomeCalibrationSample{probabilities:calibrator.apply(s.probabilities).unwrap(),actual:s.actual});}
    for s in &eval_fm {eval_fm_cal.push(OutcomeCalibrationSample{probabilities:calibrator.apply(s.probabilities).unwrap(),actual:s.actual});}
    // Day-block bootstrap: preserve within-matchday score dependence and report
    // uncertainty in incremental FM Brier improvement on the small holdout.
    let mut blocks=BTreeMap::<i32,Vec<(f64,f64)>>::new();
    for ((raw,fm_sample),&day) in eval_raw_cal.iter().zip(&eval_fm_cal).zip(&eval_days){
        let brier=|s:&OutcomeCalibrationSample| -> f64 {
            (0..3).map(|i|(s.probabilities[i]-if i==s.actual {1.0}else{0.0}).powi(2)).sum()
        };
        blocks.entry(day).or_default().push((brier(raw),brier(fm_sample)));
    }
    let days:Vec<_>=blocks.values().collect();
    let mut seed=42_u64;let mut deltas=Vec::new();
    for _ in 0..3000 {
        let (mut sum,mut count)=(0.0,0_usize);
        for _ in 0..days.len() {
            seed ^= seed << 13;seed ^= seed >> 7;seed ^= seed << 17;
            let day=days[(seed as usize)%days.len()];
            for &(raw,fm) in day {sum+=raw-fm;count+=1;}
        }
        deltas.push(sum/count as f64);
    }
    deltas.sort_by(|a,b|a.total_cmp(b));
    let fm_brier_lift_ci=[deltas[75],deltas[2924]];
    let model=TeamGoalModel::fit(&premier,&championship,cutoff+1).ok_or("full fit failed")?;
    let fixtures=fs::read_to_string(get("--fixtures")?).map_err(|e|e.to_string())?;
    let mut predictions=Vec::new();
    for line in fixtures.lines().skip(1).filter(|x|!x.trim().is_empty()){
        let v:Vec<_>=line.split('\t').collect();if v.len()!=3{return Err(format!("bad fixture {line}"));}
        let home=fm.get(v[1]).copied().unwrap_or_default();let away=fm.get(v[2]).copied().unwrap_or_default();
        let base=model.forecast(v[1],v[2]);
        let result=model.forecast_with_fm(v[1],v[2],home.attack,home.defense,away.attack,away.defense,0.16);
        let p=calibrator.apply(result.outcomes).ok_or("calibration invalid")?;
        predictions.push(json!({"date":v[0],"home":v[1],"away":v[2],
            "home_win":p[0],"draw":p[1],"away_win":p[2],
            "home_expected_goals":result.home_goals,"away_expected_goals":result.away_goals,
            "modal_score":format!("{}-{}",result.modal_home,result.modal_away),
            "base_probabilities":base.outcomes,
            "fm_home_coverage":home.coverage,"fm_away_coverage":away.coverage,
            "fm_home_attack":home.attack,"fm_home_defense":home.defense,
            "fm_away_attack":away.attack,"fm_away_defense":away.defense}));
    }
    let report=json!({"model":"Time-decayed ridge-regularized Poisson goals + 2026 pre-season Football Manager CA/role composite priors + footy-match OutcomeCalibrator",
        "training":{"pl_matches":premier.len(),"championship_matches":championship.len(),"half_life_days":270,
            "fm_log_goal_coefficient":0.16,"fm_coefficient_selection":"fixed ex ante; not estimated or selected from holdout results; FM is not independently validated",
            "snapshot":"FM26 / TM joined player-roster snapshot June 2026; no injury/start information"},
        "validation":{"calibration_samples":calibration.len(),"evaluation_samples":eval_raw.len(),
          "calibration_temperature":calibrator.temperature,"fm_brier_improvement_day_bootstrap_95pct":fm_brier_lift_ci,"calibration_draw_bias":calibrator.draw_logit_bias,
          "raw_results_only":metrics(&eval_raw),"calibrated_results_only":metrics(&eval_raw_cal),
          "raw_results_plus_fm":metrics(&eval_fm),"calibrated_results_plus_fm":metrics(&eval_fm_cal),
          "protocol":"chronological day-excluded fits; calibration from historical matches before current season, holdout from current season; pre-season FM26 snapshot fixed"},
        "fixtures":predictions,
        "limitations":["FM club membership may be stale after summer 2026 transfers; no confirmed starting XI or injuries",
        "Missing FM club coverage is zero feature adjustment, NOT zero player quality",
        "FM coefficient is conservative heuristic, not validated incremental prediction; compare holdout ablation",
        "Independent Poisson goal counts omit match-state correlations"]});
    fs::write(get("--output")?,serde_json::to_string_pretty(&report).map_err(|e|e.to_string())?+"\n").map_err(|e|e.to_string())?;
    println!("{}",serde_json::to_string_pretty(&report).unwrap());
    Ok(())
}
fn main(){if let Err(e)=run(){eprintln!("forecast error: {e}");std::process::exit(1);}}
