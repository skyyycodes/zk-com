#!/usr/bin/env bash
# The demo on Zcash testnet, with real Ironwood notes. Then saves a snapshot so
# hack day does not depend on testnet or lightwalletd being up.
#
# Before running, with an Ironwood-capable testnet wallet (Zkool, Zingo):
#   1. Mill: a seed wallet that sent exactly 480 TAZ to its own address.
#      That note is the reserve; it must be older than the check.
#   2. Optional, for the borrowed-coins cheat: a second seed wallet (Quick)
#      that received 480 TAZ from a lender shortly before you run this.
#
#   MILL_MNEMONIC="..." [QUICK_MNEMONIC="..."] scripts/testnet-demo.sh
#
# Seed phrases are read from the environment only, never from arguments, and
# only until the wallet files exist. Fund keys in $ZSOLV_DATA/funds are never
# deleted: they are the only way to update those funds on any registry.
# URL: the registry to publish to (default: a local one this script starts).
# FRESH=1 resets that local registry first. Rounds continue from the registry.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release -q
( cd web && { [ -d node_modules ] || npm ci --no-audit --no-fund; } && npm run build --silent >/dev/null )
Z="./target/release/zsolv"
export ZSOLV_DATA="${ZSOLV_DATA:-data-testnet}"
REG="zsolv-testnet-registry-1"
URL="${URL:-http://127.0.0.1:8080}"
BIRTHDAY="${BIRTHDAY:-4134000}"
T=480
mkdir -p attestations-testnet
step() { printf '\n\033[1m== %s\033[0m\n' "$*"; }

step "Testnet chain data (Ironwood, from lightwalletd)"
$Z testnet sync
H=$($Z roots | sed -nE 's/.*"height": ([0-9]+).*/\1/p')
$Z roots --height "$H" --check | tail -1

step "Mill Workers: wallet from its seed, found by trial decryption"
if [ ! -f "$ZSOLV_DATA/wallets/mill.json" ]; then
  : "${MILL_MNEMONIC:?set MILL_MNEMONIC to the Mill fund wallet seed phrase}"
  ZSOLV_MNEMONIC="$MILL_MNEMONIC" $Z wallet import mill --birthday "$BIRTHDAY"
fi
$Z wallet scan mill
X=$($Z reserve --wallet mill --amount $T | tee /dev/stderr | sed -nE 's/.*received at block ([0-9]+).*/\1/p')
echo "reserve note since block X=$X, check at H=$H"

step "Registry"
LOCAL=""
if ! curl -s "$URL/api/state" >/dev/null 2>&1; then
  case "$URL" in http://127.0.0.1:*|http://localhost:*) ;; *) echo "no registry at $URL" >&2; exit 1 ;; esac
  [ "${FRESH:-}" = 1 ] && rm -rf "$ZSOLV_DATA/registry"
  ZSOLV_RATE_LIMIT=0 ZSOLV_REBUILDS_PER_HOUR=0 $Z serve --registry "$REG" --addr "${URL#http://}" >/tmp/zsolv-testnet-serve.log 2>&1 &
  for i in $(seq 1 100); do curl -s "$URL/api/state" >/dev/null 2>&1 && break; sleep 0.3; done
  LOCAL=1
fi
fund() { $Z fund --url "$URL" --registry "$REG" "$@"; }
# The fund's next round on that registry (1 if it has none).
next_round() {
  curl -s "$URL/api/funds/$1" | python3 -c 'import json,sys; d=json.load(sys.stdin); print(((d.get("latest") or {}).get("round") or 0) + 1)' 2>/dev/null || echo 1
}
fund --id mill --name "Mill Workers' Strike Fund" \
  --description "Strike pay for 120 workers at the Halden textile mill. Fictional demo fund, real Ironwood testnet notes." \
  --obligation "Strike pay: 120 workers x 1 TAZ/week x 4 weeks=480" --min-held-blocks 20
fund --id shadow --name "Shadow Solidarity Fund" \
  --description "Tries to count the Mill Workers' reserve note as its own." --obligation "Grants=480" --min-held-blocks 20

step "Mill Workers prove: at least $T TAZ, held since X=$X, checked at H=$H"
$Z prove --wallet mill --fund-id mill --registry "$REG" --threshold $T --since "$X" --at "$H" --round "$(next_round mill)" --out attestations-testnet/mill.json
$Z verify attestations-testnet/mill.json
$Z submit attestations-testnet/mill.json --url "$URL"

if [ -n "${QUICK_MNEMONIC:-}" ]; then
  step "Cheat 1: Quick Relief claims 'held since X' with borrowed coins"
  fund --id quick --name "Quick Relief Fund" \
    --description "Claims the same reserve. Its coins arrived just before the check." --obligation "Relief grants=480" --min-held-blocks 20
  [ -f "$ZSOLV_DATA/wallets/quick.json" ] || ZSOLV_MNEMONIC="$QUICK_MNEMONIC" $Z wallet import quick --birthday "$BIRTHDAY"
  $Z wallet scan quick
  $Z reserve --wallet quick --amount $T
  $Z prove --wallet quick --fund-id quick --registry "$REG" --threshold $T --since "$X" --at "$H" --out attestations-testnet/quick-cheat.json || true
  R=$(next_round quick)
  $Z prove --wallet quick --fund-id quick --registry "$REG" --threshold $T --at "$H" --round "$R" --cheat-claim-since "$X" --out attestations-testnet/quick-relabel.json
  $Z submit attestations-testnet/quick-relabel.json --url "$URL" || true
  echo "-- the honest claim is all it can make: held since its coins arrived"
  $Z prove --wallet quick --fund-id quick --registry "$REG" --threshold $T --at "$H" --round "$R" --out attestations-testnet/quick.json
  $Z submit attestations-testnet/quick.json --url "$URL"
else
  echo "(QUICK_MNEMONIC not set: skipping the borrowed-coins cheat)"
fi

step "Cheat 2: Shadow fund submits the same note"
cp "$ZSOLV_DATA/wallets/mill.json" "$ZSOLV_DATA/wallets/shadow.json"
$Z prove --wallet shadow --fund-id shadow --registry "$REG" --threshold $T --since "$X" --at "$H" --round "$(next_round shadow)" --out attestations-testnet/shadow.json
$Z submit attestations-testnet/shadow.json --url "$URL" || true
rm "$ZSOLV_DATA/wallets/shadow.json"

if [ -n "$LOCAL" ]; then
  step "Snapshot for hack day (public data only: no wallets or fund keys)"
  SNAP="snapshots/testnet-$H.tar.gz"
  mkdir -p snapshots
  tar -czf "$SNAP" -C "$ZSOLV_DATA" chain registry -C "$PWD" attestations-testnet
  echo "saved $SNAP. Replay offline: mkdir d && tar -xzf $SNAP -C d && ZSOLV_DATA=d zsolv serve --registry $REG"
else
  echo "published to $URL (no local snapshot: the registry state lives there)"
fi
