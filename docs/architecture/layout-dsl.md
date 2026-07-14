# Layout DSL Architecture

## What It Is

The Layout DSL is a declarative binary grammar for describing the physical arrangement of bits in any Brief type. It replaces the old opaque `codec` concept with a formal pattern language that the compiler can reason about, validate, transform, and prove.

## Two Pattern Forms, One Concept

Every type is `Bits(N)`. The `layout` metadata describes how those N bytes are arranged:

| Property | Fixed-width `[...]` | Variable-width `(...)` |
|----------|---------------------|------------------------|
| Bit widths | Known at compile time: `[sign: 1, exp: 8, mant: 23]` | Determined by matching bytes at runtime: `(0x00..0x7F \| ...)*` |
| Field names | Named fields with fixed positions | Semantic labels for logical units |
| Endianness | `le:` / `be:` prefix or target-adaptive | Same |
| Typical use | Float, hardware registers, structs | Strings, protocols, packed formats |

The parser distinguishes them syntactically: `[...]` is always fixed-width slicing, `(...)` is always variable-width pattern.

## How the Compiler Uses Layout Information

### 1. Parse time — `layout <~ <pattern>` in type bodies

The parser sees `layout`, reads the token after `<~`, and stores the pattern string as-is in `TypeDefBody.metadata["layout"]`. No validation yet — just storage.

```
Source:  layout <~ le: [sign: 1, exp: 8, mant: 23];
Parsed:  TypeDefBody.metadata["layout"] = PropertyValue::String("le: [sign: 1, exp: 8, mant: 23]")
```

### 2. Resolution time — TypeUniverse population

The `TypeUniverse` reads `TypeDefBody.metadata` and copies entries into `ResolvedType.properties`. After resolution:

```
ResolvedType.properties["layout"] = PropertyValue::String("le: [sign: 1, exp: 8, mant: 23]")
```

The pattern is still a raw string. No AST node yet.

### 3. Normalizer time — DSL → LayoutPattern → actions

The normalizer is the first stage that needs to understand the layout. It:

**a) Parses the DSL string into `LayoutPattern` AST**

The layout parser (`src/bvir/layout.rs`) takes the raw string and produces the typed AST:

```rust
LayoutPattern::Slice(vec![
    LayoutField { name: "sign", bits: 1, endian: Little },
    LayoutField { name: "exp", bits: 8, endian: Little },
    LayoutField { name: "mant", bits: 23, endian: Little },
])
```

**b) Validates the pattern**

For fixed-width patterns: sum of all field bit widths must equal `bytes * 8`.

For variable-width patterns: compiles the pattern to a DFA. If the DFA is non-deterministic or unresolvable, error.

**c) Bit-shuffling synthesis (for meld layout blocks)**

When a meld has a `layout { ... }` block:

```brief
meld Float32 <:> MyFloat {
    layout {
        sign <:> sign;
        exp  <:> exp;
        mant <:> mant;
    }
}
```

The normalizer reads both layouts, maps fields by name, and synthesizes:

```llvm
; MyFloat bit arrangement: [mant: 23, exp: 8, sign: 1]
; Float32 bit arrangement: [sign: 1, exp: 8, mant: 23]

; Auto-synthesized by normalizer:
%tmp = load i32, ptr %myfloat
%sign = lshr i32 %tmp, 31         ; extract bit 31
%exp  = and i32 %tmp, 2130706432  ; mask bits 23-30
%mant = and i32 %tmp, 8388607     ; mask bits 0-22
%sign_shifted = shl i32 %sign, 31 ; move to bit 31
%exp_shifted  = lshr i32 %exp, 0  ; stays at bits 23-30
%mant_shifted = shl i32 %mant, 0  ; stays at bits 0-22
%result = or i32 %sign_shifted, %exp_shifted
%result = or i32 %result, %mant_shifted
```

The normalizer does NOT generate code directly. It attaches annotations to the AST nodes that tell the LLVM backend which shifts and masks to emit. The actual codegen still writes the IR — it just reads the annotations instead of re-deriving the logic.

