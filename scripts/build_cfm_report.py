#!/usr/bin/env python3
"""Build the CF Montréal scout report (STEERING_CFM.md section 7.2).

Pure assembly from already-produced artifacts: the shortlist, the SofaScore
enrichment, the alumni/squad files and the Monte Carlo stats. No re-merge.
"""
import json
import pandas as pd

ROOT = "/home/drewclaw/footy-models"
per = {int(k): v for k, v in json.load(open("/tmp/cfm_per.json")).items()}

t = pd.read_csv(f"{ROOT}/data/cfm_scout_top20.csv")
alumni = pd.read_csv(f"{ROOT}/data/cfm_alumni.tsv", sep="\t")
alumni["success"] = alumni["current_mv"] / alumni["transfer_fee"]
alumni = alumni.sort_values("success", ascending=False)

def fmteu(v):
    v = float(v)
    if v >= 1e6: return f"EUR {v/1e6:.2f}M"
    if v >= 1e3: return f"EUR {v/1e3:.0f}k"
    return f"EUR {v:.0f}"

L = []
add = L.append

# ---------------- header ----------------
add("# CF Montréal Scouting Report: Top-20 Psych-Fit, Positive-Yield Value-Spike Targets")
add("")
add("**As-of vintage:** TM valuations through 2026-06-12; FM26 attributes (Nov 2025); "
    "SofaScore season statistics current at fetch time (Aug 2026). "
    "**Seed:** 42 everywhere (shortlist z-scores, Monte Carlo). "
    "**Club:** CF Montréal (MLS, club_id 4078).")
add("")
add("---")
add("")
add("## 1. Executive summary")
add("")
add("CF Montréal is a bridge club: it signs young players, develops them and sells them on "
    "to stronger leagues (the alumni evidence in section 3). This report ranks the top 20 "
    "candidates in the CFM scout basket (age < 23, EUR 400k to 2.5M market value) on a "
    "combination of **psychological fit** (calibrated on successful alumni), **positive "
    "12-month yield** (v2 forecast), **spike potential** (v3 expectile-0.9 tail score) and "
    "**squad complementarity** (the current squad has no left winger and only two "
    "left-backs). Every shortlisted player has strictly positive forecast yield and a "
    "tail score in the top 40% of the basket.")
add("")
add("**Headline table (ranked by combined score; MC numbers are the spike run, seed 42, "
    "1000 paths, annual vol 0.4688):**")
add("")
add("| # | Player | Club | Pos | Age | Value | Psych fit | Tail | 12m yield | Gap | SofaScore rating | MC median 36m | P(>=1.5x) |")
add("|---|--------|------|-----|-----|-------|-----------|------|-----------|-----|------------------|---------------|-----------|")
for pid in [int(x) for x in t["player_id"]]:
    p = per[pid]
    add(f"| {t.loc[t.player_id==pid,'rank'].iloc[0]} | {p['name']} | {p['club']} | {p['pos']} | {p['age']:.1f} | "
        f"{fmteu(p['mv'])} | {p['psych']:.3f} | {p['tail']:.2f} | {p['yieldpct']:.0f}% | {p['gap']} | "
        f"{p['rating']} | {fmteu(p['med_s'])} | {p['p15_s']:.0%} |")
add("")
add("The full detail for each player is in section 6, Monte Carlo fan charts (base and "
    "spike runs) in `artifacts/cfm_mc/charts/` and `artifacts/cfm_mc_spike/charts/`, and "
    "the machine-readable shortlist in `data/cfm_scout_top20.csv`.")
add("")
add("---")
add("")
add("## 2. Methodology")
add("")
add("### 2.1 Candidate universe (the CFM scout basket)")
add("")
add("From the pre-merged panel `data/cfm_scout_panel.csv` (14,769 rows, read-only, never "
    "re-merged) we apply the CFM scout basket filter:")
add("")
add("- age < 23 years (floating, as of the 2026-06-12 TM vintage);")
add("- EUR 400,000 <= current market value <= EUR 2,500,000;")
add("- `attr_source != 'staff'` (real FM database attributes only);")
add("- club not CF Montréal (excluded via `cfm_squad.tsv` names and club string match).")
add("")
add(f"The basket contains **1,255 players**. Forecast (f36) coverage is "
    f"**{int(pd.read_csv(f'{ROOT}/data/cfm_scout_panel.csv', usecols=['f36_gap_pct'])['f36_gap_pct'].notna().sum())}/14,769** "
    "rows (the 36-month forecast universe is smaller than the full FM x TM join); tail "
    "(v3 spike) coverage is 7,962/14,769. Coverage is reported honestly, never zero-filled.")
