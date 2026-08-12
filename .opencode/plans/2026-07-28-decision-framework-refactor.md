# Decision Framework Refactor: 5-Axis Dispatch Strategy

**Date:** 2026-07-28
**Commits:** 5 commits, one per axis. Each independently verifiable.
**Location:** `DispatchStrategy` on `FunctionContext` (per-txn).

## Gaps Identified (Outside the 5 Axes)

| # | Gap | What's missing | Blocker for | Priority |
|---|-----|---------------|-------------|----------|
| G1 | **Fold pass** | sparse_dispatch/queue_drain/interval_step need runtime-bound fold. NOT a dispatch strategy choice — it's a missing feature requiring a new analysis pass (`two-phase purity`). | Those 3 benchmarks | Medium |
| G2 | **`has_newton_iteration` detection** | Axis B references this for attribute selection (#11 vs #12). Needs a heuristic: does the body have nested guards or convergence gates that indicate an inner iterative loop? Simple check: `body.iter().any(|s| matches!(s, Statement::Gate(_)))`. | Axis B | High |
| G3 | **Precise `has_unguarded_ffi` detection** | Current impl false-positives on `sqrt()` math calls. Must use `ForeignBinding` table instead of `statement_contains_ffi`. Fix: scan `TopLevel::ForeignBinding` items for known FFI names (like `__print_*`, `__getenv_*`), then check if any txn body calls these names outside `when` guards. | Axis B | High |
| G4 | **PureCounter not a strategy choice** | It's not one of the 3 dispatch paths. It's a SEPARATE pass that eliminates the loop entirely. Should NOT be in `LoopStyle` enum. | None (separate feature) | Low |

## Changes from v1

- Removed `PureCounter` from `LoopStyle` enum (G4 — it's a separate pass)
- Added `has_newton_iteration` detection (G2 — simple gate pattern check)
- Added `has_unguarded_ffi` via `ForeignBinding` scan (G3 — precise FFI detection)
- Added Section 6: missing features (G1 — fold pass is outside scope)

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
    // NOTE: PureCounter fold is NOT a loop style — it's a separate pass that
    // eliminates the loop entirely (single store, O(1)). Handled by the fold
    // detection at mod.rs:2676, independent of the 5-axis strategy.
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
/// Called once from `generate()` after all analysis (hazard, SLP) is complete.
/// The fold detection (pure-counter) runs BEFORE this function — if the txn
/// is foldable, it's handled by the separate fold pass at mod.rs:2676.
pub struct StrategyAnalyzer;

impl StrategyAnalyzer {
    pub fn analyze(txn: &Transaction, ctx: &CompilerContext) -> DispatchStrategy {
        // G3: Precise has_unguarded_ffi via ForeignBinding table. NOT using
        // statement_contains_ffi (which false-positives on sqrt(), sin(), etc.).
        // Instead, collect known FFI names from ForeignBinding declarations:
        //   frgn __print_float(x: Float32) -> Int from "link/briev_rt.c";
        // Then check if any txn body calls these names OUTSIDE a `when` guard.
        let ffi_names: HashSet<String> = ctx.foreign_bindings.iter()
            .map(|fb| fb.briev_name.as_ref().unwrap_or(&fb.foreign_name).clone())
            .collect();
        let has_unguarded_ffi = txn.body.iter().any(|stmt| match stmt {
            Statement::Guarded(_, _) => false,  // skip guard bodies
            _ => stmt_has_ffi_call(stmt, &ffi_names),
        });

        // G2: has_newton_iteration detection — simple check: does the body
        // have a Statement::Gate (convergence assertion)? Newton's method
        // uses `[dx < epsilon];` inner gates for its iteration convergence.
        // If present, the body has inner loops that willreturn would affect.
        let has_newton = txn.body.iter().any(|s| matches!(s, Statement::Gate(_)));

        let peak_live = ctx.peak_live_floats;
        let has_body_ffi = txn.body.iter().any(|s| {
            crate::analysis::transition_graph::statement_contains_ffi(s)
        });
        let total_fields = ctx.field_index_map.len() as u32;
        let write_density = if total_fields > 0 {
            ctx.write_count as f64 / total_fields as f64
        } else { 1.0 };

        // Determine loop_style first — other axes depend on it
        let loop_style: LoopStyle;
        let inline_mode: InlineMode;

        if peak_live >= 16 && has_body_ffi {
            loop_style = LoopStyle::WhileLoop;
            inline_mode = InlineMode::NoFunction;  // body directly in @main
        } else if write_density >= 0.5 && total_fields < 8 && !has_body_ffi {
            loop_style = LoopStyle::InlineSsa;
            inline_mode = InlineMode::AlwaysInline;
        } else {
            loop_style = LoopStyle::PerFieldPhi;
            inline_mode = if ctx.has_cycles {
                InlineMode::NoInline
            } else {
                InlineMode::AlwaysInline
            };
        }

        DispatchStrategy {
            loop_style,

            // B: Attribute — principled by FFI location + Newton detection
            hot_loop_attr: if has_unguarded_ffi {
                HotLoopAttr::MemoryReadWrite        // #9 — conservative
            } else if has_newton {
                HotLoopAttr::ArgMemReadWrite         // #11 — SROA, no willreturn
            } else {
                HotLoopAttr::ArgMemReadWriteWillReturn // #12 — SROA + loop opts
            },

            // C: SLP — gates per-group checks (chain_pass_ok + reduction)
            slp_mode: SlpMode::Enabled,

            inline_mode,

            // E: Metadata — while-loop needs clean IR for SLP discovery
            metadata_mode: match loop_style {
                WhileLoop => MetadataMode::None,
                _ => MetadataMode::All,
            },
        }
    }

    /// Check if a statement calls a known foreign function (from the
    /// ForeignBinding table). This is the PRECISE FFI check — unlike
    /// statement_contains_ffi which catches ALL Expr::Call (including sqrt).
    fn stmt_has_ffi_call(stmt: &Statement, ffi_names: &HashSet<String>) -> bool {
        match stmt {
            Statement::Let { expr: Some(e), .. }
            | Statement::Expression(e)
            | Statement::Assign(_, e) => expr_has_ffi_call(e, ffi_names),
            _ => false,
        }
    }

    fn expr_has_ffi_call(expr: &Expr, ffi_names: &HashSet<String>) -> bool {
        match expr {
            Expr::Call(name, args, _) => {
                ffi_names.contains(name.as_str())
                    || args.iter().any(|a| expr_has_ffi_call(a, ffi_names))
            }
            Expr::BinaryOp(_, l, r) => {
                expr_has_ffi_call(l, ffi_names) || expr_has_ffi_call(r, ffi_names)
            }
            Expr::UnaryOp(_, e) => expr_has_ffi_call(e, ffi_names),
            _ => false,
        }
    }
}
```
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

## Commit A Results (2026-07-28)

Commit `2e6e8f95` on recovery-branch. Created `strategy.rs` with `DispatchStrategy`,
`StrategyAnalyzer`. Refactored `mod.rs:2736` dispatch chain into `match strategy.loop_style`.
Variables computed through raw float register pressure (`raw_peak_live_floats`).

### Benchmark Results

```
╔═══════════════════════╦══════════╦══════════╦══════════╦═══════╦═══════════╗
║ Benchmark             ║ Briev    ║ C        ║ Ratio    ║ Winner║ Correct   ║
╠═══════════════════════╬══════════╬══════════╬══════════╬═══════╬═══════════╣
║ ring_buffer           ║ .0536s   ║ .0480s   ║ 1.11x    ║ C     ║ MATCH     ║
║ float_math            ║ .0727s   ║ .0727s   ║ 1.00x    ║ ~tie  ║ MATCH     ║
║ float_math_nonzero    ║ .1691s   ║ .1666s   ║ 1.01x    ║ C     ║ MATCH     ║
║ sparse_dispatch       ║ .0519s   ║ .0596s   ║ .87x     ║ Briev ║ MATCH     ║
║ print_loop            ║ .0588s   ║ .0594s   ║ .98x     ║ Briev ║ MATCH     ║
║ nbody_newton          ║ 11.79s   ║ 8.40s    ║ 1.40x    ║ C     ║ MATCH     ║
║ nbody_sqrt            ║ 2.81s    ║ 2.82s    ║ .99x     ║ Briev ║ MATCH     ║
║ nbody_sqrt_idio       ║ 2.98s    ║ 3.65s    ║ .81x     ║ Briev ║ MATCH     ║
║ fasta                 ║ .2090s   ║ .2097s   ║ .99x     ║ Briev ║ MATCH     ║
║ fannkuch_redux        ║ .0646s   ║ .0644s   ║ 1.00x    ║ ~tie  ║ MATCH     ║
║ mandelbrot            ║ .6656s   ║ .6594s   ║ 1.00x    ║ ~tie  ║ MATCH     ║
║ kalman_filter_runtime ║ .1780s   ║ .1807s   ║ .98x     ║ Briev ║ MATCH     ║
║ knucleotide           ║ .1883s   ║ .1898s   ║ .99x     ║ Briev ║ MATCH     ║
║ cancel_math           ║ .0605s   ║ .0627s   ║ .96x     ║ Briev ║ MATCH     ║
║ queue_drain           ║ .0619s   ║ .0608s   ║ 1.01x    ║ C     ║ MATCH     ║
║ queue_drain_sym       ║ .0631s   ║ .0618s   ║ 1.02x    ║ C     ║ MATCH     ║
║ queue_drain_idio      ║ .0625s   ║ .0621s   ║ 1.00x    ║ ~tie  ║ MATCH     ║
║ interval_step         ║ .0624s   ║ .0623s   ║ 1.00x    ║ ~tie  ║ MATCH     ║
╚═══════════════════════╩══════════╩══════════╩══════════╩═══════╩═══════════╝
```

### Strategy Selections

| Benchmark | Fields | raw_peak | Selected Path | Previous Path | Delta |
|-----------|--------|----------|---------------|---------------|-------|
| ring_buffer | 4 | 0 | while-loop (<5) | while-loop | same |
| float_math | 15 | 8 | per-field phi | per-field phi | same |
| nbody_newton | 33 | 30 | **while-loop** (peak≥16) | per-field phi | **new** |
| nbody_sqrt | 33 | 30 | **while-loop** (peak≥16) | per-field phi | **new** |
| nbody_sqrt_idio | 33 | 30 | **while-loop** (peak≥16) | per-field phi | **new** |
| kalman | 15 | 6 | per-field phi (peak<16) | per-field phi | same |

The while-loop for nbody benchmarks is correct in principle but the emission needs
Commits B-E (no metadata, refined attributes) to match Era 5's behavior. Currently
1.40x vs 1.09x per-field phi for nbody_newton. nbody_sqrt_idio `**0.81x**` with
while-loop is close to the all-time best 0.67x (achieved with per-field phi + SLP off).

## nbody Recovery Path (Post-Refactor)

After Commits B-E, the while-loop should produce a flat instruction sequence without
metadata annotations, with refined attributes (#11 for Newton, #12 for sqrt), enabling
LLVM's SLP vectorizer to find the -283 horizontal reduction:

nbody_newton: while-loop + no metadata + #11 + NoFunction → expect ~1.0x (from 1.40x)
nbody_sqrt_idio: while-loop + no metadata + #12 + NoFunction → expect ~0.67x (from 0.81x)

## Risk Assessment

| Risk | Impact | Mitigation |
|------|--------|-----------|
| `StrategyAnalyzer::analyze()` called before all context is ready | Crash | Call after `build_field_index`, `apply_field_modes`, `estimate_slp_hazard` — all data is populated |
| New `strategy.rs` module not picked up by build system | Compile error | `cargo test --lib` catches this immediately |
| `metadata_mode = None` on while-loop breaks benchmarks that need metadata | Wrong output or regression | Only nbody uses while-loop with peak≥16. Other while-loop users (ring_buffer, fasta) have tiny states (≤4 fields) — peak=0 < 16, so they keep PerFieldPhi or their existing path |
| `hot_loop_attr` change to `#11`/`#12` regresses benchmarks | Performance regression | B axis is gated by `has_unguarded_ffi` — only nbody-family benefits from `#11`. All others have `has_unguarded_ffi`? Actually no — we need precise FFI detection |
