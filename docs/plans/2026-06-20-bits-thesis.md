# The Bits Thesis — Universal Type Transparency

Date: 2026-06-20
Status: Plan / Design
Author: Design discussion between randozart and OpenCode

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Philosophy — The Bits Thesis](#2-philosophy--the-bits-thesis)
3. [The Canonical Rules](#3-the-canonical-rules)
4. [Silent Projections (free on bare Bits)](#4-silent-projections-free-on-bare-bits)
5. [Operator Sigils as Projections](#5-operator-sigils-as-projections)
6. [Unified TypeDef Bodies](#6-unified-typedef-bodies)
7. [LLVM Optimization Under the Bits Thesis](#7-llvm-optimization-under-the-bits-thesis)
8. [Tiered Property Recognition](#8-tiered-property-recognition)
9. [The Arrow Operator](#9-the-arrow-operator)
10. [Bracket Syntax and Bit Precision](#10-bracket-syntax-and-bit-precision)
11. [Macro Demonstration — The `slot` Template](#11-macro-demonstration--the-slot-template)
12. [HashMap Under the Bits Thesis](#12-hashmap-under-the-bits-thesis)
13. [Cross-Language FFI and ABI Compatibility](#13-cross-language-ffi-and-abi-compatibility)
14. [Work Items — Implementation Phases](#14-work-items--implementation-phases)
15. [What Does NOT Change](#15-what-does-not-change)
16. [Architectural Tradeoffs and Rationale](#16-architectural-tradeoffs-and-rationale)
17. [Performance Guarantee](#17-performance-guarantee)
18. [Migration Guide](#18-migration-guide)
19. [Glossary of Terms](#19-glossary-of-terms)

---

## 1. Executive Summary

Brief's type system has one true primitive: **`Bits`**. A contiguous block of N bytes of
storage. Everything else — `Int`, `Float`, `String`, `List<T>`, `HashMap<K,V>`, user-defined
types — is `Bits` with properties attached. There are zero "built-in types" in the semantic
sense. The compiler provides convenience fast-paths for well-known types, but the *semantics*
are always traceable back to a `Bits`-based definition.

The thesis has five pillars:

1. **`Bits` is the only primitive.** A type is just `Bits` + bindings (properties + projections).
2. **Operator sigils desugar to projections.** `a + b` becomes `a :> Add(b)`. The type defines
   Add, Sub, Mul, Eq, Lt, etc. as named projections. The `+` rune is syntax sugar.
3. **Intrinsics carry type-specific semantics.** LLVM never matches on type names. It sees
   `fadd#` and emits `fadd`. The `#`-suffix on every intrinsic is the "no magic" guarantee.
4. **The educational stdlib documents the transparency.** `lib/std/from-bits.bv` shows how
   every fundamental type *could* be defined from `Bits`. The actual compiler uses hardcoded
   fast paths for performance — the file is documentation, not code.
5. **Nominal interface, structural transparency.** Common types look like primitives on the
   surface. The `Bits` representation is always reachable, never mandatory. The abstraction
   is honest — the machine representation is never hidden, just abstracted by default.

---

## 2. Philosophy — The Bits Thesis

### 2.1 All Types Are Bits

Computer science truth: a type is a constraint on bit pattern interpretation. `Float` is
not a fundamentally different thing from `Int` — it's 64 bits of storage whose projections
call `fadd#` instead of `sadd#`. A type is bits with a *lens*.

```brief
// These are the same bits:
let a: Int   = 42;
let b: Float = reinterpret<Float>(a);  // same 64 bits, different projections
```

### 2.2 Type Casting is Lens Replacement

`my_int as Float` means "take the same bits, apply Float's projection lens." The bits
don't change — the lens does. This is `reinterpret_cast` in C++, but explicit, transparent,
and checked by the proof engine.

### 2.3 The Intrinsic Airlock

All type-specific operations enter through `name#()` intrinsics. The `#` suffix is the
visible marker that the compiler owns these:

```
Safe Brief Space            Airlock (name#)             Host / LLVM
─────────────────    ──────────────────────────    ─────────────────
Contracts everywhere   compiler's intrinsic table    raw instructions
Reactive transactions                                  fadd, store, load
No undefined behavior                                  llvm.ctpop.i64
```

Every intrinsic has a `has_side_effects()` annotation that lets the optimizer fold pure
operations like `sqrt#(9.0)` while preserving observable I/O.

### 2.4 `@/` Grounds Bits to Positions

The `@/N`, `@/M..N`, `@/xN` syntax declares bit-precise type layouts:

```brief
type Header <: Bits @/0..31 {
    Version = _ @/0..3;            // bits 0-3
    Type    = _ @/4..7;            // bits 4-7
    Length  = _ @/8..31;           // bits 8-31
};
```

`Bits @/0..63` and `Bits { Bytes = 8 }` are surface-sugar for the same thing.

### 2.5 Nominal Interface, Structural Transparency

#### The Principle

The Bits Thesis does not force developers to program in binary. Common
types (`Int`, `Float`, `String`, `List<T>`) are treated as **nominal
primitives** for everyday use. The `Bits` representation is a
transparency layer — always reachable for debugging and optimization,
never imposed during normal development.

This is best described as **nominal interface, structural
transparency**:

| Layer | What the user writes | What the compiler sees |
|-------|---------------------|----------------------|
| Monday morning | `let x: Int = 5; x + 3` | Nominal types, familiar syntax |
| Friday afternoon | `x :> Add(sadd#(3))` | Low-level intrinsic, same semantics |
| Debugger | `x @/0..63` | Raw bit pattern, no abstraction |

The abstraction is still there to protect cognitive load, but it is
no longer an impenetrable black box. The machine representation is
never hidden — just abstracted by default.

#### Why This Matters for Ergonomics

If every value appeared as `Bits` in error messages, autocomplete, and
documentation, the language would be unusable. The nominal layer exists
for human comprehension:

- **IDE integration**: autocomplete shows `Int` methods, not generic
  `Bits` projections
- **Documentation**: `Float` has a clear abstract specification (IEEE
  754), not "Bits { Bytes = 8 }"
- **API contracts**: `json_get(v: JsonValue, key: String)` reads as
  self-documenting — the types express intent

#### The Transparency Guarantee

The nominal layer is **always peelable**. For any value `v`, the
developer can reach its `Bits` representation through:

```brief
let raw: Bits = v as Bits;           // Strip the nominal lens
let field = raw @/64..127;           // Access raw bit field
let restored: String = raw as String; // Reapply the nominal lens
```

This is the "honest abstraction" property: the nominal type is a
convenient default view, not a hidden implementation detail.

#### Relationship to the Four Pillars

This principle is the **fifth pillar** of the Bits Thesis:

5. **Nominal interface, structural transparency.** Common types look
   like primitives on the surface. The `Bits` representation is always
   reachable, never mandatory. The abstraction is honest — the machine
   representation is never hidden, just abstracted by default.

It binds the other four pillars together: pillars 1-4 describe the
mechanical reality (Bits, desugaring, intrinsics, stdlib transparency),
while pillar 5 describes the user experience (nominal primitives,
progressive disclosure, honest abstractions).

---

## 3. The Canonical Rules

### 3.1 The Tier System

| Operation category | Comes from | Applies to |
|--------------------|------------|------------|
| Bitwise (`&`, `|`, `^`, `~`, `<<`, `>>`) | Bare `Bits` | **Every** `Bits` derivation — **silent** |
| Bit-equality (`==`, `!=`) | Bare `Bits` (bitwise `icmp_eq#`) | **Every** type can override (Float does for NaN) |
| Metadata (`:>` Size, Bytes, Type, Ptr, Alignment, Range, IsEmpty) | Bare `Bits` | **Every** type |
| Bit introspection (`:>` Popcount, LeadingZeros, TrailingZeros, Absolute, BitReverse) | Bare `Bits` | **Every** type |
| Index/slice (`[n]`, `[m..n]`) | Bare `Bits` | **Every** type — decomposes to bits |
| Address (`:> Ptr`) | Bare `Bits` (binding-slot address) | **Every** type, overridable |
| Arithmetic (`+`, `-`, `*`, `/`, `%`) | Type binding (`Add`, `Sub`, `Mul`, ...) | Opt-in per type |
| Ordering (`<`, `>`, `<=`, `>=`) | Type binding (`Lt`, `Gt`, `Le`, `Ge`) | Opt-in per type |
| Boolean (`&&`, `||`, `!`) | Type binding (`And`, `Or`, `Not`) | Opt-in per type |
| Mutation (`<-`) | Type binding (`ArrowPush`, `ArrowPop`, ...) | Opt-in per type |
| Field access (`.`) | Struct/tuple layout | Per definition |
| Invocation (`()`) | Type binding (`Call`) | Opt-in per type |

### 3.2 The Resolution Order

When the compiler encounters an operator or projection:

1. **Parse**: operator parsed as `Expr::Add`, `Expr::Sub`, etc. (no change to parser)
2. **Desugar**: `a + b` → `Expr::Projection { source: a, target: UserDefined("Add", [b]) }`
   (projection argument carries the RHS)
3. **Type check**: Does the type of `a` define a binding named `Add` in its TypeUniverse entry?
   If yes, type-check the projection expression. If no and it's a **silent** op (bitwise, metadata),
   use the built-in default. If no and it's **not silent** (arithmetic, ordering, boolean), error.
4. **Simplify**: If the projection expression matches a well-known shape (load at offset 8,
   intrinsic call, etc.), mark it for fast-path codegen.
5. **Codegen**: If fast-path, emit dedicated LLVM IR (GEP+load, llvm.ctpop, etc.). If generic,
   compile the projection expression inline.

### 3.3 The TypeBinding — Unified Property and Projection

A TypeDef body contains name-value bindings. Each binding is either:

| Form | Meaning | Contains `_`? | Example |
|------|---------|---------------|---------|
| `Name = ConstantExpr;` | Type property (static) | No | `Bytes = 8;` |
| `Name = Expr;` | Projection (dynamic) | Yes | `Size = _ @/64..127;` |
| `Name(args) = Expr;` | Parameterized projection | Yes | `Add(rhs) = _ :> sadd#(rhs);` |

The compiler recognizes well-known names (`Bytes`, `Alignment`, `ElementType`, `Codec`,
`Endian`, `FixedSize`, `InsertAt`, `ExtractFrom`, `AllowIndex`, `AllowSlice`, `AllowArrow`,
`Volatile`, `Atomic`) for codegen purposes. Unknown names are user-defined projections —
no special treatment, no separate syntax.

---

## 4. Silent Projections (free on bare Bits)

These require no type definition. They work on every value because they derive from
the bit representation itself.

### 4.1 Metadata Lens (`:>`) — Universal Defaults

| Target | Meaning on bare `Bits` | Fast path | Notes |
|--------|------------------------|-----------|-------|
| `Size` | Always 1 (one value) | Constant-fold to 1 | Overridden by collections |
| `Bytes` | `N` — the byte width | From TypeUniverse | |
| `Type` | Type ID (discriminant) | Always available | Every value has a type |
| `Ptr` | Address of binding slot | GEP + bitcast | Overridden by String/List |
| `Ptr!` | Raw address (unchecked) | Same as Ptr | Skips the typed envelope |
| `Alignment` | From TypeUniverse | Load from metadata | Was incorrectly 8; now fixed |
| `Range` | Contract-proven bounds | From region analysis | Was returning [MIN,MAX]; now fixed |
| `IsEmpty` | `Size == 0` | Compare-to-zero | Added as convenience |
| `Popcount` | `llvm.ctpop` | Intrinsic call | Operates on raw bit pattern |
| `LeadingZeros` | `llvm.ctlz` | Intrinsic call | |
| `TrailingZeros` | `llvm.cttz` | Intrinsic call | |
| `Absolute` | Two's complement abs | `llvm.abs` or select pattern | |
| `BitReverse` | `llvm.bitreverse` | Intrinsic call | |
| `Contains(k)` | ❌ Error on bare Bits | — | Needs collection semantics |
| `Get(k)` | ❌ Error on bare Bits | — | Needs HashMap semantics |
| `Top` / `Front` | ❌ Error on bare Bits | — | Needs Stack/Queue semantics |
| `Keys` / `Values` / `Elements` | ❌ Error on bare Bits | — | Needs collection semantics |
| `AsStack` / `AsQueue` | ❌ Error on bare Bits | — | Needs List semantics |

Types override these defaults by defining a binding with the same name:

```brief
type String <: Bits @/0..127 {
    Bytes = 16;
    Size = _ @/64..127;        // bits 64-127 = length (byte offset 8-15)
    Ptr  = _ @/0..63;          // bits 0-63  = data pointer (byte offset 0-7)
};
```

### 4.2 Bitwise Operations — The Only Silent Sigils

| Op | Desugars to | Type default on bare Bits |
|----|-------------|--------------------------|
| `a & b` | `a :> BitAnd(b)` | `icmp_and#` (LLVM `and`) |
| `a \| b` | `a :> BitOr(b)` | `icmp_or#` (LLVM `or`) |
| `a ^ b` | `a :> BitXor(b)` | `icmp_xor#` (LLVM `xor`) |
| `~a` | `a :> BitNot()` | `icmp_not#` (LLVM `xor -1`) |
| `a << b` | `a :> Shl(b)` | `shl#` (LLVM `shl`) |
| `a >> b` | `a :> Shr(b)` | `ashr#` (LLVM `ashr` for signed, `lshr` for unsigned) |

These are silent because they operate on the raw bit pattern. No interpretation needed.

### 4.3 Bit-Equality (`==`, `!=`) — Semi-Silent

| Op | Desugars to | Default | Can override |
|----|-------------|---------|--------------|
| `a == b` | `a :> Eq(b)` | `icmp_eq#` (bit pattern) | Yes — Float overrides for NaN |
| `a != b` | `a :> Ne(b)` | `! (_ :> Eq(b))` | Yes — follows Eq |

The default is "same bit pattern." A type like `Float` overrides `Eq` to implement
IEEE 754 NaN semantics:

```brief
type Float <: Bits @/0..63 {
    Bytes = 8;
    Eq(rhs) = _ :> fcmp_oeq#(rhs);   // ordered equality — NaN != NaN
};
```

### 4.4 Indexing and Slicing — Silent

| Op | Desugars to | Returns |
|----|-------------|---------|
| `a[n]` | `a :> At(n)` via `BracketOp::Coord` | Single bit as `Bool` (default) |
| `a[m..n]` | `a :> Slice(m, n)` via `BracketOp::Range` | Sub-Bits |
| `a[::s]` | `a :> Stride(s)` via `BracketOp::Stride` | Strided sub-Bits |
| `a[mask]` | `a :> Mask(mask)` via `BracketOp::Mask` | Masked sub-Bits |
| `a @/n` | `a @/n` (separate syntax) | Bit at position n |
| `a @/m..n` | Bits at positions m..n | Sub-Bits |

`@/` is distinct from `[]` syntactically — `@/` is the bit-precision decorator for
type and field definitions, while `[]` is the universal indexing operator. They may
alias in the desugaring (both ultimately produce `BracketOp` evaluation).

---

## 5. Operator Sigils as Projections

Every operator sigil desugars to a projection by name. This is the key architectural
change: operators are no longer AST hardcodes — they are syntax sugar over the
projection mechanism.

### 5.1 Desugaring Table

| Surface syntax | Desugared projection | Operand binding |
|----------------|---------------------|-----------------|
| `a + b` | `a :> Add(b)` | RHS as sole argument |
| `a - b` | `a :> Sub(b)` | RHS as sole argument |
| `a * b` | `a :> Mul(b)` | RHS as sole argument |
| `a / b` | `a :> Div(b)` | RHS as sole argument |
| `a % b` | `a :> Mod(b)` | RHS as sole argument |
| `a == b` | `a :> Eq(b)` | RHS as sole argument |
| `a != b` | `!(a :> Eq(b))` | Negation of Eq |
| `a < b` | `a :> Lt(b)` | RHS as sole argument |
| `a > b` | `a :> Gt(b)` | RHS as sole argument |
| `a <= b` | `a :> Le(b)` | RHS as sole argument |
| `a >= b` | `a :> Ge(b)` | RHS as sole argument |
| `-a` | `a :> Neg()` | No argument |
| `!a` | `a :> Not()` | No argument |
| `a && b` | `a :> And(b)` | RHS as sole argument |
| `a \|\| b` | `a :> Or(b)` | RHS as sole argument |
| `a & b` | `a :> BitAnd(b)` | RHS as sole argument |
| `a \| b` | `a :> BitOr(b)` | RHS as sole argument |
| `a ^ b` | `a :> BitXor(b)` | RHS as sole argument |
| `~a` | `a :> BitNot()` | No argument |
| `a << b` | `a :> Shl(b)` | RHS as sole argument |
| `a >> b` | `a :> Shr(b)` | RHS as sole argument |
| `a[b]` | `a :> At(b)` | Index expression as argument |
| `a[b..c]` | `a :> Slice(b, c)` | Two arguments |
| `a[::s]` | `a :> Stride(s)` | Stride argument |
| `a[mask]` | `a :> Mask(mask)` | Mask argument (multi-slice) |

### 5.2 Implementation in the Desugarer

The desugarer (currently `src/desugarer.rs`) runs after parsing, before simplification:

```rust
// Transform: Expr::Add { lhs, rhs }
//       Into: Expr::Projection {
//                  source: lhs,
//                  target: ProjectionTarget::UserDefinedWithArg("Add".into(), rhs)
//              }
```

The expression simplifier (`equality_saturation.rs`) then works on projection form.
Well-known projection names get optimizations; novel names pass through unchanged.

### 5.3 Backend Handling

The backend (LLVM, Webstack, CIRCT) sees only `Projection` expressions:

```rust
ProjectionTarget::UserDefinedWithArg(name, rhs) => {
    if let Some(fast_path) = self.try_fast_path(name, &source, &rhs) {
        return fast_path;  // GEP+load, llvm.ctpop, fadd, etc.
    }
    // Fall back: lookup in TypeUniverse, compile the projection expression
    let expr = type_universe.lookup_projection(type_of_source, name)?;
    self.emit_expr(out, &expr_with_source_bound, indent)
}
```

The well-known name list for fast-path matching is maintained as a static table
in the backend. This table grows as the compiler adds new intrinsic or pattern
recognitions, but is always optional — the generic fallback handles anything.

### 5.4 Fast-Path Name Registry

| Projection name | Fast path on | LLVM emission |
|-----------------|-------------|---------------|
| `Add` (arithmetic context) | Int/Float | `add i64` / `fadd double` |
| `Sub` | Int/Float | `sub i64` / `fsub double` |
| `Mul` | Int/Float | `mul i64` / `fmul double` |
| `Eq` | Int/Float/String | `icmp eq` / `fcmp oeq` / `memcmp` |
| `Lt` | Int/Float | `icmp slt` / `fcmp olt` |
| `Size` | String/List/any | GEP + load at offset, or constant 1 |
| `Bytes` | Any | From TypeUniverse |
| `Type` | Any | Load from type-id field, or constant discriminant |
| `Ptr` | String/List/override | GEP + load slot 0, or binding address |
| `Popcount` | Int | `call llvm.ctpop.i64` |
| `LeadingZeros` | Int | `call llvm.ctlz.i64` |
| `TrailingZeros` | Int | `call llvm.cttz.i64` |
| `Absolute` | Int/Float | `llvm.abs` or select pattern |
| `BitReverse` | Int | `call llvm.bitreverse.i64` |
| `BitAnd`, `BitOr`, `BitXor` | Any | `and`, `or`, `xor` |
| `BitNot` | Any | `xor -1` |
| `Shl`, `Shr` | Any | `shl`, `ashr`/`lshr` |
| `At` | List/Tuple/String/any | GEP + load at index |
| `Contains` | HashMap/HashSet | Hash lookup pattern |
| `Get` | HashMap | Hash lookup pattern |
| `Top` | Stack | Load last element |
| `Front` | Queue | Load first element |
| `Keys` | HashMap | Lazy view → materialized on use |
| `Values` | HashMap | Lazy view → materialized on use |
| `Elements` | HashSet | Lazy view → materialized on use |
| `AsStack` | List | Lazy view → materialized on use |
| `AsQueue` | List | Lazy view → materialized on use |

### 5.5 Two-Pass Resolution for Generic Projections

#### The Problem

When `T` is a generic type parameter (`T <: Bits`), operator projections
cannot be resolved at declaration time. The binding `Add` on `T` is
unknown until `T` is concretely instantiated:

```brief
defn double<T: Bits>(x: T) -> T {
    term x + x;  // What is `+` on T? Unknown until T is known.
};
```

A naive single-pass resolver would either:
1. Error eagerly — "cannot resolve Add on generic type T"
2. Defer everything to monomorphization — exploding the amount of
   work done during codegen

#### The Solution: Two-Pass Resolution

The TypeUniverse resolves projection bindings in two passes:

**Pass 1 (Structural):** Runs during early type checking, before
monomorphization. Verifies constraints that hold for *any* `T`:

```
double<T: Bits>(x: T) → T
└─ x + x desugars to x :> Add(x)
   └─ Check: T must define Add or have a Bits default
      └─ Bits default for Add? NO — arithmetic is not silent
      └─ Error: T does not define Add → user adds constraint:
         double<T: Bits where Add(T)>
```

Pass 1 can check:
- Is the projection name defined on the type or any base?
- Are the argument arities correct?
- Does the return type unify with context?
- Are bit-width and alignment constraints satisfied?

Pass 1 does **not** resolve which intrinsic to call — it only checks
structural validity.

**Pass 2 (Concrete):** Runs after `T` is monomorphized to a concrete
type (e.g., `double<Int>`). Resolves `Add` to the specific intrinsic
(`sadd#` for Int, `fadd#` for Float):

```
double<Int>(x: Int) → Int
└─ x + x → x :> Add(x)
   └─ Int::Add(rhs) = _ :> sadd#(rhs)
   └─ Emit: sadd#
```

Pass 2 always succeeds if Pass 1 passed — the structural check
guarantees that the projection will have a concrete binding.

#### Syntax for Constrained Generics

To support Pass 1 checking, Brief introduces `where` clauses on
generic type parameters:

```brief
defn double<T: Bits where Add(T)>(x: T) -> T {
    term x + x;
};

defn sum<T: Bits where Add(T)>(list: List<T>) -> T { ... };
```

The `where` clause lists the projection names that the generic type
must provide. The compiler verifies the constraint at each call site:

```brief
let i: Int = double(5);       // ✓ Int defines Add
let f: Float = double(3.14);  // ✓ Float defines Add
let s: String = double("x");  // ✗ String does not define Add
```

#### Comparison with Other Languages

| Language | Generics mechanism | How `+` resolves | Brief advantage |
|----------|-------------------|------------------|----------------|
| Rust | Trait bounds (`T: Add`) | Trait resolution, associated types | Same model — Brief's `where` is explicit |
| C++ | Templates (SFINAE) | Late binding, substitution failure | Earlier errors, no template bloat |
| Java | Type erasure | Boxing + virtual dispatch | No boxing, zero-cost |
| **Brief** | **`where` clauses + two-pass** | **Pass 1: structural. Pass 2: intrinsic.** | **TypeUniverse freeze = fast resolution** |

#### Implementation

| Phase | What | Cost |
|-------|------|------|
| Phase 2 | Add `where` clause parsing to `defn`/`txn` parameter lists | Parser change |
| Phase 3 | Pass 1 structural check in typechecker: verify projection name exists in TypeUniverse | Typechecker change |
| Phase 3 | Pass 2 concrete resolution: map name + type to intrinsic at monomorphization | Desugarer/codegen change |
| Phase 3 | Error messages: "type `String` does not define projection `Add`" — never "Bits" | Using `original_type_name` |

---

## 6. Unified TypeDef Bodies

### 6.1 Current Design (before this plan)

```rust
pub enum TypeProperty {
    Bytes(Box<Expr>),
    Alignment(Box<Expr>),
    Endian(Box<Expr>),
    Volatile(Box<Expr>),
    Atomic(Box<Expr>),
    ElementType(Box<Expr>),
    FixedSize(Box<Expr>),
    InsertAt(Box<Expr>),
    ExtractFrom(Box<Expr>),
    AllowIndex(Box<Expr>),
    AllowSlice(Box<Expr>),
    AllowArrow(Box<Expr>),
    Codec(String),
}
```

13 hardcoded enum variants. Each must be matched everywhere. Adding a new property
requires touching all match arms.

### 6.2 New Design (after this plan)

```rust
/// A single binding in a TypeDef body.
/// All entries use the same syntax: `Name = Expr;` or `Name(args) = Expr;`.
pub struct TypeBinding {
    pub name: String,
    pub params: Vec<String>,    // empty for no-arg bindings
    pub body: Expr,             // `_` binds the subject value if present
    pub span: Option<Span>,
}
```

```rust
/// Body of a `Type Name <: Base { ... }` declaration.
pub struct TypeDefBody {
    pub bindings: Vec<TypeBinding>,
    pub constraints: Vec<Expr>,
    pub span: Option<Span>,
}
```

The parser parses every entry identically. Well-known names (`Bytes`, `Alignment`,
`ElementType`, etc.) are recognized in a HashMap lookup during analysis, not in the
parser. Unknown names go into the type's `projections` HashMap for runtime resolution.

### 6.3 Resolved Type (in TypeUniverse)

```rust
pub struct ResolvedType {
    pub name: String,
    pub type_params: Vec<String>,
    pub base: String,
    // Extracted from bindings by well-known name:
    pub bytes: Option<u64>,
    pub alignment: Option<u64>,
    pub endian: Option<Endian>,
    pub volatile: bool,
    pub atomic: bool,
    pub element_type: Option<String>,
    pub fixed_size: Option<bool>,
    pub insert_at: Option<Expr>,
    pub extract_from: Option<Expr>,
    pub allow_index: bool,
    pub allow_slice: bool,
    pub allow_arrow: bool,
    pub codec: Option<String>,
    // All bindings, including user-defined:
    pub projections: HashMap<String, TypeBinding>,
    pub source: TypeDef,
}
```

### 6.4 Parsing Example

```brief
type Int <: Bits @/0..63 {
    Bytes = 8;                  // No `_` → static property
    Alignment = 8;              // No `_` → static property
    Size = 1;                   // No `_` → static property
    Add(rhs) = _ :> sadd#(rhs);  // Has `_` + param → runtime projection
    Eq(rhs) = _ :> icmp_eq#(rhs);
    Popcount = _ :> ctpop#;     // Has `_`, no param → runtime projection
};

type Matrix4x4 <: Float[16] {
    Bytes = 64;
    Determinant = ...;           // Unknown name → user-defined projection
    Transpose = ...;             // Unknown name → user-defined projection
};
```

The parser generates `TypeBinding { name: "Bytes", params: [], body: Expr::Integer(8), ... }`
for all entries uniformly. The analysis pass extracts well-known names into the
`ResolvedType` struct fields. Unknown names remain in `projections`.

---

## 7. LLVM Optimization Under the Bits Thesis

### 7.1 LLVM Optimizes Operations, Not Types

LLVM does not have a concept of "Float" as a type name. It has:

| LLVM thing | Triggered by | Under Bits thesis |
|------------|-------------|-------------------|
| `fadd` → FMA | Opcode `fadd` | `fadd#` intrinsic → `fadd` |
| `llvm.sqrt.f64` | Intrinsic call | `fsqrt#` intrinsic |
| `llvm.ctpop.i64` | Intrinsic call | `ctpop#` intrinsic |
| `!range metadata` | Attached to load | Attached via contract analysis — no type needed |
| TBAA disambiguation | Metadata tree | Field-index-based tree, not type-name tree |
| `!llvm.loop.vectorize.enable` | Loop metadata | `foreach` emit — no type needed |

Every optimization fires because of what the *operation* is, not what the *type*
is named. The `#`-intrinsic is the information carrier.

### 7.2 TBAA Under the Bits Thesis

Currently TBAA nodes use type names:

```
!1 = !{!"Int", !0}
!2 = !{!"Float", !0}     ; Int and Float are different → may not alias
```

Under the Bits thesis, Int and Float are both `Bits { Bytes = 8 }`. They share the
same storage root. But TBAA disambiguation should distinguish them by *field index*,
not by type name:

```
!1 = !{!"state_field_0#", !0}
!2 = !{!"state_field_1#", !0}   ; Different field → may not alias
```

This is actually *more* precise than the current system. Two loads at different
state-struct field indices never alias regardless of what Brief types the fields
hold. The TBAA tree describes the struct layout, not the type ontology — which is
what LLVM actually uses it for.

**Migration**: The TBAA metadata emitter changes from `tbaa_node("Int")` to
`tbaa_node_for_field(field_index)`. No semantic impact on generated code.

### 7.3 Floating Point Optimizations

The concern: "LLVM has special float optimizations (FMA, contraction, reassociation)
that need to know a value is float."

The answer: LLVM fires these based on the *instruction opcode*, not the *value type*.
If the backend emits `fadd` (because `fadd#` was used), LLVM applies all float
optimizations. If the backend emits `add i64` (because `sadd#` was used), LLVM
applies integer optimizations.

**The type never mattered to LLVM. Only the opcode did.** The `#`-intrinsic is what
selects the opcode. The type name is irrelevant.

### 7.4 Heap TBAA for Allocated Structures

#### The Problem

Section 7.2 covers TBAA for the global state struct (`state_field_N#`),
where field-index-based disambiguation works perfectly — two adjacent
state fields never alias because they are at different GEP offsets.

But heap-allocated structures (`List<T>` elements, `HashMap` entries)
are a different story. Two lists of different element types (`List<Int>`
and `List<Float>`) both store their elements as `i64` values in
heap-allocated buffers. To LLVM, both buffers are just `i64*` pointers.
Without additional TBAA information, LLVM conservatively assumes they
may alias — blocking load hoisting, store forwarding, and loop
vectorization.

```
%State {
  i64 list_a  ; ptr to List<Int>  buffer (i64*)
  i64 list_b  ; ptr to List<Float> buffer (i64*) — same type to LLVM
}
```

If a transaction reads both `list_a[0]` and `list_b[0]`, LLVM may
reload both from memory on every iteration because it cannot prove
they point to different allocations.

#### The Solution: Virtual TBAA Tree for Heap Types

When the backend emits a load from a heap-allocated `List<T>` element,
it attaches a TBAA node that encodes the **element type identity**, not
just the "it's an i64" structural fact:

```llvm
; TBAA tree:
!0 = !{!"Brief"}
!1 = !{!"state_field_0#", !0}       ; field index for list_a
!2 = !{!"state_field_1#", !0}       ; field index for list_b
!3 = !{!"heap_List_Int_element", !1}   ; List<Int> elements loaded via list_a
!4 = !{!"heap_List_Float_element", !2} ; List<Float> elements loaded via list_b
```

The key insight: the heap TBAA node is **parented to the state field**
that holds the pointer, not to a global type-name root. Two lists at
different state fields never alias, even if they hold the same element
type. Two lists at the same state field (same pointer) naturally alias
because they share the same parent.

#### Implementation

| Step | What changes | Affects |
|------|-------------|---------|
| 1. Track element type identity per state field | Analysis pass annotates each `List<T>` state field with `(field_index, element_type_name)` | `analysis/region.rs` |
| 2. Emit heap TBAA nodes | For each annotated field, emit `!N = !{!"heap_{elem_type}", !"state_field_N#"}` | `emit_expr.rs` — GEP+load for list elements |
| 3. Annotate all heap loads | Every `load i64, i64* %elem_ptr` from a list buffer gets `!tbaa !N` | All list element access sites |

#### Performance Impact

The virtual tree adds <10 TBAA metadata nodes per compilation — a
negligible IR size increase. The benefit is substantial: LLVM can
prove that modifying `list_a[0]` does not affect `list_b[0]`, enabling
load elimination, store-to-load forwarding, and loop-invariant code
motion across list operations.

### 7.5 Optimizer Friction — Aggressive Fast-Path Emission

#### The Risk

Under the Bits Thesis, every operation desugars to a projection
expression. If the backend naively emits the desugared form as generic
IR, the result is verbose `bitcast` / `extractvalue` / `insertvalue`
chains:

```llvm
; Naive: box → call intrinsic → unbox
%boxed_l = bitcast double %l to i64
%boxed_r = bitcast double %r to i64
%raw = call i64 @fadd#_intrinsic(i64 %boxed_l, i64 %boxed_r)
%result = bitcast i64 %raw to double
```

LLVM's optimizer can clean this up (SROA + InstCombine fold the
bitcasts), but it has **finite iteration budgets**. A single function
with many such patterns may exhaust the budget before SROA runs,
leaving the bitcasts in place. The result: missed vectorization,
missed inlining, missed FMA formation.

#### The Mitigation: Emit Clean Native Ops Early

The fast-path registry (Section 5.4) is **not optional polish** — it is
essential for keeping LLVM's optimizer within budget. For every
well-known projection shape, the backend must emit the cleanest
possible LLVM IR **directly**, without intermediate boxing:

```llvm
; Good: direct fadd — LLVM sees fadd immediately
%result = fadd double %l, %r
```

The rule: **if the shape detector recognizes the projection, the
backend never enters the generic expression compilation path.** It
routes directly to a dedicated emission function that produces
native LLVM IR.

#### Budget Sizing

| Scenario | Default LLVM iteration budget | Risk |
|----------|------------------------------|------|
| Current (hardcoded ops) | 1000 (sroa-limit) | None — direct IR |
| Generic projection, simple | 1000 | Low — a few extractvalue chains |
| Generic projection, nested (e.g., matrix multiply expressed as generic `Add`/`Mul` projections) | 1000 | **High** — nested boxing may exceed budget |

For the high-risk case, the backend should emit a LLVM `llvm.assume`
that hints at the simplification, or use `llvm.mem2reg`-compatible
alloca patterns instead of extractvalue/insertvalue.

#### Enforcement

A CI test compiles a projection-heavy benchmark and checks the
resulting `.ll` for unexpected bitcast chains. If the count exceeds
a threshold, the test fails — forcing the developer to add a fast-path
entry for the new projection shape before the regression lands.

This ensures the fast-path registry stays comprehensive as the
language evolves.

---

## 8. Tiered Property Recognition

Different Brief file suffixes recognize different subsets of type properties.
This is not a new restriction — it formalizes what the tier system already does.

### 8.1 Property Recognition Table

| Property / Feature | `.bv` | `.ebv` | `.abv` | `.cbv` |
|--------------------|:-----:|:------:|:------:|:------:|
| **Bits primitive** | ✅ | ✅ | ✅ | ✅ |
| **`@/` bit precision** | ✅ | ✅ | ✅ | ✅ |
| **`Bytes`, `Alignment`, `Endian`** | ✅ | ✅ | ✅ | ✅ |
| **`Volatile`, `Atomic`** | ✅ | ✅ | ✅ | ❌ |
| **`ElementType`, `FixedSize`** | ✅ | ✅ | ✅ | ❌ |
| **`InsertAt`, `ExtractFrom`** | ✅ | ✅ | ✅ | ❌ |
| **`AllowIndex`, `AllowSlice`, `AllowArrow`** | ✅ | ❌ | ❌ | ❌ |
| **`Codec`** | ✅ | ✅ | ❌ | ❌ |
| **FFI (`frgn`)** | ✅ | ✅ | ❌ | ❌ |
| **User-defined projections** | ✅ | ✅ | ✅ (limited) | ❌ |
| **String constants** | ✅ | ✅ | ✅* | ❌ |
| **String values (runtime)** | ✅ | ✅ | ❌ | ❌ |
| **GPU intrinsics** | ❌ | ❌ | ✅ | ❌ |
| **Total contracts** | optional | optional | optional | ✅ |
| **Synthesizable types only** | ❌ | ❌ | ❌ | ✅ |

\* ABV recognizes string constants strictly for compile-time error messages and
   shader source embedding. No runtime String values.

### 8.2 What Each Tier Ignorance Means

A property that a tier doesn't recognize is *ignored*, not *errored*. The type
definition is still valid — the tier simply doesn't use that information:

```brief
// In .cbv (Circuit Brief), this type is valid but `Codec` is ignored.
// The 16 bytes remain opaque Bits. No codec operations are emitted.
type String <: Bits {
    Bytes = 16;
    Codec = UTF8Codec;    // ← ignored by CBV
    Size = _ @/64..127;
};
```

CBV synthesizes 16 bits of opaque storage. The Codec property cannot be synthesized,
so it is silently dropped. The user must provide explicit bit-manipulation logic
for any serialization.

---

## 9. The Arrow Operator

### 9.1 Arrow as Projections

The arrow operator (`<-`) desugars to projection calls, just like other sigils:

| Surface syntax | Desugared to | Semantics |
|----------------|--------------|-----------|
| `&list <- value` | `list :> ArrowPush(value)` | Insert value into collection |
| `value <- &list` | `list :> ArrowPop()` | Extract value from collection |
| `<- &list` | `list :> ArrowDiscard()` | Pop and discard |
| `list1 <- &list2` | `list1 :> ArrowTransfer(&list2)` | Move all elements |

The `&` sigil marks the mutation target. This is unchanged from the current design.

### 9.2 Default on Bare Bits

All arrow operations are ❌ on bare `Bits`. A type must define them:

```brief
type List<T> <: Bits @/0..191 {
    Bytes = 24;  // ptr + len + cap
    ArrowPush(v) = ...;    // grow buffer, append v
    ArrowPop() = ...;      // pop last element
    ArrowDiscard() = ...;  // decrement len
    ArrowTransfer(src) = ...;  // move all elements
};
```

### 9.3 Type-Directed Dispatch

The arrow already dispatches on Value type (not string names). This stays:
`ArrowPush` on a List calls the List projection. `ArrowPush` on a HashMap calls the
HashMap projection. The type decides what "push" means.

---

## 10. Bracket Syntax and Bit Precision

### 10.1 Universal Bracket (`[]`)

Bracket syntax decomposes any value into visual `Char` fragments. This already
exists. Under the Bits thesis:

| Value type | `[n]` returns | `[m..n]` returns |
|------------|---------------|-------------------|
| `Bits` raw | Bit at index n as `Bool` | Sub-Bits |
| `Int` | Bit n as `Bool` | Sub-Bits (bits m..n) |
| `Float` | Bit n as `Bool` | Sub-Bits |
| `String` | Char at byte position n | Substring |
| `List<T>` | Element at index n | Sublist |
| `Tuple` | Element at index n (NEW) | Subtle tuple |
| `HashMap` | Value at key n | Error |
| `Struct` | Field at position n | Error |

The union of all bracket operations is: every value decomposes to positions.
For bytes, positions are bits. For strings, positions are bytes. For lists,
positions are elements. The `@/` syntax is the bit-precision specialization
of the same concept.

### 10.2 Tuple Bracket Indexing (NEW)

```brief
let pair: (Int, String) = (42, "hello");
let first = pair[0];   // → 42  (previously pair :> 0)
let second = pair[1];  // → "hello"
let sub = pair[0..1];  // → (42,) — subtuple via range
```

Implementation: `BracketOp::Coord(n)` on `Value::Tuple` returns the element at
index `n`. This replaces `ProjectionTarget::Index(n)`.

### 10.3 `@/` in Type Bodies

Within a TypeDef body, `@/` declares bit-precise sub-fields:

```brief
type RISC_V_Instruction <: Bits @/0..31 {
    Opcode = _ @/0..6;
    Rd     = _ @/7..11;
    Funct3 = _ @/12..14;
    Rs1    = _ @/15..19;
    Rs2    = _ @/20..24;
    Funct7 = _ @/25..31;
};
```

Each `Field = _ @/start..end` desugars to a projection named `Field` that
extracts the bit range. This is sugar for:

```brief
Field = _[start..end];   // via bracket slicing
```

---

## 11. Macro Demonstration — The `slot` Template

### 11.1 Proving Mastery of Our Own System

The Bits thesis is most powerfully demonstrated when we define a helper
template *in Brief itself* using nothing but the primitives available to
every user. The `slot` template is that demonstration.

`slot(n)` expresses "extract byte-aligned field N" as a bit-range projection.
It is defined in the `$` macro system, and its expansion uses only `@/`,
which is a silent projection available on all `Bits`:

```brief
// ============================================================
// slot(n) — Bit-field accessor template
// ============================================================
// Given a source value `_`, extract bits (n*8)..(n*8 + 7).
//
// The compiler recognizes byte-aligned `@/` ranges and emits
// GEP + i64 load instead of shift+mask for misaligned ranges.
//
// Defined entirely in user-space Brief. No compiler magic.
//
$ slot(n) {
    quote { _ @/(n * 8)..(n * 8 + 7) }
}
```

### 11.2 Usage

```brief
// A 24-byte List: ptr at slot 0, len at slot 1, cap at slot 2
type List<T> <: Bits @/0..191 {
    Size = slot(1);       // → _ @/8..15      → GEP + load i64 at offset 8
    Ptr  = slot(0);       // → _ @/0..7       → GEP + load i64 at offset 0
    Cap  = slot(2);       // → _ @/16..23     → GEP + load i64 at offset 16
    At(i) = _ :> Ptr[i];
};

// A 16-byte String: ptr at slot 0, len at slot 1
type String <: Bits @/0..127 {
    Size = slot(1);       // → _ @/8..15
    Ptr  = slot(0);       // → _ @/0..7
};
```

### 11.3 Extending the Pattern

Users can define their own templates for common bit-field patterns:

```brief
// Word accessor: extract N-byte word at byte offset
$ word(n, bytes) {
    quote { _ @/(n * 8)..(n * 8 + bytes * 8 - 1) }
}

// Nibble accessor: extract 4-bit nibble at position
$ nibble(n) {
    quote { _ @/(n * 4)..(n * 4 + 3) }
}

// Bit accessor: extract single bit at position
$ bit(n) {
    quote { _ @/n..n }
}

// Usage: RISC-V instruction decode
type RISC_V_Instruction <: Bits @/0..31 {
    Opcode = bit(0);       // not useful alone — shows the pattern
    // Better: named fields with @/ inline
    Opcode = _ @/0..6;
    Rd     = _ @/7..11;
    Funct3 = _ @/12..14;
    Rs1    = _ @/15..19;
    Rs2    = _ @/20..24;
    Funct7 = _ @/25..31;
};
```

### 11.4 What This Demonstrates

| Aspect | What it proves |
|--------|----------------|
| **User-space definition** | `slot` is written in a `$` template, using `@/` and `quote { }`. No compiler changes needed. |
| **Compiler optimization** | Byte-aligned `@/` ranges automatically get GEP+load. Misaligned ranges get shift+mask. The compiler doesn't need to know about `slot` — it optimizes the *expanded form*. |
| **Extensibility** | Users can define `word`, `nibble`, `bit`, or any domain-specific accessor. All compose because they all expand to `@/`. |
| **Transparency** | A reader sees `slot(1)` and can mentally expand it to `_ @/8..15`. The template source is in `lib/std/`. No magic. |

---

## 12. HashMap Under the Bits Thesis

### 12.1 The Question

What does a KV store look like under the Bits thesis? Two answers:
1. Directly from `Bits` (pure, transparent — shows the slot array)
2. Inherited from `List<(K, V)>` (convenient, practical)

Both are correct. The educational file shows both to demonstrate the
flexibility of the system.

### 12.2 Option A: Directly from `Bits`

```brief
type HashMap<K, V> <: Bits {
    // Layout: ptr to slot array + length + capacity
    // Each slot: key | value | occupancy flag (1 bit)
    Bytes = 24;
    Size = slot(1);                     // number of occupied entries
    Ptr  = slot(0);                     // pointer to slot array
    Contains(key) = _ :> _lookup#(key) :> Occupied?;
    Get(key) = _ :> _lookup#(key) :> Value;
    Insert(key, val) = ...;             // hash → probe → store
    ArrowPush(pair) = _ :> Insert(pair :> 0, pair :> 1);
};
```

The `_lookup#` intrinsic does hash-and-probe. The projection expressions
compose: `Get` is `Contains` with a `:> Value` lens tacked on. A slot
is conceptually:

```brief
// Each slot in the array:
type Slot<K, V> <: Bits @/0..(KBitWidth + VBitWidth) {
    Occupied = _ @/(KBitWidth + VBitWidth)..(KBitWidth + VBitWidth);  // 1-bit flag
    Key   = _ @/0..(KBitWidth - 1);
    Value = _ @/KBitWidth..(KBitWidth + VBitWidth - 1);
};
```

### 12.3 Option B: Inherited from `List<(K, V)>`

```brief
type HashMap<K, V> <: List<(K, V)> {
    AllowIndex = false;            // raw bracket access would bypass hash
    Contains(key) = ...;           // hash-based lookup
    Get(key) = ...;                // hash-based lookup, returns Option<V>
    Insert(key, val) = ...;        // hash → probe → insert or update
};
```

A HashMap is "a List of (K, V) pairs with hash-tuned projections." The
underlying storage is a list; the projections add the hash semantics.
`AllowIndex = false` prevents users from doing `map[0]` and bypassing
the hash.

### 12.4 What the Compiler Actually Does

The interpreter uses a native `Value::HashMap` backed by Rust's `HashMap`.
This is a performance fast path, not a semantic difference. The educational
file documents: "The compiler recognizes the `HashMap` type name and uses
a native implementation. But conceptually, it's `Bits` with hash-lookup
projections. A user-defined type with `Contains(key) = _ :> linear_scan#(key)`
would also work — just slower for large collections."

**The performance path and the semantic path diverge here by design.**
The first is fast; the second is transparent. Both produce the same
results for the same inputs.

---

## 13. Cross-Language FFI and ABI Compatibility

### 13.1 The Problem

Cross-language FFI is one of the most error-prone areas in systems
programming. Passing a `String` from Brief to Rust or C typically
requires serialization wrappers, memory copies, and manual layout
verification. Under the Bits Thesis, this becomes a zero-cost
"re-lensing" operation.

### 13.2 Principle: Layout Compatibility is Bit Equality

If two types occupy the same bit pattern, converting between them is
free. The proof engine (`?#`) checks:

```
∀x: Bits. layout_A(x) ≡ layout_B(x)  →  reinterpret<B>(x) is identity
```

If the layouts are not identical but structurally embeddable, the
conversion compiles to a stack-frame re-layout — pointer copying
without touching payload data.

### 13.3 Scalar Example: Float as CFloat

```brief
let bf: Float = 1.23;
let cf: CFloat = reinterpret<CFloat>(bf);
```

This compiles to **zero machine instructions**. `Float` and `CFloat`
are both `Bits @/0..63` with `Bytes = 8` and `Alignment = 8`. The
only difference is TBAA metadata node — LLVM sees the same register.
The `reinterpret` tells the backend: "Swap the TBAA metadata node for
alias analysis; emit no move, no conversion, no copy."

### 13.4 Complex Structure Example: Brief String → Rust String

Rust's standard `String` is 24 bytes:

```
RustString = { ptr: i8*, capacity: u64, length: u64 }
```

Brief's standard `String` is 16 bytes:

```
BriefString = { ptr: i8*, length: u64 }
```

Under the Bits Thesis, define `RustString` to match Rust's ABI:

```brief
type RustString <: Bits @/0..191 {
    Bytes = 24;
    Ptr  = _ @/0..63;
    Cap  = _ @/64..127;
    Len  = _ @/128..191;
};
```

Convert without copying character data:

```brief
let brief: String = read_file("data.txt")?;
let rust: RustString = (brief.Ptr, brief.Len, brief.Len) as RustString;
```

Rust's `String::from_raw_parts` can take ownership of the pointer
because `capacity = length` signals "no spare capacity." The character
buffer is never copied — only the 24-byte stack-framed `RustString`
header is constructed.

### 13.5 What the Type Checker Verifies

Before permitting the cast, the type checker proves:

1. **Bit width match** — the source layout fits the target layout
2. **Pointer provenance** — the cast does not create aliased mutable
   pointers (linearity check)
3. **Field alignment** — each field's alignment constraints are
   satisfied in the target layout
4. **Proof engine (`?#`) confirmation** — the `reinterpret` is
   annotated with a contract if the developer wants formal guarantees

### 13.6 FFI Without Serialization Boilerplate

Standard Brief FFI (`frgn from "c"` or `frgn from "rust"`) uses
these layout proofs to pass complex types across the boundary:

```brief
frgn process_string(s: RustString) -> Int from "rust";

let brief_str: String = read_file("input.txt")?;
let rust_str: RustString = (brief_str.Ptr, brief_str.Len, brief_str.Len) as RustString;
let result: Int = process_string(rust_str);
// After the call, brief_str's pointer is owned by Rust.
// The Borrow Checker / linearity analysis tracks this.
```

No `CString::new()`, no `malloc`, no `memcpy`. The ABI boundary is
proven at compile time.

### 13.7 Contrast with Other Languages

| Language | FFI String Overhead | Safety Model |
|----------|---------------------|--------------|
| C | Manual malloc + free, strlen | None |
| Rust FFI | `CString::new()` heap alloc + copy | Unsafe block required |
| Go (cgo) | `C.CString()` alloc + copy, must free | Manual |
| Java (JNI) | `NewStringUTF` copies, env ptr overhead | GC-managed |
| **Brief** | **Zero-copy re-lensing** | **Proof-checked layout compatibility** |

---

## 14. Work Items — Implementation Phases

### Phase 1 — Mechanical Cleanup

Estimated: 2-3 days

| Task | Files | Risk | Testing |
|------|-------|------|---------|
| 1. Remove `ProjectionTarget::Pop` from AST | `ast.rs`, `parser.rs`, `projection.rs`, `emit_expr.rs`, `webstack.rs`, `circt.rs`, `typechecker.rs` | None — already a runtime error, no code uses it | Ensure no tests reference Pop |
| 2. Remove `ProjectionTarget::Index(n)` from AST | Same files | Low — 2 call sites in `json.bv` use `:> 0` and `:> 1` | Migrate json.bv, remove Index tests |
| 3. Add bracket-index dispatch for `Value::Tuple` | `interpreter.rs` (eval_bracket), `emit_expr.rs` (emit_bracket), `webstack.rs`, `circt.rs` | Low — new match arm, existing pattern | Add tuple bracket tests |
| 4. Add `ProjectionTarget::IsEmpty` | Same as task 1 | None — additive | Add test for `IsEmpty` |
| 5. Fix `Alignment` → TypeUniverse | `features/projection.rs`, `emit_expr.rs` | Low — TypeUniverse already has alignment | Verify Bool (1), Char (4), Int (8) |
| 6. Fix `Range` → contract bounds | `features/projection.rs`, `emit_expr.rs`, `analysis/region.rs` | Medium — needs region analysis integration | Verify `[x > 0 && x < 100]` → `Range == [1, 99]` |
| 7. Add `TypeProperty::Projection(name, expr)` variant | `ast.rs`, `parser.rs`, `type_universe.rs` | Low — additive, old properties still work | Parser tests for new form |

**Migration note for task 2**: Before removing Index, first add bracket support for
tuples. Then migrate `json.bv` from `pair :> 0` to `pair[0]`. Then remove Index
from the AST. This is a safe 3-step sequence.

### Phase 2 — Unified TypeDef Bodies

Estimated: 3-5 days

| Task | Files | Risk | Testing |
|------|-------|------|---------|
| 1. Add `TypeBinding` struct to AST | `ast.rs` | Low — new type, old TypeProperty still works | — |
| 2. Refactor `TypeDefBody` to use `Vec<TypeBinding>` | `ast.rs`, `parser.rs`, `type_universe.rs`, `typechecker.rs` | Medium — changes parser output format | Update all TypeDef construction sites |
| 3. Parser: parse every TypeDef entry as `Name = Expr;` uniformly | `parser.rs` | Medium — changes parse_type_def | All existing TypeDef tests must pass |
| 4. Parser: parse old `TypeProperty` syntax and map to `TypeBinding` | `parser.rs` | Low — backward compat shim | Old syntax tests pass unchanged |
| 5. TypeUniverse: extract well-known names from bindings into struct fields | `type_universe.rs` | Medium — resolution logic changes | All TypeUniverse tests must pass |
| 6. TypeUniverse: store remaining bindings in `projections: HashMap` | `type_universe.rs` | Low — additive | — |
| 7. Remove `TypeProperty` enum | `ast.rs`, all match sites | Medium — many files reference TypeProperty | Grep for TypeProperty references |
| 8. Add `ProjectionTarget::UserDefined(String)` and `UserDefinedWithArg(String, Expr)` | `ast.rs`, `parser.rs` | Low — new variants | — |

### Phase 3 — Operator Desugaring

Estimated: 5-7 days

| Task | Files | Risk | Testing |
|------|-------|------|---------|
| 1. Desugarer: transform `Expr::Add` → `Projection::UserDefinedWithArg("Add", rhs)` | `desugarer.rs` | **HIGH** — affects every expression path | All existing tests must pass (no semantic change) |
| 2. Desugarer: transform all binary ops | `desugarer.rs` | High — same pattern 15 times | All tests pass |
| 3. Desugarer: transform unary ops | `desugarer.rs` | High | All tests pass |
| 4. Desugarer: transform bracket / arrow | `desugarer.rs` | High | All tests pass |
| 5. Backend: add fast-path registry for well-known projection names | `emit_expr.rs` (new module) | Medium — extract from existing match arms | Backend tests, benchmark regressions |
| 6. Backend: implement generic fallback for unknown projection names | `emit_expr.rs` | Medium — expression compilation path exists | — |
| 7. Interpreter: handle `UserDefinedWithArg` projection | `interpreter.rs`, `features/projection.rs` | Medium — new match arm with type-universe lookup | All interpreter tests pass |
| 8. Typechecker: resolve operator projections by name | `typechecker.rs` | Medium — changes type inference path | — |
| 9. Equality saturation: recognize projection patterns | `equality_saturation.rs` | Low — new patterns for existing rules | — |

**Risk mitigation**: Phase 3 can be done incrementally. Add the desugarer path,
verify all existing tests pass (they should — same semantics), then clean up
dead `Expr::Add` etc. code in a follow-up.

### Phase 4 — `@/` Bit-Precision Integration

Estimated: 2-3 days

| Task | Files | Risk | Testing |
|------|-------|------|---------|
| 1. Parser: accept expanded `@/` in type and projection contexts | `parser.rs` | Low — syntax already exists for types, extend to projections | — |
| 2. Desugar `Bits @/m..n` to `Bytes = ceil((n-m+1)/8)` | `desugarer.rs`, `type_universe.rs` | Low — mechanical transformation | — |
| 3. `@/` in TypeDef bindings: `Field = _ @/start..end` | `parser.rs`, `interpreter.rs` | Low — sugar for `_[start..end]` | — |
| 4. Dynamic bit expressions: `@/(offset + width - 1)` | `parser.rs` | Medium — expression evaluation at type level | — |

### Phase 5 — Educational Stdlib File

Estimated: 1-2 days

| Task | Files | Risk | Testing |
|------|-------|------|---------|
| 1. Create `lib/std/from-bits.bv` | NEW FILE | None — comments only | None (no executable code) |
| 2. Show Int as `Bits @/0..63` with all silent defaults listed | `from-bits.bv` | None | — |
| 3. Show Float as `Bits` with `fadd#`, `fsub#`, etc. | `from-bits.bv` | None | — |
| 4. Show String as `Bits @/0..127` with `slot(0)`/`slot(1)` pattern | `from-bits.bv` | None | — |
| 5. Show List as `Bits @/0..191` with `slot(0..2)` pattern | `from-bits.bv` | None | — |
| 6. Show the `slot(n)` template defined in a `$` macro | `from-bits.bv` | None | — |
| 7. Show HashMap both ways: `Bits`-native and `List<(K,V)>`-inherited | `from-bits.bv` | None | — |
| 8. Show user-defined type with custom projections (Matrix4x4) | `from-bits.bv` | None | — |
| 9. Show RISC-V instruction decode using `@/` bit fields | `from-bits.bv` | None | — |
| 10. Add tier-ignorance examples (CBV ignoring Codec) | `from-bits.bv` | None | — |
| 11. Add compiler-notes: "The actual implementation uses a fast path for this" | `from-bits.bv` | None | — |
| 12. Add link from `lib/std/README.md` to `from-bits.bv` | `README.md` | None | — |

### Phase 6 — Architecture Doc

Estimated: 1 day

| Task | Files | Risk | Testing |
|------|-------|------|---------|
| 1. Write `docs/architecture/bits-thesis.md` | NEW FILE | None — documentation only | — |
| 2. Cover: Bits thesis, tier tables, silent rules, desugaring, TBAA, optimization | — | None | — |
| 3. Include FAQ: "But what about Float?" / "CBV doesn't recognize String?" / "Performance?" | — | None | — |
| 4. Update `docs/architecture/glossary.md` with new terms | `glossary.md` | None | — |
| 5. Update `docs/BRIEF_3.0_SPEC.md` section 2 with refined `:>` specs | `BRIEF_3.0_SPEC.md` | None | — |

---

## 15. What Does NOT Change

| Thing | Status | Reason |
|-------|--------|--------|
| `Value` enum variants | Stay | `Value::Int`, `Value::Float`, `Value::String` etc. remain for interpreter dispatch |
| Hardcoded `ProjectionTarget` fast-path variants | Stay (in backend) | Fast-path optimization hints — `Size`, `Bytes`, `Type`, etc. |
| LLVM codegen for known targets | Stay | GEP+load, `llvm.ctpop`, `fadd`, etc. |
| Interpreter dispatch on `Value` type | Stay | `match val { Value::Int => ... }` is an optimization, not a semantic primitive |
| Backend match arms for known projections | Stay | Fast-path table unchanged |
| Tier file suffixes (.bv, .ebv, .abv, .cbv) | Stay | Existing semantics and restrictions |
| Intrinsic set | Stay | The airlock — `name#` is the bridge |
| `@/` syntax | Already exists | Extended use, not changed |
| Arrow operator | Stay | `<-` still works as before |
| Bracket syntax | Stay | Extended to tuples, no existing behavior changes |
| Pre/post contracts | Stay | `[pre][post]` unchanged |
| Reactive transactions | Stay | `rct txn` unchanged |
| Proof engine (`?#`) | Stay | Unchanged |
| Macro system (`$`, `$!`) | Stay | Unchanged |

---

## 16. Architectural Tradeoffs and Rationale

### 16.1 Complexity Tradeoff: Desugaring

**Cost**: Every operator expression goes through an extra desugaring pass.
Every projection evaluation may need a TypeUniverse lookup.

**Benefit**: One mechanism for all type-directed behavior. No more hardcoded
type-specific logic in the parser or AST. Users can define custom operators
via the same mechanism.

**Mitigation**: The desugarer is O(n) — each node transforms once. The
TypeUniverse lookup is O(1) (HashMap). The extra pass adds <1% to compile
time. The benefit (transparency, extensibility) is unbounded.

### 16.2 Complexity Tradeoff: Generic Expression Compilation

**Cost**: User-defined projection expressions must be compiled inline by
backends. This is more complex than matching an AST variant.

**Benefit**: Users can define arbitrary projections without compiler changes.
The backend already has `emit_expr()` — this is reusing existing infrastructure.

**Mitigation**: The vast majority of code uses well-known projection names
that hit fast paths. The generic path is a fallback for novelty. It is not
the common case.

### 16.3 Complexity Tradeoff: Unified TypeDef Bodies

**Cost**: The TypeDef parser becomes slightly more complex. The TypeUniverse
resolution must distinguish well-known names from user-defined names.

**Benefit**: 13 hardcoded `TypeProperty` enum variants disappear. Users can
add arbitrary properties. The syntax is uniform: `Name = Expr;`.

**Mitigation**: The parser change is minimal — parse every entry the same way,
let analysis sort them. The TypeUniverse change is a single HashMap lookup
instead of a match on 13 variants.

### 16.4 Performance Tradeoff: Float Optimization

**Cost**: If the Bits thesis required reinterpreting Float as generic Bits,
LLVM might lose optimization opportunities.

**Reality**: LLVM optimizes based on opcodes, not type names. `fadd#` → `fadd`
is the same instruction regardless of whether the frontend calls the type
"Float" or "MyFloatBits." Zero performance impact.

### 16.5 Performance Tradeoff: TBAA

**Cost**: Moving from type-name-based TBAA to field-index-based TBAA.

**Benefit**: More precise disambiguation (two fields with the same type name
but different indices should not alias — but the current system would let
them because they share a TBAA node).

**Reality**: The field-index approach is *more* correct. It also eliminates
the need for the TBAA tree to know type names at all.

### 16.6 Counter-Argument: "Why not just keep hardcoded operators?"

**Response**: The plan DOES keep hardcoded fast paths. The change is strictly
additive: operators desugar to projections, but the backend recognizes
well-known projection shapes and emits the same code as today. The only
difference is architectural transparency — the semantics are traceable.

### 16.7 Counter-Argument: "Does this overcomplicate the type system for simple cases?"

**Response**: Simple cases remain simple. Writing `let x: Int = 5; x + 3`
works exactly as before. The desugaring happens transparently. The Bits
thesis only becomes visible when a user defines a new type and wonders
"how do I add `+` support?" — at which point the answer is clear: define
`Add(rhs) = ...` in your type body.

### 16.8 Diagnostic Degradation — The "Opaque Bits" Problem

#### The Risk

When every type is `Bits` under the hood, error messages can degrade to
useless noise:

| Source error | Naive `Bits`-uniform message | User impact |
|---|---|---|
| Passed `Float` to a `String` parameter | `expected Bits { Bytes = 16 }, found Bits { Bytes = 8 }` | Confusing — user wrote `String` and `Float`, not `Bits` |
| Passed `List<Int>` to `HashMap<K,V>` parameter | `expected projection 'Contains' on Bits { Bytes = 24 }` | Does not mention List or HashMap |
| Wrong enum variant | `expected Bits { discriminant=2 }, found Bits { discriminant=5 }` | Discriminant numbers are meaningless |

#### The Mitigation: Nominal Type Identity in Diagnostics

The typechecker **never** reasons in terms of `Bits`. It preserves the
nominal type identity (`String`, `Float`, `List<Int>`) throughout all
analysis passes. The `Bits` desugaring is strictly a codegen concern.

Rule: **The error message shows what the user wrote, not what the
compiler lowered it to.**

```
// Good — typechecker preserves nominal names:
Error: expected `String` (argument 1 of `process`), found `Float`
  --> lib/main.bv:42:22

// Bad — never leak Bits representation:
Error: expected Bits { Bytes = 16 }, found Bits { Bytes = 8 }
```

#### Implementation

| Phase | What changes | Impact |
|-------|-------------|--------|
| Parser | Parses types into nominal AST nodes (`Type::String`, `Type::Applied("List", ...)`) | No change — already works this way |
| Typechecker | Reports errors using `type_to_string()` which emits the nominal name | No change — already works this way |
| TypeUniverse | Preserves `source: TypeDef` in `ResolvedType` — the original type definition is always reachable | Already implemented |
| Desugarer (Phase 3) | Strips nominal info for codegen, but a parallel metadata field `original_type_name` is preserved in the lowered IR for diagnostics | New — must be added |
| Backend | Diagnostics from codegen (e.g., "can't load this field") use the original type name | Must reference `original_type_name` |

The `original_type_name` metadata is a single `String` field attached to
every `Bits` type during desugaring. It is:
- **Preserved** during analysis (type inference, region analysis, proof)
- **Referenced** by error messages
- **Ignored** by codegen (LLVM never sees it)
- **Zero cost** at runtime (compile-time metadata only)

#### Why This Works

The compiler already has all the information needed for high-quality
diagnostics. The `Type` enum carries nominal names. The `type_to_string()`
method formats them. The only change is a rule: **never let `Bits` leak
into an error message.** The desugarer is the last place that knows the
original type name — it must preserve it as metadata for the error
reporting path.

---

## 17. Performance Guarantee

### 17.1 Zero Runtime Overhead

| Concern | Answer |
|---------|--------|
| Does operator desugaring add runtime cost? | No — it's a compile-time AST transform. Same machine code. |
| Does generic projection compilation add cost for built-in types? | No — built-in types hit fast paths. The generic path is never reached. |
| Does TypeUniverse lookup add cost per projection? | O(1) HashMap lookup, amortized across the compilation session. |
| Does floating-point performance degrade? | No — `fadd#` emits `fadd` regardless of type name. LLVM's float optimizations are opcode-driven. |
| Does TBAA correctness degrade? | No — field-index-based TBAA is *more* precise than type-name-based. |
| Does compile time increase? | ~1-3% from the desugaring pass. Negligible. |

### 17.2 Shape Matching Preserves Fast Paths

The backend fast-path registry checks expression shape, not type name:

```rust
// This fires for Int, Float, or any user type with the same expression:
fn try_fast_path(name: &str, body: &Expr) -> Option<Codegen> {
    match (name, body) {
        ("Add", Expr::Intrinsic("sadd", _)) => Some(Codegen::LLVM("add i64 %a, %b")),
        ("Add", Expr::Intrinsic("fadd", _)) => Some(Codegen::LLVM("fadd double %a, %b")),
        ("Popcount", Expr::Intrinsic("ctpop", _)) => Some(Codegen::LLVM("call llvm.ctpop.i64(%a)")),
        // ...
        _ => None,  // User-defined shape → generic path
    }
}
```

### 17.3 Fallback Costs Are the User's Choice

If a user defines a novel projection like `Determinant` with a complex
expression, the generic path compiles that expression inline. This is
slower than a GEP+load. But the user *chose* novelty — they could have
defined `Determinant` in terms of known intrinsics and hit the fast path.

The guarantee is: **built-in types pay zero cost. User types pay cost
proportional to their novelty.**

---

## 18. Migration Guide

### 18.1 Breaking Changes

| Change | Impact | Migration |
|--------|--------|-----------|
| Remove `ProjectionTarget::Pop` | None — was already a runtime error | Remove any `:> Pop` usage (none exists) |
| Remove `ProjectionTarget::Index(n)` | 2 sites in `json.bv` | Replace `:> 0` with `[0]`, `:> 1` with `[1]` |
| Tuple bracket returns element instead of error | None — wasn't implemented before | — |
| `Alignment` returns real value instead of 8 | Changes output for Bool (1), Char (4) | Verify contracts |
| `Range` returns real bounds instead of [MIN,MAX] | Changes output for constrained values | Verify contracts |

### 18.2 Additive Changes (no migration needed)

| Change | Impact |
|--------|--------|
| `ProjectionTarget::IsEmpty` | New convenience target |
| `TypeProperty::Projection(name, expr)` | New variant for user-defined targets |
| `UserDefined` / `UserDefinedWithArg` | New AST variants |
| `projections { }` block | New TypeDef syntax |
| `@/` in TypeDef bindings | Extended syntax |

### 18.3 Phase-by-Phase Migration

**Phase 1**: Only breaking change is Index removal. Steps:
1. Add bracket support for tuples
2. Migrate `json.bv` (2 lines)
3. Remove Index from AST
4. All tests pass

**Phase 2**: TypeDef body syntax remains backward compatible. Old-style
`Bytes(8)` maps to new-style `Bytes = 8`. No migration needed.

**Phase 3**: All existing `.bv` files keep working. The desugarer transforms
operators transparently. No migration needed.

**Phase 4-6**: All additive. No migration needed.

---

## 19. Glossary of Terms

| Term | Definition |
|------|------------|
| **`Bits`** | The sole primitive type. N bytes of storage with position-based decomposition. |
| **Type binding** | A named entry in a TypeDef body: `Name = Expr;` or `Name(args) = Expr;`. |
| **Type property** | A binding without `_` in the body — static type metadata (e.g., `Bytes = 8`). |
| **Projection** | A binding with `_` in the body — runtime expression over the subject value (e.g., `Size = _ @/64..127`). |
| **Silent projection** | A projection that works on bare `Bits` without a type definition. Metadata, bitwise ops, and indexing are silent. |
| **Operator sigil** | A surface syntax symbol (`+`, `-`, `==`, `<-`) that desugars to a named projection. |
| **Desugaring** | The compile-time AST transform that converts operator sigils to projection expressions. |
| **Intrinsic airlock** | The `name#()` mechanism — the boundary between safe Brief space and host LLVM/hardware. |
| **Shape matching** | The backend's recognition of projection expressions by structural pattern, not type name. |
| **Fast-path registry** | The static table in each backend mapping well-known projection name+shape pairs to dedicated codegen. |
| **`@/` anchor** | The bit-precision decorator — `@/N`, `@/M..N`, `@/xN` declares exact bit positions. |
| **Lazy view** | A `Value::LazyView` that wraps a projection without materializing it. Materialization happens on consumption. |
| **Tier** | A Brief file suffix (`.bv`, `.ebv`, `.abv`, `.cbv`) that defines which properties and capabilities are recognized. |

---

## Appendix A: Example Type Definitions

### Int

```brief
type Int <: Bits @/0..63 {
    Bytes = 8;
    Alignment = 8;
    Size = 1;
    Add(rhs)    = _ :> sadd#(rhs);
    Sub(rhs)    = _ :> ssub#(rhs);
    Mul(rhs)    = _ :> smul#(rhs);
    Div(rhs)    = _ :> sdiv#(rhs);
    Mod(rhs)    = _ :> srem#(rhs);
    Eq(rhs)     = _ :> icmp_eq#(rhs);
    Lt(rhs)     = _ :> icmp_slt#(rhs);
    Gt(rhs)     = _ :> icmp_sgt#(rhs);
    Le(rhs)     = _ :> icmp_sle#(rhs);
    Ge(rhs)     = _ :> icmp_sge#(rhs);
    Neg()       = 0 - _;
    Not()       = _ == 0;
    And(rhs)    = _ != 0 && rhs != 0;  // depends on other sigil desugaring
    Or(rhs)     = _ != 0 || rhs != 0;
};
```

### Float

```brief
type Float <: Bits @/0..63 {
    Bytes = 8;
    Alignment = 8;
    Size = 1;
    Add(rhs)    = _ :> fadd#(rhs);
    Sub(rhs)    = _ :> fsub#(rhs);
    Mul(rhs)    = _ :> fmul#(rhs);
    Div(rhs)    = _ :> fdiv#(rhs);
    Eq(rhs)     = _ :> fcmp_oeq#(rhs);   // NaN ≠ NaN
    Lt(rhs)     = _ :> fcmp_olt#(rhs);
    Neg()       = _ :> fneg#;
    Sqrt()      = _ :> fsqrt#;
    ToInt()     = _ :> fptosi#;
};
```

### String

```brief
type String <: Bits @/0..127 {
    Bytes = 16;
    Alignment = 8;
    Codec = UTF8Codec;
    Size = _ @/64..127;           // bits 64-127 = length field
    Ptr  = _ @/0..63;             // bits 0-63  = data pointer
    At(i) = _ :> Ptr :> load_u8#(i);  // char at byte index
    Concat(rhs) = ...;
};
```

### List<T>

```brief
type List<T> <: Bits @/0..191 {
    Bytes = 24;
    Alignment = 8;
    ElementType = T;
    FixedSize = false;
    Size = _ @/64..127;           // bits 64-127 = length field
    Ptr  = _ @/0..63;             // bits 0-63  = data pointer
    Cap  = _ @/128..191;          // bits 128-191 = capacity
    At(i)   = _ :> Ptr[i];       // element via bracket on pointer
    ArrowPush(v) = ...;          // grow + append
    ArrowPop() = ...;            // load + shrink
    ArrowDiscard() = ...;        // shrink only
};
```

### User-defined: Matrix4x4

```brief
type Float <: Bits @/0..63 {
    Bytes = 8;
    // Float projections as above...
};

type Matrix4x4 <: Float[16] {
    Bytes = 64;
    Alignment = 8;
    Add(rhs) = ...;               // element-wise add
    Mul(rhs) = ...;               // matrix multiplication
    At(i) = _[i];                 // row-major element access
    Determinant = ...;            // user-defined projection
    Transpose = ...;              // user-defined projection
    // Inherits Size=16 from array, Ptr from Float[16], etc.
};
```

### RISC-V Instruction Decode (Bit-Field Hardware Mapping)

Demonstrates bit-precise `@/` field access for hardware co-design,
directly decodable to GEP + mask in software or wire slicing in
Verilog/CIRCT:

```brief
type RISC_V_Instruction <: Bits @/0..31 {
    Bytes = 4;
    Opcode = _ @/0..6;
    Rd     = _ @/7..11;
    Funct3 = _ @/12..14;
    Rs1    = _ @/15..19;
    Rs2    = _ @/20..24;
    Funct7 = _ @/25..31;
};
```

Usage in a disassembler:

```brief
defn decode(instr: RISC_V_Instruction) -> String {
    [instr.Opcode == 0x33] {   // OP-type (RISC-V spec)
        let s = "";
        &s = s + register_name(instr.Rd);
        &s = s + ", ";
        &s = s + register_name(instr.Rs1);
        &s = s + ", ";
        &s = s + register_name(instr.Rs2);
        term s;
    };
    term "unknown";
};
```

Key properties:
- Each field is defined at its exact bit position — no manual masking
- The `@/` ranges are the source of truth for layout, in both software
  (LLVM backend emits GEP + load + mask) and hardware (CIRCT backend
  emits wire slicing directly)
- The type is exactly 32 bits (`Bytes = 4`) — fits in a register
- No abstraction penalty: `instr.Opcode` compiles to the same
  instructions as a hand-coded `(instr >> 0) & 0x7F`

## Appendix B: Tier Ignorance Examples

```brief
// ============================================================
// CBV (Circuit Brief) — Ignored properties
// ============================================================
// CBV requires total contracts and synthesizable types only.
// The following properties are IGNORED in CBV:
//
//   Volatile, Atomic         — No LLVM flag equivalence in Verilog
//   ElementType, FixedSize   — No runtime allocator in hardware
//   InsertAt, ExtractFrom    — No dynamic insertion
//   AllowIndex, AllowSlice, AllowArrow  — No syntax gating in CBV
//   Codec                    — No serialization library in hardware
//
// User-defined projections are VALID in CBV if they are composed
// entirely of synthesizable operations (bitwise, arithmetic on
// bounded integers, slice, etc.). Projections using frgn or
// non-synthesizable intrinsics are errors.

// ============================================================
// ABV (Accelerated Brief) — String exception
// ============================================================
// ABV compiles to SPIR-V (GPU). String constants are recognized
// at compile time for error messages and shader embedding.
// Runtime String values are NOT supported — there is no
// heap on the GPU.
//
// String constants are compiled into SPIR-V string literals.
// The Codec property is ignored — strings are treated as
// opaque byte arrays.
```

---

## Appendix C: Recognition Pattern Table

The compiler recognizes these projection expression shapes for
fast-path codegen, regardless of the type name:

| Projection | Expression shape detected | Backend emits | Optimization |
|------------|--------------------------|---------------|--------------|
| `Add` | `_ :> sadd#(rhs)` | `add i64 %a, %b` | Standard ALU |
| `Add` | `_ :> fadd#(rhs)` | `fadd double %a, %b` | FMA, reassociation |
| `Size` | `1` (constant) | Constant-fold | Inlined |
| `Size` | `_ @/64..127` (load offset, byte-aligned) | GEP + load | SROA, mem2reg |
| `Popcount` | `_ :> ctpop#` | `call llvm.ctpop.i64` | → `POPCNT` instr |
| `At(i)` | `_ :> Ptr[i]` (load at index) | GEP + load at index | LICM, CSE |
| `Eq(rhs)` | `_ :> icmp_eq#(rhs)` | `icmp eq i64` | Fold to `select` |
| `+` desugared | `a :> Add(b)` → see Add | (same as Add) | — |

The shape detector normalizes expressions through the equality saturation
pass before matching, so commutative variants (`sadd#(_, rhs)` vs
`sadd#(rhs, _)`) both match.

---

*End of plan.*
