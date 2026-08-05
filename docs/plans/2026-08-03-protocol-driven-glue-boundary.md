# Protocol-Driven GLUE Boundary — Marshalling Is Cast Deltas

**Date:** 2026-08-03
**Status:** Active plan
**Branch:** `glue-host-callable`
**Related:** `docs/plans/2026-08-03-host-callable-glue-export.md` (the original GLUE host plan)

---

## Motivation

The first GLUE implementation (phases 1–5 of `2026-08-03-host-callable-glue-export.md`)
got Briv callable from C, Python, and Rust, with callbacks and host cancellation.
But its **boundary marshalling** was a workaround: per-language `conversions`
templates (`to_abi`/`from_abi` strings) and a `resolve_protocol` lookup that
mapped a type's *category* to a C-ABI name. That duplicates — and falls short
of — the mechanism Briv was designed around.

**Briv has no type layouts.** A type is `(protocol, metadata)`, nothing else.
Conversion is `CastTo`/`CastFrom` edges in a protocol graph, and those protocols
are *adaptive*: the compiler finds the minimal path between two representations
and emits the **delta** of operations, not a hop-by-hop chain. FFI marshalling
is exactly this problem — a boundary representation is a sub-protocol
(`#String<C_String>`, `#Float<C_Double>`), a `proto` declaration supplies the
transforms, and the casting graph resolves the path. The compiler should never
carry per-language conversion strings; it should carry protocol edges.

This plan replaces the workaround with the protocol-driven design: bridge-declared
boundary types, cast-path marshalling, and delta emission.

## Design Constraints (from the author)

1. **No type layouts, only adaptive protocols.** The base protocol
   (`#String`, `#Float`, `#Int`, …) is *never directly modified*; sub-protocols
   add and adapt. `proto C_String: #String { … }` extends `#String` without
   touching it. Additive only.
2. **Boundary types are bridge-declared.** The bridge imports the C-ABI module
   and declares its boundary contract: `type CStr: #String<C_String>` used
   directly in export signatures. The export signature IS the boundary contract.
   (Also expressible as a direct variant annotation, but the declared-type form
   is the primary one.)
3. **Casting emits the delta, not the chain.** The casting graph resolves the
   *chain* (BFS finds the minimal number of hops); codegen emits the *delta* —
   the minimal incremental operations. Briv adopts whatever operations are most
   convenient: if the base protocol's representation is stable and casting
   to/from a sub-protocol is just as fast, a value is treated as whatever
   representation suits the current operation, and the cost of changing
   representation is the delta between them.

## The Delta Principle (three collapse rules)

The delta between two representations collapses under three rules, in order:

1. **Same representation → identity (zero ops).** Every `#String` variant is a
   `ptr` to `[len][bytes]` (the B0 bits model); a `#String` sub-protocol is a
   String — only its *encoding* differs. Casting between two variants with the
   same machine representation is free when the encoding matches.
2. **Single binding → one call.** `CStr → String` emits exactly one call
   (`cstr_to_briv`); `String → CStr` emits `str_to_c`. The delta transform,
   not a chain through an intermediate.
3. **Inverse pair → nothing (1-to-1).** If `A.CastTo(#String)` is `<< 1` and
   `B.CastFrom(#String)` is `>> 1`, the composition
   `B.CastFrom(A.CastTo(x)) == x` — the two ops cancel, the delta is
   *effectively nothing*, and A and B are 1-to-1. The emitter drops both.
   Established by a cross-type round-trip proof (symbolic/SMT where provable,
   e.g. linear ops), never by guessing.

The inverse-pair rule is what makes sub-protocols *adaptive* in the performance
sense: adopting a convenient sub-protocol representation costs nothing when the
encode/decode pair is proven inverse.

## Verified Current Infrastructure

All of the following exists and is wired (file:line at HEAD):

- `proto` declarations parse → `TopLevel::ProtocolDef` → `register_protocol_def`
  (`src/compile.rs:1061,1150,1189,1231`) → variant edges in the casting graph.
- `ProtocolDef` carries `cast_edges: Vec<CastEdge>` (each with a
  `CastBinding { fn_name, param }`) and `cross_ops: Vec<OperatorDef>` (variant
  op overrides) — `src/ast/top.rs`.
