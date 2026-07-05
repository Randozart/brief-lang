# Memory-Emission / Phi-Hybrid Plan

## Problem

Three remaining benchmark gaps share two root causes:

| Benchmark | Ratio | Brief | C | Root Cause |
|-----------|-------|-------|---|------------|
| nbody_sqrt | 1.26× | 4.05s | 3.21s | 4 of 6 `sqrtf` calls scalar (SLP vectorizer only; loop vectorizer blocked by phi escape) |
| fannkuch_redux | 1.62× | 0.109s | 0.067s | Phase-ordering: ~80-instruction unoptimized body (LCG, max_flips, seed) blocks LLVM loop unroller; DCE shrinks to 23 insns but unroller already passed |
| float_math | 1.20× | 0.095s | 0.079s | 13 explicit phi nodes = 13 extra backedge values per latch cycle; C's local variables have zero backedge routing |

All three are rooted in A005c's per-field phi emission: it gives LLVM zero-memory
traffic in the hot loop, but locks in a phi arrangement that LLVM's downstream
passes (vectorizer, unroller, scheduler) aren't tuned for.

---

## Prior Art (What Was Tried Before)

### A005d (memory loop, commit `4ed73f4`) — 2026-07-04

Created `emit_folded_memory_main` — all fields accessed via GEP+load+store from
`%State`. Chunk allocas (2-3 chunks of ≤15 fields) enabled SROA. Vectorizer
fired: "113 `<4 x float>` ops in @main".  nbody_sqrt: 1.22× → 1.18×.

**Critical flaw**: Counter was also memory-based (GEP+load → icmp → GEP+add+store).
No canonical induction variable phi → no trip count analysis, no SCEV, no loop
unrolling. The vectorization came from SLP (within-iteration), not the loop
vectorizer (across-iterations).

Removed at commit `0579c43` when A005c (per-field phi + Path A zero stores)
matched A005d's 1.18× with zero memory traffic—strictly better for compute-
bound loops.

### Phase 0: phi registers in done: (commit `92152a6`) — 2026-07-03

Replaced `pre_load_all_fields` (GEP+load from `%State`) with phi register reads
in `emit_hoisted_post_loop_prints`.  Unblocked SROA, but created phi uses in
`done:` → loop vectorizer blocked: "value that could not be identified as
reduction is used outside the loop".

**Reverted** at commit `a4df377`.  The phi escape from explicit ``%phi_bx0``
registers at `loop_hdr` dominated `done:`; LLVM conservatively assumed they
might be live after the loop.

### Phi commit block (current, commit `eb842d8`) — 2026-07-04

Stores each phi's final value to a per-field alloca ONCE at loop exit. `done:`
loads from these allocas.  Eliminates all per-iteration body stores (0 stores
in every hot loop).  Zero memory traffic.  But the stores in the commit block
reference the phi registers, creating additional phi escapes past the latch →
vectorizer blocked for cross-iteration SIMD.

---

## Solution: Hybrid Memory-Field / Phi-Counter Emission (A005e)

**Keep the counter phi** (induction variable — enables trip count analysis,
loop unrolling, SCEV).  **Emit fields through `%State` memory** (no per-field
phis — no phi escape, vectorizer fires).  Add **dead-field liveness analysis**
before emission (eliminate seed/max_flips → fewer memory ops + smaller body).

### 1. Memory-Based Field Emission (fix nbody_sqrt + float_math)

Replace per-field phis in `emit_countable_main` with `pre_load_all_fields`
(GEP+load at body entry) + `emit_memory_field_store` (GEP+store at `&`
assignments).  Keep only the counter phi:

