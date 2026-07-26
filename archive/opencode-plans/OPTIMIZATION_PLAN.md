# Brief Compiler — Complete Optimization Plan

**Date:** 2026-05-26  
**Status:** Proposed  
**Based on:** Full codebase audit across all 11 backends, proof engine, analysis infrastructure, and PRAXIS/OPTIMIZATIONS roadmaps.

---

## Overview

The compiler architecture is solid. 269 tests pass. 11 backends exist. But there are specific, measurable gaps across four layers:

| Layer | Health | Key Issue |
|-------|--------|-----------|
| **Proof Engine** (2595L) | ❌ Critical | Postconditions with `@prior` never verified; symbolic executor can't reason about real conditions |
| **Backend Codegen** (10 backends) | 🟡 Mixed | 7/10 drop `Unification` silently; most statements generate comments, not real code |
| **Acyclicity Usage** | 🟡 Partial | Only 4/10 backends use `analyze_program()`; 6 backends miss the optimization |
| **PRAXIS Hyper-Optimizations** | ❌ Not started | Transaction fusion, branchless code, memory overlay, guard caching — zero lines |

---

## Phase 1: Proof Engine Repair (Critical)

### 1.1 Fix `check_post_satisfiable` — Prior-State Verification

**Location:** `src/proof_engine.rs:586-611`

**Problem:** `check_post_satisfiable` returns `true` unconditionally for any postcondition containing `@prior`. This means postcondition verification is a no-op for any real contract.

**Required fix:**
- Replace unconditional `true` with actual symbolic comparison between computed post-state and the `@prior` expression
- The executor must evaluate the post-state after symbolic execution of the body, then prove it equals the stated postcondition

**Verification:** A contract like `[x == @x + 1]` assigned to a transaction that does `&x = x + 2` should **fail** verification.

---

### 1.2 Fix `init_state_from_precondition` — Precondition Constraints

**Location:** `src/proof_engine.rs:364-389`

**Problem:** Preconditions are read to *discover which variables exist* but their constraints (e.g., `x > 0`) are never added to the symbolic state's path constraints.

**Required fix:**
- Parse precondition into path constraints and attach them to the symbolic state
- The executor should know `x > 0` is a precondition when evaluating guard conditions in the body

**Verification:** A transaction with `[x > 10]` should not explore paths where `x <= 10`.

---

### 1.3 Expand `SymbolicValue::from_expr` — Expression Coverage

**Location:** `src/proof_engine.rs:109-145`

**Problem:** The catch-all `_ => SymbolicValue::Unknown` silently drops comparisons (`Eq`, `Ne`, `Lt`), booleans (`And`, `Or`, `Not`), calls, field access, and list indexing.

**Required fix:**
- Add `SymbolicValue` variants: `SymEq`, `SymAnd`, `SymOr`, `SymNot`, `SymLt`, `SymGt` etc.
- Implement `eval_symbolic` for these to support recursive reasoning
- At minimum, support: `Eq`, `Ne`, `Lt`, `Le`, `Gt`, `Ge`, `And`, `Or`, `Not`

**Verification:** `[x > 0 && x < 10]` should be representable and evaluable in the symbolic state.

---

### 1.4 Expand `analyze_postcondition` — Contradiction Detection

**Location:** `src/proof_engine.rs:1469-1498`

**Problem:** Only detects `x == @x`. Real contradictions like `x > 0 && x < 0` or `x == 1 && x == 2` pass silently.

**Required fix:**
- Walk postcondition expression for contradictions
- Check for: `a && !a`, numeric range contradictions, impossible equalities
- Use the expanded `SymbolicValue` from 1.3 to evaluate feasibility

**Verification:** `[x > 0 && x < 0]` on any assignment emits a P003 error.

---

### 1.5 Expand `negate_expr` — Lambda Counterexample Check

**Location:** `src/proof_engine.rs:356-362`

**Problem:** Only handles `Bool` and `Identifier`. Cannot negate complex expressions, so the lambda counterexample check is silently skipped for anything non-trivial.

**Required fix:**
- Add negation for: `Eq`, `Ne`, `Lt`, `Le`, `Gt`, `Ge`, `And` (via De Morgan), `Or` (via De Morgan), `Not` (double negation)
- Wire the result into the lambda verification path

**Verification:** A lambda `fn [x > 0][x > 0]` should detect that postcondition cannot be proved from precondition alone (no mutation happens).

---

### 1.6 Expand `implies` — Non-Negated Constraint Checking

