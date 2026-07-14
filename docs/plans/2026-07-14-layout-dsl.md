# Layout DSL — Formal Binary Grammar for Brief Types

## The Problem

Types currently have no way to declare their binary layout. `Float32` is known to be 4 bytes, but the compiler doesn't know which bits are sign, exponent, or mantissa. `String` is known to be an 8-byte pointer, but the compiler doesn't know the byte-level encoding of the data it points to. Codecs exist as opaque parse/format functions — the compiler cannot reason about their internal structure.

## The Solution

A `layout <~ <pattern>` metadata key on any type. Two pattern forms, both parsed by the same parser, both stored as the same `LayoutPattern` AST type:

### Fixed-width slicing — `[name: bits, ...]`

```brief
type Float32 <: Bits {
    bytes <~ 4;
    primitive <~ Float;
    layout <~ le: [sign: 1, exp: 8, mant: 23];
}
```

Named fields with bit widths. Sum must equal `bytes * 8`. Endianness prefix: `le:` or `be:`. If omitted, the normalizer adapts to target.

The SMT solver can read this and reason about sign, exponent, and mantissa independently. Two types with different field arrangements but matching total widths and a meld can be auto-shuffled by the normalizer.

### Variable-width pattern — `(byte-range | ...)*`

```brief
type String <: Bits {
    bytes <~ 8;
    primitive <~ String;
    layout <~ be: (@codepoint: (
        0x00..0x7F |
        0xC2..0xDF 0x80..0xBF |
        0xE0 0xA0..0xBF 0x80..0xBF |
        0xE1..0xEC {0x80..0xBF, 2} |
        0xED 0x80..0x9F 0x80..0xBF |
        0xEE..0xEF {0x80..0xBF, 2} |
        0xF0 0x90..0xBF {0x80..0xBF, 2} |
        0xF1..0xF3 {0x80..0xBF, 3} |
        0xF4 0x80..0x8F {0x80..0xBF, 2}
    ))*;
}
```

Semantic labels (`@codepoint`) annotate pattern branches that represent one logical unit. The SMT solver uses these boundaries for string-level proofs.

### Data-dependent lengths — `[$len: bits, data: {$len}]`

```brief
type PngChunk <: Bits {
    bytes <~ 12;
    layout <~ be: [$length: 32, kind: 32, data: {$length}, crc: 32];
}
```

The `$` prefix declares a named field whose value can be referenced later with `{$name}`. The parser/DFA engine uses the captured value to determine variable-length regions.

### Meld layout mapping — `<:>` field operators inside meld blocks

```brief
meld Float32 <:> MyCustomFloat {
    layout {
        sign <:> sign;
        exp  <:> exp;
        mant <:> mant;
    }
}
```

The `<:>` operator inside `layout { }` maps a field from the LHS layout to the corresponding field in the RHS layout. The normalizer reads these mappings and auto-synthesizes bit-shuffling instructions between the two types.

## The Layout DSL Specification

### Primitive elements

| Syntax | Meaning | Example |
|--------|---------|---------|
| `0xNN` | Literal byte | `0x89` matches byte 0x89 |
| `0xNN..0xNN` | Byte range | `0xC2..0xDF` matches any byte from 0xC2 to 0xDF |
| `{N}` | Exactly N bytes of any value | `{4}` matches any 4 bytes |
| `{expr}` | Variable-length reference | `{$length}` matches `$length` bytes |
| `{min, max}` | Bounded repetition | `{0x80..0xBF, 2}` matches two bytes in range |

### Named fields (fixed-width only)

| Syntax | Meaning |
|--------|---------|
| `name: N` | Declare a field named `name` that is `N` bits wide |
| `$name: N` | Declare a named field whose value can be referenced later |

### Structural combinators

| Syntax | Meaning | Example |
|--------|---------|---------|
| `A B` | Sequence — A followed by B | `0x89 0x50 0x4E 0x47` matches PNG magic |
| `A \| B` | Alternation — either A or B | `0x00..0x7F \| 0xC2..0xDF 0x80..0xBF` |
| `(...)*` | Zero or more repetitions | `(@codepoint: ...)*` matches a string |
| `(...)?` | Optional | `(0x00)?` matches zero or one null byte |
| `[...]` | Fixed-width slice group | `[sign: 1, exp: 8, mant: 23]` |
| `(...)` | Variable-width pattern group | `(0x00..0x7F \| 0xC2..0xDF)*` |