add("")
add("### 2.2 Psych archetype: calibrated on alumni, not opinion")
add("")
add("**Success label per alumnus:** `success = current_mv / transfer_fee` from "
    "`cfm_alumni.tsv`. Alumni with `success >= 1.5` and a meaningful fee (> EUR 300k) form "
    "the reference group (9 alumni); the **top half by success (5 alumni)** is the "
    "archetype calibration set:")
add("")
add("| Alumnus | Transfer fee | Current value | Success multiple |")
add("|---------|-------------|---------------|------------------|")
for _, a in alumni.iterrows():
    if a["success"] >= 1.5 and a["transfer_fee"] > 300000:
        mark = " (calibration set)" if a["player_name"] in ["Joel Waterman", "Nathan-Dylan Saliba", "Aarón Herrera", "Ismaël Koné", "Raheem Edwards"] else ""
        add(f"| {a['player_name']} | {fmteu(a['transfer_fee'])} | {fmteu(a['current_mv'])} | {a['success']:.2f}x{mark} |")
add("")
add("The archetype vector is the mean of non-missing FM attributes over these 5 alumni "
    "(FM26 full attrs, read-only lookup by uid):")
add("")
add("| Mental attribute | Archetype (alumni mean) |")
add("|------------------|--------------------------|")
arch = {"attr_Determination": 11.25, "attr_Bravery": 11.2, "attr_Consistency": 11.8,
        "attr_ImportantMatches": 8.75, "attr_Composure": 12.0, "attr_WorkRate": 12.4,
        "attr_Teamwork": 12.0, "attr_Leadership": 10.4, "attr_Versatility": 10.8,
        "attr_NaturalFitness": 12.75, "attr_InjuryProneness": 10.33}
for k, v in arch.items():
    add(f"| {k.removeprefix('attr_')} | {v:.2f} |")
add("")
add("Hidden personality (Pressure, Professionalism, Ambition, Adaptability, Temperament, "
    "Controversy) is present on ~32% of panel rows, but the alumni carry no usable hidden "
    "values (Koné's Ambition=0 and Adaptability=1 are FM placeholders outside the 1-20 "
    "scale). Per the steering's rule, the hidden-attribute archetype therefore uses the "
    "documented **bridge-club prior**: Pressure >= 12, Professionalism >= 12, "
    "Temperament 12, Controversy <= 8. InjuryProneness and Controversy are **negative** "
    "(lower is better).")
add("")
add("### 2.3 psych_fit score (exact formula)")
add("")
add("For each candidate and attribute the distance to the archetype is computed in "
    "z-units using the basket's per-attribute standard deviation (min 1.0):")
add("")
add("```")
add("z_dist = sqrt( mean over available attrs of ((arch[a] - v)/scale[a])^2 )")
add("         where for NEGATIVE attrs (InjuryProneness, Controversy) the signed")
add("         distance is reversed: ((v - arch[a])/scale[a])^2")
add("")
add("psych_fit = 1 / (1 + exp(K * (z_dist - Z0))),   K = 1.5, Z0 = 1.1")
add("```")
add("")
add("The score is computed over the intersection of available candidate attributes and "
    "archetype attributes; `psy_attrs_used` counts how many attributes entered the score "
    "(max 15 = 11 mental + 4 hidden). The sigmoid maps the distance to [0,1] and is "
    "monotone (higher fit, higher score). **Sanity check:** the calibration alumni "
    "themselves score 0.61 to 0.67 (min 0.614, mean 0.670), all >= 0.6 as required.")
add("")
add("### 2.4 Yield, spike and combined rank")
add("")
add("- **Yield (positive expected appreciation):** `f36_gap_pct > 0` AND "
    "`f36_predicted_12m_mv > current_mv` (both required, strictly positive). The shortlist "
    "uses only positive-yield players; `yield_12m_pct = 100 * f36_gap_pct`.")
add("- **Spike score:** `tail_tail_score` (v3 expectile-0.9 12-month log-ratio), the "
    "primary spike rank; all top-20 carry a real tail score and sit in the top 40% of the "
    "basket (>= 0.867) and above the basket spike median (0.845).")
add("- **Combined rank** (documented weights, applied over the coverage-gated pool of 617 "
    "players who pass yield, tail-coverage and attribute-coverage gates):")
add("")
add("```")
add("combined = z(psych_fit) + 0.5*z(tail_score) + 0.3*z(yield_12m_pct) + 0.4*z(squad_fit)")
add("```")
add("")
add("z-scores are over the coverage-gated pool. Selection constraints: the top-20 must be "
    "above the basket psych-fit median (0.573) and spike median (0.845), have positive "
    "yield, and respect position caps (max 3 per squad slot) so the shortlist stays "
    "complementary rather than concentrated.")
