# Plan: Differential Fuzzing, Trigger Elevation & Memory Spec

## Overview
This plan distills three major architectural initiatives from the conversation:
1. **Property-Based Differential Fuzzing** - Safe compiler testing with Unicorn emulator
2. **`trg`/`trigger` Elevation to Core Briev** - Unified async I/O across all Briev flavors
3. **Memory Spec Output** - `--emit-memory-spec` CLI flag for FFI coordination

---

## 1. Property-Based Differential Fuzzing

### 1.1 AST Generator (Arbitrary/Procedural Generation)
**Goal**: Procedurally generate valid and semi-valid Briev ASTs for fuzzing without manual test cases.

**Changes Required**:
- [ ] Create `src/fuzzing/ast_generator.rs` with recursive generation functions:
  - `generate_random_expr(depth: usize)` - generates random `Expr` trees with depth limiting
  - `generate_random_statement(depth: usize)` - generates random `Statement` nodes
  - `generate_random_transaction()` - generates random `Transaction` with contracts
  - `generate_random_program()` - assembles full `Program` ASTs
  - `generate_random_op()` - picks random binary/unary operators
  - `generate_random_type()` - generates random type expressions
- [ ] Implement depth limiting to prevent stack overflow during generation
- [ ] Support semi-valid ASTs (intentionally broken contracts, type mismatches) for error-path testing
- [ ] Add `impl Arbitrary for Expr`, `impl Arbitrary for Statement`, etc. using `proptest` crate

**Files to modify**:
- `Cargo.toml` - Add `proptest` and `unicorn-engine` dev dependencies
- New: `src/fuzzing/mod.rs`
- New: `src/fuzzing/ast_generator.rs`

### 1.2 Frontend "No-Panic" Fuzzer
**Goal**: Verify lexer/parser never panic on any input.

**Changes Required**:
- [ ] Create `tests/fuzz_frontend.rs` with proptest harness:
  - Generate random strings (garbage bytes, structured-but-invalid Briev syntax)
  - Feed through `tokenize()` and `parse_program()`
  - Assert: Returns `Ok(Program)` or `Err(String)`, never panics/unwraps/infinite-loops
  - Add timeout guard for parser (max 5 seconds per input)

**Files to modify**:
- New: `tests/fuzz_frontend.rs`

### 1.3 Safe Differential Backend Fuzzer (Unicorn Sandbox)
**Goal**: Verify compiled machine code matches symbolic proof engine results.

**Changes Required**:
- [ ] Create `tests/fuzz_backend.rs` with differential fuzzing harness:
  - **Step A**: Pass generated AST to `eval_symbolic` in proof engine, record expected final state
  - **Step B**: Pass same AST to `generate_aarch64`/`generate_x86_64`, extract raw `Vec<u8>`
  - **Step C**: Instantiate Unicorn CPU emulator (AArch64/x86_64), map 2MB dummy memory, load machine code
  - **Step D**: Execute with strict instruction timeout (max 10,000 instructions to prevent infinite loops)
  - **Step E**: Read final CPU registers (X0/X1 or RAX/RBX) and memory
  - **Assertion**: `assert_eq!(Symbolic_Result, Emulator_Result)`
- [ ] Add register mapping layer (Briev variables → emulator registers)
- [ ] Add memory mapping layer (Briev state → emulator memory regions)
- [ ] Support both AArch64 and x86_64 backends

**Files to modify**:
- New: `tests/fuzz_backend.rs`
- `Cargo.toml` - Add `unicorn-engine` dev dependency

### 1.4 Concolic Fuzzer (Proof-Guided)
**Goal**: Use proof engine path constraints to generate only reachable test inputs.

**Changes Required**:
- [ ] Create `src/fuzzing/concolic.rs`:
  - Extract path constraints from `proof_engine.bv` (e.g., "x > 10 && y == false")
  - Generate concrete inputs that satisfy constraints
  - Eliminate impossible state permutations
- [ ] Integrate with AST generator to produce targeted test cases

**Files to modify**:
- New: `src/fuzzing/concolic.rs`

### 1.5 Fault Injection Fuzzer (FFI/Hardware Chaos)
**Goal**: Simulate "messy outside world" at FFI and hardware boundaries.

