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
| 2.3 | `!range` on loads from bounded preconditions | Parse `[x < N]` → emit `!range !{ 0, N }` using signed-correct upper bound (see CONTRACT-TO-METADATA.md) |
| 2.4 | `@llvm.assume` for complex preconditions | Parse multi-variable `[a && b]` → emit assume chain |
| 2.5 | `nuw nsw` on `add`/`sub` when bounds proven | If x in `[0, 100)`, x+1 is `nuw nsw` |
| 2.6 | Guard → `select` optimization | Single-assignment guards become `select i1` |
| 2.7 | Postcondition `@llvm.assume(i1 true)` | When proof engine proves postcondition |

**Dependencies**: Phase 1 (need working load/store first).

---

## Phase 2.5: Transition Fusing + Trigger Sampling (3 days)

**Goal**: Fuse guaranteed-sequential transactions into single-tick atomic transitions. Sample volatile triggers once per tick.

| Step | What | Details |
|------|------|---------|
| 2.5.1 | Consume `detect_fusable_pairs` from `analysis` | Read existing fusable pairs (src/backend/mod.rs:291) |
| 2.5.2 | Apply inhibition rules | Reject fusion if trg dependency, WAW hazard, async, or complexity budget exceeded |
| 2.5.3 | Body composition pass | Concatenate bodies of fused txns, merge pre/post conditions |
| 2.5.4 | Trigger sample phase at tick entry | Emit `load volatile` for each trg at top of `reactor_tick()` |
| 2.5.5 | Trigger classification | Route each trg to MMIO, FFI poll, or Metropolitan model |
| 2.5.6 | Wire `__poll_triggers` call in `main()` | For OS/WASM targets, insert poll call before `reactor_tick()` |

**Dependencies**: Phase 1 (transaction bodies) + Phase 2 (contract analysis).

---

## Phase 2.8: AOT Size Inference + Conditional Attributes (2 days)

**Goal**: Promote heap-allocated `List` to stack-allocated `Vector[N]` when size is provable. Conditionally emit `nofree` and `alwaysinline`.

| Step | What | Details |
|------|------|---------|
| 2.8.1 | Literal size inference | `[1,2,3]` → size 3, promote to `Vector[3]` |
| 2.8.2 | Contract-bound inference | `[len(x) <= 16]` → max size 16 → `Vector[16]` |
| 2.8.3 | Symbolic loop-bound inference | Loop `for i in 0..N` → `Vector[N]` |
| 2.8.4 | Conditional `nofree` | Skip `nofree` on `#0` for any txn whose call graph contains heap ops |
| 2.8.5 | `alwaysinline` for acyclic | Add `alwaysinline` to acyclic txn signatures |
| 2.8.6 | Heap→Stack codegen split | LLVM IR path A: `%struct.List` (heap, `malloc`), path B: `[N x i64]` (stack, `alloca`) |

**Dependencies**: Phase 2 (contract analysis for bounds inference).

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
| 4.5 | FFI attribute `memory(argmem: readwrite)` | Foreign calls don't alias `%State` |

**Dependencies**: Phase 1 (call instructions, SSA values).

---

## Phase 5: Reactor Loop + Acyclic Dispatch (2-3 days)

**Goal**: `main()` with tick loop, trigger sampling phase, precondition evaluation, inline dispatch for acyclic graphs.

| Step | What | Details |
|------|------|---------|
| 5.1 | `main()` → `init_state()` → tick loop | Basic infinite loop calling `reactor_tick()` |
| 5.2 | `init_state()` | Initialize all rstruct fields to zero/default |
| 5.3 | Precondition evaluation in tick | Call each txn's precondition, first-true wins |
| 5.4 | Acyclic: inline bodies | No `call` — bodies are inlined in the tick loop |
| 5.5 | Cyclic: dispatch table | `call` by function pointer, priority-sorted |
| 5.6 | Load state → SSA → phi → store | Full tick body with `extractvalue`/`insertvalue` |
| 5.7 | `norecurse` + `alwaysinline` on tick for acyclic | Enables LLVM to inline everything |
| 5.8 | Equilibrium `__wait_for_event()` in noop path | Suspend instead of busy-spin (see 08c-EQUILIBRIUM-SUSPENSION.md) |

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
| 7.4 | `llvm.bv` — transition fusing + triggers | Mirror Phase 2.5 |
| 7.5 | `llvm.bv` — AOT size inference | Mirror Phase 2.8 |
| 7.6 | `llvm.bv` — `match → switch` | Mirror Phase 3 |
| 7.7 | `llvm.bv` — FFI `declare` | Mirror Phase 4 |
| 7.8 | `llvm.bv` — reactor loop + equilibrium | Mirror Phase 5 |
| 7.9 | `main.bv` — wire `llvm` dispatch | Add `[state.backend == "llvm"]` arm |

**Dependencies**: All previous phases + Phase 2.5 (for fusing + trigger support) + working self-hosted `StringBuilder` and pattern matching.

---

## Effort Summary

| Phase | Rust Backend | Self-Hosted | Total |
|-------|-------------|-------------|-------|
| 0: Scaffold | 1d | 2d | 3d |
| 1: Basic txn | 2d | 3d | 5d |
| 2: noalias + contracts | 2d | 3d | 5d |
| 2.5: Fusing + Triggers | 2d | 3d | 5d |
| 2.8: AOT size inference | 1d | 2d | 3d |
| 3: Match → switch | 2d | 4d | 6d |
| 4: FFI declare | 1d | 2d | 3d |
| 5: Reactor loop | 3d | 3d | 6d |
| 6: SIMD | 2d | - | 2d |
| 7: Self-hosted parity | - | 5d | 5d |
| **Total** | **15d** | **29d** | **44d** |