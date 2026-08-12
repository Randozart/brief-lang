# Compiler-in-Briev — dogfooding the GLUE FFI

**Date:** 2026-08-04
**Status:** Active plan
**Branch:** `compiler-in-briev` (worktree `../briev-compiler-dogfood`)
**Related:** `docs/architecture/glue-ffi.md`, `docs/guides/ffi-and-export.md`,
`docs/plans/2026-08-04-zero-friction-ffi-gate.md`

---

## Goal

The compiler uses its own FFI: **one Rust compiler pass written in Briev,
compiled by `brievc` to a native `.a`, linked into `brievc` at build time, and
called through the same GLUE C-ABI boundary every host language uses.**
Near-native (~2.4% linked overhead, in-process, no subprocess). This is the
incremental compiler-in-Briev path.

## The PoC: `compute_export_needs_state` in Briev

The smallest self-contained *decision* pass the glue layer depends on (it
decides whether every export shim carries `%state`). Two production call
sites: `src/backend/llvm/mod.rs:1691` and `src/glue/export.rs:116`.

## Phases

### P1 — the handoff (the core work)
A serializer emits a **tagged Data Briev projection** per export: name, params,
return type, the declared **state-field names**, and the **body as a tagged
tree** (statement kinds, identifier references, call names) — enough for Briev
to detect state-field access and build the export→export call graph, without
the Rust answer. This is the long-lived interchange contract.

### P2 — `lib/compiler/needs_state.bv`
A Briev program that reads the projection (`CStr`), walks each export body for
state-field reads (including inside frgn-call args — the marshalled form),
builds the transitive export→export call graph, and emits a **needs_state
bitmask** (one bit per export, as `Int`). **Contracts on the txn verify the
analysis.** Runs in the interpreter too (it is the reference).

### P3 — native linkage
A root **`build.rs`** runs `brievc build lib/compiler/needs_state.bv --library`
→ `libneeds_state.a` and links it into `brievc` (the rust-host pattern). No
circularity — the Rust `brievc` compiles the pass. `extern "C"` binding:
`needs_state_compute(projection: CStr) -> Int` (the bitmask); error code on
parse failure.

### P4 — integration + behavioral test
Replace both call sites with: serialize → `needs_state_compute(...)` → read the
bitmask. **Behavioral test**: `brievc export/bindings` on `boundary.bv` and
`node_bridge.bv` produce **byte-identical** shims to today; a transition test
asserts the Briev result equals the (now reference) Rust result on a corpus of
bridges. `cargo test --lib` green.

### P5 — generalize
Extract the reusable AST-projection serializer + Briev reader into
`lib/compiler/`; migrate a second pass (`soa_reorder`, permutation-only
handoff) to prove the pattern generalizes. Record the recipe in
`docs/architecture/compiler-in-briev.md`.

## Decisions

1. Handoff: **tagged Data Briev projection** (readable, established via
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
- `brievc`'s build gains a brievc invocation in `build.rs` (the rust-host crate
  already proves this).
- The pass's output contract must be stable — tested behaviorally.

## Verification

`cargo test --lib` green; behavioral byte-identical shims; the transition test
(Briev result == Rust reference) on a corpus; Praetor on changed dirs.

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
    for iteration, `char_at`/`briev_str_substr` frgns for scanning (the LLVM
    backend's String slice returns the whole array — see BUGS.md). The dynamic
    String slice was implemented as runtime `briev_str_substr` + a `char_at`
    frgn (Int return, no allocation).
 5. **String codegen fixes en route**: `.^Len` on a boxed String param/frgn
    result panicked — `is_semantic_string` + `string_ptr` (matches the `==`
    fix); `let_binding_allocas` leaked across functions (manual clears missed
    it; reg numbers rewind per function) — replaced with `clear_locals()`.
 6. **THE PASS WORKS**: `needs_state_compute` matches the Rust reference on all
    five bridges — boundary=0, node_bridge=31, cancel=1, rank=2, bench=2 —
    deterministically, asserted by `tests/c_driver_needs_state.rs` (the P4
    transition test). A stateful export's C signature takes the state handle
    first (`__briev_init_state()`), which the first C driver got wrong — the
    "heap corruption" was that arity bug (see BUGS.md correction).

**P2 complete.** **P3 complete (Option 1 — runtime dlopen):** the root
`build.rs` compiles `needs_state.bv` with a prebuilt brievc into
`target/compiler-in-briev/needs_state.so` and embeds its path via
`cargo:rustc-env=BRIEV_COMPILER_IN_BRIEV_SO`; `src/glue/briev_pass.rs` dlopens
it at first use (the GLUE host model) and resolves `needs_state_compute` +
`__briev_init_state` through the C ABI. First build has no brievc yet (self
-hosted bootstrap) — the runtime falls back to the Rust reference; every later
build rebuilds the .so. **P4 complete:** both production call sites
(`src/backend/llvm/mod.rs:1734`, `src/glue/export.rs:116`) now route through
`briev_pass::compute_export_needs_state` (Briev pass when loaded, reference
otherwise). `tests/c_driver_needs_state.rs` and the `briev_pass` unit test
assert the Briev result equals the Rust reference on the five-bridge corpus;
all c_driver glue tests (boundary/node/callback/cancel) pass through the
dlopen'd pass. `pass_available()` exposes load state for diagnostics.

**P5 complete — the pattern generalizes.** Second pass `soa_reorder` proves
it: a DIFFERENT handoff shape (field descriptors + sibling-reference counts →
an item-index permutation buffer, vs a statement-tree → bitmask). The shared
scanner is extracted to `lib/compiler/reader.bv` (both passes import it; the
imported-frgn bug that forced local declarations is fixed — see BUGS.md).
`briev_pass.rs` is now a generic `LoadedPass` dlopen loader; build.rs compiles
both passes. The Briev `soa_reorder_compute` produces the EXACT permutation of
the Rust `reorder_fields` on the real `nbody_newton.bv` benchmark (asserted by
`soa_reorder.rs::briev_reorder_matches_reference_on_nbody` +
`briev_pass.rs::soa_pass_matches_reorder_fields`), and the backend wires it at
`mod.rs:1808`. Recipe: `docs/architecture/compiler-in-briev.md`. Also fixed
en route: `expr_needs_state`/the needs_state projection dropped wrapping Expr
kinds (Cast/MethodCall/Reflect/Index/Slice/AddrOf) — a cast-wrapped call to a
regular defn produced a STATELESS export shim that referenced `%state`.

Remaining (future): more passes (a third would justify generalizing
`soa_projection.rs`'s section-line writer into a shared Rust helper); embed the
.so bytes via `include_bytes!` instead of an absolute path (a moved checkout
currently rebuilds it); the `String slice`/`String +`/`List<String>` codegen
gaps in BUGS.md.



