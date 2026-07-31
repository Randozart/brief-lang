# Minimal-State and Loop Purity

**Date:** 2026-07-31
**Status:** Current — design for the minimal-state / loop-carried classification
**Scope:** How the compiler decides which variables are hot-loop state (phi registers), which are hoisted loop-invariants, and which are boundary-only materializations.

---

## 1. The Principle

LLVM vectorizes a loop only when it can prove there are **no cross-iteration dependencies**. Memory traffic through the `%State` struct (`load %state[i]` / `store %state[i]`) obscures that proof — LLVM must assume each load may alias each store, blocking SROA, LICM, and vectorization.

The compiler must therefore emit the **minimal loop-carried state set** across the backedge, with **zero %State memory traffic in the hot loop body**. This is the answer to "when do we keep a variable local, and when do we make it a state variable?":

- **Local** = the value is recomputed fresh each iteration from other locals/constants, OR it is loop-invariant and can be hoisted.
- **State** = the value must survive the backedge (its value in iteration N+1 depends on its value in iteration N), OR it is read at a different point than its write (by a convergence contract, a side-effecting guard, or a post-loop print).

## 2. The Four-Class Classification

For each top-level `let` field `f`, analyze its use-def position relative to the loop:

| Class | Condition | Hot-loop storage |
|---|---|---|
| **Loop-invariant** | never written in the loop | NOT a phi — hoist to a register before the loop (load from %State once, or fold if constant) |
| **Loop-carried** | written in iteration N, read in iteration N+k (k≥1) | **phi node** — the value crosses the backedge |
| **Boundary-only** | written in the loop, read only by a guard / post-condition / post-loop print | phi in the hot loop, materialized to %State **once at the boundary** (inner_exit), not every iteration |
| **Dead** | written, never read | eliminate (keep only if ABI/observability requires) |

Body-local `let`s are always pure registers — computed and consumed within one iteration.

## 3. The Decision Rule

```
f is a hot-loop state field (phi)  ⟺  (W(f) ∧ R_later(f)) ∨ R_contract(f) ∨ R_observable(f)
f is loop-invariant                ⟺  ¬W(f)                        → hoist
f is boundary-only                 ⟺  W(f) ∧ (R_guard(f) ∨ R_post(f)) ∧ ¬R_later(f)
                                       → phi + one %State store at boundary
f is dead                          ⟺  W(f) ∧ ¬R_any(f)             → drop
```

Where:
- `W(f)` = f written in the loop body
- `R_later(f)` = f read in a later iteration (the value must survive the backedge)
- `R_contract(f)` = f read by `[pre]`/`[post]` (the convergence contract)
- `R_observable(f)` = f read by a side-effecting guard or post-loop print
- `R_guard(f)` / `R_post(f)` = f read only by the guard / post-loop

## 4. Why This Matters

### 4.1 The Purity Guarantee

The hot loop body has **zero %State load/store**. All values live in phi registers or locals. %State writes happen only at the boundary — once per batch, in the inner_exit block.

### 4.2 What the Current Code Over-Approximates

`build_field_index` (`mod.rs`) makes **all** top-level `let`s state fields. `needs_state_stores_in_body` can force a %State store **every iteration** (for post-loop hoisted prints). Both block purity:

- Loop-invariant fields (never written in the loop) get a phi or a per-iteration load, forcing them through %State when they should be hoisted to a preheader register.
- Boundary-only fields get materialized to %State every iteration instead of once at the boundary.

The minimal-state pass corrects both.

## 5. Interaction with Composite-Node Decomposition

The recursive version-DAG decomposition (`docs/plans/2026-07-30-flat-node-decomposition.md` §11) and the minimal-state classification work together:

- The **guard-absent loop** carries only the minimal loop-carried set in phis → pure, vectorizable, no %State traffic.
- The **guard-present block** (boundary) materializes the boundary-read values to %State once → the side effect reads the correct post-compute state.
- **Loop-invariant fields** are hoisted out entirely — never touch %State in the loop.

The %State struct remains the ABI/boundary representation; the hot loop uses the minimal register-resident set. Boundary materialization (inner_exit store) is where state crosses between the pure loop and the observable world.

## 6. Implementation

### 6.1 The Analysis Pass

A liveness / loop-carried analysis pass (`src/analysis/`) classifies each state field:

```
classify_field(f, body, contract):
  W = written_in_loop(f, body)
  R_loop = read_in_loop(f, body)            # any read, including after a write
  R_later = read_in_later_iteration(f, body) # via a loop-carried use
  R_contract = read_in_contract(f, contract)  # [pre] or [post]
  R_guard = read_by_side_effecting_guard(f, body)
  R_post = read_by_post_loop(f, body)
  R_any = R_loop || R_contract || R_guard || R_post

  if !W: return LoopInvariant
  if R_contract || R_observable: return LoopCarried   # must survive
  if R_later: return LoopCarried
  if R_guard || R_post: return BoundaryOnly
  return Dead
```

### 6.2 Emission

| Class | Emission |
|---|---|
| Loop-invariant | hoist to preheader register (single %State load or folded constant) |
| Loop-carried | phi node in the loop header (existing PerFieldPhi) |
| Boundary-only | phi in the loop header + **single** %State store in the inner_exit block |
| Dead | skip (no phi, no store) |

### 6.3 The Purity Check

After emission, assert the hot loop body has **no %State load/store** — only phi registers and locals. This makes the "pure loop" a **verified invariant**, not an accident. A `debug_assert!`/diagnostic that counts `getelementptr %State` instructions in the hot loop body catches regressions.

## 7. Common Pitfalls

| Mistake | Failure mode |
|---|---|
| Making every top-level `let` a state field | %State memory traffic in the hot loop blocks SROA/LICM/vectorization |
| Materializing boundary-only fields every iteration | The store-per-iteration cost adds memory ops to the pure loop |
| Hoisting a field that is actually read later (missed loop-carried use) | Wrong value — must keep the phi |
| Dropping a field that is observable (post-loop print) | Missing output — must keep boundary materialization |
| Forgetting a field read by `[pre]`/`[post]` | Contract violation — the field must survive to the convergence check |

## 8. Relationship to Other Design Elements

- **Flat allocas** (out of scope for the decomposition plan): per-field allocas would let LLVM SROA choose promotion; the minimal-state classification is the front-end's explicit version of that decision.
- **PerFieldPhi emission** (`counter.rs::emit_countable_main`): already promotes written fields to phis; the classification tells it which fields to include and which to hoist/materialize.
- **Composite-node decomposition** (§5 above): the version-DAG's guard-present/guard-absent split relies on the classification to keep the absent loop pure and materialize the present block's boundary reads.

## 9. See Also

- `docs/plans/2026-07-30-flat-node-decomposition.md` §12 — the plan's minimal-state section
- `docs/architecture/backend-architecture.md` §5.3 — composite-node decomposition
- `src/backend/llvm/loop_engine/counter.rs` — PerFieldPhi and batch-loop emission
