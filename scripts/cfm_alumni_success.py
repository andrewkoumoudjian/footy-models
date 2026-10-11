import duckdb

con = duckdb.connect('/home/drewclaw/transfermarkt/transfermarkt-datasets.duckdb', read_only=True)

# Alumni: players who left CF Montreal for a fee. Track their CURRENT market value + club.
print('=== CFM alumni: transfer fee -> current valuation (success evidence) ===')
rows = con.execute("""
SELECT t.player_id, t.player_name, t.transfer_date, t.to_club_name, t.transfer_fee,
       p.market_value_in_eur AS current_mv, p.current_club_name AS current_club,
       p.current_club_domestic_competition_id AS comp
FROM transfers t
JOIN players p ON p.player_id = t.player_id
WHERE t.from_club_id = 4078 AND t.transfer_fee > 0
ORDER BY t.transfer_fee DESC NULLS LAST
""").fetchall()
for r in rows:
    fee = r[4]
    mv = r[5]
    growth = (mv / fee - 1) if fee and mv else None
    print(f"{r[1]:<24} fee €{fee:>10,.0f} now €{mv if mv is not None else 0:>12,.0f} growth {growth*100:.0f}%  {r[6]} ({r[7]})")
