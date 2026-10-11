#!/usr/bin/env python3
"""Reproducible Premier League pre-match forecast: source/normalize, refit, validate, predict.

Only completed, pre-cutoff matches enter fitting. No bookmakers or post-fixture features.
Source: football-data.co.uk historical E0/E1 match results. Downloaded CSV is local cache
and excluded from Git. Match-specific as-of results not yet in the upstream CSV can be
supplied explicitly in the fixture JSON, with an auditable source URL.
"""
import argparse
import csv
from datetime import date, datetime
import io
import json
from pathlib import Path
import subprocess
import sys
from urllib.request import Request, urlopen

ROOT = Path(__file__).resolve().parents[1]
RAW = ROOT / 'data' / 'pl' / 'raw'
OUT = ROOT / 'data' / 'pl'
ALIASES = {"Man United": "Man United", "Man City": "Man City", "Nott'm Forest": "Nott'm Forest",
           "Nottingham Forest": "Nott'm Forest", "Hull City": "Hull", "Coventry City": "Coventry",
           "Manchester City": "Man City", "Manchester United": "Man United", "Tottenham Hotspur": "Tottenham",
           "Brighton & Hove Albion": "Brighton", "Newcastle United": "Newcastle"}

def canonical(team):
    return ALIASES.get(team.strip(), team.strip())

def csv_rows(season, division, offline):
    RAW.mkdir(parents=True, exist_ok=True)
    dest = RAW / f'{season}_{division}.csv'
    url = f'https://www.football-data.co.uk/mmz4281/{season}/{division}.csv'
    if not offline:
        try:
            req = Request(url, headers={'User-Agent': 'FootyModels/0.1 (academic research)'})
            with urlopen(req, timeout=20) as response:
                content = response.read()
            if len(content) < 1000 or not content.lstrip(b'\xef\xbb\xbf').startswith(b'Div,'):
                raise ValueError('unexpected football-data CSV response')
            dest.write_bytes(content)
        except Exception as exc:
            if not dest.exists():
                raise RuntimeError(f'cannot retrieve {url}: {exc}') from exc
            print(f'WARNING: using cached {dest}, fetch failed: {exc}', file=sys.stderr)
    if not dest.exists():
        raise FileNotFoundError(f'offline cache missing: {dest}')
    text = dest.read_text(encoding='utf-8-sig')
    rows = list(csv.DictReader(io.StringIO(text)))
    if not {'Date','HomeTeam','AwayTeam','FTHG','FTAG'}.issubset(rows[0]):
        raise ValueError(f'unrecognized schema: {dest}')
    return rows, url

# FM26 + Transfermarkt club mapping is a June-2026 snapshot (not live team sheets).
# Do not reuse a future snapshot for pre-2026 backtests and do not silently
# supplement missing teams with fuzzy name joins. A missing club gets no FM prior.
FM_CLUBS = {
    'Crystal Palace': 'Crystal Palace', "Nott'm Forest": 'Nottingham Forest',
    'Hull': 'Hull City', 'Everton': 'Everton FC', 'Liverpool': 'Liverpool FC',
    'Man City': 'Manchester City', 'Coventry': 'Coventry City', 'Newcastle': 'Newcastle United',
    'Arsenal':'Arsenal FC', 'Leeds':'Leeds United', 'Aston Villa':'Aston Villa',
    'Brentford':'Brentford FC', 'Chelsea':'Chelsea FC', 'Bournemouth':'AFC Bournemouth',
    'Ipswich':'Ipswich Town', 'Fulham':'Fulham FC', 'Sunderland':'Sunderland AFC',
    'Brighton':'Brighton & Hove Albion', 'Man United':'Manchester United', 'Tottenham':'Tottenham Hotspur'
}

