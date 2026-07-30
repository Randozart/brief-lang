# Casting Graph Rewrite — Plan Document

**Date:** 2026-07-30  
**Author:** Plan-driven agent  
**Target:** Replace `operator_defs`-based Cast/CastTo/CastFrom dispatch with a static casting graph where every base protocol has a hardcoded direct lane to every other base protocol.

---

## 1. Motivation

The current casting pipeline (Phases 0–8, implemented 2026-07-30) works but has four structural problems that will compound as new protocols are added:

1. **Scattered knowledge.** Cast dispatch is split across four locations:
   - `resolve_cast()` / `resolve_physical_cast()` in `emit_expr.rs:2489-2604`
   - `emit_intrinsic_cast()` in `intrinsics.rs:1012-1067`
   - `find_cast_impl()` / `try_cast_protocol_path()` in `intrinsics.rs:1112-1146`
   - `find_matching_cast_from()` / `find_matching_cast_to()` in `emit_expr.rs:2678-2744`
   
   Each makes independent `operator_defs` HashMap lookups, manually walks parameter lists, and duplicates protocol-membership checks. Adding a new protocol means touching all four.

2. **`operator_defs` is the wrong mechanism.** The HashMap was designed for `InsertAt`/`ExtractFrom` (ring buffer ops). Casting uses it as a makeshift protocol registry, but the actual dispatch logic duplicates what `ProtocolGraph::find_protocol_path()` already does via BFS.

3. **Normalizer injects `Cast.#` properties as side-effect.** Three separate injection passes in `register_typedefs()` (lines 336–390 of `normalizer.rs`) reach into universe properties to simulate protocol edges. This is fragile — the normalizer should normalize types, not be a protocol registry.

4. **`op Cast()` is a language design dead end.** Every protocol → protocol cast is compiler-intrinsic knowledge. There is no user-extensible cast path because the compiler already knows how to reach any protocol from any other protocol (via `#Bit`). Letting users declare `op Cast()` creates a false extensibility surface that either duplicates compiler knowledge or silently disagrees with it.

### Core Principle: Protocols are Guarantees, Types are Overlays

Every base protocol (`#Int`, `#String`, `#Float`, etc.) has a hardcoded direct lane to every other base protocol. These lanes are **compiler guarantees** — they always exist, they always work the same way, and they cannot be broken, removed, or overloaded.

Types (`type AutoString: #String`) can **extend, override, and customize** *on top of* the protocol guarantees. A type-level override of `CastTo(#Int)` on AutoString changes what happens when *that specific type* reaches `#Int`, but the `#String → #Int` lane itself is unchanged — available for any other `#String` type that doesn't override it.

`#Bit` is the root of this hierarchy with a special rule:
- **`→ #Bit`** is always hardcoded (bitcast/extractvalue/ptrtoint). No overrides, no exceptions. This is a **representation guarantee** — the caller gets the exact bits, full stop. `CastTo(#Bit)` is banned at declaration time.
- **`← #Bit`** is the **interpretation door** — `op CastFrom(#Bit)` is the sole user-extensible edge in the entire graph. A type declares how to construct itself from raw memory bits.

Three-way priority in `emit_cast()`:
1. **Type-level override** — if the specific src→dst pair has one (e.g., `AutoString::CastTo(#Int)`)
2. **Protocol default** — the hardcoded lane between the two base protocols
3. **`CastFrom(#Bit)` constructor** — if the target type declares it and the path passes through `#Bit`

Step 3 never applies when the target IS `#Bit` — that's the hard wall. `#Bit` is where all protocols meet as equals, and nobody customizes the handshake *into* it. Only the construction *out of* it.

### What the casting graph gives us

- **One BFS call** replaces four independent dispatch chains.
- **Hardcoded base-protocol edges** are static data (no HashMap, no injection).
- **Proto declarations** (`proto ASCII: #String { CastTo(#String) ... }`) feed into the same graph as variant-to-base edges.
- **Consistency checking** becomes trivial: compare the graph path against the variant's declared CastTo/CastFrom and ERROR on mismatch.
- **`operator_defs` shrinks** to only non-cast operators (`InsertAt`, `ExtractFrom`).

---

## 2. Target Architecture

### 2.1 Four-layer protocol hierarchy

```
Layer 1: #Bit (root protocol); Bit (sole primitive type, hardcoded anchor — not a primordial)
  └── `#Bit` is the protocol (the hashword, the casting graph node).
      `Bit` is the concrete primitive type (compiler axiom, never overridable).
      Cast TO #Bit = LLVM bitcast of raw memory (hardcoded, never overridable).
      Cast FROM #Bit = interpret N raw bits as target protocol semantics.
      Using `#Bit` as a protocol for your own types is legitimate:
      `type ReorganisedBit: #Bit { ... }` is fine. Only `type Bit` itself cannot be declared.

Layer 2: Base protocols (hardcoded in compiler)
  #Int, #UInt, #Float, #String, #Bool, #Char, #Data
  └── Each has a hardcoded direct lane to every other base protocol.
      Each knows its LLVM type representation.
      Each knows its operations (Add, Sub, Mul, etc.).

