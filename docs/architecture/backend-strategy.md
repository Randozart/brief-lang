<!-- 2026-06-09. Updated 2026-06-09 — LLVM backend split into subdirectory -->

# Backend Strategy

## Principle

Backend codegen is extracted into feature files via per-backend traits.
Each backend is a separate trait so changing VHDL emission never
recompiles LLVM codegen.

## Three Canonical Backends (2026-06-15)

Only three backends are actively developed. All others are dead code:

| Backend | Target | Status |
|---------|--------|--------|
| **LLVM** (`src/backend/llvm/`) | Native binary (`.ll` + `llc`) | **Active** — canonical OS target |
| **Webstack** (`src/backend/webstack.rs`) | WASM + JS glue | **Active** — canonical web target |
| **CIRCT** (`src/backend/circt.rs`) | Hardware (`.mlir` + `circt-opt` + `circt-translate`) | **Active** — canonical hardware target (NEW) |

Dead backends: `verilog.rs`, `vhdl.rs`, `c.rs`, `rust.rs`, `cobol.rs`,
`x86_64.rs`, `aarch64.rs`, `wasm.rs`, `tcl_generator.rs` — zero fixes.

## LLVM Backend (Split into Subdirectory)

The LLVM backend is at `src/backend/llvm/` (11 files, ~8,700 total lines;
original monolithic `llvm.rs` was ~7,800 lines).

### File Layout

| File | Lines | Content |
|------|-------|---------|
| `mod.rs` | 1,776 | `LlvmBackend` struct (46 fields in 9 groups), `generate()` entry point, builder methods, codegen dispatch (A000-A006 decision), `build_field_index`, `validate_schema_types` |
| `emit_toplevel.rs` | 734 | Top-level emission: `emit_header`, `emit_declares`, `emit_init_state`, `emit_definition`, `emit_transaction`, `emit_callable_txn`, `emit_precondition_check`, `emit_pre_function`, `emit_async_body`, `emit_fused`, `emit_shape_guarded_body`, `emit_fused_composed`, `emit_trg_load`, `llvm_type`, `align_of`, `declare_state_type` |
| `emit_expr.rs` | 1,037 | `emit_expr()` router — all 20+ Expr variant arms including ProjectionTarget (18 targets), BracketOp (MultiSlice), Slice, collection emissions, field access, match/pattern, tuple |
| `emit_stmt.rs` | 477 | `emit_stmt()` router — all Statement variant arms with Guarded block handling, let_bindings save/restore across guard boundaries |
| `loop_engine.rs` | 1,080 | Folded loop engine: `emit_folded_main`, `emit_folded_memory_main`, `emit_ssa_main`, `emit_folded_loop`, `emit_folded_pure_counter`, `emit_trg_step` (NEW), `pre_extract_float/int_fields`, `pre_load_all_fields` |
| `reorder.rs` | 281 | Transaction body instruction reordering: read/write set analysis, dependency DAG, Kahn's topological sort for ILP (NEW 2026-06-15) |
| `dispatch.rs` | 256 | Reactor dispatch: `emit_reactor`, `emit_parallel_reactor`, `extract_ranges` |
| `optimizer.rs` | 280 | Decision tree: `select_optimization_strategy`, classify, extract trigger/enum keys |
| `hazard.rs` | 249 | SLP hazard analysis: `estimate_slp_hazard`, `slp_attr`, `compute_peak_live_floats` |
| `tests.rs` | 2,882 | 80+ unit tests — backend correctness, wake triggers, SLP hazard, chain composition, exit conditions, natural death, struct/enum, collections, projections |
| `kani.rs` | 57 | 6 Kani proof harnesses (fast group) |

### Emit Functions Remain Centralized

