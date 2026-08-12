# Backend Routing + Async/Await — Full Implementation Plan

**Date**: 2026-06-19
**Author**: Architecture Planning Session
**Status**: Ready for implementation

---

## Motivation

Briev has evolved from universal transpilation to a **backend-per-variant** architecture. Each file extension now targets a specific backend:

| Extension | Name | Backend | Role |
|-----------|------|---------|------|
| `.bv` | Briev | LLVM | General-purpose native |
| `.sbv` | Strict Briev | LLVM | Full contract verification |
| `.abv` | Accelerated Briev | SPIR-V via LLVM | GPU acceleration |
| `.rbv` | Rendered Briev | Webstack (WASM+JS) | Web frontend |
| `.srbv` | Strict Rendered Briev | Webstack | Verified view-state isomorphism |
| `.ebv` | Embedded Briev | LLVM (restricted mode) | Bare-metal embedded |
| `.sebv` | Strict Embedded Briev | LLVM (restricted mode) | Verified embedded |
| `.cbv` | Circuit Briev | CIRCT | Hardware (synthesizable) |
| `.dbv` / `.dbvs` / `.dbvl` | Data Briev | Config schema | Configuration/data |

The compiler currently has routing gaps:
- **`.ebv` errors out** in `run_build` requiring explicit `--target` — should default to restricted LLVM
- **`.cbv` is invisible** to `run_build` — falls through to "Unknown file extension"
- **Both `.ebv` and `.cbv` infer `verilog_fpga.toml`** in `run_compile_unified` — should infer LLVM and CIRCT respectively
- **No `CompilationTarget::Embedded` or `Circuit`** — typechecker can't do target-specific checks
- **9 dead backends** consume ~10,000 lines with zero maintenance
- **CIRCT backend is 239 lines** — only handles trigger ports, no transaction body emission
- **No `async`/`await` keywords** at statement level — only `async node` at top level
- **LLVM backend has no "embedded mode"** — no restricted validation, no ISR support, no freestanding linkage

## Design Decisions

1. **`term!` in embedded mode**: Halt CPU via `halt#` intrinsic (WFI on ARM, HLT on x86), not infinite loop. The user can also explicitly call `halt#()` for WFI/sleep.
2. **`async await let x = f()`**: Both discard (`async await f()`) and capture (`async await let x = f()`) forms supported. Barrier at `term` in both cases. Captured variable is available to subsequent statements.
3. **`await` on any callable**: defn, txn, or frgn — compiler infers async capability. For pure `defn`, it's a normal sequential call. For `txn`, it runs to convergence. For `frgn`, it depends on FFI declaration.
4. **`.ebv` strategy**: Both bare-metal default (freestanding LLVM flags) AND spec files via `.dbv` for complex device configurations.
5. **Dead backends**: Move to `archive/backend/` — preserved in repo but not compiled. Not deleted.
6. **Balance**: Async/await and backend routing work interleaved equally across phases.

---

## Phase 1: Foundation — Backend Routing & CompilationTarget

### Goal
Fix `.ebv` and `.cbv` dispatch so they compile to the correct backend without `--target`. Add `CompilationTarget` variants. Move dead backends. Add `halt#` intrinsic.

### 1.1 — Add `CompilationTarget::Embedded` and `CompilationTarget::Circuit`

**File**: `src/typechecker.rs:42`

Current enum:
```rust
pub enum CompilationTarget {
    Interpreter,
    Wasm,
    Verilog,
}
```

New:
```rust
pub enum CompilationTarget {
    Interpreter,
    Wasm,
    Verilog,
    Embedded,
    Circuit,
}
```

**Impact**: All `match target` arms in typechecker. The `Expr::Float` check at line 1437 (`if self.target == CompilationTarget::Verilog`) also errors for `Embedded` and `Circuit`. Update to:
```rust
if self.target == CompilationTarget::Verilog
    || self.target == CompilationTarget::Embedded
    || self.target == CompilationTarget::Circuit
```

**Tests**: Verify `CompilationTarget::Embedded` and `CompilationTarget::Circuit` are constructable and matchable. No logic changes elsewhere yet.

### 1.2 — Route `.ebv`/`.sebv` → LLVM with embedded flag

**File**: `src/main.rs:run_build` (lines 1061-1068)

Current: Returns error "requires explicit target".

New: Match `.ebv`/`.sebv` → call `run_llvm_compile(...)` with same args as `.bv` path. Inside `run_llvm_compile`, detect embedded extension via new `is_embedded_extension(file_path)` function and set `is_embedded = true` on `LlvmBackend`.

**Files to modify**:
- `src/main.rs` — add `is_embedded_extension()` helper, change `.ebv` arm in `run_build`
- `src/main.rs` — in `run_llvm_compile`, detect embedded mode and propagate to backend
- `src/backend/llvm/mod.rs` — add `is_embedded: bool` field and builder method

**Validation**: `briev build foo.ebv` should compile through LLVM (not error). The `is_embedded` flag causes restricted-mode checks in Phase 3. For now, it's a silent flag.

### 1.3 — Route `.cbv` → CIRCT via new `run_cbv()`

**File**: `src/main.rs:run_build` (line 1069 catch-all)

Current: `.cbv` falls through to "Unknown file extension".

New: Add `.cbv` match arm → call new `run_cbv(file_path, ...)` function.

**`run_cbv` function** (new, ~200 lines):
```
1. Read source, strip annotations
2. Parse with strict mode (is_strict_extension returns true for .cbv)
3. Resolve imports
4. Synthesize builtin types + init txn
5. Desugar
6. Template/macro expansion
7. Typecheck with CompilationTarget::Circuit
8. Run shared analysis
9. Run equality saturation simplify (if --prod)
10. Call CIRCT backend: circt::generate(&program)
11. Write .mlir output file
12. Optionally run circt-opt | circt-translate --export-verilog pipeline
```

