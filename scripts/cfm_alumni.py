import duckdb

con = duckdb.connect('/home/drewclaw/transfermarkt/transfermarkt-datasets.duckdb', read_only=True)

print('=== CF Montreal outgoing transfers (alumni) ===')
rows = con.execute("""
SELECT player_name, transfer_date, to_club_name, transfer_fee, market_value_in_eur
FROM transfers
WHERE from_club_name ILIKE '%m%ntr%al%'
  AND transfer_fee > 0
ORDER BY transfer_fee DESC NULLS LAST
LIMIT 40
""").fetchall()
for r in rows:
    print(r)

print()
print('=== CF Montreal current club ids ===')
print(con.execute("SELECT club_id, name FROM clubs WHERE name ILIKE '%m%ntr%al%'").fetchall())

print()
print('=== transfers by club_id 4078 ===')
rows = con.execute("""
SELECT player_name, transfer_date, to_club_name, transfer_fee, market_value_in_eur
FROM transfers
WHERE from_club_id = 4078
ORDER BY transfer_fee DESC NULLS LAST
LIMIT 30
""").fetchall()
for r in rows:
    print(r)
