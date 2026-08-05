# Projection Expansion, Pointer Safety, and Throughput-Matched Optimization

**Date**: 2026-06-05
**Status**: Plan (ready for implementation)

---

## Phase 1: New `:>` Projection Targets + Fix LLVM Stubs

### New Projection Targets (8 total)

| Target | Source type | Return type | LLVM intrinsic | Rust std method (interpreter) |
|---|---|---|---|---|
| `Popcount` | Int | Int | `@llvm.ctpop.i64` | `i64::count_ones()` |
| `LeadingZeros` | Int | Int | `@llvm.ctlz.i64(i64, i1 false)` | `i64::leading_zeros()` |
| `TrailingZeros` | Int | Int | `@llvm.cttz.i64(i64, i1 false)` | `i64::trailing_zeros()` |
| `Absolute` | Int/Float | Int | `@llvm.abs.i64` / `@llvm.fabs.f64` | `i64::abs()` / `f64::abs()` |
| `BitReverse` | Int | Int | `@llvm.bitreverse.i64` | `i64::reverse_bits()` |
| `Type` | Any | Type | `add i64 0, <tag>` | returns type discriminant |
| `Ptr!` | Any | Int (raw) | `ptrtoint` / `load` | returns `Value::Int(0)` (stub, like Ptr) |
| `Match(str)` | String | Bool/String/Tuple | DFA scan loop | DFA interpreter |

### Fixed LLVM Stubs

- **`Bytes`**: Look up source type in `register_types` → return correct byte size
- **`Alignment`**: Look up source type → return correct alignment
- **`Range`**: Return 2-slot (min, max) tuple instead of `i64::MIN` scalar

### Files Changed (Phase 1)

| File | Change |
|---|---|
| `src/ast.rs:317-323` | Add 7 new `ProjectionTarget` variants + `Match(String)` |
| `src/parser.rs:4528-4543` | Match arms: `Popcount`, `LeadingZeros`, `TrailingZeros`, `Absolute`, `BitReverse`, `Type`, `Ptr!` (peek `!`), `Match(str)` (peek `(` + string literal) |
| `src/typechecker.rs:1337-1358` | New arms + type validation per source type |
| `src/interpreter.rs:1914-1953` | Evaluate each target to concrete value |
| `src/backend/llvm.rs:1814-1832` | Add `declare` for all LLVM intrinsics |
| `src/backend/llvm.rs:2959-2988` | Emit intrinsic calls + fix stubs |
| `src/backend/mod.rs:253` | Fallthrough arms |
| `src/backend/rust.rs:583` | `.count_ones()`, `.leading_zeros()`, etc. |
| `src/backend/c.rs` | `__builtin_popcountll`, `__builtin_clzll`, etc. |
| 7 other backends | Stub arms |
| `src/proof_engine.rs:96,120,338` | Add `Projection` to `SymbolicValue` |
| 6 analysis files | Fallthrough |

---

## Phase 2: `std/bits.bv` — Bit Manipulation Library

New file: `lib/std/bits.bv`

Pure `defn` wrappers over `:>` projections. No FFI, no TOML, no DBriv.

```briv
defn popcount(x: Int) [true][term >= 0 && term <= 64] -> Int { term x :> Popcount; };
defn leading_zeros(x: Int) [true][term >= 0 && term <= 64] -> Int { term x :> LeadingZeros; };
defn trailing_zeros(x: Int) [true][term >= 0 && term <= 64] -> Int { term x :> TrailingZeros; };
defn abs(x: Int) [true][term >= 0] -> Int { term x :> Absolute; };
defn abs_float(x: Float) [true][term >= 0.0] -> Float { term x :> Absolute; };
defn bit_reverse(x: Int) [true][true] -> Int { term x :> BitReverse; };
defn ffs(x: Int) [true][term >= 0 && term <= 64] -> Int {
    [x == 0] { term 0; };
    term (x :> TrailingZeros) + 1;
};
defn is_power_of_two(x: Int) [true][term == true || term == false] -> Bool {
    term x != 0 && x & (x - 1) == 0;
};
defn rotate_left(x: Int, n: Int) [true][true] -> Int { term (x << n) | (x >> (64 - n)); };
defn rotate_right(x: Int, n: Int) [true][true] -> Int { term (x >> n) | (x << (64 - n)); };
```

---

## Phase 3: Pointer Safety System

### 3a: Type System — `Ptr<T>` and `Ptr!`

| Projection | Parses as | Return type | Provenance |
|---|---|---|---|
| `&x :> Ptr` | `Projection(OwnedRef("x"), Ptr)` | `Ptr<T>` where T = typeof(x) | Bound = sizeof(x), non-null = true |
| `list :> Ptr` | `Projection(Identifier("list"), Ptr)` | `Ptr<T>` where T = element type | Bound = list :> Bytes, non-null = true |
| `x :> Ptr!` | `Projection(Identifier("x"), PtrBang)` | `Int` (raw) | None |
| `ptr :> Ptr` (on Ptr\<T\>) | `Projection(ptr, Ptr)` | `Int` (raw address) | Escape hatch from verified lane |

