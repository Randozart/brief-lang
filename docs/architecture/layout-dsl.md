# Layout DSL Architecture

## What It Is

The Layout DSL is a declarative binary grammar for describing the physical arrangement of bits in any Briv type. It replaces the old opaque `codec` concept with a formal pattern language that the compiler can reason about, validate, transform, and prove.

## Tutorial

This section walks through the Layout DSL from simple to complex. You can read it in order or jump to any example.

### 1. The simplest layout — a named slice

```briv
type FourBytes : Bits {
    maxbits <~ 32;
    layout <~ [first: 16, second: 16];
}
```

Two fields, each 16 bits. `first` occupies bits 0-15, `second` occupies bits 16-31. Endianness is target-adaptive (no `le:` or `be:` prefix).

Reading `x.first` returns bits 0-15 as an integer. Writing to `x.first` changes bits 0-15 without touching bits 16-31.

### 2. Float32 — standard IEEE 754 layout

```briv
type Float32 : Bits {
    maxbits <~ 32;
    primitive <~ Float;
    layout <~ le: [sign: 1, exp: 8, mant: 23];
}
```

- `sign: 1` — bit 31
- `exp: 8` — bits 23-30
- `mant: 23` — bits 0-22

The fields are read-only (no `!` prefix). The compiler generates getters for all three, but the user cannot write `x.sign = 0` directly. Changing bits here would corrupt the float's value.

### 3. Mutable fields with `!`

```briv
type PngChunk : Bits {
    maxbits <~ 96;
    layout <~ be: [$length: 32, kind: 32, data: {$length}, !crc: 32];
}
```

- `$length: 32` — structural field, controls `data` size. Readable, not directly writable.
- `kind: 32` — read-only. User can read it, can't set it.
- `data: {$length}` — variable-length region. The `$length` field determines how many bytes this occupies.
- `!crc: 32` — MUTABLE. The user can read `chunk.crc` and write `chunk.crc = 42`. Writing to `!crc` only touches the last 32 bits — it cannot corrupt `$length`, `kind`, or `data`.

### 4. String — variable-width UTF-8 pattern

```briv
type String : Bits {
    maxbits <~ 64;
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

This is the full UTF-8 specification as a binary regular expression:
- `0x00..0x7F` — single-byte ASCII character (0-127)
- `0xC2..0xDF 0x80..0xBF` — two-byte character (128-2047)
- `0xE0 0xA0..0xBF {0x80..0xBF, 2}` — three-byte, special case for 0xE0
- Each alternative matches exactly one Unicode codepoint, labeled `@codepoint`
- The outer `*` repeats for the entire string

The compiler's DFA engine validates every string literal against this pattern at compile time. If the literal contains invalid UTF-8, compilation fails with a precise error.

### 5. List<T> — generic collection with typed reference

```briv
type List<T> : Bits {
    maxbits <~ 128;
    layout <~ le: [$length: 64, data_ptr: 64, elements: {$length, $T}];
}
```

The typed reference `{$length, $T}` means:
1. `$length` stores the number of elements
2. There are `$length` consecutive elements of type T starting at `data_ptr`
3. Each element follows T's own layout rules

Accessing `list[5]` generates: bounds check `5 < $length`, GEP into `data_ptr` at offset `5 * sizeof(T)`, load value.

### 6. HashMap<K,V> — typed pair array

```briv
type HashMap<K, V> : Bits {
    maxbits <~ 192;
    layout <~ le: [$capacity: 64, $length: 64, seed: 64,
                   slots: {$capacity, ($K, $V)}];
}
```

The `($K, $V)` pair type is a two-element tuple. The compiler knows each pair occupies `sizeof(K) + sizeof(V)` bytes. Key and value follow their respective layouts within each slot.

### 7. Melding layouts — bit-shuffling between types

```briv
meld Float32 <:> MyCustomFloat {
    layout {
        sign <:> sign;
        exp  <:> exp;
        mant <:> mant;
    }
}
```

This declares that `Float32` and `MyCustomFloat` are structurally equivalent at the field level. The normalizer reads both layouts, computes bit positions, and auto-synthesizes the shuffle instructions. No manual bit-shifting code required.

### 8. What you cannot do

```briv
// ERROR: $length is structural, cannot be marked mutable
type Bad : Bits { layout <~ be: [$length: 32, !$length: 32]; }

chunk.$length = 42;        // ERROR: $ prefix fields are structural
chunk.data = something;    // ERROR: data is length-dependent, not writeable
chunk.nonexistent = 1;     // ERROR: no field named nonexistent in layout
```

The compiler enforces all three at compile time.

## The `#` Prefix — Layout Access

Every layout field and layout operation is accessed with the `#` prefix:

