# Meld Feature — Comprehensive Implementation Plan

**Date:** 2026-06-22  
**Status:** Plan — awaiting implementation approval  
**Design iteration:** 4 (initial Bits Thesis → LinkedIn discussion → chimera model → meld keyword)

---

## Table of Contents

1. [Overview](#1-overview)
2. [Syntax and Semantics](#2-syntax-and-semantics)
3. [Inference Algorithm](#3-inference-algorithm)
4. [Three Memory Paths (Adaptive Layout)](#4-three-memory-paths-adaptive-layout)
5. [Boundary-Driven Decay](#5-boundary-driven-decay)
6. [Phase Breakdown](#6-phase-breakdown)
7. [Error Message Designs](#7-error-message-designs)
8. [Generics (Deferred)](#8-generics-deferred)
9. [Proof Engine Integration](#9-proof-engine-integration)
10. [Performance Analysis](#10-performance-analysis)
11. [Relationship to Existing Plans](#11-relationship-to-existing-plans)
12. [Anti-Patterns (NEVER DO)](#12-anti-patterns-never-do)

---

## 1. Overview

### Problem

Two types that represent the same logical value — a `String` and a `CString`, a `Float` and a `CFloat`, a `JsonValue` and a `String` — require explicit conversion code every time they cross between contexts. The conversion either copies data (`strlen` + allocation for CString→String) or requires unsafe reinterpretation (`crack!`).

The `meld` feature solves this by declaring that two types are **mutually lens-compatible**: they are the same logical value seen through different lenses. A value cast across a `meld` boundary becomes a **chimera** — a unified value that satisfies both type contracts simultaneously. The compiler manages the representation adaptively based on usage, and mutations through either lens propagate to the shared value.

### Core Insight

A `meld` is not "you can cast A to B." It is "A and B share the same underlying bits — casting is just changing the lens."

This difference is critical:
- **Regular cast** (`x as Float`): converts the bits (`sitofp`), costs instructions, creates a copy
- **Meld cast** (`x as CFloat` where `meld Float <:> CFloat`): same bits, zero instructions, no copy. Mutations through either lens affect the same value.

### Key Invariant

A meld-backed value satisfies **all contracts of both types simultaneously**. When you mutate a `CString`-lensed chimera, the `String` lens sees the mutation. When you read through the `String` lens, you get the same logical value. The compiler ensures this by:

1. **Storage sharing**: The chimera's physical storage is the backing type's layout. The other type's fields are projections over the same storage.
2. **Cache coherency**: When a derived field is computationally expensive (like `strlen#`), the compiler may cache it. Mutations invalidate the cache.
3. **Boundary materialization**: At struct stores and FFI boundaries, the chimera "unpacks" to the canonical layout of the target type. This is the only case where bits are actually reorganized.

---

## 2. Syntax and Semantics

### Declaration

```brief
// Tier 1 — Fully inferred. Same bytes+alignment → zero-cost identity routing.
meld Float <:> CFloat;

// Tier 2 — Partially inferred. Compiler infers what it can, user fills gaps.
meld String <:> CString {
    // Only the non-trivial projections need explicit routes.
    // Fields with matching @/ ranges (like Ptr @/0..63) are auto-inferred.
    String.Len = CString :> Size;
};

// Tier 3 — Fully explicit. User overrides default @/-based inference.
meld MyFloat <:> YourFloat {
    MyFloat.Ptr = YourFloat.Data :> crack<Ptr<Byte>>;
};
```

### Rules

1. `meld A <:> B;` with no body is valid **iff** the inference algorithm can derive all routes in both directions automatically. This is the case when both types have the same `Bytes`, `Alignment`, and every field in each type has a matching field or projection in the other.

2. `meld A <:> B { ... };` with a body overrides specific routes. Routes not listed are still inferred automatically. The body only needs to specify what inference cannot derive or what the user wants to customize.

3. A `meld` is a `TopLevel` declaration — it lives at file scope in the `TypeUniverse`. It has no nesting, no local scope.

### Usage

```brief
meld Float <:> CFloat;

let f: Float = 3.14;
let cf: CFloat = f as CFloat;     // Zero-cost: same bits, different lens
cf :> Size                        // reads Float Size (which is 1 — both are scalar)
cf :> Add(CFloat(2.0))            // emits fadd float — same opcode regardless of lens
```

```brief
meld String <:> CString {
    String.Len = CString :> Size;
};

let s: String = "hello world";
let cs: CString = s as CString;   // Chimera created, backed by String (16 bytes)
let first = cs[0];                 // CString lens: byte load from ptr[0], no strlen
let len = cs :> Size;             // CString lens: calls strlen#(ptr) — or cached
let s_len = (cs as String) :> Size; // String lens: reads cached len (O(1))

// Mutation through String lens updates CString too:
let s2 = (cs as String) ++ " suffix";  // Allocates new buffer, updates ptr+len
let first2 = cs[0];                     // Sees the new buffer — same ptr
let len2 = cs :> Size;                  // Cache was invalidated — calls strlen again
```

---

## 3. Inference Algorithm

### Purpose

The inference algorithm automatically derives projection routes between two types in a `meld` declaration. It runs once at the declaration site (during TypeUniverse construction) and produces a set of `MeldRoute`s that the codegen and interpreter use to dispatch projections.

### Algorithm

```
fn infer_meld(a: &ResolvedType, b: &ResolvedType) -> Result<Vec<MeldRoute>, MeldError> {
    let mut routes = Vec::new();

    // Direction A → B: derive B's projections from A's structure
    for (b_name, b_proj) in &b.projections {
        // Try 1: same @/ range in A
        if let Some(a_proj) = find_by_bit_range(a, b_proj.bit_range()) {
            routes.push(MeldRoute {
                from_type: "A", from_proj: a_proj.name.clone(),
                to_type: "B", to_proj: b_name.clone(),
                strategy: RouteStrategy::Identity,
            });
        }
        // Try 2: projection in A produces same type
        else if let Some(a_proj) = find_by_semantic_name(a, b_name) {
            routes.push(MeldRoute {
                from_type: "A", from_proj: a_proj.name.clone(),
                to_type: "B", to_proj: b_name.clone(),
                strategy: RouteStrategy::Projection(a_proj.expr.clone()),
            });
        }
        // Try 3: intrinsic derivation (e.g., String.Len → CString :> Size)
        else if let Some(derived) = find_intrinsic_route(a, b, b_name) {
            routes.push(derived);
        }
        // Otherwise: ERROR — cannot derive this field
        else {
            return Err(MeldError::NoRoute {
                source: "A",
                target: "B",
                field: b_name.clone(),
            });
        }
    }

    // Direction B → A: same process reversed
    // (identical logic with A and B swapped)

    Ok(routes)
}
```

### Matching Heuristics

| Heuristic | Condition | Result |
|-----------|-----------|--------|
| **Exact bit-range match** | `A.field @/x..y` and `B.field @/x..y` | Identity route: `B.field = A.field` |
| **Semantic name match** | `A.Size` and `B.Length` (known projection synonyms) | Projection route: `B.Length = A.Size` |
| **Intrinsic derivation** | `A.Len @/64..127` but B is 8 bytes (no bits 64+) | Requires explicit route: `String.Len = CString :> Size;` |
| **Type-projection match** | `A.Ptr` produces `Ptr<Byte>` and B has `Ptr` with same `@/` | Identity route |

### Exact Bit-Range Match (Primary Heuristic)

Each field in both types has a `BitRange` annotation (`@/`). If field `X` in type A occupies `@/0..63` and field `Y` in type B occupies `@/0..63`, the compiler infers that `B.Y = A.X` — they are the same bits with different names.

**For `String <:> CString`:**

| Field | String | CString | Match? |
|-------|--------|---------|--------|
| Ptr | `@/0..63` | `@/0..63` | ✓ Identity |
| Len | `@/64..127` | **no bits 64+** | ✗ No match — CString is 8 bytes |

Result: Ptr auto-inferred, Len needs explicit route.

### Semantic Name Match (Secondary Heuristic)

If bit ranges don't match, the compiler checks a synonym table:

| Synonym group | Names |
|---------------|-------|
| Size/Length/Len | `Size`, `Length`, `Len` |
| Data/Buffer/Bytes | `Data`, `Buffer`, `Bytes` |
| Ptr/Pointer/Address | `Ptr`, `Pointer`, `Address` |

### Intrinsic Derivation (Fallback)

If no matching field exists, the compiler checks whether the target projection can be derived from any combination of the source type's projections. This is limited to known patterns:

- `String.Len` → `CString :> Size` (known: String's Len = CString's strlen semantics)
- Any projection P → if source type's `codec` matches target type's `codec`, attempt codec-based derivation

This fallback is intentionally conservative. If it fails, the error message tells the user to provide an explicit route.

### No Implicit Aliasing

The inference algorithm does NOT allow inference across generic parameters. `List<String>` and `Vec<String>` are NOT treated as compatible unless a `meld List<T> <:> Vec<T>` generic declaration exists (see Section 8). This is a deliberate constraint to keep Phase 0-2 manageable.

---

## 4. Three Memory Paths (Adaptive Layout)

### Overview

Once a meld-backed chimera exists, the compiler must decide how to lay out its physical storage. The three paths are selected **statically** at compile time based on liveness analysis evidence. There is no runtime path switching.

### Path Selection Criteria

| Path | When selected | Physical layout | When to use |
|------|---------------|-----------------|-------------|
| **Short Path** (default) | No loop appears, or only one lens is active | Backing type's canonical layout. The other lens derives on demand. | Most code — single-lens usage, infrequent access |
| **Hot Dual** | Both lenses active in a loop nest (projection-usage evidence) | Backing type + cache slots + valid flag for deferred projections | `rct txn` loops, tight iteration |
| **Unpack at Transfer** | Value crosses a struct store or FFI boundary | Canonical layout of the target type | Function returns, FFI calls, struct field writes |

### Short Path — Concrete LLVM

```
Source:  meld String <:> CString;
         let s: String = "hello";
         let cs: CString = s as CString;
         let first = cs[0];

%State layout:  { ptr: i64, len: i64 }   // 16 bytes — pure String layout
                                            // CString lens derives everything from ptr

LLVM codegen for cs[0]:
  %ptr_gep = getelementptr %State, %State* %state, i32 0, i32 0
  %ptr = load i64, i64* %ptr_gep
  %ptr_i8 = inttoptr i64 %ptr to i8*
  %char = load i8, i8* %ptr_i8
```
**Zero overhead.** No strlen. No extra fields. The CString lens reads `ptr` from the same slot String uses.

### Hot Dual — Concrete LLVM

```
Source:  rct txn loop [i < cs :> Size][i == cs :> Size] {
             process(cs[0]);
             &i = i + 1;
         };
         let len = (cs as String) :> Size;

Projection-usage evidence: `cs :> Size` appears in a loop body.
Both `CString::Size` and `String::Size` are used on the same value.

%State layout:  { ptr: i64, len: i64, strlencache: i64, cache_valid: i8 }

LLVM codegen for cs :> Size (CString lens):
  ; Check cache
  %valid_gep = getelementptr %State, ... , i32 3
  %valid = load i8, i8* %valid_gep
  %valid_bool = icmp ne i8 %valid, 0
  br i1 %valid_bool, label %cached, label %compute

compute:
  ; First and only call to strlen
  %cache_gep = getelementptr %State, ... , i32 2
  %len_val = call i64 @strlen(i8* %ptr_i8)
  store i64 %len_val, i64* %cache_gep
  store i8 1, i8* %valid_gep
  br label %done

cached:
  %cache_gep2 = getelementptr %State, ... , i32 2
  %len_val = load i64, i64* %cache_gep2
  br label %done

done:
  ; %len_val used as :> Size result

LLVM codegen for (cs as String) :> Size (String lens):
  ; Still O(1) — reads len directly
  %len_gep = getelementptr %State, ... , i32 1
  %len_val = load i64, i64* %len_gep

On mutation — cache invalidation:
  store i8 0, i8* %valid_gep    ; Set cache_valid = false
```

### Unpack at Transfer — Concrete LLVM

```
Source:  defn get_string() -> CString {
             let s: String = compute();
             term s as CString;
         };

At the `term` boundary:
  ; The function returns CString (8 bytes, ptr only).
  ; The chimera must unpack to CString's canonical form.

  ; Ensure null terminator at ptr[len] — CString must be valid
  %len = load i64, i64* %len_gep
  %ptr = load i64, i64* %ptr_gep
  %ptr_i8 = inttoptr i64 %ptr to i8*
  %null_addr = getelementptr i8, i8* %ptr_i8, i64 %len
  store i8 0, i8* %null_addr
  ; Return ptr as i64 (CString's canonical form)
  ret i64 %ptr
```

### Backing Type Determination

The backing type is the **original type before the first meld cast**:

```brief
let s: String = "hello";
let cs: CString = s as CString;     // Backed by String (original type)
// cs chimera uses String's layout. CString projections derive from String fields.

let s2: String = cs as String;      // Still backed by String (chain preserves origin)
// Zero-cost re-lens: same layout, same fields, just changing the type annotation.

let cs2: CString = some_c_function(); // This IS a CString — no meld cast
let s3: String = cs2 as String;       // Backed by CString (original was CString)
// s3 chimera uses CString's layout. String's Len derives from strlen#.
```

The rule: **the backing type is determined by the first value that is cast across a meld boundary**. If the value originated as a `String`, the chimera is backed by String. If it originated as a `CString`, the chimera is backed by CString. This is a one-way determination at the moment of the first cast and never changes for the lifetime of that value.

---

## 5. Boundary-Driven Decay

### Rules

A chimera **decays (materializes)** to the canonical layout of the target type in exactly three situations:

**Rule 1 — Struct field write:** When a chimera is stored into a struct field, it decays to the field's declared type.

```brief
struct Packet {
    name: CString;
};

let s: String = "hello";
let p: Packet = Packet { name: s as CString };  // ← Decay: chimera → CString
// p.name is now a canonical CString (8 bytes, ptr only)
// The String *backing* is gone — p.name does NOT have a len field
```

Rationale: Structs require strictly static memory footprints. If a struct field could hold a dynamic chimera, LLVM's SROA and GEP calculations would break entirely.

**Rule 2 — FFI call argument:** When a chimera is passed to a `frgn` function, it decays to the parameter's declared type.

```brief
frgn puts(s: CString) -> Int from "c";
let s: String = "hello";
puts(s as CString);  // ← Decay: chimera → CString ABI
// Ensures null terminator at ptr[len], passes i8* as expected by C
```

**Rule 3 — Function return type:** When a chimera is returned from a `defn` or `txn`, it decays to the declared return type.

```brief
defn get_path() -> CString {
    let s: String = read_file("path.txt")?;
    term s as CString;  // ← Decay: chimera → CString
};
```

### No Decay (Chimera Remains)

A chimera does NOT decay in the following situations:

- **Internal function parameter:** If the function is internal to Brief and the compiler can analyze liveness across the call graph, the parameter remains a chimera.
- **Temporary expression:** `(cs as String) :> Size` — the `cs as String` is a temporary lens switch, not a storage change. The value is still backed by whatever it was backed by.
- **Assignment to same-type variable:** `let cs2 = cs;` where both are `CString` and `cs` is a chimera — no decay, chimera identity is preserved.

### Implementation of Decay

At each boundary site, the compiler emits materialization code:

```
fn emit_decay(value, target_type, state_ptr):
  if not is_chimera(value):
    return  // nothing to do

  let backing_type = get_backing_type(value)
  if backing_type == target_type:
    return  // already canonical, no-op

  // Materialize chimera to target_type's canonical layout
  for each field in target_type.fields():
    emit_store(state_ptr, field, derive_field(value, field))

  // Mark the chimera as decayed (or emit a new non-chimera value)
```

For the common case (`CString`-typed chimera backed by `String`, returned as `CString`), the decay is trivially: extract `ptr` from the String backing and ensure null termination. No copy of character data.

---

## 6. Phase Breakdown

### Phase 0 — Foundation (2-3 days)

**Goal:** Parser + AST + TypeUniverse can register `meld` declarations. The typechecker allows `as` casts across meld boundaries. No codegen changes yet.

**Files to modify:**

| File | Change | Details |
|------|--------|---------|
| `src/ast.rs` | Add `TopLevel::Meld` variant | `Meld { name_a, name_b, routes: Vec<MeldRouteDef> }`. `MeldRouteDef { from_type, from_field, to_type, to_field, expr }`. |
| `src/ast.rs` | Add `MeldRoute` to `ResolvedType` (or equivalent type metadata) | Stores the compiled route for codegen use. |
| `src/parser.rs` | Parse `meld` declarations | Token: `meld`. Parse `meld A <:> B;` and `meld A <:> B { ... };`. The body contains `Type.Field = Expr;` statements. |
| `src/lexer.rs` | Add `Meld` token | New keyword `meld`. Also `<:` is already a token (used for type derivation). |
| `src/type_universe.rs` | Accept `TopLevel::Meld` in `build()` | Create `MeldRelation { other, routes }` in `ResolvedType`. Call `infer_meld()` during build. |
| `src/type_universe.rs` | Implement `infer_meld()` | The inference algorithm from Section 3. Runs during TypeUniverse construction. |
| `src/type_universe.rs` | Add `compatible_with: Vec<MeldRelation>` to `ResolvedType` | Stores all meld relationships for this type. |
| `src/typechecker.rs` | Extend `is_cast_valid()` | Add arm: if `src` and `dst` are `Custom` names that have a `MeldRelation` in TypeUniverse, cast is valid. Do NOT run the existing scalar-only whitelist for meld-backed casts. |
| `src/typechecker.rs` | Allow `Custom` → `Custom` cast | Currently, casting between two user-defined types falls through the whitelist to an error. Meld-backed casts must bypass this. |
| `src/analysis/mod.rs` | Add `compute_projection_usage()` | New analysis pass: for each state field, collect which `ProjectionTarget::UserDefined(name)` calls are made on it. Track the Brief type of the expression at each call site. Output: `HashMap<String, HashSet<(String, String)>>` mapping state field name → set of (type_name, projection_name) pairs. |

**Tests to add:**
- Parser: `meld Float <:> CFloat;` parses to correct AST
- Parser: `meld String <:> CString { String.Len = CString :> Size; };` parses with route
- Parser: `meld Float <:> CFloat {` with no closing brace produces parse error
- TypeUniverse: `infer_meld()` auto-derives identity routes for matching `@/` ranges
- TypeUniverse: `infer_meld()` errors on types with different `Bytes` and no explicit routes
- TypeUniverse: `infer_meld()` errors on missing field derivations
- TypeUniverse: `ResolvedType.compatible_with` is populated correctly
- Typechecker: `x as CFloat` where `meld Float <:> CFloat` passes typecheck
- Typechecker: `x as CString` with no meld declaration produces type error (not a valid cast)
- Analysis: `compute_projection_usage()` correctly tracks which projections are used per field

**Verification:** `cargo test --lib` — all existing tests pass (zero regressions). New tests cover all above cases.

### Phase 1 — Adaptive Layout Engine (5-7 days)

**Goal:** Fields that are never accessed through any lens are eliminated from `%State`. Fields with deferred projections (like `strlen#`) get cache slots when accessed in loops.

**Files to modify:**

| File | Change | Details |
|------|--------|---------|
| `src/analysis/transition_graph.rs` | Integrate `compute_projection_usage()` | Run it as part of `ReactorTransitionGraph::build()`. Store results in a new field `projection_usage: HashMap<String, HashSet<(String, String)>>`. |
| `src/analysis/mod.rs` | Add `LiveField.mode: FieldMode` enum | `FieldMode::Always | FieldMode::LazyCached { cache_index: usize } | FieldMode::Never`. The `TransitionGraph` or a new adapter pass assigns a mode to each state field based on projection usage. |
| `src/backend/llvm/mod.rs` | Store `projection_usage` and `field_modes` | Add fields to `LlvmBackend`. Populate after analysis passes run. |
| `src/backend/llvm/emit_toplevel.rs` | `declare_state_type()` omits `Never` fields | When emitting `%State = type { ... }`, skip fields with `FieldMode::Never`. The field indices shift — the `field_index_map` must be rebuilt. |
| `src/backend/llvm/mod.rs` | `build_field_index()` rebuilds after mode assignment | Run AFTER field modes are computed, NOT during `generate()`. This changes the current two-pass structure: build_field_index runs once early, then again after mode assignment. |
| `src/backend/llvm/emit_toplevel.rs` | `declare_state_type()` appends cache fields | For each `LazyCached` field, add `{ cache_i64: i64, cache_valid: i8 }` AFTER the regular fields. |
| `src/backend/llvm/emit_toplevel.rs` | `emit_inline_init_stores()` handles cache fields | Initialize cache fields to `{ 0, 0 }` (no value, not valid). |
| `src/backend/llvm/mod.rs` | Update `field_index_map` and `field_types` | After rebuilding with omitted fields and appended cache slots, ensure all downstream code (FieldAccess, state loads/stores) uses the correct indices. |
| `src/backend/llvm/emit_toplevel.rs` | `emit_precondition_check` and `emit_guard_check` use new indices | If a condition references a chimera field, the GEP index must account for any omitted earlier fields. |

**Cache slot assignment algorithm:**

```
for each state field f:
  let usage = projection_usage[f]
  if usage.is_empty():
    field_modes[f] = Never
  elif has_evidence_of_hot_dual(usage):
    field_modes[f] = LazyCached { cache_index: next_cache }
    increment next_cache
  else:
    field_modes[f] = Always
```

`has_evidence_of_hot_dual()` returns true when:
- The same state field is accessed through both lenses (e.g., both `(String, "Size")` and `(CString, "Size")` usage entries exist)
- AND at least one access occurs inside a loop body (detected by loop nest analysis)

**Tests to add:**
- State field with zero accesses is eliminated from `%State` (check LLVM IR output)
- State field with only one lens active stays in `Always` mode (no cache)
- State field with both lenses active in a loop gets `LazyCached` with valid cache slots in IR
- Cache fields are initialized to zero in init stores
- Field index remapping is correct — eliminated early fields don't break subsequent GEP offsets
- `cargo test --lib` — all existing tests pass (zero regressions)

**Warning:** This phase is the most invasive. The `%State` struct layout changes affect every field load and store in the generated IR. Every test that checks LLVM IR output must be updated if field elimination renumbers indices.

### Phase 2 — Chimera Projection Dispatch (3-5 days)

**Goal:** Projections on a meld-backed value route through the correct lens, using cache slots when available. Mutations invalidate caches.

**Files to modify:**

| File | Change | Details |
|------|--------|---------|
| `src/backend/llvm/emit_expr.rs` | `try_projection_fast_path()`: new arm for meld-backed types | When `src_val.ty` is a `Type::Custom` that has a meld relation, AND the projection is being routed through the non-backing lens: check if a cache slot exists for this projection. If so, emit cache load. If not, emit the derivation from the backing type's fields. |
| `src/backend/llvm/emit_expr.rs` | Add `emit_meld_projection()` helper | Encapsulates the logic: look up the meld route, check cache, fall back to derivation. Called by the new `try_projection_fast_path` arm. |
| `src/backend/llvm/emit_expr.rs` | Cache load codegen | Emit: load cache_valid, branch to compute-or-cache, load/store as needed. (The actual LLVM IR pattern from Section 4 Hot Dual.) |
| `src/backend/llvm/emit_stmt.rs` | Cache invalidation on stores | After every store to a meld-backed state field, if the field has `LazyCached` mode, emit `store i8 0` to the associated `cache_valid` field. |
| `src/backend/llvm/emit_stmt.rs` | `adapt_to_i64()` handles chimera field types | If a chimera field is Float but stored as boxed i64, ensure correct boxing/unboxing. |
| `src/features/projection.rs` | `eval_user_projection_fast_path()`: meld-aware dispatch | Mirror the LLVM path in the interpreter. When a projection is on a chimera value, look up the meld route and evaluate the derivation expression instead of the original projection. |
| `src/features/projection.rs` | Interpreter cache | In the interpreter, chimera values carry their cache inline in the `Value` enum. Add a `Chimera { backing: Box<Value>, meld_name: String, cache: HashMap<String, Value> }` variant or extend `Value::LazyView`. |

**`emit_meld_projection()` logic:**

```rust
fn emit_meld_projection(&mut self, out: &mut String, src_val: &TypedRegister,
    meld: &MeldRelation, proj_name: &str, indent: &str, dst: &str)
    -> TypedRegister
{
    // 1. Is there a cache slot for this projection?
    if let Some(cache_idx) = self.get_cache_index(src_val, meld, proj_name) {
        // 2. Cache-aware path: check valid, load or compute
        let valid_ty = self.field_types[cache_idx + 1].clone(); // 0=cache, 1=valid
        // emit cache check + branch + return
    } else {
        // 3. No cache: derive from backing type's field directly
        let route = meld.find_route(proj_name).unwrap();
        let backing_field = self.emit_expr_field(out, &route.from_field, indent);
        // result is TypedRegister with the value
    }
}
```

**Tests to add:**
- Meld-backed CString: `cs :> Size` loads cache when available, calls strlen only on first call
- Meld-backed CString: after `(cs as String) ++ "x"`, cache is invalidated and strlen is called again
- Non-meld type: projection codegen is unchanged (zero regression)
- Interpreter: meld-backed projections evaluate correctly
- Interpreter: cache invalidation on mutation
- `cargo test --lib` — all existing tests pass (zero regressions)

### Phase 3 — Boundary-Driven Decay (2-3 days)

**Goal:** Chimeras materialize at struct stores and FFI boundaries.

**Files to modify:**

| File | Change | Details |
|------|--------|---------|
| `src/backend/llvm/emit_expr.rs` | `Expr::FieldAccess` write path: detect chimera value being stored | When a `FieldAccess` assignee receives a chimera value, call `emit_decay()` before the store. |
| `src/backend/llvm/emit_expr.rs` | Add `emit_decay()` function | Emits code to extract the canonical fields for the target type from the chimera backing. For `CString` decay from `String` backing: extract ptr, ensure null terminator. For `String` decay from `CString` backing: extract ptr, call strlen for len. |
| `src/backend/llvm/emit_toplevel.rs` | In `term` emission: decay chimera to return type | Before the `ret` instruction, if the return value is a chimera, call `emit_decay()` to the declared return type. |
| `src/backend/llvm/emit_expr.rs` | At `frgn` call sites: decay chimera arguments | Before emitting a foreign call, check each argument for chimera status. If chimera, emit decay to the parameter's declared type. |
| `src/backend/llvm/mod.rs` | Track chimera status in `TypedRegister` | Add `is_chimera: bool` and `backing_type: Option<String>` fields. These are set during projection dispatch and checked during boundary emission. |
| `src/interpreter.rs` | Decay at `term` in interpreter | On `Statement::Term(expr)`, if the value is a chimera, materialize it to the function's declared return type. |
| `src/interpreter.rs` | Decay at struct field assignment | On assignment to a struct field, if the value is a chimera, materialize to the field's type. |
| `src/interpreter.rs` | Decay at FFI call | On `frgn` call, decay chimera arguments to their parameter types. |

**`emit_decay()` algorithm:**

```rust
fn emit_decay(&mut self, out: &mut String, value: &TypedRegister,
    target_ty: &Type, state_ptr: &str, indent: &str) -> TypedRegister
{
    if !value.is_chimera {
        return value.clone();  // not a chimera, no decay needed
    }
    if value.backing_ty == *target_ty {
        return value.clone();  // already canonical, no-op
    }

    // Materialize: derive each field of target_ty from the chimera backing
    let backing_ty = value.backing_ty.as_ref().unwrap();
    let meld = self.type_universe.find_meld(backing_ty, target_ty).unwrap();

    for each field in target_ty.fields():
        let route = meld.find_route_inverse(field.name);
        emit load/derive for this field from the backing type

    return TypedRegister { name: result, ty: target_ty, is_chimera: false };
}
```

**Tests to add:**
- Struct store: chimera decays to field's type (check `%State` has canonical layout for the field, not the chimera)
- FFI call: chimera argument decays to C ABI (null terminator written at ptr[len])
- Return: chimera returned as CString has only ptr (8 bytes) in the return register
- No decay for internal-to-internal function parameters
- `cargo test --lib` — all existing tests pass (zero regressions)

### Phase 4 — Error Messages and Diagnostics (1-2 days)

**Goal:** Clear, actionable error messages for meld declaration errors and runtime diagnostics for meld behavior.

**Error messages to implement:**

**E001 — No route for field:**

```
error[E001]: ambiguous meld — `String` has no route to `Len` from `CString`
  --> file.bv:5:1
   |
 5 | meld String <:> CString;
   | ^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = `CString` occupies 8 bytes (ptr only, @/0..63)
   = `String` occupies 16 bytes (ptr + len, @/0..63 + @/64..127)
   = no field or projection in `CString` maps to `Len @/64..127`
   |
help: add a projection router for the non-trivial derivation:
  |
  | meld String <:> CString {
  |     String.Len = CString :> Size;
  | };
  |
```

**E002 — Circular meld:**

```
error[E002]: circular meld — `A <:> B` and `B <:> C` would create cycle `A → B → C`
  --> file.bv:10:1
   |
10 | meld C <:> A;
   | ^^^^^^^^^^^^^
   |
   = TypeUniverse: `A <:> B` (line 3), `B <:> C` (line 7), would add `C <:> A`
   = this would create a non-tree structure in the meld graph
   |
help: remove one of the meld declarations, or use explicit `as` casts instead
```

**E003 — Size mismatch without router:**

```
error[E003]: size mismatch in meld — `String` (16 bytes) vs `CString` (8 bytes)
  --> file.bv:5:1
   |
 5 | meld String <:> CString;
   | ^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = types with different byte sizes require an explicit projection router
   |
help: add a router specifying how the larger type derives from the smaller:
  |
  | meld String <:> CString {
  |     String.Len = CString :> Size;
  | };
  |
```

**E004 — Field type mismatch:**

```
error[E004]: field type mismatch — `A.Ptr` is `Ptr<Byte>` but `B.Ptr` is `Ptr<Char>`
  --> file.bv:5:1
   |
 5 | meld A <:> B;
   | ^^^^^^^^^^^^^
   |
   = both fields occupy @/0..63 but have incompatible type annotations
   |
help: add an explicit route to override the default identity mapping:
  |
  | meld A <:> B {
  |     A.Ptr = crack<Ptr<Byte>>(B.Ptr);
  | };
  |
```

**E005 — Invalid route expression:**

```
error[E005]: invalid route expression — `String.Len = CString :> NonExistent`
  --> file.bv:6:22
   |
 6 |     String.Len = CString :> NonExistent;
   |                      ^^^
   |
   = `CString` has no projection named `NonExistent`
   |
help: available projections on `CString`: `Size`, `Ptr`, `At`
```

**W001 — Dead cache slot:**

```
warning[W001]: dead cache slot — `strlen` cache on `cs` is never invalidated
  --> file.bv:12:17
   |
12 |     let len = cs :> Size;
   |                 ^^^^^^^^
   |
   = the cache slot for `cs :> Size` (CString lens) is allocated but never written
   = this wastes 8 bytes in %State
   |
help: if this value is never mutated, declare it `let` instead of letting it be mutable
```

**W002 — Unnecessary meld:**

```
warning[W002]: unnecessary meld — `Float` and `CFloat` have identical @/ ranges
  --> file.bv:5:1
   |
 5 | meld Float <:> CFloat;
   | ^^^^^^^^^^^^^^^^^^^^^^
   |
   = both types have `Bytes = 8`, `Alignment = 8`, and identical field layouts
   = the meld router is fully inferred — no explicit routes needed
   |
help: remove the empty router body for clarity:
  |
  | meld Float <:> CFloat;
  |
```

**`--layout` diagnostic flag:**

Add a command-line flag `--layout` that prints the chosen layout for every state field:

```
$ brief-compiler rbv program.rbv --layout
Layout for `s` (String):
  mode: Always (16 bytes, backing type: String)
  fields: ptr @/0..63, len @/64..127

Layout for `cs` (CString, chimera backed by String):
  mode: LazyCached (24 bytes, backing type: String)
  fields: ptr @/0..63, len @/64..127
  cache: strlencache @ state[2], cache_valid @ state[3]

Layout for `path` (CString):
  mode: Always (8 bytes, canonical)
  fields: ptr @/0..63
```

### Phase 5 — Proof Engine Integration (2-3 days)

**Goal:** `?#` validates meld declarations at compile time.

**Files to modify:**

| File | Change | Details |
|------|--------|---------|
| `src/proof_engine.rs` | Add `check_meld_validity()` | Structural proof that the routed projections in a meld declaration are sound. Called during `verify_program()`. |
| `src/proof_engine.rs` | Verify `@/` range non-overlap | For each route, check that the source field's `@/` range exists within the source type. |
| `src/proof_engine.rs` | Verify route type compatibility | For each `String.Len = CString :> Size` route, verify that `Size` returns a type compatible with `Len` (both are Int/UInt). |
| `src/proof_engine.rs` | Verify field completeness | Check that every field in both types has at least one route (either inferred or explicit). |
| `src/proof_engine.rs` | Cycle detection in meld graph | Use the existing `CallGraph` infrastructure to detect meld cycles. |
| Error infrastructure | Wire E001-E005 errors to proof engine output | Each proof error maps to a specific error code. |
| `src/proof_engine.rs` | Add Kani harness for `check_meld_validity()` | Pure match dispatch on `MeldRelation` variants. Must follow Kani fast-group rules (no allocation, no formatting). |

**Proof checks:**

```
check_meld_validity(A, B, routes):
  for each route:
    verify source field/projection exists in source type
    verify target field/projection exists in target type
    verify source @/ range is within source type's byte width
    verify route expression type-checks against target field type
    verify route expression has no side effects (pure projections only)

  for each field in A:
    verify at least one route derives it from B
  for each field in B:
    verify at least one route derives it from A

  verify no cycle in meld dependency graph
```

**Tests to add:**
- Valid meld passes `check_meld_validity()`
- Invalid meld (missing field route) produces E001
- Circular meld produces E002
- Size mismatch meld without routes produces E003
- Field type mismatch produces E004
- Kani harness: pure match dispatch on 6 route variants, no allocation

---

## 7. Generics (Deferred)

### Status: Deferred until Phase 3 is stable with 50+ passing tests.

### Design (for future implementation)

```brief
// Generic meld — structural equivalence applies for any T
meld List<T> <:> Vec<T>;
```

`List<T>` in Brief: `{ ptr: Ptr<T>, len: u64, cap: u64 }` (24 bytes).  
`Vec<T>` in C/Rust: `{ ptr: Ptr<T>, len: usize, cap: usize }` (24 bytes on 64-bit).

The structural equivalence proof for generics:

```
For any T:
  List<T>.Ptr = Vec<T>.Ptr    @/0..63   ✓ (both are Ptr<T>, same width)
  List<T>.Len = Vec<T>.Len    @/64..127 ✓ (both are u64, same width)
  List<T>.Cap = Vec<T>.Cap    @/128..191 ✓ (both are u64, same width)

Therefore: meld List<T> <:> Vec<T>; is valid for any T.
```

The proof engine checks that the layout of `List<T>` and `Vec<T>` is identical **regardless of T**. This is true because:
1. Pointer width is always 64 bits (on the target architecture)
2. `len` and `cap` are always `u64` (not dependent on T)
3. The element type T only affects what `ptr` points to, not the pointer itself

### Implementation approach (Phase 4+):

1. Add `type_params` to `TopLevel::Meld`
2. When instantiating a generic type that has a generic meld, concretize the meld for the specific type arguments
3. Cache the concretized meld in TypeUniverse for reuse

### NOT in scope for Phases 0-3:

- Generic meld declarations (`meld List<T> <:> Vec<T>;`)
- Automatic instantiation of generic melds
- Proof of structural equivalence for parameterized types

---

## 8. Performance Analysis

### Worst-Case Regression

A program that casts every value across a meld boundary and accesses both lenses in tight loops could see:

| Metric | Without meld | With meld (naive) | With meld (optimized) |
|--------|-------------|-------------------|----------------------|
| Memory per chimera | 16 bytes (String) + 8 bytes (CString) = 24 bytes | 16 bytes (chimera, union of fields) | 16 bytes (Short Path, no extra cache) |
| strlen calls | 1 per `:> Size` | 1 per `:> Size` | 1 total (then cached, Hot Dual) |
| Instructions for cast | strlen + malloc + memcpy (CString→String) | 0 (bitcast only) | 0 (bitcast only) |
| Mutations | Must update both copies | 1 store + 1 cache invalidate | 1 store + 1 cache invalidate |

**Result:** Meld is at worst a memory regression of 8-12 bytes per chimera value (cache slot + valid flag) in the Hot Dual case. It is never a runtime correctness regression — cache invalidation ensures consistency.

### When Meld Is Strictly Better

| Scenario | Without meld | With meld | Win |
|----------|-------------|-----------|-----|
| Read-only CString→String | strlen + malloc + memcpy | No cost until `:> Size` | Deferred O(N) |
| Read-only String→CString | Null termination check + copy | Identity (same ptr field) | Zero-cost |
| Hot loop with both lenses | Manual dual maintenance | Compiler-managed cache | Zero-cost reads, 1 store writes |
| FFI boundary | Conversion boilerplate | Automatic decay | No user code |

### Compile-Time Cost

| Phase | Pass | Cost |
|-------|------|------|
| TypeUniverse build | `infer_meld()` | O(fields_A × fields_B) — negligible |
| Analysis | `compute_projection_usage()` | O(projections × nesting depth) — one pass |
| Codegen | Cache-aware projections | Emits up to 8 LLVM instructions more per projection (cache check branch) |
| Codegen | Decay emission | O(target_fields) — one-time per boundary |

Total compile-time impact: <3% for typical programs.

---

## 9. Relationship to Existing Plans

### Bits Thesis Plan (`docs/plans/2026-06-20-bits-thesis.md`)

| Bits Thesis concept | Meld feature relationship |
|---------------------|--------------------------|
| Types are lenses over Bits | Meld is the language mechanism to declare lens compatibility |
| Operator desugaring → projections | Meld routes reuse the same projection infrastructure |
| Lazy CString interop (Section 13.9) | Meld automates the pattern — `strlen` is deferred in the router |
| Fast-path registry (Phase 3.5) | Meld-aware projections are new arms in `try_projection_fast_path` |
| `@/` bit-range syntax | Meld inference uses `@/` ranges as the primary matching heuristic |
| Phase 4 (Bit-precision integration) | Meld inference DEPENDS on `@/` resolution being complete |

### LinkedIn Distillation (`.opencode/plans/2026-06-22-linkedin-discussion-distillation.md`)

| Proposal | Meld feature alignment |
|----------|----------------------|
| A — Adaptive Layout | **Adopted as Phase 1** — the adaptive field elimination is the core mechanism |
| B — Lazy-to-Eager Promotion | **Adopted as Hot Dual path** — deferred projections get cache slots based on loop evidence |
| C — Lens-Composition Symbolic | Not required for meld (independent optimization) |
| D — Integer Semantics | Not required for meld (independent feature) |
| E — Safe Type Introspection | **Replaced** by meld — `crack` is no longer needed as a user-facing feature because meld provides the safe version |
| F — Postcondition Instrumentation | Not required for meld (independent feature) |

### The `crack` Operation

`crack` as originally designed in the LinkedIn discussion is **not needed as a user-facing feature**. The meld system replaces it:

- **Safe reinterpretation:** `x as CString` (with a meld declaration) is the safe, ergonomic way to re-lens a value
- **Unsafe reinterpretation:** If no meld exists, the compiler errors. The developer must either declare a meld or restructure the code
- **The `crack!` escape hatch** is not part of this design — all meld operations go through `as` casts and declared melds

If safe low-level bit reinterpretation is needed in the future, it should be added as an intrinsic (`__reinterpret#(val, type)`) that the proof engine can reason about, **not** as a user-facing keyword.

---

## 10. Anti-Patterns (NEVER DO)

These are explicit prohibitions. If any code review catches one, reject the change.

### ❌ NEVER add runtime type tags to the chimera

The chimera must NOT carry a runtime tag indicating "which lens is currently active." The lens is determined statically at each projection site by `TypedRegister.ty`. A runtime tag would add overhead to every projection, contradicting the zero-cost goal.

### ❌ NEVER allow implicit meld casts (coercions)

All meld lens switches must be explicit: `x as CString`. Never allow `let cs: CString = string_var;` without the `as` keyword. Implicit coercions violate the principle of least surprise and make the code harder to reason about.

### ❌ NEVER implement dynamic memory path switching

The three memory paths (Short, Hot Dual, Unpack) are selected statically at compile time. Do NOT add runtime transitions between paths. Wrong predictions mean wasted memory (unused cache slot) but never incorrectness.

### ❌ NEVER weaken existing optimization paths for meld

Every meld addition to `try_projection_fast_path` must be an **additional match arm**. Do not modify existing arms. The `_ => return None;` fallthrough must remain unchanged — non-meld types must continue to work exactly as before.

### ❌ NEVER implement `crack<T>` as a user-facing keyword

If safe low-level reinterpretation is needed in the future, add it as an intrinsic (`__reinterpret#`), not as a keyword. The meld system is the correct user-facing abstraction for type reinterpretation.

### ❌ NEVER add generic melds before Phase 3 is stable

`meld List<T> <:> Vec<T>;` must wait until Phases 0-3 have 50+ passing tests and zero known bugs. Generic melds add exponential complexity to the proof engine and can wait.

### ❌ NEVER add implicit transitive melds

`meld A <:> B` and `meld B <:> C` does NOT imply `A <:> C`. Each meld must be explicitly declared. The compiler may issue a warning ("note: A and C are connected through B — consider declaring meld A <:> C if transitive compatibility is intended") but must NOT auto-derive it.

### ❌ NEVER leave stubs or `todo!()` in committed meld code

Every variant, every match arm, every code path in the meld feature must be fully implemented. If a path is unreachable, document why. If a path should exist but hasn't been implemented yet, the PR is not ready to merge.

---

## 11. Implementation Mandate

1. **No stubs.** Every match arm must have a real implementation. If a backend doesn't support a particular projection path, implement it or document why the path is unreachable.

2. **Test-first.** Every phase must have at least as many lines of new tests as new implementation code. At minimum: 1 parser test, 1 typechecker test, 1 interpreter test, 1 LLVM codegen test per new AST node.

3. **Existing tests must not regress.** `cargo test --lib` must pass after every phase. If a meld change modifies `%State` layout, existing LLVM IR tests that assert specific GEP indices must be updated — but the *behavior* (the value computed) must not change.

4. **Doc-per-cycle.** Each phase ships its architecture doc in the same commit as the code change. The meld feature doc should live at `docs/architecture/features/meld.md` once Phase 0 is complete.

5. **Review order:** Phase 0 → Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5. Do not start a phase until the previous one is fully merged with all tests passing.

---

*End of plan.*
