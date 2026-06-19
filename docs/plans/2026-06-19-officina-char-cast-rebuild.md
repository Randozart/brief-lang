# Plan: Officina Char→String Cast — Complete

**Date:** 2026-06-19  
**Status:** ✅ Complete — all steps executed  
**Root cause:** `adapt_to_i64` in `emit_stmt.rs` and `emit_trg_load_finish` in
`emit_toplevel.rs` assumed Type::Char registers were `i32` at the LLVM level,
but ALL emit_expr paths produce `i64` (boxed). The `zext i32 %reg to i64`
instructions were type errors when the register was already `i64`.

## Root Cause (Revised)

The `officina` binary was built with a compiler where four code paths
assumed Type::Char LLVM registers were `i32` when they were actually `i64`:

| Path | File:Line | Bug | Fix |
|------|-----------|-----|-----|
| `adapt_to_i64` | `emit_stmt.rs:20-23` | `zext i32 %r to i64` when reg is already `i64` | No-op: return `r.name.clone()` |
| `emit_trg_load_finish` | `emit_toplevel.rs:305-306` | `add i32 0, %raw` producing `i32` | `zext i32 %raw to i64` |
| Let-binding Char | `emit_expr.rs:134-144` | `zext i32 %reg to i64` when reg is already `i64` | `add i64 0, %reg` (no zext) |
| Enum variant storage | `emit_expr.rs:505-508` | `zext i32 %raw to i64` when reg is already `i64` | No-op: return `raw.name.clone()` |

Plus one inconsistency: `TtyReadKey` intrinsic at `emit_expr.rs:895-897`
returned a `phi i32` with Type::Char — fixed to zext the phi to `i64`.

## SSA extractvalue gap also fixed

Added `"i32"` arm to the SSA extractvalue `_` default in
`emit_expr.rs:115-118` as a safety net. Currently unreachable (all fields
pre-extracted) but correct for future edge cases.

## Verification

- `printf "hello\x03" | timeout 2 ./officina` → `> hello` shown in prompt, exit 0
- `cargo test --lib` → 1072 passed, 0 failed