**Validation**: `briev build foo.cbv` should parse and emit `.mlir` via CIRCT backend. If CIRCT tools are not installed, print warning and emit raw `.mlir`.

### 1.4 — Move 9 dead backends to `archive/`

**Files to move** (9 total):
| Source | Destination |
|--------|-------------|
| `src/backend/verilog.rs` | `archive/backend/verilog.rs` |
| `src/backend/vhdl.rs` | `archive/backend/vhdl.rs` |
| `src/backend/c.rs` | `archive/backend/c.rs` |
| `src/backend/rust.rs` | `archive/backend/rust.rs` |
| `src/backend/x86_64.rs` | `archive/backend/x86_64.rs` |
| `src/backend/aarch64.rs` | `archive/backend/aarch64.rs` |
| `src/backend/wasm.rs` | `archive/backend/wasm.rs` |
| `src/backend/cobol.rs` | `archive/backend/cobol.rs` |
| `src/backend/tcl_generator.rs` | `archive/backend/tcl_generator.rs` |

**File to modify**: `src/backend/mod.rs`
- Remove `pub mod` lines for all 9 backends
- Remove from `supported_hashtags()` match arms
- Remove tests that reference dead backends (test_c_backend_supports_volatile, etc. at lines 832-943)

**Validation**: `cargo build` succeeds without dead backend modules. `cargo test --lib` passes.

### 1.5 — Update `run_compile_unified` target inference

**File**: `src/main.rs:run_compile_unified` (lines 1652-1700)

Current:
```rust
"embedded" => "verilog_fpga.toml",  // wrong for both .ebv and .cbv
```

New: Split `.ebv`/`.sebv` from `.cbv`:
```rust
// Detect specific source subtype
let source_subtype = if ext == "cbv" { "circuit" } else { source_type };
// ...
"embedded" => "llvm.toml",
"circuit" => "circt.toml",
```

Also update capability validation (lines 1706-1729):
- `.ebv` requires `hardware_triggers` (MMIO/interrupts) — already checked
- `.cbv` does NOT require `hardware_triggers` — it's pure CIRCT. Instead, `.cbv` requires `circt_synthesis` capability (or no check — CIRCT is the only option)

**Validation**: `briev compile foo.ebv` infers `llvm.toml`. `briev compile foo.cbv` infers `circt.toml`.

### 1.6 — Update `hardware_validator` to accept `CompilationTarget`

**File**: `src/hardware_validator.rs`

Current signature:
```rust
pub fn validate(
    program: &Program,
    hw_config: Option<&HardwareConfig>,
    _target: &str,
    is_ebv: bool,
    target_spec: Option<&TargetSpec>,
    dbvs_engine: Option<&DbvsEngine>,
) -> Vec<Diagnostic>
```

New signature:
```rust
pub fn validate(
    program: &Program,
    hw_config: Option<&HardwareConfig>,
    _target: &str,
    target: CompilationTarget,
    target_spec: Option<&TargetSpec>,
    dbvs_engine: Option<&DbvsEngine>,
) -> Vec<Diagnostic>
```

Impact:
- `check_orphan_variables(..., is_ebv)` → `check_orphan_variables(..., target == CompilationTarget::Embedded || target == CompilationTarget::Circuit)`
- `check_hebv_restrictions()` only runs for `CompilationTarget::Circuit` (was `is_ebv`)
- New: `check_ebv_restrictions()` for `CompilationTarget::Embedded` (no `String`, no `Float`, no dynamic allocation — moved from Phase 3 as a preliminary check)

Update all call sites in `main.rs`:
- `src/main.rs:2757` (run_verilog_compile)
- `src/main.rs:2893` (run_vhdl_compile)
- `src/main.rs:3170` (hardware validation in compile path)

**Validation**: `briev build foo.ebv` does NOT run `B500x` checks. `briev build foo.cbv` DOES run `B500x` checks. All tests pass.

### 1.7 — Add `halt#` intrinsic

**File**: `src/ast.rs`

Add to `Intrinsic` enum (after D14 Debugging section, after `Backtrace`):
```rust
// ===== D14b: CPU Halt (2026-06-19) =====
/// halt#() -> Void — halt CPU (WFI on ARM, HLT on x86)
Halt,
```

**Files to modify**:

| File | Change |
|------|--------|
| `src/ast.rs` — `has_side_effects()` | `Intrinsic::Halt => true` |
| `src/ast.rs` — `from_name()` | `"halt" => Some(Intrinsic::Halt)` |
| `src/ast.rs` — `name()` | `Intrinsic::Halt => "halt"` |
| `src/interpreter.rs` | `Intrinsic::Halt => { Ok(Value::Void) }` — no-op in interpreter (no actual halt) |
| `src/backend/llvm/emit_expr.rs` | `Intrinsic::Halt => { writeln!(out, "call void asm sideeffect \"wfi\", \"\"") }` — target-aware: ARM → `wfi`, x86 → `hlt`, RISC-V → `wfi` |

**Validation**: `halt#()` intrinsic is recognized, evaluated in interpreter (no-op), and emits target-specific halt instruction in LLVM backend.

---

## Phase 2: Async/Await — Lexer, AST, Parser, Interpreter

### Goal
Add `await`, `async`, and `async await` as statement-level control flow modifiers usable inside transaction and definition bodies.

### 2.1 — `Token::Await` in Lexer

**File**: `src/lexer.rs`

Add new token:
```rust
#[token("await")]
#[token("AWAIT")]
Await,
```

Place after existing `Token::Async` (which already exists for `async node`).

### 2.2 — New Statement Variants in AST

**File**: `src/ast.rs` — `Statement` enum (around line 1617)

