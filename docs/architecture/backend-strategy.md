<!-- 2026-06-09. Updated 2026-07-05 — optimization results added -->
<!-- See docs/architecture/overview.md for current dispatch architecture -->

# Backend Strategy

## Recent Optimizations (2026-07-05)

Key updates since the original writing of this document:

- **A005e (hybrid counter-phi + memory) removed** (`4ff9bde`): Reverted to
  A005c per-field phi dispatch. interval_step: 0.01x vs 1.00x (100× faster).
- **A005a (insertvalue chain) re-added** (`4ff9bde`): Adaptive dispatch
  selects A005a for dense-write bodies with <8 fields and no FFI.
- **Vector phi emission** (`a849b2d`): `<4 x float>` vector phis for grouped
  float fields. Reduces register pressure from 32 scalar phis to ~14, fits
  in 16 XMM registers. nbody_sqrt: 1.25x → 0.79x (beats C by 21%).
- **Rotation decomposition** (`ca9f483`): GEP-reload latch breaks 12-element
  circular phi chain for fannkuch_redux. 1.65x → 1.37x.
- **Precomputation fix** (`981819c`): `@stdout` dso_local + volatile load
  prevents LLVM from eliminating fprintf. knucleotide: 0x → 0.99x.
- **Dead-field liveness** (`6529f29`): trace_live_fields correctly traces
  through guard conditions and Let-wrapped output calls, fixing nbody_newton
  and float_math MISMATCHes.
- **@stdout volatile load** (`981819c`): Prevents LLVM globalopt from
  treating @stdout as null (external global without initializer = undef).

## Principle

Backend codegen is extracted into feature files via per-backend traits.
Each backend is a separate trait so changing VHDL emission never
recompiles LLVM codegen.

## Three Canonical Backends (2026-06-15)

Only three backends are actively developed. All others are dead code.

| Backend | Target | Status |
|---------|--------|--------|
| **LLVM** (`src/backend/llvm/`) | Native binary (`.ll` + `llc`) | **Active** — canonical OS target |
| **Webstack** (`src/backend/webstack.rs`) | WASM + JS glue | **Active** — canonical web target |
| **CIRCT** (`src/backend/circt.rs`) | Hardware (`.mlir` + `circt-opt` + `circt-translate`) | **Active** — canonical hardware target |

### Dead Backends (Archived 2026-06-19)

Nine dead backends were moved to `archive/backend/` — zero fixes:
`verilog.rs`, `vhdl.rs`, `c.rs`, `rust.rs`, `cobol.rs`, `x86_64.rs`,
`aarch64.rs`, `wasm.rs`, `tcl_generator.rs`. All `main.rs` call sites
return errors. VHDL trait system (`ExprCodegenVHDL`, `StmtCodegenVHDL`)
removed from `src/features/traits.rs` and all 28 `impl` blocks.

## Backend Routing by Extension (2026-06-19, corrected 2026-08-23)

File extension determines which backend (and `CompilationTarget`) is used:

| Extension | Backend | Notes |
|-----------|---------|-------|
| `.bv`/`.sbv` | LLVM | Standard native binary |
| `.ebv`/`.sebv` | LLVM | Bare-metal LLVM, `halt#` emits `wfi` |
| `.rbv`/`.srbv` | Webstack | Rendered Briev → wasm32 via `LlvmBackend(wasm32)` + JS shim |
| `.cbv` | CIRCT | Pure logic graph → MLIR |
| `.abv` | SPIR-V | Standalone GPU-kernel binaries (`spirv/mod.rs`) |
| *(none)* | VM | Reachable via `brievc bounty`; the tamer finishes compilation on the install machine |

**Routing truth (2026-08-23):** `config/targets.dbvl` maps extensions; the
table above matches it. Two older claims in this file were wrong and are
corrected here:
- `.abv` does NOT route through the LLVM accel path. Standalone SPIR-V
  emission is its own backend (`BackendKind::Spirv`).
- GPU OFFLOAD is a different mechanism entirely: module-level `!> accel:`
  metadata drives `BackendKind::Gpu` (LLVM emitter reuse, plan
  `2026-08-06-accel-gpu-offload.md`). It is selected by metadata, not by
  file extension.