```briv
packet.#magic        // layout field read — compiler reads at known bit position
packet.#crc = 42     // layout field write — masks to field width, only touches those bits
packet.#payload_len  // structural field — $ prefixed, no setter
list.#length         // structural field on a collection
list.#get(i)         // layout operation — declared as op get(i) <~ fn
list.#halve(arg)     // custom layout operation
```

`#` means "this is a compiler-defined thing on this type" — consistent with `Sqrt#`, `Add#`, `Len#` globally. No dot-access without `#` for layout things. The only exception is bracket syntax `list[5]` which desugars to `list.#get(5)`, already documented elsewhere.

## Complete Examples

### HashMap<K, V> with operation bindings

```briv
type HashMap<K, V> : Bits {
    maxbits <~ 192;
    layout <~ le: [$capacity: 64, $length: 64, seed: 64,
                   slots: {$capacity, ($K, $V)}];

    op get(key)    <~ hashmap_lookup(#self, #key);
    op set(key, val) <~ hashmap_insert(#self, #key, #val);
    op len()       <~ field_read(#self.$length);
    op capacity()  <~ field_read(#self.$capacity);
}

// Usage:
map.#get("user:42")       // explicit layout op call
map.#len()                // reads $length field via bound op
map.#capacity()           // reads $capacity field via bound op
map.#set("user:42", user) // inserts via bound op
```

### SecurePacket with inline variable-width payload

```briv
type SecurePacket : Bits {
    maxbits <~ 96;   // header size: magic + version + flags + payload_len + crc
    layout <~ be: [
        magic: 16,
        $version: 8,
        $flags: 8,
        $payload_len: 32,
        payload: {$payload_len, (
            @codepoint: (
                0x00..0x7F |
                0xC2..0xDF 0x80..0xBF |
                0xE0 0xA0..0xBF 0x80..0xBF |
                0xE1..0xEC {0x80..0xBF, 2} |
                0xED 0x80..0x9F 0x80..0xBF |
                0xEE..0xEF {0x80..0xBF, 2} |
                0xF0 0x90..0xBF {0x80..0xBF, 2} |
                0xF1..0xF3 {0x80..0xBF, 3} |
                0xF4 0x80..0x8F {0x80..0xBF, 2}
            )
        )*},
        !crc: 32
    ];
}

// Usage:
packet.#magic            // read the 16-bit magic number at bit 0
packet.#crc = checksum   // write the CRC — only touches last 32 bits
packet.#payload_len      // read the structural payload length field
```

The variable-width `payload` field is inline — it follows directly after the header fields. `maxbits <~ 96` is the header size (magic + version + flags + payload_len + crc). The total runtime footprint is `12 + payload_byte_count`.

## Two Pattern Forms, One Concept

Every type is `Bits(N)`. The `layout` metadata describes how those N bytes are arranged:

| Property | Fixed-width `[...]` | Variable-width `(...)` |
|----------|---------------------|------------------------|
| Bit widths | Known at compile time: `[sign: 1, exp: 8, mant: 23]` | Determined by matching bytes at runtime: `(0x00..0x7F \| ...)*` |
| Field names | Named fields with fixed positions | Semantic labels for logical units |
| Endianness | `le:` / `be:` prefix or target-adaptive | Same |
| Typical use | Float, hardware registers, structs | Strings, protocols, packed formats |

The parser distinguishes them syntactically: `[...]` is always fixed-width slicing, `(...)` is always variable-width pattern.

## Access Model

Every named field in a layout has a compiler-enforced access level:

| Syntax | Access | Compiler generates | Example |
|--------|--------|-------------------|---------|
| `name: N` | Read-only | Getter (reads bits N..M) | `sign: 1` |
| `!name: N` | Read/write | Getter + setter (setter only touches bits N..M) | `!crc: 32` |
| `$name: N` | Structural read-only | Getter only, no setter ever | `$length: 32` |

### Rules

1. **`$`-prefixed fields cannot have `!`**. `$length: 32` controls the size of the `data` region. If you could write to it, the data region would be silently corrupted. The only way to change a `$` field is through a proper API call (`list.append(x)` reallocates and validates).

2. **`!` fields are guaranteed safe**. Writing to `!crc` masks the value to 32 bits and ORs it into the correct bit position. It cannot corrupt `$length` or `kind` because the layout defines exactly which bits belong to each field.

3. **Read-only fields still construct correctly**. You can build a valid value of the type (e.g., `Float32 { sign: 0, exp: 127, mant: 0 }` at construction). The restriction is on *mutation after construction*.

4. **Typed reference fields (`{$name, T}`) are always structural**. They read from the pointer but cannot be assigned directly. Element mutation goes through index accessors (`list[5] = x`), which the compiler bounds-checks against `$length`.

### Accessor auto-generation

For any type with a fixed-width layout, the normalizer auto-generates accessor annotations:

