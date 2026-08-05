# Phase 9: Remove Hardcoded Type Name Matching from Backend

**Date:** 2026-07-30
**Status:** Plan
**Principle:** Types are protocol + metadata (`docs/architecture/bits-thesis.md`). They have no canonical layout — the casting graph derives LLVM representation at codegen time. **Name matching (`Type::Custom(name) if name == "String"`) violates this architecture and must be replaced with protocol queries.**

## Architecture Reference

### How Type Resolution Works

```
Source: type String: #String { !> bytes: 16; };
   ↓ parser
AST: Type::Custom("String")
   ↓ universe registration + normalizer
TypeUniverse entry: { name: "String", properties: { "Cast.#String": true }, bytes: 16 }
   ↓ casting graph query (graph.rs:492)
type_to_protocol(universe, ty) → ("String", "")  // via Cast.#String property
   ↓ LLVM type resolution (graph.rs:540)
resolve_llvm_type(universe, ty, int_bits) → "{ i64, i64 }"  // via protocol variant
```

The normalizer injects `Cast.#<Category>` properties during universe registration. The casting graph's `type_to_protocol()` at `graph.rs:492` queries these properties — it never matches type names.

### The Violation Pattern

```rust
// WRONG — matches type name directly:
Type::Custom(name) if name == "String" => ...

// RIGHT — queries protocol membership:
let (cat, _) = graph.type_to_protocol(universe, ty);
if cat == "String" { ... }
```

### Primitives vs Primordials (from bits-thesis.md)

| | Primitive | Primordial |
|---|---|---|
| **Examples** | `Bit` | `Int`, `Float`, `Bool`, `Char`, `Data`, `Void` |
| **Overrideable?** | No | Yes — bootstrap.bv or user `.bv` files can replace |
| **Why** | Axiomatic anchor | Useful defaults |

Per Rule 2/18: `Type::Ptr(_)` | `Type::Bits(N)` | `Type::Vector(_, _)` are the ONLY compiler constructs permitted to match by constructor. Everything else goes through the casting graph.

## Phase 9a: Fix `declare_struct_types` Duplicate (~5 min)

### Problem

`emit_toplevel.rs:133-150` emits hardcoded struct type declarations:
```llvm
%SmallString64 = type { i64 x 9 }
%StaticString = type { i64, i64 }
%String = type { i64, i64 }
%UTF8View = type { i64, i64 }
```

Then `emit_toplevel.rs:152-164` iterates the universe and emits ALL struct types with fields — including `%String`, `%SmallString64`, `%UTF8View`, `%StaticString` if they have field definitions in the universe. This produces:

```llvm
%String = type { i64, i64 }    ; from hardcoded block
%String = type { i64, i64 }    ; from universe iteration — DUPLICATE!
```

Clang rejects: `error: redefinition of type '%String'`

### Fix

In `declare_struct_types` (`emit_toplevel.rs:133`), skip universe types whose names match the hardcoded list:

```rust
let hardcoded: HashSet<&str> = ["SmallString64", "StaticString", "String", "UTF8View"]
    .iter().cloned().collect();

for (name, field_tys) in &universe_fields {
    if hardcoded.contains(name.as_str()) { continue; }
    writeln!(out, "%{} = type {{ {} }}", name, field_tys.join(", ")).ok();
}
```

### Verification
```bash
grep "%String" nbody_newton.ll | wc -l   # → 1 (not 2)
bash benchmarks/build_and_bench.sh --correctness   # all MATCH
```

## Phase 9b: Protocol Query Helpers (~30 min)

### Add to `graph.rs` (or use existing `type_to_protocol`)

The `type_to_protocol` function at `graph.rs:492` already returns `(category, variant)`. All call sites can use this directly. If a shorter form is desired:

