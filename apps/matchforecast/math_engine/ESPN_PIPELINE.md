# ESPN observed-player input integration

**October 11, 2026 — RESEARCH ONLY.** This path is isolated under apps/matchforecast/math_engine and leaves the production forecast and previously frozen results unchanged.

## Data actually collected

Using the same ESPN soccer JSON data source family supported by probberechts/soccerdata, the standalone ESPN collector froze **56 completed Premier League fixtures over 18 dates** through October 10. The second archive includes the original **74 raw responses** (18 scoreboards, 56 match summaries), all checked by SHA-256. Normalized rows contain stable ESPN athlete IDs, 11 starters per team, observed appearances, on-field exposure, shots, goals, **496 substitutions**, and **8 red cards**. Send-off times reduce exposure instead of being counted as a full 90-minute match.

The official registration lists legal names in Surname, Given Middle Names format, which differs from ESPN display names. Entity reconciliation now requires a unique same-club exact full name or a unique same-club legal given-name AND surname-token pair. No approximate text distance, arbitrary identity selection, or FM current-ability tie-break is permitted.

| Crosswalk, eight candidate clubs | Count |
|---|---:|
| Original FM-matched squad candidates | 323 |
| FM candidates with matched ESPN source IDs | 152 |
| Additional official players without an FM match, but with verified ESPN identity | 23 |
| FM candidates without a uniquely verified ESPN ID | 194 |
| Total ESPN-observed player identities used across clubs | 175 |

Unmatched official registrations are not automatically converted into fictional players. The full modeled candidate pool is 323 FM-matched players plus 23 source-backed official additions — it is **not the complete original registered roster**. FM-derived shot/xG priors remain labeled as proxies whenever no measured source exists; this ESPN endpoint does not provide xG. Observed ESPN shots are used with an explicit measured-league empirical-Bayes prior of two 90-minute exposures.

## Chronology, integrity and workflow

- The ESPN collector excludes all fixture dates on or after an **exclusive as-of date**, requires a completed match, eleven starters per team, verifiable athlete IDs, consistent completed substitutions, and no target-match features.
- Every raw provider response is retained in an immutable offline archive with source SHA-256 fingerprints. The bridge rechecks these fingerprints and fails when a raw source was changed or deleted.
- The ESPN snapshot records both collection timestamps. The forecast cutoff must occur after source retrieval and strictly before the next match kickoff. Retrieval after the forecast cutoff is refused.
- The upcoming fixture is independently matched to an ESPN scoreboard by **home club, away club, exact UTC kickoff and scheduled state**; a manually guessed match timestamp is rejected.
- Only the existing as-of official roster and FM26 database are read. FM player and ESPN athlete identifiers occupy distinct namespaces. Previously registered but FM-unmatched observed players receive an explicit stable provisional official-roster ID, never a fabricated FM UID.
- No retrospective ESPN data may be spliced into the older October 10 forecast. The October 12 integration used a model-prior snapshot through October 9, with 1,190 dated EPL matches plus 552 Championship matches; the later ESPN archive has a separately recorded information cutoff.
- Outputs (source, crosswalk, match inputs, event probabilities and code/binary fingerprints) are immutable. The research CLI never changes the production simulator.

## October 12 Coventry–Newcastle integration check

ESPN schedule event **401879267** has kickoff 2026-10-12 19:00 UTC. The frozen ESPN archive was retrieved before the reproducible research cutoff **2026-10-12 02:42:20 UTC**, while the upcoming fixture had not started.

For that fixture the modeled full candidate pool has **35 Coventry and 48 Newcastle** identities, only **18 and 20**, respectively, with dated same-club ESPN participation histories. Both variants ran successfully for **2,500 paths**:

1. **Modeled-roster default:** retains all 83 candidate identities, keeps missing participation histories missing, and does not turn on the new lineup/substitution feature gates.
2. **Observed-only conditional computational scenario:** deliberately selects only those 18 and 20 players with verified observed histories, enabling exact-cardinality maximum-entropy lineup sampling and the contemporary five-substitution cap. This *excludes genuinely eligible players who might not appear in the partial dataset* and **MUST NOT** be published as a real-world XI or outcome forecast.

These are strictly engineering checks; they do not establish match calibration, predictive superiority, or suitability for betting/publishing.

## Reproduce on the local machine

Collect provider match summaries into a **new immutable directory/file before cutoff**:

~~~bash
python3 apps/matchforecast/math_engine/espn_feed.py --since 2026-08-01 --as-of 2026-10-11 --output data/ml/math_engine/espn_archive_new.json
~~~

The collector stores raw files in espn_archive_new.raw. Freeze the corresponding future fixture schedule, for example:

~~~bash
curl -fsS 'https://site.api.espn.com/apis/site/v2/sports/soccer/eng.1/scoreboard?dates=20261012' -o data/ml/math_engine/espn_schedule_oct12.json
~~~

The actual validated kickoff and original recorded source cutoff are preserved in the manifest; the following is a reproduction example **only when you have the original frozen pre-cutoff archive**, not a way to retroactively backdate a new download:

~~~bash
python3 apps/matchforecast/math_engine/run_espn.py \
  --output-dir data/ml/math_engine/new_espn_run \
  --as-of 2026-10-10 --forecast-cutoff-utc 2026-10-12T02:42:20Z \
  --fixture-date 2026-10-12 --home Coventry --away Newcastle \
  --kickoff-utc 2026-10-12T19:00:00Z \
  --squads data/pl/squads_2026-10-10.json \
  --espn data/ml/math_engine/espn_archive_new.json \
  --schedule-board data/ml/math_engine/espn_schedule_oct12.json \
  --paths 2500 --seed 42
~~~

FM26 defaults to /home/drewclaw/fm26-data/fm26_men_full_attrs.csv. Add --research-observed-only only for the clearly labeled conditional computational scenario.

Run all source and engine checks:

~~~bash
python3 -m unittest discover -s apps/matchforecast/math_engine -p 'test_*.py'
RUSTFLAGS='-D warnings' cargo test -p footy-matchforecast --lib math_engine
RUSTFLAGS='-D warnings' cargo check -p footy-matchforecast --bin footy-matchforecast-math
~~~

## Remaining research gates

1. A vetted provider-ID / birthdate-based identity crosswalk for the remaining roster, not weaker guessed name matches.
2. A genuinely pre-match, time-stamped injury/availability feed and independently measured xG or shot quality. Do not call FM-derived rates observed events.
3. Prospective proper-scoring validation on 200+ representative fixtures under fixed, pre-registered rules. The 56 ESPN source fixtures were acquired *after their outcomes*, so chronological replay on them is retrospective, not independent prospective evidence.
4. Evaluate role-aware substitutions and other complexities only if proper scoring establishes meaningful improvements against the unchanged causal baseline.
