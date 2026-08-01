#!/usr/bin/env bash
set -euo pipefail

echo "=== cargo check ==="
cargo check

echo "=== Praetor check (no new ERROR diagnostics) ==="
# 2026-08-01: --target is a DIRECTORY, not a file. The old `--target ./src`
# invocation is correct here (src is a directory); the previous baseline
# comparison was against a stale June schema ({total_diagnostics} count) that
# no longer matches praetor's current JSON ({failures, passed,
# total_diagnostics}). Report the count but treat the stale baseline as
# informational until it is re-captured at the next full-project checkpoint.
praetor validate --json --target ./src > /tmp/praetor-current.json 2>/dev/null || true
python3 -c "
import json
curr = json.load(open('/tmp/praetor-current.json'))
print(f'praetor: {curr[\"total_diagnostics\"]} unproven diagnostics, passed={curr[\"passed\"]}')
if not curr['passed']:
    print('FAIL: praetor reports unproven diagnostics in src/')
    exit(1)
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
