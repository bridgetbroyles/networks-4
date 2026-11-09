#!/usr/bin/env bash
set -euo pipefail

# Compare two switch programs across several traffic seeds, with three
# settlement-free peers, both normally and during the published landslide.
# All generated worlds and statements are temporary and are removed on exit.

ROOT=$(cd "$(dirname "$0")/.." && pwd)
BASELINE=${1:-"$ROOT/examples/a4_starter/target/wasm32-unknown-unknown/release/a4_starter.wasm"}
CANDIDATE=${2:-"$ROOT/examples/a4_competitive/target/wasm32-unknown-unknown/release/a4_competitive.wasm"}
SEEDS=${SEEDS:-"0 1 2 3 4 5"}
WITH_BACKUP=${WITH_BACKUP:-0}
SKIP_PEER3=${SKIP_PEER3:-0}
LOAD_SCALE=${LOAD_SCALE:-1}
SIM="$ROOT/target/release/competitive_net_sim"
SOURCE="$ROOT/worlds/practice/practice.toml"
WORLD="$ROOT/worlds/practice/.a4-benchmark.toml"
FAILURES="$ROOT/worlds/practice/failures/landslide.toml"
PEERING="$ROOT/examples/settlements/peering/target/wasm32-unknown-unknown/release/settle_peering.wasm"
BACKUP="$ROOT/examples/settlements/backup/target/wasm32-unknown-unknown/release/settle_backup.wasm"
TMP=$(mktemp -d)
AGREEMENTS="$TMP/agreements.toml"

cleanup() {
  rm -f "$WORLD"
  rm -rf "$TMP"
}
trap cleanup EXIT

cat >"$AGREEMENTS" <<EOF

[[a4.agreement]]
kind = "peer"
a = 1
b = 2
program = "$PEERING"
terms = { }
max = 1000

[[a4.agreement]]
kind = "peer"
a = 1
b = 4
program = "$PEERING"
terms = { }
max = 1000
EOF

if [[ $SKIP_PEER3 == 0 ]]; then
  cat >>"$AGREEMENTS" <<EOF

[[a4.agreement]]
kind = "peer"
a = 1
b = 3
program = "$PEERING"
terms = { }
max = 1000
EOF
fi

if [[ $WITH_BACKUP == 1 ]]; then
  cat >>"$AGREEMENTS" <<EOF

[[a4.agreement]]
kind = "backup"
a = 1
b = 3
program = "$BACKUP"
terms = { fee = 0, price = 0, min_on_time = 0, refund = 0 }
max = 1000
EOF
fi

printf 'seed\tmode\tbaseline_utility\tcandidate_utility\tdelta\n'
for seed in $SEEDS; do
  cp "$SOURCE" "$WORLD"
  sed -i.bak "0,/seed = 0/s//seed = $seed/" "$WORLD"
  rm -f "$WORLD.bak"
  cat "$AGREEMENTS" >>"$WORLD"
  for mode in normal landslide; do
    before="$TMP/before-$seed-$mode.json"
    after="$TMP/after-$seed-$mode.json"
    if [[ $mode == landslide ]]; then
      "$SIM" run-a4 "$WORLD" --program "$BASELINE" --as 1 --load-scale "$LOAD_SCALE" --failures "$FAILURES" --json "$before" >/dev/null
      "$SIM" run-a4 "$WORLD" --program "$CANDIDATE" --as 1 --load-scale "$LOAD_SCALE" --failures "$FAILURES" --json "$after" >/dev/null
    else
      "$SIM" run-a4 "$WORLD" --program "$BASELINE" --as 1 --load-scale "$LOAD_SCALE" --json "$before" >/dev/null
      "$SIM" run-a4 "$WORLD" --program "$CANDIDATE" --as 1 --load-scale "$LOAD_SCALE" --json "$after" >/dev/null
    fi
    b=$(jq -r '.[0].utility' "$before")
    a=$(jq -r '.[0].utility' "$after")
    delta=$(awk -v a="$a" -v b="$b" 'BEGIN { printf "%.6f", a-b }')
    printf '%s\t%s\t%.6f\t%.6f\t%s\n' "$seed" "$mode" "$b" "$a" "$delta"
  done
done
