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

## i64 Boxing Convention (Phase 0, 2026-06-16)

The LLVM backend boxes native types to `i64` for a uniform internal ABI.
This avoids SSA type proliferation — every value slot is `i64` regardless
of the Brief-level type. The `TypedRegister.ty` field tracks the Brief type,
and `adapt_to_i64()` is the canonical function that produces a boxed `i64`
from any `TypedRegister`, regardless of its current native/boxed state.

### The Convention

| Brief type | Internal LLVM type | TypedRegister.ty | adapt_to_i64 |
|------------|-------------------|------------------|-------------|
| `Bool` | `i1` (native) or `i64` (boxed) | `Type::Bool` or `Type::Int` | `zext i1 to i64` / pass-through |
| `Char` | `i32` (native) or `i64` (boxed) | `Type::Char` or `Type::Int` | `zext i32 to i64` / pass-through |
| `String` | `i8*` (native) or `i64` (boxed) | `Type::String` or `Type::Int` | `ptrtoint i8* to i64` / pass-through |
| `Int` | `i64` | `Type::Int` | pass-through |
| `Float` | `float` (via cache) | `Type::Float` | pass-through |

### Rule: When a value is already `i64` (boxed), `ty` MUST be `Type::Int`

This is the critical invariant. If a `TypedRegister` has `ty == Type::String`
but its register is actually `i64` (because it was loaded from a state field
that stores boxed values), `adapt_to_i64` will incorrectly emit
`ptrtoint i8* %i64_reg to i64` — treating an integer as a pointer.

### Where boxing happens

| Action | Emits |
|--------|-------|
| Function param entry | `ptrtoint i8* %arg to i64` (String), `zext i32 %arg to i64` (Char), `zext i8 %arg to i64` (Bool) → `ty: Type::Int` |
| State field load (`i8*` field) | `load i8*` then `ptrtoint` to i64 → `ty: Type::Int` |
| State field load (`i8` field, Bool) | `load i8` then `trunc to i1` → `ty: Type::Bool` |
| Literal emission (`LiteralExpr::Char`) | `zext i32 %char to i64` → `ty: Type::Int` |
| Literal emission (`LiteralExpr::String`) | `ptrtoint i8* %ptr to i64` → `ty: Type::Int` |
| Cast `Int → String` | `call i64 @__int_to_str` → `ty: Type::Int` |
| Cast `String → Int` | `call i64 @__str_to_int` → value stays `i64` |

### Where unboxing happens

| Action | Emits |
|--------|-------|
| Guard condition | `icmp ne i64 %boxed, 0` (treats boxed Bool as non-zero = true) |
| Internal call arg (Bool) | `adapt_to_i64` → `trunc i64 to i8` (C ABI uses `i8` for Bool) |
| Internal call arg (String) | `adapt_to_i64` → `inttoptr i64 to i8*` |
| FFI call arg (String) | `adapt_to_i64` → `inttoptr i64 to i8*` |
| Field store to `i8` (Bool) | `adapt_to_i64` → `trunc i64 to i8` |
| Comparison (`icmp`) | Both operands run through `adapt_to_i64` first |

### `adapt_to_i64` — the canonical boxer

Defined in `emit_stmt.rs` as `pub(super) fn adapt_to_i64`:

```rust
match r.ty {
    Type::Bool => zext i1 %name to i64,
    Type::Char => zext i32 %name to i64,
    Type::String | Type::Data => ptrtoint i8* %name to i64,
    _ => r.name.clone(),  // Type::Int or Type::Float → pass through
}
```

This function must be called before:
- Any `store i64` to a state field, tuple slot, or param slot
- Any `trunc i64 to i8` (Bool) or `trunc i64 to i32` (Char) for field/ABI conversion
- Any `inttoptr i64 to i8*` (String) for FFI/internal call arg passing
- Any `icmp`/`fcmp` that expects `i64` operands

### FFI Marshaling Convention (Updated 2026-06-16)

The FFI boundary uses a different convention — C types, not internal types:

| Brief type | C type | LLVM IR type | MarshaL |
|------------|--------|-------------|---------|
| `Int` | `int64_t` | `i64` | Direct |
| `Bool` | `int64_t` | `i32` | `trunc i64 %boxed to i32` at FFI call site |
| `Float` | `float` | `float` | `ensure_float_reg` |
| `String` | `const char*` | `ptr` (`i8*`) | `inttoptr i64 %boxed to ptr` |
| `Data` | `const char*` | `ptr` | Same as String |
| `Char` | `int64_t` | `i32` | `trunc i64 %boxed to i32` |

FFI calls at `emit_expr.rs:311-335` use the marshal table above.
Intrinsics that call C functions (ReadFile, Spawn) must apply the same
marshaling — this was the root cause of the `brief_read_file` bug (2026-06-14).

## Known Backend Bugs (Fixed 2026-06-16)

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

| 10 | `Type::String` on boxed i64 | `emit_expr.rs` (4 sites), `emit_stmt.rs` (3), `emit_toplevel.rs` (3), `features/literal.rs` (2) | `TypedRegister.ty` out of sync with LLVM type — `ptrtoint i8* %i64` invalid IR | `adapt_to_i64` canonical boxer; `Type::Int` for all boxed i64 values; 18 edits across 4 files |
| 11 | `emit_fcmp` assumed `i64` operands | `emit_expr.rs` | `icmp eq i64 %i1, %i64` when one operand was native i1 Bool | `adapt_to_i64` both operands before `icmp` |
| 12 | `emit_binop` assumed `i64` operands | `emit_expr.rs` | `add i64 %i1, %i64` when one operand was native i1 Bool | `adapt_to_i64` both operands before `i64` ops |
| 13 | Tuple/List element stores used raw type | `emit_expr.rs` | `store i64 %i1, i64*` when element was native i1 Bool | `adapt_to_i64` each element before storing |
| 14 | Guarded field store used raw type | `emit_stmt.rs` | `trunc i64 %i1 to i8` when value was native i1 | `adapt_to_i64` before trunc |
| 15 | SSA `insertvalue` store used raw type | `emit_stmt.rs` | `insertvalue %State, i8 %i1` when value was native i1 | `adapt_to_i64` before trunc/insert |
| 16 | Param slot store used raw type | `emit_stmt.rs` | `store i64 %i1, i64*` when value was native i1 | `adapt_to_i64` + store `Type::Int` in binding |
| 17 | `emit_init_state` field store used raw type | `emit_toplevel.rs` | `trunc i64 %i1 to i8` when literal was native i1 | `adapt_to_i64` before trunc |
| 18 | `CallableTxn` param binding used real type | `emit_toplevel.rs` | `let_binding_types["p"] = Type::Bool` for boxed i64 Bool | Store `Type::Int` for boxed types |
| 19 | `Expr::Cast` returned `Type::String` for i64 | `emit_expr.rs` | `__int_to_str` returns i64 but `ty = String` | Return `Type::Int` for String/Data casts |
| 20 | ReadFile/Spawn intrinsics returned `i8*` as `Type::String` | `emit_expr.rs` | Downstream code expected `i64` for string values | `ptrtoint` return to i64 + `Type::Int` |
| 21 | `InlineConcat` assumed `i8*` operands | `emit_expr.rs` | `bitcast i8* %i64` when operand was already i64 (boxed) | `inttoptr i64` instead of `bitcast i8*` |

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
