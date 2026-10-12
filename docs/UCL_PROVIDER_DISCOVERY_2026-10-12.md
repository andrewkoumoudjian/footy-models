# UCL provider discovery + immutable cache — 2026-10-12

**Status: executed, research-only.** Exact league/season codes were discovered
with the `soccerdata` package itself (1.9.1, `pip install soccerdata`), never
guessed. Every raw response is cached immutably with SHA-256 under
`data/ucl/raw_espn/` (gitignored by design; `SHA256SUMS` files alongside).
Multiple providers is the point: five contribute, five gaps are recorded as
gaps, not worked around with guesses.

## 1. soccerdata league codes (package-discovered, not guessed)

```
FBref.available_leagues()      = Big 5 European Leagues Combined, ENG-Premier
                                 League, ESP-La Liga, FRA-Ligue 1,
                                 GER-Bundesliga, INT-European Championship,
                                 INT-Women's World Cup, INT-World Cup,
                                 ITA-Serie A
ESPN.available_leagues()       = ENG, ESP, FRA, GER, ITA domestic only
Sofascore.available_leagues()  = ENG, ESP, FRA, GER, ITA + INT-European Ch'ship
WhoScored.available_leagues()  = ENG, ESP, FRA, GER, ITA + INT tournaments
Understat.available_leagues()  = ENG, ESP, FRA, GER, ITA
SoFIFA / MatchHistory          = same Big-5 domestic set
FotMob                         = not present in soccerdata at all
```

**No UEFA Champions League code exists in any soccerdata 1.9.1 provider.**
Consequences, each disclosed:

- **FBref standard/shooting/passing/defensive UCL tables: GAP.** `sd.FBref`
  cannot address UCL fixtures in this version, so no FBref UCL player-match
  labels enter any dataset. (Domestic Big-5 FBref for UCL clubs was NOT
  mixed in: cross-competition pooling without a translation model would be a
  silent input.)
- **SofaScore: GAP.** Only `read_league_table`/`read_schedule` exist in this
  version (no player-event readers) and leagues are Big-5-only.
- **WhoScored: GAP.** Has `read_events`/`read_schedule` but leagues are
  Big-5-only and collection needs a browser driver; UCL inaccessible.
- **FotMob: GAP.** Absent from the package entirely.
- **ClubElo: GAP (outage).** `sd.ClubElo().read_by_date('2026-10-12')` fails:
  `http://api.clubelo.com/2026-10-11` and `/2026-10-12` both return
  **HTTP 502** (verified twice, 2026-10-12 ~12:00 UTC, direct curl).
  ClubElo was in any case specified as an *evaluated prior candidate only*,
  never a silent input — with the API down there is nothing to evaluate, so
  no rating of any kind enters the forecast.

## 2. What each contributing provider supplied (all pre-cutoff)

| Provider | Artifact (sha256 prefix) | Content |
|---|---|---|
| ESPN scoreboard (`uefa.champions`, curl-frozen per-day boards, same pattern as the EPL path) | `raw_espn/board_*.json` (`SHA256SUMS` in dir) | MD1 2026-27 (18 completed, Sep 8–10), MD2/MD3 schedule (36 scheduled, Oct 13–21), 86 historical dates → **425 completed UCL fixtures** Feb 2024 – Sep 2026 |
| ESPN summaries (`apps/matchforecast/math_engine/espn_feed.py`, generalized with `--league/--dates`, EPL default unchanged) | `espn_ucl_md1_20261012.json` (`61b92860…`), `espn_ucl_202526_20261012.json` (`cb893b65…`, 188 matches / 33 dates, 1 rejected), `espn_ucl_202425_20261012.json` (`0eb524dd…`, 187 matches / 33 dates, 2 rejected) | 393 verified completed UCL player-match archives; raw summaries in `raw_espn/md1_summaries|season2526|season2425` |
| ESPN rosters (`scripts/freeze_ucl_rosters.py`, frozen 2026-10-12) | `rosters_2026-10-12.json` (`85146758…`), raw in `raw_espn/rosters/` | 36 clubs, **1,096 registrations** (registration source of record; UEFA publishes no single machine-readable list — recorded as a limitation) |
| Reep register (CC0, pinned `20261005T180536Z`, sha `f23f…8aaf`) | `squads_2026-10-12_canonical_identity_evidence.json` | 966/1,096 FM candidate UIDs via the canonical v3 matcher (typed `fm/player` bridges, TM DOB corroboration); statuses: 964 quarantined / 130 not-confident / 2 colliding |
| FM26 native exports (complete v6 + men provenance union) | via `load_fm_union` | 818/964 wanted UIDs present; 519 squad-verified + union-usable; **248 staff-db rescues** pass the MLS-parity gate |
| Transfermarkt (`data/tm_players.csv`) + Sortitoutsi roster/map | corroboration only | DOB/club/name evidence inside the canonical matcher; never traits, never ratings as inputs |

Rejected ESPN summaries (3 total: 1 in 2025-26, 2 in 2024-25) are kept in
each archive's `rejected_matches` with reasons; nothing was retried into
existence.

## 3. Vintage and cutoff discipline

- Forecast cutoff `--as-of 2026-10-12`. Every board, summary, roster, and
  history row strictly precedes it (same-day MD2 kickoffs Oct 13–14 are the
  forecast targets, never inputs).
- FM26 vintage (March 2026, v26.2) never applied before 2026-03-01: enforced
  by `--score-from-day 2026-03-01` in the FM-prior OOS slice and by the same
  rolling-origin guard the EPL path uses.
- No bookmaker inputs anywhere. Missing stays missing (397/415 team-history
  rows without measured shots; 341/1,096 cold-start player baselines; zeroed
  minutes are experience weights, never fabricated form). DNP rows are never
  zero events (unused-bench rows carry no exposure in every ESPN archive).
- Deterministic seeds throughout (simulation 42, bootstrap 20261012);
  refuse-overwrite artifacts everywhere.
