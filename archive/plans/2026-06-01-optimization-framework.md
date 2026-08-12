# Optimization Framework — Implementation Plan

## Scope

Build the optimization framework described in `docs/design/determinism-and-optimization-frontier.md` into the Briev compiler. Each phase is independent (later phases depend on earlier ones) and verifiable against the existing test suite.

---

## Phase 0: Tactical Gaps — Convergence Proof Robustness

Close three soundness gaps in the current `check_convergence` function and restructure its integration point.

### Tasks

| # | Task | Files | Verification |
|---|------|-------|-------------|
| 0.1 | **Pre-condition validation**: verify `post → ¬pre` structurally. Reject `[true][count == total]`, accept `[count < total][count == total]`. | `src/proof_engine.rs` | New unit tests for rejected/accepted patterns |
| 0.2 | **Relational post-condition matching**: add `Gte`, `Lte`, `Gt`, `Lt` arms | `src/proof_engine.rs` | Convergence recognized for `[count < total][count >= total]` |
| 0.3 | **Overshoot detection**: pass `initial_values: &HashMap<String, Expr>` to `check_convergence`, verify `(bound - init) % step == 0` when all values are compile-time literals | `src/proof_engine.rs` | `&count = count + 5` with `total = 7, init = 0` rejected |
| 0.4 | **Move convergence check** from `SymbolicExecutor::verify_transaction` to `ProofEngine::verify_contracts`, giving access to full program | `src/proof_engine.rs` | All 299 tests pass, IIR benchmark maintains speed |

### Design Notes

- `check_convergence` becomes a free function with signature `fn check_convergence(body, pre, post, initial_values) -> bool`
- `ProofEngine::verify_contracts` extracts `initial_values` from `TopLevel::StateDecl` and `TopLevel::Constant`
- Overshoot is conservatively accepted when step == 1, conservatively rejected when values aren't all compile-time known

---

## Phase 1: Atomic Region Analysis

Build the analysis pipeline that classifies variables and partitions the program into independent reactive regions.

### Tasks

| # | Task | Files | Verification |
|---|------|-------|-------------|
| 1.1 | **RegionAnalyzer** — trace dependency graph from `trg` roots through assignments, find connected components | `src/analysis/region.rs` (new module), `src/analysis/mod.rs` | Two unrelated `trg` vars produce two regions |
| 1.2 | **Variable classification** — classify variables as Pure / Bounded / Opaque | `src/analysis/region.rs` | Each category testable with known inputs |
| 1.3 | **Bound propagation** — propagate type-level and contract-level ranges through expressions | `src/analysis/region.rs` | `a / b` with zero-crossing `b` collapses to Opaque |
| 1.4 | **Value-set estimator** — compute state space size per frontier variable | `src/analysis/region.rs` | `Bool` → 2, `U8` with `[0..3]` → 4, unbounded → `None` |
| 1.5 | **Integration into ProofEngine** — proof engine can query "which region does txn X belong to?" and "what is variable Y's classification?" | `src/proof_engine.rs` | Regression tests pass |

### Design Notes

- `RegionAnalyzer` is a struct holding the computed region graph
- It sits in the analysis pipeline (between parsing/desugaring and proof checking)
- Bound propagation uses interval arithmetic (simple `(lo, hi)` pairs)
- Value-set estimation is the cheap part (traverses types and contracts, does not emit code)

---

## Phase 2: Value-Set Enumeration in the LLVM Backend

Concretize frontier variables and emit switch-dispatch fast paths.

### Tasks

| # | Task | Files | Verification |
|---|------|-------|-------------|
| 2.1 | **Region cloning** — given a region and one concretized frontier variable, fold the region with that variable set to a constant | `src/backend/llvm.rs` | `trg: Bool` → 2 `.ll` modules with `true`/`false` concretized |
| 2.2 | **Switch dispatch** — emit `switch(trg_val) { case v0: path_0; ... }` for enumerated paths | `src/backend/llvm.rs` | Multi-way branch in generated IR |
| 2.3 | **Residual fallback** — uncovered inputs fall through to segment-folded reactive execution | `src/backend/llvm.rs` | Partial coverage emits fallthrough block |
| 2.4 | **Integration** — wire enumeration into the compilation pipeline, controlled by budget | `src/main.rs`, `src/backend/mod.rs` | `briev-compiler llvm --optimize-budget 1000 input.bv` works end-to-end |

### Design Notes

- Cloning operates on the folded region schedule, not the AST — it's cheap
- The switch dispatch replaces the runtime reactive dispatcher for the enumerated portion
- The residual fallback reuses the existing segment-folding path

---

## Phase 3: Budget and Report System

CLI flags that control and visualize the optimization budget.

### Tasks

| # | Task | Files | Verification |
|---|------|-------|-------------|
| 3.1 | `--optimize-budget <N>` CLI flag | `src/main.rs` | Argument parsed, passed through pipeline |
| 3.2 | `--optimize-report` — print tradeoff table and exit | `src/main.rs`, `src/analysis/region.rs` | Output matches format in design doc |
| 3.3 | `--optimize-size <bytes>` — binary-search for optimal budget | `src/main.rs` | Resulting binary within size limit |
| 3.4 | Sweet-spot highlighting in report | `src/main.rs` | Report marks recommended budget(s) |

---

## Phase 4: Chain Equivalence (Stretch Goal)

Collapse multi-transaction chains into parallel schedules when the net effect is a known formula.

### Tasks

| # | Task | Files | Verification |
|---|------|-------|-------------|
| 4.1 | Linear transaction chain detection | `src/analysis/region.rs` | Z→A→B→C→X detected as single chain |
| 4.2 | Symbolic composition of linear transforms | `src/analysis/region.rs` | `X = f(g(h(Z)))` collapsed to `X = F(Z)` |
| 4.3 | Parallel schedule emission | `src/backend/llvm.rs` | Side effects and main result computed in same tick |

---

## Verification Strategy

Every phase must satisfy:

1. `cargo test --lib` — all 299+ existing tests pass
2. IIR benchmark — Briev maintains ≥ C speed (currently 0.14s vs 0.22s)
3. Phase-specific new tests (unit tests for new analysis, integration tests for new emission)

---

## File Changes Summary

| File | Phase | Change |
|------|-------|--------|
| `src/proof_engine.rs` | 0, 1 | Tighten convergence proof, add region analysis queries |
| `src/analysis/region.rs` | 1 | New module: RegionAnalyzer |
| `src/analysis/mod.rs` | 1 | Add `pub mod region;` |
| `src/backend/llvm.rs` | 2 | Region cloning, switch dispatch, residual fallback |
| `src/backend/mod.rs` | 2 | Enumeration pipeline integration |
| `src/main.rs` | 3 | Budget and report CLI flags |
| `docs/design/determinism-and-optimization-frontier.md` | — | Conceptual architecture (already written) |