Add `Ptr<T>` as `Type::Ptr(Box<Type>)` in the type system. Existing `Applied` generics already handle `List<T>`, `Option<T>` — `Ptr<T>` follows the same pattern.

### 3b: `ptr[i]` as Dereference

When `ListIndex(ptr, i)` has `ptr: Ptr<T>`:
- **Typechecker**: Returns `T`. Adds implicit proof obligation: `i * sizeof(T) < ptr :> Bytes`
- **LLVM backend**: Emits `getelementptr + load` (not list-header GEP). For `&ptr[i] = val`: `getelementptr + store`
- **Parser**: No changes — `ptr[i]` already parses as `ListIndex`. Dispatch on type.

### 3c: `PointerVerifier` Analysis Pass

New file: `src/analysis/pointer_verifier.rs`

Runs after typechecking, before codegen.

```
struct Provenance {
    allocation: String,    // name of origin variable
    bound: Expr,           // ptr :> Bytes
    is_non_null: bool,     // true if from &x or list
    alignment: i64,        // from x :> Alignment
}

For each ptr[i] in the program:
  - Look up ptr's Provenance
  - Prove: i * sizeof(T) < bound
    - Literal i → concrete evaluation
    - Bounded counter → range.rs max_value
    - Symbolic expression → proof_engine symbolic evaluation
  - Success → mark safe, emit raw load/store
  - Failure → ProofError(P200, "out of bounds access")
```

**ProofError codes**: P200 (bounds), P201 (alignment), P202 (null), P203 (temporal escape)

### 3d: `std/ptr.bv` — Provably Safe Pointer Operations

New file: `lib/std/ptr.bv`

```briv
defn read<T>(p: Ptr<T>, i: Int)
  [i >= 0 && (i + 1) * (T :> Bytes) <= p :> Bytes]
  [true] -> T
{ term p[i]; };

defn write<T>(p: Ptr<T>, i: Int, val: T)
  [i >= 0 && (i + 1) * (T :> Bytes) <= p :> Bytes]
  [true]
{ &p[i] = val; };

defn copy<T>(dest: Ptr<T>, src: Ptr<T>, count: Int)
  [count >= 0 &&
   count * (T :> Bytes) <= dest :> Bytes &&
   count * (T :> Bytes) <= src :> Bytes &&
   (dest :> Ptr >= src :> Ptr + count * (T :> Bytes) ||
    src :> Ptr >= dest :> Ptr + count * (T :> Bytes))]
  [true]
{
  // Proven non-overlapping → compiler emits @llvm.memcpy
  // Unprovable overlap → element-by-element loop (correct, scalar)
};

defn address<T>(p: Ptr<T>) [true][true] -> Int { term p :> Ptr; };
```

---

## Phase 4: Throughput-Matched Optimization

### 4a: `bottlenecks.dbvs` Schema

New file in target config path:

```dbvs
schema BottleneckConfig {
    pcie_bandwidth_gbs: Float;
    system_ram_bandwidth_gbs: Float;
    l1_cache_size_kb: Int;
    l2_cache_size_kb: Int;
    l3_cache_size_kb: Int;
    memory_port_width: Int;
    fpga_clock_mhz: Float;
};
```

Extended `CodegenSection` with `bottlenecks: Option<String>` pointing to `.dbvs` path.
Parsed via existing `src/dbriv/parser.rs`.

### 4b: `RooflineAnalyzer` Pass

New file: `src/analysis/roofline.rs`

- `compute_roofline(flops, bytes_moved)` → compute-bound vs memory-bound
- `lut_fits_cache(lut_size_bytes)` → `Some(CacheTier::L1|L2|L3)` or `None`
- `should_precompute_as_lut(iterations, lut_size, reuse_factor)` → guides fold decisions

Wired into `LlvmBackend` via `self.roofline`. `emit_folded_loop` consults the analyzer before choosing precomputation over runtime loop.

---

## Phase 5: DFA Regex (`:> Match`)

### New `ProjectionTarget::Match(RegexPattern)`

`RegexPattern` carries the compile-time-compiled DFA transition table.

### Parser

After matching `"Match"` in `parse_projection_target`, peek for `(`, parse string literal, compile regex to DFA at parse time. Error on invalid regex.

### New Module: `src/analysis/dfa.rs`

Thompson construction → subset construction → DFA minimization → transition table.

### Typechecker

- `0` capture groups → `Bool` (match/no-match)
- `1` capture group → `String`
- `N` capture groups → `Tuple([String; N])`

### LLVM Backend

Emit DFA table as `@dfa_table = private constant [N x [256 x i32]]`. Generate O(n) linear state-machine loop. No backtracking, no ReDoS.

---

## Dependency Graph

```
Phase 1 (projection targets + Type + Ptr!)
  ├── Phase 2 (bits.bv) — depends only on Phase 1
  ├── Phase 3a (Ptr<T>) — depends on Phase 1
  │   └── Phase 3b (ptr[i] deref) — depends on 3a
  │       ├── Phase 3c (PointerVerifier) — depends on 3b
  │       └── Phase 3d (std/ptr.bv) — depends on 3b
  ├── Phase 4 (roofline) — independent
  └── Phase 5 (regex) — independent (but uses Match target from Phase 1)
```