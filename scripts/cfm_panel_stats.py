import csv

path = '/home/drewclaw/footy-models/data/cfm_scout_panel.csv'
with open(path) as f:
    rdr = list(csv.DictReader(f))

n = len(rdr)
psy_cols = ['psy_Pressure','psy_Professionalism','psy_Ambition','psy_Adaptability','psy_Temperament','psy_Controversy','psy_Sportsmanship','psy_Loyalty']
def f(x):
    try: return float(x)
    except: return None

# coverage of hidden personality (non-zero, non-empty)
covered = sum(1 for r in rdr if f(r.get('psy_Pressure')) is not None and f(r.get('psy_Pressure')) > 0)
print(f'total {n}, hidden personality present: {covered} ({covered/n*100:.1f}%)')

# how many have mental attrs (Determination, Bravery, Consistency, ImportantMatches, Composure)
for col in ['attr_Determination','attr_Bravery','attr_Consistency','attr_ImportantMatches','attr_Composure','attr_WorkRate','attr_Teamwork']:
    c = sum(1 for r in rdr if f(r.get(col)) is not None and f(r.get(col)) > 0)
    print(f'{col}: {c} ({c/n*100:.1f}%)')

# how many are in the U23 + 400k-1.2M basket
basket = [r for r in rdr if f(r.get('age')) is not None and f(r.get('age')) < 23 and f(r.get('current_mv')) is not None and 400000 <= f(r.get('current_mv')) <= 1200000]
print(f'basket (age<23, 400k-1.2M): {len(basket)}')

# with hidden personality inside basket
basket_psy = [r for r in basket if f(r.get('psy_Pressure')) is not None and f(r.get('psy_Pressure')) > 0]
print(f'basket with hidden psy: {len(basket_psy)}')

# mental attrs in basket
for col in ['attr_Determination','attr_Consistency','attr_ImportantMatches','attr_Composure']:
    c = sum(1 for r in basket if f(r.get(col)) is not None and f(r.get(col)) > 0)
    print(f'basket {col}: {c} ({c/len(basket)*100:.1f}%)')
