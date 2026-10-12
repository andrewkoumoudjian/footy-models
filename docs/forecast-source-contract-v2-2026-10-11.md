# Premier League forecast source contract v2 — 2026-10-11

This implements the **source-integrity/structural-lineup** portion of the 2026-10-11
pipeline audit. It does **not** certify improved forecast quality or promote the
historical match engine over the causal Poisson reference. Previously published
forecast snapshots and the original OOS reports are unchanged.

## Normalization rules

* `match_premier_league_fm26.py` never breaks duplicate name matches by FM CA.
  An ambiguous name remains unresolved. An accepted name match retains its FM
  `kind`, `attr_source`, match method, club, registration type and as-of date.
  Only `kind=player, attr_source=db` can supply original FM attributes to the
  player adapter. This **verifies record type and attribute provenance**, not
  independent real-world person identity: provider-ID/DOB joins remain missing.
* `prepare_pl_player_inputs.py` creates an auditable **full** official candidate
  roster, including unmatched players with deterministic **provisional** roster
  identifiers. Its FM-backed simulation CSV remains a distinct view; incomplete
  identities are not automatically converted into fabricated FM footballers.
  Staff-classified/derived records in that view have no trusted FM ratings,
  preserve their source quality and receive broader prior uncertainty in Rust.
* Previous-season minutes are labeled previous-season priors. Missing current
  selections remain missing; zeros in the selection-count columns are the
  legacy adapter representation of *no observations*, not observed omissions.
  The candidate-roster sidecar uses `null` for unobserved participation.
* Team goals are outcomes. Raw measured `HxG/AxG`, shots and shots-on-target
  appear **only when supplied by the source**; absent values are empty and
  explicitly `*_kind=missing`, never copies of scored goals or canned shot
  totals. Rust EWMA skips missing measurements, retaining its population prior.
* Since historical E0 results lack reliable publication timestamps, source
  matches on the forecast cutoff calendar day are excluded conservatively.
  `source_published_at` is blank until substantiated; no timestamp is invented.
  Dated availability inputs published after the cutoff are rejected.

## Structural lineup contract

When `position_group` is provided, Rust uses separately located 4-4-2/4-3-3
templates with goalkeeper, defender, midfield and forward slots. Known outfield
players cannot occupy the keeper role. If no keeper or fewer than fourteen
eligible candidates exist, the adapter fails instead of padding invented players.
The core role-aware sampler treats zero role fit as forbidden, and resamples
availability if an essential role is unfilled. Legacy historical/MLS input
without dated positions retains the original unpositioned template.

Historical position labels are prior-season observations; **formations, start
probabilities, minutes, injuries and conditional event rates still require
chronological training and external calibration.** An available registered
player is never claimed to be a confirmed starter. The widened unverified-FM
variance multiplier and structural roles are **uncertified challengers**, not
established improvements to scoreline accuracy. Do not publish new production
forecasts from this code without the same-engine proper-score release gate.

## Local reproduction

Run the identity matcher offline with the frozen registration HTML and a licensed
local FM26 CSV, saving to a fresh, non-historical `squads_gate_<as-of>.json`.
Then run player preparation with `--data-dir data/pl/gate_trial` and an explicitly
dated availability file. That directory must contain an `raw` link to the
original E0 CSV directory. The coverage checker accepts `--squads`, `--players`,
`--team-rows` and `--fm` to score that staging view without overwriting the
original October 10 input snapshots. The new Python regression tests are in
`tests/test_pl_source_quality.py`, and the Rust role feasibility test is in
`crates/match/src/lineup.rs`.

**Next gate:** obtain adequate historical starts and minutes with provider IDs,
fit a small chronological appearance/start/minutes baseline, and score lineups,
player shots, W/D/L, exact score and event incidence on fresh independent
matches. No Candle promotion, incident-probability retuning or frozen OOS
reuse until these measurements exist.
