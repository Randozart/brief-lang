# Decision Framework Refactor: 5-Axis Dispatch Strategy

**Date:** 2026-07-28
**Commits:** 5 commits, one per axis. Each independently verifiable.
**Location:** `DispatchStrategy` on `FunctionContext` (per-txn).

---

## Architecture

### Before: Entangled decisions

```
mod.rs:2736       → dispatch + attr (hardcoded #9 for @main)
hazard.rs:125     → attr B for main (returns #9 always)
emit_toplevel:1432→ inline D (based on has_cycles only)
emit_toplevel:1673→ attr B for txn functions
emit_toplevel:1711→ metadata E (!prof, always emits)
helpers.rs:2830   → metadata E (!range, always emits)
counter.rs:740    → SLP C (combined with stride/chain)
```

6 files, 7 decision sites, each making independent choices unaware of the others.

### After: Analyzer → Strategy → Emission

```
StrategyAnalyzer (strategy.rs, new)
  ─── for each txn, produces ───→ DispatchStrategy
                                    ↓
  mod.rs:2736            →  reads strategy.loop_style
  hazard.rs:125          →  removed (reads strategy.hot_loop_attr)
  emit_toplevel.rs:1432  →  reads strategy.inline_mode
  emit_toplevel.rs:1673  →  reads strategy.hot_loop_attr
  emit_toplevel.rs:1711  →  reads strategy.metadata_mode
  helpers.rs:2830        →  reads strategy.metadata_mode
  counter.rs:740         →  reads strategy.slp_mode
  is_reduction_pattern   →  unchanged (per-group, not per-txn)
  chain_pass_ok          →  unchanged (per-group, not per-txn)
  estimate_template_depth→  unchanged (per-group, not per-txn)
```

---

## Data Structures

### `DispatchStrategy` (on `FunctionContext`)

```rust
/// Per-txn strategy computed once before emission, consumed by all emitters.
pub struct DispatchStrategy {
    /// A: How the convergence loop tracks state fields between iterations.
    pub loop_style: LoopStyle,

    /// B: LLVM attribute group for the hot loop function (@main or @txn_*).
    pub hot_loop_attr: HotLoopAttr,

    /// C: Whether to emit hand-rolled SLP vector groups.
    pub slp_mode: SlpMode,

    /// D: Whether the txn function should be alwaysinlined into its caller.
    pub inline_mode: InlineMode,

    /// E: Whether to emit !range / !prof metadata on field loads and branches.
    pub metadata_mode: MetadataMode,
}

pub enum LoopStyle {
    WhileLoop,      // GEP+load+store, no phis. Best for register pressure ≥ 16.
    PerFieldPhi,    // Per-field phi nodes. Default for most programs.
    InlineSsa,      // EmitInlineSsa: insertvalue chain. Best for dense small states.
    PureCounter,    // Pure-counter fold: O(1) store, no loop.
}

pub enum HotLoopAttr {
    MemoryReadWrite,          // #9 — conservative, for programs with unguarded FFI.
    ArgMemReadWrite,          // #11 — enables SROA, no willreturn (safe for Newton).
    ArgMemReadWriteWillReturn, // #12 — enables SROA + loop opts (for simple loops).
}

pub enum SlpMode {
    Enabled,   // Emit our SLP groups (chain_pass_ok + !is_reduction_pattern).
    Disabled,  // Let LLVM's auto-vectorizer handle vectorization.
}

pub enum InlineMode {
    AlwaysInline,  // alwaysinline on txn function. Default for cheap bodies.
    NoInline,      // noinline — keep separate function. For expensive/cyclic bodies.
    NoFunction,    // No separate txn function — body emitted directly in @main.
}

pub enum MetadataMode {
    All,          // Emit !range and !prof normally.
    HotPathOnly,  // Emit on non-hot-path statements only.
    None,         // Skip ALL metadata on the hot loop.
}
```

### `StrategyAnalyzer` (new file: `src/backend/llvm/strategy.rs`)

```rust
/// Computes DispatchStrategy for each reactive txn before any emission begins.
/// Called once from `generate()` after all analysis (hazard, SLP, fold) is complete.
pub struct StrategyAnalyzer;

impl StrategyAnalyzer {
    pub fn analyze(txn: &Transaction, ctx: &CompilerContext) -> DispatchStrategy {
        let has_unguarded_ffi = !txn.body.iter().all(|s| match s {
            Statement::Guarded(_, _) => true,
            _ => !crate::analysis::transition_graph::statement_contains_ffi(s),
        });
        let peak_live = ctx.peak_live_floats;
        let has_body_ffi = txn.body.iter().any(|s| {
            crate::analysis::transition_graph::statement_contains_ffi(s)
        });
        let total_fields = ctx.field_index_map.len() as u32;
        let write_density = /* existing computation */;

        DispatchStrategy {
            // A: Dispatch — principled by peak register pressure
            loop_style: if peak_live >= 16 && has_body_ffi {
                LoopStyle::WhileLoop
            } else if write_density >= 0.5 && total_fields < 8 && !has_body_ffi {
                LoopStyle::InlineSsa
            } else {
                LoopStyle::PerFieldPhi
            },

            // B: Attribute — principled by FFI location + Newton detection
            hot_loop_attr: if has_unguarded_ffi {
                HotLoopAttr::MemoryReadWrite        // #9
            } else if /* has_newton_iteration */ {
                HotLoopAttr::ArgMemReadWrite         // #11
            } else {
                HotLoopAttr::ArgMemReadWriteWillReturn // #12
            },

            // C: SLP — uses existing chain_pass_ok + is_reduction_pattern
            slp_mode: SlpMode::Enabled,  // gated later by per-group checks

            // D: Inline — principled: if while-loop, no function needed
            inline_mode: match loop_style {
                WhileLoop | PureCounter => InlineMode::NoFunction,
                _ => if ctx.has_cycles { NoInline } else { AlwaysInline },
            },

            // E: Metadata — principled: while-loop needs clean IR for SLP
            metadata_mode: match loop_style {
                WhileLoop => MetadataMode::None,
                _ => MetadataMode::All,
            },
        }
    }
}
```

