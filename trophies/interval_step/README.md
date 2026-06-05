# interval_step — Brief 0.98× (parity)

## What it tests

Interval bounds detection: `x = (x + R1) - R2` where `R1 - R2 = 1`. The
`detect_increments` interval arm computes the net step (200 - 199 = 1)
and uses it as the counter delta. 50M iterations with cumulative sum
and conditional `__print_int` every 5M ticks.

## Result

| | Time | Ratio |
|---|------|-------|
| Brief | 0.0583s | — |
| C | 0.0591s | 0.98× |

## Why parity is a win

The interval formula `(count + 200) - 199` is NOT a simple `count++`.
Brief's compiler must:
1. Parse `(count + R1) - R2` into the AST
2. Run `simplify_body` (no change — this isn't an algebraic cancellation)
3. Run `detect_increments` — the interval arm recognizes `Sub(Add(x, R1), R2)`
   and computes `net = R1 - R2`
4. Use `net = 1` as the counter delta for the unified folded loop

C's compiler sees `count = (count + 200) - 199` and immediately optimizes
it to `count++` — but the SSA loop is the same either way. The parity
proves that Brief's multi-pass analysis (simplify → increment detection →
bounded pre extraction) produces code equivalent to C's direct
optimization.

**LTO inlines `__print_int`.** Same mechanism as the other trophies.
Both Brief and C call `fprintf` at the same intervals, but Brief's
LTO pipeline inlines it.

### Key assembly evidence

Both emit nearly identical loops:
```asm
; Brief
add    $0x1,%rbx              ; count = (count + 200) - 199 → count++
add    %rbx,%r12              ; acc += count (in register)

; C  
add    $0x1,%rax              ; count = (count + 200) - 199 → count++
add    %rax,%rdx              ; acc += count (in register)
```

## Compiler optimization paths

| Pass | Brief | C |
|------|-------|---|
| Interval detection | ✅ `detect_increments` computes net=1 from `(x+R1)-R2` | N/A — direct `count++` |
| Canonicalization | ✅ Simplify to `count++` after analysis | ✅ Same result |
| LTO inlines FFI | ✅ `__print_int` inlined | ❌ `fprintf` via PLT |
| SSA mode | ✅ Scalar registers | ✅ Standard while-loop |

## Reproduce

```bash
cargo build --release --bin brief-compiler
bash benchmarks/build_and_bench.sh interval_step
```