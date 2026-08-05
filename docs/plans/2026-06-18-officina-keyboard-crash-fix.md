# Officina Keyboard Input Crash Fix

**Date**: 2026-06-18
**Component**: LLVM Backend — Char→String cast + string concat free
**Status**: Implementation complete

## Problem

After fixing the epoll-wait bug, officina reads keyboard input but crashes
with SIGSEGV when the user types any character.

## Root Cause

The `(String)k` Char→String cast emitted `call i8* @__chr_to_str(i32 %char)`
which returns a raw C string (`static char buf[2]`).  The `emit_inline_concat`
function then reads `hdr[0]` (cap) and `hdr[1]` (len) from this 2-byte buffer
as 8-byte i64 values, reading **garbage** into the length field.  `memcpy`
copies that garbage number of bytes → buffer overflow → SIGSEGV.

This is a compiler bug: the backend must produce a valid Briv string struct
`{cap, len, data}` from a Char→String cast, not a raw C string pointer.

## Fix

### 1. Char→String cast (`emit_expr.rs:3310-3335`)

Replaced the `__chr_to_str` call with inline struct construction:

1. `malloc(24)` for `{cap: i64, len: i64, data: [1]i8, null: i8}`
2. Store data pointer at offset 0 (cap)
3. Store `1` at offset 8 (len)
4. Store char byte at offset 16
5. Store null at offset 17
6. Return `ptrtoint(%alloc)` as the box pointer

### 2. Test assertion (`tests.rs:3890`)

Changed from checking `"call i8* @__chr_to_str"` to checking
`"call i8* @malloc(i64 24)"` and `"store i64 1, i64*"`.

### 3. Concat operand free re-enabled

The tag-based free logic in `emit_inline_concat` (bit-0 = static/heap)
was temporarily disabled.  With the proper Char→String struct, all
code paths produce valid `{cap, len, data}` structs, so the free
logic can be restored.  Every non-heap string path now correctly
sets bit-0 = 1:
- `Expr::String` literals: tagged ✅
- State-initialized strings: tagged ✅ (2026-06-18 fix)
- `int_to_str#()` / `__int_to_str__` results: heap, bit-0 = 0 ✅
- Char→String cast results: heap, bit-0 = 0 ✅

## Previous Fixes (same session)

- `emit_ssa_main` uses `emit_trg_event_epoll_wait` instead of `__rt_wait()`
- `emit_toplevel.rs`: state init stores tagged string pointers (bit-0 = 1)
- `mod.rs` + `emit_expr.rs`: `__int_to_str` → `__int_to_str__` name fix
- `persistence.bv`: removed `import# "std/json"` (LLVM enum construction bug)

## Verification

1. `cargo test --lib` — all tests pass
2. `cargo build --release` — no warnings
3. Recompile officina: `./target/release/briv-compiler build <path>/officina.bv`
4. Run officina, type characters, verify no crash
