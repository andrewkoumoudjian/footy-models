#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
cd "$ROOT"

FM="$TMP/fm.tsv"
TM_PLAYERS="$TMP/tm_players.tsv"
TRANSFERS="$TMP/transfers.tsv"
MANIFEST="$TMP/manifest.bin"
CANONICAL="$TMP/canonical.bin"
MATRIX="$TMP/matrix.bin"
MODEL="$TMP/model.bin"
PREDICTIONS="$TMP/predictions.bin"

printf 'uid\tdate\tname\tdob\tca\tpa\treputation\n' > "$FM"
printf 'external_id\tname\tbirth_date\theight_cm\tnationality\n' > "$TM_PLAYERS"
printf 'transfer_id\tplayer_id\tdate\tfrom_club_id\tto_club_id\tkind\tfee_eur\tloan_fee_eur\n' > "$TRANSFERS"

for i in $(seq 1 30); do
  year=$((1990 + (i % 10)))
  day=$((1 + (i % 27)))
  dob=$(printf '%04d-01-%02d' "$year" "$day")
  name="Player $i"
  ca=$((80 + i))
  pa=$((120 + i))
  reputation=$((1000 + i * 10))
  height=$((170 + (i % 20)))
  if [ "$i" -le 15 ]; then
    transfer_date='2023-01-15'
  else
    transfer_date='2024-01-15'
  fi
  fee=$((1000000 + i * 250000))
  printf 'fm%s\t2022-01-01\t%s\t%s\t%s\t%s\t%s\n' "$i" "$name" "$dob" "$ca" "$pa" "$reputation" >> "$FM"
  printf 'tm%s\t%s\t%s\t%s\tTestland\n' "$i" "$name" "$dob" "$height" >> "$TM_PLAYERS"
  printf 'tr%s\ttm%s\t%s\tclub%s\tclub%s\tpermanent\t%s\t\n' "$i" "$i" "$transfer_date" "$((i % 4))" "$(((i + 1) % 4))" "$fee" >> "$TRANSFERS"
done

cargo +1.75.0 run --quiet -p footy-ingest -- stage \
  --dataset-version 1 \
  --output "$MANIFEST" \
  --source "fm,1,$FM" \
  --source "transfermarkt,1,$TM_PLAYERS" \
  --source "transfermarkt,1,$TRANSFERS"

cargo +1.75.0 run --quiet -p footy-ingest -- verify --manifest "$MANIFEST"

cargo +1.75.0 run --quiet -p footy-ingest -- canonicalize \
  --manifest "$MANIFEST" \
  --fm "$FM" \
  --external-players "transfermarkt,$TM_PLAYERS" \
  --tm-transfers "$TRANSFERS" \
  --output "$CANONICAL"

cargo +1.75.0 run --quiet -p footy-query -- canonical --input "$CANONICAL" > "$TMP/canonical.txt"
grep -q '^players=30$' "$TMP/canonical.txt"
grep -q '^unresolved_identities=0$' "$TMP/canonical.txt"
grep -q '^transfers=30$' "$TMP/canonical.txt"

cargo +1.75.0 run --quiet -p footy-train -- materialize-canonical \
  --canonical "$CANONICAL" \
  --feature-set-version 1 \
  --through 2024-12-31 \
  --output "$MATRIX"

cargo +1.75.0 run --quiet -p footy-query -- matrix --input "$MATRIX" > "$TMP/matrix.txt"
grep -q '^rows=30$' "$TMP/matrix.txt"
grep -q '^target_rows=30$' "$TMP/matrix.txt"

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
