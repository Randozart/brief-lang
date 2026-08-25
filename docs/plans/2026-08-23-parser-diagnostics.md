# Parser Diagnostics + Match Dispatch Fix

**Date:** 2026-08-23
**Status:** active
**Trigger:** json.bv migration produced cascading misleading parse errors instead of actionable diagnostics

## Root cause

Three compounding parser issues:
1. **Dual match dispatch**: parse_statement routes Token::Match to parse_match_statement (legacy block-body/semicolon form), while parse_primary routes to parse_match_expr (expression/comma form). Inside defn bodies the statement form fires first — wrong grammar → cascade.
2. **No error recovery**: after first parse error inside a defn body, every subsequent line produces another error.
3. **Generic messages**: "expected Semicolon found '}'" doesn't tell the programmer WHY.

## Fixes

### F1 — Unified match dispatch
Route ALL Token::Match in statement position to parse_match_expr (expression form). Wrap result in Statement::Expression. Remove or bypass parse_match_statement from general dispatch.

### F2 — Diagnostic hints
Add targeted hints to common failure modes:
- expect(Semicolon) finds `}` after match → "defn body must end with '};'"
- expect(LBrace) after `=>` → context about expression-form arms
- Missing `term` before match in defn → suggest `term match`

### F3 — Error recovery
After parse error inside a defn/txn/node body: skip tokens to matching closing `}` at col 0, then continue parsing next top-level item. Report ALL errors in one pass.

## Test case

lib/std/json.bv (restored from archive) should produce either:
- Clean compilation, OR
- ONE clear error per real issue (not thirty cascading ones)