def build_fm_strengths(cutoff, clubs):
    # A pre-season June 2026 snapshot is applied only to late-August 2026 onwards.
    # Transfermarkt merged roster is stale after summer transfers; coefficients
    # shrink toward 0 when fewer than 15 players are resolved reliably.
    if not date(2026, 8, 20) <= cutoff < date(2027, 7, 1):
        return {}, {'error':'FM26 June 2026 snapshot only valid for the 2026/27 season; supply a new squad snapshot for another season'}
    with (ROOT / 'data' / 'fm_tm_joined.csv').open(encoding='utf-8') as file:
        players = list(csv.DictReader(file))
    requested = {club: FM_CLUBS.get(club, club) for club in clubs}
    by_club = {club:[] for club in clubs}
    for row in players:
        for club, official in requested.items():
            if row.get('current_club_name') != official: continue
            if row.get('attr_source') not in {'db','derived'}:continue
            try:
                ca=float(row['ca'])
                if ca < 40: continue
                if not row['latest_val_date'] or row['latest_val_date'] < '2025-12-01':continue
                def f(key, fallback):
                    try: return float(row[key]) if row[key] else fallback
                    except (KeyError,ValueError):return fallback
                pos=row.get('pos_cls')
                attack_signal=(f('sig_attacking',11)+f('sig_creative',11))/2
                defense_signal=(f('sig_defensive',11)+f('sig_mental',11))/2
                if pos=='GK':
                    attack_signal=11.0
                    defense_signal=13.0
                # CA is the stronger player-ability prior; FM role composites add
                # directional attack/defence information without using PA as CA.
                attack = 0.73*(ca-130)/22 + 0.27*(attack_signal-11)/3
                defense = 0.73*(ca-130)/22 + 0.27*(defense_signal-11)/3
                confidence = 1.0 if row['attr_source']=='db' else 0.4
                by_club[club].append({'name':row['name'],'ca':ca,'pa':f('pa',ca),
                                      'position':pos,'source':row['attr_source'],
                                      'attack':attack,'defense':defense,'confidence':confidence})
            except (TypeError, ValueError): continue
    result={};info={};raw_a=[];raw_d=[]
    for club in sorted(clubs):
        # Top 15 CA approximates squad quality, NOT a starting lineup.
        squad=sorted(by_club.get(club,[]),key=lambda p:(-p['ca'],p['name']))[:15]
        denom=sum(p['confidence'] for p in squad)
        if denom:
            attack=sum(p['confidence']*p['attack'] for p in squad)/denom
            defense=sum(p['confidence']*p['defense'] for p in squad)/denom
        else: attack=defense=0.0
        result[club]=[attack,defense,min(1.0,denom/15.0)]
        info[club]={'club_mapping':requested.get(club),'eligible_player_count':len(by_club.get(club,[])),
                    'top15_count':len(squad),'confidence_weighted_coverage':round(min(1,denom/15),3),
                    'top_players':[{'name':p['name'],'ca':p['ca'],'source':p['source']} for p in squad[:5]]}
        if denom >=10:
            raw_a.append(attack);raw_d.append(defense)
    a_mid=sum(raw_a)/len(raw_a) if raw_a else 0
    d_mid=sum(raw_d)/len(raw_d) if raw_d else 0
    a_sd=max(0.5,(sum((v-a_mid)**2 for v in raw_a)/len(raw_a))**0.5) if raw_a else 1.0
    d_sd=max(0.5,(sum((v-d_mid)**2 for v in raw_d)/len(raw_d))**0.5) if raw_d else 1.0
    for club, (a,d,coverage) in result.items():
        if coverage==0: result[club]=[0.0,0.0,0.0]
        else: result[club]=[max(-2,min(2,(a-a_mid)/a_sd))*coverage,
                           max(-2,min(2,(d-d_mid)/d_sd))*coverage,coverage]
    return result,info

