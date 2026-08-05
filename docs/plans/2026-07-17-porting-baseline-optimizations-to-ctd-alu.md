# Porting Baseline LLVM Optimizations to CTD+ALU Architecture

**Date:** 2026-07-17
**Target:** Restore Phase 3 benchmark performance after refactored loop engine and CTD+ALU migration
**Baseline commit:** `8a827db1bb600d64740daab52b4613ab7f5cedae`
**Current HEAD:** Post-CTD+ALU migration, emit_user_call fix, all 913 tests pass

---

## 1. Overview & Motivation

The baseline at commit `8a827db` achieved **all benchmarks MATCH with competitive runtime** — Briv beat C on 9 of 15 runtime benchmarks (ratio 0.01x–0.95x). Two major changes since then have degraded performance:

1. **Massive backend refactor at `fb3c335`**: The monolithic `loop_engine.rs` (4398 lines) was split into `loop_engine/{mod,counter,ssa,analysis}.rs` but ALL per-field phi optimizations were lost. The current loop engine uses a single counter phi with unconditional GEP+load+store for all field access — zero register SSA, zero stores suppression, zero vector phi groups.

2. **CTD+ALU migration (our work)**: Replaced `primitive` + `llvm_type` with `ctd` + `alu` properties. This is architecturally correct but introduced no new optimizations.

**Goal:** Port every optimization pattern from the baseline into the current CTD+ALU architecture, fixing the counter phi bug, and restoring benchmark performance while maintaining all 913 passing tests.

---

## 2. Baseline Benchmark Table (Commit 8a827db)

All MATCH. Run with: `cargo build --release && bash benchmarks/build_and_bench.sh --runtime`

| Benchmark | Briv | C | Ratio | Winner |
|-----------|-------|---|-------|--------|
| ring_buffer | 0.0686s | 0.0676s | 1.01x | C |
| float_math | 0.0631s | 0.0771s | 0.81x | Briv |
| float_math_nonzero | 0.1920s | 0.1727s | 1.11x | C |
| sparse_dispatch | 0.0060s | 0.0657s | 0.09x | Briv |
| print_loop | 0.0639s | 0.0670s | 0.95x | Briv |
| nbody_newton | 7.4132s | 9.8522s | 0.75x | Briv |
| nbody_sqrt | 3.0046s | 3.5218s | 0.85x | Briv |
| nbody_sqrt_idio | 2.9578s | 4.3184s | 0.68x | Briv |
| fasta | 0.2695s | 0.2636s | 1.02x | C |
| fannkuch_redux | 0.0763s | 0.0789s | 0.96x | Briv |
| mandelbrot | 0.7514s | 0.7538s | 0.99x | Briv |
| kalman_filter_runtime | 0.1876s | 0.1887s | 0.99x | Briv |
| knucleotide | 0.2093s | 0.2060s | 1.01x | C |
| cancel_math | 0.0682s | 0.0672s | 1.01x | C |
| bit_clear | 0.0010s | 0.0009s | 1.11x | C |
| queue_drain | 0.0007s | 0.0632s | 0.01x | Briv |
| queue_drain_sym | 0.0639s | 0.0672s | 0.95x | Briv |
| interval_step | 0.0009s | 0.0669s | 0.01x | Briv |

Optimizer benchmarks: all MATCH / SKIP (precomputed).

---

## 3. Architecture Summary — Current State

### What works correctly (CTD+ALU migration, our commits)
- `seed_primordial_types` writes ctd/alu/field annotations to properties table
- Normalizer is the single authority for `llvm_type` property
- `rt_llvm_type` reads from normalizer-set property (fallback to `derive_llvm_type` via config)
- `operator_llvm_type` reads ALU property from universe
- `emit_user_call` correctly passes `ptr %state` and adapts argument types
- All 913 tests pass

### What is dead / broken (legacy from fb3c335 refactor)
- **Per-field phi nodes**: `phi_field_regs`, `backedge_field_regs` exist but never populated
- **SSA old caches**: `ssa_old_int_regs`, `ssa_old_float_regs` exist but never read/written
- **Native float SSA**: `reg_float_cache` populated only in toplevel.rs; `ensure_float_reg` hardcodes `is_native = true`
- **Stores gating**: `needs_state_stores_in_body` exists but never set to false (always emit stores)
- **Vector phi groups**: `build_vector_phi_groups()` defined but never called
- **Counter phi wiring**: Uses `bound_reg` as backedge instead of incremented value; predecessor label may be mis-matched
- **Dead-field elimination**: `trace_live_fields`, `filter_dead_assignments` defined but never called
- **Rotation detection**: `detect_rotation_ast`, `find_permutation_cycles` defined but never called
- **Parallel-safe mode**: `parallel_safe_body`, `counter_field_name`, `parallel_safe_exempt_fields` exist but never set/read
- **Phi.rs**: `emit_forward_phis`, `emit_backedge_phis` entirely dead
- **FunctionState methods**: `register_phi_field`, `register_backedge_field`, `queue_pending_phi` entirely dead

