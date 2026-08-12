# Extensible Types — Vision Document

**See also**: `docs/plans/2026-07-11-extensible-types-comprehensive.md`
for the executable implementation plan. This document is the design thesis
and end-state picture — it explains WHY, the comprehensive plan explains
HOW in per-step detail.

## The Thesis

`type` is Briev's metaprogramming keyword. It defines how bits are
interpreted, stored, manipulated, serialized, and codegen'd. Everything
about a type should be user-definable from first principles — no magic, no
hardcoded compiler internals. A user should be able to define a type so
completely that `let fourteen: RomanNumeral = XIV` is a valid Briev
statement, driven entirely by the type universe.

## Current State

`ResolvedType` has ~30 hardcoded fields (`bytes`, `alignment`, `llvm_type`,
`box_op`, `codec`, `on_exit`, `tbaa_node`, `insert_at`, `extract_from`,
etc.). Each one is a bespoke match arm in codegen. Adding a new type
property requires:
1. A new field on `ResolvedType`
2. A match arm in `resolve_type_def()` to recognize the binding name
3. Codegen changes to consume the new field

This is the opposite of extensibility. Users cannot define new type
properties without modifying the compiler.

## Vision: Generic Property System

### Step 1: Properties become key-value in TypeUniverse

`ResolvedType` sheds its 30 hardcoded fields for a single
`properties: HashMap<String, PropertyValue>`:

```rust
pub enum PropertyValue {
    U64(u64),
    Bool(bool),
    String(String),
    Expr(Box<Expr>),           // for complex computed properties
    CodecPath(Vec<String>),    // path reference to codec definition
    Intrinsic(String),         // for op → intrinsic, box/unbox, etc.
}
```

Known properties (like `bytes`, `alignment`) fall through to codegen via
`.get("bytes")` instead of `.bytes`. Unknown properties are stored but not
acted on — users can annotate types with arbitrary metadata.

### Step 2: Codec paths

A property can reference an external codec definition:

```briev
type RomanNumeral {
    value: UInt16;
    codec <~ import "encodings/roman.bv" : RomanCodec;
};
```

The compiler ingests the codec file at compile time and uses it to:
- Derive the LLVM representation
- Register the type's literal syntax parser (so `XIV` is valid)
- Register serialization/deserialization intrinsics
- Generate validation guards

The codec file is itself Briev:

```briev
// encodings/roman.bv
codec RomanCodec for UInt16 {
    // Custom literal parser: maps "IV" → 4, "XIV" → 14, etc.
    parse(input: String) -> Result<UInt16, ParseError> {
        term parse_roman(input);
    };

    // Custom formatter: maps 14 → "XIV"
    format(value: UInt16) -> String {
        term format_roman(value);
    };

    // Constraints on valid values
    [value > 0];
    [value < 4000];
};
```

The `codec` keyword here is new — it declares a codec implementation that
the compiler links into the type system at the property resolution step.

### Step 3: Custom literal syntax

When a type has a `parse` handler registered (via codec), the parser
recognizes literals of that type. In the current parser, literal tokens are
hardcoded: `Token::Integer`, `Token::String`, `Token::BoolTrue`,
`Token::Float`, etc.

With extensible types, after type resolution, a second pass registers
custom literal parsers. When the parser encounters an identifier that
resolves to a type name, it checks whether that type has a registered
literal syntax handler. If so, the next token is parsed as that type's
literal:

```briev
let x: Int = 42;              // standard integer literal
let r: RomanNumeral = XIV;    // custom literal — parsed by RomanCodec.parse
let s: String = "hello";      // standard string literal
let b: Binary = 0b101010;     // could be a custom bit-string type
```

This requires:

1. **Type resolution before parsing bodies.** Currently, type resolution
   (`TypeUniverse::build()`) happens before codegen but AFTER parsing. For
   custom literals to work, we need type resolution during parsing — a
   forward pass that resolves types from declarations before the parser
   encounters their literals.

2. **A plugin callback for literal tokens.** The parser maintains a
   `HashMap<String, LiteralParser>` where codec declarations register their
   `parse` handler. When the parser sees `: TypeName` in a variable
   declaration, it checks if the type has a registered literal parser. If
   so, the next expression is parsed through that handler instead of the
   standard expression parser.

