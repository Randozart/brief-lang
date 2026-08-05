# Determinism and the Optimization Frontier

## Two Cases

### Case 1: Deterministic Closure

When a system's entire state evolution graph is deterministic — every mutation follows from a known prior state with no external input — the compiler can, at compile time, enumerate or prove the exact set of values every variable can hold over any number of ticks. One mutation implies another, implies another, in a closed chain.

This is what the IIR filter benchmark exercises. The reactive transaction `[count < total][count == total]` with `&count = count + 1` is entirely deterministic: `count` determines the guard, the guard determines whether we fire, firing determines the next `count`. The compiler folds the reactive loop into a single `while` canonical form — zero dispatch overhead, zero contract evaluation at runtime.

Any transaction whose body and contracts reference only variables that are themselves deterministically computed (no dependency on opaque frontier values) is a candidate for full folding.

### Case 2: Bounded Non-Determinism (the Predictability Frontier)

When the system interacts with the outside world — a user inserting text, a sensor reading, a file being processed, a random number generator — exact values are unknowable at compile time. But the *bounds* are known: the type system constrains the shape (`String`, `Int`, `Float`), and contracts can express additional constraints (`len(s) <= 140`, `0 <= temp <= 100`).

The compiler's task is to classify every expression along a predictability axis, not by which keyword introduced it.

---

## Predictability is the Axis

The `trg` keyword marks a value that originates outside the deterministic closure. The `frgn` keyword marks a call into foreign code. But **neither keyword is synonymous with "unpredictable."** The compiler classifies by what it can prove, not by what keyword was used.

| Origin | Example | Classified As | Why |
|--------|---------|---------------|-----|
| `trg clock: Bool` | hardware clock toggling at known frequency | **Predictable** | Contract proves tick window; value set `{true, false}` |
| `trg button: Bool` | user-pressed button | **Bounded** | Value set `{true, false}`, but timing is opaque |
| `trg name: String` | text input | **Bounded** | Type + contract constrain length, but alphabet is large |
| `frng "sqrt" x` | pure math function | **Predictable** | Deterministic: same input → same output |
| `frng "rand" seed` | RNG | **Opaque** | No contract constrains the output distribution |
| `frng "db" query` | database lookup | **Opaque** | Result depends on external state |

The compiler's predictability analysis starts conservatively (everything is Opaque) and narrows:

1. A variable is **Predictable** if its value set can be fully enumerated and its update timing can be bounded.
2. A variable is **Bounded** if its value set can be bounded (type + contract range) but not fully enumerated, or if timing is opaque.
3. A variable is **Opaque** if neither its value set nor its timing can be constrained.

Taintedness follows assignment: if `&x = trg_a + 1`, then `x` inherits `trg_a`'s classification.

---

## Boundedness Flows Through the Dependency Graph

Because Briv programs form an assignment dependency graph (a value implies other values through transactions), boundedness is *also* graph-structured. The bounds on a `trg` or FFI value are not isolated — they propagate through every expression and transaction that reads them.

Given:
```
trg temp: Float;          // 0.0 <= temp <= 100.0 (from contract)
let scaled: Float = 0.0;

node scale [on temp] {
    &scaled = temp * 2.5;
};
```

The compiler can prove `0.0 <= scaled <= 250.0` without any runtime checks — the bound flows from the `trg` contract through the body expression. `scaled` is not a frontier variable itself, but it is *tainted* — its exact value is opaque, yet its bounds are known.

This means **boundedness verification works at the frontier exactly like convergence proof works for deterministic loops**: we check the contracts on `trg`/FFI declarations, then the compiler propagates those ranges through the expression graph. Any downstream contract that is satisfied within the propagated range is provably safe. Any convergence claim that depends only on bounded-frontier values can still be verified structurally — just with ranges instead of exact values.

The practical consequence: `[count < total][count == total]` works identically whether `total` is a `const Int` or a `trg Int` with `[total >= 0]`. In the former case the bound is an exact literal; in the latter it's a proven range. The convergence proof, the loop folding, and the zero-overhead emission are the same either way — the only difference is that the `while` condition becomes `while (count < total)` with a `total` loaded from the frontier, not from a constant global.

### Edge Cases in Bound Propagation

- **Division**: `a / b` where `0 <= b <= 10` — `b` can be zero, so the bound interval may collapse to "any Int" (undefined behavior). The compiler rejects or conservatively widens.
- **Overflow**: `a + 1` where `a = MAX_INT` — the arithmetic is well-defined by Briv's semantics (saturating or wrapping, per contract), but bound propagation must account for it.

These don't break the model — they define where the compiler must conservatively degrade a classification (Bounded → Opaque) when arithmetic loses monotonicity.