---

## 4. Optimization Porting Plan

### 4.0 Fix Counter Phi Wiring (P0 — Correctness Critical)

**Observation:** The current `emit_countable_main` and `emit_folded_loop` in `counter.rs` emit:
```llvm
%counter = phi i64 [ %init_count, %.cm_header ], [ %bound_reg, %.cm_latch ]
```
This has TWO bugs:
1. Backedge uses `%bound_reg` (the loop bound) instead of the incremented counter value
2. Predecessor block for the first value uses `%.cm_header` (the header block itself) instead of the actual entry predecessor block

**Baseline correct pattern:**
```llvm
%pi_cnt_N = phi i64 [ %init_count_N, %pre_phi ], [ %pn_cnt_N, %latch ]
```
The backedge `pn_cnt_N` is `add i64 %pi_cnt_N, 1` — the properly incremented counter.

**Also:** The baseline used a separate `pre_phi` block for initial loads and vector construction, with `br label %loop_hdr`. The current code has no separate `pre_phi` block.

**Action:**
1. Rename the phi entry predecessor from `%.cm_header` to `%entry` (or create a `%pre_phi` block as in baseline)
2. Change the phi backedge from `%bound_reg` to `%counter_next` (the incremented value)
3. Verify with `icmp slt` (increasing) / `icmp sgt` (decreasing) — baseline uses `icmp slt` for increasing counters

**Files to modify:** `src/backend/llvm/loop_engine/counter.rs` (functions `emit_countable_main`, `emit_folded_loop`)

---

### 4.1 Per-Field Phi Nodes (P0 — Correctness Critical for benchmark correctness)

**Observation:** The baseline's `emit_countable_main` (A005c) creates per-field phi nodes for every state field. The current loop engine creates only a single counter phi. All field access goes through GEP+load from `%State` — no register SSA.

Without per-field phis, the loop body has:
- A GEP+load for every field read (in `pre_load_all_fields` and identifier lookup)
- A GEP+store for every field write (in `emit_countable_body`)

With per-field phis, field reads consume phi registers directly (no memory access), and field writes update the phi's backedge (no memory access in the hot loop body when Path A is active).

**Impact:** Per-field phis are STRUCTURAL — the entire loop body is transformed. They enable all downstream optimizations (stores suppression, vector phis, dead-field elimination, rotation).

**Implementation approach:**

Rather than resurrecting the dead `PhiState`/`function.rs` infrastructure, implement a clean version on `FunctionContext`:

```rust
// New fields on FunctionContext
pub per_field_phi_mode: bool,    // Whether per-field phi mode is active
pub phi_counter_reg: String,     // The counter phi register name
pub phi_counter_next: String,    // The incremented counter (latch backedge)
pub phi_field_regs: HashMap<String, (String, Vec<String>)>,  // field → (phi_reg, [pred_regs])
pub phi_field_init: HashMap<String, String>,  // field → init reg (from pre_phi block)
pub phi_field_backedge: HashMap<String, String>,  // field → backedge reg (set by body writes)
```

The emission pipeline:

```
pre_phi block:
  for each field: load from %State → init_reg
  for counter: load from %State → init_count
  br label %loop_hdr

loop_hdr:
  phi counter = phi [init_count, pre_phi], [counter_next, latch]
  for each field f in write_set:
    phi f_reg = phi [init_f, pre_phi], [f_be, latch]
  icmp counter < bound → body or done

body:
  for each stmt:
    // On read: check phi_field_regs first, then GEP+load fallback
    // On write &f = expr:
    //   emit expr → val
    //   phi_field_backedge[f] = val.name
    //   if needs_state_stores_in_body: GEP+store also

latch:
  counter_next = add phi_counter_reg, step
  for each field f in write_set:
    f_be = if phi_field_backedge.contains(f) { phi_field_backedge[f] }
           else { phi_field_regs[f] }  // identity backedge
  br loop_hdr
```

**CTD/ALU integration:** The per-field phi LLVM type is determined by `operator_llvm_type(ty)`:
- `alu=Float` + bytes≤4 → `"float"`
- `alu=Float` + bytes>4 → `"double"`
- All others → `"i64"`

This is already correct in the current `operator_llvm_type` — no change needed.