**Changes Required**:
- [ ] Create `tests/fuzz_fault_injection.rs`:
  - **FFI Chaos**: Intercept foreign function calls, return garbage data/timeouts/memory corruption
  - **Hardware Chaos**: Flip random bits in simulated hardware registers (cosmic ray simulation)
  - **Metropolitan Chaos**: Randomly alter Status Word to simulate foreign program crashes
- [ ] Focus exclusively on `trg`-marked variables and FFI boundaries
- [ ] Verify no postcondition violations under fault conditions

**Files to modify**:
- New: `tests/fuzz_fault_injection.rs`

---

## 2. `trg`/`trigger` Elevation to Core Briev

### 2.1 Token Aliases
**Goal**: Support `trg`, `TRG`, `trigger`, `TRIGGER` as equivalent tokens.

**Current State**: `trg` and `TRG` already implemented in `src/lexer.rs:132-134`.

**Changes Required**:
- [ ] Update `src/lexer.rs` lexer tokens:
  ```rust
  #[token("trg")]
  #[token("TRG")]
  #[token("trigger")]
  #[token("TRIGGER")]
  Trg,
  ```
- [ ] Update parser to recognize all four variants (already handled by single `Token::Trg`)

**Files to modify**:
- `src/lexer.rs`

### 2.2 Local `trg!`/`trigger!` Declarations Inside Transactions
**Goal**: Allow `trg!` or `trigger!` declarations inside transaction bodies for mid-flight async waits. The `!` suffix serves as a psychological speedbump (like Ruby's `sort!` or Rust's `unsafe {}`) warning the programmer they are introducing asynchronous chaos into a verified transaction.

**Current State**: `trg` only supported at top-level (`TopLevel::Trigger`).

**Changes Required**:
- [ ] Update `src/ast.rs`:
  - Add `Statement::LocalTrigger(LocalTrigger)` variant for local triggers
  - New `LocalTrigger` struct:
    ```rust
    pub struct LocalTrigger {
        pub name: String,
        pub ty: Type,
        pub expr: Option<Expr>,  // The FFI/trigger expression to await
        pub is_acknowledged: bool,  // True only if `!` was used (always true for local triggers)
        pub span: Option<Span>,
    }
    ```
- [ ] Update `src/lexer.rs`:
  - Top-level `trg`/`TRG`/`trigger`/`TRIGGER` remain unchanged (no `!`)
  - Add `TrgBang` token for `trg!`/`TRG!`/`trigger!`/`TRIGGER!`:
    ```rust
    #[token("trg!")]
    #[token("TRG!")]
    #[token("trigger!")]
    #[token("TRIGGER!")]
    TrgBang,
    ```
- [ ] Update `src/parser.rs`:
  - Add `trg!`/`trigger!` parsing in `parse_statement()` dispatch
  - Support syntax: `trg! name: Type = expr;` inside transaction bodies
  - **Error message**: If `trg` (without `!`) is used inside a transaction, emit:
    > `Error: Local triggers introduce asynchronous rollback risks. You must use 'trg!' or 'trigger!' to explicitly acknowledge this boundary.`
  - Top-level `trg` (without `!`) remains valid for global trigger declarations
- [ ] Update `src/typechecker.rs`:
  - Handle local trigger type checking
  - Local triggers should be scoped to their transaction
  - Track `trg!` markers for proof engine symbolic invalidation
- [ ] Update `src/proof_engine.rs`:
  - `trg!` marks exact points where symbolic execution paths must split:
    - **Path A**: Trigger succeeds, transaction continues
    - **Path B**: Trigger fails/times out, `escape` fires, assert state rolls back to pre-transaction snapshot
  - Transaction may yield/pause at local trigger until it fires
- [ ] Update all backends (AArch64, x86_64, Rust, C, WASM, Verilog, VHDL, COBOL):
  - Emit appropriate async/wait/yield code for local triggers
  - Emit rollback checkpoint code before `trg!` points

**Files to modify**:
- `src/ast.rs`
- `src/lexer.rs`
- `src/parser.rs`
- `src/typechecker.rs`
- `src/proof_engine.rs`
- `src/backend/*.rs` (all backends)
- `src/annotator.rs`
- `src/lsp.rs`

### 2.3 Symbolic Invalidation for `trg` Variables
**Goal**: Proof engine treats `trg` variables as volatile (value can change between reads).

