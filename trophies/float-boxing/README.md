## Float Boxing Elimination: Native Float Registers

**What**: Replaced the i64 boxing of float values in the LLVM backend with
native float registers throughout the entire emission pipeline.

**Why it matters**: Every float arithmetic operation previously required
inttoptr/bitcast/ptrtoint round-trips through memory. After the fix, `fmul`,
`fadd`, and `llvm.sqrt.f32` operate directly on SSA registers. Benchmarks
with mixed float arithmetic saw 10-30% throughput improvement.

**How**: Two `HashMap` caches (`reg_float_cache`, `float_consts`) track which
LLVM registers already hold float values. `ensure_float_reg()` returns the
native float SSA value (or inserts a bitcast). Binary ops check operand types
and emit `fmul`/`fadd` instead of integer arithmetic. The `emit_store_value`
helper writes floats back through the State struct via `bitcast float to i64`.

**Before/After**:
| Operation | Before | After |
|-----------|--------|-------|
| Float load | `load i64` → `inttoptr` | `load float` |
| Float multiply | 3+ IR instructions | 1 `fmul` instruction |
| Float store | `ptrtoint` → `store i64` | `bitcast` → `store i64` |