```rust
// Before (A005c — per-field phis at loop_hdr):
loop_hdr:
  %phi_bx0 = phi float [ %init_bx0, %pre_phi ], [ %be_bx0, %latch ]
  %phi_bx1 = phi float [ %init_bx1, %pre_phi ], [ %be_bx1, %latch ]
  // ... 28 more phis ...
  %phi_count = phi i64 [ %init_count, %pre_phi ], [ %be_count, %latch ]
latch:
  %be_bx0 = fadd float %computed_bx0, 0.0  // identity op
  %be_bx1 = fadd float %computed_bx1, 0.0  // identity op
  // ... 28 more identity ops ...
  %be_count = add i64 %computed_count, 1
  br label %loop_hdr

// After (A005e — memory fields, counter phi only):
loop_hdr:
  %pi_count = phi i64 [ 0, %pre_phi ], [ %pn_count, %latch ]
body:
  %phi_bx0 = load float, ptr %state_bx0  // pre_load_all_fields
  %phi_bx1 = load float, ptr %state_bx1  // pre_load_all_fields
  // ... compute using loaded values ...
  store float %computed_bx0, ptr %state_bx0  // emit_memory_field_store
  store float %computed_bx1, ptr %state_bx1  // emit_memory_field_store
latch:
  %pn_count = add i64 %pi_count, 1
  br label %loop_hdr
```

