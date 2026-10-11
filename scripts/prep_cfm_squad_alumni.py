"""Pre-merge CFM scouting inputs (SAFE PRE-MERGE — RLM must NOT re-merge).

1. data/cfm_squad.tsv    — current CFM squad (club 4078) + FM attrs where joinable
2. data/cfm_alumni.tsv   — alumni transfers out of CFM (fee>0) + current valuation + FM psych attrs
"""
import csv, json, sys
import duckdb

TM_DB = '/home/drewclaw/transfermarkt/transfermarkt-datasets.duckdb'
PANEL = '/home/drewclaw/footy-models/data/cfm_scout_panel.csv'
OUT_SQUAD = '/home/drewclaw/footy-models/data/cfm_squad.tsv'
OUT_ALUMNI = '/home/drewclaw/footy-models/data/cfm_alumni.tsv'

con = duckdb.connect(TM_DB, read_only=True)

# ---- squad ----
squad = con.execute("""
SELECT p.player_id, p.name, p.position, p.sub_position, p.date_of_birth, p.market_value_in_eur
FROM players p WHERE p.current_club_id = '4078'
ORDER BY p.market_value_in_eur DESC NULLS LAST
""").fetchall()
print(f'squad rows: {len(squad)}', file=sys.stderr)

# ---- alumni ----
alumni = con.execute("""
SELECT t.player_id, t.player_name, t.transfer_date, t.to_club_name, t.transfer_fee,
       p.market_value_in_eur AS current_mv, p.current_club_name AS current_club,
       p.current_club_domestic_competition_id AS comp
FROM transfers t JOIN players p ON p.player_id = t.player_id
WHERE t.from_club_id = 4078 AND t.transfer_fee > 0
ORDER BY t.transfer_fee DESC NULLS LAST
""").fetchall()
print(f'alumni rows: {len(alumni)}', file=sys.stderr)

# ---- panel for psych attrs by player_id ----
panel = {}
with open(PANEL) as f:
    for r in csv.DictReader(f):
        panel[r['player_id'].strip()] = r
print(f'panel: {len(panel)}', file=sys.stderr)

with open(OUT_SQUAD, 'w', newline='') as f:
    w = csv.writer(f, delimiter='\t')
    w.writerow(['player_id','name','position','sub_position','dob','market_value_eur'])
    for row in squad:
        w.writerow(row)

with open(OUT_ALUMNI, 'w', newline='') as f:
    w = csv.writer(f, delimiter='\t')
    w.writerow(['player_id','player_name','transfer_date','to_club','transfer_fee','current_mv','current_club','comp',
                'uid','fm_ca','fm_pa','pos_cls','attr_source',
                'psy_Pressure','psy_Professionalism','psy_Ambition','psy_Adaptability','psy_Temperament','psy_Controversy','psy_Sportsmanship','psy_Loyalty',
                'attr_Determination','attr_Bravery','attr_Consistency','attr_ImportantMatches','attr_Composure','attr_WorkRate','attr_Teamwork','attr_Leadership'])
    for row in alumni:
        pid = str(row[0])
        p = panel.get(pid, {})
        w.writerow([*row, p.get('uid',''), p.get('fm_ca',''), p.get('fm_pa',''), p.get('pos_cls',''), p.get('attr_source',''),
                    p.get('psy_Pressure',''), p.get('psy_Professionalism',''), p.get('psy_Ambition',''), p.get('psy_Adaptability',''),
                    p.get('psy_Temperament',''), p.get('psy_Controversy',''), p.get('psy_Sportsmanship',''), p.get('psy_Loyalty',''),
                    p.get('attr_Determination',''), p.get('attr_Bravery',''), p.get('attr_Consistency',''), p.get('attr_ImportantMatches',''),
                    p.get('attr_Composure',''), p.get('attr_WorkRate',''), p.get('attr_Teamwork',''), p.get('attr_Leadership','')])
print('WROTE', OUT_SQUAD, OUT_ALUMNI, file=sys.stderr)
