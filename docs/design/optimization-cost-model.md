# Optimization Cost Model & Chain Composition

**Timeline:** 2026-05-31  
**Author:** Randy Smits-Schreuder Goedheijt (agent-assisted design session)  
**Status:** Design phase — specifying implementation of Phases 3.4 through 4.3  
**Depends on:** `determinism-and-optimization-frontier.md`, Phase 0–4.1 (implemented)

---

## 1. Philosophical Foundation

### 1.1 The O(n) → O(1) Mandate

The optimization framework targets **O(n) → O(1) reduction** on every axis the compiler can prove safe. This is not a heuristic — it's an architectural invariant. If the proof engine verifies bounded convergence, the backend must produce code that approaches constant-time execution for the proven path.

The five reduction axes:

| Axis | Mechanism | Reduction |
|------|-----------|-----------|
| **Tick dispatch overhead** | Folded main: compress `reactor_tick()` dispatch loop into `while(count < N) { body() }` | O(N) dispatch → O(1) canonical loop |
| **Volatile trigger sampling** | Value-set enumeration + switch dispatch | O(N) volatile loads → O(1) pre-sample |
| **Full compile-time evaluation** | If state space ≤ budget, compute all results at compile time | O(N) runtime → O(1) lookup / zero runtime |
| **Sequential transaction chains** | Expression substitution + fused emission | O(N) sequential calls → O(1) fused kernel |
| **Independent region parallelism** | Region analysis partitions work with no false dependencies | Sequential → embarrassingly parallel |

### 1.2 The Optimization Score

Regions are ranked by a **ROI metric** that captures the absolute runtime saved per unit of compile-time budget spent:

```
optimization_score = (body_weight × iteration_count) / max(value_set_size, 1)
```

Where:
- `body_weight` = total statement count across all transactions in the region (proxy for per-iteration cost)
- `iteration_count` = compile-time-resolved loop bound from `bounded_pre` (proxy for total iterations)
- `value_set_size` = product of all trigger value-set sizes in the region (proxy for compile-time cost)

**Why this formula works:**
- A physics engine with `body_weight=42`, `iter=1000`, `enum_cost=2` → score 21000
- A counter loop with `body_weight=5`, `iter=50000000`, `enum_cost=1` → score 250000000
- The counter **correctly ranks higher** — eliminating 50M loop iterations to O(1) is the bigger absolute win, even though per-iteration work is lighter
- An unbounded region (`enum_cost=∞`) → score `—` → never allocated budget

### 1.3 Use Cases

The cost model is designed to serve three concrete domains:

**Video games / physics engines:**
- `Body × ContactPairs` → O(n²) collision resolution → highest priority for precomputation
- `Entity U update` → O(n) per-frame → foldable if bounded
- Multiple independent systems (physics, AI, rendering) → regions → parallelizable

**AI / inference:**
- `MatMul(layer)` → O(n³) pure → GPU-eligible, highest complexity class
- `Activation(tensor)` → O(n) pure → GPU-eligible, foldable
- Sequential pipeline `encode → attend → decode` → chain → composable into single kernel

**Shaders / rendering:**
- `Fragment(uv) → Color` → O(1) per pixel, pure → GPU kernel candidate
- `Vertex(position) → Position` → O(1) per vertex, pure → GPU kernel candidate
- The `rbv` (Rendered Briv) path already targets browser rendering — a WebGPU shader backend is the natural extension

---

## 2. Architecture Overview

### 2.1 Full Analysis Pipeline

```
RegionAnalyzer::analyze(program, transition_graph)
  │
  ├── register_declarations()        Phase A   Collect vars, consts, triggers
  ├── build_dependency_graph()       Phase B   deps, rev_deps, txn_reads/writes/bodies
  ├── seed_frontier()                Phase C   Mark trigger vars as frontier
  ├── propagate_classification()     Phase D   BFS: Pure → Bounded → Opaque
  ├── compute_regions()              Phase E   Connected components in dep graph
  ├── estimate_value_sets()          Phase F   Interval × type constraints → size
  ├── detect_linear_chains()         Phase 4.1  A → B → C dependency traversal
  ├── resolve_iteration_bounds()     NEW       Extract compile-time loop bounds
  ├── compute_region_scores()        NEW       Classify, weight, score each region
  ├── compose_chains()               NEW       Phase 4.2 — expression substitution
  └── build_budget_plan(budget)      NEW       Greedy allocation by score descending
```