---

## Timing Predictability at the Frontier

A `trg` declaration says "this value may change at any tick." But "any tick" is not always an unknowable interval. In embedded Briv, `trg` variables are wired to direct MMIO addresses:

```
// firmware.ebv — runs on a microcontroller
trg status: U8 @ 0x4000_1000;

node poll [status & 1] {
    // firmware handles the event
};
```

The corresponding hardware netlist (`peripheral.ebv`, which transpiles to gates) drives that MMIO register at a known clock frequency. From the firmware's perspective, `status` can change at any tick — but "tick" itself is bounded by the bus protocol's timing. The netlist guarantees `status` settles within N clock cycles after an event.

This means:

1. **The schedule frontier is narrower than the value frontier.** Even when exact values are opaque, the *earliest* and *latest* tick at which a `trg` can fire are often compile-time computable.

2. **Cross-boundary interaction is predictable by construction.** A `.ebv` file that maps to netlist and a `.ebv` file that maps to firmware use the same MMIO addresses — the address is the synchronization contract. The compiler knows both sides will meet at that address on a known cadence.

3. **Timing boundaries propagate like value boundaries.** If `trg A` is known to update every 100 ticks, and `trg B` depends on `trg A`, then `B`'s update window is also determinable — even though `B`'s value is opaque. Downstream schedule optimizations (pipeline depth, register lifetime, wake-up window) can be computed.

The practical significance: a reactive system with multiple `trg` variables can still have a fully deterministic **schedule** even when values are fully opaque. The compiler can answer "this chain of transactions will resolve within K ticks" without knowing what the resolved values are. This collapses the optimization gap between Cases 1 and 2 for scheduling purposes.

Only `trg` sources with known cadence (hardware bus, clock edge, MMIO cycle in .ebv) give a determinable tick window. A `trg` wired to a UART receive buffer or a network socket has no timing guarantee.

---

## Atomic Reactive Regions

A `trg` breaks equilibrium only for the operations that **depend on it**. Transactions that don't reference the `trg` (directly or transitively through assignments) remain in their current stable state — they have no reason to re-fire. This means the program is not a single reactive soup; it is a **partition of independent atomic regions**, each rooted at one or more `trg` declarations.

```
trg button: Bool;
trg sensor: Float;

let lights: Bool = false;
let log: Float = 0.0;

// Region A — depends only on `button`
node toggle [on button] [lights != button] {
    &lights = button;
};

// Region B — depends only on `sensor`
node record [on sensor] [log != sensor] {
    &log = sensor;
};
```

When `button` fires, only **Region A** re-evaluates. `sensor` is undisturbed. `log` is undisturbed. The compiler can analyze, fold, and schedule each region independently:

1. **Dependency isolation**: trace each `trg` forward through the assignment graph. The transitive closure of "reads `trg` → assigns `x` → reads `x` → assigns `y` → ..." defines a region boundary.

2. **No cross-region re-evaluation**: a `trg` change in Region A cannot cause a transaction in Region B to fire, because no variable in Region B's dependency chain changes. Equilibrium is local.

3. **Independent optimization**: each region can be compiled to its own schedule (folded loop, parallel formula, hardware pipeline, etc.) without coordinating with other regions. The only cross-region data flow is through the type/bound system — and those bounds are verified at compile time.

4. **Parallel execution by construction**: regions with non-overlapping dependency graphs have zero data hazards. They can execute on separate cores, separate hardware units, or separate devices without synchronization — the frontier *is* the synchronization boundary.

5. **Region merging on shared consumers**: Two `trg`s remain independent UNLESS a transaction reads both. If `txn A` depends on both `trg_1` and `trg_2`, those two regions merge into one. The partition is connected-components over the dependency graph.

This is a fundamentally different model from a global reactive framework (like FRP with a global push/pull phase). Briv's `trg` is a **local equilibrium break** — the rest of the system doesn't even notice.

---

## Residual-Aware Partial Folding

Not all-or-nothing. The compiler folds each maximal deterministic segment, and frontier nodes (unpredictable `frgn` calls, opaque `trg` reads) become the runtime serialization points between segments.

```
trg A ──→ txn 1 ──→ txn 2 ──→ frgn DB ──→ txn 3 ──→ trg B ──→ txn 4
         ╰──── foldable ────╯    ↑        ╰─ foldable ─╯   ↑    ╰─ foldable ─╯
                              residual                  residual
```

Three independent foldable segments, each compiled to a formula. At runtime: `formula_1(A)` → DB call → `formula_2(result, A_out)` → reads `B` → `formula_3(B, ...)`. The residual nodes are the **only** serialization points — everything between them runs at formula speed.

