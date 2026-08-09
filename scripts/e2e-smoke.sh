#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
cd "$ROOT"

FM="$TMP/fm.tsv"
TM_PLAYERS="$TMP/tm_players.tsv"
TM_VALUES="$TMP/tm_values.tsv"
TRANSFERS="$TMP/transfers.tsv"
CONTRACTS="$TMP/contracts.tsv"
FB_PLAYERS="$TMP/fb_players.tsv"
PERFORMANCE="$TMP/performance.tsv"
SALARY_PLAYERS="$TMP/salary_players.tsv"
SALARIES="$TMP/salaries.tsv"
MANIFEST="$TMP/manifest.bin"
CANONICAL="$TMP/canonical.bin"
MATRIX="$TMP/matrix.bin"
MODEL="$TMP/model.bin"
PREDICTIONS="$TMP/predictions.bin"

printf 'uid\tdate\tname\tdob\tca\tpa\treputation\n' > "$FM"
printf 'external_id\tname\tbirth_date\theight_cm\tnationality\n' > "$TM_PLAYERS"
printf 'player_id\tdate\tvalue_eur\n' > "$TM_VALUES"
printf 'transfer_id\tplayer_id\tdate\tfrom_club_id\tto_club_id\tkind\tfee_eur\tloan_fee_eur\n' > "$TRANSFERS"
printf 'player_id\tclub_id\tobserved_on\texpiry\toption_years\n' > "$CONTRACTS"
printf 'external_id\tname\tbirth_date\theight_cm\tnationality\n' > "$FB_PLAYERS"
printf 'player_id\tclub_id\tcompetition_id\tseason_id\tmatch_id\tstart_date\tend_date\tminutes\tmetric:goals\tmetric:assists\n' > "$PERFORMANCE"
printf 'external_id\tname\tbirth_date\theight_cm\tnationality\n' > "$SALARY_PLAYERS"
printf 'player_id\tclub_id\teffective_date\tcurrency\tlow_minor_units\thigh_minor_units\tbasis\tperiodicity\tkind\tconfidence\n' > "$SALARIES"

for i in $(seq 1 30); do
  year=$((1990 + (i % 10)))
  day=$((1 + (i % 27)))
  dob=$(printf '%04d-01-%02d' "$year" "$day")
  name="Player $i"
  ca=$((80 + i))
  pa=$((120 + i))
  reputation=$((1000 + i * 10))
  height=$((170 + (i % 20)))
  fee=$((1000000 + i * 250000))
  pre_value=$((fee * 9 / 10))
  post_value=$((fee * 11 / 10))
  if [ "$i" -le 15 ]; then
    transfer_date='2023-01-15'
    pre_value_date='2022-12-15'
  else
    transfer_date='2024-01-15'
    pre_value_date='2023-12-15'
  fi
  from_club="club$((i % 4))"
  to_club="club$(((i + 1) % 4))"

  printf 'fm%s\t2022-01-01\t%s\t%s\t%s\t%s\t%s\n' "$i" "$name" "$dob" "$ca" "$pa" "$reputation" >> "$FM"
  printf 'tm%s\t%s\t%s\t%s\tTestland\n' "$i" "$name" "$dob" "$height" >> "$TM_PLAYERS"
  printf 'tm%s\t%s\t%s\n' "$i" "$pre_value_date" "$pre_value" >> "$TM_VALUES"
  printf 'tm%s\t2024-06-01\t%s\n' "$i" "$post_value" >> "$TM_VALUES"
  printf 'tr%s\ttm%s\t%s\t%s\t%s\tpermanent\t%s\t\n' "$i" "$i" "$transfer_date" "$from_club" "$to_club" "$fee" >> "$TRANSFERS"
  printf 'tm%s\t%s\t2024-02-01\t2027-06-30\t1\n' "$i" "$to_club" >> "$CONTRACTS"

  printf 'fb%s\t%s\t%s\t%s\tTestland\n' "$i" "$name" "$dob" "$height" >> "$FB_PLAYERS"
  printf 'fb%s\tfbclub%s\tleague1\t2022-23\tmatch%s\t2022-12-01\t2022-12-01\t90\t%s\t%s\n' "$i" "$((i % 4))" "$i" "$((i % 3))" "$((i % 2))" >> "$PERFORMANCE"

  printf 'sal%s\t%s\t%s\t%s\tTestland\n' "$i" "$name" "$dob" "$height" >> "$SALARY_PLAYERS"
  printf 'sal%s\tsalclub%s\t2024-02-01\tEUR\t%s\t%s\tgross\tannual\tbase\t0.8\n' "$i" "$((i % 4))" "$((5000000 + i * 100000))" "$((5500000 + i * 100000))" >> "$SALARIES"
