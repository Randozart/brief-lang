# Protocol-Only Float Fix + Rust Native-Speed + Python Parity

**Date:** 2026-08-03
**Status:** Active plan
**Branch:** `glue-host-callable`
**Related:** `docs/plans/2026-08-03-protocol-driven-glue-boundary.md`

---

## Motivation

Three asks from the author:

1. **Fix the Float bug** — `Float64` (bits 64) lowers to float32 because the
   Float category's LLVM resolver is `Fixed("float")`, ignoring the `bits`
   metadata; and `2.0 as Float64` emits a bitcast+sitofp mess instead of
   `fpext float to double`.
2. **Verify export to Rust at native Rust speed** — so parts of the compiler
   itself could be written in Brief without loss of efficiency.
3. **Guarantee parity with Python** — the GLUE path must be at least as fast
   as calling C through Python's ctypes.

The overriding constraint (the author's reminder): **only protocols are hard
coded.** No Brief type names (`Float64`, `Double`, `String`, `Int`, …) in Rust.
All resolution is protocol-property-driven and metadata-driven.

## P-A — Protocol-only Float width

**Root cause (verified):** the `#Float` category's LLVM type resolver is
`Fixed("float")` (`src/casting/graph.rs` `seed_protocol_llvm_types`), so a
64-bit Float type's `bits: 64` metadata is ignored. My earlier attempt mapped
`bits == 64 → "Double"` variant — that names a variant from width, the wrong
direction.

**The protocol-only fix:** the `#Float` protocol *is* the width semantics. Add
`LlvmTypeResolver::FloatWidth` and seed `("Float", "")` with it;
`resolve_llvm_type` derives the LLVM type from the type's `bits` metadata:

| `bits` | LLVM type |
|--------|-----------|
| 16 | `half` / `bfloat` (via the existing `disamb` disambiguation) |
| 32 | `float` |
| 64 | `double` |
| 80 | `x86_fp80` |
| 128 | `fp128` |
| default | `float` |

No type names appear — width comes from protocol + metadata. The explicit
`#Float<Double>` / `#Float<C_Double>` seeded variants stay for explicit
requests (`CDouble` keeps working).

**Cast emission:** trace why `2.0 as Float64` emits `sitofp i64 %t1 to double`
(the IntToFloat lane firing) and fix the Float-width cast to emit
`fpext float to double` / `fptrunc double to float` (the identity-path width
handling in `emit_cast_steps` must be reached).

**Tests:** `Float64` returns `double`; `2.0 as Float64` → `fpext`; `scale(x:
CDouble)` unchanged; `x * 2.0` with a CDouble operand works.

## P-B — Remove name matching (rule 18)

`string_concat::is_string_category` and `boundary_marshalling::resolve_category`
shortcut `name == "String"` / `name == "Data"`. The bootstrap `String`/`Data`
universe entries already carry `Cast.#String` / `Cast.#Data`
(`src/type_universe/mod.rs:153,199`), so the protocol-property check alone
covers them; declared boundary types resolve via their `#String<C_String>`
base chain. Remove the name shortcuts — pure protocol-property + base-chain
resolution, behavior preserved.

Grep gate: zero `Type::Custom.*==` / `== "String"` / `== "Int"` in the new
frontend passes.

## P-C — Rust native speed (measure; LTO deferred)

Add a Rust benchmark in `examples/glue-host/rust-host`: `feature_hash(count,
seed)` — Rust → Brief via `librank.a` (plain C ABI, zero marshalling) vs a
native Rust FNV-1a — per-call latency + throughput over N calls. The boundary
is one function call, so Brief should land within a few % of native Rust when
the work dominates. LTO (bitcode archive for `rustc -C lto` inlining) is
deferred until the measured overhead justifies it.

## P-D — Python parity

Fresh `make speed`: confirm Brief within ~10% of C through ctypes. The ~1.9µs
per-call is ctypes marshalling (identical for both); the `.so` compute is
native.

## P-E — Extensibility gate + docs

- Full `git grep` over `src/glue/`, `src/analysis/`, `src/backend/llvm/` for
  hardcoded type names — zero.
- `docs/architecture/casting-protocol.md`: document FloatWidth resolution.
- BUGS.md: narrow/clear the Float literal item.
- This plan.

## Completion Status (2026-08-03)

- **P-A (Float fix):** DONE. `FloatWidth` resolver (width from `bits` metadata,
  protocol-owned, no type names) + `FloatWidth` cast lane (`fpext`/`fptrunc`).
  `Float64` → `double`; `2.0 as CDouble` / `2.0 as Float64` → clean `fpext`;
  `scale(x: CDouble)` → `fadd/fmul double`. The old bitcast+sitofp corruption
  is gone. BUGS.md item marked FIXED.
- **P-B (extensibility):** DONE. Removed the `name == "String"/"Data"`
  shortcuts from the frontend passes (the bootstrap universe entries carry
  `Cast.#String`/`Cast.#Data`). Grep gate clean — only protocol categories are
  hardcoded.
- **P-C (Rust native speed):** DONE. The `.a` path now runs
  `opt -passes='default<O3>'` before llc — `llc -O3` alone didn't SROA the txn
  loop's allocas (2.2× slower). Measured `feature_hash(count=1000, 200k calls)`:

  | path | ns/call | vs native |
  |------|---------|-----------|
  | Rust → Brief (GLUE) | 1127 | 2.4% |
  | native Rust | 1101 | — |
  | C → Brief (.a) | 1092 | 1% |
  | native C | 1082 | — |

  The boundary is a single C-ABI call (~26ns), compute-dominated. This is the
  **compiler-in-Brief** path — near-native without LTO.
- **P-D (Python parity):** DONE. Python → Brief 2033ns vs Python → C 1927ns
  (within 5%) — both dominated by the ~1.9µs ctypes marshalling; the Brief
  compute is native.

## Cross-Cutting

- Additive match arms only; `cargo test --lib` before each commit; Praetor on
  changed dirs; docs updated in the same commit as structural changes.
