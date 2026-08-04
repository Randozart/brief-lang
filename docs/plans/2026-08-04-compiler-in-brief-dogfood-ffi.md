# Compiler-in-Brief — dogfooding the GLUE FFI

**Date:** 2026-08-04
**Status:** Active plan
**Branch:** `compiler-in-brief` (worktree `../brief-compiler-dogfood`)
**Related:** `docs/architecture/glue-ffi.md`, `docs/guides/ffi-and-export.md`,
`docs/plans/2026-08-04-zero-friction-ffi-gate.md`

---

## Goal

The compiler uses its own FFI: **one Rust compiler pass written in Brief,
compiled by `briefc` to a native `.a`, linked into `briefc` at build time, and
called through the same GLUE C-ABI boundary every host language uses.**
Near-native (~2.4% linked overhead, in-process, no subprocess). This is the
incremental compiler-in-Brief path.

## The PoC: `compute_export_needs_state` in Brief

The smallest self-contained *decision* pass the glue layer depends on (it
decides whether every export shim carries `%state`). Two production call
sites: `src/backend/llvm/mod.rs:1691` and `src/glue/export.rs:116`.

## Phases

### P1 — the handoff (the core work)
A serializer emits a **tagged Data Brief projection** per export: name, params,
return type, the declared **state-field names**, and the **body as a tagged
tree** (statement kinds, identifier references, call names) — enough for Brief
to detect state-field access and build the export→export call graph, without
the Rust answer. This is the long-lived interchange contract.

### P2 — `lib/compiler/needs_state.bv`
A Brief program that reads the projection (`CStr`), walks each export body for
state-field reads (including inside frgn-call args — the marshalled form),
builds the transitive export→export call graph, and emits a **needs_state
bitmask** (one bit per export, as `Int`). **Contracts on the txn verify the
analysis.** Runs in the interpreter too (it is the reference).

### P3 — native linkage
A root **`build.rs`** runs `briefc build lib/compiler/needs_state.bv --library`
→ `libneeds_state.a` and links it into `briefc` (the rust-host pattern). No
circularity — the Rust `briefc` compiles the pass. `extern "C"` binding:
`needs_state_compute(projection: CStr) -> Int` (the bitmask); error code on
parse failure.

### P4 — integration + behavioral test
Replace both call sites with: serialize → `needs_state_compute(...)` → read the
bitmask. **Behavioral test**: `briefc export/bindings` on `boundary.bv` and
`node_bridge.bv` produce **byte-identical** shims to today; a transition test
asserts the Brief result equals the (now reference) Rust result on a corpus of
bridges. `cargo test --lib` green.

### P5 — generalize
Extract the reusable AST-projection serializer + Brief reader into
`lib/compiler/`; migrate a second pass (`soa_reorder`, permutation-only
handoff) to prove the pattern generalizes. Record the recipe in
`docs/architecture/compiler-in-brief.md`.

## Decisions

1. Handoff: **tagged Data Brief projection** (readable, established via
   `bridge-exports.dbvl`); packed raw buffers (the tamer style) are a later
   compaction.
2. Invocation: **linked `.a` via a root `build.rs`** — "compiled to work
   natively with Rust".
3. First pass: **`needs_state`** (analyzes bodies + a transitive graph).
   Fallback if the body serialization proves heavy: `soa_reorder`.
4. Performance bar: the ~2.4% linked overhead on a compile-time pass is a
   non-issue; the compute is native.

## Risks

- The body serialization is the crux (a statement-tree projection) — mitigated
  by the `soa_reorder` fallback.
- `briefc`'s build gains a briefc invocation in `build.rs` (the rust-host crate
  already proves this).
- The pass's output contract must be stable — tested behaviorally.

## Verification

`cargo test --lib` green; behavioral byte-identical shims; the transition test
(Brief result == Rust reference) on a corpus; Praetor on changed dirs.