Add three new variants:
```rust
/// Await: await call_expr; — blocking wait for a callable result
Await {
    expr: Expr,
    modifiers: Vec<Hashtag>,
},

/// Async: async { body } or async stmt; — fire-and-forget
Async {
    body: Box<Statement>,
    modifiers: Vec<Hashtag>,
},

/// AsyncAwait: async await expr; or async await let x = expr;
/// Fork-join: launches immediately, barriers at term.
/// lhs: Some(name) if "async await let x = expr;" form
AsyncAwait {
    body: Box<Statement>,
    lhs: Option<String>,
    modifiers: Vec<Hashtag>,
},
```

### 2.3 — Parser: `await expr;`

**File**: `src/parser.rs`

Add parsing in `parse_statement()`:
```rust
Token::Await => {
    self.advance();
    let expr = self.parse_expression()?;
    self.expect(Token::Semicolon)?;
    let modifiers = self.parse_hashtag_modifiers()?;
    Ok(Statement::Await { expr, modifiers })
}
```

### 2.4 — Parser: `async stmt;` and `async { body }`

**File**: `src/parser.rs`

Disambiguate from `async node`:
- `Token::Async` followed by `Token::Rct` or `Token::Txn` → top-level async txn (existing behavior)
- `Token::Async` followed by anything else → `Statement::Async`
- `Token::Async` followed by `Token::Await` → `Statement::AsyncAwait` (handled by 2.6)

```rust
Token::Async => {
    self.advance();
    // Check if followed by await -> async await
    if let Some(Ok(Token::Await)) = self.current_token() {
        // Delegate to async-await parser (2.6)
        return self.parse_async_await();
    }
    // Check if followed by rct/txn -> top-level async txn
    if let Some(Ok(Token::Rct)) | Some(Ok(Token::Txn)) = self.current_token() {
        return Err(SyntaxError::new("'async' at statement level must be followed by a statement or block; use 'async node' at top level"));
    }
    // Otherwise: async stmt or async { body }
    let body = Box::new(self.parse_statement()?);
    let modifiers = self.parse_hashtag_modifiers()?;
    Ok(Statement::Async { body, modifiers })
}
```

### 2.5 — Parser: `async await expr;` and `async await let x = expr;`

**File**: `src/parser.rs`

New helper `parse_async_await()`:
```rust
fn parse_async_await(&mut self) -> Result<Statement, SyntaxError> {
    // Already consumed Token::Async, now on Token::Await
    self.advance(); // consume await
    
    // Optional: "let x = "
    let lhs = if let Some(Ok(Token::Let)) = self.current_token() {
        self.advance();
        let name = self.parse_identifier()?;
        self.expect(Token::Eq)?;
        Some(name)
    } else {
        None
    };
    
    // The rest is a statement or block
    let body = Box::new(self.parse_statement()?);
    let modifiers = self.parse_hashtag_modifiers()?;
    Ok(Statement::AsyncAwait { body, lhs, modifiers })
}
```

### 2.6 — Interpreter: `Await`

**File**: `src/interpreter.rs` — `exec_stmt()`

```rust
Statement::Await { expr, modifiers: _ } => {
    let value = self.eval_expr(expr)?;
    // In the interpreter, await is sequential:
    // evaluate the callable, get the result
    self.return_value = Some(value);
}
```

### 2.7 — Interpreter: `Async`

```rust
Statement::Async { body, modifiers: _ } => {
    self.exec_stmt(body)?;
    // Discard the return — fire-and-forget
    // In interpreter, this is still sequential
}
```

### 2.8 — Interpreter: `AsyncAwait`

```rust
Statement::AsyncAwait { body, lhs, modifiers: _ } => {
    let result = {
        // Evaluate body, capture result
        self.eval_statement_result(body)?
    };
    if let Some(name) = lhs {
        self.set_variable(name, result);
    }
    // Barrier managed via pending_barriers
    self.pending_barriers.push(...);
}
```

Add `pending_barriers: Vec<Value>` field to `Interpreter` struct. On `Statement::Term`, wait for all pending barriers:
```rust
if !self.pending_barriers.is_empty() {
    // All barriers must be resolved
    self.pending_barriers.clear();
}
```

### 2.9 — Typechecker Support

**File**: `src/typechecker.rs`

- `Await { expr }`: The `expr` must be callable (resolves to a `defn`, `txn`, or `frgn`). The return type of the callable becomes the type of the await expression.
- `Async { body }`: Body can return anything or nothing. Return value is discarded.
- `AsyncAwait { body, lhs }`: If `lhs: Some(name)`, the call's return type must match `name`'s declared type. The captured variable is available to subsequent statements.

New helper `check_async_await_callable(expr)` validates that the expression is a callable.

### 2.10 — Proof Engine Updates

**File**: `src/proof_engine.rs`

- `await` call inside a txn means the txn POSTCONDITION depends on the awaited call's result. Update `check_mutual_exclusion`.
- `async` call inside a txn means the POSTCONDITION does NOT depend on the async call's result. Mark the txn as non-blocking for the async call.
- `async await` call means the result is needed but not immediately. Update `suggest_async_promotion` to account for barrier-at-term semantics.

### 2.11 — Tests

| Test | What it verifies |
|------|------------------|
| `test_parse_await_expr` | `await compute(x);` parses to `Statement::Await` |
| `test_parse_async_expr` | `async compute(x);` parses to `Statement::Async` |
| `test_parse_async_await_expr` | `async await compute(x);` parses to `Statement::AsyncAwait` with `lhs: None` |
| `test_parse_async_await_let` | `async await let result = compute(x);` parses with `lhs: Some("result")` |
| `test_parse_async_block` | `async { stmt1; stmt2; }` parses to block form |
| `test_interp_await` | `await` evaluates callable and captures result |
| `test_interp_async` | `async` evaluates body but discards return |
| `test_interp_async_await` | `async await` captures result AND blocks term |
| `test_typecheck_await_non_callable` | `await 42;` — type error (not callable) |
| `test_typecheck_async_await_let_type` | Mismatched `let` type vs callable return → type error |

---

## Phase 3: Embedded LLVM Mode (.ebv)