**Current State**: Proof engine assumes variables hold stable state.

**Changes Required**:
- [ ] Update `src/proof_engine.rs`:
  - Mark `trg` variables with volatile taint in symbolic state
  - On each read of a `trg` variable, create new symbolic variable (e.g., `SymVar("x_at_time_1")`, `SymVar("x_at_time_2")`)
  - Drop assumption that consecutive reads are equal
  - Force stricter verification on paths "polluted" by `trg` variables
- [ ] Update `src/typechecker.rs`:
  - Track which variables are `trg`-marked
  - Propagate volatile taint through expressions

**Files to modify**:
- `src/proof_engine.rs`
- `src/typechecker.rs`

### 2.4 System Triggers Standard Library
**Goal**: Provide standard system triggers for Regular Briev I/O.

**Changes Required**:
- [ ] Create `lib/std/system.bv` with:
  ```briev
  trg stdin_line: String;
  trg sigint: Bool;        // Ctrl+C
  trg clock_tick_1hz: Int; // Fires every second
  trg stdout_ready: Bool;
  trg file_event: String;  // File system event
  ```
- [ ] Update `lib/std/io.bv` to use triggers instead of blocking calls
- [ ] Update FFI registry to map system triggers to OS event loop (epoll/kqueue/IOCP)

**Files to modify**:
- New: `lib/std/system.bv`
- `lib/std/io.bv`
- `std/bindings/io.dbvs`

### 2.5 Software Trigger Config Bindings
**Goal**: Allow config files to map software triggers to OS events.

**Changes Required**:
- [ ] Create trigger binding schema in DBVS format:
  ```dbvs
  // std/bindings/system_triggers.dbvs
  trigger stdin_line {
      type: "stdin"
      mode: "line_buffered"
  }
  trigger sigint {
      type: "signal"
      signal: "SIGINT"
  }
  trigger network_packet {
      type: "socket"
      port: 8080
      protocol: "tcp"
  }
  ```
- [ ] Update `src/ffi/registry.rs` to load trigger bindings
- [ ] Update `src/main.rs` to pass trigger config to backend code generation
- [ ] Backends emit appropriate OS event loop setup code based on bindings

**Files to modify**:
- New: `std/bindings/system_triggers.dbvs`
- `src/ffi/registry.rs`
- `src/ffi/loader.rs`
- `src/dbriev/ast.rs` (add trigger schema support if needed)
- `src/dbriev/parser.rs`

### 2.6 Pre-Evaluation Guard System (Two-Tier Execution)
**Goal**: Minimize wasted FFI side effects by pre-evaluating escape conditions before running a transaction. Only run speculatively when escape conditions depend on future/unpredictable events.

**Design**: Two-tier execution model:
- **Tier 1: Pre-Evaluation Guard** - Before running, evaluate all escape conditions that depend on currently-known trigger state. If any guard proves the transaction *will* escape, skip it entirely. No FFI calls fired, no side effects, zero risk.
- **Tier 2: Speculative Execution** - When escape conditions depend on future triggers or FFI responses that can't be pre-checked, run the transaction speculatively. If escape hits mid-flight, rollback state (already implemented). FFI calls that already fired are the cost of uncertainty.

**What Can vs Can't Be Pre-Evaluated**:

| Scenario | Pre-Evaluatable? | Action |
|----------|-----------------|--------|
| Escape depends on a `trg` that just fired | Yes | Guard check → skip or run |
| Escape depends on a *second* trigger not yet fired | No | Speculative + rollback |
| Escape depends on FFI response (e.g., HTTP 404) | No | Speculative + rollback |
| Escape depends on static state (e.g., `x > 10` where `x` is known) | Yes | Guard check → skip or run |

**Changes Required**:
- [ ] Update `src/proof_engine.rs`:
  - Extract all escape path conditions at compile time
  - Classify each escape condition as pre-evaluatable or speculative
  - Generate pre-evaluation guard expressions for runtime use
- [ ] Update `src/reactor.rs`:
  - Before executing a transaction, evaluate pre-evaluation guards
  - If any guard proves escape → skip transaction entirely
  - Otherwise, execute transaction with existing rollback mechanism