**Files to modify:**
- `src/backend/llvm/context.rs` — add `per_field_phi_mode`, `phi_field_regs`, etc.
- `src/backend/llvm/loop_engine/counter.rs` — restructure `emit_countable_main` with `pre_phi` block + per-field phis
- `src/backend/llvm/loop_engine/mod.rs` — `pre_load_all_fields` becomes fallback (not primary)
- `src/backend/llvm/emit_expr.rs` — identifier lookup: check `phi_field_regs` before GEP+load
- `src/backend/llvm/emit_stmt.rs` — store assignment: set `phi_field_backedge` instead of GEP+store (when Path A active)

---

### 4.2 Counter Phi Wiring Fix (P0, rolled into 4.1)

Already covered by the per-field phi restructuring above. The correct counter phi pattern is:
```llvm
%pi_cnt = phi i64 [ %init_count, %pre_phi ], [ %pn_cnt, %latch ]
```
where `%pn_cnt = add i64 %pi_cnt, <step>` in the latch, and the exit check uses `icmp slt i64 %pi_cnt, %bound`.

---

### 4.3 `needs_state_stores_in_body` Gating (P1 — Major Performance)

**Observation:** The baseline suppresses stores when no hoisted post-loop prints exist (`pending_post_hoist.is_empty()`). When stores are suppressed, the loop body has ZERO memory traffic for field writes — all values flow through phi registers and backedge registers.

Current code unconditionally emits GEP+store for every `Statement::Assign` in `write_set`. This adds:
- N dead stores per iteration (N = written field count)
- N GEP computations per iteration
- Memory barriers that prevent LICM and vectorization

**Impact:** Without this gating, every loop body has unnecessary memory traffic. LLVM may eliminate dead stores via DSE, but only if no call barriers exist — and FFI calls (prints) create call barriers that prevent DSE from seeing through them.

**Implementation:**
```rust
// After building phi setup, determine store requirement:
self.fun.needs_state_stores_in_body = !self.fun.pending_post_hoist.is_empty();
```

Then in `emit_countable_body` (or the new per-field phi body emission):
```rust
// Path A (needs_state_stores_in_body = false):
//   field_backedge[field] = val.name
//   (GEP+store NOT emitted)

// Path B (needs_state_stores_in_body = true):
//   emit GEP+store for field
//   field_backedge[field] = val.name (still needed for latch)
```

**CTD/ALU integration:** Store type is always `i64` (the universal state storage type) — use `adapt_to_i64` to box the value before storing. `adapt_to_i64` is already CTD/ALU-aware (checks `llvm_type` property for Float→bitcast, Bool→zext, etc.).

**Files to modify:**
- `src/backend/llvm/context.rs` — `needs_state_stores_in_body` already exists (dead) — just use it
- `src/backend/llvm/loop_engine/counter.rs` — set `needs_state_stores_in_body` before body emission; gate stores on it

---

### 4.4 Native Float SSA (P1 — Major, especially nbody)

**Observation:** The baseline's `ensure_float_reg` checks `rt.storage == "Native"` — if the float is already a native LLVM float register, it returns it directly (no trunc+bitcast). The `reg_float_cache` prevents duplicate conversions.

Current `ensure_float_reg` hardcodes `is_native = true` and returns `reg.name.clone()` — effectively a no-op. But the cache is still populated in toplevel.rs entry code.

**Status in CTD+ALU:** All types with `alu=Float` use native LLVM float/double types. The `operator_llvm_type` function correctly returns `"float"` or `"double"` for these. The `adapt_to_i64` function boxes them to `i64` for state storage. So the pipeline is:

- **Entry/load:** GEP+load from `%State` → `i64` → `adapt_to_i64` does nothing (already i64) ... but wait, float fields are stored as i64 in `%State`. So loading a float field gives you a boxed i64. You need `i64_to_float_reg` to get the native float.

Actually, in the baseline, ALL state fields were i64. Float values were boxed to i64 in `%State`. When you loaded a float field into a register, you got `i64 %boxed`. Then to use it as native float, you needed `trunc i64 %boxed to i32` + `bitcast i32 %tr to float`. The `reg_float_cache` would cache this result so you didn't repeat the conversion.

