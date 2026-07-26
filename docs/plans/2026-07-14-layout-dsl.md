# Layout DSL Implementation Plan

## The Problem

Types currently have no way to declare their binary layout. `Float32` is known to be 4 bytes, but the compiler doesn't know which bits are sign, exponent, or mantissa. `String` is known to be an 8-byte pointer, but the compiler doesn't know the byte-level encoding of the data it points to.

## The Solution

A `layout <~ <pattern>` metadata key on any type. The `< >` wrapper gives the frontend parser an unambiguous end token. Everything between `<` and `>` is raw layout text consumed at parse time and parsed later by the Layout DSL parser.

### Syntax

```brief
// Fixed-width slicing — bits are known at compile time
type Float32 : Bits {
    bytes <~ 4;
    primitive <~ Float;
    layout <~ <le: [sign: 1, exp: 8, mant: 23]>;
}

// Variable-width pattern — binary regex for strings, protocols
type String : Bits {
    bytes <~ 8;
    primitive <~ String;
    layout <~ <be: (@codepoint: (
        0x00..0x7F |
        0xC2..0xDF 0x80..0xBF |
        ...complete UTF-8 spec...
    ))*>;
}

// Typed reference — list, hashmap, etc.
type List<T> : Bits {
    bytes <~ 16;
    layout <~ <le: [$length: 64, data_ptr: *elements, elements: {$length, $T}]>;
}
```

### The `< >` wrapper

`< >` is already used to denote types (like `<List<Int>>`). In the `layout <~ <...>` context, `<` after `<~` is unambiguous — it opens the layout pattern block, and the matching `>` closes it. The frontend parser collects everything between as raw text, storing it in `TypeDefBody.metadata["layout"]`.

Endianness (`le:`, `be:`) goes INSIDE the `< >`. It's handled by the Layout DSL parser.

## Implementation Phases

### Phase A — Frontend Parser: `layout <~ <...>`

Add recognition of `layout` as a metadata key in type body parsing. When `layout` is followed by `<~`, consume the `< >` block as raw text.

**File:** `src/parser/definitions.rs`

Add a helper `read_layout_body()` that consumes tokens until matching `>` at depth 0, returning the raw text.

**Store as:** `TypeDefBody.metadata["layout"] = PropertyValue::String(raw_text)`

### Phase B — `LayoutPattern` AST Type

Create `src/ast/layout.rs`:

```rust
#[derive(Debug, Clone, PartialEq)]
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
    AnyBytes(u64),                            // {N}
    TypedRef(String, Box<LayoutPattern>),     // {$name, T}
    VariableRef(String),                      // {$name}
    NamedField { name: String, bits: u64, mutable: bool },
    StructuralField { name: String, bits: u64 },
    SemanticLabel(String, Box<LayoutPattern>),
    GenericParam(String),                     // $T, $K, $V
    PointerRef(String),                       // *elements
}

#[derive(Debug, Clone, PartialEq)]
pub enum Endianness {
    Little,
    Big,
    Target,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayoutField {
    pub name: Option<String>,
    pub bits: u64,
    pub mutable: bool,
    pub structural: bool,
}
```

### Phase C — Layout DSL Parser

Create `src/beast/layout.rs`. A recursive descent parser that takes the raw pattern string and produces `LayoutPattern`:

```
layout     ::= endian? pattern
endian     ::= "le:" | "be:"
pattern    ::= "[" fields "]" | "(" alternation ")"
fields     ::= field ("," field)*
field      ::= "!"? "$"? name ":" bit_count
bit_count  ::= integer
alternation ::= sequence ("|" sequence)*
sequence   ::= repetition+
repetition ::= primary ("*" | "?")?
primary    ::= byte_literal | byte_range | any_bytes | typed_ref
               | variable_ref | pointer_ref | group | label
byte_literal ::= "0x" hex hex
byte_range ::= byte_literal ".." byte_literal
any_bytes  ::= "{" integer "}" | "{" variable_ref "}" | "{" typed_ref "}" | "{" range "}"
typed_ref  ::= variable_ref "," type_name "}"
variable_ref ::= "$" identifier
pointer_ref ::= "*" identifier
group      ::= "(" pattern ")"
label      ::= "@" identifier ":" pattern
type_name  ::= identifier | "$" identifier
```

Max 2 levels deep. Use helper extraction for sub-parsers.

### Phase D — Meld Layout Mapping

Inside meld declarations, allow a `layout { ... }` block:

```brief
meld Float32 <:> MyCustomFloat {
    layout {
        sign <:> sign;
        exp  <:> exp;
        mant <:> mant;
    }
}
```

Extend meld parsing to recognize `layout {` and parse field mappings using the `<:>` operator.

### Coding Standards

- **Max 2 nesting levels deep** — extract helpers, guard clauses, early returns
- **`///` doc comments** on every `fn`, `struct`, `enum`, `mod`
- **`// 2026-07-14:` comments** at every modification site
- **No `else-if` chains deeper than 1** — early returns instead

## Files

| File | Phase | Purpose |
|------|-------|---------|
| `src/parser/definitions.rs` | A | `layout <~ <...>` in type body parsing |
| `src/ast/layout.rs` | B | `LayoutPattern` enum, `LayoutField`, `Endianness` |
| `src/beast/layout.rs` | C | DSL → LayoutPattern recursive descent parser |
| `src/parser/definitions.rs` | D | `layout { sign <:> sign; }` in meld parsing |

## Execution Order

| Step | Phase | Est. time |
|------|-------|-----------|
| 1 | A: `read_layout_body()` + `< >` capture in parser | 10 min |
| 2 | B: `LayoutPattern` AST type | 10 min |
| 3 | C: DSL parser (tokenize + recursive descent) | 30 min |
| 4 | D: Meld layout mapping parsing | 10 min |
| 5 | Test: round-trip parse, verify patterns | 15 min |
