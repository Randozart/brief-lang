# Protocol+Maxbits Type Derivation + Native %State Widths

**Date:** 2026-07-26
**Status:** Implementation

## Problem

Three benchmarks (float_math, print_loop, kalman_filter_runtime) fail at LLVM
IR verification because phi nodes declared as i64 receive values of other types
(float, i8). Root cause: `push_field_type` hardcodes type name matching
(`*ty == Type::float()`), and the phi declaration in the counter loop
hardcodes `"i64"` instead of reading the field's actual type from
`field_types`.

Commit `8c13cd99` reverted a correct per-field phi-type fix from `7e511696` to
hardcoded `"i64"`, which broke float and narrow-int fields.

## Architectural Principle

LLVM types are derived from protocol membership + maxbits metadata, not from
hardcoded type name matching. The `ResolvedType` in `TypeUniverse` stores both
the protocol (via `Cast.#<Name>` properties and `category` field) and the
bit-width range (`min_bits`, `max_bits`). `push_field_type` reads these to
determine each %State field's LLVM type.

| Protocol | maxbits | %State type | Notes |
|----------|---------|------------|-------|
| `#Float` | ≤32 | `"float"` | Float, Half |
| `#Float` | ≤64 | `"double"` | Double |
| `#Float` | >64 | `"i64"` | FP128 — lossy, accepted limitation |
| `#Int`/`#UInt` (flexible) | 0-64 | `"i64"` | Int/UInt — uniform layout, narrowing handles SSA |
| `#Int`/`#UInt` (exact) | 8-c | native `iN` | Int8→i8, Int32→i32, Int128→i128 |
| `#Bool` | 8 | `"i64"` | Uniform — already stored as i8→zext |
| `#Ptr` | 64 | `"i64"` | Uniform — ptrtoint conversion |
| `#String` | — | 2×`i64` | SSO special case, unchanged |

## Changes

### 1. `src/type_universe/mod.rs` — Expand PRIMORDIALS

Add every combinatorial (protocol, maxbits) pair as a primordial type so the
protocol+maxbits query always finds a `ResolvedType`.

Types to add: Int128, UInt128, Half, BFloat, FP128, X86_FP80.

### 2. `src/backend/llvm/mod.rs:988-994` — `push_field_type` uses protocol+maxbits

Replace:
```rust
let llvm_ty = if *ty == Type::float64() { "double" }
    else if *ty == Type::float() { "float" }
    else { "i64" };
```

With query that checks `Cast.#Float` / `category` protocol membership, reads
`max_bits` from `ResolvedType`, and maps to the appropriate LLVM type using
the table above.

Flexible Int/UInt (min_bits != max_bits) keep `"i64"` in %State — the
narrowing pass optimizes SSA registers but does not change the struct layout.
Exact integer types (min_bits == max_bits, e.g. Int32) get native `"iN"`.

### 3. `src/backend/llvm/loop_engine/counter.rs` — Fix all hardcoded `"i64"` sites

Five sites emit `"i64"` when they should use the field's actual type from
`field_types`:

| Lines | Site | Fix |
|-------|------|-----|
| 360-365 | Counter phi decl | Read type from `field_types[counter_idx]` |
| 390 | Per-field phi decl | Read type from `field_types[fname]` |
| 402-404 | Exit condition | `sext` narrow counter to i64 before comparing with bound |
| 437-439 | Counter increment | Use counter phi's type for `add`/`sub` |
| 465 | Backedge identity | Use `field_ty` for `add` (same as float branch) |

### 4. `src/backend/llvm/helpers.rs:2727` — `adapt_to_i64` widens non-i64 ints

The default arm `_ => reg.name.clone()` assumes every non-float int register
is already `i64`. With native %State widths, an Int32 field load gives `i32`.
Add widening via `sext` when `llvm_type(&reg.ty)` returns a non-i64 integer.

### No changes needed

- `load_field_type` / `store_field_type` — already read from `field_types`
- `ensure_typed_value` — already handles all type conversions
- `operator_llvm_type` — already reads `llvm_type` property from universe
- `llvm_type` — already handles narrowing
- `is_native_float` — already checks protocol via `category` property

## Impact on existing benchmarks

All existing benchmarks use `Int` (flexible, stays `"i64"`) and `Float`/`Double`
(already `"float"`/`"double"`). The only behavioral change is:
- Float/Double field derivation now reads protocol+maxbits instead of `*ty == Type::float()` (same result)
- Phi declarations match their field type (fixes float_math, print_loop, kalman_filter)
- `adapt_to_i64` defensively widens any non-i64 int (currently not triggered by existing code)

## Validation

1. `cargo build` — zero new warnings
2. `cargo test --lib` — all 1035 tests pass
3. float_math, print_loop, kalman_filter_runtime compile and MATCH
4. mandelbrot MISMATCH rechecked (may cascade-fix)