- The VM has no user-facing extension; it exists to FINISH COMPILATION on
  any machine with a tamer — one `.bounty` archive ships everywhere, macros
  adapt to the target machine at install time (plan
  `2026-08-23-vm-compile-tail-parity.md`).

Routing dispatch is `run_build()` (`src/main.rs:453`) → `codegen()`
(`src/compile.rs`), which dispatches on `opts.backend` (inferred from the
file extension via `config/targets.dbvl`).

## Embedded LLVM Mode (.ebv) (2026-06-19)

When the source is an `.ebv` file (`get_extension == ".ebv"`), the LLVM backend
activates embedded mode:

| Feature | Status | Detail |
|---------|--------|--------|
| `CompilationTarget::Embedded` | ✅ | Typechecker uses embedded-specific rules |
| `Intrinsic::Halt` | ✅ | `halt#()` emits `asm("wfi")` |
| `is_embedded` flag | ✅ | `LlvmBackend.is_embedded: bool` + `with_embedded_mode()` builder |
| `term!` → `wfi` | ✅ | `Statement::TermBang` emits `wfi` asm before `ret` in embedded mode |
| Static bump heap | ✅ | `@embedded_heap` (configurable via `ir-lowering arena_initial_size`, default 64KB) — `Malloc#`/`Alloc#`/`Free#` use it, no `@malloc`/`@free`/`@realloc` (2026-08-04, `f2b57043`) |
| Heap types (String/List/…) | ✅ (warn) | Legal via the static arena; `check_embedded_restrictions` emits a `TargetWarning`, not an error (2026-08-04 — the old hard rejection was a `.ebv`/`.cbv` entanglement vestige; the heap rejection belongs to `.cbv`/CIRCT, not `.ebv`) |
| Reject threading | ✅ | ThreadCreate, MutexLock, CondvarWait etc. produce `TargetError` |
| Freestanding linker | ⬜ | `-ffreestanding -nostdlib -nostartfiles` |
| Unbounded recursion | ⬜ | Static call-graph depth check |
| ISR annotations | ⬜ | `#[interrupt(NAME)]` on `trg` declarations |

### Restrictions Checker

`check_embedded_restrictions()` (`src/backend/llvm/mod.rs:1519`) scans the typed AST:
- **State/local declarations of heap types** (`#String`, `#Data`, `List`, `HashMap`, …): emits a **`TargetWarning`** that the value uses the finite static bump arena — NOT an error (2026-08-04 reframe).
- **Threading intrinsics**: rejects `ThreadCreate`, `ThreadJoin`, `ThreadExit`, `MutexLock`, `MutexUnlock`, `CondvarWait`, `CondvarSignal`, `CondvarBroadcast` — bare metal has no threads.
- **Unbounded recursion**: warns (no stack growth).

Called from `generate()` before any code emission. Warnings are non-fatal.

## LLVM Backend (Split into Subdirectory)

The LLVM backend is at `src/backend/llvm/` (11 files, ~9,400 total lines;
original monolithic `llvm.rs` was ~7,800 lines).

### File Layout

| File | Lines | Content |
|------|-------|---------|
| `mod.rs` | 1,933 | `LlvmBackend` struct (46 fields in 9 groups), `generate()` entry point, builder methods, codegen dispatch (A000-A006 decision), `build_field_index`, `validate_schema_types` |
| `emit_toplevel.rs` | 734 | Top-level emission: `emit_header`, `emit_declares`, `emit_init_state`, `emit_definition`, `emit_transaction`, `emit_callable_txn`, `emit_precondition_check`, `emit_pre_function`, `emit_async_body`, `emit_fused`, `emit_shape_guarded_body`, `emit_fused_composed`, `emit_trg_load`, `llvm_type`, `align_of`, `declare_state_type` |
| `emit_expr.rs` | 2,523 | `emit_expr()` router — all Expr variant arms including ProjectionTarget (18 targets), ~60 migrated intrinsic dispatch arms (direct libc), BracketOp (MultiSlice), Slice, collection emissions, field access, match/pattern, tuple |
| `emit_stmt.rs` | 586 | `emit_stmt()` router — all Statement variant arms with Guarded block handling, let_bindings save/restore across guard boundaries, SSA field-write re-extraction for intra-body consistency |
| `loop_engine.rs` | 1,398 | Folded loop engine: `emit_folded_main`, `emit_folded_memory_main`, `emit_ssa_main`, `emit_folded_loop`, `emit_folded_pure_counter`, `emit_trg_step` (NEW), `pre_extract_float/int_fields`, `pre_load_all_fields` |
| `reorder.rs` | 281 | Transaction body instruction reordering: read/write set analysis, dependency DAG, Kahn's topological sort for ILP |
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

