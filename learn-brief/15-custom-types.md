# Defining Custom Types

Brief lets you define types that behave like built-in `Int`, `Float`, and
`String`. The same hashword protocol system that drives primitives also
drives your custom types — no special compiler support needed.

## 1. `type MyType { ... }`

Use the `type` keyword to define a new type. Layout comes from fields;
operations come from `op` declarations:

```brief
type MyInt {
    data: Bits<64>;          // layout: 8 bytes, llvm_type: i64
    op Add(#Int, #Int);      // backend knows integer addition
    op Sub(#Int, #Int);      // backend knows integer subtraction
    op Parse(#Int);           // identity literal construction — 42 → MyInt
    op Parse(Decimal);        // numeric literal via conversion function
};
```

- **Layout**: determined by fields (`data: Bits<N>`, `field: Type`)
- **Hashword ops**: `Add(#Int)` — backend dispatches to its intrinsic handlers
- **Parse ops**: `Parse(#Int)` — identity literal construction (zero-cost)
- **Parse ops**: `Parse(Decimal) = fn(#L)` — custom literal construction

No `bytes <~`, `alignment <~`, `llvm <~`, `storage <~`, `default_width`,
`commuting`, or LLVM opcode strings needed.

## 2. Protocol-Centric Ops

The hashword in an op signature tells the backend to dispatch to its
intrinsic handler for that category:

```brief
type Bfloat16 {
    data: Bits<16>;
    op Add(#Float, #Float) = bfloat_add(#L, #R);   // override with custom fn
    op Mul(#Float, #Float) = bfloat_mul(#L, #R);
    op Parse(#Float);                                // identity — literal IS float
};
```

| Op form | Meaning | Conversion function? |
|---|---|---|
| `op Add(#Int, #Int)` | Backend intrinsic integer add | No |
| `op Add(#Float) = fn(#L,#R)` | Override float add with custom fn | Yes — auto-alwaysinline |
| `op Add(Posit32) = fn(#L,#R)` | Custom op for this type only | Yes — auto-alwaysinline |
| `op Parse(#Category)` | Identity literal construction | No |
| `op Parse(Form) = fn(#L)` | Custom literal construction | Yes — auto-alwaysinline |

Functions bound to ops via `= fn(...)` are emitted with LLVM's `alwaysinline`.

## 3. Protocol Variants

Hashwords can be parameterized by protocol variant:

```brief
type ASCIIString {
    data: Bits<64>;
    len: Bits<64>;
    op CastTo(#String<utf8>) = ascii_to_utf8(#L);   // produce UTF-8
    op CastFrom(#String<utf8>) = utf8_to_ascii(#L);  // consume UTF-8
};
```

Bare hashwords resolve to their default variant at parse time:

| Hashword | Default variant | Also writable as |
|---|---|---|
| `#String` | `utf8` (for all files) | `#String<utf8>`, `#String<ascii>` |
| `#Float` | `ieee754` | `#Float<ieee754>` |
| `#Char` | `unicode` | `#Char<unicode>`, `#Char<ascii>` |

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
    op CastTo(#String) = latin1_to_utf8(#L);       // Latin1 → UTF-8
    op CastFrom(#String) = utf8_to_latin1(#L);      // UTF-8 → Latin1
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
`Concat(#String)`, `:> Size`). At instantiation, the concrete type is
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

