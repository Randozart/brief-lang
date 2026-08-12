# Hash-Prefixed Compiler Words (`#words`)

Compiler-internal tokens prefixed with `#` that carry special meaning.
They are lexed as distinct tokens, never as identifiers.

## 2026-07-20: Hashword Categories (`#Int`, `#Float`, `#String`, etc.)

Hashwords now serve an additional role as **backend category directives** in
op signatures:

```briev
type Int : Bits {
    op Add(#Int, #Int);       // "backend, emit whatever i64 add means to you"
    op Sub(#Int, #Int);
    op Mul(#Int, #Int);
};

type Bfloat16 { data: Bits<16>;
    op Add(#Float, #Float) = bfloat_add(#Lh, #Rh);  // override with custom fn
};
```

A hashword in an op signature tells the backend: **handle this operation
using your intrinsic knowledge of the `#Category` protocol.** The backend
decides what `#Int` addition means in its own terms (LLVM → `add i64`,
CIRCT → hardware adder, SPIR-V → `OpIAdd`).

**Protocol variants** parameterize hashwords: `#String<UTF8>`, `#String<ASCII>`,
`#Float<IEEE754>`. The file extension determines the default (`.bv` → UTF8,
`.ebv` → ASCII). Cross-variant calls require explicit protocol disambiguation
— the compiler errors if a `.bv` file calls a `.ebv` function using `#String`
without specifying the variant:

```briev
fn cross(a: #String<UTF8>, b: #String<ASCII>) { ... };
```

Each backend declares supported protocols in `config/targets.dbvl`. A function
requiring a protocol the backend doesn't support produces a compile error.

### `Cast#()` — Cast intrinsick

`Cast#()` is a compiler internal intrinsick — not user-declarable. When the
programmer writes `(TargetType)expr`, the compiler emits `Cast#(expr, TargetType)`,
which runs the resolution pipeline:

1. `meld Source <-> Target` — structural equivalence
2. `op Cast(Target)` on Source — direct type-to-type
3. `CastTo(#Category)` → `CastFrom(#Category)` — protocol path
4. Implicit `CastTo(#Bits)` + `CastFrom(#Bits)` — raw bytes (always)

User-declarable ops: `CastTo(#Category)`, `CastFrom(#Category)`, and
`Cast(ConcreteType)`. See `docs/architecture/casting-protocol.md`.

See `docs/architecture/casting-protocol.md` for the full protocol system.

### `op Parse(#Category)` — Compile-time identity parse

`op Parse(#Category)` uses the same hashword mechanism as `op Add(#Int)`:

| `#Category` | Tells the compiler |
|---|---|
| `#Int` | Parse literals as native integer — no conversion needed |
| `#String` | Parse quoted literals as UTF-8 — no conversion needed |
| `#Float` | Parse numeric literals as IEEE 754 float |

`op Parse(Bare)`, `op Parse(Decimal)`, and `op Parse(Quoted)` are NOT
hashword ops — they are concrete-form ops that always require a conversion
function. Only `op Parse(#Category)` is a hashword op.

```briev
type Int {
    op Add(#Int, #Int);       // backend directive: integer add
    op Parse(#Int);            // identity parse: literal IS an Int
    op Parse(Decimal);         // concrete form: numeric literal → conversion fn
};
```

## Current Words

| Token | Meaning | Used In |
|-------|---------|---------|
| `#Lh` | Left operand of `<-` | Strategy op bindings: `op InsertAt: fn(#Lh, #Rh)` |
| `#Rh` | Right operand of `<-` | Strategy op bindings: `op ExtractFrom: fn(#Rh)` |
| `#T` | Type parameter of generic collection | Strategy op bindings: `pop as #T` |
| `#StdIn` / `#StdOut` / `#StdErr` | Stream symbols (Phase 4) | `#StdOut <- value` writes (→ `Print#`); `#StdErr <- <String>` writes to stderr (→ `__eprint_str`); `#StdIn` is a `Ptr<Int>` stream-handle value |

## Semantics

### In strategy property bindings (`<~`)

Resolved at codegen time by substituting the concrete operand:

| Marker | `collection <- value` (InsertAt) | `dest <- collection` (ExtractFrom) | `<- collection` (Discard) |
|--------|----------------------------------|------------------------------------|---------------------------|
| `#Lh` | handle register for `collection` | pop target register for `dest` | void (no target) |
| `#Rh` | value register for `value` | handle register for `collection` | handle register for `collection` |
| `#T` | element type of collection | element type of collection | element type of collection |

The handle register is computed via `emit_addr_of` on the collection variable,
which produces a GEP into `%State` (for state fields) or an alloca address
(for let bindings). The `#Rh`/`#Lh` substitution is a register name pass-through
— the compiler resolves the expression to a register first, then substitutes.

### Stream symbols (`#StdIn`/`#StdOut`/`#StdErr`, Phase 4)

These are compiler-known intrinsic-pointer symbols, not protocol hashwords.
They lex as ordinary identifiers (`#` is an identifier character) and are
recognized by name in the arrow typechecker and codegen:

- `#StdOut <- value` — any value; lowered to the generic `Print#` intrinsic.
- `#StdErr <- <String>` — a String only; lowered to `__eprint_str` (stderr).
- `#StdIn` — usable as a value (`let h: Ptr<Int> = #StdIn;`); a stream handle
  for the trg read composition.

### Rule

No `#`-prefixed word is ever a user-defined identifier. They are reserved
compiler vocabulary. Adding a new `#word` requires:
1. A new token in `src/lexer.rs`
2. Parser handling in the relevant context
3. Codegen resolution in the backend
4. Entry in this document

### Reserved

- `#Self` — reserved for future use (self-reference to the type definition)

## Non-word Hash Tokens

These `#` tokens exist but are NOT compiler words — they are syntax:

| Token | Purpose |
|-------|---------|
| `#` suffix on identifiers | Intrinsic marker: `Malloc#`, `SysCall#`, `Sqrt#` |