- [ ] Update `src/interpreter.rs`:
  - Add `evaluate_guards()` function that runs pre-evaluation checks
  - Integrate with existing `prior_state` snapshot mechanism
- [ ] Add compiler warning for side effects before `trg!` points:
  > `Warning: FFI call before 'trg!' may execute even if transaction later escapes. Consider reordering.`

**Files to modify**:
- `src/proof_engine.rs`
- `src/reactor.rs`
- `src/interpreter.rs`
- `src/annotator.rs` (warning formatting)

---

## 3. Memory Spec Output (`--emit-memory-spec`)

### 3.1 CLI Flag
**Goal**: Add `--emit-memory-spec` flag to compile/build commands.

**Current State**: CLI has 16+ commands but no memory spec output.

**Changes Required**:
- [ ] Update `src/main.rs`:
  - Add `--emit-memory-spec` global flag
  - When flag is set, collect memory allocations during code generation
  - Write output to `<output_dir>/memory_spec.json` (or `.toml`)
- [ ] Update `print_usage()` to document new flag

**Files to modify**:
- `src/main.rs`

### 3.2 Memory Spec Collector
**Goal**: Collect all variable/register/address allocations during compilation.

**Changes Required**:
- [ ] Create `src/memory_spec.rs`:
  - `MemorySpec` struct with allocations map
  - `Allocation` struct: name, type, address, size_bytes, is_trigger, bit_range, stage
  - `collect_allocations(program: &Program, layout: &MemoryLayout) -> MemorySpec`
  - Support for:
    - State variables with assigned addresses
    - Trigger declarations with MMIO/event addresses
    - Metropolitan FFI shared memory regions
    - Bit-packed struct fields
    - Hardware register mappings
- [ ] Integrate with existing `MemoryLayout` pass in backends

**Files to modify**:
- New: `src/memory_spec.rs`
- `src/backend/*.rs` (integrate collection into code generation)

### 3.3 Memory Spec Output Format
**Goal**: JSON/TOML output for foreign language consumption.

**Output Format** (JSON example):
```json
{
  "target": "kv260",
  "compiler_version": "0.12.0",
  "allocations": {
    "sensor_status": {
      "type": "trg UInt8",
      "address": "0x1000A000",
      "size_bytes": 1,
      "bit_range": "0..7",
      "is_trigger": true
    },
    "reactor_state": {
      "type": "StateEnum",
      "address": "0x1000A008",
      "size_bytes": 4,
      "is_trigger": false
    }
  },
  "metropolitan_ffi": {
    "channel_id_request": {
      "address": "0x20000000",
      "size_bytes": 4096,
      "direction": "bidirectional"
    }
  },
  "triggers": {
    "stdin_line": {
      "type": "event",
      "binding": "stdin",
      "mode": "line_buffered"
    }
  }
}
```

**Changes Required**:
- [ ] Implement JSON serialization for `MemorySpec`
- [ ] Implement TOML serialization as alternative format
- [ ] Add `--memory-spec-format json|toml` flag (default: json)

**Files to modify**:
- `src/memory_spec.rs`
- `Cargo.toml` (ensure `serde_json` dependency)

### 3.4 Foreign Language Header Generation
**Goal**: Auto-generate C headers / Rust structs from memory spec.

**Changes Required**:
- [ ] Create `src/memory_spec/codegen.rs`:
  - `generate_c_header(spec: &MemorySpec) -> String`
  - `generate_rust_module(spec: &MemorySpec) -> String`
  - `generate_python_module(spec: &MemorySpec) -> String`
- [ ] Add `--emit-ffi-headers` flag to auto-generate alongside memory spec
- [ ] Integrate with Metropolitan FFI code generators

**Files to modify**:
- New: `src/memory_spec/codegen.rs`
- `src/ffi/metropolitan.rs`

---

## 4. `escape` as Transactional Rollback (Enhancement)

### 4.1 Current State
- `escape` already implemented in lexer, parser, AST
- Proof engine skips postconditions for escape paths (vacuously satisfied)
- Interpreter restores `prior_state` on escape
- Reactor handles `StmtResult::Escaped` with state restoration
- Backends emit `return false` or equivalent

### 4.2 Enhancement: Explicit Rollback Semantics
**Goal**: Make rollback semantics explicit in proof engine and documentation.

