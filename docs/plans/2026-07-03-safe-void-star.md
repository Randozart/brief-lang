# Safe `void*` — A Plan for Universal, Verified Pointers in Briev

**Date:** 2026-07-03  
**Status:** Plan  
**Author:** Design discussion between randozart and OpenCode

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Core Design](#2-core-design)
3. [Design Decisions](#3-design-decisions)
4. [What This Replaces or Deprecates](#4-what-this-replaces-or-deprecates)
5. [Phase 1 — Layout-Constrained Universal Pointer](#5-phase-1--layout-constrained-universal-pointer)
6. [Phase 2 — Layout-Compatible Casts](#6-phase-2--layout-compatible-casts)
7. [Phase 3 — Generic Algorithms Over Layout Constraints](#7-phase-3--generic-algorithms-over-layout-constraints)
8. [Phase 4 — Opaque Handles via Constrained Pointers](#8-phase-4--opaque-handles-via-constrained-pointers)
9. [Phase 5 — Function Pointers and Dynamic Dispatch](#9-phase-5--function-pointers-and-dynamic-dispatch)
10. [Phase 6 — Extract-Operate-Repack for `<:>` Types](#10-phase-6--extract-operate-repack-for--types)
11. [Appendix: Flat Control Flow During Implementation](#11-appendix-flat-control-flow-during-implementation)

---

## 1. Executive Summary

**The question:** Can Briev have a pointer system as powerful as C's `void*` while being provably safe?

**The answer:** Yes — and the architecture is already 60% built. The Bits Thesis provides the foundation. What's missing is a structured bridge between "fully typed" pointers and "fully untyped" pointers, expressed as spatial layout constraints rather than nominal type abandonment.

**The core thesis:** The `void*` in C is not powerful because it is untyped. It is powerful because it escapes *nominal* restrictions. Briev already has this escape mechanism: `Bits` with layout constraints. The plan makes that escape safe, explicit, and architecturally complete.

**Key insight from the Bits Thesis:** A pointer is `Bits @/0..63` (an address) with a projection lens over the pointee. `Ptr<Int>` = address + Int lens. `Ptr<Bits @/0..63>` = address + raw layout lens (safe `void*`). The only "magic" is crossing the LLVM `inttoptr` barrier for `load#`/`store#` — everything else falls out of Bits naturally.

---

## 2. Core Design

### 2.1 The Pointer Type Hierarchy

```
Ptr              → Ptr<Bits @/0..63>               (bare = 8-byte pointee default)
Ptr<>            → same as Ptr
Ptr<T>           → typed pointer (pointee is T's layout)
Ptr8             → Ptr<Bits @/0..7>                 (1-byte pointee)
Ptr16            → Ptr<Bits @/0..15>                (2-byte pointee)
Ptr32            → Ptr<Bits @/0..31>                (4-byte pointee)
Ptr64            → Ptr<Bits @/0..63>                (8-byte pointee)
Ptr128           → Ptr<Bits @/0..127>               (16-byte pointee)
Ptr256           → Ptr<Bits @/0..255>               (32-byte pointee)
Ptr<Bits @/0..N> → layout-constrained pointer        (explicit pointee width)
```

The pointer value (address) is always target-pointer-width. The number suffix refers to the **pointee bit width**, following the existing `Int8/16/32`, `UInt8/16/32`, `Float64` convention.

### 2.2 Operations by Pointee Type

| Pointee lens | Allowed operations |
|-------------|-------------------|
| Nominal (`Int`, `Float`, struct) | Dereference, arithmetic, field access, projections |
| `Bits @/N` (spatial) | memcpy, memcmp, memset, hash, swap, copy, address arithmetic, volatile load/store |
| Generic (`T: bytes == S`) | Whatever the constraints allow |

### 2.3 Casting Rules

```briev
// Layout-compatible: compiler checks bytes + alignment match
let f: Ptr<Float> = addr as Ptr<Float>;
let i: Ptr<Int32> = f as Ptr<Int32>;       // ✅ Float.bytes == Int32.bytes

// Re-lens through Bits (explicit intermediary)
let raw: Ptr<Bits @/0..31> = f as Ptr<Bits @/0..31>;  // strip lens
let i: Ptr<Int32> = raw as Ptr<Int32>;                  // apply new lens

// Meld (explicit routes, different layouts)
meld A <:> B { Ptr -> B.ptr; Size -> B.len; };

// No reinterpret — if you can't prove it at compile time, don't cast it
```

---

## 3. Design Decisions

### 3.1 `Ptr` is a Normal Type, Not an Intrinsic

`Ptr` does NOT get the `#` suffix. The `#` stays on the LLVM airlock intrinsics (`load#`, `store#`, `volatile_load#`). `Ptr` is fully derivable from `Bits @/0..63` with dereference projections — consistent with the Bits Thesis. The compiler recognizes the shape `Bits @/0..63 with load#/store# projections` and gives it standard pointer codegen.

### 3.2 `PtrN` Named Width — Target-Independent

`Ptr64` means "pointer to 8 bytes" on every target. The number suffix is the pointee bit width, not the address width. Target-pointer-width is an LLVM implementation detail.

This matches the existing convention: `Int` is always 64-bit regardless of target, `Int32` is always 32-bit, `Ptr` defaults to 64-bit pointee, `Ptr32` is always 4-byte pointee.

### 3.3 No `reinterpret` Escape Hatch

Every "I know better than the compiler" case reduces to: "these bits have a different meaning than the nominal type suggests." But layouts ARE the type. If layouts match, `as` works. If they don't, `melds` declare the relationship permanently.

If you can't write a `meld` because layouts truly don't match at the bit level, then `reinterpret` would be lying. The honest thing is explicit byte shuffling (`load32#`, shift, `store32#`).

### 3.4 Target-Independent Pointee Width

`Ptr` defaults to `Bits @/0..63` (8-byte pointee) on all targets. This is consistent with `Int` always being 64-bit. The address value itself is always target-address-width under the hood — that's LLVM's job, not the user's concern.

### 3.5 Layout-Compatibility Check on `as`

`Ptr<A> as Ptr<B>` is valid when the compiler can verify:
1. `bytes(A) == bytes(B)` — total width match
2. `alignment(A) >= alignment(B)` — destination alignment not stricter than source

No `meld` declaration required for simple layout compatibility. The type system already has all layout information from the TypeUniverse.

---

## 4. What This Replaces or Deprecates

### 4.1 Can Be Fully Removed

| Pattern | Reason | Scope |
|---------|--------|-------|
| `Ptr<Byte>` as void* workaround | Replaced by `Ptr<Bits @/8>` | ~10 example sites + `from-bits.bv` |
| `Ptr<Char>` type pun | Replaced by `Ptr<Bits @/32>` | 1 site in `pointer-trickery.bv` |
| `inttoptr#` / `ptrtoint#` intrinsics | Ptr is native now, no boxing needed | ~300 lines across LLVM backend |
| `Value::Ptr(u64)` interpreter variant | Bit-range projection replaces it | `src/interpreter.rs` |
| `is_ptr_ty()` helper | No special-casing needed for Ptr | `src/backend/llvm/helpers.rs` |
| `read_byte()` shift+mask in std/ptr.bv | Direct sized-pointer access | `lib/std/ptr.bv` |
| `copy()` count*8 chunk arithmetic | `Ptr<Bits @/N>` = natural block copy | `lib/std/ptr.bv` |

### 4.2 Can Be Deprecated / Simplified

| Pattern | Current | New |
|---------|---------|-----|
| `Type::Applied("Ptr", vec![T])` in 133 match sites | T is purely documentation | `Bits @/N` with real bit-width semantics |
| `Int as Ptr<Int>` round-trip in examples | Cast through Int | Direct `Ptr` literal with `@/N` width |
| `ptrtoint`/`inttoptr` for list/string headers | Boxing/unboxing handles | Native struct pointers |
| `data .#Ptr .#Ptr` double projection | Double escape to raw Int | Single sized-pointer projection |
| `volatile_load#` Ptr<T> type extraction | Must extract T from Ptr<T> at compile time | Ptr carries width natively |
| `atomic.bv` with `Ptr<Int>` only | Comment: "cast through Ptr<Int>" | Generic over `Ptr<Bits @/N>` |

### 4.3 Keep As-Is

| Pattern | Reason |
|---------|--------|
| `volatile_load#`/`volatile_store#` | MMIO semantics orthogonal to pointer typing |
| `inttoptr` in BILD inline asm | Inherently low-level, keep explicit |
| `.#Ptr` / `.#Ptr!` projections | Field extraction from compound types still useful |
| `address<T>(p) -> Int` | Raw address escape for FFI is legitimate |
| `@llvm.memcpy` for list reallocation | Internal data movement, not language feature |

---

## 5. Phase 1 — Layout-Constrained Universal Pointer

**Goal:** Create `Type::LayoutPtr(LayoutConstraint)` — a pointer parameterized by spatial layout (bytes + alignment), not nominal type. This is the safe `void*`.

### 5.1 AST Changes

```rust
/// Layout constraint for a universal pointer — safe void* equivalent.
pub struct LayoutConstraint {
    pub bytes: u64,
    pub alignment: u64,
}

// New Type variant (additive, keeps existing variants):
pub enum Type {
    // ... existing variants ...
    LayoutPtr(LayoutConstraint),  // NEW
}
```

Adding a dedicated variant avoids touching the 133 `Type::Applied("Ptr", _)` match sites. Existing `Ptr<T>` continues to work as before.

### 5.2 Parser Changes

New sugar rules in `src/parser.rs`:

```
"Ptr"          → Type::LayoutPtr(LayoutConstraint { bytes: 8, alignment: 8 })
"Ptr8"         → Type::LayoutPtr(LayoutConstraint { bytes: 1, alignment: 1 })
"Ptr16"        → Type::LayoutPtr(LayoutConstraint { bytes: 2, alignment: 2 })
"Ptr32"        → Type::LayoutPtr(LayoutConstraint { bytes: 4, alignment: 4 })
"Ptr64"        → Type::LayoutPtr(LayoutConstraint { bytes: 8, alignment: 8 })
"Ptr128"       → Type::LayoutPtr(LayoutConstraint { bytes: 16, alignment: 16 })
"Ptr256"       → Type::LayoutPtr(LayoutConstraint { bytes: 32, alignment: 32 })
"Ptr<>"        → same as "Ptr"
"Ptr<Bits @/N>" → compute LayoutConstraint from range
```

These are parse-time desugarings. No TypeUniverse lookup needed during parsing.

### 5.3 Allowed Operations

Operations permitted on `LayoutPtr` are purely spatial — valid for ALL types matching the layout:

| Operation | Example | LLVM emission |
|-----------|---------|--------------|
| memcpy | `__memcpy#(dst, src, bytes)` | `call @llvm.memcpy` |
| memcmp | `__memcmp#(a, b, bytes)` | `call @llvm.memcmp` |
| memset | `__memset#(ptr, val, bytes)` | `call @llvm.memset` |
| Raw equality | `ptr_a == ptr_b` | `icmp eq i64` |
| Hash | `__hash#(ptr, bytes)` | `call @llvm.bitwisehash` or inline |
| Copy / swap | standard variable ops | `load` + `store` |
| Address arithmetic | `ptr + 4`, `ptr & mask` | standard integer ops |
| Volatile load/store | `volatile_load#(ptr, width)` | `load volatile iN` |

### 5.4 Forbidden Operations

Operations requiring semantic interpretation are rejected by the typechecker:

| Operation | Why forbidden |
|-----------|--------------|
| `+`, `-`, `*`, `/` on pointee | No semantic meaning — bits could be anything |
| Field access (`.field`) | Field arrangement unknown |
| Bracket index (`ptr[i]`) | Element size unknown — cannot stride |

### 5.5 All 133 `Type::Applied("Ptr", ...)` Match Sites

Each match site must be examined:

| Category | Action |
|----------|--------|
| `name == "Ptr"` inner type used for type-checking (documentation) | Add `Type::LayoutPtr` parallel arm |
| `name == "Ptr"` inner type used for codegen (byte size) | Query `LayoutConstraint.bytes` instead |
| `name == "Ptr"` inner type used for TBAA | Use `LayoutConstraint` as alias class |
| `name == "Ptr"` but only checks `name == "Ptr"` (ignores inner) | Add `Type::LayoutPtr` arm, same logic |

### 5.6 Interpreter Changes

`Value::Ptr(u64)` stays as the runtime representation. The `TypedRegister.ty` carries `Type::LayoutPtr(...)` to gate operations in the interpreter.

### 5.7 LLVM Backend Changes

Backend emits typed loads/stores of width `LayoutConstraint.bytes` at alignment `LayoutConstraint.alignment`:

```llvm
; Ptr64 (8-byte pointee) volatile load:
%val = load volatile i64, ptr %ptr, align 8

; Ptr8 (1-byte pointee) volatile load:
%val = load volatile i8, ptr %ptr, align 1
```

The `inttoptr`/`ptrtoint` round-trip for boxing is removed. Pointers are native `ptr` in LLVM IR where possible, `i64` where integer arithmetic is needed.

### 5.8 Tests

| Test | What it validates |
|------|-------------------|
| `test_layout_ptr_memcpy` | LayoutPtr copies N bytes via llvm.memcpy |
| `test_layout_ptr_as_typed_ptr` | Re-lensing LayoutPtr ←→ Ptr<T> zero-cost |
| `test_layout_ptr_forbidden_ops` | Arithmetic on pointee rejected |
| `test_ptr8_parse` | `Ptr8` parses to LayoutConstraint { bytes: 1 } |
| `test_ptr64_parse` | `Ptr64` parses to LayoutConstraint { bytes: 8 } |
| `test_ptr_bare_parse` | `Ptr` parses to LayoutConstraint { bytes: 8 } |
| `test_ptr_arithmetic` | Address arithmetic on LayoutPtr works |

---

## 6. Phase 2 — Layout-Compatible Casts

**Goal:** Allow `Ptr<A> as Ptr<B>` when the compiler can verify layout compatibility, without requiring a `meld` declaration for simple cases.

### 6.1 TypeChecker Changes

Extend `is_cast_valid()` in `src/typechecker.rs`:

```rust
// New rule: Ptr<A> → Ptr<B> if bytes and alignment are compatible
(Type::Applied(n_a, inner_a), Type::Applied(n_b, inner_b))
    if n_a == "Ptr" && n_b == "Ptr" =>
{
    let bytes_a = type_universe.byte_size(&inner_a)?;
    let bytes_b = type_universe.byte_size(&inner_b)?;
    let align_a = type_universe.alignment(&inner_a)?;
    let align_b = type_universe.alignment(&inner_b)?;
    if bytes_a == bytes_b && align_a >= align_b {
        return Ok(());  // layout-compatible
    }
    // fall through to meld check
}

// Same rule for LayoutPtr ←→ Ptr<T>
(Type::LayoutPtr(la), Type::Applied(n_b, inner_b)) if n_b == "Ptr" =>
{
    let bytes_b = type_universe.byte_size(&inner_b)?;
    if la.bytes == bytes_b && la.alignment >= type_universe.alignment(&inner_b)? {
        return Ok(());
    }
}
```

### 6.2 TypeUniverse Additions

```rust
impl TypeUniverse {
    /// Returns the byte size of a type. Queries the resolved type's `bytes` field.
    pub fn byte_size(&self, ty: &Type) -> Option<u64> {
        match ty {
            Type::LayoutPtr(lc) => Some(lc.bytes),
            Type::Int | Type::UInt => Some(8),
            Type::Int32 | Type::UInt32 | Type::Float => Some(4),
            // ... existing per-type sizes ...
            Type::Custom(name) => self.resolve(name).and_then(|r| r.bytes),
            _ => None,
        }
    }

    /// Returns the alignment requirement of a type.
    pub fn alignment(&self, ty: &Type) -> Option<u64> {
        // Similar pattern to byte_size
    }
}
```

### 6.3 Interaction with Meld

- Simple layout compatibility (bytes + alignment match) → `as` works directly
- Complex compatibility (field remapping, different layouts with routes) → `meld A <:> B { ... }` required
- No compatibility (bytes differ, alignment stricter) → compiler error

The typechecker tries layout check first, falls through to meld check if layout fails.

### 6.4 Tests

| Test | What it validates |
|------|-------------------|
| `test_cast_ptr_layout_compatible` | `Ptr<Float> as Ptr<Int32>` succeeds (same bytes) |
| `test_cast_ptr_layout_mismatch` | `Ptr<Int> as Ptr<Int32>` fails (different bytes) |
| `test_cast_ptr_meld_required` | Different layouts with explicit route falls through to meld |
| `test_cast_layoutptr_to_typed` | `Ptr<Bits @/0..31> as Ptr<Int32>` succeeds |
| `test_cast_typed_to_layoutptr` | `Ptr<Float> as Ptr<Bits @/0..31>` succeeds |

---

## 7. Phase 3 — Generic Algorithms Over Layout Constraints

**Goal:** Write a function once that works for ANY type matching layout constraints, compiled to a single machine-code block. No monomorphization per type.

### 7.1 Pattern

```briev
// Write once, compile once per (bytes, alignment) shape
defn block_copy<T: bytes == S, align == A>(
    src: Ptr<T>,
    dst: Ptr<T>
) -> Bool
    [dst as Int != src as Int]
{
    let &n = S;
    term __memcpy#(dst, src, n);
};
```

### 7.2 Layout Shape Caching

```rust
/// Cache layout-shaped function variants.
/// Key: (bytes, alignment), Value: LLVM function name.
struct LayoutShapeCache {
    variants: HashMap<(u64, u64), String>,
}

impl LayoutShapeCache {
    fn get_or_create(&mut self, bytes: u64, align: u64, body: &Fn) -> String {
        let key = (bytes, align);
        if let Some(name) = self.variants.get(&key) {
            return name.clone();
        }
        let name = format!("__layout_fn_{}_{}", bytes, align);
        // Compile a new variant with S=bytes, A=align substituted as constants
        self.variants.insert(key, name.clone());
        name
    }
}
```

### 7.3 Built-in Spatial Intrinsics

New intrinsics in `src/ast.rs`:

```rust
pub enum Intrinsic {
    // ... existing ...
    Memcpy,    // __memcpy#(dst: Ptr<Bits @/N>, src: Ptr<Bits @/N>, n: Int) -> Bool
    Memcmp,    // __memcmp#(a: Ptr<Bits @/N>, b: Ptr<Bits @/N>, n: Int) -> Bool
    Memset,    // __memset#(ptr: Ptr<Bits @/N>, val: Int, n: Int) -> Bool
    Hash,      // __hash#(ptr: Ptr<Bits @/N>, n: Int) -> Int
}
```

These map directly to LLVM `@llvm.memcpy`, `@llvm.memcmp`, `@llvm.memset`, and a built-in hash loop.

### 7.4 Tests

| Test | What it validates |
|------|-------------------|
| `test_layout_generic_memcpy` | Same (S, A) → same compiled function |
| `test_layout_generic_different` | Different (S, A) → different function |
| `test_layout_generic_correctness` | Copied bytes match source |
| `test_layout_generic_no_bloat` | Only N distinct variants compiled |

---

## 8. Phase 4 — Opaque Handles via Constrained Pointers

**Goal:** Library returns `Ptr<Bits @/N>` — user cannot inspect internals. Library internally casts to concrete type.

### 8.1 Pattern

```briev
// Library: returns opaque handle
defn open_db(path: String) -> Ptr<Bits @/0..191> {
    let alloc = malloc(24);
    // ... initialize fields ...
    term alloc;
};

// User: passes handle back, cannot inspect internals
defn query_db(handle: Ptr<Bits @/0..191>, sql: String) -> Result {
    term __db_query#(handle, sql);  // goes through library API
};

// Library internals: re-lens to concrete type
inop __db_query#(handle: Ptr<Bits @/0..191>, sql: String) -> Result {
    let conn = handle as Ptr<DbConnection>;  // internal cast
    // ... use conn fields ...
};
```

### 8.2 Module Boundary Enforcement

The compiler prevents `as Ptr<DbConnection>` casts outside the module that defines `DbConnection`. New visibility check in the typechecker:

```rust
fn validate_ptr_cast(
    source_ty: &Type,
    dest_ty: &Type,
    module: &str,
    type_universe: &TypeUniverse,
) -> Result<(), String> {
    if let Type::Applied(name, _) = dest_ty {
        if name == "Ptr" {
            let inner = // extract inner type
            if let Some(def_module) = type_universe.defining_module(inner) {
                if def_module != module {
                    return Err("Cannot cast opaque handle: inner type is private".into());
                }
            }
        }
    }
    Ok(())
}
```

### 8.3 Tests

| Test | What it validates |
|------|-------------------|
| `test_opaque_handle_return` | Library returns Ptr<Bits @/N> |
| `test_opaque_handle_cast_blocked` | User cannot cast to concrete type across module boundary |
| `test_opaque_handle_internal_cast` | Library can cast internally |
| `test_opaque_handle_spatial_ops` | User can still copy/pass handle |

---

## 9. Phase 5 — Function Pointers and Dynamic Dispatch

**Goal:** Allow variables to hold function references, called via indirect dispatch.

### 9.1 Design Decision

Instead of `&f` (which conflicts with Briev's mutable `&` operator) or a new
AST variant, function references use the existing `.#Ptr` projection:

```briev
let cmp: Ptr<fn(Int, Int) -> Bool> = my_cmp .#Ptr;
let result = cmp(ptr_a, ptr_b);  // indirect call via Expr::Call on fn-ptr variable
```

### 9.2 Why No New AST Variants

- `.#Ptr` already parses to `Expr::Projection { target: Ptr }`
- `call(args)` already parses to `Expr::Call(name, args)`
- The typechecker resolves indirect calls: when `Call("cmp", args)` finds a
  variable `cmp` with a function pointer type, it validates args and returns
  the function's return type
- The LLVM backend emits `load i64` → `inttoptr` → `call %loaded(...)`

No parser changes. No new Expr variants. Pure typechecker + backend work.

### 9.3 Type Representation

Function pointer types stay as `Applied("Fn", vec![param_types, return_type])`.
This is already produced by the parser for `(Int) -> Bool` syntax.

### 9.4 LLVM Codegen

```llvm
; my_cmp .#Ptr → ptrtoint @my_cmp to i64
%addr = ptrtoint i64 @my_cmp, i64

; cmp(a, b) indirect call through function pointer variable
%fn_ptr = inttoptr i64 %cmp_val to ptr
%result = call i64 %fn_ptr(i64 %a, i64 %b)
```

### 9.4 Safety

- **Type safety:** Parameter types and return type must match
- **No dangling:** `&f` references a statically linked `defn` or `inop!` (no dynamic loading in Phase 5)
- **Optional contracts:** Function pointer type can carry contracts for verified indirect calls

### 9.5 Tests

| Test | What it validates |
|------|-------------------|
| `test_fn_ptr_type` | `fn(Int) -> Int` typechecks |
| `test_fn_ptr_addr_of` | `&my_fn` produces function pointer |
| `test_fn_ptr_indirect_call` | Calling through function pointer works |
| `test_fn_ptr_type_mismatch` | Wrong parameter type rejected |

---

## 10. Phase 6 — Extract-Operate-Repack for `<:>` Types

**Goal:** When `T <:> Int`, the compiler automatically synthesizes the extract → operate → repack cycle, allowing generic numeric operations on wrappers like `Meters`, `Seconds`.

### 10.1 Pattern

```briev
meld Meters <:> Int;

defn scale<T: T <:> Int>(val: T, factor: Int) -> T {
    // Compiler synthesizes:
    // 1. Extract: val as Int  (zero instructions)
    // 2. Operate: (val as Int) * factor
    // 3. Repack:  result as T  (zero instructions)
    term (val as Int) * factor as T;
};
```

### 10.2 Direction Rules

| Direction | Syntax | When used |
|-----------|--------|-----------|
| T → Int (read) | `val as Int` | Extracting for operation |
| Int → T (write) | `result as T` | Repacking result |
| Bidirectional | `meld T <:> Int` | Full EOR cycle |

### 10.3 EOR Detection

The desugarer detects the pattern `(x as Int) op (y as Int) as T` and eliminates redundant casts:

```rust
fn detect_eor(expr: &Expr, tu: &TypeUniverse) -> Option<Expr> {
    match expr {
        Expr::Cast(
            Expr::BinaryOp(Box::new(BinaryOpExpr {
                lhs: Box::new(Expr::Cast(inner_lhs, Type::Int)),
                rhs: Box::new(Expr::Cast(inner_rhs, Type::Int)),
                op,
            })),
            target_ty,
        ) => {
            if tu.find_meld(target_ty.name(), "Int").is_some() {
                // Emit native operation directly, wrap result
                Some(/* native int op */)
            } else {
                None
            }
        }
        _ => None,
    }
}
```

### 10.4 Tests

| Test | What it validates |
|------|-------------------|
| `test_eor_extract` | `T as Int` is identity (zero instructions) |
| `test_eor_repack` | `result as T` is identity (zero instructions) |
| `test_eor_full_cycle` | `(a as Int) + (b as Int) as T` produces same result as native |
| `test_eor_zero_cost` | LLVM IR has no redundant cast instructions |

---

## 11. Appendix: Flat Control Flow During Implementation

Every implementation phase must follow the flat control flow / minimal nesting guidelines. Review each new or refactored function for arrowhead code and flatten it.

### 11.1 Refactoring Rules

**Rule 1:** Max 2 levels of indentation.

Instead of:
```rust
fn process_ptr(ty: &Type) -> Option<u64> {
    if let Type::Applied(name, args) = ty {
        if name == "Ptr" {
            if let Some(inner) = args.first() {
                if let Some(size) = type_size(inner) {
                    return Some(size);
                }
            }
        }
    }
    None
}
```

Write:
```rust
fn process_ptr(ty: &Type) -> Option<u64> {
    let Type::Applied(name, args) = ty else { return None; };
    if name != "Ptr" {
        return None;
    }
    let inner = args.first()?;
    type_size(inner)
}
```

**Rule 2:** Extract helper functions for inner logic deeper than 2 levels.

```rust
fn process_match_site(ty: &Type) -> Option<u64> {
    // Guard clauses at top level
    let ptr_inner = extract_ptr_inner(ty)?;
    type_size(ptr_inner)
}

fn extract_ptr_inner(ty: &Type) -> Option<&Type> {
    let Type::Applied(name, args) = ty else { return None; };
    if name != "Ptr" {
        return None;
    }
    args.first()
}
```

**Rule 3:** No `else if` chains. Use guard clauses with early returns.

**Rule 4:** Every new function must have a `///` doc comment explaining its purpose, parameters, and return value.

### 11.2 Per-Implementation Checklist

Before committing each phase:
1. `cargo test --lib` — all tests pass
2. `cargo build` — no warnings
3. Run Praetor on new/changed files (complexity ≤ 15, lines ≤ 100, params ≤ 6)
4. Review all new match arms for flat control flow (max 2 levels deep)
5. Add doc comments to every new `fn`, `struct`, `enum`, `trait`, `type`, `const`
6. No `todo!()`, `unreachable!()`, or `// TODO:` in committed code
7. Kani harnesses for safety-critical pointer operations
8. Architecture docs updated if API contracts changed

### 11.3 Legacy Code Refactoring

When touching existing code during any phase, refactor the function to flat control flow if it exceeds 2 levels. This is not optional — every touched function leaves the codebase strictly cleaner than it was found.

The `inttoptr`/`ptrtoint` boxing code in the backend (arrow.rs, helpers.rs, loop_engine.rs) is the primary target. Many of these functions have 3-4 levels of nesting and will be substantially simplified by the new `Ptr` design.

---

*End of plan.*