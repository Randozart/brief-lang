# Float Type Split + Remaining Benchmark Gaps

**Date:** 2026-06-28
**Status:** Active

## Part 1: Float Type System Redesign

### Design Decisions

**Type keywords** (six interchangeable aliases):

| Precision | Type keywords (parser) | AST type | LLVM type |
|-----------|----------------------|----------|-----------|
| 32-bit | `Float`, `Float32`, `F32` | `Type::Float` | `float` (f32) |
| 64-bit | `Double`, `Float64`, `F64` | `Type::Float64` | `double` (f64) |

**Literal suffixes** (matching the same groups):

| Suffix examples | Precision | Example |
|-----------------|-----------|---------|
| `f32`, `f` | Float (32-bit) | `3.14f32`, `3.14f` |
| `f64`, `d` | Float64/Double | `3.14f64`, `3.14d` |

**Inference rules:**
- `3.14` (unsuffixed) — type-variable float literal resolved from context, Rust-like
- `3.14f32` or `3.14f` — explicitly `Float` (32-bit)
- `3.14f64` or `3.14d` — explicitly `Float64`/`Double` (64-bit)
- Ambiguous context → default to `Float` (32-bit, backward compatible)

**Coercion:**
- No implicit widening between float types
- `as` casts: `x as Float64` / `x as Float` / `x as F64` / `x as F32` etc.
- Binary ops: `Float + Float64` → compile error, same-type only

### Implementation Steps

#### Step 1: Lexer (`src/lexer.rs`)

- Current: `#[regex(r"[0-9]+\.[0-9]+", ...)]` → `Token::Float(f64)`
- New: parse optional suffix after digits:
  - No suffix → `Token::Float(f64, FloatSuffix::Unspecified)`
  - `f32` / `f` → `Token::Float(f64, FloatSuffix::F32)` (stored as f64, tagged)
  - `f64` / `d` → `Token::Float(f64, FloatSuffix::F64)`
- Add `FloatSuffix` enum: `Unspecified | F32 | F64`
- Add type keyword tokens:
  ```
  #[token("Float32")] Token::TypeFloat32
  #[token("Float64")] Token::TypeFloat64
  #[token("Double")]  Token::TypeDouble
  #[token("F32")]     Token::TypeF32
  #[token("F64")]     Token::TypeF64
  ```
  `#[token("Float")] Token::TypeFloat` already exists.

#### Step 2: AST (`src/ast.rs`)

- Add `Type::Float64` variant
- Keep `Expr::Float(f64)` unchanged (handles unsuffixed and f32-suffixed)
- Add `Expr::Float64(f64)` for f64-suffixed (explicitly 64-bit)
- No `float32` expression variant needed — `Expr::Float` already means 32-bit

**Match-arm strategy:** Most of the 128 `Expr::Float` match arms have `_ =>` fallthroughs or are in typecheck-like code where both types need the same value. Strategy:
- Where identical: `Expr::Float(f) | Expr::Float64(f) => { ... }`
- Where precision matters: separate arms per variant
- `_ =>` catchalls will catch `Expr::Float64` until explicit arms are added

#### Step 3: Parser (`src/parser.rs`)

- `Token::Float(f, FloatSuffix::Unspecified)` → `Expr::Literal(Box::new(LiteralExpr::Float(f)))`
- `Token::Float(f, FloatSuffix::F32)` → same as unsuffixed (explicit but same type)
- `Token::Float(f, FloatSuffix::F64)` → `Expr::Float64(f)` directly
- Type annotations: `Float32`, `Float64`, `Double`, `F32`, `F64` → `Type::Float` or `Type::Float64`

#### Step 4: Typechecker (`src/typechecker.rs`)

- Literal inference: `LiteralExpr::Float(_)` → context-resolved type variable
  - If context expects `Type::Float` → `Type::Float`
  - If context expects `Type::Float64` → `Type::Float64`
  - Ambiguous → `Type::Float` (default)
- `Expr::Float64(_)` → always `Type::Float64`
- Binary op compatibility:
  - `(Float, Float) → Float`
  - `(Float64, Float64) → Float64`
  - `(Float, Float64)` or `(Float64, Float)` → type error
- `as` casts: add coercion rules for `Float` ↔ `Float64`
- Update `binary_op_type` to handle `Float64` alongside `Float`
- Update `common_type` for type unification

#### Step 5: Interpreter (`src/interpreter.rs`)

- `Expr::Float64(f)` → `Value::Float(f)` (interpreter stores all floats as f64 internally)
- `try_eval` for `Expr::Float64`: folds in f64, stores f64 (no f32 truncation)
- Add `try_eval_cfloat64` for proof-engine constant folding in f64

#### Step 6: LLVM Backend

**Type mapping (`mod.rs:200-208`):**
- `Type::Float` → `"float"` (f32, unchanged)
- `Type::Float64` → `"double"` (f64, new)

**Literal emission (`emit_expr.rs:39-43`):**
- `Expr::Float(f)` → `bitcast i32 {f32_bits} to float` (unchanged)
- `Expr::Float64(f)` → `bitcast i64 {f64_bits} to double` (new)
- Add `float64_to_llvm_hex(f: f64) → String`