**Location:** `src/proof_engine.rs:564-584`

**Problem:** Only negated constraints are checked; non-negated path constraints are ignored. Combined with 1.2, this means path constraints are effectively unused.

**Required fix:**
- After checking negated constraints, also check non-negated constraints for consistency
- A path constraint `x > 5` should be checked against the precondition `x < 3` for contradiction

**Verification:** A path requiring `x > 5` in a transaction with precondition `[x < 3]` should report a dead path.

---

## Phase 2: Backend Codegen Completeness

### 2.1 Fix `Statement::Unification` Gap (7 backends)

**Affected:** `aarch64.rs`, `x86_64.rs`, `c.rs`, `llvm.rs`, `wasm.rs`, `cobol.rs`, `verilog.rs`

**Problem:** `Statement::Unification` (`[value Variant(field)]` pattern matching) is the mechanism for enum destructuring. 7/10 backends silently drop it via `_ => {}` or `_ => comment`.

**Required for each backend:**
- Emit a pattern-match equivalent in the target language
- If the target lacks pattern matching (C, COBOL, assembly), emit if/else chains comparing the discriminant, then extracting fields

**Backend-specific guidance:**

| Backend | Strategy |
|---------|----------|
| `c.rs` | `if/else if` chain comparing enum discriminant, field extraction via union access |
| `aarch64.rs` | Compare discriminant register, conditional branch or CSEL |
| `x86_64.rs` | Compare discriminant register, CMOV or conditional jump |
| `llvm.rs` | `switch` instruction on discriminant, `extractvalue` for fields |
| `wasm.rs` | `br_table` on discriminant |
| `cobol.rs` | `EVALUATE` statement on discriminant |
| `verilog.rs` | `case` statement on discriminant register |

---

### 2.2 Fix `Statement::Escape` Gap (1 backend)

**Affected:** `c.rs`

**Problem:** C backend lacks explicit `Escape` handling, silently producing `/* statement not implemented */`.

**Required fix:**
- Emit `goto escape_label;` or `return <error_code>;` depending on context
- Properly wire the escape label into the generated function structure

---

### 2.3 Upgrade Comment-Only Codegen to Real Code

**Affected:** All backends for `alka`, `OnExit`, `LocalTrigger`, and some for `InlineAsm`

**Current state across all backends:**
- `alka {}` → `// alka! {{ content }}` (comment)
- `#on_exit { body }` → `// on_exit: ...` (comment — body statements collected but not emitted)
- `trg! name: Type = expr` → `// local trigger: ...` (comment)
- `asm "instr" { clobbers }` → varies: some emit real `asm!()`, some emit comments

**Required for each construct:**

| Construct | Real Codegen Strategy |
|-----------|----------------------|
| `alka {}` | Inline the block content as normal statements (alka IS a hatch for raw statements today — no special semantics) |
| `#on_exit { body }` | Generate cleanup section at function exit point; emit body statements there. In Rust: `defer! {}` or a cleanup block. In C: `goto cleanup;` with cleanup label. In asm: emit at function epilogue. |
| `trg! name` | This represents an async await point. In Rust: `tokio::spawn` + oneshot channel. In C: thread + callback. In wasm: async callback. In assembly: interrupt handler registration. |
| `asm "instr"` | All backends that claim asm support should emit real inline assembly, not `// asm("instr")` comments |

---

### 2.4 Wire `analyze_program()` into Remaining 6 Backends

**Affected:** `x86_64.rs`, `wasm.rs`, `webstack.rs`, `cobol.rs`, `verilog.rs`, `vhdl.rs`

**Problem:** These backends do not call `analyze_program()` and therefore do not know if the call graph is acyclic. They cannot emit static dispatch or `norecurse` annotations.

**Required for each backend:**
1. Import `crate::analysis::call_graph::CallGraph` and `crate::analysis::range::ParameterRanges`
2. Call `crate::backend::analyze_program(program)` at generation start
3. Store `has_cycles` (or `is_acyclic`)
4. Use it to:
   - Emit `norecurse`/`recursive` attributes on functions (LLVM)
   - Choose static vs dynamic dispatch (Rust/C function pointers vs direct calls)
   - Omit recursion depth guards when graph is acyclic (hardware backends)

**Backend-specific guidance:**

