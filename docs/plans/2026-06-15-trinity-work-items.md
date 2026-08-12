# Trinity Work Items — SSA Fix, foreach, `?#` Oracle, Instruction Reordering

**Date:** 2026-06-15  
**Status:** Planned (no work started)  
**Context:** Following completion of trg reactive dirty-flag architecture (Phases 1–6) and intrinsic system (Phases A–H, 79 intrinsics).

---

## Priority Order

1. **SSA phi dominance fix** (officina-cli blocker)
2. **`foreach` LLVM backend + SIMD** (core feature)
3. **`?#` proof oracle** (new feature)
4. **Transaction body instruction reordering** (optimization)

---

## 1. SSA Phi Dominance Fix

### Problem
17 "Instruction does not dominate all uses" LLVM IR verification errors in general loop emission. When `loop_engine.rs` generates values across guard/loop blocks without proper phi merges, `opt` and `llc` reject the IR.

### Root cause (known)
`emit_stmt.rs` saves/restores `let_bindings` across guard boundaries (fix #6 from `backend-strategy.md`), but the fix only covers one specific pattern. The remaining 17 violations are in the general loop's event dispatch — values defined inside a loop/guard block are referenced outside it without a phi node.

### Approach
- Add a `fix_ssa_dominance` pass that walks the generated IR to find dominance violations and inserts phi nodes
- Or: fix the root cause in `emit_stmt.rs` / `loop_engine.rs` by wrapping live-out values in phi merges at block boundaries
- Verification: `opt -O2` passes without errors, officina-cli compiles clean

### Files affected
- `src/backend/llvm/loop_engine.rs` — main loop emission
- `src/backend/llvm/emit_stmt.rs` — guard block emission
- `src/backend/llvm/tests.rs` — verification tests

---

## 2. `foreach` Completion

### Background
`Statement::Foreach { item, list, body }` has existed in the AST (`ast.rs:1354`) through multiple past sessions. Lexer token, parser, interpreter, proof engine, dataflow, and region analysis all handle it. Two interpreter tests exist.

**Every backend is a comment stub.** The LLVM backend emits `; foreach item in ...` and recursively emits the body — no actual loop IR. All other backends (Webstack, dead backends) have identical stubs.

### 2.1 LLVM Loop Body

#### Target IR

```llvm
; entry: compute list length
%len = call i64 @list_len(i8* %list_ptr)
br label %foreach_hdr

; header: phi indvar, check bound
foreach_hdr:
%i = phi i64 [0, %entry], [%next, %foreach_body]
%done = icmp slt i64 %i, %len
br i1 %done, label %foreach_body, label %foreach_done

; body: load element, bind to item var, emit body stmts
foreach_body:
%elem = call i64 @list_get(i8* %list_ptr, i64 %i)
; bind %elem to 'item' in let_bindings for body emission
; emit body statements (they refer to 'item' as a local var)
; body may write back to state via GEP+store

; check back-edge
%next = add i64 %i, 1
br label %foreach_hdr

; done: continue after foreach
foreach_done:
```

When the body is provably pure (no state writes, no FFI), the loop can use a `%State` SSA value with `insertvalue`/`extractvalue` chains instead of GEP+store.

#### Tasks
1. **Emit real loop IR** in `emit_stmt.rs`: phi indvar, load/bind element, body emission, back-edge, exit
2. **Element binding**: register `item` in `let_bindings` for the body scope, then clear afterwards
3. **Body purity check**: walk body statements — if all pure (no assignment to state fields, no FFI), use SSA path; else GEP+store path
4. **Tests**: foreach in LLVM backend output matches expected IR patterns

### 2.2 SIMD Vectorization

The proof engine already has `check_list_simd_lengths()` which verifies that lists used in parallel operations have equal lengths (`proof_engine.rs:1518`). This was designed for `foreach` but never wired.

#### Approach

When `foreach` has a **pure body** (no cross-iteration dependencies, no FFI) and the list is **proven equal-length** (via `check_list_simd_lengths`):

1. Attach `!llvm.loop.vectorize.enable` metadata to the loop branch
2. Optionally attach `!llvm.loop.interleave.count` based on the proven list length
3. The SLP hazard analyzer (`hazard.rs`) gates this: if peak register pressure exceeds hardware, disable vectorization

```llvm
br i1 %done, label %foreach_body, label %foreach_done, !llvm.loop !0
...
!0 = !{!0, !1}
!1 = !{!"llvm.loop.vectorize.enable", i1 true}
```

LLVM's Loop Vectorizer + SLP then handle the SIMD lowering — the backend just enables the metadata.

#### Tasks
1. Wire `check_list_simd_lengths` result into foreach emission
2. Emit `!llvm.loop.vectorize.enable` metadata on pure-body foreach loops
3. SLP hazard integration: disable metadata when `compute_peak_live_floats` exceeds threshold

### 2.3 Feature File Migration

Follow the `sync_block.rs` pattern: create `src/features/stmt/foreach.rs` with trait impls and thin dispatch from the central `emit_stmt` functions.

#### Tasks
1. Create `src/features/stmt/foreach.rs` with `ForeachStmt { item, list, body }`
2. Implement `StmtTypecheck`, `StmtEval`, `StmtCodegenLLVM`, `StmtCodegenWebstack`
3. Thin dispatch in `emit_stmt.rs`, `webstack.rs` etc. to delegate to feature module
4. Dead backends: leave as stubs (zero fixes policy)

### 2.4 Documentation

Update `docs/architecture/features/statement.md`:

- Replace the "comment stub" backend table with real coverage
- Add LLVM IR example for the emitted loop
- Add SIMD lowering section with metadata and SLP hazard interaction

---

## 3. `?#` Proof Oracle

### Background

Briev already has two watchdog forms:

| Form | Name | Semantics |
|------|------|-----------|
| `?[cond]` | Optional | "Check this if you can" — runtime check at `term`, optional preemptibility analysis |
| `?![cond]` | Required | "Prove this or fail to compile" — fatal on proof failure |

The proposed **`?#`** (third form) is different in kind: it asks "will this even terminate?" and when the compiler can't answer statically, it inserts a runtime safety net.

### Semantics

```
?#[handler] {
    // body
};
```

At **compile time**, the proof engine tries all available strategies in order. If any proves termination, compilation proceeds transparently — zero-cost, no runtime overhead.

If **no strategy succeeds**, the compiler desugars to:

```briev
txn __fuel_guard [__fuel > 0][true] {
    // body
    &__fuel = __fuel - 1;
    term;
};
// On fuel exhaustion: rollback state changes, execute handler
// handler runs, program continues
```

The handler is:
- **Mandatory** when compile-time proof fails (author must decide what "survivable failure" looks like)
- **Optional** when proof succeeds (compiler never generates the rollback path)

### Compile-Time Strategies (in order)

| Strategy | What it checks | Scope |
|----------|---------------|-------|
| **Bounded counter** (exists) | Transition graph: `[i < N][i == N]` with proven convergence | `node` bodies |
| **Structural recursion** (new) | Recursive call on strictly smaller sub-term (list tail, tree child, `n-1`) | `defn` recursion |
| **SMT ranking function** (new) | Encode loop body as transition relation, ask Z3 for a decreasing measure | Any loop/recursion |
| **Fuel budget injection** (exists) | `--optimize-budget` iteration cap becomes the fuel limit | Fallback when all else fails |
| **Closed-form simplification** (partial) | Equality saturation folds recurrence to direct formula | Pure arithmetic loops |

### Runtime Injection (when compile-time fails)

1. **Fuel counter**: inject `&__fuel = __fuel - 1;` at start of body, wrap in `[__fuel > 0]`
2. **State rollback**: the `?#` block executes as a sub-transaction. On fuel exhaustion, all writes to state fields are reverted (same `Escape` semantics). The `__fuel` counter itself is NOT part of the program state — it is a synthetic local.
3. **Handler execution**: after rollback, the handler runs. It can set error flags, log, increment retry counters, push to a retry queue — anything that keeps the program alive.
4. **State merge**: the handler's writes survive (they are not rolled back). The enclosing `txn` / `defn` resumes as if the `?#` block returned normally.

### Syntax

```briev
// Simple — proof engine chooses strategy order
?#[&retries = retries + 1] {
    encode_video_frame(raw, params);
};

// On defn — prove recursion terminates
?#[&status = "timeout"] defn fibonacci(n: Int) -> Int {
    [n <= 1] { term n; };
    term fibonacci(n - 1) + fibonacci(n - 2);
};

// On foreach — prove list iteration terminates (trivial, but enables SIMD)
?#[&partial = partial + 1] foreach (item in stream) {
    process(item);
};
```

### Syntax Note: Handler Binding

The handler `#[handler]` uses a single bracket `#[...]` to distinguish it from the contract brackets `[pre][post]` and watchdog brackets `?[cond]`. Alternative: `?# { body } ?handler;` but the `#[...]` prefix reads more naturally as "in case of failure, do this."

### Ordering Hints (Future)

```briev
?#[handler] @[structural > z3(3s) > fuel(10000)] {
    body;
};
```

Deferred until we know which strategies actually fire in practice.

### Comparison to Existing Systems

| System | Mechanism | Briev analogue |
|--------|-----------|----------------|
| **SPARK** | `Subprogram_Variant` (decreasing expression) | Structural recursion checker (new) |
| **eBPF verifier** | Static loop bound analysis | Bounded counter (exists) |
| **Ethereum gas** | Pre-paid instruction budget + rollback | Fuel injection + rollback (new) |
| **LOOPER** | Trace autocorrelation + SMT | Runtime watchdog extension (future) |
| **Jolt** | State-snapshot convergence detection | Thrash watchdog (future) |

### Tasks

1. **Structural recursion checker** (proof engine)
   - Walk `defn` body for recursive calls
   - Check that the argument is a strict sub-term (e.g., `list.tail()`, `n - 1`)
   - Report: proven / unprovable
2. **`?#` AST and parser**
   - New `Statement::Oracle { handler: Vec<Statement>, body: Vec<Statement> }` variant
   - Parser: `?#` keyword + `#[...]` handler block + `{...}` body
3. **Proof engine dispatch**
   - Run all strategies; if any proves termination, emit transparently (no runtime guard)
   - If none succeed, desugar to fuel-injected form
4. **Fuel injection + rollback**
   - Interpreter: synthetic `__fuel` local, decrement on entry, `ContractViolation("fuel exhausted")` on zero
   - LLVM backend: emit descending counter with early-exit to rollback path
   - Rollback: revert all state field writes made inside the `?#` block
   - Handler emission: emit handler statements after rollback (their writes survive)
5. **Handler mandatory check**
   - Compile-time error if `?#` lacks a handler AND proof fails
   - Warning (not error) if handler is present but proof succeeds (dead code, but harmless)
6. **Tests**
   - Parser tests: `?#[handler] { body };`
   - Interpreter tests: fuel exhaustion triggers handler; body completes within fuel
   - Proof engine tests: bounded counter proven (zero-cost); no proof → fuel injected

### Future Extensions (Not in Scope)

- **Runtime thrash detection**: autocorrelation on tick counter / field-write pattern
- **State-snapshot convergence**: Jolt-style snapshot comparison
- **`?@` ordering hints**: explicit strategy priority list

---

## 4. Transaction Body Instruction Reordering

### Motivation

A transaction body like:

```briev
node compute [ready][done] {
    &x = a + b;
    &y = c + d;
    &z = x * y;
    term;
};
```

Has `x` and `y` as independent calculations that can execute in parallel. Modern CPUs extract ILP, but the window is narrow (~352 µops on Zen 4). The compiler can help by:

1. **Reordering** statements to maximize ILP (interleave independent field writes)
2. **Emitting `noalias` GEP annotations** so LLVM knows field accesses don't alias

### Approach

1. **Dependency analysis within a transaction body**: build a small DAG of statement-level read/write sets (reuse `collect_expr_identifiers` from the dependency graph)
2. **Topological reorder**: group independent statements together
3. **`noalias` emission**: annotate field GEPs with `!noalias` metadata

### Tasks

1. Build intra-txn dependency DAG (field-level read/write sets)
2. Reorder body statements for maximal ILP
3. Emit `noalias` metadata on GEP instructions
4. Verify: runtime performance improves (benchmark suite)

### Interaction with SLP Hazard

The SLP hazard analyzer already disables vectorization when peak register demand exceeds hardware. Instruction reordering should run BEFORE SLP hazard analysis so the hazard analyzer sees the final instruction order.

---

## Summary: Four Work Items

| # | Item | Type | Difficulty | Dependencies |
|---|------|------|-----------|-------------|
| 1 | SSA phi dominance fix | Bug fix | Medium | None |
| 2 | foreach: LLVM loop + SIMD + feature file | Feature | Medium | #1 (loop emission needs working SSA) |
| 3 | `?#` proof oracle | Feature | Large | #1 (fuel injection uses loop emission) |
| 4 | Instruction reordering | Optimization | Small–Medium | #2 (reuses dependency graph from foreach) |

### Test Impact

| Item | New tests expected |
|------|-------------------|
| SSA fix | 5–10 (LLVM IR assertions) |
| foreach | 15–20 (LLVM IR, interpreter, parser) |
| `?#` oracle | 25–35 (proof engine, interpreter, LLVM, parser) |
| Reordering | 5–10 (dependency analysis, output assertions) |

Total expected: **897 → ~960–980 tests** at completion.
