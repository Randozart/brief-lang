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

The interpreter has a single representational value type. Every value in the
Brief universe — from a boolean to a 10-megabyte database index — is
represented internally as a raw byte sequence:

```rust
pub enum Value {
    /// The ONLY representational storage cell for program data.
    /// 2026-07-11: All scalars, structs, and collections are bits.
    Bits(Vec<u8>),

    // Compiler-internal meta-objects (not representational data)
    Defn(String),
    Void,
    Ref(Box<Value>),
    Expr(Box<Expr>),
    Stmt(Box<Statement>),
    Block(Vec<Statement>),
    Items(Vec<TopLevel>),
    Type(Type),
    Regex(RegexPattern),
    DbvlTable(Arc<DbvlTableInner>),
}
```

No `List`, no `HashMap`, no `Tuple`, no `Instance`, no `Enum`. A `List<T>`
containing a million elements is `Value::Bits(24 bytes)` — the struct layout
`{ ptr: Ptr<T>, len: Int, cap: Int }`, where `ptr` is a **virtual memory
address** into the interpreter's sandboxed heap (see §6).

#### VirtualHeap: The Compile-Time Memory Model

The interpreter maintains a sandboxed virtual memory space for compile-time
execution. This is the same pattern used by Miri (Rust's compile-time
interpreter) and every safe partial evaluator:

```rust
/// Sandboxed heap for compile-time allocation and pointer arithmetic.
/// 2026-07-11: Phase 8A — enables List, HashMap, Box etc. as pure Bits.
pub struct VirtualHeap {
    allocations: HashMap<u64, Vec<u8>>,
    next_address: u64,
}
```

When compile-time code runs `list.push(val)`:
1. The intrinsic `__list_insert#` receives `Value::Bits(24)` representing
   `{ ptr, len, cap }` and `Value::Bits(N)` representing `val`
2. It copies the virtual address from the struct, checks bounds, optionally
   allocates new memory in the VirtualHeap
3. It writes the new element's bytes at `ptr + len * sizeof<T>()` in the heap
4. It returns a new `Value::Bits(24)` with updated `len`

This is a `HashMap::get` + `Vec::extend_from_slice` at compile time —
O(1), no different from the current `Value::List` access path.

#### Prelude Cache

To avoid re-evaluating the standard library on every compilation, the
interpreter state is cached after first evaluation:

```
cache/prelude.bincode:
  - VirtualHeap allocations (type structures, string constants)
  - Type universe (resolved types, operator bindings)
  - FFI registry entries

Invalidation: file timestamps on lib/std/*.bv + compiler version hash
```

On subsequent compilations, the cached prelude is deserialized instead of
re-evaluated. The VirtualHeap allocation cost for prelude types is paid
once, not once per build.

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

### 6. The Three Token Forms (Compiler Axioms)

The parser produces exactly three primitive token forms, each representing
raw bytes from source text with no semantic interpretation:

| Form | AST node | Source | `formatting <~` value |
|------|----------|--------|----------------------|
| QuotedValue | `Expr::Quoted(Vec<u8>)` | `"..."` | `Quoted` |
| DecimalValue | `Expr::Decimal(i64)` | `[0-9]+`, `[0-9]+\.[0-9]+` | `Numeric` |
| Bareword | `Expr::Identifier(String)` | `[a-zA-Z][a-zA-Z0-9_]*` | `Bare` |

These are compiler axioms — they must exist because the lexer must produce
something. All semantic meaning is attached by the type's codec via the
`formatting <~` property:

```brief
codec HexColor {
    formatting <~ Bare;     // ← FF00FF is accepted
    parse      <~ parse_hex;  // converts text to Value::Bits
};
```

The `@` prefix modifier converts any token to `QuotedValue(raw_bytes)`,
bypassing lexer interpretation. `@FF00FF`, `@42`, `@"..."` all produce
`Expr::Quoted(bytes)`.

No name-based magic. `String` accepts `"..."` because `DefaultQuoted`
declares `formatting <~ Quoted`, not because the type is named `String`.

### 7. The Complete Type Hierarchy from First Principles

Every type is `Bits(N)` + metadata. Nothing is special-cased:

```
Bits(axiom)          — 3 axioms, the only hardcoded type
  │
  ├─ Void            — Bits(0), llvm <~ "void"
  ├─ Int             — Bits(8), op Add = __add_i64#
  ├─ Float           — Bits(8), op Add = __fadd_f64#
  ├─ Bool            — Bits(1), op Eq = __eq_i1#
  ├─ Char            — Bits(4), op Eq = __eq_i32#
  ├─ String          — Bits(24), op Drop = __free_string_allocation#
  ├─ Box<T>          — Bits(8), op Drop = __free_heap_allocation#
  ├─ List<T>         — Bits(24), op InsertAt/ExtractFrom
  ├─ HashMap<K,V>    — Bits(24), op ExtractFrom
  └─ MyCustomType    — same mechanism, user-defined
```