## CIRCT Backend

(`src/backend/circt.rs`, 860 lines, 17 unit tests) — emits MLIR text in
HW + Comb + Seq dialects. Trigger variables become input ports, state
variables become `seq.firreg` sequential registers, contract guards are
lowered to `comb.icmp` operations, and transaction bodies become FSMs
with state registers.

### Architecture

```
CirctBackend
├── trigger_port_name()      — LinkRef → port name mapping
├── mlir_type()              — Type → MLIR type (supports sized ints via Constrained)
├── emit_module()
│   ├── input_ports          — clock, reset, trg variables (in % prefix)
│   ├── output_ports         — state vars, wake_<trg>, halt (return signature)
│   ├── seq.firreg           — sequential registers for state variables
│   ├── emit_expr()          — recursion into comb/arithmetic ops
│   │   ├── emit_binary_comb — comb.add/sub/mul/divu/and/or/xor/icmp
│   │   └── emit_unary_comb  — comb.xor/neg (via xor with 1)
│   ├── emit_contract_condition — pre/post guards via comb.icmp
│   └── emit_txn_body()      — FSM state register + body + halt
│       ├── FSM state reg    — 2-bit seq.firreg for idle/running/done
│       ├── Await handling   — sub-module start/done handshake + stall
│       ├── Async handling   — fire-and-forget body emission
│       ├── SyncBlock        — parallel combinational paths
│       └── TermBang         — drives halt output port high
```

### Module Output Convention (modern CIRCT)

```mlir
hw.module @top(in %clock: i1, in %reset: i1, in %sensor: i64)
    -> (counter: i64, wake_btn: i1, halt: i1) {
  %counter_0 = seq.firreg initial_value { init_value = 0 : i64 } : i64
  %counter_next = comb.add %counter_0, %c1_i64 : i64
  seq.always(posedge %clock) {
    %counter_0 <= %counter_next
  }
  hw.output %counter_0 : i64, %c0_i1 : i1,
}
```

### FSM State Machine

Each `node` body becomes a finite state machine:

| State | Value | Meaning |
|-------|-------|---------|
| Idle  | 0 | Waiting for precondition |
| Running | 1 | Executing body |
| Done  | 2 | Postcondition met, commit |

State transitions: `pre → run → (stall on await) → post → done → idle`

### Sized Integer Support

`mlir_type()` maps `Type::Constrained(inner, BitRange::Single(N))` to `iN`,
enabling precise bit-widths (i8, i16, i32, i64) for efficient FPGA synthesis.

### Trigger Port Mapping

| `LinkRef` variant | Port name | Example |
|-------------------|-----------|---------|
| `Explicit(addr)` | Trigger name | `sensor` |
| `Linked("name")` | Linked name | `button0` |
| `Timer(freq)` | `timer_<freq>hz` | `timer_1000hz` |
| `Signal("name")` | Signal name | `irq_line` |
| `is_wake` (default `true`) | Additional `wake_<name>` output port | `wake_sensor` |

## Webstack Backend

> **⚠️ SUPERSEDED 2026-07-26** — This section describes the old TypeScript emitter
> (`src/backend/webstack.rs`). The webstack v2 architecture compiles to **WASM + JS shim**
> via the existing `LlvmBackend(wasm32)` and a new `GlueWebGenerator`. See
> `docs/architecture/features/rendered-briev-wasm.md` and
> `docs/plans/2026-07-26-rendered-briev-webstack-v2.md` for the current design.
>
> The content below is preserved for historical reference during the migration.