The `emit_expr` and `emit_stmt` functions remain in `emit_expr.rs` and
`emit_stmt.rs` as methods on `LlvmBackend`. The long-term plan is to
move individual Expr-variant arms into feature file `ExprCodegenLLVM`
impls (~20 cycles), matching the interpreter's `ExprEval` migration
pattern from Phase 9. For now, the directory split is sufficient: each
file is small enough to navigate, and no optimization path was touched.

## VHDL Backend

(`src/backend/vhdl.rs`, 1,261 lines) — expression emission extracted
into feature `ExprCodegenVHDL` impls. Optimizations deferred until
LLVM pattern is proven.

## CIRCT Backend (NEW 2026-06-15)

(`src/backend/circt.rs`, 240 lines) — emits MLIR text in HW + Comb
dialects. Trigger variables become top-level input ports. State variables
with initializer expressions become combinational logic. No dirty-bit
`step()` function needed — hardware is purely combinatorial.

```mlir
hw.module @top(clock: i1, reset: i1, sensor: i64, c: i64) -> () {
  %c0_c = hw.constant 0 : i64
  hw.output_assign c, %sensor : i64
  hw.output
}
```

Invoked externally: `brief-compiler circt <file.bv> → program.mlir →
circt-opt → circt-translate → verilog`. Same proven pattern as LLVM
backend emitting `.ll` text.

## Webstack Backend

(`src/backend/webstack.rs`, 2,327 lines) — expression emission extracted
into feature `ExprCodegenWebstack` impls. Now integrates `TopLevel::Trigger`
declarations as reactive signals via `collect_signals_and_transactions`.
Generates `step_triggers()` function that marks dirty signals and propagates
to dependent transactions. Optimizations deferred until LLVM pattern is proven.

## FFI Marshaling Convention (Critical)

The LLVM backend and C runtime must agree on how Brief types cross the FFI
boundary. The convention is documented at `brief_rt.c:376`:

> The LLVM backend marshals String as i8*, Int as i64, Bool as i64.

### Convention

| Brief type | C type | LLVM IR type | Notes |
|------------|--------|-------------|-------|
| `Int` | `int64_t` | `i64` | Passed directly |
| `Bool` | `int64_t` | `i64` | Truncated to `i32`/`i8` at frgn call sites only |
| `Float` | `float` / `double` | `float` / `double` | Float-specific SSA registers, bitcast to/from `i32` for storage |
| `String` | `const char*` | `ptr` (`i8*` in older LLVM) | Heap-allocated, null-terminated C string |
| `Data` | `const char*` | `ptr` | Same as String |
| `Char` | `int64_t` | `i64` | Unicode codepoint |

### Implementation pattern

**Frgn calls** (`emit_expr.rs:262-272`) use explicit `inttoptr`/`ptrtoint` casts:

```llvm
; frgn __print(msg: String) -> Bool  →  int64_t __print(const char* msg)
%fp  = inttoptr i64 %raw_msg to ptr
%ret = call i64 @__print(ptr %fp)
```

**Intrinsics** (like `read_file#`) must follow the same pattern — never pass raw
`i64` to C functions expecting pointers. The intrinsic at `emit_expr.rs:414-421`
uses the same `inttoptr` → `call ptr` → `ptrtoint` marshaling:

```llvm
%fp   = inttoptr i64 %path_val to ptr
%raw  = call ptr @brief_read_file(ptr %fp)
%data = ptrtoint ptr %raw to i64
```

This was the root cause of the `brief_read_file` bug (2026-06-14) — the intrinsic
was passing `i64` directly, and the C function interpreted raw characters as a
Brief header pointer. See `docs/architecture/fixes/brief-read-file-ffi-marshal.md`.

## Known Backend Bugs (Fixed 2026-06-13)

All bugs below were found during officina-cli compilation testing. Each
produced invalid LLVM IR that `opt` or `llc` rejected.