### 2.2 Backend Integration

```
LLVM Backend::generate(program, optimize_budget, optimize_report)
  │
  ├── foldable check                  Single bounded txn, no triggers → folded main
  ├── enumerable check                Trigger value sets ≤ budget → switch dispatch
  ├── chain composability check       Composed chains available → fused emission
  │
  ├── [folded path]                   emit_folded_main / emit_folded_pure_counter
  ├── [enumerable path]               emit_enum_main with per-value switch dispatch
  ├── [chain path]                    emit_enum_main calling fused txn functions
  ├── [standard path]                 emit_reactor + emit_main
  │
  └── [report]                        Ranking table + budget allocation plan
```

### 2.3 Data Flow

```
Program AST
    │
    ▼
RegionAnalyzer ──→ AnalysisResults ──→ LLVM Backend
    │                     │                    │
    ├─ var_info           ├─ region_analyzer   ├─ foldable
    ├─ regions            ├─ transition_graph  ├─ enumerable (budget-gated)
    ├─ linear_chains      │                    ├─ enum dispatch
    ├─ region_scores      │                    ├─ fused chain emission
    ├─ composed_chains    │                    └─ optimization report
    └─ budget_plan        │
                          ▼
                   LLVM IR output
```

---

## 3. New Types

### 3.1 Complexity Classification

```rust
/// Classification of a transaction body by computational weight.
///
/// Determines optimization priority: higher-complexity regions
/// benefit more from O(n) → O(1) reduction.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ComplexityClass {
    /// ≤2 statements, pure counter bump — lowest optimization value.
    /// Example: `&count = count + 1;` with no other operations.
    Trivial,

    /// 3–5 statements, simple arithmetic transform.
    /// Example: IIR filter (5 float ops, 4 shift assignments).
    Light,

    /// 6–20 statements, moderate compute.
    /// Example: multiple arithmetic ops with conditionals.
    Medium,

    /// 21+ statements, compute-heavy.
    /// Example: physics integrator, matrix multiply, collision resolution.
    Heavy,

    /// Contains FFI calls or volatile triggers — cannot be fully analyzed.
    /// Partial optimization still possible (segment folding, loop unrolling).
    Unbounded,
}
```

### 3.2 Region Score

```rust
/// Optimization score and metadata for a single atomic reactive region.
///
/// Regions are independent subgraphs in the dependency graph.
/// Each region gets scored for budget allocation priority.
pub struct RegionScore {
    /// Which atomic region (from compute_regions)
    pub region_id: usize,

    /// All transaction names grouped in this region
    pub txn_names: Vec<String>,

    /// Worst complexity class in the region (Trivial < Light < Medium < Heavy < Unbounded)
    pub complexity: ComplexityClass,

    /// Total statement count across all txns in this region
    pub body_weight: usize,

    /// Compile-time-resolved loop bound value from bounded_pre
    /// (e.g., if pre is [count < 50000], this is 50000)
    pub iteration_count: u64,

    /// Product of all trigger value-set sizes in this region.
    /// None = unbounded (at least one trigger has unknown size).
    pub value_set_size: Option<u64>,

    /// body_weight × iteration_count / max(value_set_size, 1)
    /// Higher = better return on compile-time budget investment.
    pub optimization_score: f64,

    /// Whether this region has been fused from a linear transaction chain
    pub chain_composed: bool,

    /// Whether this region is eligible for GPU offload.
    /// Requires: pure body, bounded writes, no volatile triggers, no Term stmts.
    pub gpu_eligible: bool,
}
```

### 3.3 Budget Plan