(`src/backend/webstack.rs`, 1,200 lines) — TypeScript emitter for the web target.
Replaced the Rust/wasm-bindgen codegen (Phase A, 2026-06-19). The archived Rust
codegen lives at `archive/backend/webstack_rust_codegen.rs` (1,758 lines).

### Architecture

```
WebstackGenerator
├── collect_signals_and_transactions()  — AST → signal map + txn map (shared with archived codegen)
├── generate_ts_code()                  — main TS emitter entry point
│   ├── ts_type_for_signal()            — SignalType → "number" / "string" / "boolean" / "any[]"
│   ├── ts_ident()                      — name normalization (hyphens → underscores)
│   ├── emit_ts_txn_body()              — transaction body → TS method body
│   │   └── statement_to_ts()           — Statement → TS code (Assignment, Let, Guarded, Term, etc.)
│   ├── expr_to_ts()                    — Expr → native TS expression (no JsValue boxing)
│   ├── ffi_ts_impl                    — frgn from "javascript" → inline TS methods on App class
│   └── js_glue via generate_js_glue() — view binding watchers (b-text, b-trigger, b-show, b-class, b-each)
└── generate_arm_rust_code()            — dead code path (placeholder body)
```

### TypeScript Output

Each `.rbv` file compiles to an `App` class with typed fields and async transaction methods:

```typescript
class App {
  count: number = 0;
  name: string = "";
  items: any[] = [];

  async increment(): Promise<void> {
    this.count = this.count + 1;
    return;
  }
}

export function createApp(): App {
  return new App();
}
```

### Key Differences from Old Rust/wasm-bindgen Codegen

| Aspect | Old (archived) | New (TS emitter) |
|--------|----------------|-------------------|
| Output language | Rust (`#[wasm_bindgen]`) | TypeScript |
| Signal storage | `Vec<JsValue>` (all boxing) | Typed fields (`number`, `string`, `boolean`) |
| Arithmetic | `JsValue::from(a + b)` → JS number | Direct `a + b` (native) |
| FFI calls | `js_sys::Reflect::get()` + dynamic dispatch | Inline TS implementation from `frgn from "javascript"` |
| Intrinsics | Matched on `Intrinsic` enum → Rust | Matched on `Intrinsic` enum → native TS (`console.log`, `String.fromCharCode`, etc.) |
| Toolchain | `wasm-pack` + `wasm-bindgen` | `tsc` (standard TypeScript compiler) |
| DOM bindings | Rust struct methods called from JS glue | Direct `app.` property/method calls from JS glue |
| Build dependencies | Rust + wasm-pack + wasm-bindgen | TypeScript only |

### Signal Type Mapping

| Briev type | TS type | Initializer |
|------------|---------|-------------|
| `Int` | `number` | `0` |
| `Float` | `number` | `0` |
| `Bool` | `boolean` | `false` |
| `String` | `string` | `""` |
| `List<T>` | `any[]` | `[]` |
| `Vector(N)` | `number[]` | `new Array(N).fill(0)` |

### frgn from "javascript" FFI

When a `.bv` file declares `frgn from "javascript"`, the `wasm_impl` and
`wasm_setup` fields (which contain JavaScript code) are registered in
`ffi_ts_impl` / `ffi_ts_setups`. The TS emitter inlines these as methods
on the `App` class:

```typescript
class App {
  myFn(arg0: any): any {
    // wasm_impl code inlined here
  }
}
```

The `wasm_impl` field name is historical (from when Rust/wasm-bindgen targeted WASM).
In the TS emitter, it serves as the JavaScript implementation code for any
`frgn` declaration — whether or not WASM is involved.

### View Binding JS Glue

The `generate_js_glue()` method emits DOM watchers that reference the `App` instance:

| Directive | Generated Code |
|-----------|---------------|
| `b-text="x"` | `el.textContent = String(app.x)` |
| `b-trigger:click="txn"` | `el.addEventListener('click', () => app.txn())` |
| `b-show="cond"` | `el.style.display = app.cond ? '' : 'none'` |
| `b-class="cond:cls"` | `el.classList.toggle('cls', Boolean(app.cond))` |

