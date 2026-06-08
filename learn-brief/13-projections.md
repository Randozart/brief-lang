# Projections: `:>` and `<:` Operators

**Brief has two projection operators: `:>` for compile-time metadata extraction,
and `<:` for compile-time-optimized queries and subtype projections.**

---

## `:>` — Metadata Lens

The `:>` (metadata lens) operator reads compile-time-known properties from
values without runtime overhead. All targets map to either a constant or a
zero-cost LLVM intrinsic.

### Syntax

```bnf
projection ::= expression ":>" projection_target

projection_target ::= "Size" | "Bytes" | "Ptr" | "Alignment" | "Range"
                    | "Popcount" | "LeadingZeros" | "TrailingZeros"
                    | "Absolute" | "BitReverse" | "Type" | "Ptr!"
                    | "Keys" | "Values" | "Contains" "(" expression ")"
                    | "Pop" | "Index" "(" integer ")" | "Get" "(" expression ")"
                    | "Top" | "Front" | "Elements"
                    | "AsStack" | "AsQueue"
```

---

## Metadata Projections

### `:> Size`

Returns the number of elements in a collection (List, String, HashMap, etc.).

```brief
let items: List<Int> = [10, 20, 30];
let n = items :> Size;             // 3

let s = "hello";
let len = s :> Size;              // 5
```

**LLVM:** Load length from 2-slot header slot 1.

### `:> Bytes`

Returns the byte size of a value at compile time.

```brief
let x: Int = 42;
let sz = x :> Bytes;               // 8

let s: String = "hello";
let byte_len = s :> Bytes;        // 5
```

**LLVM:** Compile-time constant.

### `:> Ptr`

Creates a `Ptr<T>` — a verified pointer with compile-time bounds tracking.
The compiler guarantees non-null and bounds-safety.

```brief
let p: Ptr<Int> = &my_var :> Ptr;    // Pointer to a state variable
let lp: Ptr<Int> = my_list :> Ptr;   // Pointer to list data
let raw: Int = p :> Ptr;             // Escape hatch: raw Int address
```

See `05-data-types.md §7` for the full `Ptr<T>` reference.

### `:> Alignment`

Returns the memory alignment of a value.

```brief
let x: Int = 42;
let align = x :> Alignment;        // 8 (Int alignment)
```

**LLVM:** Compile-time constant.

### `:> Range`

Returns `(min, max)` bounds of a value. Useful in contracts and assertions.

```brief
// In a contract:
[i >= 0 && i < buffer :> Range]
```

**LLVM:** Compile-time constant pair.

---

## Bit Manipulation Projections

These map directly to LLVM bit manipulation intrinsics. Each compiles to a
single CPU instruction.

### `:> Popcount`

Number of set bits (1-bits) in an integer. Also known as "population count."

```brief
let v = 0x0F0F0F0F0F0F0F0F;
let ones = v :> Popcount;          // 32
```

**Postcondition:** `[term >= 0 && term <= 64]`
**LLVM intrinsic:** `@llvm.ctpop.i64`

### `:> LeadingZeros`

Number of leading zero bits (from the most significant bit).

```brief
let v = 0x0F0F0F0F0F0F0F0F;
let lz = v :> LeadingZeros;        // 4
```

**Postcondition:** `[term >= 0 && term <= 64]`
**LLVM intrinsic:** `@llvm.ctlz.i64(i64, i1 false)`

### `:> TrailingZeros`

Number of trailing zero bits (from the least significant bit).

```brief
let v = 0x0F0F0F0F0F0F0F0F;
let tz = v :> TrailingZeros;       // 4
```

**Postcondition:** `[term >= 0 && term <= 64]`
**LLVM intrinsic:** `@llvm.cttz.i64(i64, i1 false)`

### `:> Absolute`

Absolute value of an integer or float.

```brief
let a = (-42) :> Absolute;         // 42
let b = (-3.14) :> Absolute;       // 3.14
```

**Postcondition:** `[term >= 0]`
**LLVM intrinsic:** `@llvm.abs.i64(i64, i1 false)` or `@llvm.fabs.f64(double)`

### `:> BitReverse`

Reverses all 64 bits of an integer.

```brief
let v = 0x0F0F0F0F0F0F0F0F;
let rev = v :> BitReverse;
```

**LLVM intrinsic:** `@llvm.bitreverse.i64`

---

## Reflection Projections

### `:> Type`

Returns a compile-time discriminant identifying the type of the value.

```brief
let x: Int = 42;
let tag = x :> Type;               // 1 (Int)

let f: Float = 3.14;
let ftag = f :> Type;              // 2 (Float)
```

**LLVM:** Compile-time constant.

### `:> Ptr!`

Returns the raw memory address as an `Int`. **No safety envelope** — you are
responsible for all bounds, null, and alignment checks.

```brief
let raw_addr = my_var :> Ptr!;     // Raw Int address

// You can do anything with raw addresses:
let shifted = raw_addr + 8;        // Arithmetic
let masked = raw_addr & 0xFF;      // Bit ops
```

