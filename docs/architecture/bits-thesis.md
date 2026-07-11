# The Bits Thesis

**Date:** 2026-07-11  
**Status:** Foundational  
**Applies to:** Brief compiler core architecture, interpreter, type system, backends

---

## Preamble

Most compilers hardcode primitive types (`Int`, `Float`, `Bool`, `String`) into
their intermediate representations and interpreter internals. This makes the
compiler simple to write initially, but permanently locks the language's
semantics: a user cannot define their own `Int` with saturating arithmetic,
their own `Float` with a different rounding mode, or their own `Bool` with
three-valued logic without modifying the compiler itself.

The Bits thesis rejects this compromise. It treats the compiler as a pure
execution engine for **uninterpreted bit-vectors**, where all semantic meaning
is injected dynamically from the type universe at compile time. The compiler
frontend knows nothing about integers, floats, strings, or pointers — it only
knows about bits, and everything else is a metadata overlay.

---

## The Three Axioms

The entire Brief language is built from exactly three hardcoded assumptions.
Everything else — every type, every operation, every data structure — follows
from these axioms and is defined in the standard library prelude.

### Axiom 1: `Bits` Is the Sole Primitive

`Bits` is a built-in type representing a contiguous sequence of N uninterpreted
bytes. It is the only type the compiler knows about axiomatically.

```
type Bits {
    bytes <~ N;      // width in bytes
};
```

`Bits` is special-cased in the type resolver: it has no base type because it
*is* the base. Every other type in the language inherits from `Bits`, directly
or transitively:

```
type Int      <: Bits { bytes <~ 8;  ... }
type Float    <: Bits { bytes <~ 8;  ... }
type Bool     <: Bits { bytes <~ 1;  ... }
type Char     <: Bits { bytes <~ 4;  ... }
type String   <: Bits { bytes <~ 24; ... }
type Void     <: Bits { bytes <~ 0;  ... }   // zero-width => void
```

The only property the frontend hardcodes at the type level is `bytes`. It must
know the width of every type to compute struct field offsets, allocate
interpreter storage, and emit LLVM struct layouts.

### Axiom 2: Bitwise Operations Are the Laws of Physics

The `Bits` type intrinsically supports the six operations of boolean algebra
without consulting any `op` bindings in the type universe:

| Operation | Symbol | Semantics |
|-----------|--------|-----------|
| Bitwise AND | `&` | Byte-wise AND |
| Bitwise OR  | `\|` | Byte-wise OR  |
| Bitwise XOR | `^` | Byte-wise XOR |
| Bitwise NOT | `~` | Byte-wise complement |
| Shift left  | `<<` | Byte array shift |
| Shift right | `>>` | Byte array shift |

These are the only operations the interpreter applies to `Value::Bits` without
looking up type properties. They represent the native laws of physics of the
underlying hardware — wires and gates. All other operations (addition,
multiplication, comparison, string concatenation, etc.) are **semantic
interpretations** bound to bits via the `op` metadata system.

### Axiom 3: Runes Map to `op` Bindings

The Brief language surface has operator symbols (runes): `+`, `-`, `*`, `/`,
`==`, `<`, `>`, `[]`, `<-`, etc. The frontend hardcodes the mapping from each
rune to an `op` contract name:

| Rune | `op` name | Purpose |
|------|-----------|---------|
| `+`  | `op Add`  | Addition |
| `-`  | `op Sub`  | Subtraction |
| `*`  | `op Mul`  | Multiplication |
| `/`  | `op Div`  | Division |
| `==` | `op Eq`   | Equality |
| `<`  | `op Lt`   | Less-than |
| `[]` | `op ExtractFrom` | Index read |
| `<-` | `op InsertAt` / `op DiscardAt` | Collection mutation |
| `term` | `op Term` | Return/implicit terminal |

The type determines the *binding* for each `op`. The binding is either a
compiler intrinsic (identified by a trailing `#`) or a standard Brief function
(no trailing `#`):

```brief
type Int: Bits {
    bytes <~ 8;
    llvm  <~ "i64";
    op Add(Int, Int) -> Int  = __add_i64#;     // compiler intrinsic
};

type Complex: Bits {
    real: Float;
    imag: Float;
    op Add(Complex, Complex) -> Complex = complex_add;  // user function
};
```

Intrinsics route to the interpreter's `execute_intrinsic` table, which
performs the operation on raw byte arrays. User functions are AST-level
rewrites: `a + b` becomes `complex_add(a, b)` before codegen.

---

## Derived Concepts

From these three axioms, the entire language emerges.

### 1. The Universal Value: `Value::Bits(Vec<u8>)`

The interpreter has a single representational value type:

```rust
pub enum Value {
    Bits(Vec<u8>),           // representational — the only compute cell
    List(Vec<Value>),        // structural — heap-allocated array (optimization)
    HashMap(HashMap<K,V>),   // structural — associative map (optimization)
    Tuple(Vec<Value>),       // structural — fixed-size heterogeneous
    Instance { typename, fields },  // structural — struct instance
    Enum(name, variant, fields),    // structural — tagged union
    Defn(String),            // compiler internal
    Void,                    // compiler internal
    Ref(Box<Value>),         // compiler internal
    Expr(Box<Expr>),         // compiler internal (macro system)
    Stmt(Box<Statement>),    // compiler internal
    Block(Vec<Statement>),   // compiler internal
    Items(Vec<TopLevel>),    // compiler internal
    Type(Type),              // compiler internal
    Regex(...),              // compiler internal
    DbvlTable(...),          // compiler internal
}
```

