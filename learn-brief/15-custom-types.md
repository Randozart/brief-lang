# Defining Custom Types

Brief lets you define types that behave like built-in `Int`, `Float`, and
`String`. The same hashword protocol system that drives primitives also
drives your custom types — no special compiler support needed.

## 1. `type MyType : #Protocol { ... }`

Use the `type` keyword with a protocol hashword to define a new type. Layout
comes from fields; the protocol provides its own self-arithmetic; overloads
are declared **RHS-only** (the LHS is the declaring type):

```brief
type MyInt : #Int {
    data: Bits<64>;              // layout: 8 bytes
    op Add: func(#L, #R);        // binding form — the generic self-add
    op Add(Float): func(#L, #R); // RHS-only overload: MyInt + Float
    op Parse(Decimal): parse_num(#L);  // literal construction: 42 → MyInt
};
```

- **Layout**: determined by fields (`data: Bits<N>`, `field: Type`)
- **Self-arithmetic is implicit**: a `#Int`-protocol member already knows how
  to add to itself — you only declare overloads whose RHS differs from the
  declaring type (`op Add(Float)`, declared on the LHS type)
- **The two-variant form is removed**: `op Add(#Int, #Int)` never lists the
  LHS — the declaring type/protocol IS the left operand
- **Parse ops**: `op Parse(Decimal): fn(#L)` — custom literal construction

No `!> maxbits:`, `!> alignment:`, `!> llvm:`, `!> storage:`, `default_width`,
`commuting`, or LLVM opcode strings needed.

## 2. Protocol-Centric Ops

The hashword in an op signature tells the backend to dispatch to its
intrinsic handler for that category:

```brief
type Bfloat16 : #Float {
    data: Bits<16>;
    op Add(#Float): bfloat_add(#L, #R);   // protocol-family RHS overload
    op Mul(#Float): bfloat_mul(#L, #R);
    op Parse(#Float): identity(#L);       // identity — literal IS float
};
```

| Op form | Meaning |
|---|---|
| `op Add: func(#L, #R);` | Binding form — operands are `#L`/`#R` placeholders |
| `op Add(Float): func(#L, #R);` | RHS-only overload for the concrete type `Float` |
| `op Add(#Float): func(#L, #R);` | RHS-only overload for the whole `#Float` protocol family |
| `op Parse(Decimal): fn(#L);` | Custom literal construction |

Functions bound to ops via `: fn(...)` are emitted with LLVM's `alwaysinline`.

### `prop` — Metaproperty Declarations

Alongside `op`, types declare metaproperties via `prop`:

```brief
type MyString: Bits #String {
    op CastTo(#Bits) = my_encode(#L);
    prop Size = my_chars(#L);     // .^Len → character count
    prop Bytes = my_byte_len(#L); // .^^Bytes → encoded byte length
};
```

A `prop` declares a metaproperty accessible via reflection (`expr.^Name` / `expr.^^Name`). The compiler
resolves it through the protocol chain — `#String` provides `Size` and `Bytes`,
but a custom type can override them. Same resolution mechanism as `op`.

**Built-in metaproperties by protocol:**

| Protocol | Metaproperties |
|----------|---------------|
| `#Bits` | `.^^Bits`, `.^^Alignment` |
| `#String` | `.^Len`, `.^^Bytes` |
| Any type | User-defined via `prop` |

## 3. Protocol Variants

Hashwords can be parameterized by protocol variant:

```brief
type ASCIIString {
    data: Bits<64>;
    len: Bits<64>;
    op CastTo(#String<UTF8>) = ASCII_to_UTF8(#L);   // produce UTF-8
    op CastFrom(#String<UTF8>) = UTF8_to_ASCII(#L);  // consume UTF-8
};
```

Bare hashwords resolve to their default variant at parse time:

| Hashword | Default variant | Also writable as |
|---|---|---|
| `#String` | `UTF8` (for all files) | `#String<UTF8>`, `#String<ASCII>` |
| `#Float` | `IEEE754` | `#Float<IEEE754>` |
| `#Char` | `unicode` | `#Char<unicode>`, `#Char<ASCII>` |

