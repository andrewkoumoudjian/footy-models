import json, sys
d = json.load(open('data/statsbomb/events/3754159.json'))
for e in d:
    if e['type']['name'] == 'Starting XI':
        print('team', e['team']['name'], 'formation', e.get('tactics', {}).get('formation'))
        for p in e['tactics']['lineup'][:4]:
            print(' ', p)
        break
shots = [e for e in d if e['type']['name'] == 'Shot']
print('shots:', len(shots))
s = shots[0]
print(json.dumps(s, indent=1)[:900])
