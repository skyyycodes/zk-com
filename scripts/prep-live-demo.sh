#!/usr/bin/env bash
# Prepares a live, hands-on demo on a local Ironwood devnet, then stops just
# before the moments you show the audience. It writes a cheat sheet with the
# real block heights filled in.
#
# Usage: scripts/prep-live-demo.sh      (rerun any time to reset to the start)
#
# Data goes to LIVE_DATA (default ./data-live), so scripts/demo.sh never wipes
# it. The registry runs on http://127.0.0.1:8080 until you stop it.
#
# Funds after prep:
#   mill       Mill Workers' Strike Fund  already published, green (the reference)
#   northwind  Northwind Custody          registered, no proof yet: you prove live
#   quickline  Quickline Exchange         borrowed 5000 ZEC one block before H
#   shadow     Shadow Custody             a copy of Northwind's keys
set -euo pipefail
cd "$(dirname "$0")/.."

need="1.97"
have=$(rustc --version | awk '{print $2}')
if [ "$(printf '%s\n%s\n' "$need" "$have" | sort -V | head -1)" != "$need" ]; then
  echo "rustc $have is older than $need. Run: rustup update stable   (or RUSTUP_TOOLCHAIN=1.98.1 $0)" >&2
  exit 1
fi

D="${LIVE_DATA:-data-live}"
A="$D/attestations"
URL="http://127.0.0.1:8080"
Z="./target/release/zsolv"
export ZSOLV_DATA="$D"

if [ -e "$D/chain/ledger.bin" ]; then
  echo "refusing to wipe $D: it holds testnet chain data. Set LIVE_DATA to another directory." >&2
  exit 1
fi

step() { printf '\n\033[1m== %s\033[0m\n' "$*"; }

step "Build"
cargo build --release -q
( cd web && { [ -d node_modules ] || npm ci --no-audit --no-fund; } && npm run build --silent >/dev/null )

step "Stop the registry from a previous prep"
if [ -f "$D/serve.pid" ]; then kill "$(cat "$D/serve.pid")" 2>/dev/null || true; sleep 0.5; fi
if curl -s "$URL/api/state" >/dev/null 2>&1; then
  echo "something is still answering on $URL (maybe scripts/demo.sh's registry)." >&2
  echo "Stop it first:  pkill -f '[z]solv serve'" >&2
  exit 1
fi
rm -rf "$D" && mkdir -p "$A/backup"

step "Chain and wallets"
$Z devnet init
for w in mill northwind lender quickline members clients; do $Z wallet new $w; done

step "Deposits arrive, shielded (memos are what a viewing key would expose)"
$Z send --to northwind --amount 2200 --label "Helios Capital"     --memo "client deposit: Helios Capital, acct 4471"
$Z send --to northwind --amount 1800 --label "Orbit Family Office" --memo "client deposit: Orbit FO, quarterly top-up"
$Z send --to northwind --amount 1500 --label "anonymous"           --memo "client deposit: retail pool 12"
$Z send --to northwind --amount 900  --label "Kestrel DAO"         --memo "treasury custody, Kestrel DAO"
$Z send --to mill --amount 150 --label "Dockworkers Local 9" --memo "solidarity from the docks"
$Z send --to mill --amount 220 --label "anonymous"           --memo "for the families"
$Z send --to mill --amount 95  --label "Riverside Teachers"  --memo "we stand with you"
$Z send --to mill --amount 60  --label "a nurse at St. Anne's" --memo "keep going"
$Z send --to lender --amount 5000 --label "lender's savings" --memo ""

step "Reserves locked: one note of exactly T each"
X=$($Z reserve --wallet northwind --amount 5000 | sed -E 's/^block ([0-9]+).*/\1/')
XM=$($Z reserve --wallet mill --amount 480 | sed -E 's/^block ([0-9]+).*/\1/')
echo "Northwind reserve at block X=$X, Mill reserve at block $XM"

step "Time passes: business as usual from the operating notes"
$Z devnet mine --blocks 8
$Z send --to northwind --amount 300 --label "Helios Capital" --memo "client deposit: Helios Capital, acct 4471"
$Z send --from northwind --to clients --amount 250 --memo "withdrawal: Orbit FO to their cold wallet"
$Z send --from mill --to members --amount 30 --memo "strike pay wk1: M. Okafor, J. Lindqvist, +28"
$Z devnet mine --blocks 12

step "Quickline borrows 5000 ZEC one block before the check"
$Z send --from lender --to quickline --amount 5000 --reserve --memo "short-term loan, repay after audit"
$Z devnet mine --blocks 1
H=$($Z devnet status | sed -E 's/.* tip ([0-9]+) .*/\1/')
echo "check height H=$H"