```rust
/// Result of greedy budget allocation across regions.
///
/// Allocates budget to highest-scoring regions first until
/// either all regions are covered or the compile-time budget is exhausted.
pub struct BudgetPlan {
    /// The compile-time optimization budget (e.g., from --optimize-budget 256)
    pub total_budget: u64,

    /// Regions selected for full enumeration, with their costs and scores.
    /// Format: (region_id, complexity_class, value_set_size_cost, optimization_score)
    pub allocated: Vec<(usize, ComplexityClass, u64, f64)>,

    /// Remaining budget after allocation
    pub residual_budget: u64,

    /// Regions that were not allocated because they exceeded the remaining budget
    /// Format: (region_id, complexity_class, value_set_size_cost)
    pub skipped: Vec<(usize, ComplexityClass, u64)>,
}
```

### 3.4 Composed Chain

```rust
/// Result of expression substitution across a linear transaction chain.
///
/// A chain A→B→C may produce multiple ComposedChains — one per trigger
/// value that gates the root transaction A. When A's write expression
/// references a trigger variable (e.g., `&x = sensor`), each concretized
/// trigger value produces a distinct fused body.
pub struct ComposedChain {
    /// Transaction names in order: A → B → C
    pub chain: Vec<String>,

    /// Variables that bridge between chain links (x: A→B, y: B→C)
    pub link_vars: Vec<String>,

    /// Triggers referenced by the root transaction's body.
    /// Each produces a separate composed variant.
    pub root_triggers: Vec<String>,

    /// The fully substituted merged body.
    /// Compact form: A's non-counter ops + B'(substituted) + C'(substituted).
    /// Intermediate state writes for chain-internal variables are eliminated —
    /// they become local temps that LLVM can promote to registers.
    pub composed_body: Vec<Statement>,

    /// Loop counter variable for convergence folding
    pub counter_var: Option<String>,

    /// Statement count of the composed body after substitution
    pub fused_weight: usize,
}
```

### 3.5 New Fields on RegionAnalyzer

```rust
pub struct RegionAnalyzer {
    // ... existing fields ...

    /// Transaction bodies indexed by name, for composition and complexity analysis
    txn_bodies: HashMap<String, Vec<Statement>>,

    /// Resolved iteration bounds: txn_name → bound_value
    /// Extracted from bounded_pre by resolving the bound variable
    /// through TopLevel::Constant and TopLevel::StateDecl initializers.
    iter_bounds: HashMap<String, u64>,

    /// Per-region optimization scores, sorted by score descending
    pub region_scores: Vec<RegionScore>,

    /// Greedy budget allocation plan (built on demand with budget parameter)
    pub budget_plan: Option<BudgetPlan>,

    /// Composed chains from linear dependency traversal + expression substitution
    pub composed_chains: Vec<ComposedChain>,
}
```

---

## 4. Phase-by-Phase Implementation Detail

### 4.1 Phase C: Complexity Estimation

**Function:** `classify_complexity(body: &[Statement]) → ComplexityClass`

Recursively counts statements including nested `Guarded` and `OnExit` blocks. Classification rules:

```
count_statements_recursive(body)
  └── walks all Statement variants
      ├── Let, Assignment, Expression: counts as 1
      ├── Guarded: counts as 1 + recursive(body)
      ├── OnExit: counts as 1 + recursive(body)
      ├── Term: marks Unbounded (premature exit)
      ├── Unification: marks Unbounded (pattern match)
      └── InlineAsm, Alka, LocalTrigger, Escape: marks Unbounded

has_ffi_or_trigger_refs(body) → bool
  └── checks if any statement reads trigger vars or calls frgn functions

classify: match (weight, has_ffi)
  0..=2 stmts, no ffi  → Trivial   (counter bump only)
  3..=5 stmts, no ffi  → Light     (IIR filter level)
  6..=20 stmts, no ffi → Medium    (moderate compute)
  21+ stmts, no ffi    → Heavy     (physics, matmul)
  any weight, has ffi  → Unbounded (can't fully analyze)
```

**GPU eligibility check** (same pass):

A region is GPU-eligible when all its transactions satisfy:
- Classification is `Medium` or `Heavy` (enough work to justify transfer overhead)
- No `Statement::Term`, `Statement::Unification`, `Statement::Escape`
- No `Expr::Call` referencing trigger variables
- All state writes are to bounded or local variables (not opaque/volatile)

