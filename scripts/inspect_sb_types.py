import json
from collections import Counter

d = json.load(open('data/statsbomb/events/3754159.json'))
c = Counter(e['type']['name'] for e in d)
print(c.most_common(30))