add("")
add("### 2.5 Squad-fit rules")
add("")
add("`squad_fit = positional need + age fit + value-band fit` with:")
add("")
add("- **Positional need:** LW and LB = 3 (squad has 0 LW, 2 LB); AM/CM/DM = 2; "
    "RW/RM/CF/SS/CB/RB/GK = 1 (overstocked: 6 CB, 5 CF, 4 RB, 4 RW, 3 GK).")
add("- **Age fit:** 17-21 preferred (MLS U22 initiative window): +2 if age < 22, else +1.")
add("- **Value band:** EUR 400k-1.2M ideal for CFM: +2, else (1.2M-2.5M) +1.")
add("")
add("### 2.6 Monte Carlo configuration")
add("")
add("The deterministic Rust binary `footy-mc` (run via `scripts/run_cfm_mc.sh`) simulates "
    "36 monthly steps with geometric-Brownian-style value paths:")
add("")
add("- seed 42 everywhere; 1,000 paths per player; annual vol `sigma_ann = 0.4688` "
    "(the calibrated constant used by the 36m basket run);")
add("- **Base run** (`artifacts/cfm_mc`): drift `mu_ann = ln(f36_predicted_12m_mv / "
    "current_mv)` (annual log drift from the v2 mean forecaster);")
add("- **Spike run** (`artifacts/cfm_mc_spike`): drift `mu_ann = tail_tail_score` (the "
    "v3 expectile-0.9 12-month log-ratio IS the spike scenario);")
add("- inputs are `data/cfm_mc_input.tsv` and `data/cfm_mc_input_spike.tsv` (11 columns "
    "each, written by `scripts/prep_cfm_mc.py`).")
add("")
add("MC stats are terminal-36-month percentiles; mean/median/p10/p90 are in EUR (stats.tsv "
    "is post-processed from cents by the chart renderer), P(>=1.5x), P(>=2x), P(>=3x) and "
    "downside (P(< 0.5x)) are probabilities.")
add("")
add("### 2.7 FBref and SofaScore join rules")
add("")
add("- **SofaScore** (primary): search `api.sofascore.com/api/v1/search/all?q=<name>`; "
    "identity confirmed by normalized name + Transfermarkt DOB (19/20 exact DOB match; "
    "Leandersson matches by club + name without middle name, confidence medium). Season "
    "line chosen by documented rule: current season (2026 / 25/26 naming as of the June "
    "2026 vintage), youth national-team competitions excluded, rated lines preferred, then "
    "most appearances (so loan and B-team players report the league where they actually "
    "play). Missing values stay blank; assists null in Serie C is reported as `missing`.")
add("- **FBref** (secondary): blocked. Plain HTTP returns 403 and curl_cffi chrome "
    "impersonation also returns 403 (Cloudflare); the max-3-attempts rule was respected. "
    "All 20 players are flagged `fbref_unavailable = yes (Cloudflare 403)` and the per-90 "
    "columns are recorded as `unavailable`.")
add("")
add("**Coverage counts:** SofaScore resolved 20/20 player ids, 20/20 with high/medium "
    "confidence, 20/20 season ratings; FBref 0/20 (blocked).")
add("")
add("---")
add("")
add("## 3. Alumni validation (the evidence base)")
add("")
add("All 18 CF Montréal players who left for a fee in the TM transfers record, with their "
    "transfer outcome and success multiple (`current_mv / transfer_fee`):")
add("")
add("| Player | Left (date) | To | Fee | Current value | Current club | Multiple |")
add("|--------|------------|----|-----|---------------|--------------|----------|")
for _, a in alumni.iterrows():
    add(f"| {a['player_name']} | {a['transfer_date']} | {a['to_club']} | {fmteu(a['transfer_fee'])} | "
        f"{fmteu(a['current_mv'])} | {a['current_club']} ({a['comp']}) | {a['success']:.2f}x |")
add("")
add("Nine alumni meet the `success >= 1.5` and `fee > EUR 300k` bar; the top half by "
    "success (Waterman 4.65x, Saliba 3.75x, Herrera 3.23x, Koné 3.12x, Edwards 2.32x) "
    "defines the psych archetype in 2.2. The pattern is the classic bridge-club loop: "
    "sign young, develop, sell at a multiple (Crépeau 40x, Binks 36x on tiny fees; Koné "
    "3.1x from EUR 8M to EUR 25M at Watford/Sassuolo).")
