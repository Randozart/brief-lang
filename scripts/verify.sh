#!/usr/bin/env bash
set -euo pipefail

echo "=== cargo check ==="
cargo check

echo "=== Praetor check (no new ERROR diagnostics) ==="
# Fast check: verify no new ERROR-level diagnostics outside _monolithic/
praetor validate --json --target ./src > /tmp/praetor-current.json 2>/dev/null
python3 -c "
import json
base = json.load(open('praetor-baseline.json'))
curr = json.load(open('/tmp/praetor-current.json'))
diff = curr['total_diagnostics'] - base['total_diagnostics']
if diff > 0:
    print(f'FAIL: {diff} new diagnostics (baseline: {base[\"total_diagnostics\"]}, current: {curr[\"total_diagnostics\"]})')
    exit(1)
else:
    print(f'PASS: {curr[\"total_diagnostics\"]} diagnostics (baseline: {base[\"total_diagnostics\"]})')
"

echo "=== cargo test --lib ==="
cargo test --lib

echo "=== cargo kani (fast group) ==="
cargo kani --lib

echo ""
echo "=== Full Kani suite (requires --features kani_full) ==="
echo "Run separately: cargo kani --lib --features kani_full"
echo ""
echo "All verifications passed."
