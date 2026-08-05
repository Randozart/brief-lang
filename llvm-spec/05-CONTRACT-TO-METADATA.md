# Contract-Driven Optimization: !range + llvm.assume

Three forms of contract information are injected into LLVM IR, each enabling different optimizations.

## Form 1: `!range` Metadata on Loads

Preconditions that bound a variable to a numeric range become `!range` metadata on the corresponding `load` instruction.

| Precondition | LLVM !range | Optimization Enabled |
|-------------|-------------|---------------------|
| `[x < 100]` | `!{ i64 0, i64 100 }` | Dead branch elimination on `x >= 100` |
| `[x >= 0]` | `!{ i64 0, i64 9223372036854775808 }` | Sign-extend removal, unsigned comparison simplification |
| `[x in 5..10]` | `!{ i64 5, i64 10 }` | Array bounds check elimination |
| `[len > 0]` | `!{ i64 1, i64 9223372036854775808 }` | Loop trip count known to be ≥ 1 |

**Implementation:**

```rust
// In the LLVM backend, when a guard [x < N] is encountered:
let range_meta = format!("!range !{{ i64 {}, i64 {} }}", lower_bound, upper_bound);
writeln!(output, "{} = load i64, i64* %ptr, align 8, {}", reg, range_meta);
```

This works because `!range` on a load tells LLVM's `ScalarEvolution` pass the loaded value is in `[lower, upper)`, enabling LICM, IV elimination, and branch folding.

## Form 2: `@llvm.assume` for Complex Invariants

When a precondition involves multiple variables or relationships that can't be expressed as simple range metadata:

```briv
// Complex precondition
txn transfer [from > 0 && to < 100 && from + amount == to] { ... }
```

### Debug vs. Release Mode

`@llvm.assume` with a false precondition triggers **immediate Undefined Behavior** (LLVM is legally allowed to emit corrupt machine code if the assumption is violated). To prevent this during development:

**Debug mode** — emit a runtime panic check:
```llvm
%from = load i64, i64* %from_ptr
%to = load i64, i64* %to_ptr
%c1 = icmp sgt i64 %from, 0
%c2 = icmp slt i64 %to, 100
%c3 = icmp eq i64 %from, %to
%cond = and i1 %c1, %c2
%cond2 = and i1 %cond, %c3
br i1 %cond2, label %safe, label %panic

panic:
    call void @__panic(i8* "precondition failed: from > 0 && to < 100 && from + amount == to")
    unreachable

safe:
    ; Transaction body continues here
```

**Release mode** — emit `@llvm.assume` for optimization:
```llvm
%from = load i64, i64* %from_ptr
%to = load i64, i64* %to_ptr
%c1 = icmp sgt i64 %from, 0
%c2 = icmp slt i64 %to, 100
%c3 = icmp eq i64 %from, %to
%cond = and i1 %c1, %c2
%cond2 = and i1 %cond, %c3
call void @llvm.assume(i1 %cond2)
```

**Compiler switch:** Controlled by `--release` flag. Default is debug mode (runtime checks). Only emit `@llvm.assume` when the proof engine has achieved Z3-verified 100% certainty.

**What `@llvm.assume` enables:**

1. **Loop vectorization**: Assumptions about trip counts enable safe SIMD
2. **Dead code elimination**: Guards that are proven true → body always executes → no branch
3. **Algebraic simplification**: `from + 1` where `from < 100` → `nuw nsw add` (no overflow check needed)
4. **Speculative execution safety**: LLVM can speculate past the assumption because it's guaranteed true

## Form 3: Constant Propagation from Contracts

When a guard is `[x == 5]`, the backend can constant-propagate:

```briv
[x == 5] {
    let y = x + 10;  // y is always 15
    &result = y * 2;  // result is always 30
};
```

```llvm
; After LLVM's constant propagation from @llvm.assume(x == 5):
%y = add nsw i64 5, 10   ; → constprop to 15
%result = mul nsw i64 15, 2  ; → constprop to 30
store i64 30, i64* %result_ptr
```

## Postcondition Proof → `@llvm.assume(i1 true)`

When the proof engine verifies a postcondition at compile time, the backend emits:

```llvm
call void @llvm.assume(i1 true)
```

This is a no-op for LLVM's optimizer, BUT it signals that no runtime postcondition check is needed. The transaction body can be optimized as if the postcondition has already been proven.

## Guard Select Conversion

Instead of branching on guards, the backend emits `select i1`:

```briv
let x: Int = 0;
[cond] {
    &x = 42;
};
// Use x here
```

```llvm
; Instead of:
;   br i1 %cond, label %then, label %end
;   then: store i64 42, i64* %x_ptr; br label %end
;   end: load i64, i64* %x_ptr

; Emit:
%x_val = select i1 %cond, i64 42, i64 0
; x_val is always in a register — no branch, no load, no mispredict
```

**When to use `select` vs `br`:**
- **Guard → `select`**: When the guarded block has a single assignment. No branch penalty.
- **Guard → `br`**: When the guarded block has multiple statements or side effects. LLVM may convert to `select` later if profitable.
- **MMIO guard → `br`**: Hardware writes must be `volatile` and must actually execute. Never use `select` for MMIO.