```rust
impl LlvmBackend {
    fn protocol_of(&self, ty: &Type) -> &str {
        let Some(graph) = self.ctx.casting_graph.as_ref() else { return "Bit" };
        let Some(univ) = self.ctx.type_universe.as_ref() else { return "Bit" };
        graph.type_to_protocol(univ, ty).0
    }
}
```

## Phase 9c: Replace All Violations (~2h)

### Group A: Simple string/UTF8View/Slice checks (8 sites)

| File | Line | Current code | Replacement |
|------|------|-------------|-------------|
| `mod.rs` | 497 | `name == "String" \|\| name == "UTF8View"` | `self.protocol_of(ty) == "String"` |
| `emit_toplevel.rs` | 272 | `name == "UTF8View"` | Check `feature_sso_strings` OR `self.protocol_of(ty) == "String"` |
| `emit_toplevel.rs` | 302 | `name == "String"` | `self.protocol_of(ty) == "String"` |
| `emit_toplevel.rs` | 316 | `name == "String" \|\| name == "Data"` | `matches!(self.protocol_of(ty), "String" \| "Data")` |
| `emit_toplevel.rs` | 1751 | `s == "Float" \|\| s == "Float32"` | `self.protocol_of(ty) == "Float"` |
| `emit_toplevel.rs` | 1786 | `s == "Float"` | `self.protocol_of(ty) == "Float"` |
| `emit_expr.rs` | 1516 | `ty_name == "String" \|\| ty_name == "Data"` | `matches!(self.protocol_of(ty), "String" \| "Data")` |

### Group B: Multi-arm match blocks (4 sites)

| File | Line | Current | Replacement |
|------|------|---------|-------------|
| `emit_toplevel.rs` | 568-580 | `match ty { Custom(t) if t == "Bool"/"Int"/... }` | `match self.protocol_of(ty) { "Bool" => ..., "Int" => ..., "Float" => ..., "String" => ... }` |
| `emit_toplevel.rs` | 1232-1244 | Same in `emit_param_load` | Same |
| `emit_toplevel.rs` | 2237-2250 | Same in `emit_txn_param_load` | Same |
| `mod.rs` | 2329 | Same in trigger type dispatch | Same |

### Group C: `builder.rs` box/unbox fallbacks (20+ lines)

`box_to_i64_fallback` (line 546) and `unbox_from_i64_fallback` (line 594) match `Custom(t) if t == "Bool"/"String"/"Data"/"Float"/"Float64"/"Int8"/"Int16"/"UInt8"/"UInt16"/"Int32"/"UInt32"`.

Fix: Pass `&CastingGraph` and `&TypeUniverse` to the builder's fallback functions. Replace name matches with:

```rust
fn category_and_bytes(graph: &CastingGraph, univ: &TypeUniverse, ty: &Type) -> (&str, u64) {
    let cat = graph.type_to_protocol(univ, ty).0;
    let bytes = ty.universe_key()
        .and_then(|k| univ.get(k))
        .map(|rt| rt.bytes)
        .unwrap_or(8);
    (cat, bytes)
}
```

Then dispatch on `(category, bytes)` instead of type names.

### Group D: `primitive_from_name` in `mod.rs:115-132`

This maps LLVM type string → Briv type name for `operator_defs` lookup. Fix: iterate the universe looking for a type whose `resolve_llvm_type()` matches the requested LLVM type string:

```rust
fn resolve_primitive_name(llvm_ty: &str, graph: &CastingGraph, univ: &TypeUniverse) -> Option<String> {
    for rt in univ.types.values() {
        let briv_ty = rt.to_type();
        if graph.resolve_llvm_type(univ, &briv_ty, 64) == llvm_ty {
            return Some(rt.name.clone());
        }
    }
    None
}
```

Cache the result (universe is small — O(n) is fine).

## Phase 9d: Write the AI Backend Guide (~2h)

**File:** `docs/architecture/backend-architecture.md`

**Audience:** AI coding agent or new contributor who understands Briv's type system (protocol + metadata) and LLVM IR, but not the specific backend codebase.