### Goal
LLVM backend gains a restricted "embedded" mode with hard errors for dynamic allocation, threading, unbounded recursion, and ISR annotation support.

### 3.1 — Add `EmbeddedConfig` to `LlvmBackend`

**File**: `src/backend/llvm/mod.rs`

New struct:
```rust
#[derive(Debug, Clone)]
pub struct EmbeddedConfig {
    pub target_triple: String,          // e.g. "armv7em-none-eabi"
    pub linker_script: Option<String>,  // path to .ld file
    pub freestanding: bool,             // default true
    pub halt_on_term: bool,             // default true (emit halt# instead of ret)
}
```

Add to `LlvmBackend`:
```rust
pub struct LlvmBackend {
    // ... existing fields ...
    pub(crate) is_embedded: bool,
    pub(crate) embedded_config: Option<EmbeddedConfig>,
}
```

Builder method:
```rust
pub fn with_embedded_mode(mut self, enabled: bool) -> Self {
    self.is_embedded = enabled;
    if enabled {
        self.embedded_config = Some(EmbeddedConfig {
            target_triple: "generic".to_string(),
            linker_script: None,
            freestanding: true,
            halt_on_term: true,
        });
    }
    self
}
```

### 3.2 — Pass `is_embedded` through compilation pipeline

**File**: `src/main.rs`

In `run_llvm_compile`, after creating `LlvmBackend`:
```rust
let is_embedded = is_embedded_extension(file_path);
// ...
let mut llvm_backend = crate::backend::llvm::LlvmBackend::new()
    // ... existing builder chain ...
    .with_embedded_mode(is_embedded);
```

### 3.3 — Reject dynamic allocation in embedded mode

**File**: `src/backend/llvm/mod.rs` or new analysis pass

New function `check_embedded_restrictions(program)` called in `LlvmBackend::generate()` when `is_embedded` is true:
- Scan typed AST for: `Box`, `Vec`, `String`, `HashMap`, `HashSet` constructors (both expression and statement forms)
- Scan for `List::push` (if unbounded), `append`, `concat` (if unbounded)
- Scan for `frgn` that reference heap
- Error: `"TargetError: Dynamic allocation not supported on target 'Embedded'"`

Strategy: Walk all `TopLevel::Transaction`, `TopLevel::Definition`, `TopLevel::StateDecl` items. For each:
- Check `Type` for heap-allocated types (`List<T>`, `String`, `HashMap<K,V>`, `HashSet<V>`)
- Check `Expr` for `ListLiteral`, `MapLiteral`, `SetLiteral`, `String` that exceed a configurable capacity
- Check `Statement::Let` with `String` / `List` type

### 3.4 — Reject threading intrinsics in embedded mode

**File**: `src/backend/llvm/emit_expr.rs`

In embedded mode, match `Intrinsic::ThreadCreate` etc. and emit an error instead of code:
```rust
if self.is_embedded {
    panic!("TargetError: threading intrinsics not supported on target 'Embedded'");
}
```

Intrinsics to reject:
- `ThreadCreate`, `ThreadJoin`, `ThreadExit`
- `MutexLock`, `MutexUnlock`
- `CondvarWait`, `CondvarSignal`, `CondvarBroadcast`
- `Futex`
- `AtomicLoad`, `AtomicStore` (unless the target triple explicitly supports them)

### 3.5 — Reject unbounded recursion in embedded mode

**File**: `src/analysis/` — new `recursion_depth.rs` analysis

Add static recursion-depth analysis. For each `defn`, compute max call depth:
- If `defn A` calls `defn B` and `defn B` calls `defn A` (direct or mutual recursion): error unless provably bounded by precondition `[depth < MAX]`
- If a `defn` calls itself recursively more than `MAX_RECURSION_DEPTH` (configurable, default 10): warning, but allow

This can reuse the existing `CallGraph` from `src/analysis/call_graph.rs`.

### 3.6 — ISR annotation for `trg` declarations

**File**: `src/parser.rs` + `src/backend/llvm/emit_toplevel.rs`

Parser: Support `#[interrupt(VECTOR_NAME)]` attribute on `trg`:
```briev
#[interrupt(TIM2)]
trg timer: Bool @ linked("__timer_isr");
```

LLVM Backend: When `is_embedded` and `#[interrupt]` is present:
- Emit an ISR wrapper function with `__attribute__((interrupt))` calling convention
- Auto-insert save/restore of callee-saved registers
- The ISR wrapper calls the reactive handler function
- Place the function at the appropriate vector table entry (from `.dbv` spec or explicit `VECTOR_NAME`)

Implementation:
```llvm
define void @isr_TIM2() #0 {
    ; Save context, call handler
    call void @async_body_timer(%State* @__state)
    ret void
}
attributes #0 = { interrupt }
```

### 3.7 — Embedded `term!` → `halt#()` instead of `ret`

**File**: `src/backend/llvm/emit_stmt.rs`

When `is_embedded` and `halt_on_term` is true:
- `Statement::TermBang` emits `call void @llvm_briev_halt()` instead of `br label %done`
- `term` (non-bang) emits normally (commit action, no halt — the txn continues)

The `halt#()` intrinsic emits:
```llvm
; ARM
call void asm sideeffect "wfi", ""()
; x86
call void asm sideeffect "hlt", ""()
; RISC-V
call void asm sideeffect "wfi", ""()
```

### 3.8 — Freestanding linker integration

**File**: `src/main.rs` — link step (lines 2560-2690)

When `is_embedded`:
- Add `-ffreestanding -nostdlib -nostartfiles -T<linker_script>` to the link command
- Skip `-lpthread` (no threading)
- Skip `-lrt` (no real-time signals)
- If no linker script specified, use a default minimal layout:
  - `.text` at `0x08000000` (typical STM32 flash)
  - `.data` at `0x20000000` (typical STM32 SRAM)
  - `.bss` after `.data`

### 3.9 — Volatile MMIO hardening audit