- `type_to_protocol(universe, ty)` maps any Type → `(category, variant)`
  (`src/casting/graph.rs:505`), including `Type::HashWordVariant` and the
  `Cast.#`/base-chain walk.
- `resolve_llvm_type(universe, ty, int_bits)` derives the LLVM type from
  `(protocol, metadata)` (`graph.rs:594`). `#Float<Double>` → `"double"` is
  already seeded (`graph.rs:291`); the default `#Float`/`IEEE754` → `"float"`.
- `find_path(src_cat, src_var, dst_cat, dst_var)` = BFS over variant edges +
  base lanes, returning the minimal chain (`graph.rs:388`).
- `emit_cast_path`/`emit_cast_steps` (`src/backend/llvm/emit_expr.rs:2955/2969`)
  emit each `CastStep`, including `LaneKind::ExtCall` (`emit_expr.rs:3026`).
- Round-trip verification machinery: `verify_protocol_roundtrip` +
  `symbolic_deep_equals` + SMT builders (`src/analysis/protocol_graph.rs:31`).
- `Cast#` in the backend (`src/backend/llvm/intrinsics.rs:86`) → `emit_cast_path`.

## The Workaround Being Removed

- `config/glue.dbvl` `conversions` (`to_abi`/`from_abi` string templates with
  `{name}`/`result_abi` placeholders) — **removed**; conversion is protocol edges.
- `resolve_protocol` in `src/glue/export.rs` (category → `c_abi` name lookup) —
  replaced by `type_to_protocol` + the variant-keyed config lexicon for the
  *wrapper's* type names (what the generated Rust/Python/C header calls a type —
  the language's vocabulary, still config; conversion is never config).
- The BUGS.md `#Float` boundary item: `Float` lowers to `"float"` (32-bit) by
  default; the boundary requests `#Float<C_Double>` (`CDouble`) → `"double"`.

## Phase 1 — Close the protocol-machinery gaps

These are the places where the protocol system does not yet do what the design
says it should. Each verified against source.

1. **Variant bases in `type_to_protocol`** (`graph.rs:567`): the base-chain walk
   only matches bare categories (`"String"`, `"Float"`, …). `type CStr:
   #String<C_String>` sets `rt.base = "#String<C_String>"`, which falls through to
   `(Bit, "")`. Parse `#Cat<Variant>` bases → `(String, CString)`.
2. **Variant LLVM-type fallback** (`graph.rs:604`): an unseeded
   `#String<C_String>` resolves to `None` → struct-fields fallback → `"i64"`.
   Correct rule: a `#String` variant IS a `ptr`; a `#Float` variant defaults to
   the base (`float`); `#Int` is WidthParametric (driven by `!> bits` metadata).
   Fall back to `default_variant(category)`, then the base `""` resolver.
3. **Bindings → real calls** (`graph.rs:341`): `register_protocol_def` hardcodes
   `LaneKind::Bitcast`, ignoring `CastEdge.binding.fn_name`. Add
   `LaneKind::ExtCallDyn(String)` (additive — seeded base lanes keep
   `ExtCall(&'static str)`) and emit `call <dst> @<fn>(...)` for variant edges.
4. **Wire `cross_ops`** (parsed at `src/parser/definitions.rs:2132`, **never
   consumed**): register per-variant op overrides and resolve them in the
   typechecker — an op on a sub-protocol value prefers the variant's own op
   (zero cast); falls back to the base op via a delta cast. This is "adopt
   whatever operations are most convenient."
5. **Inverse-delta collapse** (the new piece):
   - Extend `verify_protocol_roundtrip` to **cross-type** compositions:
     `B.CastFrom(base)(A.CastTo(base)(x)) == x`, symbolic + SMT (linear ops
     like `<<1`/`>>1` prove cleanly).
   - Record proven-inverse pairs as zero-cost edges in the graph.
   - `emit_cast_steps`: collapse a same-category `variant → base → variant`
     pair to identity when the pair is a proven inverse. Non-provable pairs
     emit both calls (correct, not free).

