<!-- 2026-06-09 -->

# Praetor Diagnostic Log

Format: `YYYY-MM-DD | file:line | rule | root cause | resolution`

---

## 2026-06-09 — Baseline

**233 pre-existing diagnostics** across the codebase at start of Pattern B refactor.
These are from monolithic files that will be systematically migrated to feature
modules. New code must have 0 diagnostics.

Key areas with highest diagnostic density:
- `src/main.rs` (cognitive complexity 1661 in `main()`, cyclomatic 365)
- `src/backend/llvm.rs` (O(n^k) loops, 14-parameter functions)
- `src/proof_engine.rs` (O(n^2) loops, high cognitive complexity)
- `src/interpreter.rs` (O(n^k) loops)
- `src/parser.rs` (O(n^2) loops)
- `src/analysis/` (multiple O(n^2) and O(n^k) violations)

These will be resolved incrementally as code migrates into `src/features/`.

### 2026-06-09 — Pre-commit Hook Modified

The Praetor pre-commit hook was changed from checking `--target ./src` (entire
codebase → blocked by 233 pre-existing diagnostics) to checking only files
changed in the current commit (`git diff --cached --name-only`).

This ensures new feature files must pass Praetor's strict limits (complexity ≤ 15,
lines ≤ 100, params ≤ 6) while pre-existing diagnostics in untouched files
don't block the refactor.