This produces analysis-level annotation only — a future GPU backend queries `region_scores.iter().filter(|s| s.gpu_eligible)` to find emission candidates. Since LLVM targets GPU code natively (AMDGPU/NVPTX backends), the existing LLVM backend could toggle the target triple and add `addrspace` qualifiers rather than requiring a completely separate compilation path.

---

### 4.2 Phase D: Region Scoring

**Function:** `compute_region_scores(&mut self)`

Groups transactions by `region_id` (from Phase E's `compute_regions`). For each region:

1. **Group:** Collect all txn names whose written variables belong to this region
2. **Weight:** Sum `count_statements_recursive(body)` across all txns
3. **Complexity:** Take the worst (most restrictive) class — `max(Trivial, Light, Medium, Heavy, Unbounded)`
4. **Iteration count:** Resolve each txn's bounded_pre bound variable → compile-time integer, take the maximum (or 1 if unresolvable)
5. **Value set size:** For each txn in the region, find all triggers it reads. Multiply their `value_set_size_of(trg)` values. Multiply across txns. None if any trigger is unbounded.
6. **Score:** `(body_weight as f64 × iteration_count as f64) / max(cost, 1) as f64`
7. **GPU flag:** Check all txns for the GPU eligibility criteria
8. **Sort:** By `optimization_score` descending

**Edge cases:**
- Region with only Pure variables → region_id=0 → not scored (nothing to optimize)
- Single txn region → score computed normally
- Multi-txn region with all Unbounded → score = `—` (not allocatable)
- Region with `value_set_size = 0` (no triggers at all) → `cost = 1` (div-by-zero guard), score = `body_weight × iter`

---

### 4.3 Phase E: Budget Planning

**Function:** `build_budget_plan(&mut self, budget: u64)`

Greedy allocation algorithm:

```
remaining = budget
allocated = []
skipped = []

for each region in region_scores (sorted by score desc):
    if region.value_set_size is None:
        skipped.push(region)   // unbounded — can't allocate
        continue
    cost = region.value_set_size.unwrap()
    if cost <= remaining:
        allocated.push(region)
        remaining -= cost
    else:
        skipped.push(region)   // exceeds remaining budget

self.budget_plan = BudgetPlan {
    total_budget: budget,
    allocated,
    residual_budget: remaining,
    skipped,
}
```

**Desired properties:**
- **Monotonic:** Adding budget never reduces which regions are allocated
- **Deterministic:** Same program + same budget = same allocation every time
- **Greedy is optimal here** because regions are independent (no shared budget artifacts), costs are additive, and scores are comparable

**Why greedy works:** The value-set size of region A does not affect the value-set size of region B. Regions are connected components — by definition, they share no variables. Budget consumption is additive and non-interfering. Greedy by score descending is optimal for this knapsack variant.

---

### 4.4 Phase F: Chain Composition (Phase 4.2)

#### 4.4a — Expression Substitution Engine

**Function:** `substitute_var(body: &[Statement], old_var: &str, new_expr: &Expr) → Vec<Statement>`

Deep AST traversal. For every `Expr::Identifier(name)` where `name == old_var`, replace with `new_expr.clone()`. Handles all expression forms recursively:

```
substitute_var_impl(expr, old, new):
  match expr:
    Identifier(n) if n == old → new.clone()
    Add(a,b)   → Add(sub(a), sub(b))
    Sub(a,b)   → Sub(sub(a), sub(b))
    ... all binary ops similarly ...
    Not(a)     → Not(sub(a))
    Call(f,args) → Call(f, args.map(sub))
    Block(stmts,last) → Block(stmts.map(sub), sub(last))
    Tuple(elems) → Tuple(elems.map(sub))
    ListLiteral(elems) → ListLiteral(elems.map(sub))
    ... all composite forms ...
    _ → expr.clone()  // literals, unhandled
```

Important: `OwnedRef` matches plain `Identifier` for substitution purposes (both reference the same variable).

#### 4.4b — Chain Composition Algorithm

**Function:** `compose_chains(&mut self)`

For each detected linear chain `A → B → C`:

```
1. Find root trigger(s):
   root_triggers = self.trigger_vars ∩ self.txn_reads[A]
   If empty: root is not trigger-gated → produce one composed chain
   If non-empty: produce one composed chain per trigger value

2. For each trigger value combination:
   a. Build fused_body starting with A's body
   b. Remove counter-bump statements from A (they stay separate)
   c. For each link B, C, ...:
      i.   Find link_var: the variable A writes that B reads
      ii.  Find write_expr: the RHS of &link_var = expr in A's body
      iii. Substitute link_var → write_expr in B's body
      iv.  Append B's substituted statements to fused_body (skip counter bumps)
      v.   Find next_link_var: the variable B writes that C reads
      vi.  Repeat for each subsequent link

3. Store as ComposedChain:
   chain: [A, B, C]
   link_vars: [x, y]  (variables bridging links)
   root_triggers: [sensor] or []
   composed_body: fused statements
   counter_var: count (from first txn's bounded_pre)
   fused_weight: count_statements_recursive(&composed_body)
```

#### 4.4c — Chain Branching (the user's key insight)

A single linear chain `A → B → C` can produce **multiple composed chains** — one per trigger value combination that gates the root transaction.

**Example:**
```
trg sensor: Bool @0;
let x: Int = 0;
let y: Int = 0;

node step_a [count < total][count == total] {
    &x = sensor;              // x depends on trigger value!
    &count = count + 1;
};
node step_b [count < total][count == total] {
    &y = x + 1;               // depends on x from step_a
};
```

This produces **two** composed chains:

| Trigger value | Substituted body | Fused txn name |
|--------------|-----------------|----------------|
| `sensor == 0` | `&x = 0; &y = 0 + 1; &count = count + 1;` | `@txn_fused_trg_0` |
| `sensor == 1` | `&x = 1; &y = 1 + 1; &count = count + 1;` | `@txn_fused_trg_1` |

The enum dispatch then switches on `sensor` to call the correct fused variant. This multiplies the enum cost (2 variants × chain length) but also **multiplies the optimization win** (N sequential calls eliminated per trigger value).

**If no trigger gates the root:** The write expression is a constant (e.g., `&x = 42`) → produce one composed chain. No branching. Enum cost stays at 1.

#### 4.4d — Composition Constraints

A chain is **composable** when all conditions are met:

| Constraint | Meaning | Why |
|-----------|---------|-----|
| **Single upstream producer** | Each link's input variable has exactly one writer within the chain | Expression substitution is deterministic |
| **Counter in root only** | The loop counter increment is in the first txn (or identifiable single txn) | Fused body shares one counter bump |
| **No external state leaks** | Chain-internal variables (x, y) are not read by transactions outside the chain | Can eliminate intermediate writes safely |
| **Same convergence contract** | All txns share the same `bounded_pre` var and bound | Folding uses one while-loop condition |
| **No FFI in chain body** | No link calls foreign functions | Can't substitute through opaque calls |

**Partial composability:** If a chain-internal variable IS read externally, the variable's store is preserved in the fused body — we still eliminate reads/stores between chain links, just not the final store. The optimization is partial but still valid.

#### 4.4e — Counter Bump Detection

**Helper:** `is_counter_bump_stmt(stmt: &Statement, counter_var: &str) → bool`

Recognizes the pattern `&count = count + N` where N is a positive integer constant:

```
matches!(stmt, Statement::Assignment {
    lhs: Expr::OwnedRef(lhs_name) | Expr::Identifier(lhs_name),
    expr: Expr::Add(box Expr::Identifier(add_name), box Expr::Integer(delta))
}) if lhs_name == counter_var && add_name == counter_var && delta > 0
```

---

### 4.5 Phase G: Score Update After Composition

**Function:** (inline, called at end of `compose_chains()`)

After chain composition, update `region_scores`:

```
for each composed_chain:
    for each region_score:
        if region_score.txn_names overlaps composed_chain.chain:
            region_score.chain_composed = true
            region_score.body_weight = composed_chain.fused_weight  // actual fused count
            region_score.optimization_score *= 1.5  // bonus for chain composition
            // The 1.5× bonus reflects that chain composition eliminates
            // N−1 dispatch overheads (function call, stack frame, precondition check)
```

The score bonus is conservative (1.5×) because the actual reduction in dispatch overhead depends on the target architecture and the number of eliminated txn calls. A more precise bonus could be `1.0 + (N−1) × 0.3` where N is the chain length, but 1.5× is a safe default.

---

### 4.6 Phase H: Fused Emission (Phase 4.3)

#### 4.6a — Fused Transaction Function

**Function:** `emit_fused_txn(&mut self, out: &mut String, name: &str, composed_body: &[Statement])`

```llvm
define void @txn_fused_trg_0(%State* %state) #0 {
  entry:
    ; inline all substituted statements from the composed body
    ; chain-internal variables (x, y) become SSA values — no stores
    ; only final output writes + counter bump remain as stores to %State
    ; LLVM will promote these to registers via mem2reg
    ret void
}
```

For each composed chain with trigger variants:
- `@txn_fused_trg_0(...)`  — body concretized for trigger=0
- `@txn_fused_trg_1(...)`  — body concretized for trigger=1

Emission uses the existing `emit_statement` infrastructure — the fused body is valid Briv statements that the LLVM backend already handles. No new IR emitters needed.

#### 4.6b — Enum Dispatch with Fused Targets

**Function:** `emit_enum_main()` (extended)

When composed chains are available, the enum dispatch calls `@txn_fused_trg_X` instead of `@txn_name`:

```llvm
define i32 @main() #0 {
  entry:
    call void @init_state()
    %trg = load volatile i8, i8* @sensor
    switch i8 %trg, label %residual [
      i8 0, label %case_0_hdr
      i8 1, label %case_1_hdr
    ]
  case_0_hdr:
    %gt0 = getelementptr inbounds %State, %State* @global_state, i32 0, i32 0
    %lt0 = load i64, i64* %gt0, align 8
    br label %case_0_loop
  case_0_loop:
    %gp0 = getelementptr inbounds %State, %State* @global_state, i32 0, i32 5
    %lp0 = load i64, i64* %gp0, align 8
    %cp0 = icmp slt i64 %lp0, %lt0
    br i1 %cp0, label %case_0_body, label %case_0_done
  case_0_body:
    call void @txn_fused_trg_0(%State* @global_state)   ; ← fused, not @step_a
    br label %case_0_loop
  case_0_done:
    ret i32 0

  ; ... similar for case_1 with @txn_fused_trg_1 ...

  residual:
    ; standard reactor_tick() loop for uncovered values
    br label %residual_loop
  residual_loop:
    call void @reactor_tick()
    br label %residual_loop
}
```

#### 4.6c — Pure Counter Elimination

If the composed body after all substitutions contains **only the counter bump** (all other state writes were chain-internal and eliminated), the fused txn is pure. In this case, use `emit_folded_pure_counter`:

```llvm
define i32 @main() #0 {
  entry:
    call void @init_state()
    ; All compute was evaluated at compile time.
    ; Store the final counter value directly.
    %gp = getelementptr inbounds %State, %State* @global_state, i32 0, i32 5
    store i64 50000000, i64* %gp, align 8
    ret i32 0
}
```

This is true O(1) — zero runtime iterations.

---

## 5. Report Format

### 5.1 Full Report Example

Invoked via `briv-compiler llvm file.bv --optimize-report [--optimize-budget 256] [--optimize-size 10240]`:

```
=== Optimization Report ===
Optimize budget: 256

Transaction graph:
  Nodes: 5  |  Has triggers: true  |  Max bounded pre: count < 50000

Linear transaction chains:
  Chain 1: step_a → step_b → step_c  (root trigger: sensor, 2 variants)
  Chain 2: init → transform           (no trigger, 1 variant)

Optimization priority ranking:
  Rank  Region ID    Transactions         Class    Weight  Iter     EnCost  Score        Chain   GPU  Status
  ────────────────────────────────────────────────────────────────────────────────────────────────────────────────
  1     R3           step_a,step_b,step_c  Medium   18      50000    2       450000.0     ✓       ✓    FIT ✅  → switch-dispatch
  2     R1           process               Light    8       100      1       800.0               ✓    FIT ✅  → folded main
  3     R2           physics               Heavy    42      1000     ∞       —                         UNBOUNDED
  4     R4           init,transform        Heavy    15      1        2       7.5          ✓             FIT ✅  → composed+enum
  5     R5           io_handler            Light    3       ∞        ∞       —                         UNBOUNDED
  ────────────────────────────────────────────────────────────────────────────────────────────────────────────────

Region details:

  R3 (step_a, step_b, step_c) — ★ Top priority
    Complexity:   Medium (18 stmts fused)
    Iterations:   50,000 (bounded pre: count < total, total=50000)
    Enum cost:    2 (trigger=sensor, Bool → [0,1])
    Score:        450000.0
    Optimization: Chain-composed ✓ — N→1 txn call elimination
    GPU eligible: ✓ (pure body, bounded writes, no volatile triggers)
    Allocation:   Enumerated within budget → per-value switch dispatch

  R2 (physics) — ⚠ Cannot fully enumerate
    Complexity:   Heavy (42 stmts)
    Iterations:   1,000 (bounded pre: frame < max_frames, max_frames=1000)
    Enum cost:    ∞ (Int trigger → unbounded value set)
    Optimization: Partial — segment folding still applies, loop unroll factor 8
    GPU eligible: ✓ (pure body, bounded writes, no FFI — compute shader candidate)
    Allocation:   Skipped (unbounded cost)

Budget plan (budget=256):
  Allocated:  R3 (cost: 2) + R1 (cost: 1) + R4 (cost: 2) → spent 5/256
  Residual:   251 budget units available
  Skipped:    R2, R5 (unbounded — partial optimization still applied)
  Sweet spot: Full enumeration of bounded regions uses <2% of budget

Size estimation (--optimize-size 10240):
  Base binary (standard reactor):         ~5 KB
  With R3 + R4 enumeration (3 combos):    ~5.3 KB
  Within 10 KB limit: ✅  (5.3 KB < 10 KB)
  Recommended budget: 3 (all bounded regions enumerated)
```

### 5.2 Report Sections

1. **Header** — Budget value, command flags
2. **Transaction graph** — Node count, trigger presence, bounded pre info
3. **Linear chains** — Detected chains, trigger gating, variant count
4. **Priority ranking table** — All scores, sorted by optimization impact
5. **Region details** — Per-region analysis with recommendations
6. **Budget plan** — Allocation table, residual budget, skipped regions
7. **Size estimation** — (when `--optimize-size` is set) binary size estimates

---

## 6. GPU Offload Analysis

### 6.1 Eligibility Criteria

A region is `gpu_eligible` when **all** conditions hold:

1. **Complexity class is Medium or Heavy** — enough compute to justify PCIe/NUMA transfer overhead. Trivial and Light regions are faster on CPU.

2. **Pure body** — no `Statement::Term`, `Statement::Escape`, `Statement::Unification`, `Statement::InlineAsm`, `Statement::Alka`. These represent control flow or side effects that don't map cleanly to GPU kernels.

3. **No volatile triggers** — the body does not read any `trigger_vars`. GPU kernels cannot perform volatile I/O loads.

4. **No foreign function calls** — `Expr::Call` nodes are not present in the body. FFI functions are CPU-only.

5. **Bounded writes only** — all `Statement::Assignment` targets are either classified as Pure or Bounded by the region analyzer. Opaque writes mean the kernel would need CPU intervention.

### 6.2 Emission Strategy (Future)

The existing LLVM backend already supports AMDGPU and NVPTX targets via LLVM's native backends. To emit GPU code:

1. **Detect GPU-eligible regions** from `region_scores.iter().filter(|s| s.gpu_eligible)`
2. **Emit kernel wrapper** — `define amdgpu_kernel void @region_3_kernel(...)` with `addrspace(1)` for global memory
3. **Emit buffer transfer** — host-side `@main` allocates GPU buffers, calls kernel, copies results back
4. **Wire into existing fold path** — the `while(count < N)` loop becomes a `for i in 0..N` GPU grid dispatch

This is a separate implementation phase (not part of the current cost-model work) but the analysis infrastructure is designed to support it.

---

## 7. Verification Strategy

### 7.1 Unit Tests

| Test | What it validates | Expected |
|------|------------------|----------|
| `test_complexity_trivial` | Counter-only body | Returns `Trivial` |
| `test_complexity_light` | IIR filter body (5 stmts) | Returns `Light` |
| `test_complexity_unbounded` | Body with `term` | Returns `Unbounded` |
| `test_region_scoring` | Two txns in same region | Higher-weight txn dominates score |
| `test_region_independent` | Two txns in different regions | Two separate RegionScores |
| `test_budget_plan_fit` | Budget=10, two regions with cost 2+3 | Both allocated, residual=5 |
| `test_budget_plan_exceeds` | Budget=3, region cost=5 | Skipped |
| `test_chain_substitution` | A writes x=42, B reads x | Substituted body has 42 not x |
| `test_chain_composition` | A→B→C with constants | Fused body has all vars resolved |
| `test_chain_branching` | Root txn gated by Bool trigger | Two composed chains produced |
| `test_gpu_eligible` | Pure Medium body, no triggers | gpu_eligible = true |
| `test_gpu_ineligible_term` | Body with `term` | gpu_eligible = false |

### 7.2 Integration Tests

| Test | What it validates |
|------|------------------|
| `test_report_shows_ranking` | `--optimize-report` produces ranking table |
| `test_report_shows_budget` | `--optimize-report --optimize-budget 100` shows allocation |
| `test_report_shows_chains` | Multi-txn program shows chain detection |
| `test_report_shows_size` | `--optimize-size 10000` shows size estimation |
| `test_enum_with_composed_chain` | Bool-triggered chain program uses fused txn in enum dispatch |
| `iir_filter_benchmark_regression` | IIR 50M iterations unchanged at 0.15s |

### 7.3 Regression Guarantee

- All 316 existing tests must pass unchanged
- IIR benchmark must maintain 0.15s (no structural changes to folding path)
- No changes to public AST types
- No changes to existing `ProofEngine` or `TypeChecker` behavior

---

## 8. File Impact Summary

| File | Lines Added | Description |
|------|------------|-------------|
| `src/analysis/region.rs` | ~250 | Types (ComplexityClass, RegionScore, BudgetPlan, ComposedChain), complexity estimator, region scoring, budget planning, expression substitution engine, chain composition, pipeline integration |
| `src/backend/llvm.rs` | ~80 | `emit_fused_txn()`, `emit_enum_main()` extended for chains, report section with ranking table and budget allocation display |
| **Total** | **~330** | No new files, no breaking changes to existing APIs |

---

## 9. Relationship to Existing Work

| Prior Phase | How this extends it |
|-------------|-------------------|
| Phase 0 (convergence proof) | Iteration bounds now resolved at compile time for scoring |
| Phase 1 (region analysis) | Regions now carry optimization scores and GPU eligibility |
| Phase 2 (value-set enumeration) | Switch dispatch now takes fused chain functions as targets |
| Phase 3 (budget/report CLI) | Report now shows ranked priority table instead of flat listing |
| Phase 4.1 (linear chains) | Chains are now composed into fused bodies via substitution engine |

The cost model is the **unifying layer** — it takes all prior analyses and produces a single, actionable priority ranking that the backend uses to decide what to enumerate, what to fold, and what to compose.

---

## 10. Design Decisions

| Decision | Rationale |
|----------|-----------|
| Score formula uses `body_weight × iteration_count` | Absolute runtime reduction matters more than per-iteration complexity. A 50M-iteration counter loop saved is a bigger win than a 1000-iteration physics integrator. |
| Greedy knapsack allocation | Regions are independent (disjoint variable sets) → additive costs → greedy is optimal. No need for DP/ILP. |
| 1.5× chain composition bonus | Conservative bonus for N→1 txn call elimination. Actual savings depend on target architecture (function call + stack frame + precondition check overhead). |
| GPU eligibility is analysis-only (no emission) | GPU codegen is a separate backend decision. The analysis provides the data; the backend decides how to use it. LLVM targets GPU natively, so the existing backend can toggle target triples. |
| Chain branching per trigger value | A single chain can produce multiple fused bodies when the root txn's write references a trigger. This is the user's key insight — not a bug, a feature. Enum cost multiplies, but so does the optimization win. |
| Complexity class is monotonic within a region | If any txn in a region is Heavy, the whole region is Heavy. Optimistic classification (Light region with one Heavy txn) would produce wrong budget decisions. |
