import duckdb

con = duckdb.connect('/home/drewclaw/transfermarkt/transfermarkt-datasets.duckdb', read_only=True)
rows = con.execute("""
SELECT player_id, player_name, transfer_date, to_club_name, transfer_fee, market_value_in_eur
FROM transfers
WHERE from_club_id = 4078 AND transfer_fee > 0
ORDER BY transfer_fee DESC NULLS LAST
""").fetchall()
for r in rows:
    print(r[0], '|', r[1], '|', r[2], '|', r[3], '|', r[4], '|', r[5])
