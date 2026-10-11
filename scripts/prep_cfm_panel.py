"""Pre-merge the CFM scout psych panel (SAFE PRE-MERGE — RLM must NOT re-merge).

Joins:
  A. data/fm_tm_joined.csv  -> TM overlay + CA/PA + sigs (player_id keyed, 14,783 rows)
  B. /home/drewclaw/fm26-data/fm26_men_full_attrs.csv -> full 54 attrs by uid
  C. /home/drewclaw/fm26-data/work/people_v3.jsonl -> hidden personality (8 attrs) by uid
  D. forecast/spike scores from data/forecast_36m_all.csv + data/demo/candidates_v3_tail.csv

Output: data/cfm_scout_panel.csv (one row per joined player with all psych + value + forecast cols)
"""
import csv, json, sys

FM_TM = '/home/drewclaw/footy-models/data/fm_tm_joined.csv'
ATTRS = '/home/drewclaw/fm26-data/fm26_men_full_attrs.csv'
HIDDEN = '/home/drewclaw/fm26-data/work/people_v3.jsonl'
OUT = '/home/drewclaw/footy-models/data/cfm_scout_panel.csv'

# --- C: hidden personality by uid (keep last non-empty) ---
hidden = {}
with open(HIDDEN) as f:
    for line in f:
        r = json.loads(line)
        h = r.get('hidden')
        uid = r.get('uid')
        if h and uid is not None:
            hidden[int(uid)] = h
print(f'hidden personality: {len(hidden)} uids', file=sys.stderr)

# --- B: full attrs by uid ---
attrs = {}
with open(ATTRS) as f:
    rdr = csv.DictReader(f)
    for row in rdr:
        attrs[row['uid'].strip()] = row
print(f'full attrs: {len(attrs)} uids', file=sys.stderr)

# --- A: TM joined, keyed by player_id and uid ---
tm_by_pid = {}
tm_by_uid = {}
with open(FM_TM) as f:
    rdr = csv.DictReader(f)
    for row in rdr:
        pid = (row.get('player_id') or '').strip()
        uid = (row.get('uid') or '').strip()
        if pid: tm_by_pid[pid] = row
        if uid: tm_by_uid[uid] = row
print(f'TM joined: {len(tm_by_pid)} by player_id, {len(tm_by_uid)} by uid', file=sys.stderr)

# --- D: forecast_36m + tail scores by player_id ---
f36 = {}
with open('/home/drewclaw/footy-models/data/forecast_36m_all.csv') as f:
    rdr = csv.DictReader(f)
    for row in rdr:
        f36[row['player_id'].strip()] = row
tail = {}
with open('/home/drewclaw/footy-models/data/demo/candidates_v3_tail.csv') as f:
    rdr = csv.DictReader(f)
    for row in rdr:
        tail[row['player_id'].strip()] = row
print(f'forecast_36m: {len(f36)}, tail: {len(tail)}', file=sys.stderr)

# --- Merge on uid (primary) ---
rows = []
seen = set()
for uid, tmrow in tm_by_uid.items():
    if uid in seen: continue
    seen.add(uid)
    arow = attrs.get(uid, {})
    hrow = hidden.get(int(uid), {}) if uid else {}
    pid = tmrow.get('player_id', '')
    frow = f36.get(pid, {})
    trow = tail.get(pid, {})
    out = {
        'uid': uid,
        'name': tmrow.get('name_tm') or tmrow.get('name', ''),
        'player_id': pid,
        'club': tmrow.get('current_club_name', ''),
        'competition': tmrow.get('competition_name', ''),
        'country': tmrow.get('country_name', ''),
        'pos_cls': tmrow.get('pos_cls', ''),
        'age': tmrow.get('age', ''),
        'current_mv': tmrow.get('current_mv', ''),
        'fm_ca': tmrow.get('ca', ''),
        'fm_pa': tmrow.get('pa', ''),
        'attr_source': tmrow.get('attr_source', ''),
    }
    # full attrs (mental + all)
    for k, v in arow.items():
        if k.startswith('attr_'):
            out[k] = v
    # hidden personality
    for k in ['Adaptability','Ambition','Loyalty','Pressure','Professionalism','Sportsmanship','Temperament','Controversy']:
        out['psy_' + k] = hrow.get(k, '')
    # forecast + tail
    for k in ['predicted_12m_mv','gap_pct','gap_36m_pct','mu_12','predicted_36m_mv','interval_10','interval_90']:
        if k in frow: out['f36_' + k] = frow[k]
    for k in ['tail_score','predicted_12m_mv','gap_pct']:
        if k in trow: out['tail_' + k] = trow[k]
    rows.append(out)

print(f'merged rows: {len(rows)}', file=sys.stderr)
if rows:
    fieldnames = list(rows[0].keys())
    with open(OUT, 'w', newline='') as f:
        w = csv.DictWriter(f, fieldnames=fieldnames)
        w.writeheader()
        for r in rows:
            w.writerow({k: r.get(k, '') for k in fieldnames})
    print('WROTE', OUT, file=sys.stderr)
else:
    print('NO ROWS', file=sys.stderr)