Tests for each: variant base mapping, LLVM fallback, ExtCall lane, variant-op
preference, inverse-pair collapse (the `<<1`/`>>1` example). `cargo test --lib`
green; confirm nothing relied on the `i64` fallback.

## `+` is string concat (2026-08-03, author request)

`+` now concatenates `#String`/`#Data` values (the `++`/Concat operation) —
`++` was a wart. Wired end-to-end: `('String','Add') → StringConcat#` in the
binding table (single source), the typechecker maps `+` → Concat for string
categories and prefers the variant's cross-op, a post-typecheck pass
(`src/analysis/string_concat.rs`) rewrites `BinaryOp(Add)` → `Concat` on the
typed AST (the backend can't see String types after i64 boxing), the backend
routes string `+` to the concat emitter, and the interpreter concatenates on
`+` and `++` (rule 4). Tests for each.

## Phase 1 status

P1 is DONE: variant bases, variant LLVM fallback, bindings → ExtCallDyn,
inverse-delta collapse, and the cross-variant op overrides (P1.4) — the graph
registers `variant_cross_ops`, and the boundary_marshalling pass rewrites a
`CStr + CStr` (or `++`) into the variant's own `cstring_concat` binding call
(the generic inline concat would treat a nul-terminated C string as
`[len][data]`, which is wrong). `briv_cstring_concat` was added to the
runtime. The `+`-for-concat fix rides the same machinery.

## Phase 2 status — `lib/glue/c.bv` DONE (ABI), marshalling = follow-up

The C-ABI boundary module exists (`lib/glue/c.bv`): `proto C_String` with
`cstr_to_briv`/`str_to_c` bindings, and the boundary types `CStr`/`CFloat`/
`CDouble`/`CI64`/`CI32`/`CBool`/`CChar`/`CPtr`. The ABI derivation works:
- `CStr` → `ptr` (a #String sub-protocol IS a ptr), `CDouble` → `double`
  (the Float ABI fix — declaring the boundary type clears the BUGS.md item).
- The normalizer now registers the declared protocol hashword as the base
  (was `td.parent` only → `Bit`), so `type X: #String<C_String>` resolves
  its category; and the import resolver's project-root walk-up was
  generalized from `std/`-only to any `lib/` module.
- Demo `examples/glue-host/boundary.bv` (echo: CStr→ptr, identity: CDouble→double).

**Marshalling: DONE.** `name as String` for a CStr param emits `cstr_to_briv`
(and `s as CStr` emits `str_to_c`) — the graph-resolved binding calls. Briv's
boxing turns CStr values into i64 registers, so the decision is made on the
typed AST BEFORE codegen: `src/analysis/boundary_marshalling.rs` builds the
casting graph from the program's protos + a type→protocol map, and rewrites a
same-category representation cast into `find_path`'s binding call. (The import
resolver also had to stop dropping `ProtocolDef`s — they fell to the `_ =>
None` filter arm — so library boundary modules register their variant edges.)

**P3 (export ABI naming): DONE.** `resolve_protocol` takes a type→protocol map
so boundary types resolve to their category's ABI names in the generated
header/wrapper: `CStr` → `int64_t`, `CDouble` → `double`.

Verified end-to-end via a C driver: `echo(CStr)` pass-through (ptr ABI),
`greet` marshals a C string through `briv_cstr_to_briv`/`briv_str_to_c`,
and `identity(CDouble)` returns `3.14` as `double` — the Float ABI bug
(BUGS.md) is fixed by declaring the boundary type.

## Phase 2 — `lib/glue/c.bv` (the C-ABI boundary module)

```briv
// The C string representation: nul-terminated bytes.
proto C_String: #String {
    CastTo(#String<UTF8>) = cstr_to_briv(#L);   // nul-terminated → [len][data]
    CastFrom(#String<UTF8>) = str_to_c(#L);       // [len][data] → nul-terminated
};

type CStr:    #String<C_String> { };
type CFloat:  #Float<C_Float>   { };
type CDouble: #Float<C_Double>  { };
type CI64:    #Int              { };
type CI32:    #Int<C_I32>       { !> bits: 32; };
type CBool:   #Bool<C_I8>       { };
type CChar:   #Char<C_I32>      { };
type CPtr:    #Data<C_Ptr>      { };
```

The `cstr_to_briv`/`str_to_c` frgns to `lib/runtime/briv_rt.c` are declared
here (they already exist in the runtime). Verify:
- `find_path(CStr → String)` = one `cstr_to_briv` call.
- `llvm_type(CDouble)` = `double`; `llvm_type(CI32)` = `i32`.
- A declared CString-native op (e.g. `Concat`) is used without any cast when a
  `CStr` value is concatenated.

## Phase 3 — Export boundary via the casting graph

- `resolve_protocol` (`src/glue/export.rs`) → derive `(category, variant)` via
  `type_to_protocol(universe, ty)`; the wrapper/header vocabulary comes from
  `config/glue.dbvl` protocols **keyed by variant** (`"#String<C_String>" →
  { native, c_abi }`), with bare-category fallback.
- `format_type` handles `Type::HashWordVariant` and boundary custom types.
- **Remove** the config `conversions` templates. The `.ll` ABI type already
  flows from `llvm_type(param)` — correct once Phase 1 lands.
- Body marshalling is ordinary Briv casts (`(String)name`) → the
  `Cast#`/`emit_cast_path` machinery emits the delta.

## Phase 4 — Migrate demos + verify

- `pp-types.bv`, `examples/glue-host/rank.bv`, `callback.bv` migrate to boundary
  signatures (`CStr`, `CDouble`); the manual `cstr_to_briv`/`str_to_c` calls
  inside bodies become plain casts.
- **Float exports now work** (clears the BUGS.md item): `export defn
  scale(x: CDouble) -> CDouble` → `double` end-to-end.
- Round-trip tests (`c_driver_library`, `c_driver_callback`, `c_driver_cancel`,
  `pp_roundtrip_tests`) + C/Python/Rust hosts + benchmark unchanged.
- Docs updated in the same commits: `docs/architecture/casting-protocol.md`
  (boundary variants + the delta/inverse rule), `frgn-export-glue-architecture.md`,
  `features/callbacks.md`, this plan.

## Completion Status (2026-08-03)

- **P1 (protocol machinery):** DONE — variant bases in `type_to_protocol`,
  variant LLVM-type fallback, bindings → `ExtCallDyn` lanes (real calls),
  inverse-delta collapse (proven 1-to-1 pairs are zero-cost), cross-variant
  op overrides.
- **P2 (boundary module):** DONE — `lib/glue/c.bv` (`proto C_String` +
  boundary types); `CStr` → `ptr`, `CDouble` → `double` (the Float ABI fix).
- **P3 (export boundary):** DONE — `resolve_protocol` uses a type→protocol
  map so the generated header resolves boundary types to C ABI names
  (`CStr` → `int64_t`, `CDouble` → `double`); `boundary_marshalling` rewrites
  `CStr ⇄ String` casts into the graph's binding calls (`cstr_to_briv`/
  `str_to_c`).