The structural variants (`List`, `HashMap`, `Tuple`, `Instance`, `Enum`) are
optimizations — the interpreter could theoretically represent everything as
`Bits` with layout metadata, but doing so for dynamic collections would
require a full memory allocator inside the interpreter. These variants are
keepered for pragmatic efficiency reasons. They are *not* representational
primitives — the type system does not know about them.

### 2. `Void = Bits(0)`

The `Void` type is not a base type. It is a zero‑width specialization of
`Bits`:

```brief
type Void: Bits {
    bytes <~ 0;
    alignment <~ 1;
    llvm  <~ "void";
};
```

Because `bytes <~ 0`, the type resolver allocates zero bytes for any slot of
type `Void`. The LLVM backend reads the `llvm` property and emits `void`. The
compiler's frontend has zero hardcoded knowledge of "Void" as a concept.

### 3. `Box<T>` Is a Struct with `op Drop`

Memory management is not a compiler intrinsic. `Box<T>` is a library type:

```brief
type Box<T>: Bits {
    ptr: Ptr<T>;
    op Drop(self) = __free_heap_allocation#;
};
```

The compiler tracks variable lifetimes. When a value of a type implementing
`op Drop` goes out of scope, the compiler inserts a call to the destructor.
If no `op Drop` exists, the compiler generates zero deallocation code.

This replaces the old `storage <~ "Boxed"` magic string. The compiler does not
need to know what a "box" is — it only needs to know whether `op Drop` exists.

### 4. Collections Are Stdlib Structs

`List`, `HashMap`, `HashSet`, `Stack`, `Queue` are not compiler primitives.
They are structs defined in `lib/std/` that manage pointers to heap memory:

```brief
type List<T>: Bits {
    ptr: Ptr<T>;
    len: Int;
    cap: Int;
    op Drop(self) = free_list_allocation;
    op InsertAt(self, index: Int, val: T) = list_insert;
    op ExtractFrom(self, index: Int) -> T = list_get;
};
```

Indexing a list (`list[i]`) routes through `op ExtractFrom`, which computes
`ptr + i * sizeof<T>()` and dereferences. The interpreter evaluates this as a
regular function call. No special-casing for "list operations."

### 5. Backend‑Intrinsic Metadata Is Opaque

The frontend recognizes a fixed set of metadata properties for its own use:

| Property | Purpose | Hardcoded? |
|----------|---------|------------|
| `bytes` | Byte width of the type | Yes (Axiom 1) |
| `alignment` | Memory alignment | Yes (layout engine) |
| `op X` | Operator binding | Yes (Axiom 3 — rune→op, not op→intrinsic) |
| `llvm` | LLVM type representation | **No** — opaque to frontend |
| `hw_storage` | Hardware storage type | **No** — opaque to frontend |

Any property the frontend does not recognize is stored, serialized to the
`.bvsa` archive, and ignored. Only backend-specific tooling (e.g.,
`brief-llvm`, `brief-circt`) interprets it.

### 6. The Complete Type Hierarchy from First Principles

```
Bits(axiom)          — 3 axioms
  ├─ Void            — Bits(0), no properties
  ├─ Int             — Bits(8), op Add = __add_i64#
  ├─ Float           — Bits(8), op Add = __fadd_f64#
  ├─ Bool            — Bits(1), op Eq = __eq_i1#
  ├─ Char            — Bits(4), op Eq = __eq_i32#
  ├─ String          — Bits(24), struct { ptr, len, codec }, op Drop = __free_string#
  ├─ Box<T>          — Bits(8), struct { ptr }, op Drop = __free_heap#
  ├─ List<T>         — Bits(24), struct { ptr, len, cap }, op InsertAt/ExtractFrom
  ├─ HashMap<K,V>    — Bits(24), struct { buckets, len, cap }, op ExtractFrom
  └─ UserDefined     — any composition of the above
```

Everything on the right side of `Bits` is defined in the standard library
prelude (`bootstrap.bv`), not in the compiler's Rust code. A user could
omit the prelude, define their own `Int` with saturating arithmetic, and
the compiler would handle it identically.

---

## How This Enables Decoupled Backends

The `.bvsa` (Brief Value Semantic Archive) is a serialized representation of
the typed program. It contains:

- Type definitions with all properties (frontend- and backend-intrinsic)
- Function bodies as AST nodes
- Operator bindings as property references

A decoupled LLVM backend reads the `.bvsa` archive. For each type, it queries
the `llvm` property to determine the LLVM type representation. If `llvm` is
not present, it derives a default from `bytes` (e.g., `bytes=8` → `i64`).
It never asks the frontend "is this an Int?" — it only reads properties.

A decoupled CIRCT backend reads the same `.bvsa` archive. It queries
`hw_storage` or ignores `llvm` entirely. The frontend does not need to know
which properties a backend will use.

---

## Sufficiency Proof

These three axioms are sufficient to build an entire programming language's
behavior because:

1. **All data is bits.** Every value in every program is a sequence of bytes.
   The interpreter can store, copy, and compare any value without knowing
   what it represents.

2. **All computation reduces to bit manipulation.** Addition, subtraction,
   comparison, indexing — every operation is a transformation on byte arrays.
   The intrinsics table maps named operations (`"__add_i64"`) to concrete
   byte-array logic. Backends map the same names to native instructions.

3. **All semantics are metadata.** The meaning of a value — whether it is an
   integer, a float, a pointer, or a color — is not in its bits. It is in
   the properties attached to its type in the universe. Changing the
   properties changes the semantics without changing the bits.

4. **All extension is user-defined.** A new type, new operator, new backend
   target — none require compiler changes. Types are declared, operators
   bound, backends subscribe to properties they understand.