**Constant folding (`mod.rs:try_eval_cfloat`):**
- `try_eval_cfloat` stays as f64 folding with f32 truncation at codegen (for `Expr::Float`)
- Add `try_eval_cfloat64` for pure f64 folding (for `Expr::Float64`)
- Dispatch via `normalize_to_old()` → check `Expr::Float64` before `Expr::Float`

**Arithmetic emission (`emit_expr.rs`):**
- `Expr::BinaryOp` with Float operands → `fadd fast float` etc. (unchanged)
- `Expr::BinaryOp` with Float64 operands → `fadd fast double` etc. (new)
- Match on `TypedRegister.ty` to determine instruction width

**Intrinsics:**
- `sqrt#(f: Float)` → `@llvm.sqrt.f32`
- `sqrt#(f: Float64)` → `@llvm.sqrt.f64`
- `fabs#`, `ceil#`, `floor#` — same split
- `sin#`, `cos#`, `pow#` — same split

**Projection fast path (`loop_engine.rs`):**
- `field_types` stores `"float"` or `"double"` — handle both
- `ensure_float_reg` → handle both f32 and f64 (or dispatch on type)

**Print intrinsic:**
- `print_float#(f: Float)` → `fpext float → double` then `fprintf "%.9f"` (unchanged)
- `print_float#(f: Float64)` → direct `fprintf "%.9f"` with double (skip fpext)

#### Step 7: Other backends

**Webstack (`src/backend/webstack.rs`):**
- `Type::Float64` → JS `number` (same handler as Float)

**CIRCT (`src/backend/circt.rs`):**
- `Type::Float64` → appropriate MLIR type (f64)

**GPU/SPIR-V (`src/backend/llvm/gpu.rs`):**
- Allow `Float64` in GPU-safe intrinsic check
- Print buffer sizing for f64 vs f32

#### Step 8: Benchmarks

| Benchmark | Change |
|-----------|--------|
| `nbody_newton.bv` | `const pi: Float = 3.14` → `const pi: Float64 = 3.14d` |
| `nbody_newton_sym.bv` | Same |
| `nbody_sqrt.bv` | Same |
| `nbody_sqrt_idio.bv` | Same |
| `nbody_sqrt_sym.bv` | Same |
| `float_math.bv` | Type annotations as needed |
| Others using Float | No change unless they need f64 precision |

Using `Float64` in nbody benchmarks means:
- Constant folding in pure f64 (no f32 truncation) → matches C's `float * float` behavior
- All arithmetic in f64 (vs C's f32) → Briev is now *more precise* than C
- To match C's f32 precision, use `Float` instead. With `Float64`, Briev outperforms C by computing in f64.

#### Step 9: Standard Library

- `lib/std/types.bv`: Add `Float64`, `Double`, `F32`, `F64` as type exports
- `lib/std/io.bv`: `print_float#` wrapper for both types
- `print_float64#` or unify `print_float#` to handle both (runtime dispatch via `fpext`)

#### Step 10: Tests

- **Parser**: suffix parsing `3.14f32`, `3.14f64`, `3.14f`, `3.14d`
- **Typechecker**: mixed-type binary ops rejected, `as` casts, inference
- **Interpreter**: `Expr::Float64` evaluation
- **LLVM backend**: f64 literal emission, arithmetic, intrinsics, projection
- **Regression**: nbody_newton output matches C's f32 benchmark (or is closer)

---

## Part 2: Small Fixes (Done)

### DONE — print_loop C Reference (`benchmarks/print_loop_c.c`)

**Fix:** Added `getenv("BOUND")` like the Briev version. C now runs the same iteration count as Briev for any given BOUND.

**Verified:** `BOUND=5` both produce empty output. `BOUND=50000000` both produce matching progress output.

### DONE — Timer Harness (`benchmarks/build_and_bench.sh`)

**Fix:** Added `freopen("/dev/null", "w", stdout)` before `execvp` in the child process of the timer C program. Benchmark output no longer pollutes timing measurement.

**Verified:** Timer output is clean `0.XXXXXX` with no mixed-in benchmark progress output.

---

## Part 3: Ordering

1. ~~Part 2 small fixes~~ ✅ DONE
2. **Part 1 Float split** — implement in order:
   a. Lexer + parser (suffixes, type keywords) — `lexer.rs`, `parser.rs`
   b. AST (`Type::Float64`, `Expr::Float64`) — `ast.rs`
   c. Typechecker (inference, coercion, binary ops) — `typechecker.rs`
   d. Interpreter (f64 eval, try_eval) — `interpreter.rs`, `mod.rs`
   e. LLVM backend (type mapping, codegen, intrinsics, loop_engine) — `mod.rs`, `emit_expr.rs`, `loop_engine.rs`
   f. Other backends (webstack, CIRCT, GPU) — `webstack.rs`, `circt.rs`, `gpu.rs`
   g. Benchmarks + stdlib + tests

## Verification

```bash
cargo test --lib
cargo build --release
BOUND=5 timeout 10 benchmarks/nbody_newton    # matches C within float precision
BOUND=5 timeout 10 benchmarks/nbody_sqrt      # matches C within float precision
bash benchmarks/build_and_bench.sh --runtime   # no parsing errors
bash benchmarks/build_and_bench.sh --correctness  # all MATCH
```