| Backend | Acyclic Behavior | Cyclic Behavior |
|---------|-----------------|-----------------|
| `x86_64.rs` | Emit direct `call txn_name` | Emit indirect `call rax` through dispatch table |
| `wasm.rs` | Direct `call $txn_name` | Indirect via `call_indirect` |
| `webstack.rs` | Direct Rust function calls | Use `Box<dyn Fn>` dispatch |
| `cobol.rs` | Direct `CALL 'TXNNAME'` | `CALL` via `PROGRAM-POINTER` |
| `verilog.rs` | Static case statement | Dynamic state register + next-state logic |
| `vhdl.rs` | Static case | Dynamic FSM with state transition table |

---

## Phase 3: VHDL Feature Parity (per PLAN.md)

**Location:** `src/backend/vhdl.rs` (exact line count as of the audit — needs ~930 lines to match Verilog)

**10 steps from PLAN.md:**

| Step | Feature | Est. Lines | Dependencies |
|------|---------|-----------|--------------|
| 3.1 | Read pragmas (filter by `target: "vhdl"`) | 20 | None |
| 3.2 | Separate-component output: `Vec<(filename, source)>` | 150 | 3.1 |
| 3.3 | AXI4-Lite slave bridge | 200 | 3.1 |
| 3.4 | RAM inference (BRAM/URAM with `attribute ram_style`) | 100 | 3.1 |
| 3.5 | Clock divider from `TargetConfig.clock_hz` | 50 | 3.1 |
| 3.6 | Full type mapping (all Type variants → VHDL) | 120 | 3.1 |
| 3.7 | State machine FSM from reactive transactions | 100 | 3.2 |
| 3.8 | Pipeline stages with generate loops | 80 | 3.2 |
| 3.9 | Testbench generation (clock, reset, stimulus, VCD) | 60 | 3.2 |
| 3.10 | PSL assertion generation from contracts | 50 | 3.2 |

---

## Phase 4: PRAXIS Hyper-Optimizations

**Status:** All from PRAXIS.md "Next Steps" — zero lines implemented in any backend.

### 4.1 Branchless Code Generation (PRAXIS v0.13.0)

**Target backends:** `x86_64.rs`, `aarch64.rs`

**What to implement:**
- For simple guards (`[x > 0] { &y = a; }; [x <= 0] { &y = b; };`), emit `CMOV`/`CSEL` instead of `cmp; je; mov; jmp; label: mov`
- Requires proving that guard conditions are mutually exclusive and cover all cases
- Use CallGraph acyclicity to justify the branchless choice (cyclic graphs may need safety branches)

**Verification:** A counter with increment/decrement guards should compile to branchless assembly with zero `jmp` instructions.

---

### 4.2 Transaction Fusion (PRAXIS v0.13.0)

**Target backends:** All

**What to implement:**
- Analyze pairs of reactive transactions where `post(A) ⇒ pre(B)` always holds
- When proven: merge them into a single transaction, eliminating intermediate state
- Requires: proven implication between postcondition of A and precondition of B

**Verification:** Two sequential arithmetic transactions that always fire together should produce a single fused transaction.

---

### 4.3 Guard Pre-Computation (PRAXIS v0.14.0)

**Target backends:** All

**What to implement:**
- Cache guard expression results when guard inputs are unchanged
- In the reactor loop, skip guard re-evaluation for transactions whose dependencies haven't changed
- Use the signal graph (`src/signal_graph.rs`) to track guard dependencies

**Verification:** A guard `[items > 10 && total > 100.0]` should not be re-evaluated on every reactor cycle if `items` and `total` are unchanged.

---

### 4.4 Register Allocation — Linear Scan (OPTIMIZATIONS Tier 7)

**Target backends:** `x86_64.rs`, `aarch64.rs`

**What to implement:**
- `compute_live_intervals(instructions) → Vec<Interval>` — O(n)
- `sort_by_start_position(intervals)` — O(n log n)
- `linear_scan_allocate(intervals) → Vec<Register>` — O(n)
- Spill to stack when registers exhausted

**Current state:** OPTIMIZATIONS.md labels this "Planned." Both assembly backends currently use hardcoded register names with no allocation logic.

---

### 4.5 Peephole Optimization (OPTIMIZATIONS Tier 7)

**Target backends:** `x86_64.rs`, `aarch64.rs`

**What to implement:**
- Sliding window (size 2-3 instructions)
- Patterns: redundant load/store elimination, constant folding, strength reduction
- `mov rax, rax` → eliminate (no-op)
- `add rax, 0` → eliminate
- `mov rax, rbx; mov rbx, rax` → `mov rax, rbx` (swap elimination)

**Current state:** Both assembly backends emit instructions directly with no peephole pass.