Everything on the right side of `│` is defined in the standard library
prelude (`bootstrap.bv`), not in the compiler's Rust code. A user could
omit the prelude entirely, define their own `Int` with saturating
arithmetic, their own `List` with arena allocation, their own `String`
with a different encoding — and the compiler would handle them identically
because it only sees `Bits` + properties. There is no "stdlib" path and
"user" path in the compiler. There is only one path.

### 7. SMT Solver Alignment

#### 7.1 Eliminating the Translation Gap

In traditional solver-aided compilers, the compiler must bridge between
its high-level type representation and the SMT solver's logic. Translating
an algebraic enum, a struct with padding, or a string into SMT-LIB requires
complex lowering rules that are brittle and hard to verify.

Under the Bits thesis, a type's physical representation in the compiler is
identical to its representation in the SMT solver:

| Brief type | Compiler value | SMT-LIB sort |
|-----------|---------------|--------------|
| `Int` | `Value::Bits(8 bytes)` | `(_ BitVec 64)` |
| `Bool` | `Value::Bits(1 byte)` | `(_ BitVec 8)` |
| `String` | `Value::Bits(24 bytes)` | `(_ BitVec 192)` |
| Custom struct | `Value::Bits(N bytes)` | `(_ BitVec N*8)` |

The translation function is a single match arm:

```rust
fn value_to_smt(value: &Value) -> SmtExpr {
    match value {
        Value::Bits(bytes) => SmtExpr::Bitvector(bytes.len() * 8, bytes),
        _ => unreachable!(), // meta-objects never reach the solver
    }
}
```

No type dispatch. No enum-variant branching. The frontend and the solver
speak the same bit-level language. The mathematical model can never diverge
from the runtime behavior because they are the same representation.

#### 7.2 Bit-Blasting Performance

SMT solvers are exceptionally fast at solving bit-vector constraints because
of **bit-blasting**: every bit of a `(_ BitVec N)` variable becomes a boolean
variable, and operations become networks of primitive logic gates. These gate
networks are fed to the solver's CDCL (Conflict-Driven Clause Learning) SAT
engine, which can solve millions of boolean variables in microseconds.

Because Brief values are already raw bytes, the compiler can emit SMT-LIB
bit-vector constraints directly — no type-to-logic lowering step. The solver
receives the same bit-level problem the hardware would execute.

#### 7.3 Elimination of the Modulo Arithmetic Tax

If a solver models machine integers using Linear Integer Arithmetic (LIA),
it must enforce wrapping on every operation:

```
; LIA model of 64-bit addition — expensive for the solver
(assert (= result (mod (+ a b) 18446744073709551616)))
```

In QF_BV, wrapping is native to the representation:

```
; QF_BV — wrapping is free, the bit-vector is finite
(assert (= result (bvadd a b)))
```

The solver's bit-blasting circuit for `bvadd` naturally wraps at 64 bits,
just like physical CPU silicon. Overflow checks, saturating arithmetic, and
bounds verification are all derived from the same native bit-vector
operations — no modulo constraints needed.

#### 7.4 Elimination of Pointer Aliasing Complexity

In SMT solvers, modeling a heap with arbitrary pointer aliasing requires
expensive array theory axioms (select/store with frame conditions). Because
Brief's value semantics and linear ownership guarantee that variables are
unaliased, independent bit-vectors, the solver never needs to reason about
aliasing. Every variable maps to a fresh `(_ BitVec N)` constant. The
solver's state space is flat and localized — no interconnected pointer web.

#### 7.5 Impact on Synthesis and Verification

The synthesis engine (Phase 9) and contract verifier (Phase 10) both
translate function bodies and contracts into SMT-LIB queries. Because the
input is already `Value::Bits`:

- The solver interface is a single function, not a type-directed translator
- Example values from `:=` derivation blocks map directly to solver constants
- Contract pre/post-conditions become bit-vector constraints without lowering
- The solver's counterexample models are trivially convertible back to
  `Value::Bits` — no reification step needed

This alignment is not an accident of implementation. It follows directly from
the Bits thesis: if everything is bits at the compiler level, everything is
bits at the solver level, because the solver is just another consumer of the
same representation.

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