| # | Bug | File | Root Cause | Fix |
|---|-----|------|-----------|-----|
| 1 | Bare label `%` prefix | `emit_expr.rs` | Label definitions used `%name:`, should be `name:` | Removed `%` from 6 `writeln!` format strings |
| 2 | `%state` SSA scoping | `emit_toplevel.rs`, `emit_expr.rs` | `defn`/callable `txn` emitted functions without `%State* %state` parameter | Added `%State*` as first param in 2 function signatures + at call sites |
| 3 | Duplicate function definitions | `import_resolver.rs` | Same module imported through N paths produces N copies of items | `dedup_items()` after resolution loop — first occurrence wins |
| 4 | Unterminated basic block (9 sites) | `emit_toplevel.rs` | Leaked `self.terminated` from Guarded then-path suppressed caller's `ret` | Changed 9 `if !terminated { ret }` to unconditional `ret` (later refined) |
| 5 | Unterminated `post:` label | `emit_toplevel.rs` | Callable txn's body loop didn't emit `br %post` after last statement | Added `br label %post` when last statement didn't terminate |
| 6 | SSA dominance (17 violations) | `emit_stmt.rs` | Values from guard then-path (`%then_l`) referenced in merge path (`%end_l`) without phi | Save/restore `let_bindings` around then-path emission |
| 7 | Expr simplification exponential blowup | `equality_saturation.rs` | Fixpoint loop × recursive simplify on children = O(10^n) for 32-term `||` chain | Rewritten as bottom-up O(n) with hash-cons cache |
| 8 | Wrong ret after terminated guard | `emit_stmt.rs`, `emit_toplevel.rs` | Unconditional `ret` fix created dead code after terminator | Reverted to conditional; Guarded handler emits `ret` for else-path itself |
| 9 | Non-linear body with SSA insertvalue | `mod.rs`, `loop_engine.rs` | A005 SSA path used phi nodes even with non-exclusive guard conditions | Added `prove_linear()` check + A005b memory fallback |

**Verification**: After all fixes, `opt -O2` and `llc` pass on officina-cli
(4,280-line IR) with zero SSA violations. 777 compiler tests pass. All 30
benchmarks compile, all 7 runtime benchmarks produce correct output, and
`clang -O3` accepts the generated IR.

## `@ link` String Semantics (Fixed 2026-06-14)

### Problem
`@ link` for String types declared `external global i8*` and emitted
`load volatile i8*, i8** @sym; ptrtoint i64; add i64 0, %ptr`. This loaded
a **pointer address**, not string content. For C functions mapped as linked
globals (e.g., `tty_read_key`), the GOT entry contained the function's entry
point — comparing this against the empty string literal's address always
produced "not empty," making the trigger always fire.

### Fix
`@ link String` now uses **single-byte storage** (`i8` instead of `i8*`):

| Aspect | Before | After |
|--------|--------|-------|
| Storage type | `"i8*"` | `"i8"` |
| Load op | `load volatile i8*, i8** @sym` | `load volatile i8, i8* @sym` |
| Convert | `ptrtoint i8* %val to i64` | `zext i8 %val to i64` |
| Value meaning | Address of external symbol | First byte of data at symbol |

### Comparison with string literals
When a linked String trigger variable is compared against a string literal
(e.g., `keypress != ""`), the backend's `emit_fcmp` now checks for this pattern
and compares the trigger's byte value against the first byte of the literal
(0 for `""`). This ensures `keypress != ""`, `keypress == "\n"`, etc. all
compile to correct integer comparisons.

### C runtime
The linked global must be a `volatile char` variable in the C runtime.
The runtime's `__rt_wait()`/`__rt_poll()` read stdin data into the global
via epoll/kqueue events.

### Files changed
- `src/backend/llvm/mod.rs:318` — storage type
- `src/backend/llvm/emit_toplevel.rs:99-103` — load+zext
- `src/backend/llvm/emit_expr.rs` — `emit_fcmp` special case
- `src/backend/llvm/loop_engine.rs:622` — loop exit fix
- `lib/runtime/brief_rt.c` — `__tty_read_key` global + epoll reads, `__print` flush