add("")
add("---")
add("")
add("## 4. Squad gap analysis (from data/cfm_squad.tsv, 33 players)")
add("")
add("| Sub-position | Count | Assessment |")
add("|--------------|-------|------------|")
add("| Left Winger | 0 | **CRITICAL GAP: no left winger in the squad** |")
add("| Left-Back | 2 | Gap (both 26+; Petrasso 26.0, Pearce 28.2) |")
add("| Centre-Back | 6 | Overstocked |")
add("| Centre-Forward | 5 | Overstocked |")
add("| Right-Back | 4 | Stocked |")
add("| Right Winger | 4 | Stocked (Synchuk 19.9 is the only young one) |")
add("| Attacking Midfield | 3 | Medium (Jaime 25.7, Carmona 23.3, Escobar 19.8) |")
add("| Central Midfield | 3 | Medium |")
add("| Defensive Midfield | 3 | Medium (Piette 31.6) |")
add("| Goalkeeper | 3 | Low need (Gillier 22, Breza 28, Keita 16.9) |")
add("")
add(f"Squad age profile: mean 24.2, range 16.9-32.8; total squad value EUR 37.65M. "
    "The structural reads: **LW is the #1 gap**, **LB is #2** (age profile is old at the "
    "position), and AM/CM/DM are medium-priority. The shortlist fills gaps in order: "
    "LW x3, LB x3, CM x3, DM x3, CB x3, AM x2, GK/CF/RW x1 each.")
add("")
add("---")
add("")
add("## 5. Model artifacts and seeds")
add("")
add("- Seed 42: used for the Monte Carlo (`--seed 42`) and for z-score construction "
    "determinism; no other stochastic steps exist in the scoring path.")
add("- `data/forecast/model.bin` (v2 mean forecaster) SHA-256: "
    "`1d023e96f7fc045f23b2de0842af90df65f0851235883a7111606a5cadd4596f` (unchanged).")
add("- `data/tail/model_exp090_r1e6.bin` (v3 expectile-0.9): mtime "
    "2026-08-10 10:56:55 -0400 (unchanged).")
add("- `data/surge/model.bin` (surge classifier): mtime 2026-08-10 02:16:18 -0400 (unchanged).")
add("")
add("---")
add("")
add("## 6. Top-20 detail")
add("")

def eur(v):
    v = float(v)
    return f"{v/1e6:.2f}M" if v >= 1e6 else f"{v/1e3:.0f}k"

notes = {
    1301427: "18.9-year-old left winger out of Banfield with the strongest combined score; consistent and composed, fits the LW gap exactly and sits in the U22 window.",
    1255754: "IFK Göteborg left-back, 29 Allsvenskan starts last season; high natural fitness and teamwork, fills the old LB spot at 20.8.",
    1421514: "AIK central midfielder, determination 14; cheap, high-yield, U22 CM depth for the bridge pipeline.",
    1283982: "Gimnasia left winger, balanced mental profile; second LW option, U22 initiative eligible.",
    1083702: "Istra 1961 left-back, 13 HNL starts at 20.3; high work rate and fitness, second LB option.",
    697280: "West Ham academy product (on loan, 10 Championship apps); versatile AM with good composure; the AM/10 slot behind the forwards.",
    796300: "Tottenham loanee at Aberdeen, CB with determination 15; premium long-term CB asset at EUR 500k.",
    1342148: "17.9-year-old Mjällby central midfielder; youngest CM in the list, fits the U22 window perfectly.",
    911224: "Roma youth product on loan at Spezia (19 Serie B apps); high ImportantMatches, DM depth at 19.9.",
    1058353: "AZ's 18.4-year-old CB, 11 Eerste Divisie starts; determination 15, archetype-adjacent, cheap CB option.",
    937720: "Benfica B central midfielder, 22 Liga Portugal 2 apps; high natural fitness and consistency.",
    1285240: "Platense DM, 11 LPF starts; team-first profile (teamwork 14) for the holding slot.",
    979850: "Cagliari youth CB, 30 Primavera apps with a 7.02 rating; ImportantMatches 15.",
    990008: "AZ left winger, 20 Eerste Divisie apps with 4 goals and 3 assists (7.08 rating); direct LW cover.",
    801446: "Benfica B goalkeeper, 26 Liga Portugal 2 apps at 6.85; long-term GK succession behind Gillier.",
    1138260: "17.5-year-old Young Boys U21 left-back already with 7 Super League apps; the youngest LB prospect.",
    1133013: "Lokomotiva Zagreb DM, 32 HNL apps; versatile with high composure, DM depth.",
    930768: "Inter youth striker on loan at Mantova, 6 goals in 26 Serie C starts; U22 CF option despite CF stock.",
    1006939: "Kristiansund attacking midfielder, 16 Eliteserien apps; highest psych_fit in the list (0.704).",
    866250: "Genoa winger on loan at Frosinone, 1 goal and 1 assist in 8 Serie B apps at 7.06 rating; RW succession.",
}
notes = {int(k): v for k, v in notes.items()}

