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

## Session report (2026-08-04)

Completed, in order:

1. **Serializer** (P1): `src/analysis/needs_state_projection.rs` — flat-preorder
   token format, tested.
2. **Real compiler bugs fixed along the way** (each committed with the
   investigation that found it):
   - import resolver swallowed imported-file parse errors (`unwrap_or_default`)
     — now a visible warning; this surfaced a systemic `onst` → `const` typo
     (546×, 21 `std/os` files), `Slice<T>.prop Size: len` (prop parser needs
     call syntax), and string.bv's legacy `..` slices.
   - generic struct layout was silently zeroed: `List<T>.len` collided with
     `inner.cap` at offset 8 (type_size(Ptr) via universe = 0; normalizer
     slot-sum read raw rt.bytes = 0 for flexible Int/String; re-registering
     `type Int: #Int` wiped the Cast.#* properties). List init/push verified.
   - `List.init` allocated 16 BYTES but advertised cap 16 ELEMENTS (overflow);
     grow(cap) added (memcpy via `Copy#`).
   - a let reassigned inside a `when`/if guard demoted to an alloca AT the
     assignment site → LLVM dominance violation; emit_definition now
     pre-declares entry allocas for reassigned top-level lets.
3. **Verified language facts that shaped the pass**: `when` is an if-guard, not
   a while loop (interpreter + backend agree) → iteration must be recursion;
   recursion + `build --library` link/run correctly; List init/push/grow run.
4. **`lib/compiler/needs_state.bv`**: rewritten as PURE string scanning (no
   List reads — generic `T` element reads and String `+` are broken), recursion
   for iteration, `:` slices + `==` + `.^Len` for scanning. Type-checks clean.

**Blocked at P2 execution by the String-slicing codegen** (see BUGS.md): both
constant-bounds (narrowed to `Vector`) and dynamic-bounds slices return the
whole string — `s[a:b]` never creates a substring, and slicing a boxed String
param passes the i64 handle to brief_char_len as a raw ptr. The pass therefore
cannot build as a library yet. Next step: implement `brief_str_substr` in the
runtime + a `frgn __str_substr(s, a, b) -> String` (verify the frgn String
marshalling first — no stdlib frgn with String params is currently exercised),
OR fix the dynamic-slice codegen to construct a length-prefixed substring; then
test `needs_state_compute` against the Rust reference (`export_abi.rs`) on the
boundary/node/bridge corpus and proceed to P3 (root build.rs linkage).

