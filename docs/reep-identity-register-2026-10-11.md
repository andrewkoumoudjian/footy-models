# Reep DuckDB register — source-neutral identity layer

Research integration, 2026-10-11. Reep is a **cross-provider identity register**,
not a player-attributes dataset or a source of historical event-rate labels.
The pinned public release is `20261005T180536Z`, Creative Commons CC0 1.0.

## Installed locally

```text
data/identity/reep/reep-20261005T180536Z.duckdb
bytes: 779366400
sha256: f23fbce71e142dc76703a065bd14a820564222ca13a775e4b24e30bb648da8af
```

The database is untracked and **must not be committed**. The synced
`data/identity/reep/active.json` records the release stamp, manifest hash,
full local path and checked SHA-256. To refresh the upstream weekly release:

```bash
python3 scripts/sync_reep_register.py --output data/identity/reep
```

The synchronizer checks both the publisher's manifest checksum and the whole
779 MB database checksum, resumes HTTP range transfers, and atomically
activates only a complete file. The old stamped file remains for reproducible
historical replay. The register is downloaded from the publisher's R2 server,
not from Hugging Face or an unverified GitHub mirror.

## Local EPL and SofaScore crosswalk audit

```bash
uv run --no-project --with duckdb python scripts/audit_reep_crosswalk.py \
  --output data/identity/reep/coverage_v3.json
python3 -m unittest discover -s tests -p 'test_reep_crosswalk.py'
```

Reads the local FM26 export, official dated EPL registrations and existing
SofaScore historical match sheets. The audit uses **typed ID bridges**
(`fm/player`, `sofascore/player`, `fbref/person`, `transfermarkt/spieler`),
resolves Reep redirects, preserves multi-ID ambiguity and checks the native FM
record's identity, DOB, kind and attribute provenance. It never promotes an
ambiguous name or guesses a player from CA.

| EPL | Count |
| --- | ---: |
| Official registrations, including U21 | 600 |
| Previously assigned FM UIDs | 385 |
| Those FM UIDs with one Reep bridge | 293 |
| Still unassigned an FM identity | 215 |

The **293 are a subset of 385**, not 293 additional people recovered from the
unresolved EPL registrations. Those remaining 215 name-only EPL registrations
need an actual provider ID or independently verified person identification
before Reep can improve their FM coverage. The full 600 stay in the candidate
universe; no unverified player is deleted.

| Local SofaScore match-history identities | Count |
| --- | ---: |
| Distinct SofaScore player IDs | 931 |
| One Reep player identity | 616 |
| Linked to one FM UID through Reep | 561 |
| Of these, also linked to FBref | 468 |
| FM UID absent from our local FM export | 22 |
| Native FM record classified as staff/nonplayer | 148 |
| Source DOB contradicts native FM DOB | 4 |
| DOB-consistent native FM player, actual database attribute block | 370 |
| DOB-consistent native FM player, reconstructed attributes | 17 |

These are the observed SofaScore identities from the historical local cache,
not the whole MLS player population. **Reep's ability to map an ID does not
prove the retrieved FM player attributes are measured or authentic.** The
370 database-attribute cases form the higher-confidence integration subset;
the 17 reconstructed-attribute cases must retain a reconstruction flag and
uncertainty, and the other 174 are quarantined from exact FM attribute input.
All numbers refer to the pinned release; they can change after new releases or
corrections.

## One-model integration boundary

Keep one canonical player representation per forecast fixture, with separate
feature-source blocks and provenance masks:

* **Identity:** Reep ID, native FM UID, SofaScore ID, FBref ID, Transfermarkt
  ID; provider namespace, release stamp, verification evidence and conflicts.
* **FM26:** native CA and all directly measured mental, physical and technical
  attributes; keep `db` versus `derived` separate. Derived attributes are
  draws from a distribution, **not observed values** for that particular
  footballer. Source-level mental-team aggregates should preserve coverage and
  avoid implying an unconfirmed starting XI is known.
* **SofaScore:** strictly prior player minutes/ratings/events, historic starts
  and team performance observations. Use dated match sheets and distinguish
  missing API fields from observed zeros.
* **FBref:** player events and advanced measures only with a verified ID
  crosswalk and a known data-availability date. A Reep FBref ID is the join
  key; it does not mean the historical FBref event record was ingested.
* **Chemistry:** previous observed co-starts and player combinations before
  the forecast cutoff, not shared current club membership or an invented
  latent chemistry score. Keep the origin and sample size explicit.

Those blocks feed a **single source-neutral multitask distribution model**
for EPL and MLS, with competition embeddings/partial pooling and proper
train-only normalization. Its outputs are start/appearance/minutes, player
shot and scoring intensities, team strengths, and incident distributions.
The Rust match engine consumes its versioned distributions and remains
independent of provider, training framework and data warehouse.

Before production, require chronological EPL and MLS holdouts including
lineup coverage, model-vs-baseline proper scoring, incident calibration and
same-engine W/D/L and exact-score comparisons. The identity mapping does
**not** certify that a new forecasting architecture improves accuracy.