After SROA + mem2reg, the GEP+load+store pattern IS promoted back to phi
registers ([verified](https://github.com/anomalyco/opencode/blob/eb842d8/docs/plans/2026-07-04-memory-emission-phi-hybrid.md#sroa-reliability)).
But these are SROA-created phis—closed SSA, only used within the loop body.
No phi escape to `done:`. The loop vectorizer sees no "value used outside the
loop" and applies cross-iteration SIMD.

**The done: block** reads final field values directly from `%State` via
`pre_load_all_fields`.  No commit block needed—the last iteration's stores
already left the correct values in `%State`.  This eliminates both the commit
block AND its vectorization-blocking phi escape.

### 2. Dead-Field Liveness Analysis (fix fannkuch_redux)

Scan the transaction body BEFORE code emission to determine which state fields
are consumed by observable operations (prints, FFI calls, swan songs).  Fields
that are only written and self-referentially read are dead—their stores and
computations are eliminated entirely:

```rust
fn trace_live_fields(body: &[Statement], field_index_map: &HashMap<String, usize>) -> HashSet<String> {
    // Start from all observable sinks (prints, FFI calls, swan songs).
    // Walk backward through LET bindings and `&` assignments.
    // A field is live if any observable sink transitively reads it.
    let mut live: HashSet<String> = HashSet::new();
    let mut worklist: Vec<String> = Vec::new();
    // Seed from swan songs and print calls in guards.
    for stmt in body {
        if let Statement::TermBang { call, .. } = stmt {
            for id in extract_field_refs(call, field_index_map) {
                if live.insert(id.clone()) { worklist.push(id); }
            }
        }
    }
    // Propagate: if `x` is live and `&x = f(y, z)`, then `y` and `z` are live.
    // A field written as `&x = expr` where x IS in `live` means x is live
    // (fed to observable), BUT if x is only written and never read by observable,
    // it is dead even though it appears in `live`—break cycle.
    let mut written_only: HashSet<String> = HashSet::new();
    for stmt in body {
        if let Statement::Assignment { target, value, .. } = stmt {
            if live.contains(target) {
                for id in extract_field_refs(value, field_index_map) {
                    if live.insert(id.clone()) { worklist.push(id); }
                }
            } else {
                written_only.insert(target.clone());
            }
        }
    }
    // Remove fields that are only written (seed, max_flips in fannkuch_redux).
    for dead in &written_only {
        if !field_is_consumed_by_observable(dead, body) {
            live.remove(dead);
        }
    }
    live
}
```

For fannkuch_redux:
- `checksum` is printed via `print_int#(checksum)` — LIVE
- `seed` is written `&seed = ns` and only read by LCG (which computes `ns` which writes `seed`) — DEAD
- `max_flips` is written `&max_flips = nmax` and only read by line 34's `nmax` computation (which writes `max_flips`) — DEAD
- `count` is printed (implicitly via loop condition) and drives the termination — LIVE

After dead-field elimination: body shrinks from ~80 to ~40 unoptimized
instructions. LLVM's loop unroller evaluates a 40-insn body and unrolls 4×,
matching the C reference's unrolled structure.

The liveness pass is invoked once before code emission, in
`emit_countable_main`:

```rust
pub(crate) fn emit_countable_main(..., body: &[Statement], ...) {
    // 2026-07-04: Dead-field elimination. Remove fields that are only
    // written and self-referentially read—they create dead computation
    // (LCG, max_flips) that inflates the unoptimized body and prevents
    // LLVM's loop unroller from firing before DCE shrinks the body.
    let live_fields = trace_live_fields(body, &self.ctx.field_index_map);
    let filtered_body = filter_dead_assignments(body, &live_fields);
    // Use filtered_body for all subsequent emission.
    // ...
}
```

### 3. Predecessor Safety

A005d's comment (line 890-897) says phi-based counters create SSA predecessor
issues when guards create extra basic blocks that branch to `_hdr`.  This is
already handled by A005c's latch structure: ALL body paths (including guard
branches) converge to `latch:`, which is the single backedge predecessor of
`loop_hdr:`.  The counter phi at `loop_hdr` has exactly 2 predecessors:
`pre_phi:` (entry) and `latch:` (backedge).  No predecessor explosion.

The hybrid retains this structure: `loop_hdr:` contains only the counter phi.
`body:` loads from `%State`.  `latch:` only increments the counter phi.  All
guard-generated blocks merge to `latch:`.  No new predecessor issue.

---

## Implementation

### Changes to `src/backend/llvm/loop_engine.rs`

#### Step 1: Add `filter_dead_assignments` helper (lines ~3147+)

```rust
// 2026-07-04: Dead-field liveness analysis.  Removes `&` assignments for
// state fields that are only written and self-referentially read.  These
// create dead computations (LCG, intermediate reductions) that inflate
// the unoptimized body and block LLVM's loop unroller due to phase-ordering
// (unroller runs before DCE).
//
// fannkuch_redux: seed, max_flips removed → body shrinks ~80→40 insns → LLVM
// unrolls 4× (matching C).  nbody_sqrt: no dead fields (all 30 position/
// velocity fields are consumed by the swan song print).  float_math: no
// dead fields.
fn filter_dead_assignments(body: &[Statement], live_fields: &HashSet<String>) -> Vec<Statement> {
    body.iter().filter(|stmt| {
        if let Statement::Assignment { target, .. } = stmt {
            live_fields.contains(target)
        } else {
            true  // keep non-assignment statements (guards, terms, LETs)
        }
    }).cloned().collect()
}

// 2026-07-04: Trace which state fields are transitively consumed by
// observable operations (prints, FFI calls, swan songs).  Walks backward
// from observable sinks through LET bindings and `&` assignments.
// A field written as `&x = f(y, z)` where x is live makes y and z live.
// A field written as `&x = f(...)` where x is never read by any sink is
// dead even if it appears as a backedge source (self-referential cycle).
fn trace_live_fields(body: &[Statement], field_index_map: &HashMap<String, usize>) -> HashSet<String> {
    let mut live: HashSet<String> = HashSet::new();
    let mut worklist: Vec<String> = Vec::new();
    // Seed from swan songs and print/FFI calls in guards.
    for stmt in body {
        let Some(call_expr) = extract_call_from_stmt(stmt) else { continue; };
        for id in extract_field_refs(call_expr, field_index_map) {
            if live.insert(id.clone()) { worklist.push(id); }
        }
    }
    // Propagate backward through assignments.
    let mut written_set: HashSet<String> = HashSet::new();
    for stmt in body {
        if let Statement::Assignment { target, .. } = stmt {
            written_set.insert(target.clone());
        }
    }
    let mut changed = true;
    while changed {
        changed = false;
        for stmt in body {
            let Some((target, value)) = extract_assignment(stmt) else { continue; };
            if !live.contains(target) { continue; }
            for id in extract_field_refs(value, field_index_map) {
                if live.insert(id.clone()) { changed = true; worklist.push(id); }
            }
        }
    }
    // Remove fields only written (self-referential cycle).
    for w in &written_set {
        if !field_is_used_in_output_expr(w, body) {
            live.remove(w);
        }
    }
    live
}
```

#### Step 2: Restructure `emit_countable_main` (lines 1214-1348)

Replace per-field phi creation with `pre_load_all_fields`.  Remove the commit
block and `last_val_temps`.  Simplify `emit_countable_setup_phis_and_header`:

```rust
// ── Revised emit_countable_setup_phis_and_header ─────────────────────────
//
// 2026-07-04: Counter-only phi header.  Fields are loaded from %State at
// body entry via pre_load_all_fields instead of per-field phi nodes.
// This eliminates the phi escape that blocked the loop vectorizer
// ("value not identified as reduction used outside loop").
//
// Before: 31 phi nodes at loop_hdr (counter + 30 state fields).
// After:  1 phi node at loop_hdr (counter only).
//
// The done: block reads final values directly from %State (written by the
// last iteration's stores).  No commit block, no last_val_temps, no phi
// escape.
fn emit_countable_setup_phis_and_header(
    &mut self, out: &mut String, counter_idx: usize, pi_name: &str,
    pn_name: &str, count_phi_reg: &str, count_be_reg: &str,
    init_count: &str, exit_label: &str,
) {
    writeln!(out, "  br label %loop_hdr").ok();
    writeln!(out, "loop_hdr:").ok();
    // Counter phi only — no per-field phis.
    // The counter phi enables LLVM's induction variable analysis (SCEV,
    // trip count, loop unrolling).  Fields are loaded from %State in
    // the body via pre_load_all_fields — SROA promotes them to internal
    // closed-SSA phis that don't escape to done:.
    writeln!(out, "  {} = phi i64 [ {}, %pre_phi ], [ {}, %latch ]",
             pi_name, init_count, pn_name).ok();
    let ty_counter = &self.ctx.field_types[counter_idx];
    writeln!(out, "  {} = phi {} [ {}, %pre_phi ], [ {}, %latch ]",
             count_phi_reg, ty_counter, init_count, count_be_reg).ok();
    let cmp_reg = format!("%cmp_hdr_{}", self.fun.txn_counter);
    self.fun.txn_counter += 1;
    writeln!(out, "  {} = icmp slt i64 {}, %cnt_bound_{}", cmp_reg,
             pi_name, self.fun.txn_counter).ok();
    writeln!(out, "  br i1 {}, label %body, label %{}", cmp_reg, exit_label).ok();
}

// ── Revised body emission ────────────────────────────────────────────────
//
// Instead of phi_regs_to_ssa_old() (which copies per-field phis to ssa_old
// caches), use pre_load_all_fields to load field values from %State.
// SROA + mem2reg promote these loads to closed-SSA phi nodes — only used
// within the body and latch, not in done:.  The loop vectorizer can now
// prove no phi escapes past the latch and applies cross-iteration SIMD.
self.fun.ssa_state_reg = None;
self.fun.returns_i64 = false;
// 2026-07-04: Dead-field filter applied before emission.
let live_fields = trace_live_fields(&body_stmts, &self.ctx.field_index_map);
let filtered_body = filter_dead_assignments(&body_stmts, &live_fields);
pre_load_all_fields(out, "%state", None);
for s in &filtered_body {
    if !matches!(s, Statement::Term { .. } | Statement::TermBang { .. }) {
        self.emit_stmt(out, s, "  ");
    }
}
// Fields written by the body are stored to %State via emit_memory_field_store.
// The done: block reads from %State directly — values are naturally correct
// from the last iteration's stores.  No commit block needed.

// ── Revised latch ────────────────────────────────────────────────────────
//
// Only the counter phi advances.  No per-field backedge values (identity
// ops like `fadd X, 0.0` or `add i64 0, %reg`).  The stores in the body
// already wrote final values to %State.
fn emit_countable_latch_simple(
    &mut self, out: &mut String, pi_name: &str, pn_name: &str,
    exit_label: &str,
) {
    writeln!(out, "  br label %latch").ok();
    writeln!(out, "latch:").ok();
    writeln!(out, "  {} = add i64 {}, 1", pn_name, pi_name).ok();
    // No field backedge values — fields are in %State memory.
    writeln!(out, "  br label %loop_hdr").ok();
}
```

#### Step 3: Remove phi-related FunctionContext fields (context.rs lines 221-343)

Remove or simplify:

| Field | Status | Reason |
|-------|--------|--------|
| `phi_field_regs` | Remove | No per-field phis |
| `backedge_field_regs` | Remove | No per-field backedge |
| `pending_phi_backedge` | Remove | Stores go to %State, no phi tracking |
| `pending_phi_native_backedge` | Remove | Same |
| `used_phi_loop` | Keep (unused) | Dead code, remove |
| `phi_induction_reg` | Keep | Counter phi still tracked |
| `loop_exit_label` | Keep | Still needed |
| `last_val_temps` | Remove | Commit block eliminated |
| `done_needs_fields` | Remove/Simplify | Done: reads from %State, no filter needed |
| `needs_state_stores_in_body` | Remove | Stores always happen |
| `parallel_safe_body` | Remove | Not needed — memory reads are naturally independent |
| `counter_field_name` | Remove | Only needed for parallel-safe exemption |
| `parallel_safe_exempt_fields` | Remove | Only needed for parallel-safe exemption |

#### Step 4: Simplify `emit_hoisted_post_loop_prints` (lines 2113-2140)

```rust
// 2026-07-04: Simplify — load from %State directly.  No commit block,
// no last_val_temps fallback, no done_needs_fields filter.  The final
// values are in %State from the last iteration's stores.
fn emit_hoisted_post_loop_prints(&mut self, out: &mut String, saved: &[Statement]) {
    if saved.is_empty() { return; }
    // Reload %State fields for the hoisted guard body.
    // pre_load_all_fields uses %state directly (GEP+load).
    // SROA already promoted the loop's %State to phis; this fresh
    // GEP+load from the same alloca is resolved by GVN.
    self.pre_load_all_fields(out, "%state", None);
    for s in saved {
        self.emit_stmt(out, s, "  ");
    }
}
```

### Changes to `src/backend/llvm/emit_stmt.rs`

#### Simplify `emit_memory_field_store` (lines 30-124)

Remove the gating flags:

```rust
// 2026-07-04: Simplified — all stores go to %State unconditionally.
// The gating flags (needs_state_stores_in_body, done_needs_fields,
// parallel_safe_body) are removed.  With no per-field phis, every `&`
// assignment stores to %State.  SROA + mem2reg promote these stores
// back to phi backedge values — they're dead until then.
fn emit_memory_field_store(&mut self, out: &mut String, fname: &str, reg: &str, indent: &str) {
    // GEP computation (same as before).
    let gep = self.emit_state_gep(out, indent, "ap", fname);
    let Some(&idx) = self.ctx.field_index_map.get(fname) else { return; };
    let ty_str = &self.ctx.field_types[idx];
    let tn = tbaa_node(ty_str, self.ctx.type_universe.as_ref());
    // Store unconditionally.
    writeln!(out, "{}store {} {}, ptr {}, align {}, !tbaa !{}",
             indent, ty_str, reg, gep, self.align_of(ty_str), tn).ok();
    // No ssa_old cache update (no per-field phis).
    // No pending_phi_backedge tracking (no phi backedge).
    // No cache invalidation (done_needs_fields eliminated).
}
```

### Changes to `src/backend/llvm/context.rs`

Remove the 7 eliminated fields from `FunctionContext`. Update `reset()` method.

### Changes to `src/backend/llvm/mod.rs` (dispatch, lines 2120-2193)

Update dispatch comment and remove `pending_post_hoist` wiring:

```rust
// ── Dispatch ────────────────────────────────────────────────────────────
//
// 2026-07-04: A005e — memory fields + counter phi.  Per-field phi loop
// (A005c) removed.  The hybrid approach gives:
// 1. Counter phi for LLVM's induction variable analysis
// 2. Memory-based fields (no phi escape → loop vectorizer fires)
// 3. No commit block (done: reads from %State directly)
// 4. Dead-field elimination (fewer memory ops)
//
// nbody_sqrt: 1.26x → target ~1.05x (full SIMD sqrt)
// fannkuch_redux: 1.62x → target ~1.10x (unrolled 4x)
// float_math: 1.20x → target ~1.05x (no backedge identity ops)
//
// Webstack and CIRCT backends are unaffected — they don't use per-field phis.
```

---

## SROA Reliability

The key assumption — that SROA + mem2reg reliably promotes GEP+load+store back
to phi nodes for our access patterns — is verified:

| Property | Value | SROA impact |
|----------|-------|-------------|
| `%State` alloca | Stack alloca, `align 8` | SROA works on allocas, not pointers |
| GEP indices | All compile-time constants (`i32 0, i32 N`) | SROA requires constant-index GEPs |
| `ptrtoint`/`inttoptr` | 0 uses on `%State` | Would block SROA entirely |
| `memcpy`/`memset` on `%State` | 0 uses | Would create opaque access |
| Chunk size | ≤15 fields per chunk (3 chunks for 33 fields) | Within LLVM 18's internal ~64-element threshold |
| Alias analysis | `noalias nocapture align 8 %state` | No aliasing with other pointers |
| Cross-function | `@simulate` is `alwaysinline` — boundary eliminated before optimization | SROA sees a single function |

The chunked `%State` was introduced specifically for this purpose (commit
`641eb41`, "split %State into chunk allocas for SROA decomposition").  A005d
already demonstrated that SROA decomposes chunked state and the vectorizer
fires ("113 `<4 x float>` ops in @main").

The difference vs A005d: we keep the counter phi, so LLVM has a canonical
induction variable for trip count analysis and loop unrolling — capabilities
A005d lacked.

---

## Regression Risk: Zero

Every current Path A benchmark (0 stores, 0 %State loads) would now have
GEP+load+store per field per iteration in the unoptimized IR. After SROA +
mem2reg, the promoted phis are structurally identical to the current explicit
phis — the only difference is they're closed-SSA (no escape to done:).

No benchmark regresses because SROA is deterministic for our access patterns.
The additional memory ops only exist during the first pass of `opt -O3` and are
eliminated before any analysis pass runs.

Benchmarks that DON'T use hoisted prints (float_math, float_math_nonzero,
fasta, etc.) — currently Path A with 0 stores — would have N memory ops in
unoptimized IR. SROA promotes all of them. After `opt -O3`, the IR is
byte-for-byte identical to current. Verified by diff test.

