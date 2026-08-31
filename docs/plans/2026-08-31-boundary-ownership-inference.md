# Boundary Ownership Inference — Self-Inferred Zero-Copy FFI

**Date:** 2026-08-31
**Status:** Proposed
**Related:** `docs/plans/2026-08-20-ownership-algebra-phase9.md` (explicit
keywords), `docs/plans/2026-07-10-zero-copy-glue-bridge-phases.md` (zero-copy
bridge phases), `docs/architecture/glue-ffi.md` (FFI model),
`src/analysis/export_abi.rs` (the analysis pattern this extends).

---

## 1. The problem

The GLUE FFI already delivers native calling speed (verified 2026-08-31:
`benchmarks/results/2026-08-31-zero-friction-gate-verify.md`). But data copies
at the boundary are decided by **ad-hoc per-binding functions** (`cstr_to_briev`
copies, `str_to_c` is zero-copy) — the ownership story is implicit in which
binding function the casting graph picks, never surfaced as a first-class
analysis result the compiler can reason about or the developer can audit.

The user's goal: **zero-copy FFI to and from Briev** — "as if Briev were the
native language, just faster." The barrier to fully-zero-copy is ownership:
Briev's arena owns its data; hosts borrow read-only. That ownership is
currently invisible to the type system.

## 2. The insight: the compiler already has the signals

Three signals already exist and are sufficient to infer boundary ownership for
the common cases, deterministically (no heuristics):

| Signal | Source |
|--------|--------|
| Calling convention | `GlueTarget.calling_convention` (`c_abi` / `lto` / `wasm_import`) |
| Protocol variant | casting graph (`#String<C_String>` vs `#String<UTF8>`) |
| Direction | AST node type (`export` = Briev→host; `frgn` = host→Briev) |

Inference table (pointer-representation types only; scalars are always `Value`):

| Convention | Variant | Direction | Inferred |
|------------|---------|-----------|----------|
| `lto` | any | any | `ZeroCost` (IR merger, no boundary) |
| `c_abi` | `#String<C_String>` | export return | `ZeroCopy` (Briev sends data ptr) |
| `c_abi` | `#String<C_String>` | frgn param | `Borrowed` (host-owns, Briev copies) |
| `c_abi` | `#String<UTF8>` | export return | `Owned` (Briev sends `[len][data]` handle) |
| `c_abi` | `#String<UTF8>` | frgn param | `Owned` (Briev owns the handle) |
| `c_abi` | `#Int`/`#Float`/`#Bool`/`#Char` | any | `Value` (pass-by-value) |
| `wasm_import` | `#String<*>` | frgn param | `Borrowed` (wasm linear memory) |

Plus transitive propagation: an `export` whose body returns the result of a
`frgn` call inherits the frgn's return ownership; an `export` whose body stores
a param into state or copies it becomes `Owned`.

## 3. Scope of this phase

A new frontend analysis pass, `src/analysis/boundary_ownership.rs`, that:

1. Classifies every `export` parameter and return.
2. Classifies every `frgn` parameter and return (via protocol variant +
   convention).
3. Propagates ownership transitively through the call graph (memoized DFS,
   mirroring `compute_export_needs_state` in `export_abi.rs`).
4. Stores results in `AnalysisResults.boundary_ownership`.

It does **not** change codegen yet. The pass makes ownership *queryable* and
*auditable*. The follow-up phase wires it into GLUE wrapper generation to
eliminate provably-unnecessary copies (see §7).

## 4. Design

### 4.1 The ownership lattice

```rust
pub enum BoundaryOwnership {
    /// Passed by value (Int/Float/Bool/Char) — no ownership concern.
    Value,
    /// Briev owns the backing memory; the host borrows read-only. Arena-lifetime.
    Owned,
    /// A C-string pointer (NUL-invariant) — zero-copy out of Briev's arena.
    ZeroCopy,
    /// Host owns the memory; Briev must copy into its own arena to use it.
    Borrowed,
    /// LLVM LTO — IR merged, no boundary exists. Nothing to reason about.
    ZeroCost,
}
```