step "Registry"
ZSOLV_RATE_LIMIT=0 ZSOLV_REBUILDS_PER_HOUR=0 nohup $Z serve >"$D/serve.log" 2>&1 &
echo $! >"$D/serve.pid"
for i in $(seq 1 50); do curl -s "$URL/api/state" >/dev/null 2>&1 && break; sleep 0.2; done

cp "$D/wallets/northwind.json" "$D/wallets/shadow.json"
# Funds are unlisted: `zsolv fund` replies with the fund's share-link token.
link_of() { python3 -c 'import json,sys; print(json.load(sys.stdin)["link"])'; }
MILL_LINK=$($Z fund --url "$URL" --wallet mill --id mill --name "Mill Workers' Strike Fund" \
  --description "Strike pay for 120 workers at the Halden textile mill. Fictional demo fund." \
  --obligation "Strike pay: 120 workers x 1 ZEC/week x 4 weeks=480" --min-held-blocks 20 \
  --zec-usd 40 --price-time "demo price, 1 Nov 2026" | link_of)
NORTHWIND_LINK=$($Z fund --url "$URL" --wallet northwind --id northwind --name "Northwind Custody" \
  --description "Holds shielded ZEC for institutional clients. Proves its reserve without revealing clients, addresses or history. Fictional demo." \
  --obligation "Client balances (institutional)=4000" --obligation "Client balances (retail)=1000" \
  --min-held-blocks 20 --zec-usd 40 --price-time "demo price, 1 Nov 2026" | link_of)
QUICKLINE_LINK=$($Z fund --url "$URL" --wallet quickline --id quickline --name "Quickline Exchange" \
  --description "Claims a 5000 ZEC reserve. Its coins arrived the block before the check. Fictional demo." \
  --obligation "Customer balances=5000" --min-held-blocks 20 --zec-usd 40 --price-time "demo price, 1 Nov 2026" | link_of)
SHADOW_LINK=$($Z fund --url "$URL" --wallet shadow --id shadow --name "Shadow Custody" \
  --description "Tries to count Northwind's reserve note as its own. Fictional demo." \
  --obligation "Customer balances=5000" --min-held-blocks 20 | link_of)
for l in "$MILL_LINK" "$NORTHWIND_LINK" "$QUICKLINE_LINK" "$SHADOW_LINK"; do
  [ -n "$l" ] || { echo "a fund registration returned no share link" >&2; exit 1; }
done
step "An auditor, approved by the registry operator"
$Z authority new --name "Acme Audit LLP" --role auditor --url "$URL" >/dev/null
AUDITOR=$($Z authority show | sed -n 's/^public key //p')
$Z registry approve-authority --key "$AUDITOR" --name "Acme Audit LLP" --role auditor
$Z viewing-key-demo --wallet northwind --fund-id northwind
$Z viewing-key-demo --wallet mill --fund-id mill

step "Mill publishes round 1 (the green reference)"
$Z prove --wallet mill --fund-id mill --threshold 480 --since "$XM" --at "$H" --out "$A/mill.json"
$Z submit "$A/mill.json" --url "$URL"

step "Backup proofs, in case proving fails live (not submitted)"
$Z prove --wallet northwind --fund-id northwind --threshold 5000 --since "$X" --at "$H" --out "$A/backup/northwind.json"
python3 -c 'import json,sys; a=json.load(open(sys.argv[1])); a["threshold_zat"]*=2; json.dump(a, open(sys.argv[2], "w"), indent=2)' \
  "$A/backup/northwind.json" "$A/backup/northwind-tampered.json"
$Z prove --wallet quickline --fund-id quickline --threshold 5000 --at "$H" --cheat-claim-since "$X" --out "$A/backup/quickline-relabel.json"
$Z prove --wallet shadow --fund-id shadow --threshold 5000 --since "$X" --at "$H" --out "$A/backup/shadow.json"

step "Cheat sheet"
cat >"$D/CHEATSHEET.md" <<EOF
# Live demo cheat sheet

Prepared $(date '+%Y-%m-%d %H:%M'). Reserve locked at **X = $X**, check height **H = $H**.

In every terminal you use, first run:

    cd $(pwd)
    export ZSOLV_DATA=$D
    alias zsolv=./target/release/zsolv

Funds are unlisted: each page is reachable only through its share link.
Browser tabs to open beforehand:
- Landing:   $URL
- Mill:      $URL/#/v/$MILL_LINK   (green, the reference)
- Northwind: $URL/#/v/$NORTHWIND_LINK
- Quickline: $URL/#/v/$QUICKLINE_LINK
- Shadow:    $URL/#/v/$SHADOW_LINK

## 1. The problem (landing page + viewing-key panel)
Landing page: the viewing-key ledger redacted down to one line. Mill's link is green (already proven); Northwind, Quickline and Shadow are grey (registered, no proof yet).
> "There's no public list of funds. A fund shares its link with whoever needs to check it."
Open Northwind's page and scroll to the viewing-key panel: client names, account numbers, withdrawals.
> "This is what a viewing key gives away, past and future, forever. We publish one line instead."