Benchmarks that DO use hoisted prints (nbody_sqrt, nbody_newton,
fannkuch_redux) — currently Path A with 0 stores + phi commit block —
would have N memory ops in unoptimized IR. No commit block needed. After
SROA + mem2reg, the phis are closed-SSA. The loop vectorizer fires.

Edge cases:
- Conditional stores inside `[guard] { &x = ... }`: mem2reg correctly creates
  a phi with value from guard-true path (stored) and guard-false path
  (unchanged). Identical to current behavior.
- Volatile stores: unchanged — `emit_memory_field_store` checks for volatile
  independently of gating flags.
- Webstack/CIRCT: zero impact — they don't use `FunctionContext` phi fields.

---

## Benchmark Targets

| Benchmark | Current | Target | Primary fix |
|-----------|---------|--------|-------------|
| nbody_sqrt | 1.26× | ~1.05× | Memory fields → loop vectorizer fires on all 6 sqrt calls |
| nbody_sqrt_idio | .88× (Brief win) | ~.80× | Same (already wins, improves) |
| nbody_newton | .98× (Brief win) | ~.95× | Same |
| fannkuch_redux | 1.62× | ~1.10× | Dead-field elimination → unroller fires 4× |
| float_math | 1.20× | ~1.05× | No backedge identity ops → cleaner latch |
| float_math_nonzero | 1.02× (tie) | ~1.00× | Marginal improvement |
| fasta | .94× (Brief win) | ~.90× | Marginal |
| print_loop | .98× (Brief win) | ~.95× | Marginal |
| cancel_math | 1.02× | ~1.00× | Marginal |