**File**: `src/backend/llvm/emit_expr.rs` + `emit_toplevel.rs`

Verify ALL `@` address accesses use `load volatile`/`store volatile`:
- `emit_inline_init_stores()` for `@` state variables
- `emit_state_load()` for `@` state reads
- `emit_state_store()` for `@` state writes
- Trigger declarations at `@` addresses

Non-MMIO state continues using normal loads/stores (no volatile).

### 3.10 — Tests

| Test | What it verifies |
|------|------------------|
| `test_embedded_no_dynamic_alloc` | `String s = "hello"` in .ebv → compile error |
| `test_embedded_no_threads` | `thread_create#(...)` in .ebv → compile error |
| `test_embedded_isr_wrapper` | LLVM IR contains `define void @isr_*() #0 { ... attributes #0 = { interrupt }` |
| `test_embedded_term_halt` | `term!;` in .ebv emits `asm "wfi"` not `ret` |
| `test_embedded_freestanding_link` | Link command includes `-ffreestanding -nostdlib` |
| `test_mmio_volatile_load_store` | `@` variable reads emit `load volatile`, writes emit `store volatile` |

---

## Phase 4: CIRCT Backend Full Implementation (.cbv)

### Goal
Transform the 239-line stub into a real hardware compiler that emits `hw` + `comb` + `seq` CIRCT dialect MLIR, with transaction body emission, FSM control flow, contract guards, and async/await.

### 4.1 — Sequential registers for state variables

**File**: `src/backend/circt.rs`

State variables become `seq` dialect registers:
```mlir
%reg = seq.firreg initial_value { init_value = 0 : i64 } : i64
```

Each register is clocked on `posedge clock`. The clock and reset signals are module inputs:
```mlir
hw.module @top(in %clock : i1, in %reset : i1, ...) {
    // Register declarations with clock enable
}
```

### 4.2 — Combinational expression codegen

Map `Expr` variants to CIRCT `comb` dialect ops:

| Briev Expr | CIRCT Op |
|------------|----------|
| `a + b` | `comb.add %a, %b` |
| `a - b` | `comb.sub %a, %b` |
| `a * b` | `comb.mul %a, %b` |
| `a / b` | `comb.divu %a, %b` |
| `a < b` | `comb.icmp ult %a, %b` |
| `a == b` | `comb.icmp eq %a, %b` |
| `a && b` | `comb.and %a, %b` |
| `!a` | `comb.xor %a, %true` |
| `a \| b` | `comb.or %a, %b` |
| `cond ? a : b` | `comb.mux %cond, %a, %b` |
| `a as UInt[32]` | `comb.extract` or bitcast |
| `a .#Size` | `comb.extract` (top bit) |

### 4.3 — Transaction body → FSM with `cf` dialect

Each `node` body becomes a finite state machine using `scf.while` or `cf` branch blocks:
```mlir
// FSM for a transaction with precondition [done < N] and postcondition [done == N]
%running = hw.wire : i1
%done_signal = hw.wire : i1

// Precondition check
%pre_cond = comb.icmp ult %done, %N : i64
hw.wire assign %running = %pre_cond

// Body (comb)
%done_next = comb.add %done, %i64_1 : i64

// Postcondition check
%post_cond = comb.icmp eq %done_next, %N : i64
hw.wire assign %done_signal = %post_cond

// Register update (sequ)
seq.always(posedge %clock) {
    seq.firreg(%done_next, %running) ...
}
```

Pattern: `scf.while` wrapping the txn body:
```mlir
%result = scf.while (%running = %pre_init) : (i1) -> i1 {
    scf.condition(%running)
} do {
^bb0(%running: i1):
    // Body
    scf.yield %next_running
}
```

### 4.4 — Contract guards as hardware

`[x < N]` precondition: `comb.icmp ult %x, %N` → feeds the FSM enable/start signal

`[x == N]` postcondition: `comb.icmp eq %x, %N` → commits the transaction result

For multiple preconditions `[a && b]`:
```mlir
%pre_a = comb.icmp ult %x, %N : i64
%pre_b = comb.icmp ne %y, %zero : i64
%pre = comb.and %pre_a, %pre_b : i1
```

### 4.5 — `trg` → input ports (extend existing)

Current CIRCT stub maps triggers to input ports. Extend to:
- `trg @ 0xADDR` → external input port with appropriate bit width
- `trg @ linked("name")` → input port named `name`
- `trg @ 1khz` → input port `timer_1khz` (expects periodic pulse from testbench)
- `#wake` modifier → separate output port `wake_<name>` per trigger

### 4.6 — `@ MMIO` → external ports

State variables with `@` address become `hw.module` external ports:
```mlir
hw.module @top(in %clock : i1, in %reset : i1, in %mmio_status : i32, out %mmio_control : i32) {
    // Direct I/O, not registers
    hw.output %mmio_control, %mmio_status
}
```

### 4.7 — `await` → FSM stall

When `await sub_module(args)` is encountered:
- Instantiate a sub-module with Start/Done handshake protocol
- FSM enters a stall state: continuous check of `sub_done` every cycle
- Output: `sub_start = 1` on entry, `sub_start = 0` after

```mlir
// Sub-module instantiation
%sub_done, %sub_result = hw.instance "sub" @sub_module(start: %sub_start : i1, ...)
// FSM stall
%stall = comb.and %running, comb.not %sub_done : i1
```

### 4.8 — `async` → FSM continue

Sub-module `start` asserted, FSM moves to next state in same cycle:
```mlir
// Fire and forget
%sub_start = hw.wire
hw.wire assign %sub_start = %fire_async  // valid for one cycle
// FSM does not check %sub_done
```

### 4.9 — `async await` → FSM fork-join

Sub-module `start` asserted, FSM continues executing other logic:
```mlir
// Fork
%sub_start = hw.wire
hw.wire assign %sub_start = %fire_async

// ... other FSM states execute in parallel ...

// Join (at commit/term)
%join_ready = comb.and %all_other_done, %sub_done : i1
```

