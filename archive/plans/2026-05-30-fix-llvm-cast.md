# Fix: Expr::Cast is a no-op in the LLVM Backend

**Date:** 2026-05-30
**Scope:** `src/backend/llvm.rs` — `Expr::Cast` match arm (line 1019)
**Root cause:** Every subsystem treats `Cast` as a pass-through:

| System | Line | Behavior |
|---|---|---|
| Interpreter | rust:1880 | `eval_expr(inner)` — just evaluates inner |
| Typechecker | rust:1378 | Returns `Custom("unknown")` — doesn't propagate type |
| LLVM backend | llvm.rs:1019 | `add i64 0, %inner ; cast` — emits only a comment |

**Impact:** Cannot use `Float(index)` or any type conversion in LLVM-compiled code. This blocks DSP benchmarks and any non-trivial numeric computation.

## LLVM Register Encoding

All values in LLVM registers are i64, but their native representations differ:

| Briv Type | Native LLVM Type | i64 Encoding |
|---|---|---|
| Int / UInt | i64 | native |
| Float | float | bitcast float→i32, zext to i64 |
| Bool | i8 → i1 | 0 or 1 in i64 |
| Char | i32 | zext to i64 |
| String / Data | i8* | ptrtoint to i64 |

## Fix

Replace the no-op arm with type-aware conversion. Determine source type via:

1. `self.register_types.get(&inner_register)` — tracks let-bound/fresh registers
2. `self.is_float_expr(inner)` — float-specific path
3. Pattern matching the inner expression variant (Integer, Float, Bool, etc.)

Then emit the appropriate LLVM conversion instructions for every (source, target) pair:

| Source → Target | LLVM IR |
|---|---|
| Int → Float | trunc→i32, bitcast→float, sitofp float←int... (need two values: the Int as i64 AND the float result) — actually simpler: `sitofp i64 %r to float`, then bitcast→i32, zext→i64 |
| Float → Int | trunc→i32, bitcast→float, fptosi float→i64 |
| Float → Float | nop |
| Int/UInt → Int/UInt | nop (same repr) |
| Int → Bool | `icmp ne i64 %r, 0` → zext i1→i64 |
| Bool → Int | nop (already 0/1) |
| Int → Char | nop (both i64) |
| Char → Int | nop (both i64) |
| Float → Bool | trunc→i32, bitcast→float, fcmp une float 0.0 → zext i1→i64 |
| Bool → Float | select i1 %r, float 1.0, float 0.0 → bitcast→i32, zext→i64 |
| String → String | nop |
| Anything → String/Data | inttoptr i64→i8* → ptrtoint i8*→i64 (no real conversion within i64 world) |
| Same type pair | nop (`add i64 0, %r`) |

## Recording Result Type

After conversion, record `target_type` in `self.register_types.insert(v.clone(), target_type)` so downstream float-aware ops (binop, fcmp) detect the float register.
