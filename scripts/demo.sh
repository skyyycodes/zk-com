#!/usr/bin/env bash
# Builds the hack-day demo from scratch on a local Ironwood devnet:
#   one fund passing, a borrowed-coins fund flagged, a reused note refused.
# Usage: scripts/demo.sh            (starts the registry on :8080 and leaves it running)
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release -q
( cd web && { [ -d node_modules ] || npm ci --no-audit --no-fund; } && npm run build --silent >/dev/null )
Z="./target/release/zsolv"
export ZSOLV_DATA="${ZSOLV_DATA:-data}"
URL="${URL:-http://127.0.0.1:8080}"
rm -rf "$ZSOLV_DATA" attestations && mkdir -p attestations

step() { printf '\n\033[1m== %s\033[0m\n' "$*"; }

step "Chain and wallets"
$Z devnet init
for w in mill members lender quick; do $Z wallet new $w; done

step "Donations arrive, shielded (memos are what a viewing key would expose)"
$Z send --to mill --amount 150 --label "Dockworkers Local 9"  --memo "solidarity from the docks"
$Z send --to mill --amount 220 --label "anonymous"            --memo "for the families"
$Z send --to mill --amount 95  --label "Riverside Teachers"   --memo "we stand with you"
$Z send --to mill --amount 60  --label "a nurse at St. Anne's" --memo "keep going"
$Z send --to lender --amount 480 --label "lender's savings"   --memo ""

step "The fund locks a reserve note of exactly T = 480 ZEC (4 weeks x 120 workers x 1 ZEC)"
X=$($Z reserve --wallet mill --amount 480 | sed -E 's/^block ([0-9]+).*/\1/')
echo "reserve created at block X=$X"

step "Weeks pass: more donations, strike pay goes out from operating notes"
$Z devnet mine --blocks 6
$Z send --to mill --amount 40 --label "Bakery co-op" --memo "bread money"
$Z send --from mill --to members --amount 30 --memo "strike pay wk1: M. Okafor, J. Lindqvist, +28"
$Z devnet mine --blocks 6
$Z send --to mill --amount 25 --label "anonymous" --memo "from a manager who agrees"
$Z send --from mill --to members --amount 30 --memo "strike pay wk2: S. Haddad, P. Ruiz, +28"
$Z devnet mine --blocks 8

step "Cheat setup: a lender moves 480 ZEC into Quick Relief Fund just before the check"
$Z send --from lender --to quick --amount 480 --reserve --memo "short-term loan, return Monday"
$Z devnet mine --blocks 1
H=$($Z devnet status | sed -E 's/.* tip ([0-9]+) .*/\1/')
echo "check height H=$H"

step "Registry"
if ! curl -s "$URL/api/state" >/dev/null 2>&1; then
  ZSOLV_RATE_LIMIT="${ZSOLV_RATE_LIMIT:-0}" $Z serve >/tmp/zsolv-serve.log 2>&1 &
  for i in $(seq 1 50); do curl -s "$URL/api/state" >/dev/null 2>&1 && break; sleep 0.2; done
fi
$Z fund --url "$URL" --id mill --name "Mill Workers' Strike Fund" \
  --description "Strike pay for 120 workers at the Halden textile mill. Fictional demo fund." \
  --obligation "Strike pay: 120 workers x 1 ZEC/week x 4 weeks=480" --min-held-blocks 20 \
  --zec-usd 40 --price-time "demo price, 1 Nov 2026"
$Z fund --url "$URL" --id quick --name "Quick Relief Fund" \
  --description "Claims the same reserve. Its coins arrived the block before the check." \
  --obligation "Relief grants=480" --min-held-blocks 20 --zec-usd 40 --price-time "demo price, 1 Nov 2026"
$Z fund --url "$URL" --id shadow --name "Shadow Solidarity Fund" \
  --description "Tries to count the Mill Workers' reserve note as its own." \
  --obligation "Grants=480" --min-held-blocks 20
$Z viewing-key-demo --wallet mill --fund-id mill

step "Mill Workers prove: at least 480 ZEC, held since X=$X, checked at H=$H"
$Z prove --wallet mill --fund-id mill --threshold 480 --since "$X" --at "$H" --out attestations/mill.json
$Z verify attestations/mill.json
$Z submit attestations/mill.json --url "$URL"

step "Cheat 1: Quick Relief tries to claim 'held since X' with borrowed coins"
$Z prove --wallet quick --fund-id quick --threshold 480 --since "$X" --at "$H" --out attestations/quick-cheat.json || true
echo "-- relabelling the since-height anyway:"
$Z prove --wallet quick --fund-id quick --threshold 480 --at "$H" --cheat-claim-since "$X" --out attestations/quick-relabel.json
$Z verify attestations/quick-relabel.json || true
$Z submit attestations/quick-relabel.json --url "$URL" || true
echo "-- the honest claim is all it can make: held since H-1"
$Z prove --wallet quick --fund-id quick --threshold 480 --at "$H" --round 2 --out attestations/quick.json
$Z submit attestations/quick.json --url "$URL"

step "Cheat 2: Shadow fund submits the same note"
cp "$ZSOLV_DATA/wallets/mill.json" "$ZSOLV_DATA/wallets/shadow.json"
$Z prove --wallet shadow --fund-id shadow --threshold 480 --since "$X" --at "$H" --out attestations/shadow.json
$Z submit attestations/shadow.json --url "$URL" || true

step "Done. Open $URL"