If `async await let x = sub(...)`:
```mlir
%x_reg = seq.firreg clock(%clock) reset(%reset) : i64
// x_reg captures sub_result when sub_done fires
```

### 4.10 — `sync { }` → parallel paths

Each statement in a sync block gets independent `hw.wire` path. All paths execute combinatorially in parallel.
```mlir
// Parallel paths
%path1_result = comb.add %a, %b : i64
%path2_result = comb.sub %c, %d : i64

// Done when all paths stable (combinatorial, always done in one cycle)
%sync_done = hw.constant true
```

### 4.11 — `term!` → halt clock / idle

In `.cbv`, `term!` drives module output `halt` signal:
```mlir
hw.module @top(... out %halt : i1) {
    %halt_sig = hw.wire
    hw.wire assign %halt_sig = %program_done
    hw.output %halt_sig
}
```

In FPGA synthesis, `halt` connects to `global_enable = 0` or clock gate.

### 4.12 — Pipeline invocation

`run_cbv` writes `.mlir` file, then runs:
```bash
circt-opt --lower-seq-to-sv --lower-comb-to-sv program.mlir | \
    circt-translate --export-verilog > program.sv
```

If CIRCT tools not installed:
```bash
circt-opt program.mlir > /dev/null 2>&1 || \
    echo "Warning: circt-opt not found. Raw MLIR emitted."
```

### 4.13 — `CompilationTarget::Circuit` in typechecker

Additional Circuit-specific type checks:
- `Type::Int` / `Type::UInt` → error (must use `UInt[N]` / `SInt[N]` — sized types)
- `Type::Float` → error (not synthesizable)
- `Type::String` → error (not synthesizable)
- `frgn` → error (no external dependencies)
- `import "link/..."` → error (no external dependencies)
- `Struct` with pointer fields → error (no heap)

### 4.14 — Tests

| Test | What it verifies |
|------|------------------|
| `test_circt_empty_program` | Empty .cbv → MLIR with `hw.module @top` and clock/reset ports |
| `test_circt_state_register` | State variable → `seq.firreg` declaration |
| `test_circt_combinational_add` | `let x = a + b` → `comb.add %a, %b` |
| `test_circt_txn_fsm` | `node [x < N][x == N] { ... }` → FSM pattern with `scf.while` |
| `test_circt_precondition_guard` | `[x < N]` → `comb.icmp ult` |
| `test_circt_trg_input_port` | `trg btn: Bool @ 0x8000` → input port `btn` |
| `test_circt_await_stall` | `await compute(x)` → FSM stall pattern |
| `test_circt_sync_parallel` | `sync { a; b; }` → parallel wire paths |
| `test_circt_term_halt` | `term!` → `halt` output port |

---

## Phase 5: Async/Await Backend Codegen

### Goal
Wire `Statement::Await`/`Async`/`AsyncAwait` through all three active backends (LLVM, CIRCT, Webstack).

### 5.1 — LLVM: `await call(args)`

**File**: `src/backend/llvm/emit_stmt.rs`

```rust
Statement::Await { expr, .. } => {
    let result = self.emit_expr(out, &expr, indent);
    // Await is sequential in LLVM — just use the result directly
    if let Some(name) = result.reg_name {
        // Result is available in SSA register
    }
}
```

### 5.2 — LLVM: `async call(args)` — fire-and-forget

```rust
Statement::Async { body, .. } => {
    let result = self.emit_stmt(out, &body, indent);
    // Discard result — fire-and-forget
    // In a thread pool context, emit as thread-pool task:
    if self.has_async_txns {
        // Emit via emit_async_body
    }
}
```

### 5.3 — LLVM: `async await call(args)` — fork-join

```rust
Statement::AsyncAwait { body, lhs, .. } => {
    let result = self.emit_stmt(out, &body, indent);
    if let Some(name) = lhs {
        // Store in alloca for the captured variable
    }
    // Register barrier: at term, wait for all async_await calls
    self.pending_async_await_count += 1;
}
```

At the `term` boundary in `emit_loop` or `emit_term`:
```rust
// Barrier: wait for all async-await calls
if self.pending_async_await_count > 0 {
    writeln!(out, "  call void @__barrier_wait__()")?;
}
```

### 5.4 — LLVM: `async await let x = call(args)` — capture

Same as 5.3, but result is stored in `x`'s SSA register. The value is available after the barrier at term.

### 5.5 — CIRCT: `await call(args)` — FSM stall

(Already described in 4.7 — implement in `emit_stmt.rs` for CIRCT)

### 5.6 — CIRCT: `async call(args)` — FSM continue

(Already described in 4.8)

### 5.7 — CIRCT: `async await` — FSM fork-join

(Already described in 4.9)

### 5.8 — Webstack: `await call(args)`

**File**: `src/backend/webstack.rs`

```rust
Statement::Await { expr, .. } => {
    // Emit: let result = await __wasm_bindgen::call(args);
    writeln!(out, "let result = await {};", self.emit_call(expr))?;
}
```

### 5.9 — Webstack: `async call(args)`

```rust
Statement::Async { body, .. } => {
    // Emit: __wasm_bindgen::call(args); — no await
    writeln!(out, "{};", self.emit_call(body))?;
    // Fire-and-forget
}
```

### 5.10 — Webstack: `async await`

```rust
Statement::AsyncAwait { body, lhs, .. } => {
    // Emit: let promise = __wasm_bindgen::call(args);
    let promise_var = format!("__promise_{}", self.promise_counter);
    self.promise_counter += 1;
    writeln!(out, "let {} = {};", promise_var, self.emit_call(body))?;
    self.pending_promises.push(PendingPromise {
        var: promise_var,
        capture: lhs.clone(),
    });
}
```