Use `:> Ptr!` when you need absolute control. Use `:> Ptr` when you want the
compiler to verify safety.

---

## `<:` — Subtype Projection

The `<:` operator performs a **compile-time optimized projection** from a source
value into a derived value. Two forms exist depending on the source type.

### Collection Projection

For `List<T>`, `HashMap<K,V>`, or other collections, `<:` applies a sequence of
relational operations in a single fused pass with zero intermediate allocations:

```brief
let regional_stats <: transactions {
    FILTER(.is_active);
    GROUP(.region);
    COUNT;
};
```

**Allowed operations:**

| Op | Semantics |
|----|-----------|
| `FILTER(.expr)` | Keep elements matching predicate |
| `MAP(.expr)` | Transform each element |
| `SORT(.expr)` | Sort by key expression |
| `LIMIT(N)` | Take first N elements |
| `SKIP(N)` | Skip first N elements |
| `UNIQUE` | Remove adjacent duplicates |
| `JOIN(other, .key)` | Merge with another collection |
| `GROUP(.key)` | Group by key (must be followed by aggregate) |
| `COUNT` | Count elements (terminal) |
| `SUM(.field)` | Sum of field (terminal) |
| `AVG(.field)` | Average of field (terminal) |
| `MIN(.field)` | Minimum of field (terminal) |
| `MAX(.field)` | Maximum of field (terminal) |

The last operation determines the return type. Aggregates are terminal — they
collapse the collection to a scalar. Non-aggregates return `List<T>`. GROUP
must be followed by an aggregate.

### String Projection

For `String` sources, `<:` compiles a regular expression into a DFA at compile
time and captures groups in a single O(n) scan:

```brief
// Multiple capture groups return a Tuple
let (user, domain) <: email["^([a-z]+)@(.+)$"];

// Patterns can be a const variable:
const pat = "^([a-z]+)@(.+)$";
let (user, domain) <: email[pat];
```

**Return types:** 0 groups → Bool, 1 group → String, N groups → Tuple.

The DFA is built via Thompson construction → subset construction at parse time.
Invalid patterns produce a compile-time error. The transition table is embedded
as a constant; the scan loop is O(n) linear with zero dynamic allocations.

---

## Standard Library Wrappers

The standard library provides thin `defn` wrappers for all bit manipulation
projections in `std/bits.bv`, and safe pointer operations in `std/ptr.bv`:

```brief
import { popcount, leading_zeros, abs } from "std/bits.bv";
import { read_i64, write_i64 } from "std/ptr.bv";

// These compile to the same intrinsics as the raw projections:
let ones = popcount(0x0F0F0F0F0F0F0F0F);
let v = read_i64(p, 0);            // Bounds-checked by contract
```

---

## Complete Reference

| Target | Input type | Output type | LLVM | stdlib |
|--------|------------|-------------|------|--------|
| `Size` | List, String, Map | Int | Header load | `std/collections` |
| `Bytes` | Any | Int | Constant | — |
| `Ptr` | State, List | `Ptr<T>` | Header load | `std/ptr` |
| `Alignment` | Any | Int | Constant | — |
| `Range` | Any | Int | Constant | — |
| `Popcount` | Int | Int | `@llvm.ctpop` | `std/bits` |
| `LeadingZeros` | Int | Int | `@llvm.ctlz` | `std/bits` |
| `TrailingZeros` | Int | Int | `@llvm.cttz` | `std/bits` |
| `Absolute` | Int, Float | Int/Float | `@llvm.abs` / `@llvm.fabs` | `std/bits` |
| `BitReverse` | Int | Int | `@llvm.bitreverse` | `std/bits` |
| `Type` | Any | Int | Constant | — |
| `Ptr!` | Any | Int | Header load | — |
| `Keys` | HashMap | `List<K>` | Iteration | — |
| `Values` | HashMap | `List<V>` | Iteration | — |
| `Contains(val)` | Map, Set | Bool | Lookup | — |
| `Pop` | HashSet | `T` | Remove+return | — |
| `Index(n)` | Tuple | `T` | Element access | — |
| `Get(key)` | HashMap | `Option<V>` | Lookup | — |
| `Top` | Stack | `Option<V>` | Peek | — |
| `Front` | Queue | `Option<V>` | Peek | — |
| `Elements` | HashSet | `List<T>` | Enumeration | — |
| `AsStack` | List | Stack | Conversion | — |
| `AsQueue` | List | Queue | Conversion | — |

**`<:` projection ops** (not `:>` targets):

| Op | Source type | Output type | Notes |
|----|------------|-------------|-------|
| `FILTER(.p)`, `MAP(.x)`, `SORT(.k)` | Collection | `List<T>` | Fused pass, no intermediate alloc |
| `COUNT`, `SUM(.f)`, `AVG(.f)` | Collection | Int / Float | Terminal aggregate |
| `MIN(.f)`, `MAX(.f)` | Collection | `typeof(f)` | Terminal aggregate |
| `GROUP(.k)` + aggregate | Collection | `Map<K, V>` | Group-then-aggregate |
| `["pattern"]` | String | Bool / String / Tuple | DFA scan loop, O(n) |