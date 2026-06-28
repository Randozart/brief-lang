# Float Types + Remaining Benchmark Gaps

**Date:** 2026-06-28
**Status:** Plan

## Overview

Two categories of work remain after the knucleotide/mandelbrot `glob_counter` → `txn_counter` register collision fix:

1. **Float type system redesign**: Split `Float` into `Float` (f32) and `Float64`/`Double` (f64), with Rust-like literal suffixes and no implicit widening.
2. **Small benchmark harness fixes**: Timer program output parsing, print_loop C reference asymmetry.

---

## Part 1: Float/Float64 Type Split

### Design Decisions

| Decision | Value |
|----------|-------|
| `Float` | f32 (LLVM `float`), same as current behavior |
| `Float64` | f64 (LLVM `double`), new type |
| `Double` | Parser alias for `Float64` (same AST node) |
| Default literal | `3.14` → `Float` (f32, backward compatible) |
| Suffixes | `3.14f32` / `3.14f` → `Float`; `3.14f64` / `3.14d` → `Float64` |
| Context inference | Unsuffixed literal's type is resolved from context (Rust-like); ambiguous defaults to `Float` |
| Widening | None — explicit `as Float64` / `as Float` required |
| Binary ops | `Float + Float64` → type error; same-type only |

### Implementation Steps

### Step 1: Lexer (`src/lexer.rs`)

- Current float regex `[0-9]+\.[0-9]+` → `Token::Float(f64)`
- New: parse optional suffix after the digits:
  - No suffix → `Token::Float(f64, FloatSuffix::Unspecified)`
  - `f32` or `f` suffix → `Token::Float(f64, FloatSuffix::F32)` — store as f64 but tagged
  - `f64` or `d` suffix → `Token::Float(f64, FloatSuffix::F64)`
- Add `FloatSuffix` enum: `Unspecified | F32 | F64`
- Add type keyword tokens: `#[token("Float64")] Token::TypeFloat64`, `#[token("Double")] Token::TypeDouble`

### Step 2: AST (`src/ast.rs`)

- Add `Type::Float64` variant
- Modify `Expr::Float(f64)` → `Expr::Float { val: f64, precision: FloatPrecision }` (or keep as `Expr::Float(f64)` for unsuffixed and add `Expr::Float64(f64)` for f64-suffixed)
- Add `FloatPrecision` enum: `Default | Float32 | Float64`
- Keep `Expr::Float` for backward compat where possible

**Recommended approach**: Since `Expr::Float` appears in 128+ match arms, the least invasive change is:
- Keep `Expr::Float(f64)` as-is (unsuffixed or f32-suffixed, resolved to `Float`)
- Add `Expr::Float64(f64)` for explicitly f64-suffixed literals (or context-resolved to `Float64`)
- Most match arms already have `_ =>` fallthrough; add `Expr::Float64(f) =>` alongside `Expr::Float(f) =>` where needed

### Step 3: Parser (`src/parser.rs`)

- Map suffixed tokens to the appropriate `Expr` variant
- Parse `Float64` and `Double` type annotations → `Type::Float64`

### Step 4: Typechecker (`src/typechecker.rs`)

- `LiteralExpr::Float(_)` → infers `Float` or `Float64` based on:
  - Explicit suffix → the suffixed type
  - Unsuffixed → context inference (type variable unified with expected type)
  - Ambiguous → default to `Float`
- Binary operations: `(Float, Float) → Float`, `(Float64, Float64) → Float64`, `(Float, Float64) → type error`
- No implicit widening — `as` cast required
- Coercion rules for `as`: `Float as Float64` and `Float64 as Float` both allowed explicitly

### Step 5: Interpreter (`src/interpreter.rs`)

- `Expr::Float64(f)` → `Value::Float(f as f64)` (currently float is stored as f64 in the interpreter, which works for both)
- Actually, the interpreter already stores Float as f64. `Value::Float` is always f64. So:
  - `Expr::Float(f)` → `Value::Float(f)` (truncate to f32 and back? No — interpreter keeps f64)
  - `Expr::Float64(f)` → `Value::Float(f)` (keep as f64)
- The distinction only matters at the type level (for type checking) and in LLVM codegen
- Update `try_eval` functions: `try_eval_float32` vs `try_eval_float64` for constant folding

### Step 6: LLVM Backend

#### Type mapping (`mod.rs:200-208`)
- `Type::Float` → `"float"` (f32) — unchanged
- `Type::Float64` → `"double"` (f64) — new

#### Float literal emission (`emit_expr.rs:39-43`)
- `Expr::Float(f)` → `bitcast i32 {f32_bits} to float` (unchanged, truncate f64→f32)
- `Expr::Float64(f)` → `bitcast i64 {f64_bits} to double` (new, keep f64 as f64)
- Add `float64_to_llvm_hex(f: f64) -> String` that emits f64 bit pattern as i64

#### Constant folding (`mod.rs:try_eval_cfloat`)
- Keep f64 folding for `Expr::Float` (unchanged — truncation at codegen)
- Add `try_eval_cfloat64` that folds in f64 and returns f64 (no truncation)
- `Expr::Float64` uses `try_eval_cfloat64`