---

### 4.6 Memory Overlay (PRAXIS v0.13.0)

**Target backends:** `c.rs`, `rust.rs`, `aarch64.rs`, `x86_64.rs`

**What to implement:**
- SMT-based lifetime analysis: prove that two state variables are never simultaneously live
- When proven: overlay them at the same memory address (reducing stack/global size)
- Maps to: `union` in C, `#[repr(C)] union` in Rust, same stack offset in assembly

**Verification:** Two state variables used in mutually exclusive transactions should share the same memory address.

---

### 4.7 Parallel Transaction Scheduling (PRAXIS v0.14.0)

**Target backends:** `x86_64.rs`, `aarch64.rs`, `verilog.rs`, `vhdl.rs`

**What to implement:**
- Use CallGraph + dataflow analysis to identify transactions with no read/write conflicts
- For proven-independent transactions: emit parallel execution paths
- In assembly: interleave instructions from independent transactions
- In hardware: place in separate clock domains or parallel processes

**Verification:** Two transactions modifying different state variables should have no sequential dependency in generated code.

---

### 4.8 SIMD Vectorization (PRAXIS v0.15.0+)

**Target backends:** `x86_64.rs`, `aarch64.rs`, `wasm.rs`

**What to implement:**
- Detect transaction bodies that apply the same operation element-wise to arrays
- Emit: SSE/AVX instructions (x86), NEON instructions (ARM), SIMD instructions (WASM)
- Requires proven equal-length lists (already checked by `check_list_simd_lengths` in proof engine)

---

## Phase 5: Self-Hosted Compiler Mirroring

### 5.1 `is_acyclic` Codegen Paths in Brief Backends

**Status:** The only unchecked item in the self-hosted mirroring checklist.

**Location:** `lib/compiler/backends/`

**What to implement:**
- Brief versions of `aarch64.bv`, `x86_64.bv`, `rust.bv`, `c.bv` must check acyclicity
- Each backend needs: `if has_cycle then emit_dynamic_dispatch else emit_static_dispatch`
- Mirrors the Rust-side `analyze_program()` integration

---

## Implementation Order (Recommended)

```
Priority       Phase / Item                     Effort     Value
────────       ──────────────────────────       ─────      ─────
HIGHEST        1.1 @prior postcondition fix      ~2 days    Unlocks all contract verification
HIGHEST        1.2 Precondition constraints      ~2 days    Required for 1.1 correctnss
HIGHEST        1.3 Symbolic expression coverage  ~3 days    Required for 1.1 + 1.4 + 1.6
HIGH           1.4 Contradiction detection       ~1 day     Low effort, high confidence gain
HIGH           2.1 Unification (7 backends)      ~3-5 days  Eliminates silent code drops
HIGH           2.4 Acyclicity wiring (6 bknds)   ~2 days    Unlocks optimization in 6 backends
MEDIUM         2.3 Real codegen (alka/exit/trg)  ~3-5 days  Comment-only is embarrassing for v0.14
MEDIUM         3.x VHDL parity (10 steps)        ~4 days    Per PLAN.md
MEDIUM         1.5 Lambda negation check         ~1 day
MEDIUM         1.6 Non-negated constraint check  ~1 day
MEDIUM         2.2 Escape fix (c.rs)             ~2 hrs     Single backend, trivial
LOW            5.1 Brief backends acyclicity     ~2 days    Last self-hosted mirror item
FUTURE         4.x PRAXIS optimizations          ~3-5 weeks All unchecked, no timeline
```

---

## Effort Estimate

| Scope | Est. Person-Days |
|-------|-----------------|
| Phase 1: Proof Engine (1.1–1.6) | ~10-12 days |
| Phase 2: Backend Completeness (2.1–2.4) | ~8-12 days |
| Phase 3: VHDL Parity (3.1–3.10) | ~4 days |
| Phase 4: PRAXIS (4.1–4.8) | ~15-25 days |
| Phase 5: Self-Hosted (5.1) | ~2 days |
| **Total** | **~39-55 days** |

---

## How to Resume

1. Start with **Phase 1.1 (`check_post_satisfiable`)** — this is the biggest gap in the product's core promise
2. Read the full proof engine (`src/proof_engine.rs`) before making changes — 2595 lines, all interconnected
3. Run `cargo test --lib` after each change — 269 tests must pass
4. After each Phase 1 item, write a new test that verifies the previously-missing detection works
5. Once proof engine is sound, move to Phase 2 — backend codegen completeness