---

## Commit Plan

### Commit A: Axis A (Dispatch) — + strategy.rs, modify mod.rs:2736

**Changes:**
1. Create `src/backend/llvm/strategy.rs` — `DispatchStrategy` struct, `LoopStyle` enum
2. Add `strategy: DispatchStrategy` to `FunctionContext` in `context.rs`
3. In `generate()` (mod.rs), call `StrategyAnalyzer::analyze()` before emission
4. Replace `mod.rs:2736` dispatch chain with `match strategy.loop_style { ... }`
5. The existing emission functions are called unchanged — they just get dispatched differently

**Verification:**
- `cargo test --lib` — 1045 tests pass
- `bash build_and_bench.sh --runtime` — all 19 MATCH, no regression
- The dispatch is the same as before (same thresholds), just moved to strategy

### Commit B: Axis B (Attributes) — modify hazard.rs + emit_toplevel.rs

**Changes:**
1. Add `hot_loop_attr: HotLoopAttr` to `DispatchStrategy`
2. Compute in `StrategyAnalyzer::analyze()` using `has_unguarded_ffi` + Newton detection
3. Modify `hazard.rs:slp_attr()` to accept `DispatchStrategy` instead of hardcoding `#9`
4. Modify `emit_toplevel.rs:1673` to read `strategy.hot_loop_attr` instead of `if outlined {#11} else {#0}`

**Verification:**
- All 19 MATCH, no regression
- nbody-family benchmarks that use `#11` instead of `#0` should benefit

### Commit C: Axis C (SLP) — modify counter.rs:740

**Changes:**
1. Add `slp_mode: SlpMode` to `DispatchStrategy`
2. Modify `counter.rs:740`'s `should_vec` formula to check `strategy.slp_mode` first

**Verification:**
- All 19 MATCH, no regression
- SLP behavior unchanged (same chain_pass_ok + is_reduction_pattern gates)

### Commit D: Axis D (Inlining) — modify emit_toplevel.rs:1432

**Changes:**
1. Add `inline_mode: InlineMode` to `DispatchStrategy`
2. Compute: if while-loop → NoFunction, if cycles → NoInline, else → AlwaysInline
3. Modify `emit_toplevel.rs:1432` to emit `alwaysinline`/`noinline`/nothing based on strategy

**Verification:**
- ring_buffer 1.06x maintained (NoFunction — no txn fn emitted)
- All 19 MATCH

### Commit E: Axis E (Metadata) — modify helpers.rs:2830 + emit_toplevel.rs:1711

**Changes:**
1. Add `metadata_mode: MetadataMode` to `DispatchStrategy`
2. Compute: if while-loop → None, else → All
3. Modify `helpers.rs:2830` to gate `!range` emission on `strategy.metadata_mode != None`
4. Modify `emit_toplevel.rs:1711` to gate `!prof` emission on `strategy.metadata_mode != None`

**Verification:**
- nbody with while-loop + no metadata → SLP should find the -283 reduction
- All other benchmarks: metadata continues as before (metadata_mode = All)

---

## nbody Recovery Path (Post-Refactor)

After commit E, nbody_newton uses:
- `loop_style: WhileLoop` (peak=33≥16)
- `hot_loop_attr: MemoryReadWrite` (#9 — unguarded_ffi detection)
- `slp_mode: Enabled` (chain_pass_ok + is_reduction_pattern)
- `inline_mode: NoFunction` (body in @main directly)
- `metadata_mode: None` (no !range/!prof obscuring the hot loop)

Expected: SLP finds the -283 horizontal reduction, nbody_newton recovers from 1.09x toward 0.85x.

If not: the remaining gap is from other structural differences (noundef, state_ptr_param, etc.) that exist in @main's emission independently of metadata. These would need separate investigation.

---

## Risk Assessment

| Risk | Impact | Mitigation |
|------|--------|-----------|
| `StrategyAnalyzer::analyze()` called before all context is ready | Crash | Call after `build_field_index`, `apply_field_modes`, `estimate_slp_hazard` — all data is populated |
| New `strategy.rs` module not picked up by build system | Compile error | `cargo test --lib` catches this immediately |
| `metadata_mode = None` on while-loop breaks benchmarks that need metadata | Wrong output or regression | Only nbody uses while-loop with peak≥16. Other while-loop users (ring_buffer, fasta) have tiny states (≤4 fields) — peak=0 < 16, so they keep PerFieldPhi or their existing path |
| `hot_loop_attr` change to `#11`/`#12` regresses benchmarks | Performance regression | B axis is gated by `has_unguarded_ffi` — only nbody-family benefits from `#11`. All others have `has_unguarded_ffi`? Actually no — we need precise FFI detection |