At transaction boundary:
```rust
// Joint barrier for all pending promises
for promise in &self.pending_promises {
    if let Some(name) = &promise.capture {
        writeln!(out, "let {} = await {};", name, promise.var)?;
    } else {
        writeln!(out, "await {};", promise.var)?;
    }
}
```

### 5.11 — Tests

| Test | What it verifies |
|------|------------------|
| `test_llvm_await_seq` | LLVM IR for `await f(x)` emits call + use result |
| `test_llvm_async_fire` | LLVM IR for `async f(x)` emits call, no result used |
| `test_llvm_async_await_barrier` | LLVM IR for `async await f(x)` has barrier call before ret |
| `test_circt_await_fsm_stall` | CIRCT MLIR has stall state for await |
| `test_circt_async_fsm_fire` | CIRCT MLIR fires and continues |
| `test_webstack_await_js` | WASM JS glue has `await` keyword |
| `test_webstack_async_fire` | WASM JS glue omits `await` |
| `test_webstack_async_await_promise` | WASM JS glue has `Promise.all`-style barrier |

---

## Phase 6: `.dbv` Spec File Integration for `.ebv`

### Goal
MCU configurations via `.dbv` spec files — linker scripts, register maps, interrupt tables — enabling the "spec files for complex devices" part of the `.ebv` strategy.

### 6.1 — Define `.dbv` schema for MCU targets

**File**: `lib/targets/mcu.dbvs`

```dbvs
schema mcu_target {
    target_triple: String = "armv7em-none-eabi",  // LLVM triple
    linker_script: String?,                         // path to .ld file
    cpu: String?;                                   // e.g. "cortex-m4"

    memory_regions: List<MemoryRegion>;
    struct MemoryRegion {
        name: String;
        base: UInt[64];
        size: UInt[64];
        kind: String;  // "flash" | "sram" | "peripheral"
    }

    interrupts: List<InterruptEntry>;
    struct InterruptEntry {
        name: String;       // e.g. "TIM2"
        vector: UInt[32];   // vector table index
        trg_name: String?;  // matches #[interrupt(NAME)] in .ebv
    }

    peripherals: List<Peripheral>;
    struct Peripheral {
        name: String;
        base_addr: UInt[64];
        registers: List<Register>;
    }
    struct Register {
        name: String;
        offset: UInt[32];
        size: UInt[8];     // 8, 16, 32
        access: String;    // "rw" | "ro" | "wo"
    }
}
```

Example `.dbv` for STM32F407:
```dbv
import "mcu.dbvs";

target_triple = "armv7em-none-eabi";
cpu = "cortex-m4";
linker_script = "lib/targets/stm32f407.ld";

memory_regions = [
    { name: "FLASH", base: 0x08000000, size: 0x00100000, kind: "flash" },
    { name: "SRAM",  base: 0x20000000, size: 0x00020000, kind: "sram" },
];

interrupts = [
    { name: "TIM2",  vector: 44,  trg_name: "timer" },
    { name: "USART1", vector: 51, trg_name: "serial_rx" },
];

peripherals = [
    {
        name: "USART1",
        base_addr: 0x40011000,
        registers: [
            { name: "SR",   offset: 0x00, size: 16, access: "rw" },
            { name: "DR",   offset: 0x04, size: 16, access: "rw" },
            { name: "BRR",  offset: 0x08, size: 16, access: "rw" },
        ]
    },
];
```

### 6.2 — Auto-discover `.dbv` for `.ebv` compilation

**File**: `src/main.rs` — in `run_llvm_compile`, when `is_embedded` is true:

Scan `lib/targets/` for matching `.dbv` files. Use `--target <name>` or `--target-dbv <path>`:
```rust
if is_embedded {
    let target_name = embedded_config.target_triple.clone();
    // Search lib/targets/ for matching .dbv
    // If not found, use bare-metal defaults
}
```

### 6.3 — Register map → `@ REG_NAME` resolution

Import resolver loads `.dbv`, pre-populates `mmio_fields`. Source code can write:
```briev
let usart1_sr: UInt @ USART1_SR;
```

Compiler resolves `USART1_SR` to `0x40011000 + 0x00 = 0x40011000` from `.dbv` peripheral register map.

### 6.4 — Interrupt vector table generation

From `.dbv` interrupt entries + `#[interrupt(TIM2)]` annotations:
- Auto-generate `.section .vectors` in the output
- Each ISR entry point is the `trg` handler wrapper (from 3.6)
- Vector table placed at the correct memory address (from `memory_regions.flash.base`)

Generated LLVM IR:
```llvm
@__vectors = global [N x ptr] {
    [N x ptr] zeroinitializer,
    ptr @isr_TIM2,   ; vector 44
    ptr @isr_USART1, ; vector 51
    ...
}, section ".vectors" align 4
```

### 6.5 — Memory bank validation

Each `@ ADDRESS` access checked against `.dbv` memory regions:
```rust
fn validate_memory_access(addr: u64, regions: &[MemoryRegion]) -> Result<(), String> {
    if regions.iter().any(|r| addr >= r.base && addr < r.base + r.size) {
        Ok(())
    } else {
        Err(format!("Address 0x{:08X} outside declared memory regions", addr))
    }
}
```

### 6.6 — Tests

| Test | What it verifies |
|------|------------------|
| `test_ebv_dbv_register_resolution` | `.ebv` with `@ REG_NAME` resolves to correct address |
| `test_ebv_dbv_vector_table` | `.ebv` with `#[interrupt]` generates vector table entries |
| `test_ebv_dbv_memory_validation` | Out-of-bounds MMIO access → compile error |
| `test_ebv_dbv_linker_script` | Link command includes `.ld` file from `.dbv` |
| `test_ebv_no_dbv_fallback` | `.ebv` without `--target` compiles with bare-metal defaults |

---

## Dependency Graph

