#!/usr/bin/env python3
"""Extract Transfermarkt features for value-surge modeling."""
import duckdb, os

OUT = "/home/drewclaw/footy-models/data"
os.makedirs(OUT, exist_ok=True)
con = duckdb.connect("/home/drewclaw/transfermarkt/transfermarkt-datasets.duckdb", read_only=True)

# 1) players with current market value
con.execute(f"""
COPY (
    SELECT p.player_id, p.name, p.first_name, p.last_name, p.date_of_birth,
           p.position, p.sub_position, p.foot, p.height_in_cm,
           p.market_value_in_eur AS current_mv,
           p.highest_market_value_in_eur,
           p.current_club_id, p.current_club_name,
           p.current_club_domestic_competition_id,
           p.last_season, p.international_caps, p.international_goals,
           p.contract_expiration_date, p.agent_name
    FROM players p
) TO '{OUT}/tm_players.csv' (FORMAT CSV, HEADER)
""")
print("players:", con.execute("SELECT COUNT(*) FROM players").fetchone()[0])

# 2) latest valuation date per player + mv 6m/12m ago
con.execute(f"""
COPY (
    WITH latest AS (
        SELECT player_id, MAX(date) AS max_date FROM player_valuations GROUP BY player_id
    ),
    cur AS (
        SELECT pv.player_id, pv.market_value_in_eur AS latest_mv, pv.date AS latest_date,
               pv.current_club_name AS val_club
        FROM player_valuations pv JOIN latest l ON pv.player_id = l.player_id AND pv.date = l.max_date
    ),
    back6 AS (
        SELECT pv.player_id, pv.market_value_in_eur AS mv_6m
        FROM player_valuations pv
        JOIN (SELECT player_id, MAX(date) d FROM player_valuations WHERE date <= DATE '2025-12-12' GROUP BY player_id) b
          ON pv.player_id = b.player_id AND pv.date = b.d
    ),
    back12 AS (
        SELECT pv.player_id, pv.market_value_in_eur AS mv_12m
        FROM player_valuations pv
        JOIN (SELECT player_id, MAX(date) d FROM player_valuations WHERE date <= DATE '2025-06-12' GROUP BY player_id) b
          ON pv.player_id = b.player_id AND pv.date = b.d
    ),
    first AS (
        SELECT pv.player_id, MIN(date) AS first_date, COUNT(*) AS n_valuations
        FROM player_valuations pv GROUP BY player_id
    )
    SELECT c.player_id, c.latest_mv, c.latest_date, c.val_club,
           COALESCE(b6.mv_6m, -1) AS mv_6m, COALESCE(b12.mv_12m, -1) AS mv_12m,
           f.first_date, f.n_valuations
    FROM cur c
    LEFT JOIN back6 b6 ON c.player_id = b6.player_id
    LEFT JOIN back12 b12 ON c.player_id = b12.player_id
    LEFT JOIN first f ON c.player_id = f.player_id
) TO '{OUT}/tm_valuations_summary.csv' (FORMAT CSV, HEADER)
""")
print("valuations:", con.execute("SELECT COUNT(*) FROM player_valuations").fetchone()[0])

# 3) appearances aggregated by season (last 3 seasons + career)
con.execute(f"""
COPY (
    SELECT player_id, season, n_apps, goals, assists, minutes_played FROM (
        SELECT a.player_id,
               CASE WHEN strftime(a.date, '%m') >= '07' THEN strftime(a.date, '%Y') || '/' || CAST(CAST(strftime(a.date,'%Y') AS INT)+1 AS VARCHAR)
                    ELSE CAST(CAST(strftime(a.date,'%Y') AS INT)-1 AS VARCHAR) || '/' || strftime(a.date, '%Y') END AS season,
               COUNT(*) AS n_apps, SUM(a.goals) AS goals, SUM(a.assists) AS assists, SUM(a.minutes_played) AS minutes_played
        FROM appearances a
        WHERE a.date >= DATE '2023-07-01'
        GROUP BY a.player_id, season
    ) ORDER BY player_id, season
) TO '{OUT}/tm_appearances.csv' (FORMAT CSV, HEADER)
""")
print("appearances:", con.execute("SELECT COUNT(*) FROM appearances").fetchone()[0])

# 4) transfers (most recent per player)
con.execute(f"""
COPY (
    SELECT player_id, transfer_date, transfer_season, from_club_name, to_club_name,
           transfer_fee, market_value_in_eur AS mv_at_transfer,
           ROW_NUMBER() OVER (PARTITION BY player_id ORDER BY transfer_date DESC) AS rn
    FROM transfers
) TO '{OUT}/tm_transfers_all.csv' (FORMAT CSV, HEADER)
""")
print("transfers:", con.execute("SELECT COUNT(*) FROM transfers").fetchone()[0])

# 5) clubs + competitions
con.execute(f"COPY (SELECT * FROM clubs) TO '{OUT}/tm_clubs.csv' (FORMAT CSV, HEADER)")
con.execute(f"COPY (SELECT * FROM competitions) TO '{OUT}/tm_competitions.csv' (FORMAT CSV, HEADER)")
print("done")