### Known Limitations

- **ArrowMut/ArrowDiscard/ArrowTransfer**: Emit as `splice` calls; filtered
  transfer falls through to a no-op comment
- **Block expressions**: Emit as IIFE `(() => { ...; return last; })()`
- **Pattern B feature dispatch**: Feature module `ExprCodegenWebstack` trait
  still uses Rust-codegen return values (`"JsValue::TRUE"`); TS emitter
  bypasses these by matching Expr variants directly in `expr_to_ts()`
- **ARM target**: `generate_arm_rust_code` emits Rust for bare-metal KV260
  (dead code path — not actively maintained)

## FFI Marshaling Convention (Critical)

The LLVM backend and C runtime must agree on how Briev types cross the FFI
boundary. The convention is documented at `briev_rt.c:376`:

> The LLVM backend marshals String as i8*, Int as i64, Bool as i64.

### Convention

| Briev type | C type | LLVM IR type | Notes |
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
%raw  = call ptr @briev_read_file(ptr %fp)
%data = ptrtoint ptr %raw to i64
```

This was the root cause of the `briev_read_file` bug (2026-06-14) — the intrinsic
was passing `i64` directly, and the C function interpreted raw characters as a
Briev header pointer. See `docs/architecture/fixes/briev-read-file-ffi-marshal.md`.

## i64 Boxing Convention (Phase 0, 2026-06-16)

The LLVM backend boxes native types to `i64` for a uniform internal ABI.
This avoids SSA type proliferation — every value slot is `i64` regardless
of the Briev-level type. The `TypedRegister.ty` field tracks the Briev type,
and `adapt_to_i64()` is the canonical function that produces a boxed `i64`
from any `TypedRegister`, regardless of its current native/boxed state.

### The Convention

| Briev type | Internal LLVM type | TypedRegister.ty | adapt_to_i64 |
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

| Briev type | C type | LLVM IR type | MarshaL |
|------------|--------|-------------|---------|
| `Int` | `int64_t` | `i64` | Direct |
| `Bool` | `int64_t` | `i32` | `trunc i64 %boxed to i32` at FFI call site |
| `Float` | `float` | `float` | `ensure_float_reg` |
| `String` | `const char*` | `ptr` (`i8*`) | `inttoptr i64 %boxed to ptr` |
| `Data` | `const char*` | `ptr` | Same as String |
| `Char` | `int64_t` | `i32` | `trunc i64 %boxed to i32` |

FFI calls at `emit_expr.rs:311-335` use the marshal table above.
Intrinsics that call C functions (ReadFile, Spawn) must apply the same
marshaling — this was the root cause of the `briev_read_file` bug (2026-06-14).

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
- `lib/runtime/briev_rt.c` — `__tty_read_key` global + epoll reads, `__print` flush

---

## Dispatch Architecture: GEP+Store vs SSA Insertvalue

The LLVM backend has two loop dispatch paths with a fundamental trade-off:

### Path A: GEP+Store (emit_ssa_main, "direct SSA loop")
- **What**: Loads each field from `%State` via `getelementptr` + `load` at tick entry.
  Writes via `getelementptr` + `store`. Each field is an independent memory operation.
- **When**: Default path for programs with guarded bodies, async triggers, or when
  the analysis can't prove purity.
- **Pros**: No wide SSA registers. LLVM's register allocator handles fields independently.
- **Cons**: LLVM can't track reductions through memory — `checksum` stores appear as
  opaque memory writes, blocking vectorization.
- **Performance**: Faster for benchmarks with 15+ fields (fannkuch_redux: 0.068s at
  BOUND=50M vs Path B's 0.100s).

### Path B: SSA Insertvalue (emit_folded_main, "folded SSA")
- **What**: Builds a single wide `%State` SSA register via `extractvalue` at tick start,
  `insertvalue` for each field update. The entire struct flows through phi nodes.
- **When**: Pure bodies without branching guards that the analysis determines are foldable.
- **Pros**: LLVM can track reductions in SSA form — enables reduction identification
  for vectorization.
- **Cons**: The wide `%State` SSA register (15+ fields) creates register pressure.
  LLVM must spill to memory when SROA can't decompose the struct.
- **Performance**: 47% slower for fannkuch_redux (0.100s vs 0.068s at BOUND=50M).

### Root Cause: SROA Blockage

Both paths could be equally fast if LLVM's **SROA** (Scalar Replacement of Aggregates)
pass decomposed the `%State` struct into scalar registers. SROA is blocked by:

```
%state = alloca %State, align 8
call void @init_state(%State* noalias nocapture %state)
```

The `alloca` address **escapes** to `@init_state`. Once a pointer escapes, SROA can't
prove it's not aliased and refuses to decompose the struct.

### The Fix: Inline init_state

If the `init_state` function body is inlined into `main()` before the `alloca`, the
`alloca` no longer escapes and SROA can decompose the struct. This requires:

1. Move `init_state`'s body from its standalone function into `main()` entry
2. Remove the `call void @init_state(%State*)` instruction
3. Replace with inline stores/insertvalue for each field's initial value

This makes both GEP+store and SSA insertvalue equally efficient, and enables LLVM
to identify reductions through either path.

### SSA Path B Bugfix: Intra-body Field Reads After Writes (2026-06-17)

The SSA extract-all-at-entry pattern (`pre_extract_int_fields`) caused a
correctness bug when a field is both written and read within the same
transaction body. Example from `fannkuch_redux_sym.bv`:

```briev
&seed = p0;                    // writes seed field
&checksum = checksum + seed % 13;  // reads seed — should see p0, not seed_old
```

Without the fix, `seed` in the second line reads the **pre-tick** `seed_old`
extracted at body entry, not the `p0` that was just written. **Fix**
(`emit_stmt.rs:317-328`): After each SSA-mode field write (`insertvalue`),
re-extract the field from the new state and update `ssa_old_int_regs` so
subsequent reads use the latest value.

### Auto-Linking briev_rt.c (2026-06-17)

`briev_rt.c` is now auto-linked for **all** native builds (not just programs
with `@ link` triggers). The linker drops unused symbols, so this is safe
even for programs that use no C shims. This unblocks officina-cli, which
uses `#`-intrinsics via the stdlib but has no `import "link/..."` statements.