Layer 3: Sub-protocols / variants (stdlib proto declarations)
  proto ASCII: #String { CastTo(#String): ascii_to_utf8(#L); };
  proto UTF16: #String { CastFrom(#String): utf16_from_utf8(#L); };
  └── Declared in .bv files as `proto Name: #Category { ... }`.
      Normalizer reads these, feeds edges into the casting graph.
      CastTo/CastFrom must match the base-protocol path or ERROR.

Layer 4: User types (stdlib type declarations)
  type String: #String;
  type Int32: #Int { !> bits: 32; };
  └── All behavior inherited from protocol. No body needed.
      LLVM type derived from protocol + metadata.
```

### 2.2 Casting graph structure

```rust
pub struct CastingGraph {
    /// Static base-protocol-to-base-protocol edges.
    /// Indexed by (src_category, dst_category).
    /// Every base protocol has an entry for every other base protocol.
    base_lanes: HashMap<(&'static str, &'static str), LaneEmitter>,

    /// Per-variant CastTo/CastFrom edges (from proto declarations).
    /// Indexed by (category, variant).
    variant_edges: HashMap<(String, String), Vec<CastEdge>>,
    variant_reverse: HashMap<(String, String), Vec<CastEdge>>,

    /// Default variant per category (e.g., String→UTF8, Float→IEEE754).
    defaults: HashMap<String, String>,

    /// Cross-variant op overrides.
    cross_ops: HashMap<(String, String, String), String>,
}

impl CastingGraph {
    /// Find a protocol path from (src_category, src_variant) to (dst_category, dst_variant).
    /// BFS through base_lanes + variant_edges.
    pub fn find_path(...) -> Option<Vec<EmitterStep>> { ... }

    /// Emit the LLVM IR for a given path.
    /// Each LaneEmitter produces the IR for its step.
    pub fn emit_path(&self, out: &mut String, ...) -> TypedRegister { ... }
}
```

### 2.3 Edge types in the graph

| Edge type | Source | Target | Emitter | Overridable? |
|-----------|--------|--------|---------|-------------|
| Base→Base (hardcoded) | `#Int` | `#Float` | `sitofp i64 to double` | No |
| Base→Base (hardcoded) | `#String` | `#Int` | Call `@__str_to_int` | No |
| Base→Base (hardcoded) | `#String` | `#Bit` | `extractvalue 0` (`.data`) | **Banned** — never overridable |
| Base→Base (hardcoded) | `#Bit` | `#Int` | `bitcast iN to i64` | **Banned** — never overridable |
| Base→Base (hardcoded) | `#Bit` | `#String` | `CastFrom(#Bit)` callback | **Yes** — only `CastFrom(#Bit)` is overridable |
| Variant→Base (proto) | `String<ASCII>` | `#String` | Call `@ascii_to_utf8` | Yes (per proto) |
| Base→Variant (proto) | `#String` | `#String<ASCII>` | Call `@utf8_to_ascii` | Yes (per proto) |
| Cross-variant (proto) | `String<ASCII>` | `#Int` | `ascii_to_utf8` + `__str_to_int` (two-step path resolved by BFS) | Yes (per proto) |

### 2.4 What stays and what goes

### 2.5 The `→ #Bit` ban and the `← #Bit` door

This is the single most important constraint in the entire casting graph:

- **`op CastTo(#Bit)` is banned at declaration time.** The compiler rejects it with: `"CastTo(#Bit) is hardcoded — use x as Bit or Cast#(x, target) for bitcasts."` Casting *to* `#Bit` is a **representation guarantee**: the compiler always does the mechanical job (bitcast, extractvalue, ptrtoint) with zero semantic transformation. No type overrides this.

- **`op CastFrom(#Bit)` is the sole user-extensible cast edge.** It is the **interpretation door** — a type declares how to construct itself from raw memory bits. This is the one place where user code gives meaning to `#Bit`.

- **`op CastTo(#Category)` for non-`#Bit` categories** remains allowed. `type AutoString: #String { op CastTo(#Int): my_parse(#L); };` still works — it registers a type-level override for the `#String → #Int` lane. The casting graph always prefers a type-level override over the protocol default.

- **`op CastFrom(#Category)` for non-`#Bit` categories** remains allowed. Symmetric with the above.

Why this asymmetry? Because `→ #Bit` is a promise of mechanical representation — the caller gets the exact bits with no interpretation. `← #Bit` is a constructor — the type decides how to interpret those bits. One is a lossless projection, the other is a semantic bridge.

**Stays:**
- `is_protocol_member()` — still needed for `#Int`, `#Float` checks in operators
- `ProtocolGraph` — renamed/merged into `CastingGraph` (variant edges + BFS)
- `proto` declarations in parser + AST — still needed
- `operator_defs` — for non-cast operators (InsertAt, ExtractFrom) only
- `Expr::Cast(expr, target)` — the AST node stays
- `Cast#` intrinsic — still needed for `Cast#(expr, target)` calls
- `CastTo(X)` / `CastFrom(X)` for non-`#Bit` categories — type-level lane overrides
- `CastFrom(#Bit)` — the one user-extensible interpretation door

