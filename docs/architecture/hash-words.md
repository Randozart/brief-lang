# Hash-Prefixed Compiler Words (`#words`)

Compiler-internal tokens prefixed with `#` that carry special meaning.
They are lexed as distinct tokens, never as identifiers.

## 2026-07-20: Hashword Categories (`#Int`, `#Float`, `#String`, etc.)

Hashwords now serve an additional role as **backend category directives** in
op signatures:

```brief
type Int <: Bits {
    op Add(#Int, #Int);       // "backend, emit whatever i64 add means to you"
    op Sub(#Int, #Int);
    op Mul(#Int, #Int);
};

type Bfloat16 { data: Bits<16>;
    op Add(#Float, #Float) = bfloat_add(#L, #R);  // override with custom fn
};
```

A hashword in an op signature tells the backend: **handle this operation
using your intrinsic knowledge of the `#Category` protocol.** The backend
decides what `#Int` addition means in its own terms (LLVM → `add i64`,
CIRCT → hardware adder, SPIR-V → `OpIAdd`).

**Protocol variants** parameterize hashwords: `#String<utf8>`, `#String<ascii>`,
`#Float<ieee754>`. The file extension determines the default (`.bv` → utf8,
`.ebv` → ascii). Cross-variant calls require explicit protocol disambiguation
— the compiler errors if a `.bv` file calls a `.ebv` function using `#String`
without specifying the variant:

```brief
fn cross(a: #String<utf8>, b: #String<ascii>) { ... };
```

Each backend declares supported protocols in `config/targets.toml`. A function
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

```brief
type Int {
    op Add(#Int, #Int);       // backend directive: integer add
    op Parse(#Int);            // identity parse: literal IS an Int
    op Parse(Decimal);         // concrete form: numeric literal → conversion fn
};
```

## Current Words

| Token | Meaning | Used In |
|-------|---------|---------|
| `#L` | Left operand of `<-` | Strategy property bindings: `InsertAt <~ fn(#L, #R)` |
| `#R` | Right operand of `<-` | Strategy property bindings: `ExtractFrom <~ fn(#R)` |
| `#T` | Type parameter of generic collection | Strategy property bindings: `pop as #T` |

## Semantics

### In strategy property bindings (`<~`)

Resolved at codegen time by substituting the concrete operand:

| Marker | `queue <- value` (InsertAt) | `x <- &queue` (ExtractFrom) | `<- &queue` (Discard) |
|--------|-----------------------------|-----------------------------|----------------------|
| `#L` | handle register for `queue` | pop target register for `x` | void (no target) |
| `#R` | value register for `value` | handle register for `queue` | handle register for `queue` |
| `#T` | element type of collection | element type of collection | element type of collection |

The handle register is computed via `emit_addr_of` on the collection variable,
which produces a GEP into `%State` (for state fields) or an alloca address
(for let bindings). The `#R`/`#L` substitution is a register name pass-through
— the compiler resolves the expression to a register first, then substitutes.

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