- **P4 (migrate + verify):** DONE — boundary round-trip test
  (`tests/c_driver_boundary.rs`), Float export in `rank.bv`
  (`scale(x: CDouble) → fadd double`), all C-driver/roundtrip tests + the
  benchmark green (0.92× vs C per-call).
- **`+` is string concat** (author request): end-to-end.
- Verified end-to-end via C driver: echo/greet/join/identity all correct.

### Known follow-ups
- Float LITERAL codegen is still corrupted (`2.0 as CDouble` emits a
  bitcast+sitofp mess — the BUGS.md Float item beyond the boundary ABI). The
  boundary type gives the correct ABI; literal→Float→CDouble casts need the
  deeper Float literal fix.
- `sync<group>` still has no codegen (out of scope).
- The cancel flag is process-global (per-state would allow concurrent
  instances).

## Cross-Cutting

- Every feature wired parser → AST → analysis → codegen → tests; `cargo test
  --lib` before each commit; Praetor on changed dirs; additive-only match arms.
- Rules honored: no type-name matching (everything via `type_to_protocol`/
  `universe_key`), no hardcoded language knowledge in `src/` (wrapper lexicon
  stays in config), interpreter remains the reference for casts.
- The inverse-proof gate is symbolic/SMT where provable (linear ops) with a
  correct-but-not-free fallback — never a guess.