```
Phase 1 ─────────────────────────────────────────────────────────────────
  │                                                                      │
  ▼                                                                      ▼
Phase 2 (Async/Await core)                                    Phase 3 (Embedded LLVM)
  │                                                                      │
  └──────────┬───────────────────────────────────────────────────────────┘
             ▼
      Phase 5 (Backend codegen: Async/Await in LLVM/CIRCT/Webstack)
             │
             ▼
      Phase 4 (CIRCT full implementation) ────► Phase 6 (.dbv spec files)
                                                     [can be parallel with Phase 4]
```

### Dependencies:
- **Phase 2** (async/await core) depends on **Phase 1** (CompilationTarget, routing paths)
- **Phase 3** (embedded LLVM) depends on **Phase 1** (is_embedded flag, halt# intrinsic)
- **Phase 5** (async/await backend codegen) depends on **Phase 2** (statement types) + **Phase 3** (embedded term! handling)
- **Phase 4** (CIRCT full) depends on **Phase 1** (run_cbv, CompilationTarget::Circuit) and can proceed in parallel with Phases 2-3
- **Phase 6** (.dbv specs) depends on **Phase 3** (EmbeddedConfig, ISR support)

### Parallelization:
- Phases 2 and 3 are INDEPENDENT — can be implemented in parallel
- Phase 4 can start after Phase 1 is complete (routing exists)
- Phase 6 can start after Phase 3.6 (ISR support) is complete

---

## Files Changed Summary

| Phase | File | Change Type |
|-------|------|-------------|
| 1 | `src/typechecker.rs` | Add `CompilationTarget::Embedded`, `Circuit` |
| 1 | `src/ast.rs` | Add `Intrinsic::Halt` |
| 1 | `src/main.rs` | Route `.ebv`→LLVM, `.cbv`→CIRCT; add `run_cbv`; target inference |
| 1 | `src/hardware_validator.rs` | Replace `is_ebv: bool` with `CompilationTarget` |
| 1 | `src/backend/mod.rs` | Remove 9 dead backends from module list |
| 1 | `archive/backend/` | Move 9 dead backend files |
| 1 | `src/backend/llvm/mod.rs` | Add `is_embedded` flag, builder method |
| 1 | `src/backend/llvm/emit_expr.rs` | Handle `Intrinsic::Halt` |
| 1 | `src/interpreter.rs` | Handle `Intrinsic::Halt` |
| 2 | `src/lexer.rs` | Add `Token::Await` |
| 2 | `src/ast.rs` | Add `Statement::Await`, `Async`, `AsyncAwait` |
| 2 | `src/parser.rs` | Parse all async/await forms, `#[interrupt]` |
| 2 | `src/interpreter.rs` | Evaluate new statement variants |
| 2 | `src/typechecker.rs` | Type-check new statements |
| 2 | `src/proof_engine.rs` | Update mutual exclusion / async promotion |
| 3 | `src/backend/llvm/mod.rs` | `EmbeddedConfig`, restricted mode validation |
| 3 | `src/backend/llvm/emit_stmt.rs` | Embedded `term!` → `halt#`, async/await stmts |
| 3 | `src/backend/llvm/emit_toplevel.rs` | ISR wrapper emission |
| 3 | `src/main.rs` | Freestanding link flags for embedded |
| 3 | `src/analysis/call_graph.rs` | Recursion depth analysis for embedded |
| 4 | `src/backend/circt.rs` | Full rewrite (~2000 lines) |
| 4 | `src/main.rs` | `run_cbv` implementation |
| 5 | `src/backend/llvm/emit_stmt.rs` | Async/await codegen |
| 5 | `src/backend/circt.rs` | Async/await codegen |
| 5 | `src/backend/webstack.rs` | Async/await codegen |
| 6 | `lib/targets/mcu.dbvs` | New schema file |
| 6 | `lib/targets/*.dbv` | Example MCU configurations |
| 6 | `src/main.rs` | `.dbv` auto-discovery and binding |
| 6 | `src/backend/llvm/emit_toplevel.rs` | Vector table generation |

---

## Pre-Implementation Checklist

Before starting coding:
- [x] Create `docs/plans/2026-06-19-backend-routing-async-await.md` (this document)
- [ ] Create `archive/backend/` directory
- [ ] Verify `cargo test --lib` passes (952 tests, 0 fail)
- [ ] Verify `cargo build` succeeds with no warnings

After each phase:
- [ ] `cargo test --lib` — all tests pass
- [ ] `cargo build` — no warnings
- [ ] Update `docs/architecture/features/` for new features
- [ ] Log bugs/gotchas in BUGS.md or `docs/architecture/praetor-log.md`
- [ ] Add Kani harnesses for safety-critical code
- [ ] Run Praetor on new/changed files (complexity ≤ 15, lines ≤ 100, params ≤ 6)

---

## Design Decision Log

| ID | Decision | Rationale |
|----|----------|-----------|
| D01 | `term!` in embedded mode halts CPU, not infinite loop | Power efficiency on battery-powered devices. User can use explicit `halt#()` for sleep. |
| D02 | `async await` supports both discard and capture forms | Flexibility. Discard for fire-and-forget-with-barrier, capture for pipeline. |
| D03 | `await` on any callable | Consistency. Awaiting a pure `defn` is a no-op; awaiting a `txn` runs to convergence. |
| D04 | Dead backends move to `archive/`, not deleted | Preserve for reference. No compilation, no maintenance. |
| D05 | `.ebv` uses both bare-metal default and `.dbv` spec files | Simple programs work out of the box. Complex devices get full configuration. |
| D06 | `halt#` is an intrinsic, not a language keyword | Keeps core language small. `halt#()` is explicitly a CPU-level operation, not a control flow construct. |
| D07 | CIRCT backend emits `comb` + `seq` + `cf` dialects | These are the standard CIRCT dialects for RTL generation. `hw` for module structure, `comb` for logic, `seq` for registers, `cf` for FSM control flow. |
| D08 | `.cbv` bypasses LLVM entirely | CIRCT is the correct IR for hardware. LLVM IR is not synthesizable. |