done

cargo +1.75.0 run --quiet -p footy-ingest -- stage \
  --dataset-version 1 \
  --output "$MANIFEST" \
  --source "fm,1,$FM" \
  --source "transfermarkt,1,$TM_PLAYERS" \
  --source "transfermarkt,1,$TM_VALUES" \
  --source "transfermarkt,1,$TRANSFERS" \
  --source "transfermarkt,1,$CONTRACTS" \
  --source "fbref,1,$FB_PLAYERS" \
  --source "fbref,1,$PERFORMANCE" \
  --source "salary,1,$SALARY_PLAYERS" \
  --source "salary,1,$SALARIES"

cargo +1.75.0 run --quiet -p footy-ingest -- verify --manifest "$MANIFEST"

cargo +1.75.0 run --quiet -p footy-ingest -- canonicalize \
  --manifest "$MANIFEST" \
  --fm "$FM" \
  --external-players "transfermarkt,$TM_PLAYERS" \
  --external-players "fbref,$FB_PLAYERS" \
  --external-players "salary,$SALARY_PLAYERS" \
  --tm-values "$TM_VALUES" \
  --tm-transfers "$TRANSFERS" \
  --tm-contracts "$CONTRACTS" \
  --fbref-performance "$PERFORMANCE" \
  --salary "$SALARIES" \
  --output "$CANONICAL"

cargo +1.75.0 run --quiet -p footy-query -- canonical --input "$CANONICAL" > "$TMP/canonical.txt"
grep -q '^players=30$' "$TMP/canonical.txt"
grep -q '^unresolved_identities=0$' "$TMP/canonical.txt"
grep -q '^issues=0$' "$TMP/canonical.txt"
grep -q '^market_values=60$' "$TMP/canonical.txt"
grep -q '^transfers=30$' "$TMP/canonical.txt"
grep -q '^contracts=30$' "$TMP/canonical.txt"
grep -q '^performance_observations=30$' "$TMP/canonical.txt"
grep -q '^salary_observations=30$' "$TMP/canonical.txt"
grep -q '^metrics=2$' "$TMP/canonical.txt"

cargo +1.75.0 run --quiet -p footy-train -- materialize-canonical \
  --canonical "$CANONICAL" \
  --feature-set-version 1 \
  --through 2024-12-31 \
  --output "$MATRIX"

cargo +1.75.0 run --quiet -p footy-query -- matrix --input "$MATRIX" > "$TMP/matrix.txt"
grep -q '^rows=60$' "$TMP/matrix.txt"
grep -q '^target_rows=60$' "$TMP/matrix.txt"

cargo +1.75.0 run --quiet -p footy-train -- fit \
  --input "$MATRIX" \
  --dataset-version 1 \
  --feature-set-version 1 \
  --training-cutoff 2023-06-30 \
  --calibration-cutoff 2024-12-31 \
  --model-version 1 \
  --calibration-version 1 \
  --ridge 1.0 \
  --seed 42 \
  --output "$MODEL"

cargo +1.75.0 run --quiet -p footy-score -- canonical \
  --model "$MODEL" \
  --canonical "$CANONICAL" \
  --as-of 2025-06-01 \
  --output "$PREDICTIONS"

cargo +1.75.0 run --quiet -p footy-query -- predictions --input "$PREDICTIONS" > "$TMP/predictions.tsv"
if [ "$(wc -l < "$TMP/predictions.tsv")" -ne 31 ]; then
  echo "expected prediction header plus 30 players" >&2
  exit 1
fi

printf 'end-to-end canonical pipeline passed\n'