Cross-variant calls require explicit protocol. A `.bv` file calling an `.ebv`
function using `#String` produces a compile error if the default variants
differ.

## 4. Literal Construction via Parse Ops

Types declare how they are constructed from source text:

```brief
type HexColor {
    data: Bits<24>;
    op Parse(Bare) = parse_hex(#L);              // FF00FF → HexColor
    op Cast(#Bits);
};

type RomanNumeral {
    data: Bits<16>;
    op Parse(Bare) = roman_from_identifier(#L);  // XIV → RomanNumeral
    op CastTo(#Int) = roman_to_int(#L);           // RomanNumeral → Int
};

type BFloat16 {
    data: Bits<16>;
    op Parse(Decimal, pre: "0x") = hex_to_bfloat(#L);  // 0x3F80 → 1.0
    op Parse(Decimal, suf: "bf") = suffix_bf(#L);       // 1.5bf → bfloat
    op Parse(#Float);                                     // identity
};
```

When the compiler encounters a literal, it checks the target type's Parse ops:

1. `op Parse(#Category)` — identity, zero-cost, no conversion
2. `op Parse(Form, pre: "prefix")` — discriminator match, most specific
3. `op Parse(Form, suf: "suffix")` — discriminator match, most specific
4. `op Parse(Form)` — fallback, matches any literal of that form

## 5. Protocol Conversion (CastTo / CastFrom)

The `CastTo`/`CastFrom` pair handles conversion between types via the
protocol category:

```brief
type Latin1String {
    data: Bits<64>;
    len: Bits<64>;
    op CastTo(#String) = latin1_to_UTF8(#L);       // Latin1 → UTF-8
    op CastFrom(#String) = UTF8_to_latin1(#L);      // UTF-8 → Latin1
};
```

At compile time, the compiler inlines both functions and LLVM's `InstCombine`
removes redundant operations. A Latin1→UTF-8→Latin1 round-trip for ASCII data
(0–127) collapses to a no-op.

Cast resolution priority:
1. `meld Source <-> Target` — structural equivalence
2. `op Cast(Target)` on source — direct type-to-type
3. `CastTo(#Category)` → `CastFrom(#Category)` — protocol path
4. Implicit `Cast(#Bits)` — raw bytes, always available

## 6. Protocol Satisfaction (Compile-Time Verification)

For every type with both a Parse op and a Cast/CastTo op, the compiler
performs symbolic execution at compile time to verify round-trip fidelity:

```
Parse("FF00FF") → 0xFF00FF → Cast(#Bits) → "FF00FF"  ✓
```

If the round-trip fails, a warning with the exact input and output values
is reported.

## 7. Type Parameter Constraints

```brief
type HashMap<K: #String, V> {
    data: Bits<64>;
    len: Bits<64>;
    cap: Bits<64>;
    op Insert(V) = hashmap_insert(#L, #R1, #R2);
    op Get(K) -> V = hashmap_get(#L, #R);
};
```

`K: #String` checks that `K` implements the `#String` protocol ops
(`CastTo(#String)`, `CastFrom(#String)`, `Extract(#Char)`, `InsertAt(#Char)`,
`Concat(#String)`, `.^Len`). At instantiation, the concrete type is
verified against the protocol.

## 8. Protocol Participation and GLUE Bridge

Types that declare `op CastTo(#Category)` gain automatic FFI support through
the GLUE bridge. When a function with a custom type parameter is exported:

1. The protocol BFS (`find_cast_path`) discovers the `Cast.#Category` property
2. The protocol category is looked up in `lib/glue.toml`
3. The language-appropriate native type and C ABI type are selected
4. The `CastTo` function generates the conversion code at the boundary

```brief
// A custom type that speaks the #Int protocol
type MyFixedInt {
    data: Bits<32>;
    op CastTo(#Int);
    op CastFrom(#Int);
};

// Exported to Rust: the bridge automatically maps #Int → i64
export defn process(n: MyFixedInt) -> MyFixedInt {
    ...
};
```

No `lib/glue.toml` changes needed — the protocol system discovers
`CastTo(#Int)` from the type declaration. The TOML only needs to know
about protocol categories, not every custom type that implements them.

See `docs/architecture/protocol-types.md` for the full architecture doc.

