# Abstraction Discovery for Depth-Bounded Enumerative Synthesis

Date: 2026-07-29
Status: Plan

## Executive Summary

The enumerative synthesis engine (`src/derive/engine.rs`) hits a combinatorial
wall at depth 4 (beam: ~16000 candidates, raw space: ~4 million). We add an
abstraction-discovery phase — adapted from Koza's Automatically Defined
Functions (ADFs) [GP'92], Feser et al.'s λ² lambda abstraction [PLDI'15], and
Polozov & Gulwani's version-space algebra [POPL'15] — that extracts reusable
sub-expressions from the pruned LevelCache and promotes them to first-class
helper functions. Composition at depth N+1 then uses the helper call DAG
instead of the raw expression cross product, compressing the search space.

The mechanism is entirely additive (see AGENTS.md §5): no existing optimization
paths are modified. Helpers live only during search and are garbage-collected
if unused. The final output emits `defn _hN(...) { ... }` in the doppelganger
for only the consumed helpers.

## Literature Grounding

### Automatically Defined Functions (ADFs) — Koza 1992 [GP'92]

Koza, J. R. "Genetic Programming: On the Programming of Computers by Means of
Natural Selection." MIT Press, 1992. Chapter 6: "Automatically Defined
Functions."

ADFs are subroutines discovered during genetic programming that are added to
the primitive set for subsequent evolution. A function-defining branch and a
value-returning branch are co-evolved; the ADF body is discovered via the same
evolutionary process as the main program. The key insight: once a useful
computation pattern emerges, it should be named and reused rather than
re-evolved in each context.

This plan's `discover_helpers()` mirrors ADFs but replaces GP's crossover
operators with deterministic frequency/cost analysis over the LevelCache.
Koza's ADFs required the user to specify the number and arity of ADFs
beforehand; this plan discovers them automatically from the pruned expression
pool.

### Sub-expression Reuse in Symbolic Regression — Schmidt & Lipson 2009 [SL'09]

Schmidt, M. D. & Lipson, H. "Distilling Free-Form Natural Laws from
Experimental Data." *Science* 324(5923), 2009.

Eureqa uses Pareto-optimization over model complexity and accuracy. It
discovers reusable sub-expressions by tracking which expression trees share
common subtrees. Sub-expression reuse is scored by how much complexity it
saves across the Pareto frontier.

This plan's cost-savings scoring (`savings = body_cost × use_count -
call_cost × use_count - decl_overhead`) is directly adapted from Eureqa's
complexity-savings metric, applied to the Occam cost model that already exists
(`CostModel` in `engine.rs:18`).

### Version-Space Algebra — Gulwani 2011, Polozov & Gulwani 2015 [PG'15]

Gulwani, S. "Automating String Processing in Spreadsheets Using Input-Output
Examples." POPL 2011.

Polozov, O. & Gulwani, S. "FlashMeta: A Framework for Inductive Program
Synthesis." POPL 2015. §3: "Version Space Algebra."

A version-space is a compact representation of all programs consistent with a
set of examples. In FlashMeta, operations produce new version-spaces from old
ones via join, compose, and union. The key scaling insight: version-spaces
grow in width (number of components) not depth (recursive expansion), keeping
search tractable.

This plan's `LevelCache` is a primitive version-space (per-type expression
sets). The missing operation is **abstraction**: extracting a subset of
expressions into a named, reusable component that can be referenced at higher
depths. This is exactly the operation that transforms a flat version-space
into a hierarchical one. Without it, the space grows as O(N^depth) instead of
O(N × components^depth).

### Lambda Abstraction in Synthesis — Feser, Chaudhuri, Dillig 2015 [FCD'15]

Feser, J. K., Chaudhuri, S. & Dillig, I. "Synthesizing Data Structure
Transformations from Input-Output Examples." PLDI 2015. §4: "Abstraction."

λ² synthesizes recursive programs on algebraic data types by discovering
lambda abstractions that capture common sub-computations. An abstraction
`λ(x, y). body` is extracted when the same sub-expression pattern appears in
multiple branches of a conditional or recursive call. The abstraction is then
applied via β-reduction at synthesis time.

This plan's helper-as-abstraction differs from λ² in two ways:
1. λ² discovers abstractions via anti-unification of conditional branches;
   this plan discovers them via frequency analysis over the LevelCache.
2. λ² abstractions are permanent (they define the program structure); this
   plan's helpers are ephemeral (discarded if unused).

### Bottom-Up Enumerative Synthesis — Udupa et al. 2013 [U+13]

Udupa, A., Raghavan, A., Deshmukh, J. V., Mador-Haim, S., Martin, M. M. K. &
Alur, R. "TRANSIT: Specifying Protocols with Concolic Snippets." PLDI 2013.
§4.1: "Bottom-Up Enumerative Search."

TRANSIT enumerates expressions bottom-up: all expressions of size 1, then
size 2 composed from size 1, etc. The key insight: each size's expressions
are cached and reused as building blocks for the next size. This is the same
pattern as our `LevelCache` + `generate_next_level()` but without abstraction.

TRANSIT's limitation (and ours) is that the building block set grows
monotonically — no abstraction step ever shrinks it back. The helper discovery
in this plan adds the missing abstraction step, allowing the building block
set to be "refactored" into smaller, reusable components between depth levels.

### Counterexample-Guided Inductive Synthesis — Solar-Lezama 2008 [SL'08]

Solar-Lezama, A. "Program Synthesis by Sketching." PhD Thesis, UC Berkeley,
2008. Chapter 2: "The Sketching Approach."

CEGIS: sketch + spec → synthesizer → verify. If verification finds a
counterexample, refine the sketch and repeat. This is the 5-iteration loop
already in `synthesize()` (`mod.rs:71`).

This plan does not modify the CEGIS loop. Helpers are purely a search-space
reduction technique within the enumerative oracle used by CEGIS. The existing
5-iteration loop, Z3 verification (`verify_smt.rs`), and random verification
(`verify.rs`) are untouched.

### Relevance to This Codebase

| Citation | Idea | Previous occurrence in codebase |
|----------|------|--------------------------------|
| Koza 1992 (ADFs) | Promote subroutines to primitives during search | Zero references |
| Schmidt & Lipson 2009 (Eureqa) | Cost-savings scoring for sub-expression reuse | Zero references |
| Gulwani/Polozov 2011-2015 (Version-spaces) | Hierarchical vs flat enumeration | LevelCache exists but lacks abstraction |
| Feser et al. 2015 (λ²) | Lambda abstraction from common sub-patterns | Zero references |
| Udupa et al. 2013 (TRANSIT) | Bottom-up caching without abstraction | LevelCache implements caching only |
| Solar-Lezama 2008 (CEGIS) | Counterexample-driven refinement | Already implemented (mod.rs:71) |

## System-Level Data Flow (Checklist §1)

Current flow (unchanged paths in `monospace`, new paths in **bold**):

```
synthesize_enumerative()
  for depth in 1..=max_depth {
    candidates = generate_next_level(params, ret_type, &prev_cache)
      ├── constants + variables
      ├── unary ops on prev_level
      ├── binary ops: cross product of prev_level per type
      ├── IF expressions: prev bool × prev then × prev else
      └── Call/Match for compound types

    [synthesize_enumerative: sort by cost, apply beam]

    for each (cost, candidate) in sorted {
      if candidate_matches_all_examples() → store as best
      push to next_level
    }

    prev_cache = prune_level(next_level, ...)
      ├── evaluate all candidates
      ├── fingerprint: prune constant-output and redundant
      └── store in LevelCache per type

    **NEW: if depth >= 2 {
      helpers = discover_helpers(&prev_cache, depth)
        ├── extract sub-expressions from LevelCache
        ├── score by frequency × cost-savings
        └── return top-k helpers
      register_helpers(&mut prev_cache, &helpers)
        └── inject Expr::Call(helper_name, args) into LevelCache
    }**
  }
```

### Decision Points (Checklist §5)

| # | Decision | Location | What determines it |
|---|----------|----------|--------------------|
| 1 | When to discover | `synthesize_enumerative()` after prune, if depth ≥ 2 | Frequency threshold `MIN_FREQ_PCT` (config, default 5%) |
| 2 | Which sub-expressions are candidates | `discover_helpers()` | All depth-2 sub-trees of LevelCache expressions |
| 3 | Whether to promote a candidate | `discover_helpers()` | cost_savings = body_cost × use_count - call_cost × use_count - decl_overhead. Promote if savings > 0 |
| 4 | How many helpers to register | `register_helpers()` | `MAX_HELPERS_PER_TYPE` (config, default 20) |
| 5 | Whether a helper survives | `generate_next_level()` (implicit) | Reference count > 0 after next depth |
| 6 | Whether to emit in output | `format_body()` in doppelganger | Helper is consumed by final expression |

### Verification Hypothesis (Checklist §6)

**Hypothesis H1**: Adding abstraction discovery between depths reduces the
candidate-to-solution ratio at depth 4 by ≥10×, because the cross product of
k helpers replaces the cross product of O(N) raw expressions at depth 3.

**Test**: `synthesize_enumerative` with the same examples produces a solution
at depth 4 using ≤ beam_width candidates, where without helpers it would
exhaust the beam and return NoSolution. Test on popcount, abs, minmax.

**Falsification**: If with helpers active, depth-4 search produces NO solution
that wasn't already found by depth-3 (or by unmodified depth-4), then the
helpers are not contributing. We would observe `best.depth <= 3` in all cases.

**Counterfactual**: If we run with helpers but artificially set
`MAX_HELPERS_PER_TYPE = 0`, the engine must behave identically to the current
unmodified engine (same candidates, same pruning, same results). This is the
regression guard.

## Design Specification

### 1. New Types

```rust
// ── src/derive/library.rs ──────────────────────────────────────────
// 2026-07-29: Abstraction discovery for depth-bounded enumerative
// synthesis. Extracts reusable sub-expressions from the LevelCache
// and promotes them to helper functions. Adapts Koza's ADF [GP'92]
// and Feser et al.'s λ abstraction [PLDI'15] to the depth-bounded
// enumerative search. Helpers are ephemeral: discovered, registered,
// potentially consumed, then garbage-collected if unused.
// Flat code: each function max 2 levels of nesting.

/// 2026-07-29: A helper function discovered during synthesis.
/// Represents a reusable sub-expression extracted from the LevelCache.
/// Name is auto-generated ("_h0", "_h1", ...).
/// Params are the free variables in the sub-expression.
/// Body is the extracted expression tree.
/// Fields are all pub for LevelCache iteration; this is an internal type.
#[derive(Debug, Clone)]
pub struct HelperFunction {
    /// Auto-generated name ("_h0", "_h1", ...)
    pub name: String,
    /// Parameter names (free variables of the extracted sub-expression)
    pub params: Vec<String>,
    /// Parameter types corresponding to params
    pub param_types: Vec<String>,
    /// The extracted expression body
    pub body: Expr,
    /// Return type of the helper
    pub ret_type: String,
    /// The body's full cost (for debugging/provenance)
    pub body_cost: u64,
    /// Cost to CALL the helper (cheaper than body to incentivize reuse)
    pub call_cost: u64,
    /// How many candidates at the next depth reference this helper
    pub use_count: usize,
}
```

### 2. Discovery Algorithm

```rust
// 2026-07-29: Discover useful helper functions from the pruned LevelCache.
// Algorithm (adapted from Koza ADF [GP'92] §6.3 and Schmidt/Lipson Eureqa
// [SL'09] cost-savings metric):
//
// 1. For each expression in the LevelCache at depth >= 2, extract all
//    depth-2 sub-trees (Expr nodes whose children are depth 1 or less).
//    A depth-2 sub-tree corresponds to a binary operation on variables
//    and constants, or a unary operation on a variable.
//
// 2. Score each sub-tree by:
//    - frequency: how many LevelCache expressions contain this sub-tree
//    - cost_savings = body_cost × use_count
//                     - call_cost × use_count
//                     - decl_overhead (fixed, = call_cost × 2)
//
// 3. Return top-k (MAX_HELPERS_PER_TYPE) where cost_savings > 0.
//
// 4. Pruning: skip sub-expressions that are:
//    - constant-only (no variables → not reusable across inputs)
//    - identity operations (handled by existing is_identity_op)
//    - already in the helper set (dedup by structural equality)
//    - larger than max_helper_depth (default 2)
//
// The frequency analysis uses the LevelCache's evaluated expressions —
// not the raw candidates — because the LevelCache is the pruned,
// non-redundant set. If a sub-tree appears in 10% of LevelCache
// expressions at depth N, it's a good abstraction candidate.
//
// Flat code: max 2 nesting levels enforced by extraction into named
// helper functions (collect_sub_trees, score_sub_trees).
pub fn discover_helpers(
    cache: &LevelCache,
    param_names: &[String],
    param_types: &[String],
    config: &DiscoverConfig,
) -> Vec<HelperFunction>;
```

### 3. Registration

```rust
// 2026-07-29: Register discovered helpers into the LevelCache so that
// generate_next_level() can reference them via Expr::Call.
//
// Each helper is injected into the appropriate per-type bucket in the
// LevelCache (e.g., an Int-returning helper goes into int_exprs).
// The expression stored is Expr::Call(helper_name, params, None) where
// params are the helper's free variables as Expr::Identifier nodes.
//
// The cost model for a helper call is call_cost (lower than body_cost
// to amortize over multiple uses). The existing cost_of_expr handles
// Expr::Call via:
//
//   Expr::Call(_, args, _) => 3 + sum(args costs)
//
// which means a helper call costs 3 + N × variable_cost. This is
// naturally cheaper than the helper body (which contains the full
// operator tree) for any helper with body_cost > 3.
//
// No modification needed to CostModel::cost_of_expr.
pub fn register_helpers(
    cache: &mut LevelCache,
    helpers: &[HelperFunction],
);
```

### 4. Integration into Depth Loop

In `synthesize_enumerative()` (`engine.rs:1145`), after the existing
`prune_level()` call:

```rust
        // 2026-07-29: Abstraction discovery — extract reusable
        // sub-expressions from the pruned LevelCache and promote to
        // helper functions. Adapted from Koza's ADFs [GP'92] and
        // Feser et al.'s λ² abstraction [PLDI'15].
        // Only activate at depth >= 2, because depth-1 expressions
        // are just variables and constants — no useful abstraction.
        if depth >= 2 {
            let helpers = discover_helpers(
                &prev_cache,
                param_names,
                param_types,
                &DISCOVER_CONFIG,
            );
            if !helpers.is_empty() {
                // 2026-07-29: Store helpers on SynthesizedProgram for
                // doppelganger emission and reference tracking.
                // Initially all helpers are unused; reference counts
                // are incremented during generate_next_level.
                register_helpers(&mut prev_cache, &helpers);
                // Track for final emission filtering
                helper_registry.extend(helpers);
            }
        }
```

### 5. Helper Call Generation in `generate_next_level()`

In `generate_next_level()` (`engine.rs:724`), after existing per-type loops:

```rust
    // 2026-07-29: Helper call generation — emit Expr::Call for each
    // helper whose return type matches the current target type.
    // A helper call has cost = call_cost, which is cheaper than the
    // helper body (amortized over multiple uses). The helper's params
    // are mapped to current-level parameter names by position.
    //
    // This is the key scaling improvement: instead of O(N²) raw
    // expression combinations, we get O(H × N) where H = helpers.
    // If H=20 and N=500, this is 10,000 candidates vs 250,000.
    //
    // Flat code: single loop over private helper list.
    for helper_name in &prev.helper_names {
        if let Some(h) = helper_map.get(helper_name) {
            if h.ret_type == ret_type && !h.params.is_empty() {
                let args: Vec<Expr> = h.params.iter()
                    .map(|p| Expr::Identifier(p.clone()))
                    .collect();
                result.push(Expr::Call(helper_name.clone(), args, None));
            }
        }
    }
```

### 6. Reference Counting and GC

Each helper starts with `use_count = 0`. During `generate_next_level()`, when a
helper call is generated, the caller checks the helper_map and increments the
helper's use count. After depth N+1 completes (prune is done), any helper with
`use_count == 0` is removed from the LevelCache and the global helper list.

```rust
// 2026-07-29: Garbage-collect unused helpers after each depth level.
// A helper with zero references at depth N+1 is unlikely to be used
// at depth N+2 (the search is monotonically widening). Removing it
// keeps the helper set from accumulating irrelevant abstractions.
// This is the ephemeral lifecycle: helpers live only during the
// depths where they're actively referenced.
fn gc_helpers(
    cache: &mut LevelCache,
    helpers: &mut Vec<HelperFunction>,
) {
    helpers.retain(|h| h.use_count > 0);
    // Remove from LevelCache's helper_names
    let active: HashSet<String> = helpers.iter().map(|h| h.name.clone()).collect();
    cache.helper_names.retain(|n| active.contains(n));
}
```

### 7. Doppelganger Output

In `format_body()` (`doppelganger.rs` or new helper), emit helpers before the
synthesized function:

```rust
// 2026-07-29: Emit discovered helper functions as defn blocks.
// Only helpers with use_count > 0 are emitted — unused helpers
// are silently discarded. This is the "ephemeral library" concept:
// helpers are created during search, used if beneficial, and
// only persisted in output if consumed.
fn emit_helpers(out: &mut String, helpers: &[HelperFunction]) {
    for h in helpers {
        if h.use_count == 0 { continue; }
        let params: Vec<String> = h.params.iter()
            .zip(h.param_types.iter())
            .map(|(n, t)| format!("{}: {}", n, t))
            .collect();
        out.push_str(&format!(
            "// 2026-07-29: Auto-discovered helper (scope-based abstraction via ADF extraction)\n\
             defn {}({}) -> {} {{ {} }};\n\n",
            h.name,
            params.join(", "),
            h.ret_type,
            body_to_string(&h.body),
        ));
    }
}
```

### 8. Configuration

```rust
// 2026-07-29: Configuration for abstraction discovery.
// These are constants for now; can be promoted to --derive flags.
// The defaults are conservative: discovery activates at depth 2,
// requires 5% frequency, and caps at 20 helpers per type.
#[derive(Debug, Clone)]
pub struct DiscoverConfig {
    /// Minimum depth for discovery (2 = after binary ops exist)
    pub min_depth: u8,
    /// Minimum frequency (0.0-1.0) for a sub-expression to be considered
    pub min_frequency: f64,
    /// Maximum helpers per type (Int, Float, Bool)
    pub max_helpers_per_type: usize,
    /// Maximum depth of sub-tree to extract (depth = 2 means binary ops)
    pub max_helper_depth: u8,
    /// Fixed overhead for helper declaration (in cost units)
    pub decl_overhead: u64,
}

impl Default for DiscoverConfig {
    fn default() -> Self {
        DiscoverConfig {
            min_depth: 2,
            min_frequency: 0.05,
            max_helpers_per_type: 20,
            max_helper_depth: 2,
            decl_overhead: 6, // defn + call site = ~2× call_cost
        }
    }
}
```

## Changes to Existing Types

### `LevelCache` (engine.rs:589)

Add a field for helper names:

```rust
struct LevelCache {
    int_exprs: Vec<Expr>,
    float_exprs: Vec<Expr>,
    bool_exprs: Vec<Expr>,
    compound_exprs: HashMap<String, Vec<Expr>>,
    // 2026-07-29: Names of registered helper functions.
    // Stored separately from per-type exprs because helper calls
    // are generated from name-to-type mapping, not from raw exprs.
    // Iterated in generate_next_level() to find matching-return-type
    // helpers. See abstraction-discovery plan §5.
    helper_names: Vec<String>,
}
```

### `SynthesizedProgram` (engine.rs:1134)

Add a field for consumed helpers:

```rust
pub struct SynthesizedProgram {
    pub body: Vec<Expr>,
    pub cost: u64,
    pub depth: u8,
    // 2026-07-29: Helper functions consumed by the synthesized body.
    // Only helpers with use_count > 0 are included. Emitted as defn
    // blocks before the main function in doppelganger output.
    // See abstraction-discovery plan §7.
    pub helpers: Vec<HelperFunction>,
}
```

## Test Strategy

### Unit Tests (in `src/derive/library.rs`)

| Test | What it verifies | Behavioral assertion |
|------|------------------|---------------------|
| `test_discover_empty_cache` | Discovery with no expressions returns empty vec | `helpers.is_empty()` |
| `test_discover_single_expr` | Single expression with no reusable sub-trees | `helpers.is_empty()` |
| `test_discover_reusable_add` | `x+y` appearing in ≥5% of LevelCache | `helpers[0].body` is `x + y`, `helper[0].use_count` reflects frequency |
| `test_discover_frequency_threshold` | Sub-tree at 3% frequency is below 5% threshold | Not returned |
| `test_discover_const_only_rejected` | Sub-tree with only constants (e.g., `1+1`) | Not returned |
| `test_discover_dedup` | `x+y` and `y+x` from commutative ops | Only one appears in results |
| `test_register_helpers` | `register_helpers()` adds to `int_exprs` | `cache.int_exprs` contains `Expr::Call("_h0", ...)` |
| `test_register_max_cap` | Registering 25 helpers with cap 20 | Only 20 added |
| `test_gc_unused` | Helper with `use_count=0` is removed | Removed from `cache.helper_names` |
| `test_gc_keeps_used` | Helper with `use_count>0` survives | Still in `cache.helper_names` |

### Integration Tests (in `src/derive/engine.rs`)

| Test | What it verifies |
|------|------------------|
| `test_synthesize_with_helpers_popcount` | popcount at depth 4 succeeds with helpers, fails without |
| `test_synthesize_abs_composition` | `abs(x) + abs(y)` finds solution at depth 4 with helpers |
| `test_helper_identity_disabled` | With `MAX_HELPERS_PER_TYPE=0`, results match unmodified engine exactly |

### Regression Guard (Checklist §7-9)

1. **Match arm inspection**: The only new match arms in `generate_next_level()`
   are the helper loop — no existing arms are modified. `register_helpers()`
   pushes to existing Vec fields, never clears them.

2. **IR verification**: Not applicable (no backend changes).

3. **Architecture comments**: All added code sites have `// 2026-07-29:` with
   literature citations. The `LevelCache` doc comment on `helper_names` refers
   to this plan.

## File Changes Summary

| File | Lines added | Nature |
|------|-------------|--------|
| `src/derive/library.rs` | ~250 | New: HelperFunction, discover_helpers, register_helpers, gc_helpers, DiscoverConfig |
| `src/derive/mod.rs` | ~10 | Add `mod library; pub use library::*;` |
| `src/derive/engine.rs` | ~60 | LevelCache: +helper_names; SynthesizedProgram: +helpers; synthesize_enumerative: +discovery call; generate_next_level: +helper loop |
| `src/derive/doppelganger.rs` | ~30 | emit_helpers() and call in format_body() |
| `src/derive/accept.rs` | ~15 | Fold helper defns into source with // auto-discovered comment |
| `docs/architecture/features/derivation-blocks.md` | ~15 | Reference to abstraction discovery phase |

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Wrong helpers pollute search space | Medium | Low-Medium | Cost-savings threshold ensures only beneficial helpers survive. GC removes unused ones after each depth. |
| Helper combinatorics at high depth | Low | Medium | MAX_HELPERS_PER_TYPE=20 keeps cross product manageable. 20² = 400 binary ops = 2.5% of beam. |
| Helper call cost skews cost ordering | Low | Low | call_cost = 3 + N × 1 (Expr::Call baseline in CostModel). This is already correct for any helper body_cost > 3. |
| Discovered helper never used | Medium | None | GC removes it. No emitted code. |

## Plan Directives Compliance

| Directive | How this plan meets it |
|-----------|----------------------|
| **§1: FLAT CONTROL FLOW** | `discover_helpers` extracts sub-trees via named helpers `collect_sub_trees` and `score_sub_trees`. The depth loop in `synthesize_enumerative` adds 4 new lines (if depth >= 2 { ... }), maintaining 2-level nesting. |
| **§2: COMMENT THE CODE** | Every new code site gets `// 2026-07-29:` with literature citation and rationale. No existing rationale comments are modified. |
| **§3: UPDATE ALL EXAMPLES** | No syntax changes (helpers are purely a compiler-internal mechanism). Doppelganger output may contain `_hN` defns; existing examples are unchanged. |
| **§4: DOCUMENTATION IS CODE** | `derivation-blocks.md` gets a reference to abstraction discovery. This plan document is the ground truth. |
| **§5: BEHAVIORAL TESTS, NOT LITERAL** | Tests assert discovery frequency, cost-savings scoring, GC lifecycle, and solution existence — not specific helper names or IR snapshots. |

## Implementation Order

### Step 1: `src/derive/library.rs` — HelperFunction + discover_helpers + register_helpers + gc_helpers
- Types, algorithm, scoring. Unit tests.

### Step 2: `engine.rs` — LevelCache + SynthesizedProgram changes + integration in synthesize_enumerative
- Add `helper_names`, `helpers` fields.
- Call discovery after prune_level, register, GC after each depth.
- Add helper call generation in `generate_next_level()`.
- Integration tests.

### Step 3: `doppelganger.rs` + `accept.rs` — Helper emission
- Emit helper defn blocks.
- Fold helpers on brief accept.

### Step 4: Documentation update
- `derivation-blocks.md` — abstraction discovery phase.

### Step 5: `cargo test --lib` + `cargo build` — Full verification

## References

[GP'92] Koza, J. R. *Genetic Programming: On the Programming of Computers by
Means of Natural Selection.* MIT Press, 1992. Chapter 6.

[SL'09] Schmidt, M. D. & Lipson, H. "Distilling Free-Form Natural Laws from
Experimental Data." *Science* 324(5923), 2009.

[PG'15] Polozov, O. & Gulwani, S. "FlashMeta: A Framework for Inductive
Program Synthesis." POPL 2015.

[FCD'15] Feser, J. K., Chaudhuri, S. & Dillig, I. "Synthesizing Data Structure
Transformations from Input-Output Examples." PLDI 2015.

[U+13] Udupa, A. et al. "TRANSIT: Specifying Protocols with Concolic Snippets."
PLDI 2013.

[SL'08] Solar-Lezama, A. "Program Synthesis by Sketching." PhD Thesis, UC
Berkeley, 2008.