```briv
chunk.$length    // → reads bits 0-31, returns as i32  (structural)
chunk.kind       // → reads bits 32-63, returns as i32  (read-only)
chunk.crc        // → reads bits ?-?  (getter)
chunk.crc = 42   // → masks to 32 bits, shifts, ORs   (setter, because !crc)
chunk.data       // → reads $length elements from pointer (typed ref)
list[5]          // → bounds-check 5 < $length, GEP, load
map[key]         // → hash key, find slot, return value
```

These accessors are compiler-internal lowering rules. The user writes `chunk.crc` and the compiler emits the correct shift/mask — no function call overhead.

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

The layout parser (`src/beast/layout.rs`) takes the raw string and produces the typed AST:

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

```briv
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

```briv
let pi: Float32 = 0x40490FDB;  // 3.14159... in IEEE 754
```

The normalizer's DFA engine compiles the `layout` pattern into a minimal DFA, then runs it over the literal's bytes. If accepted, the literal is valid. If rejected, compile error.

For string literals:

```briv
let name: String = "hello";
```

The DFA engine walks each byte of `"hello"` through the UTF-8 DFA. If all bytes are accepted, the string is proven valid UTF-8 at compile time.

### 5. SMT proof time — `@codepoint` labels

When a variable-width pattern contains `@codepoint` labels:

```briv
layout <~ (@codepoint: (0x00..0x7F | 0xC2..0xDF 0x80..0xBF | ...))*;
```

The SMT solver receives:
- The `@codepoint` boundary as a Z3 `re` constraint
- A string-length function that counts `@codepoint` units, not bytes
- The ability to prove that `StringLength#(s)` returns the correct character count without ever seeing a byte

## Meld Layout Mapping

### Syntax

```briv
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
| `src/beast/layout.rs` | Recursive descent DSL → LayoutPattern parser |
| `src/parser/definitions.rs` | `layout <~` and `layout { ... }` in meld parsing |
| `src/backend/normalizer.rs` | Bit-shuffling synthesis, DFA validation |
| `config/ctd-llvm-mappings.toml` | (renamed from llvm-primitives.toml; section headers: [ctd.*]) |
| `docs/plans/2026-07-14-layout-dsl.md` | Implementation plan |

## Inline vs Heap Indirection

The `*` operator distinguishes inline variable-width fields from heap-allocated ones:

```briv
// Inline: data follows directly after the header fields
type PngChunk : Bits {
    layout <~ be: [$length: 32, kind: 32, data: {$length}, !crc: 32];
}

// Heap: data_ptr points to the elements region on the heap
type List<T> : Bits {
    layout <~ le: [$length: 64, data_ptr: *elements, elements: {$length, $T}];
}
```

Without `*`, a variable-width field is inline. With `*`, the preceding field is a pointer to memory elsewhere.

## Operation Bindings for Collections

Layout describes structure. `op` bindings describe behavior. The same `op` syntax used for arithmetic operations extends to collection operations:

```briv
type HashMap<K, V> : Bits {
    maxbits <~ 192;
    layout <~ le: [$capacity: 64, $length: 64, seed: 64,
                   slots: {$capacity, ($K, $V)}];

    op get(key)    <~ hashmap_lookup(#self, #key);
    op set(key, val) <~ hashmap_insert(#self, #key, #val);
    op len()       <~ field_read(#self.$length);
}

type List<T> : Bits {
    maxbits <~ 128;
    layout <~ le: [$length: 64, data_ptr: *elements, elements: {$length, $T}];

    // Auto-synthesizable from layout alone
    op get(i)    <~ field_index(#self.$data_ptr, #i, T);
    op set(i, v) <~ field_index(#self.$data_ptr, #i, T) <- #v;
    op len()     <~ field_read(#self.$length);
}
```

`field_read` and `field_index` are builtins:
- `field_read(#self.$name)` — emit the correct bit slice for field `$name`
- `field_index(#self.$ptr, #i, T)` — emit `bounds_check(i < $len); ptr + i * sizeof(T); load`

User-defined functions (`hashmap_lookup`, `hashmap_insert`) are provided by the standard library. The compiler validates signatures but doesn't need to know the internal algorithm. The layout provides the memory shape for SMT-level reasoning; the bound function provides the runtime behavior.

## Padding

Anonymous fields `_: N` for padding:

```briv
type AlignedStruct : Bits {
    maxbits <~ 64;
    layout <~ le: [a: 8, _: 24, b: 32];
}
```

## Concurrency

Bitfield read-modify-write is non-atomic. The `atomic:` prefix on a `!` field generates atomic CAS loops:

```briv
type SharedFlags : Bits {
    maxbits <~ 32;
    layout <~ le: [atomic: !flag: 1, _: 31];
}
```

## Runtime Validation

`ValidateAs#<T>()` compiles the layout's DFA at compile time and runs it at runtime over a byte buffer:

```briv
let bytes: Bytes = socket.read();
let chunk = ValidateAs#<PngChunk>(bytes);  // explicit, opt-in
```
