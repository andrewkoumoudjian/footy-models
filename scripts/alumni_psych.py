import json

targets = ["Kone", "Koné", "Johnston", "Mihailovic", "Saliba", "Choiniere", "Choinière", "Waterman", "Campbell", "Sirois"]
found = {}
with open('/home/drewclaw/fm26-data/work/people_v3.jsonl') as f:
    for line in f:
        r = json.loads(line)
        nm = r.get('name') or ''
        for t in targets:
            if t.lower() in nm.lower():
                key = (t, nm)
                found.setdefault(key, []).append(r)

for (t, nm), recs in sorted(found.items()):
    for r in recs[:3]:
        h = r.get('hidden') or {}
        print(f"{nm} | ca {r.get('ca')} pa {r.get('pa')} | hidden: {h}")
    print()