The compiler never poisons a whole chain because one node is unfoldable. 70% of a chain can be formula-speed even if 30% requires reactive dispatch. The folding is **residual-aware** — you fold each maximal deterministic segment, and the frontier becomes the runtime boundary.

The region partition is a **DAG cut**: find every node that is an opaque or unpredictable frontier read, cut the graph there, and fold each connected component independently. The cuts are the runtime serialization boundary.

---

## Value-Set Enumeration

For Predictable and Bounded variables, the compiler can sometimes do more than segment-fold. It can **enumerate** the value set, clone the region once per possible value, fold each clone independently, and emit a switch:

- `Bool` → 2 clones behind `switch(trg) { case true: ...; case false: ... }`
- `U8` with `[0 <= val && val <= 3]` → 4 clones behind a 4-way switch
- `String` with `[len(name) <= 5]` → alphabet-dependent, but bounded

After cloning, each clone is fully deterministic — the frontier value is concretized. The folding proof applies identically to each clone. The runtime cost is a single indirect branch (or lookup) instead of reactive dispatch.

The compiler's decision to enumerate is based on the **product of the value set sizes** across all frontier reads in a region. If the product is small enough (within the user's budget), enumeration replaces segmentation. If it's large, the compiler falls back to segment-fold.

This reframes the axis:

| Classification | Value Set Size | Optimization Strategy |
|---------------|---------------|----------------------|
| Predictable   | Fully enumerable, small | Enumerate + fold each clone |
| Bounded       | Bounded but large or unbounded in time | Segment-fold (cuts at frontier) |
| Opaque        | No bound | Segment-fold (cuts at frontier) |

The same proof engine that checks convergence (and the same boundedness propagation) computes the value set size for every frontier node. The compiler then decides to enumerate or segment based on the budget.

For `trg name: String` with `[len(name) <= 5]` and a budget covering 26 letters × 5 positions ≈ 11M paths, the compiler emits a lookup table: 11M folded formulas, each computing the exact chain outcome for every possible 5-letter name. At runtime: `result = table[name]` — zero evaluation, zero dispatch. The `.ll` file is enormous but the runtime path is a single indirect load.

---

## Configurable Compile-Time Budget

The developer controls how much enumeration the compiler performs:

| Flag | Behavior |
|------|----------|
| `--optimize-budget <N>` | Max combinations to enumerate. 0 = segment-fold only. |
| `--optimize-report` | Print tradeoff table (budget × binary size × latency × coverage) and exit without compiling. |
| `--optimize-size <bytes>` | Binary-search for the budget that fits under a binary size constraint. |

The `--optimize-report` flag gives the developer a table:

```
  Optimize-budget tradeoffs for 'process':
    budget  │ paths │ binary ▲ │ latency ▼ │ coverage
    ────────┼───────┼──────────┼───────────┼─────────
        0   │  seg  │  4 KB    │ reactive  │ —       (segment-fold only)
      100   │  102  │  8 KB    │ switch    │  0.2%
     1000   │ 1024  │ 44 KB    │ switch    │  2.1%
    10000   │ 9710  │ 380 KB   │ switch    │ 19.4%
   100000   │ 89K   │  3.4 MB  │ switch    │ 74.2%
  1000000   │ 410K  │ 16.1 MB  │ switch    │ 97.0%
      all   │ 11.9M │ 468 MB   │ switch    │ 100%

  Sweet-spot candidates:
    budget 100000  — 3.4MB binary, switch dispatch, 74% of inputs covered
    budget 1000000 — 16MB binary, switch dispatch, 97% of inputs covered
```

The developer picks a budget or size, and the correctness proof is identical at every level — only the runtime speed and binary size change.

The upper limit is the **product of the value set sizes** across all frontier reads in a region. If the product is enormous, the report makes it visible. The developer can then decide whether the compile time / binary size is worth the latency improvement.

---

## Implications for the Compiler

The convergence proof (`check_convergence`) is a special case of a broader determinism and boundedness analysis. The proof engine should eventually:

- Classify every variable along the Predictable / Bounded / Opaque axis.
- Propagate type-level and contract-level bounds through the expression graph.
- Compute connected-components over the dependency graph (atomic regions).
- For Pure subgraphs: admit full folding as today.
- For Predictable Bounded subgraphs: admit value-set enumeration (clone-and-fold behind switch).
- For Opaque subgraphs: admit segment-folding around the frontier cuts.
- Compute value-set sizes and produce tradeoff reports.
- For multi-transaction chains with deterministic net effect: collapse to parallel schedule.

The `trg` and `frgn` keywords are not "escape hatches from the type system." They are origin markers. The compiler's predictability analysis — not the keyword — determines the optimization strategy.