### 4. Literal validation time — DFA engine

When the compiler encounters a literal:

```brief
let pi: Float32 = 0x40490FDB;  // 3.14159... in IEEE 754
```

The normalizer's DFA engine compiles the `layout` pattern into a minimal DFA, then runs it over the literal's bytes. If accepted, the literal is valid. If rejected, compile error.

For string literals:

```brief
let name: String = "hello";
```

The DFA engine walks each byte of `"hello"` through the UTF-8 DFA. If all bytes are accepted, the string is proven valid UTF-8 at compile time.

### 5. SMT proof time — `@codepoint` labels

When a variable-width pattern contains `@codepoint` labels:

```brief
layout <~ (@codepoint: (0x00..0x7F | 0xC2..0xDF 0x80..0xBF | ...))*;
```

The SMT solver receives:
- The `@codepoint` boundary as a Z3 `re` constraint
- A string-length function that counts `@codepoint` units, not bytes
- The ability to prove that `StringLength#(s)` returns the correct character count without ever seeing a byte

## Meld Layout Mapping

### Syntax

```brief
meld TypeA <:> TypeB {
    layout {
        field_a <:> field_b;
    }
}
```

The `<:>` inside `layout { }` reuses the existing meld token. It maps a named field from TypeA's layout to a named field from TypeB's layout.

### Layout resolution

The compiler checks:
1. Both TypeA and TypeB have a `layout` property
2. Both layouts are fixed-width (variable-width patterns cannot be shuffled by position)
3. Both layouts have the same total bit width
4. Every named field in the mapping exists in its respective layout

If all checks pass, the normalizer synthesizes the bit-shuffling logic between the two layouts.

### No auto-shuffle

Without a `layout { ... }` meld block, two types with matching total widths are NOT auto-shuffled. The meld is the explicit declaration of structural equivalence.

## Type Layout — Which Backend Reads What

| Metadata | LLVM normalizer | CIRCT normalizer | Webstack normalizer |
|----------|----------------|------------------|---------------------|
| `bytes` | Storage width | Bit width | Storage width |
| `primitive` | Kept — operation dispatch | Stripped — not needed | Kept — JS type mapping |
| `layout` | Bit-shuffling synthesis | Field position generation | Stripped — JS uses dynamic types |
| `le:` / `be:` | Emit byte-swap if mismatched | Emit endianness adapter | Stripped — JS handles endianness |

## The Language Grammar (Formal)

```
layout      ::= endian? "[" fields "]" | endian? "(" pattern ")"
endian      ::= "le:" | "be:"
fields      ::= field ("," field)*
field       ::= ref? name ":" integer
ref         ::= "$"
name        ::= identifier
pattern     ::= alternation
alternation ::= sequence ("|" sequence)*
sequence    ::= repetition+
repetition  ::= primary ("*" | "?")?
primary     ::= byte_literal | byte_range | any_bytes | variable_ref
               | "(" pattern ")" | label ":" pattern | "[" fields "]"
byte_literal ::= "0x" hex hex
byte_range  ::= byte_literal ".." byte_literal
any_bytes   ::= "{" integer "}" | "{" variable_ref "}" | "{" range "}"
variable_ref ::= "$" identifier
label       ::= "@" identifier
range       ::= integer ".." integer
```

## Files

| File | Purpose |
|------|---------|
| `src/ast/layout.rs` | `LayoutPattern` enum, `LayoutField` struct, endianness |
| `src/bvir/layout.rs` | Recursive descent DSL → LayoutPattern parser |
| `src/parser/definitions.rs` | `layout <~` and `layout { ... }` in meld parsing |
| `src/backend/normalizer.rs` | Bit-shuffling synthesis, DFA validation |
| `config/llvm-primitives.toml` | (unaffected — still handles type→LLVM string mapping) |
| `docs/plans/2026-07-14-layout-dsl.md` | Implementation plan |
