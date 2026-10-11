import csv, json, sys

# Load hidden personality from people_v3.jsonl by uid
hidden_by_uid = {}
with open('/home/drewclaw/fm26-data/work/people_v3.jsonl') as f:
    for line in f:
        r = json.loads(line)
        h = r.get('hidden')
        if h and r.get('uid') is not None:
            hidden_by_uid[r['uid']] = h

alumni_ids = {921655, 637446, 484756, 842420, 568179, 585769, 589006, 742297, 401362, 441738, 170904, 367429, 693412, 749407, 471004, 424993, 329713, 189014}

# Read fm_tm_joined.csv, find rows with these player_ids
with open('/home/drewclaw/footy-models/data/fm_tm_joined.csv') as f:
    reader = csv.DictReader(f)
    for row in reader:
        pid = row.get('player_id', '').strip()
        if pid and int(pid) in alumni_ids:
            uid = int(row.get('uid', 0))
            hidden = hidden_by_uid.get(uid, {})
            print(f"{row.get('name_tm','')} | uid {uid} | ca {row.get('ca')} pa {row.get('pa')} | pos {row.get('pos_cls')} | mv {row.get('current_mv')} | attr_src {row.get('attr_source')}")
            print(f"   hidden: {hidden}")
