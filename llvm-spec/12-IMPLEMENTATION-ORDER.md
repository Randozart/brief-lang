# Implementation Order

## Phase 0: Scaffold (1-2 days)

**Goal**: `brief llvm file.bv` runs and produces a `.ll` file.

| Step | Rust (`src/backend/llvm.rs`) | Self-Hosted (`lib/compiler/backends/llvm.bv`) |
|------|------------------------------|-----------------------------------------------|
| 0.1 | Ensure `generate()` writes module ID, target triple, data layout | Create `llvm.bv` with entry point function |
| 0.2 | Emit `%struct.State` type with all rstruct fields | Wire `llvm` case into `main.bv` backend dispatch |
| 0.3 | Emit `@global_state = global %struct.State` | Create `lowering.bv` for AST helpers |
| 0.4 | Emit `main()` with tick loop stub | Verify `selfhost` produces `.ll` output |

**Dependencies**: None. Pure boilerplate.

---

## Phase 1: Basic Transaction Emission (2-3 days)

**Goal**: A single `txn` with `Int` fields compiles to correct LLVM IR.

| Step | What | Details |
|------|------|---------|
| 1.1 | `txn → define void @name(%State*)` | Function signature, basic block, `ret void` |
| 1.2 | Field GEP + load | `getelementptr inbounds` + `load` for each field used |
| 1.3 | `let` assignments | SSA register per `let`, no memory stores |
| 1.4 | `&field = expr` | `store` instruction |
| 1.5 | `term` | `ret void` or `ret i64 %val` |
| 1.6 | Integer arithmetic | `add`, `sub`, `mul`, `sdiv`, `srem` |
| 1.7 | Comparison + guards | `icmp` + `br i1 %cond, label %then, label %end` |

**Dependencies**: Phase 0.

---

## Phase 2: `noalias` + Contract Optimization (2 days)

**Goal**: Every `%State*` parameter gets `noalias` + `nocapture`. Preconditions inject `!range` and `@llvm.assume`.

| Step | What | Details |
|------|------|---------|
| 2.1 | `noalias nocapture` on all `define` | Add attributes to function signature |
| 2.2 | `local_unnamed_addr` + `mustprogress` etc. | Attributes #0 block |
| 2.3 | `!range` on loads from bounded preconditions | Parse `[x < N]` → emit `!range !{ 0, N }` |
| 2.4 | `@llvm.assume` for complex preconditions | Parse multi-variable `[a && b]` → emit assume chain |
| 2.5 | `nuw nsw` on `add`/`sub` when bounds proven | If x in `[0, 100)`, x+1 is `nuw nsw` |
| 2.6 | Guard → `select` optimization | Single-assignment guards become `select i1` |
| 2.7 | Postcondition `@llvm.assume(i1 true)` | When proof engine proves postcondition |

**Dependencies**: Phase 1 (need working load/store first).

---

## Phase 3: Match Expression → `switch` (2-3 days)

**Goal**: `match val { V1(x) => ..., V2 => ..., _ => ... }` generates `switch i64 %discriminant`.

| Step | What | Details |
|------|------|---------|
| 3.1 | Discriminant load | Load the i64 discriminant from the enum |
| 3.2 | `switch` with arm labels | Default label for `_ =>`, explicit labels for variants |
| 3.3 | `extractvalue` for variant fields | Extract payload from the union at the variant offset |
| 3.4 | `phi` for expression-returning match | Merge point selects the right arm's value |
| 3.5 | Guard support | Branch to next arm if guard fails |
| 3.6 | `unreachable` for exhaustive match | If all variants covered + no `_ =>`, default = `unreachable` |

**Dependencies**: Phase 1 (SSA values + branching).

---

## Phase 4: FFI `declare` + `call` (1-2 days)

**Goal**: `frgn strlen(s: String) -> Int from "libc.so.6"` generates `declare i64 @strlen(i8*)` and call sites.

| Step | What | Details |
|------|------|---------|
| 4.1 | `declare` from `ForeignBinding` | Module-level `declare` for each frgn |
| 4.2 | C ABI argument marshaling | String → `i8*`, Bool → `i32`, etc. |
| 4.3 | `call` instruction at call sites | `call i64 @name(i64 %arg0, ...)` |
| 4.4 | Return value unwrapping | `frgn` returns `Result<T,E>` → unwrap to `T` in IR |
| 4.5 | FFI attribute `memory(inaccessiblemem: readwrite)` | Foreign calls don't alias `%State` |

**Dependencies**: Phase 1 (call instructions, SSA values).

---

## Phase 5: Reactor Loop + Acyclic Dispatch (2-3 days)

**Goal**: `main()` with tick loop, precondition evaluation, inline dispatch for acyclic graphs.

| Step | What | Details |
|------|------|---------|
| 5.1 | `main()` → `init_state()` → tick loop | Basic infinite loop calling `reactor_tick()` |
| 5.2 | `init_state()` | Initialize all rstruct fields to zero/default |
| 5.3 | Precondition evaluation in tick | Call each txn's precondition, first-true wins |
| 5.4 | Acyclic: inline bodies | No `call` — bodies are inlined in the tick loop |
| 5.5 | Cyclic: dispatch table | `call` by function pointer, priority-sorted |
| 5.6 | Load state → SSA → phi → store | Full tick body with `extractvalue`/`insertvalue` |
| 5.7 | `norecurse` on tick for acyclic | Enables LLVM to inline everything |

**Dependencies**: Phase 1 + Phase 2 (working transactions with preconditions).

---

## Phase 6: SIMD Vectorization (2-3 days)

**Goal**: Array loops with `!llvm.loop.vectorize.enable` metadata for SIMD vectorization.

| Step | What | Details |
|------|------|---------|
| 6.1 | Vector load/store | `<N x T>` type loads with alignment |
| 6.2 | Loop metadata | `!llvm.loop.vectorize.enable` + `!llvm.loop.interleave.count` |
| 6.3 | Element extraction | `extractelement` for individual array access |

**Dependencies**: Phase 1 (loops, basic blocks).

---

## Phase 7: Self-Hosted Parity (1 week)

**Goal**: `brief-compiler selfhost file.bv --target llvm` produces the same `.ll` as `brief llvm file.bv`.

| Step | What | Details |
|------|------|---------|
| 7.1 | `llvm.bv` — module header + `%State` type | Mirror Phase 0 in Brief |
| 7.2 | `llvm.bv` — load/store/arith | Mirror Phase 1 |
| 7.3 | `llvm.bv` — `noalias` + `!range` | Mirror Phase 2 |
| 7.4 | `llvm.bv` — `match → switch` | Mirror Phase 3 |
| 7.5 | `llvm.bv` — FFI `declare` | Mirror Phase 4 |
| 7.6 | `llvm.bv` — reactor loop | Mirror Phase 5 |
| 7.7 | `main.bv` — wire `llvm` dispatch | Add `[state.backend == "llvm"]` arm |

**Dependencies**: All previous phases + working self-hosted `StringBuilder` and pattern matching.

---

## Effort Summary

| Phase | Rust Backend | Self-Hosted | Total |
|-------|-------------|-------------|-------|
| 0: Scaffold | 1d | 2d | 3d |
| 1: Basic txn | 2d | 3d | 5d |
| 2: noalias + contracts | 2d | 3d | 5d |
| 3: Match → switch | 2d | 4d | 6d |
| 4: FFI declare | 1d | 2d | 3d |
| 5: Reactor loop | 2d | 3d | 5d |
| 6: SIMD | 2d | - | 2d |
| 7: Self-hosted parity | - | 5d | 5d |
| **Total** | **12d** | **22d** | **34d** |