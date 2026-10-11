# Exact-cardinality starting XI research challenger (October 11, 2026)

## Problem and scope

The historical same-engine study shows start-probability compression and a worse
player-start Brier score than the causal selection prior. A necessary feasibility
condition for an eleven-player starting lineup is `sum_i P(start_i) = 11`, yet the
historical adapter's input priors often do not satisfy this. A sequential
role-by-role sampler also changes the marginal probabilities as a consequence
of slot ordering and competition for positions.

The new `crates/match/src/lineup_entropy.rs` implements a **research-only**
alternative for *role-neutral* historical inputs. The production sampler and
all pre-existing simulation settings remain the defaults. Invoke the historical
driver with `--exact-cardinality-lineup` and a **new** output path to evaluate
the challenger; do not overwrite any frozen run.

## Exact algorithm and failure boundaries

1. Preserve the boundaries `P_i=0` and `P_i=1` exactly. If confirmed zeroes
   and ones make eleven starters mathematically impossible, fail closed.
2. Among uncertain candidates, project the vector of start probabilities
   onto `[0,1]^n` with `sum_i P_i=11`, using the Euclidean projection given
   by a shared offset and clipping. The size of this adjustment is reported as
   a diagnostic; it is a *changed prediction*, not an empirical calibration.
3. Fit the maximum-entropy fixed-size subset law

   `P(S | |S|=11) = exp(sum_{i in S} theta_i) / Z_11(theta)`.

   The normalizing polynomial uses log-sum-exp elementary-symmetric
   dynamic programming in `O(n*11)`; a strictly offline-from-the-path dual
   fit finds weights whose exact conditional subset marginals match the
   projected target. The fit aborts if the marginal residual exceeds `1e-8`.
4. Given a path's available players, use an exact DP backwards sampler to
   draw the starting set. Assign the selected players randomly to slots only
   when all players have equal fit to every slot. Role-sensitive inputs are
   rejected, not silently treated as exchangeable. The path-local RNG remains
   the repository's deterministic `Rng64`.

**Important limitation:** Historical candidate availability is itself
stochastic (`0.96` per player in the older adapter). The dual matches the
projected marginals **conditional on a fully available squad**. Conditioning
that law on a smaller available roster does not guarantee that its *unconditional*
start marginals equal the source priors. Repeated availability draws that
enforce at least eleven available players also change the unconditional law.
This experiment preserves that pre-existing mechanism to make an otherwise
matched engine comparison; it makes no claim of solving the joint selection
and availability calibration problem.

## Reproduction and promotion gate

```bash
cargo test -p footy-match lineup_entropy --lib
cargo test -p footy-match opt_in_exact_cardinality --lib
cargo build --release -p footy-matchforecast --bin footy-matchforecast-pl-hist
./target/release/footy-matchforecast-pl-hist \
  --exact-cardinality-lineup --paths 200 --jobs 4 --seed 42 \
  --output data/ml/pl-hist-maxent-research-20261011.json
python3 scripts/score_pl_hist_backtest.py \
  --predictions data/ml/pl-hist-maxent-research-20261011.json \
  --hist-dir data/statsbomb/pl_hist_elapsed_v2 \
  --compare-predictions artifacts/validation/pl-hist-same-engine-paths.json \
  --experiment 'role-neutral exact-cardinality max-entropy XI' \
  --output data/ml/pl-hist-maxent-oos-research-20261011.json
```

The 25 historical held-out outcomes have already been examined for previous
experiments. Any new result against them is an **exploratory engineering
comparison**, not an unbiased promotion test, regardless of an apparently
significant confidence interval. A defensible promotion requires new,
chronologically later independent evaluation fixtures from a broader set of
clubs and the same proper-scoring suite, including start/appearance Brier,
minutes MAE, player goals, W/D/L and exact score, with matched Monte Carlo
precision and uncertainty bounds. The new sampler changes the joint team and
player distribution, so improving marginal selection alone is insufficient.

### Optional Modal execution (CPU, not GPU)

The second-resolved Rust simulator spends its work inside many independent
paths, so CPU parallelism is appropriate; a GPU or Candle model does not speed
up the current event clock. `scripts/modal_pl_hist.py` can execute the *same*
locally compiled Rust release binary on a Modal eight-CPU function, shipping
only the public StatsBomb pre-match feature cache and verifying the binary
SHA256 on both ends. Outputs return compressed and the adapter refuses to
overwrite an existing result. Modal SDK and account authentication are
optional external orchestration dependencies, not core Rust dependencies.

```bash
# After the release executable is built and Modal CLI is authenticated:
modal run scripts/modal_pl_hist.py --paths 200 --seed 42 \
  --exact-cardinality \
  --output data/ml/modal-maxent-20261011.json
```

Reproduce the same run locally and compare normalized outputs and scores
before trusting cloud parity; the remote Ubuntu glibc must be compatible with
the locally compiled binary. To scale to new datasets, shard work across
fixtures **only after** ensuring the as-of inputs and frozen fit-period
calibration are computed before sharding and that aggregation is order-stable.
Cloud execution accelerates evidence collection but cannot create statistical
independence or make a reused OOS period fresh. The optional runner has not
been asserted to validate any new prediction merely by running remotely.

**Remote smoke verified:** The Modal run of October 11 using two paths on
StatsBomb match `3754107` completed. The local and Modal output JSONs were
byte-identical (`SHA256 966d6827fb7bacc74c514548cc5213bd6e056b123a9c10e3bd54f36aec6fbf19`)
with the same binary (`SHA256 08d72d9f7f91985d7382e8f4ed3314818d85201270b7905dad053487e5e058f9`).
The remote adapter verified the pre-match feature-input fingerprint
`e976303462e53f640d77d8d08efe83471235eefe76d5f3f170cee3f3d0b7f916`.
This establishes execution parity for a single-fixture smoke run, not
full-archive reproducibility or forecasting accuracy.

## Liverpool–City interpretation

The frozen October 10 forecast must remain untouched. Its 60–75 peak was a
relative pressure statistic, driven substantially by simulated Liverpool shot
decline; later ablations showed sensitivity to scheduled substitutions and
random path realizations. A correct lineup law may change the composition of
simulated pressure but provides **no retrospective causal explanation** for
the red card or City's real goals. The corrected StatsBomb elapsed-clock test
does not establish superior event timing. Future tactical/substitution hazard
work should use an as-of expanded event sample, genuinely completed-event
labels and common-random-number *event-level* counterfactuals; sharing a seed
alone does not pair RNG draws when changed hazards consume different draws.