Partial order for the meet: `Value ⊏ Owned ≈ ZeroCopy ⊏ Borrowed ⊏ ZeroCost`.
When a value flows to multiple consumers, take the **most conservative**
(strongest obligation): if any consumer needs a copy, the boundary copies.

### 4.2 The per-boundary result

```rust
pub struct BoundaryEntry {
    /// ownership of each exported/frgn parameter, by position
    pub params: Vec<BoundaryOwnership>,
    /// ownership of the return value
    pub ret: Option<BoundaryOwnership>,
}

pub struct BoundaryOwnershipResult {
    /// exports keyed by defn name
    pub exports: HashMap<String, BoundaryEntry>,
    /// frgns keyed by briev name
    pub frgns: HashMap<String, BoundaryEntry>,
}
```

### 4.3 Seeding from protocol variant + convention

```rust
fn seed_from_protocol(
    ty: &Type,
    convention: &str,   // "c_abi" | "lto" | "wasm_import"
    direction: Direction, // Param (host→Briev) or Return (Briev→host)
    graph: &CastingGraph,
    universe: &TypeUniverse,
) -> BoundaryOwnership
```

- `convention == "lto"` → `ZeroCost` (all pointer-representation types).
- Scalar categories (`#Int`, `#Float`, `#Bool`, `#Char`) → `Value`.
- `#String<C_String>` + `Return` → `ZeroCopy`; `Param` → `Borrowed`.
- `#String<UTF8>` (or other owned) → `Owned`.
- Unresolvable / custom pointer types → `Borrowed` (conservative; the Phase 9
  keywords override this).

The `Direction` encodes the asymmetry the casting graph's binding functions
already imply (`str_to_c` zero-copy out, `cstr_to_briev` copy-in) — this pass
makes it explicit and checkable.

### 4.4 Transitive propagation

Mirror `export_abi.rs` exactly: index regular defns / exports / frgns, then a
memoized DFS (`visiting` set breaks cycles conservatively → `Borrowed`).

For an `export` body, classify:
- `term <expr>` — the return's ownership is the meet of the expr's flow
  (param passthrough → param's ownership; frgn call result → frgn return's
  ownership; a new allocation / literal → `Owned`).
- `let x = <expr>` then `term x` — propagate the expr's ownership to the let,
  then to the term.
- storing a param into a state field, or passing a param to a copying intrinsic
  (`cstr_to_briev`, `StringConcat#`, …) → that param becomes `Owned` at the
  boundary (Briev took ownership by copying).

Expression flow helper `expr_ownership(expr, env) -> BoundaryOwnership` mirrors
`expr_needs_state`, walking the same expression kinds (Cast, MethodCall,
Identifier, Call, BinaryOp, Index, …). The `visiting` set + memo are shared.

### 4.5 AnalysisResults

Add to `src/backend/mod.rs`:

```rust
/// 2026-08-31 (boundary ownership plan): per-export and per-frgn boundary
/// ownership — Borrowed / Owned / ZeroCopy / Value / ZeroCost. Computed once
/// in the frontend from protocol variant + calling convention + direction,
/// propagated transitively through the call graph. Consumed by GLUE wrapper
/// generation to eliminate provably-unnecessary copies (and auditable by the
/// developer via the ownership report). See
/// docs/plans/2026-08-31-boundary-ownership-inference.md.
pub boundary_ownership: crate::analysis::boundary_ownership::BoundaryOwnershipResult,
```

Populate in `analyze_program` (default empty when not requested, so non-GLUE
backends are unaffected).

## 5. Consumption (follow-up, not this phase)

Once the analysis is trustworthy, wire it into:
- `src/glue/export.rs` — tag `ExportDecl` with per-param/return ownership so
  `briev export|bindings|extension` render ownership-aware wrappers.
- `src/glue/bridge.rs` / `src/backend/llvm/emit_expr.rs` — when the boundary
  proves `ZeroCopy`, skip the `cstr_to_briev` copy (emit the `str_to_c`
  zero-copy path).
