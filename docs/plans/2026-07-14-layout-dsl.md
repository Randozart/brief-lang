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

### Inline vs Heap Indirection

The DSL must distinguish data that follows inline from data behind a pointer. The `*` operator binds a pointer field to its target region:

```brief
// Inline variable-width: data follows inline after the header fields
type PngChunk <: Bits {
    bytes <~ 12;
    layout <~ be: [$length: 32, kind: 32, data: {$length}, !crc: 32];
}

// Heap variable-width: * binds data_ptr to the elements region
type List<T> <: Bits {
    bytes <~ 16;
    layout <~ le: [$length: 64, data_ptr: *elements, elements: {$length, $T}];
}
```

Without `*`, a variable-width field `{$length}` is inline — it occupies space immediately after the preceding field. With `*`, the preceding field is a pointer to memory elsewhere, and the variable-width region describes the data at that pointer.

This also resolves the `bytes <~ N` ambiguity. For inline variable types, `bytes` is the minimum size (header + minimum inline data). For heap types, `bytes` is the handle size only — the total footprint is `bytes + total_pointed_to_size`.

## Operation Bindings for Collections

Layout describes structure. Operation bindings describe behavior. The `op` keyword binds a collection operation to a function:

```brief
type HashMap<K, V> <: Bits {
    bytes <~ 24;
    layout <~ le: [$capacity: 64, $length: 64, seed: 64,
                   slots: {$capacity, ($K, $V)}];

    // Collection operations — bound to functions
    op get(key)    <~ hashmap_lookup(#self, #key);
    op set(key, val) <~ hashmap_insert(#self, #key, #val);
    op len()       <~ field_read(#self.$length);      // auto-synthesizable
    op capacity()  <~ field_read(#self.$capacity);    // auto-synthesizable
    op iter()      <~ hashmap_iter(#self);            // iterator binding
}
```

The compiler knows:
- `#self` is the HashMap value
- `$length` and `$capacity` are structural fields — `field_read` is a builtin that emits the correct bit slice
- `get` and `set` call user-provided functions — the compiler validates the signatures but doesn't care about internal behavior
- `iter` returns an iterator — the layout tells the compiler the slot structure for SMT-level reasoning

For simpler collections, the compiler can auto-synthesize:

```brief
type List<T> <: Bits {
    bytes <~ 16;
    layout <~ le: [$length: 64, data_ptr: *elements, elements: {$length, $T}];

    // These can be auto-synthesized from layout alone
    op get(i)    <~ field_index(#self.$data_ptr, #i, T);
    op set(i, v) <~ field_index(#self.$data_ptr, #i, T) <- #v;
    op len()     <~ field_read(#self.$length);
}
```

`field_index` is a builtin that emits: `bounds_check(i < $length); ptr = data_ptr + i * sizeof(T); load/store`.

The same pattern extends to any collection:

```brief
type RingBuffer<T> <: Bits {
    bytes <~ 24;
    layout <~ le: [$capacity: 64, $head: 64, $tail: 64,
                   slots: {$capacity, $T}];

    op push(v)   <~ ring_push(#self, #v);
    op pop()     <~ ring_pop(#self);
    op len()     <~ ring_len(#self);
}
```

## Padding

Anonymous fields `_: N` for padding bits that don't have a semantic name:

```brief
type AlignedStruct <: Bits {
    bytes <~ 8;
    layout <~ le: [a: 8, _: 24, b: 32];
    // a occupies bits 0-7, 24 bits of padding, b occupies bits 32-63
}
```

## Concurrency

Bitfield read-modify-write is non-atomic. The compiler restricts `!` writable fields to thread-local contexts by default. For atomic access:

```brief
type SharedFlags <: Bits {
    bytes <~ 4;
    layout <~ le: [atomic: !flag: 1, _: 31];
    // atomic: prefix generates atomic CAS loop for writes
}
```

The `atomic:` prefix on a `!` field tells the normalizer to emit an atomic compare-and-swap loop instead of a non-atomic read-modify-write.

## Runtime Validation

Compile-time DFA validation handles literals. For runtime data (network, file I/O):

```brief
let bytes: Bytes = socket.read();
let chunk = bytes.ValidateAs#<PngChunk>();
// The compiler generates a DFA-based validator from the layout
// If validation fails: error with precise byte position
```

The `ValidateAs#<T>()` call compiles the layout's DFA at compile time and runs it at runtime over the byte buffer. This is an explicit call — the compiler does not inject hidden runtime checks on every dereference. Performance overhead is opt-in.

## Auto-Accessor Synthesis

For types with fixed-width layouts, the normalizer auto-generates:

```brief
// Given: layout <~ le: [$length: 32, kind: 32, !crc: 32]

chunk.$length           // → getter: reads bits 0-31
chunk.kind              // → getter: reads bits 32-63
chunk.crc               // → getter: reads bits 64-95
chunk.crc = 42          // → setter: mask to 32 bits, shift, OR (because !crc)

// Given: layout <~ le: [... data_ptr: *elements, elements: {$length, $T}]

list[5]                 // → bounds-check 5 < $length, GEP, load
list[5] = val           // → bounds-check, GEP, store
list.len()              // → field_read($length)
```

These are compiler-internal lowering rules. Zero function call overhead.

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