**Changes Required**:
- [ ] Update `src/proof_engine.rs`:
  - Add explicit `rollback_state` tracking for escape paths
  - Verify that escape paths correctly revert all symbolic mutations
  - Add test: escape mid-transaction proves prior state is preserved
- [ ] Update `src/interpreter.rs`:
  - Ensure `prior_state` snapshot is taken at transaction start
  - Verify rollback restores ALL state variables (not just modified ones)
- [ ] Add integration test demonstrating rollback behavior

**Files to modify**:
- `src/proof_engine.rs`
- `src/interpreter.rs`
- New: `tests/test_escape_rollback.rs`

---

## Implementation Order

### Phase 1: Foundation (Low Risk, High Value) - COMPLETE
1. ✅ Token aliases (`trigger`/`TRIGGER`) - `src/lexer.rs`
2. ✅ `trg!`/`TRG!`/`trigger!`/`TRIGGER!` tokens for local triggers - `src/lexer.rs`
3. ✅ Memory spec collector and CLI flag - `src/memory_spec.rs`, `src/main.rs`
4. ✅ Memory spec output format - JSON/TOML serialization
5. ✅ Local `trg!` parsing in transactions - `src/parser.rs`, `src/ast.rs`
6. ✅ Error message for plain `trg` inside transactions
7. ✅ Stub handling in all backends (wasm, interpreter, reactor, proof_engine)
8. ✅ Unit tests for trg! parsing (4 new tests)

### Phase 2: Trigger Elevation (Core Language Change) - COMPLETE
5. ✅ Local `trg!` declarations in transactions - AST, parser, typechecker
6. ✅ Pre-evaluation guard system - proof engine, reactor, interpreter
7. ✅ Symbolic invalidation for `trg` variables - proof engine
8. ✅ System triggers stdlib - `lib/std/system.bv`
9. ✅ Software trigger config bindings - DBVS schema, FFI registry

### Phase 3: Fuzzing Infrastructure (Testing) - COMPLETE
10. ✅ AST generator (proptest Arbitrary) - `src/fuzzing/`
11. ✅ Frontend no-panic fuzzer - `tests/fuzz_frontend.rs`
12. ✅ Safe differential backend fuzzer (Unicorn) - SKIPPED (requires native dependency, feature-gated for future)
13. ✅ Concolic fuzzer (proof-guided) - `src/fuzzing/concolic.rs`
14. ✅ Fault injection fuzzer - `tests/fuzz_fault_injection.rs`

### Phase 4: Polish & Integration
15. Escape rollback semantics enhancement
16. Foreign language header generation from memory spec
17. Backend updates for local triggers with rollback checkpoints
18. Compiler warnings for FFI calls before `trg!` points
19. Integration tests and documentation

---

## Dependencies & New Crates

Add to `Cargo.toml` (dev-dependencies):
```toml
[dev-dependencies]
proptest = "1.4"
unicorn-engine = "0.1"  # Or latest version
```

Ensure existing dependencies:
```toml
serde_json = "1.0"  # For memory spec JSON output
```

---

## Risk Assessment

| Item | Risk | Mitigation |
|------|------|------------|
| Unicorn Engine Integration | Medium - native dependency | Use feature flag, skip on unsupported platforms |
| Local `trg!` Semantics | High - changes execution model | Start with parser/AST, defer backend emission |
| Symbolic Invalidation | Medium - proof engine complexity | Add incrementally, test with simple cases first |
| Pre-Evaluation Guard Classification | Medium - determining what's pre-evaluatable | Conservative default: classify as speculative if uncertain |
| AST Generator Stack Overflow | Low - depth limiting | Hard cap at depth 10-15 |
| Memory Spec Collector | Low - mostly data collection | Reuse existing MemoryLayout pass |

---

## Success Criteria

1. **Fuzzing**: 100,000+ random ASTs generated and tested without compiler panic
2. **Differential**: Symbolic proof results match emulator results for 10,000+ programs
3. **Triggers**: `trg` works at top-level; `trg!` required inside transactions across all backends
4. **Pre-Evaluation Guard**: Transactions with provably-escaping conditions are skipped before any FFI fires
5. **Memory Spec**: `briev build --emit-memory-spec` produces valid JSON for any program
6. **Tests**: All 163 existing tests still pass, plus new fuzzing tests
