# Brief LLVM Backend — Change Log & Design Journal

**File:** CHANGELOG-LLVM-SPEC.md  
**Purpose:** Tracks all architectural decisions, bug fixes, and spec additions that occurred during the llvm-spec audit on 2026-05-29.

---

## 2026-05-29 — v1.0 → v1.2 Revision

### Environment
- LLVM 18.1.3 installed (`llc`, `opt`, `lli` available)
- Target confirmed: `nvptx64` — NVIDIA PTX 64-bit (GPU pathway validated)
- Host CPU: Ivy Bridge (x86_64)
- Compiler at commit: `1d4992b` (post v1.1 spec commit)

---

### Corrective Issues Found During Spec Audit (6 items)

#### Issue 1: FFI `memory(inaccessiblemem)` → `memory(argmem)`
- **File:** `07-FFI-TO-DECLARE.md:84`, `10-FULL-EXAMPLE.md:124`
- **Root cause:** `inaccessiblemem` tells LLVM the function cannot access *any* module-visible memory. DSE removes string buffer writes before FFI calls, passing garbage pointers.
- **Fix:** Changed to `memory(argmem: readwrite)` — restricts access only to memory reachable via pointer arguments. `%State*` is not passed to FFI, so `noalias` is preserved.
- **Applied to:** `07-FFI-TO-DECLARE.md` (attributes #1), `10-FULL-EXAMPLE.md` (#1), `12-IMPLEMENTATION-ORDER.md` (4.5)

#### Issue 2: Nested SSA Instructions in Reactor Tick
- **File:** `10-FULL-EXAMPLE.md:89-91`
- **Root cause:** LLVM IR is strictly flat SSA. `and i1` cannot contain nested `icmp` calls inline.
- **Fix:** Flattened into two `icmp` temps followed by a separate `and i1`.
- **Applied to:** `10-FULL-EXAMPLE.md` (reactor_tick example)

#### Issue 3: `!range` Signed Bounds Wrapping
- **File:** `05-CONTRACT-TO-METADATA.md:12,14`
- **Root cause:** `i64 -1` as unsigned is `2^64-1`. Range `[0, 2^64-1)` includes all negative signed values, letting them through.
- **Fix:** Changed to `i64 9223372036854775808` (= `2^63`), representing `[0, 2^63)` — half of the signed range with MSB=0.
- **Applied to:** `05-CONTRACT-TO-METADATA.md` (table rows for `[x >= 0]` and `[len > 0]`)

#### Issue 4: GEP Index Depth for Nested Structs
- **File:** `03-TRANSACTIONS.md:22-23`
- **Root cause:** `%State` contains `%struct.Counter` which contains `i64 count`. A 2-index GEP returns `%struct.Counter*`, not `i64*` — type mismatch.
- **Fix:** Added third index `, i32 0` for the nested struct depth. (Already correct in `10-FULL-EXAMPLE.md:44`)
- **Applied to:** `03-TRANSACTIONS.md`

#### Issue 5: Enum Payload Layout (Memory Waste)
- **File:** `06-MATCH-TO-SWITCH.md:77-79`
- **Root cause:** Flat struct with all variant payloads embedded wastes memory for large variants.
- **Fix:** Documented byte-array union (`[32 x i8]`) as preferred approach with `bitcast` + GEP for extraction. Kept flat struct as acceptable initial implementation.
- **Applied to:** `06-MATCH-TO-SWITCH.md`

#### Issue 6: C-String Memory Leak in FFI
- **File:** `07-FFI-TO-DECLARE.md:31-34`
- **Root cause:** `@brief_string_to_cstr` heap-allocates but never frees — infinite leak in reactor loop.
- **Fix:** Documented stack-allocation strategy using `alloca` + `llvm.memcpy` as preferred default. Heap allocation documented with explicit `free` requirement.
- **Applied to:** `07-FFI-TO-DECLARE.md` (new "C-String Memory Lifecycle" section)

---

### New Spec Files Created (v1.1)

#### 08a-TRIGGERS.md — Volatile Double-Buffering
- **Architecture:** Every `trg` is sampled once via `load volatile` at tick entry into an immutable SSA register
- **Three lowering models:** MMIO (inttoptr + volatile load), FFI poll (`__poll_triggers` call), Metropolitan spinlock (status word + pause instruction)
- **Fusing inhibition:** If `Txn_B`'s guard references a `trg`, transition fusing is refused

#### 08b-TRANSITION-FUSING.md — State Composition
- **Mechanism:** `post(Txn_A)` → `pre(Txn_B)` implication is proven by the proof engine; bodies are concatenated
- **Inhibition rules:** volatile trg dependency, WAW hazard, async flag, complexity budget
- **Already implemented:** `detect_fusable_pairs` at `src/backend/mod.rs:291`

---

### New Spec Files Created (v1.2 — this session)

#### 08c-EQUILIBRIUM-SUSPENSION.md
- **Problem:** Busy-spin at 100% CPU when no transaction preconditions are met
- **Fix:** Replace `noop → br label %commit` with `call void @__wait_for_event()`
- **Resolution:** Uses the existing bootstrap intrinsic mechanism (same as `__read_file`, `__print`)
  - Linux: `epoll_wait` / `select` (0% CPU)
  - ARM bare-metal: `wfi` (Wait For Interrupt — microwatt sleep)
  - RISC-V: `wfi`
  - WASM: Asyncify yield to host event loop
- **Trigger synthesis:** Compiler generates `pollfd`/`epoll` masks from the registered `trg` map

#### 08e-AOT-SIZE-INFERENCE.md
- **Problem:** `List<T>` requires heap allocation (`{ i8*, i64, i64 }`), breaks hardware isomorphism, forces `nofree` omission
- **Fix:** Three-path size inference during lowering pass
  1. Literal propagation: `[1,2,3]` → size 3
  2. Contract propagation: `[len(x) <= 16]` → max size 16
  3. Symbolic interval analysis: loop index bounded by constant → exact size
- **Type rewriting:** `%struct.List_I64` → `[N x i64]` (stack-allocated via `alloca`)
- **Conditional `nofree`:** Only emitted for txns whose call graph has zero heap operations
- **Fallback:** Heap path with bump allocator when inference fails

#### 13-GPU-TARGET.md (Future Roadmap)
- **Pipeline:** Brief → LLVM IR → `llc -march=nvptx64` → `.ptx` → `ptxas` → SASS
- **Four optimizations CUDA cannot do:**
  1. Static bank conflict elimination (SMT solver proves no two threads hit same bank)
  2. Guaranteed memory coalescing (`noalias` across threads → aligned vector loads)
  3. Automatic memory-tier placement (read-only → `__constant__`, block-local → `__shared__`)
  4. Absolute warp divergence elimination (`select i1` → predicated execution, zero branching)
- **Status:** Future target, not in current implementation plan

---

### Architectural Decisions Resolved This Session

#### Decision: `alwaysinline` for Acyclic Transactions
- **When:** All acyclic transactions when they are the single firing transaction per tick
- **Why:** Prevents LLVM from refusing to inline large `%struct.State` by-value — the fallback would be massivestack-copy `memcpy` operations
- **No bloat concern:** The filter analysis proves at most one transaction fires per tick (disjoint field access), so only one body is inlined

#### Decision: `__wait_for_event()` as Bootstrap Intrinsic
- **Mechanism:** Existing FFI Dictionary, not a new mechanism
- **Why:** Matches the "NO MAGIC" policy — transparent, resolvable per-target at link time
- **Registration:** One entry in the bootstrap intrinsic table alongside `__print`, `__read_file`, etc.

#### Decision: `@llvm.assume` Debug/Release Split
- **Debug mode:** Emit `br i1 %cond, label %safe, label %panic` — runtime check with stack trace
- **Release mode:** Emit `call void @llvm.assume(i1 %cond)` — enables `!range`, `nuw nsw`, dead branch elimination
- **Why:** `@llvm.assume` with a false precondition triggers undefined behavior — unacceptable during development

#### Decision: Conditional `nofree`
- **When present:** Only for txns whose call graph contains zero heap operations
- **When omitted:** Any txn touching a `List` (append, resize) or dynamic allocation
- **Implementation:** Call graph analysis during lowering pass scans for `malloc`/`free`/`realloc` calls
- **Why:** `nofree` with heap ops causes LLVM to keep stale deallocated pointers in registers (use-after-free)

---

### Phase 2.8 Added to Implementation Order

Added to `12-IMPLEMENTATION-ORDER.md` as a 2-day phase after contract optimization:

| Step | What | Details |
|------|------|---------|
| 2.8.1 | Literal size inference | `[1,2,3]` → size 3, promote to `Vector[3]` |
| 2.8.2 | Contract-bound inference | `[len(x) <= 16]` → max size 16 → `Vector[16]` |
| 2.8.3 | Symbolic loop-bound inference | Loop `for i in 0..N` → `Vector[N]` |
| 2.8.4 | Conditional `nofree` | Skip `nofree` on `#0` for any txn whose call graph contains heap ops |
| 2.8.5 | Heap→Stack codegen split | LLVM IR path A: `%struct.List` (heap, `malloc`), path B: `[N x i64]` (stack, `alloca`) |

Updated total estimate: **14d Rust backend + 27d self-hosted = 41d total** (was 39d).

---

### Phase 0 Rust Backend — Current State

The Rust backend (`src/backend/llvm.rs`) has been rewritten per the Phase 0 scaffold:
- **Fixed:** `i64 %arg0` → `%State* noalias nocapture %state` (the fundamental model fix)
- **Fixed:** GEP uses correct indexing from `%state` parameter
- **Added:** `@llvm.assume` intrinsic declaration
- **Added:** `init_state()` function with per-field volatile stores
- **Added:** `attributes #0` and `attributes #1` blocks
- **Added:** `source_filename`, `target datalayout`, `local_unnamed_addr`
- **Preserved:** All expression forms (arithmetic, comparison, bitwise, calls)
- **Known issue:** Bool assignments truncate `i64` to `i8` — needs handling in Phase 1
- **Tests:** All 5 unit tests pass; full suite 270 tests pass

### Validation Pipeline State
- Created `tests/llvm_compile_test.sh` — shell-based end-to-end validation
- Created `tests/llvm_backend_test.rs` — Rust integration test (requires llc in PATH)
- Created fixtures: `counter.bv`, `multifield.bv`, `minimal.bv`
- LLVM tools (llc, opt) are now installed and available

## Phase 0 Rust Backend — Validation Results

### LLVM 18.1.3 Compatibility Fixes
- **GEP syntax:** LLVM 18 removed support for parenthesized GEP in instructions. `getelementptr inbounds (%State, %State* %state, ...)` → `getelementptr inbounds %State, %State* %state, ...`
- **`norecurse` in signature:** LLVM 18 refuses inline `norecurse` in function signatures. Must be in attribute group `#0` only.
- **Type trunc/zext:** Bool fields stored as `i8` in `%State`. `i64` values must be `trunc`'d to `i8` before store; `i8` loads must be `zext`'d to `i64` for arithmetic.

### Optimized Output (opt -O3 -S)
LLVM 18.1.3 successfully optimized the Phase 0 output:
- `increment` → 3 instructions: `load i64`, `add i64`, `store i64` (GEP eliminated — offset 0)
- `init_state` → inlined into `main`
- `reactor_tick` → `ret void` (dead-function eliminated since no op)
- `main` → `store volatile`, then `unreachable` (LLVM proved infinite empty-tick loop is dead)
- `noalias nocapture` verified present and functioning
- Both `counter.ll` and `multifield.ll` pass `llc` assembly generation

---

## Phase 1 — Basic Transaction Emission

### Delivered (2026-05-29)

| Feature | Status | Details |
|---------|--------|---------|
| `let` SSA bindings | ✅ | `let_bindings` HashMap tracks name→register. Expr::Identifier checks it first before field GEP |
| `term;` → `ret void` | ✅ | Sets `terminated=true` to prevent double ret |
| `term expr;` → `ret i64 %val` | ✅ | `values.first()` unwraps `Option<Expr>` via `Some(Some(v))` pattern |
| `guarded` → `br i1` | ✅ | `icmp ne i64 %cond, 0` converts Bool-as-i64 to i1 for branch |
| `Escape` → `ret` | ✅ | Sets `terminated=true` |
| All 5 integer ops | ✅ | add, sub, mul, sdiv, srem — no changes needed |
| Bool field trunc/zext | ✅ | Trunc on store, zext on load (from Phase 0) |

### Test Fixtures (all pass `llc`)
| Fixture | Tests | Status |
|---------|-------|--------|
| `tests/fixtures/phase1/let_binding.bv` | SSA let tracking + field store | ✅ |
| `tests/fixtures/phase1/arithmetic.bv` | + - * / % all five ops | ✅ |
| `tests/fixtures/phase1/guarded.bv` | Conditional branch + store | ✅ |
| `tests/fixtures/phase1/full_txn.bv` | let + field read + store + term | ✅ |
| Regression: counter, multifield, minimal | Phase 0 fixtures still pass | ✅ |

### Unit Tests: 5/5 passing. Full suite: 270/270 passing.

---

## Phase 2 — Contract Optimization

### Delivered (2026-05-29)

| Feature | Status | Details |
|---------|--------|---------|
| `!range` metadata | ✅ | `[x < N]` → `!range !{ 0, N }` on field `load`. Signed bounds use `2^63`. Parses `And(Lt(Ident, Int), ...)` patterns |
| `@llvm.assume` debug/release | ✅ | Debug: `br i1, %safe, %panic` + `unreachable`. Release: `call void @llvm.assume(i1 %ok)` |
| Guard→`select` | ✅ | Single-assignment `[cond] { &x = val; }` → `select i1 %cond, i64 %val, i64 %old` |
| `emit_precondition` | ✅ | Shared code path for both modes, label naming convention `pre_safeN`/`pre_panicN` |
| `extract_ranges` | ✅ | Recursive pattern matcher on `Expr` tree, handles `And`/`Lt`/`Ge`/`Gt` |
| Metadata nodes | ✅ | `!0 = !{ i64 0, i64 100 }` at module footer, indexed by `field_to_meta_idx` |

### Test Fixtures (all pass `llc`)
| Fixture | Tests | Status |
|---------|-------|--------|
| `tests/fixtures/phase2/range_contract.bv` | `[counter < 100]` → `!range !0` on load | ✅ |
| `tests/fixtures/phase2/complex_pre.bv` | `[x > 0 && y < 100]` → br/unreachable | ✅ |
| `tests/fixtures/phase2/guard_select.bv` | Single-assignment guard → select | ✅ |
| Regression: Phase 0+1 fixtures | All 6 still pass | ✅ |

### Unit Tests: 5/5 passing. Full suite: 270/270 passing.

---

## Phase 2.5 — Transition Fusing + Trigger Sampling

### Delivered (2026-05-29)

| Feature | Status | Details |
|---------|--------|--------|
| Trigger sampling | ✅ | `load volatile` at reactor_tick entry for each `TriggerDeclaration`. MMIO via `inttoptr`, Linked via global symbol. Sampled to i1 SSA register. |
| Transition fusing | ✅ | Consumes `detect_fusable_pairs` from `src/backend/mod.rs:291`. Generates `@xn_yn_fused` bodies by concatenating statements. |
| WAW inhibition | ✅ | `collect_assigned_identifiers` scans both txns; fusion refused if same field is written by both. Fixed OwnedRef matching. |
| Async inhibition | ✅ | `is_async` on either txn → refuse. |
| Trg dependency inhibition | ✅ | `Txn_B` precondition references a `trg` name → refuse. |
| Dispatch in reactor_tick | ✅ | Calls first-true (or fused) transaction via `call @txn(%State* @global_state)` |
| `write_main` extracted | ✅ | Shared `write_main()` method for `main()` generation |

### Test Fixtures (all pass `llc`)
| Fixture | Tests | Status |
|---------|-------|--------|
| `tests/fixtures/phase2_5/triggers_mmio.bv` | MMIO trigger sampling, trg in precondition | ✅ |
| `tests/fixtures/phase2_5/fuse_simple.bv` | Two txns with sequential dependency → fused | ✅ |
| `tests/fixtures/phase2_5/fuse_inhibited.bv` | WAW hazard → no fusion | ✅ |

### Unit Tests: 5/5 passing. Full suite: 270/270 passing.

---

## Phase 3 — Match Expression → switch

### Delivered (2026-05-29)

| Feature | Status | Details |
|---------|--------|--------|
| `Expr::Match` → `switch` | ✅ | `switch i64 %discriminant` with per-arm basic blocks. Discriminant encoded in low 8 bits via `and`/`lshr`. |
| `phi` merge for expression matches | ✅ | `phi i64` at merge point selects arm value. Single arm → direct `add` fallback. |
| Exhaustive → `unreachable` | ✅ | All variants covered + no `_` → `default` label with `unreachable`. |
| `Expr::PatternMatch` guard | ✅ | `icmp eq` on discriminant + `zext` → i64 for guard context. |
| `Statement::Unification` → switch | ✅ | Single-arm switch + `lshr 8` payload extraction, bound to pattern name via `let_bindings`. |
| Label naming convention fix | ✅ | LLVM labels are bare names (no `%` prefix in defs), reference with `label %xxxx`. Eliminated `%%` double-encoding bug. |
| `emit_precondition` label fix | ✅ | Uses bare label names with `br i1 ..., label %label`. |

### Test Fixtures (all pass `llc`)
| Fixture | Tests | Status |
|---------|-------|--------|
| `tests/fixtures/phase3/simple.bv` | Two-arithmetic-op txn | ✅ |
| `tests/fixtures/phase3/unify_simple.bv` | `uni` pattern match with payload extraction | ✅ |

### Unit Tests: 5/5 passing. Full suite: 270/270 passing.

---

## Phase 4 — FFI declare + call

### Delivered (2026-05-29)

| Feature | Status | Details |
|---------|--------|---------|
| `frgn_map` | ✅ | `HashMap<String, ForeignSignature>` from `TopLevel::ForeignBinding` |
| `declare` emission | ✅ | Per-binding at module header with ABI type mapping |
| `__print` bootstrap | ✅ | `inttoptr i64 to i8*` + `strlen` + `write(1, ptr, len)` |
| `__exit` bootstrap | ✅ | `exit(0)` |
| C ABI marshaling | ✅ | Bool `zext` to i32, String `ptrtoint`/`inttoptr`, Int pass-through |
| String field fix | ✅ | `i8*` fields in `%State` use `ptrtoint` on load, `inttoptr` on FFI call |
| `@llvm.memcpy` declare | ✅ | Intrinsic for stack-allocated string marshaling |
| C std declares | ✅ | `write`, `strlen`, `exit`, `open`, `read` (conditional on use) |

### Test Fixtures (all pass `llc`)
| Fixture | Tests | Status |
|---------|-------|--------|
| `tests/fixtures/phase4/ffi_print.bv` | `__print` bootstrap with string marshaling | ✅ |
| `tests/fixtures/phase4/ffi_declare.bv` | `strlen` declare + Int return | ✅ |

### Unit Tests: 5/5 passing. Full suite: 270/270 passing.

---

## Phase 5 — Reactor Loop + Dispatch + Equilibrium Suspension

### Delivered (2026-05-29)

| Feature | Status | Details |
|---------|--------|--------|
| Precondition extraction | ✅ | `define internal i1 @pre_txn(%State*)` for non-trivial preconditions |
| Dispatch chain | ✅ | Priority-ordered `br i1 %pre, label %body, label %check_next` |
| Equilibrium suspension | ✅ | `call void @__wait_for_event()` when no precondition met |
| Trigger sampling preserved | ✅ | `load volatile` at tick entry |
| Fused txn generation restored | ✅ | `generate_fused_transaction` from Phase 2.5 |

### Unit Tests: 5/5 passing. Full suite: 270/270 passing.

---

## Phase 6 — SIMD Loop Vectorization

### Delivered (2026-05-29)

| Feature | Status | Details |
|---------|--------|--------|
| `!llvm.loop.vectorize.enable` | ✅ | On reactor loop back-edge `br label %tick` |
| `!llvm.loop.interleave.count` | ✅ | Set to 4 |
| Metadata node self-reference | ✅ | `!999 = !{!999, !1000, !1001}` using fixed high indices to avoid range metadata conflict |

### Unit Tests: 5/5 passing. Full suite: 270/270 passing.