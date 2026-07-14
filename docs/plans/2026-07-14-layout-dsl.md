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

### Access model

Every named field in a layout has an access level:

| Syntax | Access | Example |
|--------|--------|---------|
| `name: N` | Read-only — compiler generates a getter but no setter | `sign: 1` |
| `!name: N` | Read/write — compiler generates both getter and setter | `!crc: 32` |
| `$name: N` | Read-only structural field — cannot be written directly, controls dependent fields | `$length: 32` |

The compiler guarantees that writing to a `!`-prefixed field only touches the bits assigned to that field. It cannot corrupt adjacent bits because the layout defines exactly which bit range belongs to each field.

`$`-prefixed fields are structural invariants. Writing to `$length` would invalidate the `data` region that depends on it. These fields are readable through accessors but never directly writable — the only way to change them is through a proper API call (e.g., `list.append(x)`) that reallocates and validates.

### Typed references for generic collections

```brief
type List<T> <: Bits {
    bytes <~ 16;
    layout <~ le: [$length: 64, data_ptr: 64, elements: {$length, $T}];
}

type HashMap<K, V> <: Bits {
    bytes <~ 24;
    layout <~ le: [$capacity: 64, $length: 64, seed: 64,
                   slots: {$capacity, ($K, $V)}];
}
```

A typed reference `{$name, T}` tells the compiler:
1. The field `$name` stores the element count
2. The next field is a pointer to `$name` consecutive elements of type T
3. Each element follows T's own layout rules
4. The normalizer can emit bounds-checking: every index access is validated against `$name`

For `HashMap<K,V>`, the slots field is `$capacity` consecutive key-value pairs. The compiler knows each pair occupies `sizeof(K) + sizeof(V)` bytes, and the key/value within each pair follow their respective layouts.

The `$T`, `$K`, `$V` placeholders are generic type parameters resolved at monomorphization. The normalizer substitutes the concrete type's layout when the generic is instantiated.

### Auto-accessor synthesis

For any type with a fixed-width layout, the compiler auto-generates field accessors:

```brief
chunk.length   // reads bits 0-31  → getter
chunk.kind     // reads bits 32-63  → getter
chunk.crc      // writes bits ?-?   → getter AND setter (because !crc)
chunk.data     // reads $length elements from the pointer → getter
```

The getter for `!crc` reads the bits and returns the value. The setter takes a value, masks it to the field's bit width, shifts it to the field's position, and ORs it into the type value — without corrupting adjacent fields.

For collection types:

```brief
list[5]        // bounds-check: 5 < length → GEP → load
map[key]       // hash key → find slot → return value
```

These accessors are generated by the normalizer during Phase F. They are not user-visible functions — they are compiler-internal lowering rules. The user writes `list[i]` and the compiler emits the correct bounds-checked GEP.

## Implementation Phases (Updated)

### Phase A — Parser: `layout <~ ...` in type bodies

Extend `parse_type_definition()` in `src/parser/definitions.rs` to recognize `layout` as a metadata key in type bodies. The value after `<~` is either `[...]` (fixed-width) or `(...)` (variable-width). Store the parsed pattern string as-is in `TypeDefBody.metadata["layout"]`.

No AST type needed yet — the pattern string is stored verbatim and parsed later.

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
    AnyBytes(u64),                // {N}
    TypedRef { count: String, elem: Box<LayoutPattern> }, // {$name, T}
    VariableRef(String),          // {$name}
    NamedField { name: String, bits: u64, mutable: bool }, //  name: N, !name: N
    StructuralField { name: String, bits: u64 },           // $name: N
    SemanticLabel(String, Box<LayoutPattern>),             // @name: pattern
    GenericParam(String),                                   // $T, $K, $V
}
```

### Phase C — Layout parser (DSL → LayoutPattern)

Create `src/bvir/layout.rs`. A recursive descent parser that takes the pattern string and produces `LayoutPattern`. Max 2 depth.

### Phase D — DFA compiler for literal validation

For variable-width patterns, compile the `LayoutPattern` into a Deterministic Finite Automaton. Walks the byte-level structure of any literal in source code and validates it against the compiled DFA. Runs at compile time during the normalizer pass.

### Phase E — Normalizer bit-shuffling synthesis

For melds that contain `layout { ... }` blocks, the normalizer reads the field mappings and emits bit-shifting/masking instructions:

1. **Identity** — fields match exactly: no shuffle needed.
2. **Reorder** — fields exist in both layouts but at different positions: synthesize `lshr`, `and`, `shl`, `or`.
3. **Partial** — some fields match, some don't: shuffle what can be shuffled, error on the rest.

### Phase F — Auto-accessor synthesis

For any type with a fixed-width layout, the normalizer auto-generates:
- Getters for every named field
- Setters for every `!`-prefixed field
- Bounds-checked index accessors for collection types
- Structural field readers for `$`-prefixed fields (no setter)

These accessors are annotations on AST nodes, not emitted functions. The LLVM backend reads them and emits the correct GEP/shift/mask when the user writes `chunk.crc` or `list[5]`.

### Phase G — SMT proof integration

Translate `@codepoint`-labeled patterns to Z3 `re` constraints. Wire into the proof pipeline so string operations can be verified.

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
