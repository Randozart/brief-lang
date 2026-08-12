# Int Width Narrowing & Type System Ranges
## 2026-07-24

## Goal

Make `Int` and `Bits` width-flexible: the compiler picks the narrowest safe
width based on value-range analysis, bounded only by explicit maximums or
exact constraints. On WASM this eliminates BigInt (i64 → i32). On all targets
it produces tighter code.

## Philosophy

`Int` is **not** a fixed-width type. It is a *range*: `x ∈ [min, max]` bounded
by `maxbits` (the declaration). The compiler narrows the physical storage to
whatever width safely covers the inferred range. The user only constrains when
they must (e.g., `frgn` with `int64_t`, or `as Int32`).

This follows the Bits-thesis: all types are `Bits(N)` with protocol overlays.
`Int` is `Bits` with signed arithmetic protocol. Width is an optimization
detail, not a semantic property.

## Architecture

### 1. `Bits<N>` Syntax

`Bits<64>` not `Bits(64)`. The parser follows the same `Ptr<T>` path in
`parse_named_type_body`: after seeing `Bits`, if `<` follows, parse a numeric
literal as the bit width. `Bits` with no annotation is flexible-width.

### 2. Primordial Type Universe

Each primordial gets `min_bits` and `max_bits` instead of `bytes`:

| Type | min_bits | max_bits | Semantics |
|------|----------|----------|-----------|
| `Int` | 0 | 64 | Up to 64 bits, compiler picks |
| `Int8` | 8 | 8 | Exactly 8 bits |
| `Int16` | 16 | 16 | Exactly 16 bits |
| `Int32` | 32 | 32 | Exactly 32 bits |
| `Int64` | 64 | 64 | Exactly 64 bits |
| `UInt` | 0 | 64 | Up to 64 bits, unsigned |
| `Float` | 32 | 32 | Exactly 32 bits |
| `Float64` | 64 | 64 | Exactly 64 bits |
| `Bool` | 0 | 8 | Up to 8 bits, compiler picks |
| `Ptr<T>` | 32/64 | 32/64 | Target pointer width |

### 3. Bootstrap Declaration

```briev
// New: maxbits is an upper bound, not an exact size
type Int : Bits {
    maxbits <~ 64;
    alignment <~ 8;
};

// Explicit bytes means exact width (backward compat and frgn bindings)
type Int32 : Bits {
    bytes <~ 4;        // exactly 32 bits
    alignment <~ 4;
};
```

`bytes <~ N` means exactly `N*8` bits. `maxbits <~ N` means at most N bits.
If neither is given, the compiler has full freedom.

### 4. ResolvedType

```rust
pub struct ResolvedType {
    pub min_bits: u64,      // lower bound (0 = unknown)
    pub max_bits: u64,      // upper bound (0xFF = unknown)
    pub alignment: u64,
    pub llvm_type: Option<String>,
    pub properties: HashMap<String, PropertyValue>,
}
```

`bytes` is computed as `max_bits.div_ceil(8)` for LLVM lowering.

### 5. Value-Range Inference Pass

File: `src/optimizer/narrow_int.rs`

Runs after the normalizer, before codegen. Walks each function body
independently (no inter-procedural analysis in v1).

#### Range type

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntRange {
    pub min: i128,
    pub max: i128,
}

impl IntRange {
    pub const UNKNOWN: IntRange = IntRange { min: i128::MIN, max: i128::MAX };
    pub fn exactly(v: i128) -> Self { IntRange { min: v, max: v } }
    pub fn width(&self) -> Option<u64> {
        // Smallest power-of-two bit width that fits this range
        // [0, 255]   → 8
        // [-128, 127] → 8
        // [0, 65535] → 16
        // i128::MIN..i128::MAX → None (unknown)
    }
    pub fn union(self, other: IntRange) -> IntRange { ... }
    pub fn intersect(self, other: IntRange) -> IntRange { ... }
}
```

#### Expression walker

Maps `Expr` → `IntRange`:

| Pattern | Range |
|---------|-------|
| `Literal(n)` | `[n, n]` |
| `BinOp(Add, a, b)` | `[a.min + b.min, a.max + b.max]` (saturating) |
| `BinOp(Sub, a, b)` | `[a.min - b.max, a.max - b.min]` |
| `BinOp(Mul, a, b)` | min/max of the four products (conservative) |
| `BinOp(Div, a, b)` | if b.min > 0 or b.max < 0: `[a.min / b.max, a.max / b.min]` |
| `BinOp(And, a, b)` | `[0, min(a.max, b.max)]` |
| `BinOp(Or, a, b)` | `[0, max(a.max, b.max).next_power_of_two() - 1]` |
| `BinOp(Shl, a, b)` | if b ∈ [0, 63]: `[a.min << b.min, a.max << b.max]` |
| `BinOp(Shr, a, b)` | if b ∈ [0, 63]: `[a.min >> b.max, a.max >> b.min]` |
| `Ident(x)` | from scope or `UNKNOWN` |
| `Call(fn, ...)` | frgn → `UNKNOWN`; intrinsic → known range |
| `Guarded(cond, body)` | within body: intersect with condition's implied range |
| `Term(expr)` | return value range = expr's range |
| All others | `UNKNOWN` |

#### Statement walker

Maintains a `HashMap<String, IntRange>` scope. For `let x = expr;`,
inserts `x → range(expr)`. For `Statement::Guarded(when x < 10, body)`,
evaluates body with `x` intersected with `[-∞, 9]`.

At each `term expr`, records the return range as `fn_return_range`.

#### Narrowing decision

After the walk, for each `let` binding and return type:

```rust
fn pick_max_bits(range: IntRange, declared_max: u64) -> u64 {
    if range == IntRange::UNKNOWN { return declared_max; }
    // Signed: need 1 sign bit
    let unsigned_max = range.max.max(range.min.unsigned_abs() as i128);
    let needed = if unsigned_max == 0 { 1 }
                 else { 64 - unsigned_max.leading_zeros() as u64 };
    // Include sign bit if min < 0
    let sign_bit = if range.min < 0 { 1 } else { 0 };
    let bits = needed + sign_bit;
    bits.min(declared_max).max(1)  // at least 1 bit, at most declared
}
```

The `ResolvedType.max_bits` is updated in the universe for each binding
whose range is known. Backends read `max_bits` to pick LLVM type width.

### 6. `match` in `$defn` bodies

New AST node:

```rust
pub enum Statement {
    // ... existing variants ...
    Match {
        expr: Box<Expr>,
        arms: Vec<MatchArm>,
    },
}

