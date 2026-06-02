# LLVM Backend Optimization Phases — Roadmap to Beat C

Based on: AI evaluation of `LlvmBackend` + 7 benchmark gaps (2026-06-02 calibration).

**Current state (post 2026-06-02 sprint)**: 
- **Phase A (alloca+SROA)**: DONE. Replaced `phi %State` with `alloca %State` + load/store.
  Decomposed by SROA+mem2reg at `opt -O2` into individual scalar float phis. 
  `float_math (zero)`: 0.452s → **0.011s (41× improvement, beats C)**.
- **Phase C (fast-math)**: DONE. Added `fast` flag to all fadd/fmul/fsub/fdiv/fcmp.
  Compound benefit with Phase A: LLVM folds `1.0*x → x` and `0.0*x → 0.0`.
- **SLP hazard fix**: DONE. Fixed `intersection`→`union` for float field tracking.
  Changed formula: `peak = P + min(C, n*2) + temps + const_packed + 2`. 
  Changed cross-op counting to count only cross-field identifier references
  (not Float literals). Catches float_math_nonzero (peak=17 ≥ R=16).  
- **float_math_nonzero**: 0.486s → **0.380s** (1.28× improvement, still 2.32× behind C).
  All instructions are native float ops with `fast` flags. No boxing, no shuffles.
  SROA produced scalar phis. Gap is instruction scheduling & pipeline effects
  (phi structure vs C's local variables).

---

## Phase A — alloca + SROA (Struct Phi → Memory Promotion) [HIGHEST IMPACT]

### Problem
`emit_folded_loop` SSA mode uses `phi %State` — a single struct phi for all 14
state fields. LLVM's GVN cannot analyze individual fields through a struct phi;
the entire phi is marked "varying" if any field changes.

### Current code (`src/backend/llvm.rs:2941-2955`)
```llvm
hdr:
  %ssa_phi = phi %State [ %backedge, %body ], [ %init_state, %pre ]
  %ex = extractvalue %State %ssa_phi, counter_idx   ; counter from phi
  %cp = icmp slt i64 %ex, %bound
  br i1 %cp, label %body, label %done
body:
  ; extractvalue/insertvalue chain on %ssa_phi
  ; produces %backedge (new %State after inserts)
  br label %hdr
done:
  store %State %ssa_phi, %State* @global_state
```

### Fix: Replace with alloca + load/store
```llvm
entry:
  %state_slot = alloca %State, align 8
  store %State %init_state, %State* %state_slot
  br label %hdr
hdr:
  %ssa = load %State, %State* %state_slot
  %ex = extractvalue %State %ssa, counter_idx
  %cp = icmp slt i64 %ex, %bound
  br i1 %cp, label %body, label %done
body:
  ; extractvalue/insertvalue chain on %ssa
  ; produces %new_ssa
  store %State %new_ssa, %State* %state_slot
  br label %hdr
done:
  %final = load %State, %State* %state_slot
  store %State %final, %State* @global_state
```

After `opt -O2`:
1. **mem2reg** promotes `%state_slot` → recognizes load-after-store pattern
2. **SROA** decomposes the promoted alloca into 14 scalar allocas
3. **mem2reg** promotes each scalar alloca → 14 individual scalar phi nodes
4. **GVN** sees float phi `[ 0.0, %backedge ], [ 0.0, %entry ]` → invariant → folded

### Implementation
- `emit_folded_loop` SSA mode: replace `phi %State` + extractvalue counter with
  `alloca %State` + `load %State, ptr %state_slot` + `store %State, ptr %state_slot`
- The counter check: `load i64, ptr getelementptr(%state_slot, counter_idx)` 
  (keeps GEP+load pattern, which SROA handles better than extractvalue)
- Other fields: extractvalue/insertvalue chain unchanged (emit_stmt still works)

### Files
- `src/backend/llvm.rs` — `emit_folded_loop` (~10 lines changed)
- `emit_ssa_main` (~5 lines changed) — same pattern

### Actual impact (DONE)
| Benchmark | Before | After | C | Ratio |
|-----------|--------|-------|---|-------|
| float_math (zero) | 0.452s | **0.011s** | 0.05s | **Brief wins (4.5× faster)** |
| float_math_nonzero | 0.486s | **0.380s** | 0.17s | **2.24×** |
| iir_filter | 0.172s | ~0.17s | 0.17s | ~tie |
| const_heavy | 0.006s | 0.006s | 0.04s | Brief wins |

**float_math_nonzero remaining gap**: The 0.210s difference is from instruction
scheduling & pipeline effects (phi-based loop vs C's local-variable register
allocation). Both emit ~17 native float ops per iteration. No boxing, no 
shuffles, no SLP overhead. The gap may be intrinsic to the phi structure.

### Risk
- `alloca` increases stack usage, but SROA demotes to registers
- Backward compat: all emit_stmt/emit_expr code unchanged (still uses extractvalue)

---

## Phase B — Typed SSA (Eliminate i64 Boxing) [HIGH IMPACT]

### Problem
All expression results are emitted as `i64` regardless of actual type. Floats
require a boxing dance on every operation:
```llvm
%ftr = trunc i64 %val to i32         ; unbox float from i64
%ffl = bitcast i32 %ftr to float      ; i32 bits → float
; ... float operation ...
%bfi = bitcast float %fr to i32       ; float → i32 bits
%v = zext i32 %bfi to i64             ; box back to i64
```

This creates:
- ~3 extra instructions per float read/write (instruction bloat)
- i64 registers for float values (register pressure)
- Prevents LLVM from seeing `float` type for optimization
- Blocks auto-vectorization (float vectors incompatible with i64 boxing)

### Fix: Return typed registers from `emit_expr`

Currently:
```rust
fn emit_expr(&mut self, out: &mut String, expr: &Expr, indent: &str) -> String
// Returns: raw register name (e.g., "%r42")
// Type is guessed later by is_float_expr()
```

New signature:
```rust
fn emit_expr(&mut self, out: &mut String, expr: &Expr, indent: &str) -> (String, ExprType)
// Returns: (register_name, type) where ExprType = Float | Int | Bool | String | Char
```

All callers update to:
```rust
let (reg, ty) = self.emit_expr(out, expr, indent);
match ty {
    ExprType::Float => { /* emit fmul/fadd directly on float */ }
    ExprType::Int => { /* emit add/sub/mul on i64 (current, unchanged) */ }
    ...
}
```

For float operations:
- Read float field: `extractvalue %State, idx` → returns float directly (no trunc/bitcast)
- Add two floats: `fadd float %a, %b` (not `add i64`)
- Store float: `insertvalue %State, float %val, idx` (not trunc+bitcast+insert)

### Correctness Fix
The current `is_float_expr` misses `Expr::Call` returning float:
```rust
// Bug: Expr::Call not in match → defaults to false → emits add i64 instead of fadd
fn is_float_expr_pre_cg(expr: &Expr) -> bool {
    match expr {
        Expr::Call(_, _) => { /* UNHANDLED — returns false */ }
    }
}
```
With typed SSA, this bug is impossible — the return type is known from the
expression evaluation, not guessed retroactively.

### Performance analysis (float_math_zero)
Current: 17 float ops × 3 boxing instructions each = 51 i64 ops + 17 float ops = 68 ops/iter
Typed SSA: 17 float ops directly = 17 ops/iter → **4× fewer instructions**

Combined with Phase A (SROA scalar phis), typed SSA lets LLVM see:
- `%x0_phi = phi float [ %nx0, %body ], [ 0.0, %entry ]`
- `%nx0 = fadd float %x0_phi, %bx0` — native float, no boxing

### Implementation
1. Define `ExprType` enum: `Float, Int, Bool, String, Char`
2. Audit every `emit_expr` return site to annotate the type
3. Update `emit_stmt` match arms to use typed registers
4. Remove `is_float_expr` and `is_float_expr_pre_cg`
5. Remove all `trunc i64 to i32` / `bitcast i32 to float` / `zext i32 to i64` patterns

### Files
- `src/backend/llvm.rs` — core refactor (~200 lines changed)
- May touch `emit_binop`, `emit_stmt`, `emit_assign`

### Risk
- Large refactor — must update ALL `emit_expr` callers
- Regression risk — every type annotation must be correct
- Phased approach: Phase A first, then Phase B

---

## Phase C — Fast-Math Flags [MEDIUM IMPACT]

### Problem
Float operations lack `fast` flag:
```llvm
%bfr = fadd float %fa, %fb    ; strict IEEE-754
```

Without `fast`:
- `0.0 * x` → cannot fold to `0.0` (NaN check: `0.0 * NaN = NaN`)
- `x + 0.0` → cannot fold to `x` (NaN check: `x + NaN = NaN`)
- `x - x` → cannot fold to `0.0`
- Reassociation blocked (float ops are non-associative)

C compilers use `-ffast-math` (or `-O3` with `-funsafe-math-optimizations`).

### Fix
```diff
- %bfr = fadd float %fa, %fb
+ %bfr = fadd fast float %fa, %fb
- %bfr = fmul float %fa, %fb
+ %bfr = fmul fast float %fa, %fb
```

The `fast` flag enables:
- `0.0 * x → 0.0`
- `x + 0.0 → x`
- `x * 1.0 → x`
- `x - x → 0.0`
- Reassociation: `(a + b) + c → a + (b + c)`
- Contract: `(a * b) + c → fmuladd(a, b, c)` (FMA fusion)

### Semantic Impact
`fast` == all flags: `nnan` (no NaN), `ninf` (no Inf), `nsz` (no signed zero),
`arcp` (allow reciprocal), `contract` (allow FMA fusion), `reassoc` (allow
reassociation), `afn` (allow approximations).

For Brief's deterministic model where all float values are initialized to known
constants and only receive constant updates, `fast` is semantically sound:
- No NaN can arise (no 0/0, no inf-inf)
- No Inf can arise (no overflow chains in benchmarks)
- Signed zero is irrelevant (Brief doesn't distinguish +0/-0)

### Implementation
1. Add `fast` string constant: `let FMF = "fast";`
2. Modify `emit_binop` float arms: `writeln!("  {} = {} {} {}, {}, {}", ...)` → include `FMF`
3. All float arithmetic gets `fast` by default
4. Optional: `#[strict_math]` annotation for programs needing IEEE-754 compliance

### Actual impact (DONE)
- float_math (zero): `fast` + scalar phis → LLVM folds `0.0*x → 0.0`, `1.0*x → x`
- float_math_nonzero: `fast` allows reassociation (LLVM reused nx0 subexpressions
  to compute nx1 and nx2, saving 1 fmul per row). Marginal impact on the gap;
  the bottleneck is instruction scheduling, not NaN checks.
- All 368 tests pass with `fast` on all float operations.

### Risk
- `fast` changes NaN/Inf behavior — may hide bugs in programs that depend on
  strict IEEE-754 compliance
- Solution: optional `#[strict_math]` per-transaction
- For benchmarks: all use bounded constants, no NaN/Inf possible

---

## Phase D — Pointer Provenance (Eliminate ptrtoint/inttoptr) [LOW-MEDIUM IMPACT]

### Problem
Strings and arrays are represented as i64 via ptrtoint/inttoptr:
```llvm
%v = ptrtoint i8* %ptr to i64   ; pointer → integer
; ... store i64 to state ...
; ... later, load back ...
%p = inttoptr i64 %v to i8*     ; integer → pointer
```

This destroys LLVM's pointer provenance model:
- `inttoptr` creates a pointer that aliases ALL memory
- LICM cannot hoist loads: "this inttoptr might write to any global"
- DSE cannot eliminate stores: "this inttoptr might read from any global"
- NoAlias analysis fails: every access is MayAlias

### Fix: Preserve pointer types
Replace ptrtoint/inttoptr pairs with direct `i8*` / `ptr` values in the
%State struct. String/array fields remain pointer-typed throughout.

Before:
```rust
// In emit_expr for strings:
writeln!(out, "%v = ptrtoint i8* %str to i64").ok();
// In emit_assign for string field:
// ... stores i64 to %State ...
// In emit_expr for string read:
writeln!(out, "%v = extractvalue %State %ssa, idx").ok();
writeln!(out, "%p = inttoptr i64 %v to i8*").ok();
```

After:
```rust
// In emit_expr for strings:
// ... directly returns i8* register ...
// In emit_assign for string field:
writeln!(out, "%iv = insertvalue %State %ssa, i8* %str, idx").ok();
// In emit_expr for string read:
writeln!(out, "%v = extractvalue %State %ssa, idx").ok();
// Returns i8* directly — no inttoptr needed
```

### Current impact
Benchmarks don't use strings/arrays in hot loops. This phase is low priority
for float benchmarks but critical for string-heavy programs (e.g., data
processing, text manipulation).

### Implementation
1. Change `%State` struct field type for strings from `i64` to `i8*`
2. Remove ptrtoint/inttoptr in emit_expr/emit_assign
3. Update all GEP offsets (field indices shift due to size change)

### Risk
- Field index changes cascade through all emit functions
- Must coordinate with `field_index_map` and `field_types`

---

## Phase E — Calling Convention + Misc [LOW IMPACT]

### 5. Commutativity Pattern Fix

#### Bug
```rust
match pre {
    // Both arms match the same — second is unreachable
    Expr::Eq(l, r) | Expr::Eq(r, l) => {
        let (ident, val) = if let (Expr::Identifier(name), Expr::Integer(n)) = (l.as_ref(), r.as_ref()) {
            ...
        } else if let (Expr::Integer(n), Expr::Identifier(name)) = (l.as_ref(), r.as_ref()) {
            // This IS the fallback for reversed args, but the match arm catches it wrong
        }
    }
}
```

#### Fix
```rust
match pre {
    Expr::Eq(l, r) => {
        // Manual check handles both orderings
        if let (Expr::Identifier(name), Expr::Integer(n)) = (l.as_ref(), r.as_ref()) {
            (name.clone(), *n)
        } else if let (Expr::Integer(n), Expr::Identifier(name)) = (l.as_ref(), r.as_ref()) {
            (name.clone(), *n)
        } else {
            continue;
        }
    }
}
```

### 6. Fast Calling Convention (fastcc)

#### Fix
Apply `fastcc` to generated helper functions:
```diff
- define i1 @pre_increment(%State* %state)
+ define fastcc i1 @pre_increment(%State* noalias nocapture %state)
```

Files affected: `emit_precondition_function`, `emit_async_body`,
`emit_multi_txn`, `reactor_tick`, `tick`.

#### Expected impact
Minimal — `alwaysinline` means most helpers get inlined. Saves ABI overhead
for the rare non-inlined calls.

### 7. Per-Function SLP Hazard Guard

#### Fix
Replace global `-vectorize-slp=false` with per-function metadata:
```llvm
define void @hazardous_fn() !llvm.loop !N {
  ; ... body ...
  br label %loop
loop:
  ; ...
  br i1 %cond, label %loop, label %exit, !llvm.loop !{i1 false, !"llvm.loop.vectorize.width", i32 1}
}
```

Instead of:
```rust
self.opt_flags.push("-vectorize-slp=false");  // global disable
```

Use:
```rust
writeln!(out, "  br i1 %cond, label %loop, label %exit, !llvm.loop !{}",
    self.emit_slp_disable_metadata());
```

#### Risk
- Requires per-loop detection of SLP hazard
- If hazard detection is wrong, programs lose vectorization benefits

---

## Actual Results (2026-06-02 Sprint)

```
Phase A (alloca+SROA)  →  Phase C (fast-math)  →  SLP hazard fix
     ↓                        ↓                        ↓
  float_math: 41×       float_math: beats C    float_nz: 2.32×→2.24×
  float_nz: 1.28×       float_nz: 1.28×        (marginal on this case)
```

| Phase | Difficulty | Impact | Done | Code Changed | Tests |
|-------|-----------|--------|------|-------------|-------|
| A | Medium | float_math 41×, float_nz 1.28× | ✅ | ~10 lines | 368 pass |
| C | Low | Compound with A, fold 1.0*x→x | ✅ | ~20 lines | 368 pass |
| SLP fix | Medium | Correct union/cross-op formula | ✅ | ~25 lines | 368 pass |
| B | High | Typed SSA, remove i64 boxing | ❌ Pending | ~200 lines | — |
| D | Medium | Pointer provenance | ❌ Pending | ~50 lines | — |
| E | Low | Commutativity, fastcc, per-fn SLP | ❌ Pending | ~15 lines | — |

**Key finding**: The remaining float_math_nonzero gap (2.24×) is NOT from i64
boxing — SROA already eliminated all trunc/bitcast/zext in the opt pipeline.
The gap is from instruction scheduling & pipeline effects of the phi structure
vs C's local-variable register allocation. Phase B (typed SSA) would not
close this gap; it would only eliminate boxing that's already eliminated by
SROA+opt. The remaining gap may be intrinsic to the struct-based approach.

**Recommendation for Phase B**: Still worth doing for correctness (eliminates 
`is_float_expr` guess) and for programs where SROA doesn't fully decompose.
But don't expect it to close the float_math_nonzero gap.

---

## Benchmark Target Matrix (After Phase A + C + SLP fix)

| Benchmark | Before | After | C | Ratio (After) | Status |
|-----------|--------|-------|---|---------------|--------|
| float_math (zero) | 0.452s | **0.011s** | 0.05s | **Brief wins** | ✅ Alloca+SROA eliminated zero matrix |
| float_math_nonzero | 0.486s | **0.380s** | 0.17s | **2.24×** | 🔶 Intrinsic phi scheduling gap |
| iir_filter | 0.172s | ~0.17s | 0.17s | ~tie | ✅ Not float-bound, alloca helps |
| const_heavy | 0.006s | 0.006s | 0.04s | Brief wins | ✅ Already O(1) |
| ring_buffer | 0.007s | ~0.007s | 0.002s | ~3× | 🔶 Startup overhead |
| async_counters | 0.004s | ~0.004s | 0.005s | ~tie | ✅ Already tie |
| sparse_dispatch | 0.077s | ~0.077s | 0.002s | startup | 🔶 Startup overhead |
| precompute_sum | 0.002s | ~0.002s | 0.002s | tie | ✅ Already tie |