**Outline:**

1. **Three-Layer Architecture**
   - `CompilerContext` (global, read-only during codegen)
   - `FunctionContext` (per-function, SSA registers, phi nodes, caches)
   - `LlvmBackend` (orchestrator, delegates to modules)

2. **Code Generation Flow**
   ```
   generate()
     → build_field_index() — assign state slot indices
     → declare_types() — emit LLVM type declarations
     → declare_state() — emit %State struct
     → emit_main_or_bootup() — emit @main or __briv_init_state
       → init_state / reactor / txn dispatch
       → body emission (PerFieldPhi / InlineSsa / Batch-loop)
   ```

3. **The Protocol Dispatch Chain (THE GOLDEN RULE)**
   - Normalizer injects `Cast.#<Category>` properties
   - CastingGraph::type_to_protocol() maps type → protocol category
   - LLVM backend queries `is_protocol_member()` — **never matches type names**
   - `Type::Ptr(_)`, `Type::Bits(N)`, `Type::Vector(_, _)` are the ONLY exceptions (compiler constructs)

4. **Loop Dispatch Strategies**
   - **Pure counter fold** — O(1), single store
   - **Inline SSA** — folded loop with insertvalue/extractvalue
   - **PerFieldPhi** — per-field phi nodes (default)
   - **Batch-loop** — outer structural + inner pure-compute (when guards detected)

5. **Key Data Structures**
   - `field_index_map`: field name → state slot index
   - `field_types` / `field_briv_types`: LLVM / Briv type per slot
   - `phi_field_regs`: per-field phi register (inside loop body)
   - `pending_phi_backedge`: computed values awaiting latch emission
   - `field_to_phi` / `field_to_lane`: vector phi group mapping

6. **The Casting Graph in Detail**
   - `graph.rs:492` — `type_to_protocol()`: type → (category, variant)
   - `graph.rs:540` — `resolve_llvm_type()`: (protocol, bytes) → LLVM type string
   - `graph.rs:85-100` — 64 base protocol lanes + variant edges
   - Hardcoded lanes: Bit→Int, Int→Float, Float→String, etc.
   - `find_path()`: BFS through the graph for cross-protocol casts

7. **Common Pitfalls (Rule 18 Enforcement)**
   - ❌ `Type::Custom(name) if name == "String"` → use `is_protocol_member(ty, "#String")`
   - ❌ Matching on `field_types[idx].as_str() == "float"` → use `resolve_llvm_type()` result
   - ❌ Hardcoding alignment values → use `align_of()`
   - ✅ `Type::Ptr(_)` | `Type::Bits(N)` | `Type::Vector(_, _)` permitted

8. **Adding a New Type**
   - Step 1: Define in stdlib `.bv` with protocol membership
   - Step 2: If new protocol, add lane in `graph.rs` `new()` function
   - Step 3: Normalizer checks: does `normalizer.rs` need a new `Cast.#<Category>`?
   - Step 4: Backend: add protocol arm in `type_to_protocol` priority chain
   - **No name-based matching anywhere in the backend**

## Timeline

| Phase | Description | Effort |
|-------|-------------|--------|
| 9a | Fix `%String` duplicate in `declare_struct_types` | 5 min |
| 9b | Protocol query helpers | 30 min |
| 9c | Replace all violation sites (Groups A-D) | 2-3h |
| 9d | Backend architecture guide | 2h |

## Verification

```bash
cargo test --lib                          # all tests pass
bash benchmarks/build_and_bench.sh --correctness   # all MATCH
bash benchmarks/build_and_bench.sh --runtime       # no regressions
# Audit — must return ZERO:
grep -rn 'Type::Custom.*if.*==.*"' src/backend/llvm/ | grep -v 'test\|tb aa\|Int\b\|Float\b\|Bool\b\|Ptr\b'
```
