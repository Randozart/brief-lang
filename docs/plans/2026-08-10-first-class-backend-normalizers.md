# First-class backend normalizers + backend folder reorg

**Date:** 2026-08-10
**Status:** active
**Related:** `docs/plans/2026-07-31-frontend-driven-dispatch.md` (module-táméteorem), rules 14/18 (no type-name matching).

## Problem

`register_typedefs` — the only pass that registers user `TypeDef`s into the
`TypeUniverse` — lives in `src/backend/llvm/normalizer.rs` and runs only for
`Llvm|Gpu` (compile.rs:509). Consequences:

1. **wasm/Webstack**: `webstack_normalizer::normalize` (compile.rs:513) never
   registers typedefs. When `LlvmBackend` runs with `wasm32` as target
   (compile.rs:1283), the universe holds only primordials → custom types fail
   to resolve.
2. `webstack_normalizer` name-matches `rt.base` for `js_type` (rule 18
   violation) instead of deriving the native type from the casting graph.
3. `state_layout` (llvm/mod.rs:3931) emits a constant-zero stub instead of real
   per-field `FieldLayout` (offset/size/`TypeTag`) consumed by
   `glue/web_generator.rs`.

## Principle

Each backend owns a **first-class normalizer pass** that converts Briv protocol
+ metadata into the backend's **native type**:

| Backend | Native type | Derived via |
|---|---|---|
| LLVM | IR type | `resolve_llvm_type` (universe + CastingGraph) |
| Webstack | `js_type`/`TypeTag` | `protocol_category` (Cast.#) |
| CIRCT | bit_width | Cast.# properties |
| SPIR-V | kernel/op support flags | ops-validation + Cast.# |
| VM | minimal (VM is untyped) | shared `register_typedefs` only |

Shared helper: `register_typedefs` (llvm/normalizer.rs) + its layout
computations move to `src/backend/register_types.rs`, backend-agnostic, still
universe/Cast.#-driven.

## Work items

1. Extract `register_typedefs`/`compute_layout_total_bits`/`layout_pattern_bits`/`attach_layout_fields`
   → `src/backend/register_types.rs`. LLVM normalizer calls it (behavior unchanged).
2. Folder reorg with `git mv`: `circt.rs`/`circt_normalizer.rs` → `circt/{mod.rs,normalizer.rs}`;
   `webstack.rs`/`webstack_normalizer.rs` → `webstack/{mod.rs,normalizer.rs}`. Update `mod.rs`,
   `compile.rs`, `features/*.rs` paths.
3. Rewrite `webstack/normalizer.rs`: `register_typedefs(items, universe, 32)` + per-universe-type
   `protocol_category` → `js_type` (no name matching) + `build_supported_ops` + strip.
4. Fix `webstack.rs::signal_type_for` via `protocol_category` (category → `SignalType`); delete
   `TempPhase14`/debug scaffolding.
5. Wire `TypeTag` derivation + real per-field `state_layout` export (replaces 4-constant stub).
6. CIRCT/SPIR-V normalizers: shared `register_typedefs` + keep existing validation/strip.
   VM minimal normalizer: shared `register_typedefs` only (VM untyped; `vm_field_size` stays).
7. Delete dead legacy files: `c.rs`, `rust.rs`, `verilog.rs`, `vhdl.rs`, `cobol.rs`,
   `tcl_generator.rs`, `x86_64.rs`, `aarch64.rs`, `router.rs`, `wasm.rs`.
   Keep `metadata.rs` (used), `bindgen.rs` (test), `normalizer.rs` (shared intrinsics).
8. Tests + Praetor + docs (this plan, `docs/architecture/backend-architecture.md`).

## Why VM keeps a normalizer

VM exists for **partial compilation** and the **tamer system**. Its normalizer
must be minimal but present so the universe is uniformly populated for any
backend — a hand-rolled partial path (skipping registration) would let the
normalizer invariant rot for the one backend that exists to support partial
work.