### Step 4: Operators as plugin calls

Currently, operators are resolved through `OpRune` → intrinsic mappings in
the TypeUniverse. With the generic property system, operators become just
another property:

```briev
type RomanNumeral {
    value: UInt16;
    op Add(RomanNumeral) -> RomanNumeral = roman_add#;
    op Add(Int) -> RomanNumeral = roman_add_int#;
    op ToString() -> String = format_roman#;
};
```

The `op` keyword is already supported. The only change is that the
operator → intrinsic mapping moves from a hardcoded `operators` HashMap on
`ResolvedType` to the generic `properties` system — same semantics, just
no special-case struct field.

### Step 5: LLVM codegen extensibility

Today, LLVM type emission is hardcoded:

```rust
match ty {
    Type::Custom("Float") => "float",
    Type::Custom("Int") => "i64",
    Type::Custom("String") => "i8*",
    ...
}
```

With the property system, `llvm_type` is just another property — already
true in the TypeUniverse. A type declares its LLVM representation:

```briev
type RomanNumeral {
    value: UInt16;
    llvm <~ "i16";
    bytes <~ 2;
    alignment <~ 2;
};
```

And the codegen queries `properties.get("llvm_type")` instead of matching
on hardcoded type names. This is already partially implemented (Phase 7A).
The vision completes it — NO hardcoded type-name matches in codegen.

### Step 6: Briev as universal bridge

With extensible types, the GLUE bridge is no longer a separate protocol.
It IS the type system:

- **Foreign type** = a Briev `type` declaration with a codec
- **Zero-copy projection** = the type's slot layout matches the foreign
  struct layout (same byte offsets)
- **Meld** = declaring that two types share the same bit interpretation
  (same codec, different names)
- **Export** = telling the compiler "this type crosses the FFI boundary"

A Python `PyLongObject` is just a type:

```briev
type PyLongObject {
    ob_refcnt: UInt64;
    ob_type: Ptr<Byte>;
    ob_size: Int64;
    ob_digit: UInt32Array;   // variable-length at end of struct
    codec <~ import "codecs/cpython36.bv";
};
```

A C `struct timespec` is just a type:

```briev
type Timespec {
    tv_sec: Int64;
    tv_nsec: Int64;
    codec <~ import "codecs/posix.bv";
};
```

Both register in the TypeUniverse. Both participate in field access,
export, meld, and codegen. The compiler doesn't need to know about CPython
or POSIX — it just stores properties and lets codec handlers drive the
interpretation.

## Migration path

| Phase | What changes | Who benefits |
|-------|-------------|--------------|
| Now | Slot syntax in `type` + `struct` auto-registers in TypeUniverse | GLUE bridge, String as user type |
| Phase A | `ResolvedType` fields → HashMap (generic properties) | Any codec or property extension |
| Phase B | Codec declarations + compile-time ingestion | Custom serialization formats |
| Phase C | Custom literal parsers (type resolution during parsing) | DSL-like type usage |
| Phase D | Remove all hardcoded type-name matches in codegen | Compiler becomes truly extensible |

## Non-goals

- `struct` does NOT become extensible. `struct` is organized data — it
  references types but does not define them.
- Existing `type` syntax does not break. All current property bindings
  continue to work.
- Performance is not sacrificed. The generic property system is resolved at
  compile time — runtime is the same as hardcoded fields.

## Risks

- **Complexity**: The generic property system is more complex than
  hardcoded fields. Property lookups replace direct field access, which may
  obscure type resolution logic.
- **Parser ordering**: Custom literal syntax requires type resolution
  during parsing, which is a significant change to the compilation
  pipeline. Currently, parsing → type checking → codegen is strictly
  sequential.
- **Error messages**: Generic properties mean less specific error messages
  for unknown/malformed properties. We'd need a property linting pass.

## Summary

A type answers one question: "what do these bits mean?" The answer should
be fully user-definable through property bindings, slot declarations, and
codec files — no compiler magic. Briev becomes the universal bridge not
because it has hardcoded FFI support for N formats, but because its type
system can express ANY format from first principles.