pub struct MatchArm {
    pub pattern: MatchPattern,
    pub body: Vec<Statement>,
}

pub enum MatchPattern {
    Literal(i128),        // match specific integer
    String(String),       // match specific string
    Wildcard,             // match anything (_)
}
```

Parser handles `match expr { pattern => body; pattern => body; };`
Eval handles by evaluating `expr`, trying each arm's pattern, executing
the first match.

### 7. Parser: `Bits<N>` with angle brackets

In `parse_named_type_body`, add a `Bits` special case:

```rust
if name == "Bits" || name == "bits" {
    if self.eat(&Token::Lt) {
        let bits = self.expect_integer()?;
        self.expect(Token::Gt)?;
        return Ok(Type::Bits(bits));  // bits, not bytes
    }
    return Ok(Type::Bits(0));  // flexible Bits
}
```

`expect_integer()` parses a numeric literal token. `Type::Bits(u64)` now
stores bits, not bytes (unit change throughout the compiler).

### 8. `compile.rs` Integration

```rust
// After normalizer, before Normalized stage:
#[cfg(feature = "optimizer")]
briev_compiler::optimizer::narrow_int::narrow_types(&mut items, &mut universe)?;
```

The pass is optional (behind `optimizer` feature) so `--optimize-budget 0`
disables it. Default: on.

## Implementation Order

### Step 1: Parser — `match` statement in `$defn` bodies

- `src/ast/stmt.rs`: Add `Statement::Match` variant with `MatchArm`/`MatchPattern`
- `src/lexer.rs`: No new tokens needed (uses existing `match` keyword)
- `src/parser/statements.rs`: Parse `match expr { ... }`
- `src/macros/eval.rs`: Evaluate `match`. For each arm, evaluate the
  expression and compare against the pattern. String patterns use string
  equality, integer patterns use integer equality, wildcard always matches.

### Step 2: Parser — `Bits<N>` angle brackets

- `src/ast/types.rs`: Update docs, `Bits(u64)` stores bits
- `src/parser/types.rs`: Add `Bits` case to `parse_named_type_body`
- `src/backend/llvm/types.rs`: `Bits(n)` → `format!("i{}", n)`

### Step 3: Type universe — min_bits/max_bits

- `src/type_universe/mod.rs`: `seed_primordial_types` uses min/max
- `src/type_universe/resolve.rs`: Resolve using range semantics
- `lib/std/types/bootstrap.bv`: `maxbits <~ 64` instead of `bytes <~ 8`

### Step 4: Value-range inference pass

- `src/optimizer/narrow_int.rs`: Full implementation as described above
- `src/optimizer/mod.rs`: New module
- `src/lib.rs`: Add `pub mod optimizer`
- `src/compile.rs`: Call after normalizer

### Step 5: Update generators to use `match`

- `lib/ffi/gen_python.bv`: Replace `when` chains with `match`
- `lib/ffi/gen_node.bv`: Replace `when` chains with `match`
- `lib/ffi/gen_wasm.bv`: Replace `when` chains with `match`
- `lib/ffi/gen_c.bv`: Replace `when` chains with `match`
- All other gen_*.bv files similarly updated

### Step 6: Benchmark

- `bash benchmarks/metropolitan/run` before and after
- Expected: WASM bridge drops from 120ns toward ~70ns for small-int functions
- Python/Node.js/C unchanged (they already use native widths)

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Narrowing changes C ABI for `export defn` | Exports always match declared type; narrowing only applies to internal bindings |
| Range analysis too complex for v1 | Start with literals + simple arithmetic; add `&` masks, `when` guards later |
| Backward compat: existing `.bv` files use `bytes <~ 8` | Keep `bytes` as an alias for `maxbits = bytes * 8` during transition |
| Narrowing introduces overflow | Narrowing only affects storage width, not semantics. Arithmetic is still i64; LLVM handles overflow |