def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--fixtures',default='configs/pl_2026-10-11.json',help='JSON config with as_of, fixtures, optional manual_results')
    ap.add_argument('--as-of',help='override as_of date from fixture config')
    ap.add_argument('--offline',action='store_true',help='use saved source CSV cache (no HTTP)')
    ap.add_argument('--release',action='store_true',help='compile optimized Rust binary')
    ap.add_argument('--output',default='data/pl/forecasts.json')
    args=ap.parse_args()
    config=json.loads((ROOT/args.fixtures).read_text())
    cutoff=date.fromisoformat(args.as_of or config['as_of'])
    fixtures=config['fixtures']
    if not fixtures: raise ValueError('at least one fixture required')
    for fx in fixtures:
        fx['home'],fx['away']=canonical(fx['home']),canonical(fx['away'])
        if date.fromisoformat(fx['date']) <= cutoff: raise ValueError('fixture must be after as_of: '+str(fx))
    observed={}; origins={}
    season_start = cutoff.year if cutoff.month >= 7 else cutoff.year - 1
    seasons = [f'{y % 100:02d}{(y+1) % 100:02d}' for y in range(season_start-3, season_start+1)]
    championship_season = f'{(season_start-1) % 100:02d}{season_start % 100:02d}'
    for season in seasons:
        for division in ('E0','E1') if season == championship_season else ('E0',):
            rows,url=csv_rows(season,division,args.offline)
            for row in rows:
                if not row['FTHG'].strip() or not row['FTAG'].strip():continue
                match_date=datetime.strptime(row['Date'], '%d/%m/%Y').date()
                if match_date > cutoff: continue
                home,away=canonical(row['HomeTeam']),canonical(row['AwayTeam'])
                key=(division,match_date.isoformat(),home,away)
                goals=(int(row['FTHG']),int(row['FTAG']))
                if any(n<0 or n>15 for n in goals): raise ValueError(f'impossible goals: {key}')
                observed[key]=goals; origins[key]=url
    for row in config.get('manual_results',[]):
        day=date.fromisoformat(row['date'])
        if day > cutoff: continue
        home,away=canonical(row['home']),canonical(row['away'])
        key=('E0',day.isoformat(),home,away)
        goals=(int(row['home_goals']),int(row['away_goals']))
        if key in observed and observed[key] != goals:
            raise ValueError(f'conflicting official result {key}: upstream {observed[key]} vs override {goals}')
        observed[key]=goals
        origins[key]=config.get('manual_results_source','manual (no link)')
    overlap={('E0',fx['date'],fx['home'],fx['away']) for fx in fixtures} & set(observed)
    if overlap: raise ValueError(f'prediction leakage: completed fixture in training: {overlap}')
    OUT.mkdir(parents=True,exist_ok=True)
    result_file=OUT/'results.tsv'
    with result_file.open('w') as output:
        output.write('day\tdivision\thome\taway\thome_goals\taway_goals\n')
        for key,goals in sorted(observed.items(),key=lambda item:(item[0][1],item[0][0],item[0][2])):
            division,day,home,away=key
            output.write(f'{(date.fromisoformat(day)-date(1970,1,1)).days}\t{division}\t{home}\t{away}\t{goals[0]}\t{goals[1]}\n')
    fm_clubs={team for key in observed if key[0]=='E0' and key[1]>='2026-08-01' for team in (key[2],key[3])}
    fm_clubs.update(team for fx in fixtures for team in (fx['home'],fx['away']))
    fm, fm_coverage=build_fm_strengths(cutoff,fm_clubs)
    if not fm:
        raise RuntimeError(f'No eligible FM26 club snapshot for {cutoff}: {fm_coverage}. Provide an updated snapshot and club map.')
    fm_file=OUT/'fm_strengths.tsv'
    with fm_file.open('w') as out:
        out.write('club\tattack\tdefense\tcoverage\n')
        for club, strength in sorted(fm.items()):
            out.write(f'{club}\t{strength[0]:.9f}\t{strength[1]:.9f}\t{strength[2]:.9f}\n')
    (OUT/'fm_coverage.json').write_text(json.dumps(fm_coverage,indent=2)+'\n')
    fixture_file=OUT/'fixtures.tsv'
    with fixture_file.open('w') as output:
        output.write('date\thome\taway\n')
        for fx in fixtures:output.write(f"{fx['date']}\t{fx['home']}\t{fx['away']}\n")
    output=ROOT/args.output
    output.parent.mkdir(parents=True,exist_ok=True)
    cmd=['cargo','run','--quiet']
    if args.release:cmd+=['--release']
    cmd+=['-p','footy-plforecast','--','--results',str(result_file),'--fixtures',str(fixture_file),
          '--as-of-day',str((cutoff-date(1970,1,1)).days),'--fm',str(fm_file),'--output',str(output)]
    print(f'Training on {sum(k[0]=="E0" for k in observed)} Premier League and {sum(k[0]=="E1" for k in observed)} Championship matches through {cutoff}',flush=True)
    subprocess.run(cmd, cwd=ROOT,check=True)
    metadata={'as_of':str(cutoff),'fixture_config':args.fixtures,'matches':len(observed),'source_urls':sorted(set(origins.values())),
              'leakage_policy':'completed matches dated at or before cutoff; no bookmaker odds, no future dates; FM26 June 2026 snapshot only used on August 2026 and later observations; one forecast per future fixture'}
    (OUT/'provenance.json').write_text(json.dumps(metadata,indent=2)+'\n')
    print(output.read_text())

if __name__=='__main__':main()