#### Arithmetic emission (`emit_expr.rs`)
- `Expr::BinaryOp` with `Float32` operands → `fadd fast float`, `fmul fast float`, etc. (unchanged)
- `Expr::BinaryOp` with `Float64` operands → `fadd fast double`, `fmul fast double`, etc. (new)
- `Expr::UnaryOp` same pattern
- Intrinsics: `sqrt#` → `@llvm.sqrt.f32` for Float, `@llvm.sqrt.f64` for Float64
- `fabs#`, `ceil#`, `floor#` — same split

#### Projection fast path (`loop_engine.rs`)
- `field_types` stores `"float"` or `"double"` — handle both
- `ensure_float_reg` → renamed to handle both f32 and f64, or two functions

#### Print intrinsic
- `print_float#(f: Float)` → fpext to double → `fprintf "%.9f"` — already handles this for f32 by extending
- `print_float#(f: Float64)` → no fpext needed, just pass double directly
- Or: add `print_float64#` intrinsic

#### Format strings (`mod.rs`)
- `@FMT_FLOAT = private unnamed_addr constant [6 x i8] c"%.9f\0A\00"` — already uses double for %f, works for both

### Step 7: GPU Backend (SPIR-V)

- `is_gpu_safe_intrinsic` — update to handle both float types
- Print buffer handling — f32 vs f64 sized differently

### Step 8: Webstack Backend

- `Type::Float` → JS number (no change)
- `Type::Float64` → also JS number
- Print handlers: `console.log` works for both

### Step 9: CIRCT Backend

- Add `Type::Float64` → appropriate MLIR type

### Step 10: Benchmarks

- **nbody_newton.bv**: Change `const pi: Float = 3.141592653589793` to `const pi: Float64 = 3.141592653589793f64` (or `3.141592653589793d`) to match C's `const float pi = 3.141592653589793f` behavior at f64 precision
- **nbody_sqrt.bv**: Same
- **nbody_sym variants**: Same
- **float_math.bv**: May need type annotations

### Step 11: Standard Library

- `lib/std/types.bv`: Add `Float64` and `Double` as exportable type names
- `lib/std/io.bv`: `print_float` wrapper should handle both `Float` and `Float64`
- Update any `frgn` declarations that use `Float` → may need `Float64` variants

### Step 12: Tests

- Add parser tests for suffix parsing: `3.14f32`, `3.14f64`, `3.14f`, `3.14d`
- Add typechecker tests: mixed-type binary ops rejected, `as` casts working
- Add interpreter tests for `Expr::Float64`
- Add LLVM backend tests verifying f64 codegen
- Update existing tests that create `Expr::Float` if signature changes

---

## Part 2: Timer Harness — Child Stdout Leak

**Root cause:** `/tmp/brief_bench_timer.c` forks the benchmark process. Both the timer program (parent) and the benchmark (child) write to stdout. The shell captures ALL of stdout with `$(...)`, mixing progress prints with the timing value. Shell arithmetic (`bc`) can't parse the mixed output.

**Fix:** `freopen("/dev/null", "w", stdout)` before `execvp` in the child process.

**Location:** `benchmarks/build_and_bench.sh`, lines 183-184 (the inline C source for TIMER_BIN)

```c
if (pid == 0) {
    freopen("/dev/null", "w", stdout);  // suppress benchmark output
    execvp(argv[1], &argv[1]);
    _exit(127);
}
```

**Effect:** All benchmarks that print progress (ring_buffer, async_counters, etc.) will still run correctly but their output won't pollute the timing measurement. The timer program is the only thing writing to stdout → clean parse.

---

## Part 3: print_loop C Reference — Hardcoded N

**Root cause:** `print_loop_c.c` hardcodes `const long N = 50000000;` instead of reading `BOUND` from the environment. The Brief version reads `get_env_int("BOUND")`. At BOUND=5 (correctness check), the C version runs 50M iterations (lots of output) while Brief runs 5 iterations (no output) → false MISMATCH.

**Fix:** Replace with `long N = getenv("BOUND") ? atol(getenv("BOUND")) : 50000000;`

**Location:** `benchmarks/print_loop_c.c`, lines 8-9

**Effect:** The C reference runs the same number of iterations as the Brief version for a given BOUND. Correctness checks at BOUND=5 produce identical (empty) output. Timing at BOUND=50000000 is unaffected.

---

## Verification

```bash
# Part 1 (after implementation)
cargo test --lib        # all existing + new tests pass
cargo build --release   # no warnings
BOUND=5 ./target/release/brief-compiler/benchmarks/nbody_newton   # matches C output

# Part 2
bash benchmarks/build_and_bench.sh --runtime  # no parsing errors, clean output

# Part 3
BOUND=5 bash benchmarks/build_and_bench.sh --correctness  # print_loop shows MATCH
```

---

## Ordering

1. **Part 3 (print_loop C ref)** — 2 min, trivial. Do first.
2. **Part 2 (timer harness)** — 5 min, trivial. Do second.
3. **Part 1 (Float/Float64 split)** — Larger effort, implement subsystem by subsystem:
   a. Lexer + parser (add suffix parsing, type keywords)
   b. AST (add `Type::Float64`, `Expr::Float64`)
   c. Typechecker (inference, coercion rules)
   d. Interpreter (f64 handling)
   e. LLVM backend (double codegen, literal emission, intrinsics)
   f. Other backends (webstack, CIRCT, GPU)
   g. Benchmarks + stdlib + tests
