# Narrowing by Proof — Width Inference Architecture
## 2026-07-25

### Overview

Every `Int`-typed value in Briv is a *range*, not a fixed-width word.
The compiler proves the maximum bits each code path needs and emits the
corresponding LLVM integer type — i8, i16, i32, or i64. On WASM, anything
≤32 bits maps to i32, eliminating BigInt entirely.

### Pipeline

```
Parser → Stage blocks → Type checker → NARROWING PASS → Normalizer → Codegen
                                             │
                                             └── populates self.fun.narrowed
                                                       per function, per binding
                                                       │
                                              Codegen reads at emit time:
                                              llvm_type() checks narrowed first
```

### The Narrowing Pass

Located at `src/optimizer/narrow_int.rs`. Walks each function body after
typechecking, tracking value ranges as `IntRange { min: i128, max: i128 }`.

**Range sources:**

| Source | Example | Result |
|--------|---------|--------|
| Literal | `term 42` | `[42, 42]` → 7 bits |
| Arithmetic | `a + b` | `[min(a)+min(b), max(a)+max(b)]` |
| Bitwise AND | `x & 0xFF` | `[0, 255]` → 8 bits |
| `when` guard | `when x < 1000 { ... }` | within arm: `x ∈ [min(x), 999]` |
| Contract precondition | `[a < 1000 && b < 1000]` | params constrained to `[0, 999]` |
| Unknown (param, frgn call) | `a: Int` | `[i128::MIN, i128::MAX]` — no narrowing |

**Propagation:**

```rust
Add:  [a.min + b.min, a.max + b.max]    (saturating)
Sub:  [a.min - b.max, a.max - b.min]
Mul:  [min(4 products), max(4 products)]
And:  [0, min(a.max, b.max)]
Or:   [0, next_power_of_two(a.max | b.max) - 1]
Shl:  wrapping_shl with min/max shift amounts
```

**Output:** `HashMap<fn_name, HashMap<binding, max_bits>>`
- `"ret"` → max bits needed for the return value
- `"param_0"`, `"param_1"` → max bits per parameter
- `"let_x"` → max bits for a let-binding
- `"assign_y"` → max bits for an assignment

### Code Generation

At emit time, `llvm_type(&self, ty: &Type)` in `emit_toplevel.rs` checks
the narrowed map before falling through to the universe lookup:

```rust
// For Int/UInt — check narrowed max_bits first:
if let Some(&bits) = self.fun.narrowed.get("ret") {
    // Map to LLVM's native integer widths:
    if bits <= 8  => return "i8";
    if bits <= 16 => return "i16";
    if bits <= 32 => return "i32";
    // >32 bits → fall through to universe → "i64"
}
```

This affects every code path that calls `llvm_type()`:
- **Function signatures:** `define i16 @add(i16 %arg0, i16 %arg1)`
- **Binary ops:** `add nsw i16 %arg0, %arg1` (via `binop_int_type()`)
- **Config templates:** `template_for_op` uses passed `llvm_ty` not `bytes*8`
- **Returns:** `ret i16 %t0` (no trunc needed — types are consistent)

### Contract Precondition Integration

Contract preconditions like `[a < 1000 && b < 1000]` are parsed by
`apply_contract_ranges()` in the narrowing pass. It handles:

- `Expr::BinaryOp(Lt, Expr::Identifier(name), Expr::Decimal(n))` →
  `name` is constrained to `[min_of_name, n-1]`
- `Expr::BinaryOp(And, lhs, rhs)` → recurse into both sides
- `Expr::Bool(true)` → no constraint (default precondition)

### Width Mapping to LLVM

| Proven max bits | LLVM type | WASM promotes to | BigInt? |
|----------------|-----------|-------------------|---------|
| ≤ 8 | `i8` | i32 | No |
| ≤ 16 | `i16` | i32 | No |
| ≤ 32 | `i32` | i32 | No |
| > 32 | `i64` | i64 | Yes |

WASM's `llc` automatically promotes i8/i16 to i32 in the WASM binary.
JavaScript receives and returns plain Numbers — no BigInt conversion.

### Performance

| Test | Before (i64/BigInt) | After (narrowed i16) | Native JS |
|------|--------------------|----------------------|-----------|
| `add(3,4)` via WASM | 120ns | **63ns** | 66ns |

The narrowed WASM bridge matches native JavaScript performance because
the function uses i32 (from i16 promotion), which is WASM's native word
type. The eliminated BigInt conversion accounts for ~57ns per call.

### Future Work

- **Float narrowing:** Float uses `minbits` for accuracy constraints.
  `Float` at `minbits <~ 32` means "at least 32 bits for precision."
  The narrowing pass does NOT narrow Float below its declared `minbits`.
- **Inter-procedural:** When function A calls function B with proven-small
  values, the caller-proven ranges could propagate to B's parameters.
- **Dynamic contracts:** Runtime `when`-guard conditions propagate through
  the narrowing pass, but only statically provable bounds are used.