### stdout Buffering Policy

```
call void @setvbuf(ptr @stdout, ptr null, i32 1, i64 0)
```

Line-buffered stdout (`_IOLBF = 1`). Only `println#` emits explicit `fflush(stdout)`.
`print_int#`, `print_float#`, and `putchar#` rely on automatic line-buffering to flush.
This is a single declarative policy at program startup — no per-intrinsic flush logic
to maintain.

## Known Limitations (2026-06-19)

The following are tracked but not yet implemented:

| Area | Issue | Priority |
|------|-------|----------|
| `PriorState` | `@var` fixed — now loads from committed SSA state | ✅ Fixed |
| Named addresses | `@FOO` now produces clear error instead of 0 | ✅ Fixed |
| `to_str`/`float_to_str` | LLVM panics fixed — uses `__int_to_str__`/`snprintf` | ✅ Fixed |
| Webstack `IntrinsicCall` | Replaced `unimplemented!()` panic with JS codegen | ✅ Fixed |
| Webstack `foreach`/`oracle` | Replaced silent no-op with implementations | ✅ Fixed |
| Webstack Rust codegen | Replaced with TypeScript emitter (Phase A, 2026-06-19) | ✅ Fixed |
| Pattern B dispatch | 13 `features/stmt/` files + 16 interpreter arms — stub | Deferred |
| `$!` macro expansion | Full engine required — `expand_macro_call` returns error | Deferred |
| `Private` field visibility | Parsed but silently not enforced in typechecker | Low |
| LLVM `add i64 0, 0` no-ops | 38 error-handling fallbacks across LLVM backend | Low |
| Webstack Arm codegen | `generate_arm_rust_code` uses placeholder body (no `&mut self`) | Low |