- The ownership report (`brievc` flag) so a developer can see, per boundary:
  is this copy-free or not, and why.

This is deliberately out of scope for the analysis phase — the pass must be
proven correct before any codegen relies on it (AGENTS Rule 9: tests first).

### 5.1 Wiring status — IMPLEMENTED (2026-08-31)

- **`ExportDecl` ownership tagging**: `src/glue/export.rs` now carries
  `param_ownership: Vec<String>` and `return_ownership: Option<String>` on each
  export, populated from `compute_boundary_ownership`. The `bridge-exports.dbvl`
  metadata serializes them as fields 6 (params) and 7 (return):
  `export,echo,CStr,CStr,pure,borrowed,zero-copy`.
- **Ownership report**: `brievc ownership <file.bv>` prints every export and
  frgn boundary's per-param/return ownership class.
- **Copy-elimination verification**: the codegen already honors the asymmetry —
  `String → CStr` (classified `ZeroCopy`) emits `str_to_c` (the pointer-offset
  zero-copy delta), never `cstr_to_briev` (the allocating copy). A regression
  test (`string_to_cstr_cast_becomes_zero_copy_str_to_c`) locks this. No
  redundant copy existed to remove; the wiring makes the zero-copy contract
  explicit and auditable.
- **Declared-protocol resolution**: the analysis resolves boundary types
  (`CStr`, `CDouble`) from `type X: #Proto<Var>` declarations as a fallback,
  because the GLUE commands (`bindings`/`extension`/`export`/`ownership`) use
  `parse_and_check` which does not run the normalizer that registers `Cast.*`
  universe properties.

## 6. Composition with Phase 9 keywords

Phase 9 (`2026-08-20-ownership-algebra-phase9.md`) provides explicit
`borrow`/`consume`/`owned`/`shared`/`borrowed<source>` annotations. They are
the **override** for the cases this pass cannot infer:
- Custom pointer types with no protocol variant.
- `frgn` returns that hide pointers as integers (opaque C — undeclarable).
- Multi-language GLUE bridges (Python/Node/Java) whose ownership is mediated by
  the host's GC.

Resolution order: **explicit annotation wins; otherwise inferred; otherwise
conservative (`Borrowed`).** Tier 1 (derived/silent) of Phase 9 aligns with
this pass's `Inferred` output.

## 7. What cannot be inferred (and must stay explicit)

1. `frgn` returning a pointer disguised as an integer — the compiler cannot
   analyze opaque C.
2. Custom pointer types outside the protocol system.
3. Host-GC-owned lifetimes across managed bridges — ownership is the host's
   memory-management policy, not derivable from Briev types.

These are exactly the Phase 9 keyword cases.

## 8. Tests

New unit tests in `src/analysis/boundary_ownership.rs` (mirroring
`export_abi.rs` tests):
- scalar export → all params/ret `Value`.
- `c_abi` export `-> CStr` from a `String` literal → ret `ZeroCopy`.
- `c_abi` export taking `CStr` param and returning it → param `Borrowed`,
  ret `Borrowed` (passthrough).
- export returning a `frgn`'s `CStr` result → ret `ZeroCopy` (transitive).
- export copying a param into state → param `Owned`.
- `lto` convention → everything `ZeroCost`.
- mutual recursion → conservative `Borrowed` (no hang).
- unknown custom type → conservative `Borrowed`.

Integration: `cargo test --lib`; gate still green.

## 9. Docs to update (same commit)

- `docs/architecture/glue-ffi.md` — add an "Ownership inference" note linking
  this plan; the §6 speed table already documents parity.
- `src/analysis/export_abi.rs` — none (untouched).
- This plan stays as the historical record (Rule: timestamped docs never
  retroactively edited).

## 10. Rationale comments

Each seeded inference table entry carries a `// 2026-08-31 (<why>, <pattern
targeted>, <undo>)` comment, per AGENTS Rule 16. Example:
`// 2026-08-31 (C-String return is the data ptr + NUL invariant; undo: if CStr
loses the NUL invariant, revert to Owned) ZeroCopy`.