## 2. Northwind proves its reserve
    zsolv prove --wallet northwind --fund-id northwind --threshold 5000 --since $X --at $H --out /tmp/northwind.json
> "Two zero-knowledge proofs, one at block $X and one at block $H, over the same notes. A few seconds."

    zsolv verify /tmp/northwind.json
> "Anyone can run this. It rebuilds the trees from public chain data and trusts nothing in the file. 13 checks."

    zsolv submit /tmp/northwind.json
Refresh Northwind's page: it turns **green**. Press **Verify** on the page: the registry re-checks it live.
Backup if proving fails: \`zsolv submit $A/backup/northwind.json\`

## 3. Tamper: claim double the amount
    python3 -c 'import json;a=json.load(open("/tmp/northwind.json"));a["threshold_zat"]*=2;json.dump(a,open("/tmp/bad.json","w"))'
    zsolv verify /tmp/bad.json
Expect **INVALID**: the signed message no longer matches, and the notes prove only 5000 < 10000.
Backup: \`zsolv verify $A/backup/northwind-tampered.json\`

## 4. Cheat: Quickline borrowed coins the block before the check
    zsolv prove --wallet quickline --fund-id quickline --threshold 5000 --since $X --at $H --out /tmp/q.json
Expect: refused, "you can prove held since $((H-1)), not since $X".
> "Classic audit-day fraud: borrow the money, show it, return it. It can't prove it held the coins."

Fake the height anyway:

    zsolv prove --wallet quickline --fund-id quickline --threshold 5000 --at $H --cheat-claim-since $X --out /tmp/q2.json
    zsolv submit /tmp/q2.json
Expect **REFUSED**: "proof does not verify against block $X".
Backup: \`zsolv submit $A/backup/quickline-relabel.json\`
Refresh Quickline's page: **red** (its only submission was refused).

Optional: the honest claim is all it can make:

    zsolv prove --wallet quickline --fund-id quickline --threshold 5000 --at $H --round 2 --out /tmp/q3.json
    zsolv submit /tmp/q3.json
Expect **ACCEPTED**, "held since $((H-1))". Refresh: **amber**, because it hasn't held the coins for the promised 20 blocks.

## 5. Cheat: Shadow counts Northwind's coins as its own
    zsolv prove --wallet shadow --fund-id shadow --threshold 5000 --since $X --at $H --out /tmp/s.json
    zsolv submit /tmp/s.json
Expect **REFUSED**: "note already counted by another fund" (it doesn't say which one).
> "Two custodians can't count the same coins, and the registry still doesn't learn which note it was."
Refresh Shadow's page: **red**.
Backup: \`zsolv submit $A/backup/shadow.json\`

## 6. The treasurer's side
    zsolv console
Choose "Open a wallet already here", then \`northwind\` (registry URL $URL if asked).
Press **Prove and publish**: each step shows its timing. Then refresh Northwind's page: round 2.
> "The keys never leave this machine. The console only answers its own page."

## 7. Auditors and regulators (Compliance)
Start the auditor's console in a second terminal (same setup lines as above):

    zsolv console --authority --addr 127.0.0.1:7071

In the fund console (Northwind), open **Compliance**: the registry already certified each published round.
Choose **Acme Audit LLP**, add a note (e.g. "Q4 2026 reserve review") and press **Request certification**.
> "The company never sends its books. It asks an approved auditor to certify one round."

In the auditor's console: the request appears. Press **Verify on this machine**: 13 of 13 checks, against the auditor's own chain data.
Press **Sign and issue certificate**.
> "The auditor checks the zero-knowledge proof itself and signs with its own key. It still never sees a balance or an address."

Back in the fund console: the certificate is listed. **View**, then **Print or save as PDF**, or **Download**.
Anyone can check the file:

    zsolv certificate verify ~/Downloads/z-solvency-certificate-northwind-*.json --url $URL

Expect "signature valid" and "issuer is an approved authority". Certificates never appear on the public link page.

## Reset
To start over: \`scripts/prep-live-demo.sh\`  (wipes $D and restarts the registry)
To stop the registry: \`kill \$(cat $D/serve.pid)\`
EOF

step "Ready"
echo "Registry: $URL   (log: $D/serve.log)"
echo "Cheat sheet: $D/CHEATSHEET.md   (X=$X, H=$H)"
echo "Share links:"
echo "  Mill       $URL/#/v/$MILL_LINK"
echo "  Northwind  $URL/#/v/$NORTHWIND_LINK"
echo "  Quickline  $URL/#/v/$QUICKLINE_LINK"
echo "  Shadow     $URL/#/v/$SHADOW_LINK"