In the current code with CTD+ALU, this is STILL the case — `%State` fields are always i64. So loading a float field gives you a boxed i64. But the current `ensure_float_reg` just returns the register name (assuming it's already native), which is WRONG when the register holds a boxed float.

However, `operator_llvm_type` returns `"float"` for ALU=Float types, so the emit_binary_op dispatcher correctly uses `fadd` for floats. But the operands need to be actual `float` or `double` LLVM values, not `i64`. If the identifier lookup returns an i64 register (from GEP+load), then using it in `fadd` would be a type mismatch.

Wait, let me re-read `emit_binary_op`:
```rust
let is_float = l.ty == Type::float() || r.ty == Type::float() 
    || l.ty == Type::float64() || r.ty == Type::float64();
```

This checks the `TypedRegister.ty` field, which is the Briv type (e.g., `Type::Custom("Float")`). If the identifier lookup returns `TypedRegister { ty: Type::int() }` for a float field (because it loaded `i64` from `%State`), then `is_float` would be false, and the binary op would emit `add` or `sub` instead of `fadd`/`fsub`!

That's a **correctness bug**! Let me verify by looking at how the identifier handles type info...

In `emit_expr.rs:52-67`:
```rust
Expr::Identifier(name) => {
    if let Some(reg) = self.get_local(name) {
        TypedRegister { name: reg.clone(), ty: self.get_local_type(name) }
    } else if let Some(&idx) = self.ctx.field_index_map.get(name) {
        let gep = ...;
        writeln!(out, "... load i64, ptr {}", ...).ok();
        TypedRegister { name: v.to_string(), ty: Type::int() }  // ← Always Type::int()!
    } else {
        writeln!(out, "... load i64, ptr @{}", ...).ok();
        TypedRegister { name: v.to_string(), ty: Type::int() }
    }
}
```

The state field path ALWAYS returns `Type::int()`, even for float fields! This means:
- Loading a float field from `%State` returns `{ ty: int(), name: "%t42" }` where `%t42` holds an `i64` (boxed float)
- `emit_binary_op` checks `is_float` using `Type::int()` → false
- Emits `add i64` instead of `fadd float` → WRONG for float arithmetic

This IS a bug — but tests pass, which suggests:
1. Either no test exercises float field arithmetic
2. Or LLVM can somehow handle the mismatch
3. Or the float fields go through `reg_float_cache` by happenstance

Wait, for nbody benchmarks which DO float field arithmetic and ARE correct (MATCH) at baseline... unless the CTD+ALU changes broke something and nbody no longer compiles correctly. But we haven't run correctness since our changes — we only know 913 unit tests pass.

Actually, the identifier lookup for local bindings (first branch) uses `self.get_local_type(name)` which returns the actual Briv type from `let_binding_types`. So if a float value was assigned to a `let` binding, it would carry the correct type. But GEP+load from `%State` always returns `Type::int()`.

For the nbody benchmark, the state fields (vx, vy, vz, etc.) are Float type. When the body reads them via `pre_load_all_fields`, that function emits:
```
%vx_old_N = load i64, ptr %gep, align 8
```
And stores the register name in... wait, `pre_load_all_fields` used to store in `ssa_old_float_regs`/`ssa_old_int_regs`. But in the current code, it just emits the loads without storing them anywhere accessible.

Actually, let me re-read the current `pre_load_all_fields`:
```rust
pub(crate) fn pre_load_all_fields(&mut self, out: &mut String, state_ptr: &str, write_set: Option<&HashSet<String>>)
```
It loads ALL state fields into registers but the registers are just named and emitted — they're NOT stored in `ssa_old_*_regs`. So when the body references `vx`, it goes through `Expr::Identifier("vx")` which does GEP+load again — another redundant load!

So the current code has:
1. `pre_load_all_fields` at body entry (emits N GEP+loads that are never used)
2. Each `Expr::Identifier` for a state field (emits ANOTHER GEP+load — completely redundant with step 1)
3. Each `Statement::Assign` for a state field (emits GEP+store)

This is terrible for performance.

**OK so this confirms that the current loop engine has fundamental correctness and performance issues.** The identifier lookup losing type information (always returning `Type::int()`) means float arithmetic operands are treated as integers — producing wrong results at the machine level.

But wait — if this were really a correctness bug, tests would fail. Unless no test exercises Float state field arithmetic... but the print_loop benchmark uses Int fields only. The nbody benchmarks haven't been run through our CI.

**This is a critical finding.** It means the current compiler CANNOT correctly compile float field arithmetic. Interpreter passes (because it handles types correctly regardless of LLVM representation), but LLVM codegen is broken.

**Action for native float SSA:**
1. Fix `Expr::Identifier` to return the correct `ty` for state fields (look up `field_types` or type universe)
2. Fix `pre_load_all_fields` to actually store results in accessible caches (with correct types)
3. Alternatively, eliminate `pre_load_all_fields` entirely and use per-field phi registers instead
4. `ensure_float_reg` should check `rt.storage` — since CTD+ALU has no explicit "Native" storage flag, the equivalent check is: if the register's LLVM type (from `llvm_type`) is `float` or `double`, return as-is. If it's `i64` but the Briv type is Float, emit trunc+bitcast.

Actually, with CTD+ALU, ALL types have `alu=Float` mapping to native float LLVM types. So `ensure_float_reg` should work like this:
```rust
fn ensure_float_reg(&mut self, out: &mut String, indent: &str, reg: &TypedRegister) -> String {
    // If already a native float register, return as-is
    if reg.ty == Type::float() || reg.ty == Type::float64() {
        return reg.name.clone();
    }
    // Check cache for previously boxed float
    if let Some(cached) = self.fun.reg_float_cache.get(&reg.name) {
        return cached.clone();
    }
    // Is the Briv type a float? Then we have an i64 boxed value
    let is_float_ty = match &reg.ty {
        Type::Custom(t) => {
            let alu = self.ctx.type_universe.as_ref()
                .and_then(|u| rt_property(u, &reg.ty, "alu"))
                .and_then(|pv| if let PropertyValue::Identifier(s) = pv { Some(s.as_str()) } else { None });
            alu == Some("Float")
        }
        _ => false,
    };
    if !is_float_ty {
        return reg.name.clone();
    }
    // Boxed float → native float conversion
    self.native_float_or_box(out, indent, &reg.name)
}
```

**Files to modify:**
- `src/backend/llvm/emit_expr.rs` — fix identifier lookup to return correct type for state fields
- `src/backend/llvm/emit_toplevel.rs` — rewrite `ensure_float_reg` to check `reg.ty` and universe ALU
- `src/backend/llvm/helpers.rs` — improve `reg_float_cache` usage (cache on population, not just consumption)

---

### 4.5 Vector Phi Groups (P2 — Important for nbody_sqrt)

**Observation:** The baseline groups float vector fields (vx0..vx3, vy0..vy3, etc.) into `<4 x float>` vector phi nodes. For nbody_sqrt's 30 float fields, this reduces phi count from 32 scalar to ~8 vector phis, eliminating register spills.

**Action:**
1. Reactivate `build_vector_phi_groups()` — call it in `emit_countable_main` before phi setup
2. In latches, use `insertelement` for vector group members
3. At loop exit, `bitcast <4 x float>` to store-compatible type in commit block
4. For hoisted post-loop loads, load once and `extractelement`

**CTD/ALU integration:** Only group fields with `alu=Float`. Use `operator_llvm_type` to check size (≤4 bytes → `float`, >4 → `double`).

---

### 4.6 Dead-Field Elimination (P2 — Important for fannkuch_redux)

**Observation:** The baseline traces live fields backward from observable sinks (prints, FFI calls, swan songs) and eliminates dead field assignments from the loop body. For fannkuch_redux, this reduced body size from ~80 to ~40 instructions, enabling 4× unrolling.

**Action:**
1. Reactivate `trace_live_fields()` — scan body for observable sinks
2. Filter `write_set` to only live fields before body emission
3. Optionally filter `phi_field_regs` to only live fields (fewer phi nodes)

**For CTD+ALU:** Ast traversal is type-agnostic — no changes needed.

---

### 4.7 Rotation Detection (P2 — Important for fannkuch_redux)

**Observation:** The baseline detects circular phi chains (each field assigned from another) and decomposes them into rotation unrolls. For fannkuch_redux's 12-cycle permutation, decomposes a 12-cycle into 3-cycles that SCEV can analyze.

**Action:**
1. Reactivate `detect_rotation_ast()` — scan body for assignment chain patterns
2. If rotation detected, add `rotation_fields` to force GEP-reload in latch (breaks circular phi chains)
3. Unroll body with straight-line copies (rot_full path)

---

### 4.8 Parallel-Safe Mode (P2 — SIMD Enabler)

**Observation:** The baseline treats all loop bodies as parallel-safe — `ssa_old` caches are NOT updated after `&` assignments. All computations see old phi values (independent), enabling SIMD.

**Action:**
1. When per-field phis are active, set `parallel_safe_body = true`
2. Body reads always use the phi register (old value), not the updated value
3. Counter field and exempt fields still see updated values (for guard conditions)
4. No effect on correctness — updates flow through phi backedge for next iteration

---

### 4.9 EmitUserCall — Float Return Handling (P1)

**Observation:** The baseline `call.rs` correctly handles float-returning defns:
```rust
let is_float_ret = def_rets.iter().any(|t| matches!(t, Type::Custom(__t) if __t == "Float"));
let call_ret = if is_float_ret { "float" } else { "i64" };
```

Current `emit_user_call` emits:
```rust
let ret_llvm = lower_type(&ret_type);
writeln!(out, "{}{} = call {} @{}({})", indent, v, ret_llvm, name, ...).ok();
```

`lower_type` returns `"double"` for `Type::float64()`, which is correct. But `lower_type` for `Type::Custom("Float")` falls through to... let me check.

Actually, `lower_type` is in `types.rs` — it's a simple function. For `Type::Custom(t)` with no particular match, it returns `"i64"`. So `lower_type(Type::Custom("Float"))` returns `"i64"`! This is WRONG for float-returning defns — they should return `"float"` or `"double"`.

**Action:**
1. Fix `emit_user_call` to use `operator_llvm_type` (or `llvm_type`) for the return type, not `lower_type`
2. `operator_llvm_type` correctly returns `"float"`/`"double"` for ALU=Float types

**Files to modify:** `src/backend/llvm/emit_expr.rs` — `emit_user_call` return type handling

---

### 4.10 PreExtract Fields — Inline SSA State Pipeline (P2)

**Observation:** The baseline's `pre_extract_float_fields` and `pre_extract_int_fields` extract values from an `ssa_state_reg` (the `%State` struct SSA register) via `extractvalue`. This is used by the A005a inline SSA path (`emit_folded_loop` with `body=Some`).

The current `pre_extract_float_fields` and `pre_extract_int_fields` in `mod.rs` still exist but are never called (they are dead).

**Action:** Either reactivate them for the inline SSA path, or remove the dead code. They serve as a simpler alternative to per-field phis — using `extractvalue`/`insertvalue` on the whole `%State` struct rather than individual phi nodes.

This is lower priority than per-field phis because per-field phis achieve better LLVM optimization (SROA can decompose them into individual scalars more easily than struct-SSA).

---

## 5. Implementation Sequence

The optimizations below are ordered by impact and dependency. Each step must compile and pass `cargo test --lib` before moving to the next.

### Phase A: Correctness Fixes (commits A1-A3)

**A1 — Fix identifier type info:** Ensure `Expr::Identifier` returns the correct Briv type for state fields (not always `Type::int()`). This fixes float arithmetic in the loop body.

**A2 — Fix emit_user_call return type:** Use `operator_llvm_type` instead of `lower_type` for defn return types. This fixes float-returning defn calls.

**A3 — Fix counter phi wiring:** Change phi backedge from `bound_reg` to incremented counter value. Create `pre_phi` block for initial loads. Fix predecessor labels.

After Phase A: All 913 tests pass. Basic correctness is restored for float field arithmetic and defn calls. (*Counter phi fix may not affect any specific test but is structurally correct.*)

### Phase B: Per-Field Phi Architecture (commits B1-B3)

**B1 — Implement per-field phi setup:** Add `phi_field_regs`, `phi_field_init`, `phi_field_backedge` to FunctionContext. Restructure `emit_countable_main` to create per-field phis after the counter phi. Add `pre_phi` block with initial field loads.

**B2 — Route identifier reads through phi registers:** Modify `emit_expr.rs` identifier lookup to check `phi_field_regs` before falling back to GEP+load. When per-field phi mode is active and the field has a phi register, return the phi register directly (no memory access).

**B3 — Implement per-field backedge in latch:** Restructure `emit_countable_latch` (or its equivalent) to emit per-field backedge wiring. For written fields: use `phi_field_backedge` value (possibly native-typed for floats). For unwritten fields: use identity backedge. For rotation fields: GEP reload.

After Phase B: Loop body has zero memory traffic for field reads. Stdlib/print_loop benchmarks produce correct output. (~913 tests pass.)

### Phase C: Store Gating (commits C1-C2)

**C1 — Implement needs_state_stores_in_body:** Set `needs_state_stores_in_body = pending_post_hoist.is_empty()`. Gate body stores on this flag.

**C2 — Add phi commit block:** When `done_needs_fields` is non-empty (hoisted prints), add a commit block between latch and done that stores phi final values to `last_val_temps` allocas. `emit_hoisted_post_loop_prints` reads from these allocas.

After Phase C: Zero memory traffic in hot loop when no post-loop prints. Equivalent to baseline Path A.

### Phase D: Native Float SSA (commits D1-D3)

**D1 — Fix ensure_float_reg:** Check `reg.ty` and universe ALU property. If the register is an i64 boxed float, emit trunc+bitcast chain. Cache the result in `reg_float_cache`.

**D2 — Add native-typed backedge for floats:** In the per-field phi backedge, use `ensure_float_reg` to derive a native-typed value for the phi backedge (instead of the i64 boxed value). This eliminates the trunc+bitcast roundtrip when the body produces a native float result.

**D3 — Fix adapt_to_i64 for state stores:** Ensure the unconditional store path (when `needs_state_stores_in_body = true`) uses `adapt_to_i64` to box the float before storing to `%State`.

After Phase D: Float loop bodies avoid box→unbox roundtrip. nbody benchmarks produce correct output.

### Phase E: Advanced Optimizations (commits E1-E4)

**E1 — Vector phi groups:** Reactivate `build_vector_phi_groups`. Group float fields with sequential numeric suffixes into `<4 x float>` vector phis. Use `insertelement`/`extractelement` for body writes/reads.

**E2 — Dead-field elimination:** Reactivate `trace_live_fields` + `filter_dead_assignments`. Filter `write_set` and `phi_field_regs` to only live fields. Reduces IR size for complex benchmarks.

**E3 — Rotation detection:** Reactivate `detect_rotation_ast` + `find_permutation_cycles`. For circular phi chains, emit rotation-unrolled body.

**E4 — Parallel-safe mode:** When `parallel_safe_body = true`, phi registers are never updated during body emission. All computations see old phi values (independent). Counter field exempted.

### Phase F: Cleanup (commits F1-F2)

**F1 — Remove dead code:** Delete `function.rs`, `phi.rs`, or mark them with clear dead-backend comments. Remove dead analysis functions that weren't reactivated.

**F2 — Documentation:** Update architecture docs for the restored optimizations. Add rationale comments at each modified site.

---

## 6. Trade-off Analysis per Optimization

### Per-Field Phis

| Aspect | Assessment |
|--------|-----------|
| **Targets** | All bounded single-txn loops (`[count < N][count == N]`) |
| **Gains** | Zero memory traffic for field reads; register SSA enables downstream optimizations |
| **Costs** | More phi nodes in header (~2N for N written fields); larger IR initially; ~0.1% compile-time increase |
| **When it hurts** | Loops with very few iterations (< 3) — phi setup overhead dominates; tiny bodies (< 3 fields) — GEP+load was already cheap |
| **Trade-off decision** | Always enable when loop has any written state field. Baseline used A005c for all bounded txns. Threshold: if write_set is empty, skip phis entirely (use counter-only phi). |

### Store Gating (needs_state_stores_in_body)

| Aspect | Assessment |
|--------|-----------|
| **Targets** | Loops with no post-loop print (`term!` without swan song) |
| **Gains** | Zero dead stores in hot loop body; LLVM sees pure register pipeline; enables vectorization |
| **Costs** | ~0 when stores are already suppressed; if stores ARE needed, no change from baseline |
| **When it hurts** | Never — stores suppressed only when correct |
| **Trade-off decision** | Always suppress when `pending_post_hoist` is empty. Dual-path: detect at compile time, emit different IR for each path. |

### Native Float SSA

| Aspect | Assessment |
|--------|-----------|
| **Targets** | Float-heavy loops (nbody, float_math) |
| **Gains** | Eliminates trunc+bitcast roundtrip per float field operation; native float phi backedge avoids unbox in latch |
| **Costs** | ~2 extra LLVM registers per float field (one for boxed, one for native); ~0 comp time impact |
| **When it hurts** | Float fields read once but never operated on (rare) |
| **Trade-off decision** | Always compute native float value on first use; cache via `reg_float_cache`. No downside. |

### Vector Phi Groups

| Aspect | Assessment |
|--------|-----------|
| **Targets** | Float fields with contiguous numeric suffixes (nbody_sqrt) |
| **Gains** | Reduces phi count from 32→8 for nbody_sqrt; eliminates register spills (~16 stack slots → 0) |
| **Costs** | `insertelement`/`extractelement` overhead for vector group setup (~1 insn per element) |
| **When it hurts** | Non-contiguous float fields; fields with matrix-style suffixes (p00, p01, p10, p11) |
| **Trade-off decision** | Always build groups for float fields with sequential [0,1,2,3] suffixes. Detection is cheap (one scan). No runtime cost when no groups found. |

### Dead-Field Elimination

| Aspect | Assessment |
|--------|-----------|
| **Targets** | Loops with dead stores (fannkuch_redux rotation) |
| **Gains** | ~50% body size reduction for fannkuch_redux; enables 4× unrolling |
| **Costs** | AST traversal to trace liveness (~O(body_size) per function) |
| **When it hurts** | Bodies where all fields are live (most benchmarks) — traversal time is wasted but <0.1ms |
| **Trade-off decision** | Always run trace. If all fields live, skip filtering (no change). If some fields dead, eliminate them. The O(N) traversal is negligible vs llc time. |

---

## 7. Documentation Plan

### Files requiring new/updated `///` doc comments

| File | What to document |
|------|-----------------|
| `src/backend/llvm/context.rs` | New `per_field_phi_mode`, `phi_field_regs`, `phi_field_init`, `phi_field_backedge` fields |
| `src/backend/llvm/loop_engine/counter.rs` | Restructure doc comment for `emit_countable_main` explaining pre_phi block, per-field phis, dual-path gating |
| `src/backend/llvm/emit_expr.rs` | Doc comment for identifier lookup explaining phi register check, state field fallback |
| `src/backend/llvm/emit_toplevel.rs` | Updated `ensure_float_reg` doc: CTD+ALU float detection, not hardcoded is_native |
| `src/backend/llvm/helpers.rs` | Updated `operator_llvm_type` and `adapt_to_i64` doc comments |

### Rationale comments to add

Every modified function must have rationale comments at each significant code site:
```
// 2026-07-17: Per-field phi mode — field reads check phi registers first.
// When per_field_phi_mode is true, state fields have an SSA register
// from the loop header phi. Reading from the phi register eliminates
// the GEP+load memory access, enabling LLVM SROA decomposition.
```

### Architecture docs to update

| Document | What to update |
|----------|---------------|
| `docs/architecture/ctd-and-alu.md` | Add section on how ALU=Float maps to native LLVM float types and the `ensure_float_reg` pipeline |
| `docs/architecture/backend-type-dispatch.md` | Confirm that `operator_llvm_type` reading ALU property is the canonical dispatch |
| `docs/architecture/layout-dsl.md` | No changes needed (layout DSL is unrelated to loop engine) |

---

## 8. Regression Guard Plan

### Before each phase

1. **Record baseline:** Run `bash benchmarks/build_and_bench.sh --runtime` and save results
2. **Verify all MATCH:** Check correctness column for every benchmark
3. **Record IR:** Save unoptimized `.ll` for `print_loop`, `nbody_newton`, `fannkuch_redux`, `float_math`

### After each commit

1. **`cargo test --lib`** — all 913 tests must pass (non-negotiable)
2. **`cargo build`** — no warnings
3. **Run Praetor** on new/changed files (complexity ≤ 15, lines ≤ 100, params ≤ 6)
4. **Record results:** `bash benchmarks/build_and_bench.sh --runtime` — compare against baseline
5. **Inspect IR:** Compare unoptimized `.ll` for key benchmarks — verify expected pattern

### What constitutes a regression

- Any benchmark that flips from MATCH to MISMATCH (correctness bug)
- Any benchmark that slows by >10% without a documented trade-off
- Any benchmark that regresses by >50% (optimization accidentally removed)
- Any benchmark that goes precomputed (dead-code folding) when it shouldn't

### Exception policy

- A benchmark that slows by <5% is noise
- A benchmark that slows by 5-10% must have a comment explaining why (e.g., "adds 2 phi nodes for better SROA decomposition")
- A benchmark that slows by >10% must be investigated before proceeding
- All trade-offs must be documented in the per-optimization table above

---

## 9. Current Known Bugs (Pre-Existing)

These bugs exist in the current HEAD and are NOT introduced by this plan:

1. **Counter phi uses bound_reg as backedge** (covered in Phase A3)
2. **Identifier lookup returns Type::int() for state fields** even when the field is Float (covered in Phase A1)
3. **emit_user_call uses lower_type for return type** instead of operator_llvm_type (covered in Phase A2)
4. **pre_load_all_fields emits unused loads** — registers are computed but never stored in accessible caches (fixed by per-field phis in Phase B)
5. **ensure_float_reg is a no-op** — hardcodes `is_native = true` which is wrong for boxed float values from state loads (fixed in Phase D1)

These bugs trace to the massive refactor at `fb3c335` which rewrote the loop engine and type dispatch without porting the optimization infrastructure.

---

## 10. Untouched Systems (Keep Hands Off)

The following are working correctly and must NOT be modified:

- **Test suite infrastructure** (all 913 tests pass)
- **Parser/AST** (CTD+ALU property annotations)
- **Normalizer** (sets `llvm_type`, validates ALU×CTD compatibility)
- **Interpreter** (reference implementation)
- **Webstack/CIRCT backends** (separate concern)
- **Config files** (`config/ctd-llvm-mappings.toml`)
- **Dead backends** (verilog, vhdl, c, rust, cobol, etc.)
- **Benchmark harness** (`benchmarks/build_and_bench.sh`)