for pid in [int(x) for x in t["player_id"]]:
    p = per[pid]
    top_ps = ", ".join(f"{k} {v:.0f}" for k, v in p["toppsych"].items())
    add(f"### {p['name']} (#{int(t.loc[t.player_id==pid,'rank'].iloc[0])})")
    add("")
    add(f"- **Club/league:** {p['club']} ({p['comp']}, {p['country']}), {p['pos']}/{p['sub']}, age {p['age']:.1f}.")
    add(f"- **Value & FM:** {fmteu(p['mv'])} | CA {p['ca']} / PA {p['pa']}.")
    add(f"- **Psych profile:** fit {p['psych']:.3f} (top attrs: {top_ps}).")
    add(f"- **Yield/spike:** 12m yield {p['yieldpct']:.1f}% | tail score {p['tail']:.2f}.")
    add(f"- **Squad gap filled:** {p['gap']} (squad_fit {p['sfit']}).")
    ga = p["assists"]
    ga = int(ga) if isinstance(ga, (int, float)) and not isinstance(ga, bool) else ga
    ss_line = f"SofaScore {p['rating']} ({p['stourn']}, {p['sseason']}): {p['goals']}G, {ga}A, {p['apps']} apps."
    add(f"- **Performance:** {ss_line} FBref: unavailable (Cloudflare 403).")
    add(f"- **Monte Carlo (spike run, seed 42):** median {eur(p['med_s'])} | p10 {eur(p['p10_s'])} | "
        f"p90 {eur(p['p90_s'])} | P(>=1.5x) {p['p15_s']:.0%} | P(>=2x) {p['p2_s']:.0%} | P(>=3x) {p['p3_s']:.0%} | "
        f"downside {p['down_s']:.0%}. Base run (forecast drift) median {eur(p['med_b'])}.")
    add(f"- **Rationale:** {notes[pid]}")
    add("")
    rk = int(t.loc[t.player_id == pid, "rank"].iloc[0])
    add(f"Fan charts: `artifacts/cfm_mc/charts/chart_player_{rk:02d}.png` (base) and "
        f"`artifacts/cfm_mc_spike/charts/chart_player_{rk:02d}.png` (spike); overview: "
        f"`chart_overview.png` in each directory.")
    add("")
    add("---")
    add("")

add("## 7. Honest limitations")
add("")
add("- **Hidden personality coverage is partial (~32% of the panel; 0 values are FM "
    "placeholders and treated as missing).** The archetype's hidden component therefore "
    "relies on the documented bridge-club prior, not on alumni hidden data. All top-20 "
    "players do carry real values for the four hidden attrs used (Pressure, "
    "Professionalism, Temperament, Controversy) where present.")
add("- **The alumni calibration set is small (5 players).** It is the best available "
    "evidence for a club of this size, but psych-fit differences of a few hundredths "
    "should not be over-read.")
add("- **FBref coverage is 0/20** (Cloudflare 403; max-attempts rule respected). "
    "SofaScore covers 20/20 with ratings but the underlying season lines vary in "
    "competition level (youth leagues, loan clubs, second tiers), which is stated per "
    "player.")
add("- **Data vintage mismatch:** FM26 attributes reflect approximately November 2025; TM "
    "valuations run through 2026-06-12; SofaScore statistics are live. Loan moves after "
    "the FM/TM vintage (Spezia, Mantova, Frosinone, Championship, League Two) are visible "
    "in SofaScore but not in the TM transfers table.")
add("- **Tail scores are model outputs, not guarantees.** The expectile-0.9 drift is the "
    "spike scenario: P(>=1.5x) near 100% under that regime describes what happens *if* "
    "the spike materializes, not a probability-weighted forecast. Base-run medians are the "
    "conservative comparison.")
add("- **Single-window model scores are not guarantees** of future transfer outcomes; "
    "they rank candidates within this basket and vintage only.")
add("")
add("---")
add("")
add("*Generated by the CFM scouting pipeline (STEERING_CFM.md). Seed 42; models unchanged "
    "(hashes in section 5).*")
add("")

open(f"{ROOT}/data/cfm_scout_report.md", "w").write("\n".join(L))
print("wrote", f"{ROOT}/data/cfm_scout_report.md", "lines:", len(L))
