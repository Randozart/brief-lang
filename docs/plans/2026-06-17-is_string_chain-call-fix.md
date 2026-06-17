# Fix: officina-cli SIGSEGV — Three Bugs Root-Caused and Fixed

**Date:** 2026-06-17
**Session:** Compiler Correctness — Root-Cause Every Runtime Crash

## Bugs Found and Fixed

### Bug 1: `is_string_chain` missing `Expr::Call` arm

**Symptom**: `draw_prompt` crashed with SIGSEGV in `int_to_str`. The
`n >= 10` concat path emitted `add i64` instead of proper string concat.

**Root Cause**: `src/backend/llvm/emit_expr.rs:2763` — `is_string_chain()`
detects string `+` for inline concat but doesn't handle `Expr::Call`.
`int_to_str(2) + int_to_str(3)` became `add i64` of two struct pointers.

**Fix**: Added `Expr::Call(name, _)` arm checking `defn_return_types` for
`String`/`Data` return type.

**File**: `src/backend/llvm/emit_expr.rs:2777-2783`

### Bug 2: `\0` char escape not handled in lexer

**Symptom**: `'\0'` parsed as backslash (ASCII 92) instead of null (0).
Process_input precondition `keypress != '\0'` was always true when keypress
was 0, causing process_input to fire spuriously every tick.

**Root Cause**: `src/lexer.rs:371-382` only handles `\n`, `\t`, `\\`, `\'`,
and `\u{...}`. `\0` falls through to `inner.chars().next()` → `\` (92).

**Fix**: Added `if inner == "\\0" { return Some('\0'); }`.

**File**: `src/lexer.rs:371-374`

### Bug 3: `done_{name}` SSA dispatch branches to exit instead of next txn

**Symptom**: After fixing Bug 2, officina booted but exited before the render
txn could fire. `done_process_input: br label %done` skipped the render txn.

**Root Cause**: `src/backend/llvm/loop_engine.rs:778`: `done_l` (precondition
false) unconditionally branches to `%done` (exit) instead of `%{skip_l}` (next
txn). The June-14 fix claimed to address this but only fixed boot's done label;
the template kept `br label %done` for all txns.

**Fix**: Changed `br label %done` to `br label %{skip_l}`. Precondition false
now chains to the next txn's skip label, which falls through to post-loop code.

**File**: `src/backend/llvm/loop_engine.rs:778`

## Results

- **Officina**: boots, renders top bar + prompt correctly, no crash
- **Tests**: 911/911 pass (same as pre-fix)
- **Root cause chain**: Bug 2 → spurious process_input → heap corruption →
  Bug 1 → garbage string pointer → SIGSEGV. Bug 3 was a separate issue
  masked by Bug 2.

## Remaining Known Issue

- **`<-` arrow stub**: LLVM backend emits `i64 0` for all List mutations
  (`ArrowMut`, `ArrowDiscard`, `ArrowTransfer`). `&history <- rec` silently
  drops items. Not related to the SIGSEGV but needed for full officina
  functionality.