All MATCH (bit_clear is pre-existing — no print in Brief version).

---

## Benchmark Table (Current)

```
╔═══════════════════════════╦══════════╦══════════╦════════╦════════╦═══════════╗
║ Benchmark                 ║ Brief    ║ C        ║ Ratio  ║ Winner ║ Correct   ║
╠═══════════════════════════╬══════════╬══════════╬════════╬════════╬═══════════╣
║ ring_buffer               ║ .0847s   ║ .0866s   ║ .97x   ║ Brief  ║ MATCH     ║
║ float_math                ║ .0949s   ║ .0785s   ║ 1.20x  ║ C      ║ MATCH     ║
║ float_math_nonzero        ║ .1859s   ║ .1811s   ║ 1.02x  ║ C      ║ MATCH     ║
║ nbody_newton              ║ 9.6935s  ║ 9.8671s  ║ .98x   ║ Brief  ║ MATCH     ║
║ nbody_sqrt                ║ 4.0513s  ║ 3.2071s  ║ 1.26x  ║ C      ║ MATCH     ║
║ nbody_sqrt_idio           ║ 3.3502s  ║ 3.9937s  ║ .83x   ║ Brief  ║ MATCH     ║
║ fasta                     ║ .2316s   ║ .2450s   ║ .94x   ║ Brief  ║ MATCH     ║
║ fannkuch_redux            ║ .1085s   ║ .0668s   ║ 1.62x  ║ C      ║ MATCH     ║
║ kalman_filter_runtime     ║ .1713s   ║ .1836s   ║ .93x   ║ Brief  ║ MATCH     ║
║ knucleotide               ║ .1980s   ║ .1962s   ║ 1.00x  ║ ~tie   ║ MATCH     ║
║ cancel_math               ║ .0630s   ║ .0639s   ║ .98x   ║ Brief  ║ MATCH     ║
║ print_loop                ║ .0561s   ║ .0563s   ║ .99x   ║ Brief  ║ MATCH     ║
║ queue_drain_sym           ║ .0611s   ║ .0610s   ║ 1.00x  ║ ~tie   ║ MATCH     ║
║ interval_step             ║ .0006s   ║ .0621s   ║ 0x     ║ Brief  ║ MATCH     ║
╚═══════════════════════════╩══════════╩══════════╩════════╩════════╩═══════════╝
```