### Semantic annotations

| Syntax | Meaning |
|--------|---------|
| `@name: pattern` | Label a pattern branch with a semantic name (e.g., `@codepoint`) |
| `le:` | Force little-endian byte order for this layout |
| `be:` | Force big-endian byte order for this layout |
| (no prefix) | Target-adaptive endianness |

### Meld layout block

```brief
meld TypeA <:> TypeB {
    layout {
        field_a <:> field_b;
    }
}
```

Every `<:>` inside a `layout { }` block maps one named field from the LHS type's layout to one named field from the RHS type's layout. The normalizer reads this and auto-synthesizes the bit-shuffle.

## Implementation Phases

### Phase A — Parser: `layout <~ ...` in type bodies

Extend `parse_type_definition()` in `src/parser/definitions.rs` to recognize `layout` as a metadata key in type bodies. The value after `<~` is either `[...]` (fixed-width) or `(...)` (variable-width). Store the parsed pattern string as-is in `TypeDefBody.metadata["layout"]`.

No AST type needed yet — the pattern string is stored verbatim and parsed later by the normalizer/validator.

### Phase B — LayoutPattern AST type

Create `src/ast/layout.rs`:

```rust
pub enum LayoutPattern {
    // Fixed-width
    Slice(Vec<LayoutField>),
    // Variable-width
    Sequence(Vec<LayoutPattern>),
    Alternation(Vec<LayoutPattern>),
    Repetition(Box<LayoutPattern>),
    Optional(Box<LayoutPattern>),
    ByteLiteral(u8),
    ByteRange(u8, u8),
    AnyBytes(u64),              // {N}
    VariableRef(String),        // {$name}
    NamedField(String, u64),    // name: N
    RefField(String, u64),      // $name: N
    SemanticLabel(String, Box<LayoutPattern>), // @name: pattern
}
```

### Phase C — Layout parser (DSL → LayoutPattern)

Create `src/bvir/layout.rs`. A recursive descent parser that takes the pattern string and produces `LayoutPattern`. This is the same pattern as the S-expression parser in `sexpr.rs` — flat, max 2 depth.

### Phase D — DFA compiler for literal validation

For variable-width patterns, compile the `LayoutPattern` into a Deterministic Finite Automaton. Walk the byte-level structure of any literal in source code and validate it against the compiled DFA. This runs at compile time, during the normalizer pass.

### Phase E — Normalizer bit-shuffling synthesis

For melds that contain `layout { ... }` blocks, the normalizer reads the field mappings and generates bit-shifting/masking instructions. Three cases:

1. **Identity** — fields match exactly (same type, same layout): no shuffle needed.
2. **Reorder** — fields exist in both layouts but at different bit positions: synthesize `lshr`, `and`, `shl`, `or` sequence.
3. **Partial** — some fields match, some don't: shuffle what can be shuffled, error on the rest.

### Phase F — SMT proof integration

Translate `@codepoint`-labeled patterns to Z3 `re` constraints. Wire into the existing proof pipeline so string operations can be verified.

## Coding Standards

Every function in this plan must follow:
- **Max 2 nesting levels deep** — extract helpers, guard clauses, early returns
- **`///` doc comments** on every `fn`, `struct`, `enum`, `mod`
- **`// 2026-07-14:` comments** explaining why each change exists
- **No `else-if` chains deeper than 1** — early returns instead

## Files

| File | Phase | Purpose |
|------|-------|---------|
| `src/ast/layout.rs` | B | `LayoutPattern` enum |
| `src/bvir/layout.rs` | C | DSL → LayoutPattern parser |
| `src/parser/definitions.rs` | A | `layout <~` syntax in type bodies |
| `src/backend/normalizer.rs` | E | Bit-shuffling synthesis helpers |
| `docs/architecture/layout-dsl.md` | — | Architecture document |