**Goes (replaced by casting graph):**
- `resolve_cast()` in `emit_expr.rs` → replaced by graph dispatch
- `resolve_physical_cast()` → replaced by graph dispatch (#Bit lane)
- `resolve_cast_should_try()` → removed (graph always knows)
- `physical_cast_bits()` → removed
- `find_matching_cast_from()` → removed
- `find_matching_cast_to()` → removed
- `category_from_first_param()` → removed
- `find_cast_impl()` in `intrinsics.rs` → removed
- `try_cast_protocol_path()` → removed
- `category_from_params()` → removed
- `emit_simple_call()` → removed (or kept for non-cast operators)
- `emit_meld_shuffle()` → removed (or kept for non-cast usage)
- Cast.# property injection in `normalizer.rs` (3 passes, ~55 lines) → removed
- CastTo/CastFrom → OperatorDef conversion in `compile.rs` (~30 lines) → replaced: CastFrom(#Bit) goes to casting graph's `type_overrides`, other CastTo/CastFrom stay as OperatorDefs (type-level lane overrides for the graph)
- `Cast#` signature in `intrinsic_signatures.rs` → removed
- `String::CastTo(#Bit)` in `bootstrap.bv` → removed (hardcoded lane, banned from overloading)
- `String::CastFrom(#Int)`, `String::CastFrom(#Float)` in `bootstrap.bv` → removed (these are protocol-default lanes now; if String wants custom parse logic, it declares `op CastFrom(#Int): string_from_int(#L)` — a type-level lane override
- `string_get_content_bytes` → removed (replaced by hardcoded `#String → #Bit` lane)
- `string_from_int`, `string_from_float` → moved to stdlib as type-level CastFrom overrides if String's parse differs from the protocol default
- `llvm_type` property in `ResolvedType.properties` → removed (replaced by casting graph `resolve_llvm_type()`)
- `disamb` metadata → removed (replaced by hardcoded protocol variants: `#Float<BFloat>`)
- `resolve_protocol_llvm_type()` in `normalizer.rs` → removed
- Three-phase `llvm_type` derivation in `normalizer.rs` → removed
- `rt_llvm_type()` in `helpers.rs` → removed (duplicate; `emit_toplevel.rs` version also removed)

---

## 3. Phased Implementation

### Phase 0a: Build the CastingGraph module (NEW FILE) — DONE

**File:** `src/casting/mod.rs` + `src/casting/graph.rs`

- Define the `CastingGraph` struct with `base_lanes`, `variant_edges`, `defaults`
- Populate `base_lanes` with hardcoded entries for all base protocols:

```rust
// Base protocol list (used as keys throughout)
const BASE_PROTOCOLS: &[&str] = &["Bit", "Int", "UInt", "Float", "String", "Bool", "Char", "Data"];

// Lane emitter enum
pub enum LaneKind {
    /// LLVM bitcast: src_ty to dst_ty
    Bitcast,
    /// Integer to float: sitofp
    IntToFloat,
    /// Float to integer: fptosi
    FloatToInt,
    /// External function call: call @fn_name
    ExtCall(&'static str),
    /// Struct field extract: extractvalue .data (index 0)
    ExtractData,
    /// Pointer to integer: ptrtoint
    PtrToInt,
    /// Function composition: call forward_fn, then emit second lane
    Chain(Box<LaneKind>, Box<LaneKind>),
}
```

- Implement `find_path()` BFS
- Implement `emit_path()` IR emission
- Add `CastingGraph::new()` that seeds `base_lanes` with all pairs
- Add `register_protocol_def()` for proto declaration edges
- Wire into `compile.rs` (build from items, pass to backend)

### Phase 0b: Remove `llvm_type` metadata, hardcode protocol variant LLVM types

**Type philosophy:** Types are `protocol + metadata`. There is no cached LLVM type,
no precomputed layout hint. The LLVM type is derived on demand from `(protocol, metadata)`
by the casting graph's `resolve_llvm_type()`.

**`!> bits: N`** is a shortcut for `!> minbits: N; !> maxbits: N;` — it
asserts that the type is **exactly** N bits wide on every target. The compiler
honours this as a hard contract: "I must always have this type with this width
for my own sanity and predictability." A type with `!> bits: 32` is always
emitted as `i32`, regardless of `int_bits`.

**Width resolution priority** (for `WidthParametric` protocols `#Int`, `#UInt`):
1. `!> bits: N` → exact width, emitted as `iN`
2. `!> maxbits: N` → upper bound (narrower is fine if contracts prove it)
3. `!> minbits: N` → lower bound (optimizer may narrow, but never below this)
4. `int_bits` (target default, e.g. 64 for x86_64, 32 for wasm32)

**`minbits`** is a proof bound — if the optimizer can prove from contracts that
values fit in fewer bits, it may narrow down to `minbits`. `maxbits` constrains
the upper end of LLVM type selection. `bits` pins both.

**Files modified:**

`src/casting/graph.rs`:
- Add `protocol_llvm_types: HashMap<(&'static str, &'static str), LlvmTypeResolver>`
- `LlvmTypeResolver` enum: `Fixed(&'static str)` or `WidthParametric`
- `seed_protocol_llvm_types()` populates all base protocol + hardcoded variant entries
- Hardcoded Float variants replace the `disamb` workaround:
  - `("Float", "")` → `Fixed("float")`
  - `("Float", "BFloat")` → `Fixed("bfloat")`
  - `("Float", "Half")` → `Fixed("half")`
  - `("Float", "Double")` → `Fixed("double")`
  - `("Float", "FP128")` → `Fixed("fp128")`
  - `("Float", "X86_FP80")` → `Fixed("x86_fp80")`
- Hardcoded String variants:
  - `("String", "")` → `Fixed("{ i64, i64 }")`
  - `("String", "UTF8")` → `Fixed("{ i64, i64 }")`
  - `("String", "ASCII")` → `Fixed("{ i64, i64 }")`
- `resolve_llvm_type(universe, ty, int_bits) -> String` — the single public entry point
  - Calls `type_to_protocol()` to get `(category, variant)`
  - Looks up `(category, variant)` in `protocol_llvm_types`
  - If `WidthParametric`: checks `!> bits` → `!> maxbits` → `!> minbits` → `int_bits`
  - If `Fixed`: returns the type string directly
  - If not found: falls through to `fallback_llvm_type()` (existing codegen fallback)
- `resolve_llvm_type_for_universe(rt, int_bits) -> String` — convenience overload that
  resolves from a `ResolvedType` entry directly (for normalizer-adjacent use)

`src/type_universe/mod.rs`:
- Remove `llvm_type` column from PRIMORDIALS table (currently 7th field in each entry)
- Remove `disamb` from BFloat entry properties
- Remove `llvm_type` insertion from the primordial loop (line 144)
- Remove `llvm_type` from String seeding (lines 167-171)
- Keep `Cast.#<Category>` properties — these signal protocol membership

`src/backend/llvm/normalizer.rs`:
- **Remove** the 60-line three-phase `llvm_type` derivation block (lines 31-89)
- **Remove** `resolve_protocol_llvm_type()` function (lines 547-586)
- **Remove** `get_exact_bits()` and `get_maxbits()` helpers (lines 588-600)
- **Remove** `"llvm_type"`, `"disamb"` from the propagate-to-nonprimordial list (line 153)
- **Remove** the primordial llvm_type inheritance block (lines 290-298)
- The normalizer now ONLY registers types in the universe — it does NOT resolve
  LLVM types. Resolution is deferred to the casting graph at codegen time.

`src/backend/llvm/emit_toplevel.rs`:
- **Remove** `rt_llvm_type()` function (lines 11-12)
- In `llvm_type()` (line 242), replace the final universe property lookup (lines 320-327)
  with a call to `self.ctx.casting_graph.as_ref()?.resolve_llvm_type(...)`
- Remove `&& !rt.properties.contains_key("llvm_type")` from the struct-field
  derivation gate (line 312) — struct types always derive from fields now
- Update line 581: replace `rt_llvm_type(rt) == ty` with
  `self.llvm_type(&Type::Custom(rt.name.clone())) == ty`

`src/backend/llvm/helpers.rs`:
- **Remove** duplicate `rt_llvm_type()` (lines 28-29) and its single call site (line 587)

`src/backend/llvm/mod.rs`:
- Line 492: replace `rt.properties.get("llvm_type")` with `self.llvm_type(ty)`
- Line 972: remove the `llvm_type` property check (struct type comparison becomes unconditional)

**Prerequisite for Phases 1-8:** This phase MUST be completed before wiring `Expr::Cast`
to the graph, because the old codegen reads `llvm_type` from properties. If we remove
that property, the new codegen path must be in place first.

### Phase 1: Wire Expr::Cast to casting graph

**File:** `src/backend/llvm/emit_expr.rs`

Replace the protocol-based dispatch block (lines 548–559):
```rust
// BEFORE:
if self.resolve_cast_should_try(&src.ty, target)
    || type_name_str(&src.ty).and_then(|n| find_cast_impl(self, &n, "Cast")).is_some()
    || ...
{
    if let Some(result) = self.resolve_cast(out, v, &src, target, indent) {
        return result;
    }
}

// AFTER:
if let Some(result) = self.casting_graph.emit_cast(out, v, &src, target, indent) {
    return result;
}
```

- Remove `resolve_cast()` (lines 2489–2543)
- Remove `resolve_physical_cast()` (lines 2550–2604)
- Remove `resolve_cast_should_try()` (lines 2459–2483)
- Remove `physical_cast_bits()` (lines 2609–2648)
- Remove `resolve_shuffle_data()` (lines 2651–2668)
- Remove `find_matching_cast_from()` (lines 2683–2702)
- Remove `find_matching_cast_to()` (lines 2709–2733)
- Remove `category_from_first_param()` (lines 2736–2744)
- Remove `get_shuffle_int()` (lines 2672–2676)

Reduce block to:
```rust
Expr::Cast(expr, target) => {
    let src = self.emit_expr(out, expr, indent);
    if let Some(result) = self.casting_graph.emit_cast(out, v, &src, target, indent) {
        return result;
    }
    // LLVM coercion fallback (unchanged)
    ...
}
```

### Phase 2: Wire Cast# intrinsic to casting graph

**File:** `src/backend/llvm/intrinsics.rs`

Replace the dispatch in `emit_intrinsic_cast()` (lines 1012–1067):
```rust
fn emit_intrinsic_cast(...) -> BTypedRegister {
    let src = backend.emit_expr(out, &args[0], indent);
    if let Some(result) = backend.casting_graph.emit_cast(out, v, &src,
        /* target type from args[1] */, indent) {
        return result;
    }
    // Fallback: bitcast
    ...
}
```

- Remove `find_cast_impl()` (lines 1112–1121)
- Remove `try_cast_protocol_path()` (lines 1123–1137)
- Remove `category_from_params()` (lines 1139–1146)
- Remove `emit_simple_call()` (lines 1151–1163) — if no remaining non-cast callers
- Remove `emit_meld_shuffle()` (lines 1070–1091) — if no remaining non-cast callers

### Phase 3: Remove Cast.# injection from normalizer

**File:** `src/backend/llvm/normalizer.rs`

Remove lines 336–390 (the three injection passes):
- TypeDef.protocol `Cast.#` injection (lines 345–351)
- Old-style operator CastTo/CastFrom injection (lines 354–367)
- New-style op_binding CastTo/CastFrom injection (lines 369–379)
- Cast.#Bit blanket injection for all Bit-based types (lines 382–390)

The casting graph replaces these. `is_protocol_member()` must be updated to check the graph instead of universe properties (file `helpers.rs:1411–1421`).

### Phase 4: Split CastFrom(#Bit) out of operator_defs pipeline

**File:** `src/compile.rs`

The op_bindings → OperatorDef conversion (lines 897–930) needs a targeted change:
- `CastFrom(#Bit)` binding → register in casting graph's `CastFrom(#Bit)` override table instead of `operator_defs`
- Other `CastTo(X)` / `CastFrom(Y)` (where X/Y ≠ #Bit) → remain in `operator_defs` as type-level lane overrides for the casting graph
- `CastTo(#Bit)` → reject with compiler error (banned)

The `operator_defs` HashMap continues to carry non-cast ops (InsertAt, ExtractFrom) and non-#Bit CastTo/CastFrom overrides.

### Phase 5: Remove `op Cast()` from parser grammar

**File:** `src/parser/definitions.rs`

- Remove any parsing path that produces `op Cast(...)` — search for `op_name == "Cast"` in the op definition parser
- The `Expr::Cast(expr, target)` expression (`x as Type`) stays — that's the user-facing cast syntax
- Only remove `op Cast(Name)` declarations inside type bodies

### Phase 6: Update stdlib bootstrap types

**File:** `lib/std/types/bootstrap.bv`

Simplify `String` to protocol-only:
```brief
type String: #String {
    !> alignment: 8;
    !> encoding: "UTF-8";
    op CastFrom(#Bit): string_from_bits(#L);
};
// No fields, no CastTo ops, no props —
// all inherited from #String protocol via the casting graph.
// CastFrom(#Bit) is the sole overridable edge — String uses it
// to construct from raw memory (tag-bit check for SSO).
```

The fields `data: Int` and `len: Int` are no longer needed in the type body because:
- The normalizer derives LLVM type `{ i64, i64 }` from `#String` protocol (hardcoded in normalizer's llvm_type resolver)
- `.data` field access for physical cast is now a `#String → #Bit` lane (extractvalue 0) — hardcoded, not overridable
- `.#Size` / `.#Bytes` metaproperties are protocol-level knowledge
- Note: `CastTo(#Bit)` is gone (banned). `CastFrom(#Int)` and `CastFrom(#Float)` are also gone — those are protocol-default lanes. If String wants custom number parsing, it re-declares them as type-level overrides.

Also simplify RingBuffer, UTF8View, etc. — remove cast-related fields and ops that are now provided by protocol membership.

### Phase 7: Wire proto declarations into the casting graph

**File:** `src/backend/llvm/normalizer.rs` + `src/compile.rs`

- In `register_typedefs()` (or a separate pass), scan for `TopLevel::ProtocolDef` items
- Call `casting_graph.register_protocol_def(pd)` for each
- Validate consistency: for each CastTo/CastFrom on a proto, compute the equivalent path through base protocols and compare. If different, emit ERROR.

### Phase 8: Clean up and test

- Remove dead functions (get_shuffle_int_owned, type_name_str_from_llvm, etc.)
- Remove `Cast#` from `intrinsic_signatures.rs` (line 72)
- Update `is_protocol_member()` to check casting graph instead of universe properties
- Remove `ProtocolGraph` from `src/analysis/protocol_graph.rs` (merged into CastingGraph)
- Ensure all 1211+ tests pass
- Run full benchmark suite to verify no regressions

---

## 4. File-by-File Change Summary

| File | Lines added | Lines removed | Net |
|------|-------------|---------------|-----|
| `src/casting/mod.rs` (NEW) | ~20 | 0 | +20 |
| `src/casting/graph.rs` (NEW) | ~480 | 0 | +480 |
| `src/type_universe/mod.rs` | 0 | ~20 | -20 |
| `src/backend/llvm/normalizer.rs` | ~10 | ~130 | -120 |
| `src/backend/llvm/emit_toplevel.rs` | ~15 | ~20 | -5 |
| `src/backend/llvm/helpers.rs` | 0 | ~15 | -15 |
| `src/backend/llvm/mod.rs` | ~5 | ~10 | -5 |
| `src/backend/llvm/emit_expr.rs` | ~30 | ~280 | -250 |
| `src/backend/llvm/intrinsics.rs` | ~20 | ~160 | -140 |
| `src/compile.rs` | ~10 | ~35 | -25 |
| `src/analysis/protocol_graph.rs` | 0 | ~570 | -570 |
| `src/intrinsic_signatures.rs` | 0 | ~1 | -1 |
| `lib/std/types/bootstrap.bv` | ~5 | ~20 | -15 |
| `lib/std/string.bv` | 0 | ~10 | -10 |
| **Total** | **~595** | **~1271** | **-676** |

---

## 5. Protocol → LLVM Type Resolution

LLVM types are resolved from `(protocol, metadata)` by the **casting graph**,
not stored as cached properties on universe entries. The normalizer registers
types in the universe; the graph answers LLVM type queries at codegen time.

| Protocol / Variant | Metadata | LLVM type | Resolution |
|---|---|---|---|
| `#Int` | (none) | `i{int_bits}` | `WidthParametric` → `int_bits` |
| `#Int` | `bits: 32` | `i32` | `WidthParametric` → `!> bits` |
| `#UInt` | (none) | `i{int_bits}` | `WidthParametric` → `int_bits` |
| `#Float` | (none) | `float` | `Fixed("float")` |
| `#Float<BFloat>` | (none) | `bfloat` | `Fixed("bfloat")` — no `disamb` |
| `#Float<Half>` | (none) | `half` | `Fixed("half")` |
| `#Float<Double>` | (none) | `double` | `Fixed("double")` |
| `#Float<FP128>` | (none) | `fp128` | `Fixed("fp128")` |
| `#Float<X86_FP80>` | (none) | `x86_fp80` | `Fixed("x86_fp80")` |
| `#String` | (none) | `{ i64, i64 }` | `Fixed("{ i64, i64 }")` |
| `#String<UTF8>` | (none) | `{ i64, i64 }` | `Fixed("{ i64, i64 }")` |
| `#String<ASCII>` | (none) | `{ i64, i64 }` | `Fixed("{ i64, i64 }")` |
| `#Bool` | (none) | `i8` | `Fixed("i8")` |
| `#Char` | (none) | `i32` | `Fixed("i32")` |
| `#Bit` | (none) | `i{int_bits}` | `WidthParametric` → `int_bits` |
| `#Data` | (none) | `ptr` | `Fixed("ptr")` |

`WidthParametric` resolution priority:
1. `!> bits: N` → exact width (overrides everything — "my sanity demands this width")
2. `!> maxbits: N` → upper bound (optimizer may narrow further, but never widen past N)
3. `!> minbits: N` → lower bound (optimizer may widen, but never narrow below N)
4. `int_bits` → target default

### 5.1 Primordial type population

The PRIMORDIALS table provides overrideable defaults for `Int`, `Float`,
`Bool`, `Char`, `Data`, `Void`, and all fixed-width types. `Bit` is not in
PRIMORDIALS — it is seeded separately as the axiomatic anchor before the
loop and cannot be overridden (the normalizer errors if any code declares
`type Bit`). Every PRIMORDIALS entry can be overridden by stdlib or user
`.bv` files.

Each primordial entry specifies `(name, bytes, min_bits, max_bits, alignment, properties)`.
No `llvm_type`, no `disamb`, no `Cast.#` properties — those are either
hardcoded in the casting graph (cast paths, protocol LLVM types) or derived
from metadata (`!> bits`). The normalizer fills in
`bytes`/`min_bits`/`max_bits`/`alignment` from protocol defaults + metadata.
Protocol membership for `is_protocol_member()` checks the casting graph instead
of universe properties.

---

## 6. Base Protocol Lane Table

All 8 base protocols × 8 base protocols = 64 entries. Each entry is a direct lane (no BFS needed for base→base casts):

| src → dst | Lane type | LLVM IR |
|-----------|-----------|---------|
| `#Int` → `#Bit` | Bitcast | `bitcast i64 %v to i64` |
| `#Int` → `#Float` | IntToFloat | `sitofp i64 %v to double` |
| `#Int` → `#String` | ExtCall("__int_to_str__") | `call i64 @__int_to_str__(i64 %v)` |
| `#Int` → `#Bool` | Bitcast+trunc | `trunc i64 %v to i8` |
| `#Int` → `#Char` | Bitcast+trunc | `trunc i64 %v to i32` |
| `#Int` → `#UInt` | Bitcast (identity) | (no-op, same representation) |
| `#Float` → `#Bit` | Bitcast | `bitcast double %v to i64` |
| `#Float` → `#Int` | FloatToInt | `fptosi double %v to i64` |
| `#Float` → `#String` | ExtCall("__float_to_str__") | `call i64 @__float_to_str__(double %v)` |
| `#Float` → `#Bool` | FloatToInt+trunc | (chain) |
| `#String` → `#Bit` | ExtractData | `extractvalue { i64, i64 } %v, 0` |
| `#String` → `#Int` | ExtCall("__str_to_int") | `call i64 @__str_to_int(i64 %v)` (or ptr cast) |
| `#String` → `#Float` | ExtCall("__str_to_float") | `call double @__str_to_float(i64 %v)` |
| `#String` → `#Bool` | ExtCall("__str_to_bool") | `call i8 @__str_to_bool(i64 %v)` |
| `#Bool` → `#Bit` | Zext | `zext i8 %v to i64` |
| `#Bool` → `#Int` | Zext | `zext i8 %v to i64` |
| `#Bool` → `#String` | ExtCall("__bool_to_str") | `call i64 @__bool_to_str(i8 %v)` |
| `#Char` → `#Int` | Zext | `zext i32 %v to i64` |
| `#Char` → `#String` | ExtCall("__char_to_str") | `call i64 @__char_to_str(i32 %v)` |
| `#UInt` → `#Int` | Bitcast (identity) | (no-op, same representation) |
| `#Data` → `#Int` | PtrToInt | `ptrtoint ptr %v to i64` |
| `#Data` → `#Bit` | PtrToInt | `ptrtoint ptr %v to i64` |
| `#Bit` → `#Int` | Bitcast | `bitcast i64 %v to i64` (identity for 64-bit) |
| `#Bit` → `#String` | ExtCall("__bits_to_str") | `call i64 @__bits_to_str(i64 %v)` |
| ... | ... | ... |

### 6.1 CastFrom(#Bit) overrides

For all `#Bit → X` lanes, the graph emits the **protocol default** first (bitcast the raw bits to the protocol's LLVM type). If the target type declares `op CastFrom(#Bit): constructor(#L)`, the graph uses that instead:

```
#Bit → #String: protocol default = bitcast i64 to {i64, i64}
                with CastFrom(#Bit) override = call @constructor(i64 %v)
```

This is the **only** user-extensible edge direction. It is registered during `register_typedefs()` by scanning type bodies for `op CastFrom(#Bit)` bindings. No other CastTo/CastFrom edge can be declared on types (only CastTo(#Category)/CastFrom(#Category) for non-#Bit categories are type-level lane overrides; CastTo(#Bit) is banned entirely).

### 6.2 Lane override priority for non-#Bit lanes

For lanes like `#String → #Int`, the graph checks:
1. Does the source type have a `CastTo(#Int)` override? → use that
2. Otherwise → use the protocol default (`__str_to_int`)

For lanes like `#Int → #String`, the graph checks:
1. Does the target type have a `CastFrom(#Int)` override? → use that
2. Otherwise → use the protocol default (`__int_to_str__`)

This is encoded in the graph's `emit_cast()` logic as a simple two-check before falling through to the hardcoded table entry. Both `type_overrides` are populated from `op CastTo`/`op CastFrom` declarations in type bodies (excluding `CastTo(#Bit)` which is banned, and `CastFrom(#Bit)` which is handled separately).

---

## 7. Edge Cases and Validation

### 7.1 Variant consistency check

When a proto declaration defines `CastTo(#String): ascii_to_utf8(#L)`, the compiler:
1. Records `(String, ASCII) → (String, UTF8)` as a variant edge
2. Computes the base-protocol path: `(String, ASCII) → (String, UTF8) → [#String → #String]` = identity through default
3. Since the base path is identity, no cross-check is needed for same-category variants
4. **Cross-category check:** If a proto declares `CastTo(#Int): my_custom_parse(#L)`, the compiler:
   - Records `(String, ASCII) → (Int, UTF8)` (Int has no variants, so default is empty)
   - Computes base path: `(String, ASCII) → (String, UTF8) → (#String → #Int)` = `ascii_to_utf8` + `__str_to_int`
   - Compares: does `ascii_to_utf8(x)` + `__str_to_int(...)` == `my_custom_parse(x)` for all x?
   - If not provably equal → **ERROR**. The variant override would produce different results than the base-protocol path.

### 7.2 `is_protocol_member()` migration

Currently checks universe properties for `Cast.#Int` etc. Under casting graph, checks:
```
fn is_protocol_member(ty: &Type, protocol: &str) -> bool {
    let (cat, var) = resolve_type_to_category_variant(ty);
    graph.find_path(&cat, &var, protocol, "")  // O(1) for base protocols
        .or(graph.find_path(protocol, "", &cat, &var))
        .is_some()
}
```

This is a two-directional check: `is_protocol_member(String, "#Int")` asks "can String reach Int or vice versa?" — answered by checking if a path exists.

For primitive types (`Type::Int`, `Type::Bool`, `Type::Ptr`, etc.):
```rust
match ty {
    Type::Int => protocol == "#Int" || protocol == "#Bit",
    Type::Bool => protocol == "#Bool" || protocol == "#Bit",
    Type::Ptr(_) => protocol == "#Data" || protocol == "#Int" || protocol == "#Bit",
    Type::Bits(_) => protocol == "#Bit",
    Type::Custom(n) | Type::Applied(n, _) => {
        // Look up in casting graph
        graph.is_reachable(n, protocol)
    }
    _ => false,
}
```

### 7.3 Backwards compatibility

- `Expr::Cast(42, String)` — still works, casting graph replaces `resolve_cast`
- `Cast#(42, target)` — still works, casting graph replaces `emit_intrinsic_cast` dispatch
- `x as String` — still works (same AST node)
- Primordial types populate same universe fields (bytes, alignment) — `llvm_type` and `Cast.#` properties removed
- `self.llvm_type(ty)` returns the same strings as before — just resolved through the casting graph instead of universe properties
- `!> bits: N` works identically to before (the old normalizer also read `bits` from metadata)
- `operator_defs` still passes InsertAt/ExtractFrom to ring buffer code — unchanged
- Tests that assert IR snapshots with `__str_to_int` / `__int_to_str__` still pass (same LLVM IR emitted)

### 7.4 Layout Optimization interaction

The `find_cast_path()` BFS in `layout_optimizer.rs` used the `Cast.#` properties in the universe to find protocol paths. After this rewrite, that BFS must either:
- (a) Check the casting graph instead of universe properties, or
- (b) Accept any type that `is_protocol_member()` returns true for

Option (b) is simpler and already correct — `find_cast_path` in `layout_optimizer.rs` calls `check_property` to find `Cast.#Category` entries. Since `is_protocol_member()` will check the graph, the layout optimizer's BFS will still work if it uses `is_protocol_member()`.

Check: `layout_optimizer.rs` line references.

---

## 8. Test Strategy

1. **Phase 0** (module creation): Unit tests for `CastingGraph::find_path()` with all base protocol pairs. Verify no false negatives.
2. **Phase 1** (Expr::Cast): Existing test suite — all 1211 tests pass. Key tests: `test_string_cast`, `test_int_to_float`, `test_cast_intrinsic`.
3. **Phase 2** (Cast# intrinsic): Same — existing tests cover the intrinsic path.
4. **Phase 3** (normalizer): Verify removing Cast.# injection doesn't break `is_protocol_member()` — the graph replacement in helpers.rs must be correct.
5. **Phase 4** (compile.rs): Verify operator_defs still contains InsertAt/ExtractFor ring buffer ops.
6. **Phase 5** (parser): Only removes dead `op Cast()` syntax path — no test impact.
7. **Phase 6** (stdlib): Update bootstrap.bv and verify all stdlib tests pass.
8. **Phase 7** (proto declarations): Add new tests for proto variant consistency checking.
9. **Phase 8** (cleanup): `cargo test --lib`, `cargo build --release`, benchmark suite.

### Regression guard

Before each phase commit:
1. `cargo test --lib` — all tests pass
2. `cargo build` — no warnings
3. Verify specific benchmark: `bash benchmarks/compare_baseline.sh nbody` for runtime, `bash benchmarks/compare_baseline.sh ring_buffer` for optimizer

---

## 9. Documentation Updates

| Document | Update |
|----------|--------|
| `docs/architecture/casting-protocol.md` | Rewrite to describe casting graph, remove reference to operator_defs-based dispatch |
| `docs/architecture/backend-type-dispatch.md` | Update `is_protocol_member()` section, remove Cast.# property description |
| `docs/features/casting.md` | Update to describe protocol-first casting with graph |
| `AGENTS.md` | Update summary (remove Cast.# injection, Cast->graph references) |
| `AGENTS_HISTORY.md` | Add entry for casting graph rewrite |
| `spec/SPEC.md` | Remove `op Cast()` from grammar reference, add proto declaration semantics |

---

## 10. Rejected Alternative: type_overrides HashMap

An earlier draft used a `type_overrides: HashMap<(TypeName, SrcCat, DstCat), LaneEmitter>` to let any type override any lane. This was rejected because:

- `CastTo(#Bit)` must **never** be overridable (representation guarantee). A `type_overrides` HashMap makes it too easy to accidentally allow it.
- Only `CastFrom(#Bit)` is meaningfully user-extensible. Non-#Bit CastTo/CastFrom overrides already exist as type-level declarations that feed into the same path resolution.
- The HashMap adds complexity (edge case: what if two types in the same hierarchy declare different overrides for the same lane?) with no practical benefit.

The final design has exactly **one** user-extensible edge (`CastFrom(#Bit)`) and **zero** user-overridable `CastTo` edges into `#Bit`.

---

## 11. Rollback Path

If the casting graph rewrite causes regressions that cannot be resolved within 24 hours:

1. `git revert HEAD~8` (revert all 8 phases)
2. Restore `operator_defs`-based dispatch as the primary path
3. Keep `CastingGraph` as an optional optimization (dual-path, selected via `--use-casting-graph` flag)
4. Debug the root cause with controlled A/B: same inputs, compare graph path vs operator_defs path output

The dual-path approach follows AGENTS.md rule 4 (Dual-Path / Adaptive Optimizations): `--use-casting-graph` flag defaults to false until the graph path matches operator_defs output for ALL 1211 tests + ALL 19 benchmarks.