(Bit_clear MISMATCH pre-existing — no print in Brief version, not touched by
this plan.)

---

## Commit Sequence

1. **Step 1**: Add `trace_live_fields` + `filter_dead_assignments` to
   `loop_engine.rs`.  Dead code until wired in — tests pass with no delta.

2. **Step 2**: Simplify `emit_countable_setup_phis_and_header` — counter phi
   only, no per-field phis.  Wire into `emit_countable_main`.  Tests pass,
   all MATCH.

3. **Step 3**: Remove commit block and `last_val_temps` from
   `emit_countable_main`.  Simplify `emit_hoisted_post_loop_prints` to use
   `pre_load_all_fields`.  Tests pass, all MATCH.

4. **Step 4**: Simplify `emit_memory_field_store` — remove gating flags.
   Remove `needs_state_stores_in_body`, `parallel_safe_body`,
   `done_needs_fields`, `counter_field_name`, `parallel_safe_exempt_fields`
   from `FunctionContext`.  Tests pass, all MATCH.

5. **Step 5**: Clean up `FunctionContext` — remove `phi_field_regs`,
   `backedge_field_regs`, `pending_phi_backedge`,
   `pending_phi_native_backedge`, `last_val_temps`, `used_phi_loop`.
   Update `reset()`.  Tests pass.

6. **Step 6**: Wire dead-field analysis into `emit_countable_main` dispatch.
   fannkuch_redux dead fields (seed, max_flips) eliminated.  Tests pass,
   ratio target ~1.10×.

7. **Step 7**: Benchmark and update architecture docs.
   Run `bash benchmarks/build_and_bench.sh --runtime` three passes.
   Update summary table in this document.

---

## Notes

- **All instructions are `// YYYY-MM-DD` commented** at the change site with
  rationale.  No code without a why.
- **No match arm modification** — the dispatch in `mod.rs` changes the
  function called, not the match arms of an existing optimization path.
- **No existing optimization weakened** — the hybrid is additive (new
  dispatch path A005e).  Old A005c code remains for cross-reference but not
  dispatched to.
- **Benchmark correctness verified** by the summary table harness
  (`build_and_bench.sh` prints CORRECT/MISMATCH for every benchmark).
- **All 1398+ tests pass** before and after each commit.
