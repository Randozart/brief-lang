# Strong Bits Thesis — Type System Redesign

**Date:** 2026-07-08
**Status:** Draft — awaiting review before implementation
**Branch:** `feat/language-decluttering` (continues from existing branch)
**Supersedes:** The Phase 2 section of `docs/plans/2026-07-07-language-decluttering.md`
**Relation to main plan:** After this redesign is committed, execution resumes at Phase 3 (Intrinsic Reduction) of the main decluttering plan.

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Design: The Strong Bits Thesis](#2-design-the-strong-bits-thesis)
3. [Flat Control Flow Mandate](#3-flat-control-flow-mandate)
4. [Current State Assessment](#4-current-state-assessment)
5. [Phase 2A — Simplify Type Enum](#5-phase-2a--simplify-type-enum)
6. [Phase 2B — Expand TypeUniverse](#6-phase-2b--expand-typeuniverse)
7. [Phase 2C — NormalizeTypes Desugar Pass](#7-phase-2c--normalizetypes-desugar-pass)
8. [Phase 2D — Migrate LLVM Codegen to Universe-Driven Dispatch](#8-phase-2d--migrate-llvm-codegen-to-universe-driven-dispatch)
9. [Phase 2E — Migrate Remaining Consumers](#9-phase-2e--migrate-remaining-consumers)
10. [Phase 2F — Add `:>` Metadata Lens + Endianness Syntax](#10-phase-2f--add--metadata-lens--endianness-syntax)
11. [Phase 2G — String Removal + Struct Layout in Universe](#11-phase-2g--string-removal--struct-layout-in-universe)
12. [Phase 2H — Pre-Removal Tests and Verification](#12-phase-2h--pre-removal-tests-and-verification)
13. [Verification Gates](#13-verification-gates)
14. [Return to Main Plan](#14-return-to-main-plan)

---

## 1. Executive Summary

The current `Type` enum has ~20+ concrete numeric variants (`Int`, `Int8`, `Int16`..., `Float`, `Float64`, `Bool`, `Char`, `String`, `Data`). Codegen dispatches by matching these variants — a closed, non-extensible system. The TypeUniverse already carries per-type metadata (`llvm_type`, `storage`, `box`, `unbox`, `operators`) declared in `bootstrap.bv`. The Rust `Type` enum duplicates what the universe already knows.

**The strong Bits thesis states:** `Bits(N)` — N bits of raw storage — is the ONLY primitive type. Every other type is a named convention over `Bits(N)` defined by:
- Its **width** (how many bits it occupies)
- Its **operations** (what LLVM instructions operate on it)
- Its **layout** (for compound types: field offsets, sizes)
- Its **metadata** (endianness, encoding, TBAA)

This redesign removes all concrete numeric variants from the `Type` enum, expands the TypeUniverse to carry struct layout and operator mappings, and migrates ALL codegen dispatch from enum matching to universe queries. Users can define new types with the same mechanisms.

### What Changes

| Area | Before | After |
|------|--------|-------|
| `Type` enum | 20+ numeric+string variants | `Bits(u64)` only + `Custom`/`Applied` |
| Codegen dispatch | `match ty { Type::Int8 => …, Type::Float => … }` | `universe.get(ty.name).ops["add"]` |
| Codec/encoding | Implicit in String codegen | Type parameter: `String<UTF8>` |
| Endianness | Hardcoded per target | Annotation: `Bits<32> <~ (endian: be)` |
| `:>` operator | Structural projection only | Structural + metadata projection |
| String | Concrete variant `Type::String` | `Custom("String")` with struct layout in universe |
| Data | Concrete variant `Type::Data` | Alias `Ptr<Bits<8>>` in universe |

### What Stays (Unchanged)

- `Void` (0-bit return marker — degenerate special case)
- `Tuple`, `Union`, `Vector` (structural types defined by element layout, not by name)
- `Constrained`, `TypeVar`, `Generic`, `LayoutPtr`, `Sig`, `Enum`
- All parser syntax for user-facing type expressions
- All existing `.bv` test files (they use `Int`, `u8`, `Float64`, etc. which now resolve through universe)

### What Remains Unchanged from the Main Plan

- Phase 0 (cleanup) — COMPLETE
- Phase 1 (annotation system) — COMPLETE
- Phase 3 (intrinsic reduction) — follows this redesign
- Phase 4 (documentation) — follows this redesign
- Phase 5 (error/warnings) — follows this redesign

---

## 2. Design: The Strong Bits Thesis

### 2.1 Final `Type` Enum

```rust
pub enum Type {
    // ── Primitives ──────────────────────────────────────────
    /// N bits of raw storage. The only scalar primitive.
    /// Every numeric type reduces to Bits(N) with named operations.
    Bits(u64),

    /// Type-level width literal: used inside Applied("Int", [Width(8)])
    Width(u64),

    // ── Named types (all resolved through TypeUniverse) ──────
    /// User-defined type by name: Custom("Int"), Custom("String"), Custom("MyType")
    Custom(String),

    /// Applied generic: Applied("List", [Custom("Int")]), Applied("String", [Custom("UTF8")])
    Applied(String, Vec<Type>),

    // ── Structural types (layout defined by variant, not by name) ──
    /// Ordered composition: Tuple([Bits(8), Bits(32)])
    Tuple(Vec<Type>),

    /// Discriminated sum: Union([Int, String])
    Union(Vec<Type>),

    /// SIMD/array: Vector(Bits(32), [4])
    Vector(Box<Type>, Vec<Dimension>),

    // ── Special ──────────────────────────────────────────────
    /// Zero bits — function return marker. Cannot be stored.
    Void,

    // ── Unchanged constraint / meta types ────────────────────
    Constrained(Box<Type>, BitRange),
    TypeVar(String),
    Generic(String, Vec<Type>),
    LayoutPtr(LayoutConstraint),
    Sig(String),
    Enum(String),
}
```

### 2.2 What Is NOT Bits

These types have NO `to_bits()` representation — they remain as concrete variants:

| Variant | Reason | Example |
|---------|--------|---------|
| `Void` | Zero bits, no storage. Cannot be allocated. | `defn foo() -> Void` |
| `Tuple` | Product of multiple Bits regions, structural | `(Int<8>, Float<32>)` |
| `Union` | Sum of multiple interpretations, structural | `None \| Some<Int<32>>` |
| `Vector` | SIMD/array — multiple elements, structural | `Vector<Int<32>, 4>` |
| `Constrained` | Bit-range constraint ON a type | `Int @/0..7` |
| `LayoutPtr` | Spatial pointer — address semantics | `Ptr<Bits @/0..1023>` |
| `TypeVar` | Type parameter placeholder | `fn map<T>(...)` |
| `Generic` | Generic definition | `type List<T> { ... }` |
| `Sig` | Signal/event type | `sig button_pressed` |
| `Enum` | Discriminated union tag | `enum Option<T> { None, Some(T) }` |

### 2.3 The TypeUniverse (Expanded)

The existing `ResolvedType` struct in `type_universe.rs` gains:

```rust
pub struct ResolvedType {
    // Existing fields (unchanged)
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub base: Option<String>,
    pub bytes: u64,
    pub alignment: u64,
    pub llvm_type: String,
    pub storage: String,
    pub tbaa_node: String,
    pub box_op: Option<String>,
    pub unbox_op: Option<String>,
    pub endian: Option<String>,
    pub codec: Option<String>,
    pub operators: HashMap<(OpRune, Option<String>), OpDeclaration>,
    pub defining_module: String,
    pub source: TypeSource,

    // NEW: Struct layout for compound types (String, user structs)
    pub struct_layout: Option<StructLayout>,

    // NEW: Default type parameter values
    pub default_params: Vec<(String, Type)>,

    // NEW: Optimization metadata
    pub commuting_ops: Vec<OpRune>,
    pub constant_time: bool,
}
```

**`StructLayout`:**
```rust
pub struct StructLayout {
    pub fields: Vec<StructField>,
    pub packed: bool,
    pub total_bytes: u64,
    pub alignment: u64,
}

pub struct StructField {
    pub name: String,
    pub ty: Type,
    pub offset_bits: u64,
    pub size_bits: u64,
}
```

### 2.4 Endianness as Annotation

Per design decision, endianness is a per-value annotation using `<~`:

```briv
// In type definition: default endianness for the type
type NetworkPacket : Bits<1024> {
    endian <~ "big";
};

// On a specific variable: override for this instance
let buf: Bits<64> <~ (endian: be);

// The annotation attaches to the Type via Annotation Vec,
// and is queried during codegen for load/store byte order.
```

The `endian` field already exists on `ResolvedType` (`type_universe.rs`). It just needs to be surfaced through the `<~` annotation system and `:>` projection.

### 2.5 `:>` Unifies Structural + Metadata Projection

The `:>` operator becomes the universal lens:

```rust
pub enum Projection {
    // Existing
    Index(usize),         // tuple :> 0
    Field(String),        // struct :> field_name
    Size,                 // list .#Size

    // NEW — metadata projections
    Width,                // Int<8> :> width → 8
    Endian,               // Bits<32> :> endian → "little"
    Codec,                // String<UTF8> :> codec → "UTF8"
    Ops,                  // Int :> ops → { "add": "add nsw", ... }
}
```

Metadata projections are constant-folded at compile time — they produce `Expr::Integer` or `Expr::String` results, never runtime operations.

### 2.6 Op Declaration in bootstrap.bv

The `bootstrap.bv` file gains operator annotations for each numeric type:

```briv
type Int : Bits {
    bytes <~ 8;
    alignment <~ 8;
    llvm <~ "i64";
    storage <~ "Boxed";
    tbaa <~ "Int";
    box <~ "sext.i64.to.i8#?";   // existing syntax for boxing
    unbox <~ "trunc.i64.to.i8#?";
    op add <~ "add nsw";          // NEW: operator → LLVM opcode
    op sub <~ "sub nsw";
    op mul <~ "mul nsw";
    op div <~ "sdiv";
    default_width <~ 64;          // NEW: Int → Int<64>
    commuting <~ true;            // NEW: optimization hint
};
```

These are parsed by `apply_binding()` — the existing `TypeUniverse` binding system handles arbitrary key-value pairs. Only three new binding names are needed: `op <name>`, `default_width`, and `commuting`.

---

## 3. Flat Control Flow Mandate

**This is non-negotiable.** Every function modified or created by this plan must not exceed 2 levels of nesting depth. The one and only exception is LLVM IR emission functions (`emit_expr.rs`, `emit_stmt.rs`, `emit_toplevel.rs`, builder methods, backend `mod.rs` functions that produce `.ll` output directly). Helper functions called by these emitters ARE NOT exempt.

### 3.1 Anti-Patterns — Banned

```rust
// BANNED: if-let deeper than 1 level
fn process(x: Option<Value>) -> Option<i64> {
    if let Some(val) = x {           // level 1
        if let Some(result) = ... {  // level 2 — EXCEEDS LIMIT
            // level 3 — VIOLATION
        }
    }
    None
}

// BANNED: match inside match
fn analyze(ty: &Type) {
    match ty {                       // level 1
        Custom(name) => {            // level 2
            match name.as_str() {    // level 3 — VIOLATION
                ...
            }
        }
    }
}

// BANNED: if inside for inside match
fn process(items: &[Item]) {
    for item in items {              // level 1
        if item.active {             // level 2
            ...                      // level 3, but this is exactly 2 — OK
            if item.valid {          // level 3 — VIOLATION
                ...
            }
        }
    }
}
```

### 3.2 Acceptable Patterns

```rust
// OK: Guard clause with ?
fn process(x: Option<Value>) -> Option<i64> {
    let val = x?;                    // level 1
    let result = val.as_i64()?;      // level 1
    if result <= 0 { return None; }  // level 1
    Some(result)                     // level 1
}

// OK: Single-level match with extracted helpers
fn analyze(ty: &Type) {
    let name = match ty {            // level 1
        Custom(n) => n,
        _ => return,
    };
    dispatch_analysis(name);         // level 1 — extracted
}

// OK: for with one conditional (exactly 2 levels)
fn process(items: &[Item]) {
    for item in items {              // level 1
        if item.active {             // level 2 — exactly 2
            push(item);              // level 2
        }
    }
}
```

### 3.3 God Function Extraction

Any function exceeding ~100 lines encountered during this redesign MUST be evaluated for extraction. Known extraction candidates:

| Candidate | File | Approx Lines | What It Mixes |
|-----------|------|-------------|---------------|
| `parse_type_inner` | `parser.rs` | ~400 | Type atom parsing, generic args, vector, union, function types |
| `emit_binop` | `helpers.rs` | ~180 | Operator dispatch, constant folding, custom operators, boxing |
| `emit_transaction` | `emit_toplevel.rs` | ~800 | Init block, convergence loop, phi nodes, term dispatch |
| `emit_intrinsic_call` | `expr/intrinsics.rs` | ~120 | 120+ intrinsic dispatch, argument marshalling |

Each extraction follows: **extract → commit (no behavior change)** → **refactor for flat control flow → commit** → **then make behavioral changes**.

### 3.4 Rationale Comments on Every Change

Every modified or new code site must have:
```rust
// 2026-07-08: <phase> — <why this exists>
// <what pattern it targets, what would break if removed>
```

Never delete existing rationale comments. Rewrite them to explain the new structure.

---

## 4. Current State Assessment

### 4.1 What Exists

- `Type::Bits { width, interpretation }` and `Type::Width(u64)` — added in Phase 2a (committed)
- Parser accepts `Int<N>` syntax → `Applied("Int", [Width(N)])` → resolved to Bits — Phase 2b (committed)
- TypeUniverse with `ResolvedType`, `build()`, `get()`, `llvm_type`, `storage`, `box_op`, `unbox_op` — fully operational
- `bootstrap.bv` declares Int, UInt, Int8..UInt32, Float, Float64, Bool, Char, String, Data — all `: Bits`
- `to_bits()`, `bit_width()`, `is_signed()`, `universe_key()` — all normalize to Bits form
- Codegen: `emit_binop` matches `Type::Float`, `Type::Float64`, `is_integral()` — hardcoded
- Codegen: `emit_neg` matches `Type::Float`, `Type::Float64` — hardcoded
- Codegen: `llvm_type()` queries universe with `fallback_llvm_type()` fallback — hardcoded match

### 4.2 What Must Change

| File | Hardcoded Patterns | Replace With |
|------|--------------------|--------------|
| `ast.rs` ~170 lines | `Type::Int`, `Type::Int8`, `Type::Float` etc. in enum def | Remove concrete variants |
| `ast.rs` ~80 lines | `to_bits()`, `bit_width()`, `is_signed()`, `is_integral()`, `is_float_type()` | Simplify — only match on `Bits(u64)`, `Custom`, `Applied` |
| `ast.rs` ~40 lines | `universe_key()` | Remove concrete variant arms |
| `parser.rs` ~120 lines | `parse_type_inner()` keyword→variant mapping | Map all numeric keywords to `Custom(name)` |
| `lexer.rs` ~40 lines | TypeInt, TypeInt8..TypeChar token→variant mapping | Remove ~26 tokens, keep only TypeString/TypeVoid/TypeData |
| `type_universe.rs` ~200 lines | Add operator parsing, struct layout, default params | Expand `apply_binding()`, `ResolvedType` |
| `helpers.rs` ~180 lines | `emit_binop()` matching `Type::Float`, `Float64`, `is_integral()` | Universe-driven op dispatch |
| `emit_toplevel.rs` ~30 lines | `fallback_llvm_type()` matching concrete variants | Remove, fold into `llvm_type()` |
| `emit_expr.rs` ~50 lines | `matches!(ty, Type::Int8)` etc. in various dispatch points | `universe_key()` or `to_bits()` queries |
| `emit_stmt.rs` ~40 lines | Same pattern | Universe queries |
| `expr/math.rs` ~80 lines | `emit_neg`, bitwise ops matching `Type::Float`/`Type::Float64` | Universe-driven dispatch |
| `expr/call.rs` ~30 lines | Type matching for argument coercion | Universe queries |
| `expr/projection.rs` ~20 lines | Type matching for `:>` projection | Metadata projection for `:> width`, `:> endian` |
| `analyse/region.rs` ~30 lines | Type matching for register analysis | Universe queries |
| `proof_engine.rs` ~50 lines | Type matching for constraint solving | Universe queries |
| `interpreter.rs` ~80 lines | Type matching for value creation | Universe queries |
| `webstack.rs` ~20 lines | Type matching for WASM mapping | `_ => {}` stub fallthrough |
| `circt.rs` ~15 lines | Type matching for MLIR types | `_ => {}` stub fallthrough |
| `bootstrap.bv` ~50 lines | Missing ops, default_width, commuting | Add to each type |

**Total affected files:** ~20
**Total concrete variant references to migrate:** ~400 (estimated from `git grep "Type::Int[^e8]\|Type::Float\|Type::Bool\|Type::Char\|Type::String[^T]\|Type::Data"`)

---

## 5. Phase 2A — Simplify Type Enum

**Goal:** Remove `Interpretation`, change `Type::Bits{width,interpretation}` → `Bits(u64)`, remove ALL concrete numeric variants from `Type` enum. Remove `String` and `Data` variants.

### 5.1 Files Changed

| File | Change | Commit |
|------|--------|--------|
| `src/ast.rs` | Remove `Interpretation` enum, `BitsInfo` struct, 12 concrete variants (`Int`, `Int8`, `Int16`, `Int32`, `UInt`, `UInt8`, `UInt16`, `UInt32`, `Float`, `Float64`, `Bool`, `Char`, `String`, `Data`). Change `Type::Bits{...}` → `Bits(u64)`. | 2a-simplify-enum |
| `src/ast.rs` | Update `to_bits()` → returns `Option<u64>` (just width). Update `bit_width()`, `is_signed()`, `is_integral()`, `is_float_type()`, `is_numeric()`, `universe_key()`. | 2a-simplify-enum |

### 5.2 `to_bits()` After Redesign

```rust
// 2026-07-08: Phase 2A — strong Bits thesis
// Returns Some(width) for Bits(u64), None for non-Bits types (Void, Tuple, etc.)
// All numeric types are Custom/Applied, resolved through universe.
// Bits(u64) is the only scalar storage primitive.
pub fn bit_width(&self) -> Option<u64> {
    match self {
        Type::Bits(n) => Some(*n),
        Type::Width(n) => Some(*n),
        _ => None,
    }
}
```

### 5.3 `universe_key()` After Redesign

All type dispatch goes through the universe key:

```rust
pub fn universe_key(&self) -> &str {
    match self {
        Type::Custom(name) | Type::Applied(name, _) | Type::Sig(name) | Type::Enum(name) => name.as_str(),
        Type::Bits(_) => "Bits",
        Type::Void => "Void",
        Type::Tuple(_) | Type::Union(_) => "Tuple",  // structural — not in universe
        Type::Constrained(inner, _) => inner.universe_key(),
        Type::TypeVar(name) => name.as_str(),
        Type::Generic(name, _) => name.as_str(),
        Type::LayoutPtr(_) => "LayoutPtr",
        Type::Vector(_, _) => "Vector",
        Type::Width(_) => "Width",
    }
}
```

### 5.4 Pre-Removal Tests

Before ANY removal, write tests that verify:

- `let x: Int = 0;` → `Custom("Int")` register in parse, resolved through universe
- `let y: Int<8> = 0;` → `Applied("Int", [Width(8)])` → `Bits(8)` with Int's ops
- `let z: u32 = 0;` → `Custom("u32")` or `Applied("u32", [Width(32)])`
- `let f: Float = 0.0;` → `Custom("Float")` → resolved through universe
- `let b: Bool = true;` → `Custom("Bool")` → resolved through universe
- Type compatibility: `Int<8>` and `i8` are compatible (same width, same ops)

These tests are committed BEFORE Phase 2A.

### 5.5 God Function Extraction Candidates

During this phase, if `parse_type_inner` (~400 lines) needs significant modification, extract its sub-responsibilities first:

1. `parse_type_atom()` — handles token → base Type
2. `parse_generic_args()` — handles `<Type, Type>` and `[Type]` generic arg parsing
3. `parse_vector_dimensions()` — already extracted in Phase 2b
4. `parse_function_type()` — handles `Type -> Type`
5. `parse_union_type()` — handles `Type | Type`

**Commit order:** extraction → commit (no behavior change) → then simplify → commit.

---

## 6. Phase 2B — Expand TypeUniverse

**Goal:** Add `op <name>` binding, `default_width`, `commuting`, `struct_layout`, `endian` query, and `llvm_type_for_width()` to the TypeUniverse. All before any codegen changes so the data is ready when codegen migrates.

### 6.1 Changes to `type_universe.rs`

**Add to `apply_binding()`:**

```rust
// 2026-07-08: Phase 2B — operator binding
if binding.name == "op" && binding.params.len() == 1 {
    let op_name = binding.params[0].clone();
    rt.operators.insert((op_str_to_rune(&op_name), None), OpDeclaration {
        name: value_to_string(&binding.value),
        // ...
    });
}

// 2026-07-08: Phase 2B — default width
if binding.name == "default_width" {
    if let Some(n) = value_to_u64(&binding.value) {
        rt.default_params.push(("W".to_string(), Type::Width(n)));
    }
}
```

**Add `struct_layout` support:**

```rust
// 2026-07-08: Phase 2B — struct layout for String and compound types
// String has fields: ptr (Ptr<Bits<8>>), len (Bits<64>), codec (Bits<8>)
// User-defined structs get their layout computed from type params + annotations.
```

**Add `llvm_type_for_width()`:**

```rust
// 2026-07-08: Phase 2B — compute LLVM type string from base type + width
// For Int<8>: base "Int" + width 8 → "i8"
// For Float<32>: base "Float" + width 32 → "float"
pub fn llvm_type_for_width(&self, base_name: &str, width: u64) -> Option<Cow<'static, str>> {
    let rt = self.get(base_name)?;
    match rt.storage.as_str() {
        "Native" => match base_name {
            "Float" if width <= 32 => Some(Cow::Borrowed("float")),
            "Float" if width <= 64 => Some(Cow::Borrowed("double")),
            _ => Some(Cow::Owned(format!("i{}", width))),
        },
        "Boxed" => Some(Cow::Borrowed("i64")),  // boxing to native register
        _ => Some(Cow::Owned(format!("i{}", width))),
    }
}
```

### 6.2 Changes to `bootstrap.bv`

Add operator annotations, `default_width`, and `commuting` to every numeric type:

```briv
type Int : Bits {
    bytes <~ 8;
    alignment <~ 8;
    llvm <~ "i64";
    storage <~ "Boxed";
    tbaa <~ "Int";
    box <~ "sext.i64.to.i8#?";
    unbox <~ "trunc.i64.to.i8#?";
    op add <~ "add nsw";
    op sub <~ "sub nsw";
    op mul <~ "mul nsw";
    op div <~ "sdiv";
    default_width <~ 64;
    commuting <~ true;
};

type String : Bits {
    bytes <~ 8;
    alignment <~ 8;
    llvm <~ "{ i8*, i64, i8 }";
    storage <~ "Boxed";
    tbaa <~ "String";
    box <~ "ptrtoint#";
    unbox <~ "inttoptr#";
    codec <~ "UTF8";
    default_codec <~ "UTF8";
    op len <~ "extractvalue %this, 1";
    op codepoint_at <~ "extractvalue %this, 0";
};
```

---

## 7. Phase 2C — NormalizeTypes Desugar Pass

**Goal:** A new desugar pass that runs after parsing, before typechecking. It resolves:
1. `Custom("Int")` → `Applied("Int", [Width(64)])` (default width from universe)
2. `Custom("String")` → `Applied("String", [Custom("UTF8")])` (default codec)
3. `"hello"` literal → struct literal `{ ptr: &"hello", len: 5, codec: 0 }`
4. `:> width`, `:> endian`, `:> codec` → constant-folded values

### 7.1 New File: `src/normalize_types.rs`

```rust
// 2026-07-08: Phase 2C — type normalization pass
// Resolves Custom/Applied types to their concrete Bits widths using
// universe defaults. Converts string literals to struct literals.
// Runs after parsing, before typechecking and codegen.

pub fn normalize_types(program: &mut Program, universe: &TypeUniverse) -> Result<(), String> {
    for item in &mut program.items {
        normalize_toplevel(item, universe)?;
    }
    Ok(())
}

fn normalize_toplevel(item: &mut TopLevel, universe: &TypeUniverse) -> Result<(), String> {
    match item {
        TopLevel::StateDecl(decl) => {
            decl.ty = normalize_type(&decl.ty, universe);
            normalize_expr(&mut decl.initializer, universe);
        }
        TopLevel::Txn(txn) => {
            // normalize types in all signatures, parameters, and statements
        }
        // ... all other top-level items
    }
}

fn normalize_type(ty: &Type, universe: &TypeUniverse) -> Type {
    match ty {
        Type::Custom(name) => {
            let rt = universe.get(name)?;
            // Apply default type parameters
            if let Some(default) = rt.default_params.first() {
                Type::Applied(name.clone(), vec![default.1.clone()])
            } else {
                Type::Bits(rt.bytes * 8)  // compute from bytes
            }
        }
        Type::Applied(name, args) => {
            let resolved_args: Vec<Type> = args.iter().map(|a| normalize_type(a, universe)).collect();
            Type::Applied(name.clone(), resolved_args)
        }
        _ => ty.clone(),
    }
}
```

### 7.2 String Literal → Struct Literal Desugaring

```briv
// Before normalize_types:
let s: String = "hello";

// After normalize_types:
let s: String = String {
    ptr: &"hello",
    len: 5,
    codec: 0,       // UTF8 tag
};
```

This desugaring is a separate commit within Phase 2C.

### 7.3 Integration into Pipeline

The NormalizeTypes pass plugs into the compilation pipeline:

```rust
// In lib.rs or compiler.rs:
pub fn compile(program: &mut Program, options: &Options) -> Result<(), Error> {
    // Phase 1: Universe build
    let mut universe = TypeUniverse::new();
    universe.build(program)?;

    // Phase 2: Normalize types (NEW)
    normalize_types(program, &universe)?;

    // Phase 3: Desugar (existing)
    desugar(program);

    // Phase 4: Typecheck + analyze (existing)
    typecheck(&program, &universe)?;

    // Phase 5: Codegen (existing)
    backend.generate(program, &universe)?;
}
```

---

## 8. Phase 2D — Migrate LLVM Codegen to Universe-Driven Dispatch

**Goal:** Replace ALL `match ty { Type::Int8 => … }` patterns in the LLVM backend with universe queries. This is the largest phase — done file-by-file, with benchmarks after every file.

### 8.1 `helpers.rs` — `emit_binop()` Rewrite

**Before:**
```rust
fn emit_binop(out: &mut String, op: &str, int_op: &str, float_op: &str,
              a: &TypedRegister, b: &TypedRegister) {
    // ~180 lines of match on Type::Float, Type::Float64, is_integral(), etc.
    match &a.ty {
        Type::Float => { write!(out, "  {} = {} fast float {}, {}\n", ...) }
        Type::Float64 => { write!(out, "  {} = {} fast double {}, {}\n", ...) }
        _ if a.ty.is_integral() => { write!(out, "  {} = {} i64 {}, {}\n", ...) }
        _ => { /* box to i64 */ }
    }
}
```

**After:**
```rust
// 2026-07-08: Phase 2D — universe-driven operator dispatch
fn emit_binop(out: &mut String, op: &str, _int_op: &str, _float_op: &str,
              a: &TypedRegister, b: &TypedRegister, ctx: &BackendContext) {
    let rune = op_str_to_rune(op);
    let type_name = a.ty.universe_key();
    let rt = ctx.universe.get(type_name);

    // Query the operator from the universe
    let (llvm_op, llvm_ty) = match rt.and_then(|r| r.operators.get(&(rune, None))) {
        Some(decl) => {
            let llvm_ty = ctx.llvm_type_for_width(type_name, a.ty.bit_width().unwrap_or(64));
            (decl.name.as_str(), llvm_ty)
        }
        None => {
            // Fallback: box to i64
            return emit_boxed_binop(out, op, a, b, ctx);
        }
    };

    let result = ctx.gen_reg();
    writeln!(out, "  {} = {} {} {}, {}", result, llvm_op, llvm_ty, a.reg, b.reg).ok();
}
```

**Key constraint:** This function must NOT exceed 2 nesting levels. The universe query and operator lookup are extracted into helpers.

### 8.2 `emit_toplevel.rs` — `llvm_type()` Simplification

**Before:**
```rust
fn fallback_llvm_type(ty: &Type) -> &'static str {
    match ty {
        Type::Int | Type::UInt => "i64",
        Type::Int8 | Type::UInt8 => "i8",
        // 10 more arms...
    }
}

pub fn llvm_type(&self, ty: &Type) -> &str {
    self.ctx.type_universe.as_ref()
        .and_then(|u| u.get_by_type(ty))
        .map(|r| r.llvm_type.as_str())
        .unwrap_or_else(|| Self::fallback_llvm_type(ty))
}
```

**After:**
```rust
// 2026-07-08: Phase 2D — universe-only LLVM type dispatch
// fallback_llvm_type() removed. Universe provides ALL type mappings.
// For parameterized types (Int<8>), llvm_type_for_width() computes the width.
pub fn llvm_type(&self, ty: &Type) -> Cow<'static, str> {
    let name = ty.universe_key();
    let width = ty.bit_width();

    if let Some(ref universe) = self.ctx.type_universe {
        if let Some(rt) = universe.get(name) {
            if let Some(w) = width {
                return universe.llvm_type_for_width(name, w)
                    .unwrap_or(Cow::Owned(rt.llvm_type.clone()));
            }
            return Cow::Borrowed(rt.llvm_type.as_str());
        }
    }

    // Ultimate fallback for types not in universe (Custom names after resolution)
    if let Some(w) = width {
        Cow::Owned(format!("i{}", w))
    } else {
        Cow::Borrowed("i64")
    }
}
```

### 8.3 File Migration Order

| Order | File | Key Changes | Benchmark Check |
|-------|------|-------------|-----------------|
| 1 | `helpers.rs` | `emit_binop()` → universe-driven | Yes — arithmetic benchmarks |
| 2 | `expr/math.rs` | `emit_neg`, bitwise ops → universe queries | Yes — float_math, nbody |
| 3 | `emit_toplevel.rs` | `llvm_type()`, `fallback_llvm_type()` removed | Yes — all benchmarks |
| 4 | `emit_expr.rs` | Expression dispatch type checks | Yes |
| 5 | `emit_stmt.rs` | Statement dispatch, box/unbox, `adapt_to_i64()` | Yes |
| 6 | `expr/call.rs` | Argument coercion | Yes — fannkuch |
| 7 | `expr/intrinsics.rs` | Intrinsic return type handling | Yes |
| 8 | `expr/projection.rs` | `:>` projection rewrite | Yes |
| 9 | `expr/rest.rs` | All expression sub-dispatch | Yes |

**Benchmark rule:** After EVERY file, run `bash benchmarks/build_and_bench.sh --runtime`. Any regression below 0.97x of the Phase 2C baseline blocks the commit. Document the ratio in the commit message.

### 8.4 Flat Control Flow Checklist for Codegen

Each migrated function must pass:

- [ ] No `if` deeper than 2 levels
- [ ] No `match` inside `match` — extract inner logic to named helper
- [ ] No `for` inside `if` inside `match` — extract to helper
- [ ] Guard clauses over `else if`
- [ ] `let val = opt else { return }` for Option unwrapping
- [ ] Helper names describe their single responsibility
- [ ] `?` operator for early returns from Result/Option
- [ ] Each helper is ≤ ~40 lines

---

## 9. Phase 2E — Migrate Remaining Consumers

**Goal:** Update all non-LLVM-backend consumers of the Type enum to use universe queries or `universe_key()`. These consumers don't need full migration — just enough to compile without errors.

### 9.1 Files and Strategy

| File | Variant References | Strategy |
|------|-------------------|----------|
| `src/interpreter.rs` | ~80 | Replace `Type::Int8` → `ty.to_bits() == Some(8)` etc. Keep value creation paths working. |
| `src/proof_engine.rs` | ~50 | Replace concrete variant matches with `ty.universe_key()` queries. |
| `src/analysis/region.rs` | ~30 | Same — replace with `ty.universe_key()` or `ty.bit_width()`. |
| `src/analysis/dataflow.rs` | ~15 | Replace with `ty.bit_width().is_some()` (numeric check). |
| `src/analysis/transition_graph.rs` | ~10 | Same — bit_width() for numeric types. |
| `src/backend/webstack.rs` | ~20 | Add `Bits(_) => {}` fallthrough. Other concretes handled via `_ => {}`. |
| `src/backend/circt.rs` | ~15 | Same — `Bits(_) => {}` + `_ => {}`. |
| `src/backend/mod.rs` | ~10 | Hashtag validation — uses `.name`, unaffected. |
| `src/desugarer.rs` | ~5 | Type inference for postcondition state decls. Replace concrete with `to_bits()`. |
| `src/typechecker.rs` | ~40 | Type compatibility — uses `universe_key()` already in many paths. Add missing ones. |

### 9.2 Handling `String` and `Data` Removal

Since String and Data are removed from the Type enum, every `matches!(ty, Type::String)` or `Type::Data` must become:

```rust
// Before:
Type::String => { ... }
Type::Data => { ... }

// After:
name if name == "String" || name == "Data" => { ... }
// where name = ty.universe_key()
```

Add a helper to reduce churn:

```rust
// 2026-07-08: Phase 2E — universe name query helper
pub fn type_is_name(ty: &Type, name: &str) -> bool {
    ty.universe_key() == name
}
```

### 9.3 Backward Compatibility for Old `.bv` Files

Existing `.bv` files use `Int`, `u8`, `Float64`, `String`, etc. These type names are still valid — they become `Custom("Int")`, `Custom("String")` etc. which resolve through the universe. No source-level changes needed. Only the internal Rust Type enum changes.

---

## 10. Phase 2F — Add `:>` Metadata Lens + Endianness Syntax

**Goal:** Expand `:>` to project metadata (`width`, `endian`, `codec`) in addition to structural fields. Add `<~ (endian: be)` annotation syntax for per-value endianness.

### 10.1 `:>` Metadata Projection

New projection runes in `src/ast.rs`:

```rust
pub enum Projection {
    // Existing (structural)
    Index(usize),
    Field(String),
    Size,

    // NEW (metadata queries)
    Width,
    Endian,
    Codec,
    Ops,
}
```

**Parser changes:** When `:>` is followed by a metadata keyword (`width`, `endian`, `codec`), produce the corresponding metadata projection instead of a field lookup.

**Constant folding:** Metadata projections fold to constants during the NormalizeTypes pass:

```rust
// Int<8> :> width → 8
fn fold_projection(expr: &Expr, universe: &TypeUniverse) -> Option<Expr> {
    match expr {
        Expr::Projection(box Expr::TypeAnnotation(ty), Projection::Width) => {
            Some(Expr::Integer(ty.bit_width()? as i64))
        }
        Expr::Projection(box Expr::TypeAnnotation(ty), Projection::Endian) => {
            let rt = universe.get(ty.universe_key())?;
            Some(Expr::String(rt.endian.clone()?))
        }
        _ => None,
    }
}
```

### 10.2 Endianness Annotation

```briv
// Per-value syntax:
let x: Bits<32> <~ (endian: big);

// In type definition (default):
type NetworkInt16 : Bits<16> {
    endian <~ "big";
};

// Querying at compile time:
if x :> endian == "big" {        // constant-folded to true/false
    // big-endian path
};
```

The `<~` annotation on a value attaches `Annotation` to the type annotation's modifier list. The codegen for load/store checks `ty_modifiers.iter().any(|a| a.name == "endian")`.

**LLVM codegen for endianness:**

```rust
// 2026-07-08: Phase 2F — emit byte swap for endian mismatch
fn emit_load_with_endian(out: &mut String, ptr: &str, ty: &Type,
                          endian: &str, ctx: &BackendContext) {
    let native_endian = ctx.target.endian;  // "little" or "big"
    let llvm_ty = ctx.llvm_type(ty);

    if endian == native_endian {
        // Native byte order — normal load
        writeln!(out, "  %val = load {}, ptr {}", llvm_ty, ptr).ok();
    } else {
        // Non-native — load + bswap
        writeln!(out, "  %tmp = load {}, ptr {}", llvm_ty, ptr).ok();
        writeln!(out, "  %val = call {} @llvm.bswap.{}({} %tmp)", llvm_ty, llvm_ty, llvm_ty).ok();
    }
}
```

---

## 11. Phase 2G — String Removal + Struct Layout in Universe

**Goal:** Remove `Type::String` and `Type::Data` as concrete variants. String becomes `Custom("String")` with struct layout in the universe. Data becomes an alias for `Ptr<Bits<8>>`.

### 11.1 String Struct Layout

The universe declares String as:

```rust
"String" => ResolvedType {
    bytes: 24,        // ptr(8) + len(8) + codec(1) + padding(7)
    alignment: 8,
    llvm_type: "{ i8*, i64, i8 }",
    storage: "Boxed",
    struct_layout: Some(StructLayout {
        fields: vec![
            StructField { name: "ptr".into(), ty: Type::Applied("Ptr".into(), vec![Type::Bits(8)]), offset_bits: 0, size_bits: 64 },
            StructField { name: "len".into(), ty: Type::Bits(64), offset_bits: 64, size_bits: 64 },
            StructField { name: "codec".into(), ty: Type::Bits(8), offset_bits: 128, size_bits: 8 },
        ],
        packed: false,
        total_bytes: 24,
        alignment: 8,
    }),
    box_op: Some("ptrtoint#"),
    unbox_op: Some("inttoptr#"),
    codec: Some("UTF8"),
    operators: {
        len: extractvalue %this, 1,
        ptr: extractvalue %this, 0,
    },
}
```

### 11.2 Codegen for String Operations

`"hello".len()` → compiler:

1. Queries universe for `String` → finds `operators["len"]` → `"extractvalue %this, 1"`
2. Emits: `%len = extractvalue { i8*, i64, i8 } %str_val, 1`

This is the SAME LLVM that `fallback_llvm_type` + match on `Type::String` produced. The information just comes from the universe instead of from hardcoded Rust match arms.

### 11.3 Data Removal

```briv
// In bootstrap.bv:
type Data = Ptr<Bits<8>>;    // Simple alias — no struct layout needed
```

Every `Type::Data` reference in Rust code becomes `ty.universe_key() == "Data"` and codegen uses the universe's `llvm_type` ("i8*") directly.

---

## 12. Phase 2H — Pre-Removal Tests and Verification

### 12.1 Pre-Removal Tests (committed BEFORE Phase 2A)

1. **Int<N> parsing**: `let x: Int<8> = 0;` produces `Applied("Int", [Width(8)])` → resolves to `Bits(8)` with Int's ops
2. **Default width resolution**: `let x: Int = 0;` → `Custom("Int")` → NormalizeTypes pass → `Applied("Int", [Width(64)])` 
3. **Shorthand types**: `let x: i8 = 0;` → same as `Int<8>`
4. **Float parsing**: `let f: Float = 0.0;` → universe resolution
5. **Bool**: `let b: Bool = true;` → universe resolution
6. **Type compatibility**: `Int<8>` and `i8` are compatible
7. **String**: `let s: String = "hello";` → struct layout resolution
8. **Endianness**: `Bits<32> <~ (endian: be)` → annotation attached

### 12.2 Post-Phase 2D Verification

After LLVM codegen migration:
```
cargo build                        # 0 warnings
cargo test --lib                   # all tests pass
bash benchmarks/build_and_bench.sh --runtime   # compare against Phase 2C baseline
bash benchmarks/build_and_bench.sh --correctness  # all output checks pass
```

### 12.3 Post-Phase 2G Verification

```
cargo build                        # 0 warnings
cargo test --lib                   # all tests pass
bash benchmarks/build_and_bench.sh --runtime   # compare against Phase 2C baseline
bash benchmarks/build_and_bench.sh --correctness  # all output checks pass
bash benchmarks/build_and_bench.sh --optimizer  # optimizer benchmarks
```

### 12.4 String Behavior Tests

```briv
// test_string_len.bv
import# "std/io.bv";
defn main() -> Int {
    let s: String = "hello";
    term s.len();          // should produce 5
};
```

This test verifies that String struct layout projection works end-to-end through the compiler.

---

## 13. Verification Gates

### 13.1 Per-Commit Gate

```
cargo build                        # 0 warnings
cargo test --lib                   # all tests pass (running count: ~1400+)
```

If `cargo build` produces ANY warnings, the commit is blocked. No exceptions.

### 13.2 Per-Phase Gate (post-codegen phases)

```
bash benchmarks/build_and_bench.sh --runtime        # compare against pre-phase baseline
bash benchmarks/build_and_bench.sh --correctness    # all output checks
```

Any runtime benchmark below 0.97x of its pre-phase baseline blocks that phase.

### 13.3 Flat Control Flow Gate

Before every commit, run `praetor` on all new/modified files:
```
cargo install praetor
praetor -f src/normalize_types.rs  # (or whatever the tool invocation is)
```

If Praetor reports complexity > 15 or lines > 100 for any function, that function must be extracted before committing.

### 13.4 Full Suite After Phase 2G

```
cargo build                        # 0 warnings
cargo test --lib                   # all tests pass
bash benchmarks/build_and_bench.sh --runtime        # full baseline comparison
bash benchmarks/build_and_bench.sh --correctness    # all output checks
bash benchmarks/build_and_bench.sh --optimizer      # optimizer benchmarks
```

---

## 14. Return to Main Plan

After Phase 2G is committed, execution resumes on the main decluttering plan (`docs/plans/2026-07-07-language-decluttering.md`) at:

| Phase | What | Section in Main Plan |
|-------|------|----------------------|
| **Phase 3** | Intrinsic Reduction + Prelude | Section 8 |
| **Phase 4** | Documentation Overhaul | Section 9 |
| **Phase 5** | Error/Warning Improvements | Section 10 |

**Phase 3 is now substantially cleaner** because:
- Intrinsic signatures use `Int`, `Ptr<Byte>`, `Bool` etc. — all of which are just `Custom` types resolved through the universe
- The inop declarations in `std/os/*.bv` don't need to match concrete Type variants — they work with any type name
- The NormalizeTypes pass handles default resolution, so `Int` in an intrinsic signature becomes `Int<64>` automatically

**Workflow rules from the main plan still apply:**
- Section 3.1: Git discipline (commit on `feat/language-decluttering`, one commit per logical step)
- Section 3.3: Architectural comments on every change
- Section 3.4: Pre-removal tests committed before removals
- Section 3.5: God function extraction before behavioral change
- Section 11: Validation strategy
- Section 12: Rollback plan

### What We've Done

| Phase | Main Plan Equivalent | Status |
|-------|---------------------|--------|
| Phase 0a-d | Cleanup | ✅ Complete |
| Phase 1a-c | Annotation system | ✅ Complete |
| Phase 2a (old) | Add Type::Bits/Width | ✅ Complete (will be replaced by this redesign) |
| Phase 2b (old) | Parser Int<N> syntax | ✅ Complete (will be refactored) |
| Phase 2A-G (new) | Strong Bits Thesis Redesign | ⬜ **This plan** |
| Phase 3 | Intrinsic Reduction | ⬜ Next |
| Phase 4 | Documentation | ⬜ After Phase 3 |
| Phase 5 | Error/Warnings | ⬜ After Phase 4 |

---

## Appendix A: Summary of All New Functions to Create

| Function | File | Responsibility | Approx Lines |
|----------|------|---------------|-------------|
| `normalize_types()` | `src/normalize_types.rs` | Entry point for normalization pass | 15 |
| `normalize_toplevel()` | `src/normalize_types.rs` | Dispatch per top-level item type | 50 |
| `normalize_type()` | `src/normalize_types.rs` | Resolve defaults for Custom/Applied types | 30 |
| `normalize_string_literal()` | `src/normalize_types.rs` | Convert `"hello"` to struct literal | 40 |
| `fold_metadata_projection()` | `src/normalize_types.rs` | Constant-fold `:> width`, `:> endian` | 40 |
| `universe_key()` | `src/ast.rs` | Already exists — simplify | (existing) |
| `bit_width()` | `src/ast.rs` | Already exists — simplify | (existing) |
| `type_is_name()` | `src/ast.rs` | Name query convenience | 5 |
| `llvm_type_for_width()` | `src/type_universe.rs` | Compute LLVM type from base + width | 25 |
| `emit_load_with_endian()` | `src/backend/llvm/helpers.rs` | Endian-aware load | 30 |
| `emit_boxed_binop()` | `src/backend/llvm/helpers.rs` | Box-to-i64 fallback for unknown ops | 25 |

## Appendix B: Files to Delete

| File | Reason |
|------|--------|
| None yet | All existing files remain; `Type::String` etc. are just no longer used |

The `Interpretation` enum removal and `BitsInfo` struct removal happen within `ast.rs` — no files deleted.

## Appendix C: Lexer Token Changes

| Token | Action |
|-------|--------|
| `TypeInt` | Remove — `Int` becomes `Identifier` |
| `TypeInt8` through `TypeInt32` | Remove — `Int8` etc. become `Identifier` |
| `TypeUInt` through `TypeUInt32` | Remove — `UInt` etc. become `Identifier` |
| `TypeFloat`, `TypeFloat64` | Remove — `Float`/`Float64` become `Identifier` |
| `TypeBool`, `TypeChar` | Remove — `Bool`/`Char` become `Identifier` |
| `TypeString`, `TypeData` | Remove — `String`/`Data` become `Identifier` |
| `TypeVoid` | Keep — `void` is a keyword, not an identifier |
| `TypeSigned`, `TypeSgn`, `TypeUnsigned`, `TypeUSgn` | Remove — these are identifier aliases |
| `TypeI8` through `TypeI64` | Remove — `i8`..`i64` become `Identifier` |
| `TypeU8` through `TypeU64` | Remove — `u8`..`u64` become `Identifier` |
| `TypeF32`, `TypeF64`, `TypeDouble` | Remove — aliases become `Identifier` |

Total tokens removed: ~26

The `TypeVoid` token stays because `void` is a Rust keyword and has special parser handling (`Type::Void`). All other type names become regular `Identifier` tokens, which `parse_type_inner` maps to `Type::Custom(name)`. The type resolution then happens entirely in the TypeUniverse.

---

*End of plan. Ready for review.*
