# Fix Remaining SKIP & Performance Regressions

## Priority 1: Sqrt# Return Type (`intrinsics.rs:87`)

**Root cause**: Template-based intrinsic dispatch always returns `Type::int()`
regardless of the intrinsic's actual return type. `Sqrt#(float)` produces
`call i64 @llvm.sqrt.f32` — the register is i64 but the intrinsic returns float.

**Fix**: Determine return type from argument type. Float arg → `Type::float()`,
Float64 arg → `Type::float64()`, Int arg → `Type::int()`.

**Blocks**: nbody_sqrt_idio, nbody_sqrt rebuild

**File**: `src/backend/llvm/intrinsics.rs:87`

## Priority 2: `<-` Parser Regression (`parser/statements.rs`)

**Root cause**: The `<-` token is rejected as "unexpected token '<-'" at
statement level. The parser rewrite didn't carry over the `<-` statement
handler for collection push/pop.

**Fix**: Add `Token::ArrowLeft` (or `Token::LArrow`) handling to
`parse_statement` to parse `<- &queue` (pop) and `&queue <- expr` (push).

**Blocks**: queue_drain_idio, queue_drain rebuild

**File**: `src/parser/statements.rs`

## Priority 3: sparse_dispatch Performance (+617%)

**Root cause**: The modulo-rotated loop does 8 `icmp eq` + branches per
iteration. Phase 3 used `switch` which LLVM lowers to a jump table. Our
bound check (`icmp sge counter, total` + early branch) fires every
iteration even though total doesn't change.

**Fix**: Move bound check out of the main loop — check once at loop
start, or move the latch increment AFTER the exit check. Also consider
using LLVM's `switch` instruction instead of chained `icmp eq + br`.

**Impact**: sparse_dispatch goes from 0.63x to ~0.09x

**File**: `src/backend/llvm/loop_engine/ssa.rs`

## Priority 4: float_math_nonzero / nbody_newton Performance

**Root cause**: Our uncapped state stores fix emits GEP+store for ALL
fields in the per-field phi loop when `needs_state_stores_in_body` is
true. This adds ~30 extra stores per iteration for benchmarks that have
no post-loop hoisted prints.

**Fix**: Only emit state stores for fields that are actually referenced
by hoisted post-loop prints. Track which fields the swan song reads,
and only store those.

**Files**: `src/backend/llvm/loop_engine/counter.rs`

## Priority 5: Cross-benchmark Correctness Comparison (Harness)

**Fix**: Add `CORRECTNESS_REF[queue_drain_idio]=queue_drain_sym`
override in `build_and_bench.sh`.

**File**: `benchmarks/build_and_bench.sh`
