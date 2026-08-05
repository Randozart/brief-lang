# Transaction and Node Semantics

Briv has three executable constructs. They share a common contract-driven
foundation but differ in how they are called and what state they can touch.

## 0. The Three Constructs

| Construct | Role | Calling convention | Parameters | Return value | State mutation |
|-----------|------|--------------------|------------|--------------|----------------|
| `node` | Reactive state machine | Auto-fired when precondition true | No | No | Yes — full access |
| `txn` | Callable atomic block | Explicit — `NAME(args)` | Yes | Yes | Yes — full access |
| `defn` | Pure function | Explicit — `NAME(args)` | Yes | Yes | **No** — pure |

### `node` — reactive state machine

A `node` is a self-contained unit in the program's reactive fabric. It has
no calling site, no parameters, and no return value — it reads and writes
state fields directly. The runtime engine evaluates all nodes' preconditions
and fires every node whose precondition is true. The program converges
(naturally exits) when all node preconditions are false — no more work to
do.

```
node tick [count < total][count == total] {
    count = count + 1;
};
```

Nodes that operate on disjoint state fields are eligible to fire
**concurrently**. The compiler checks read/write sets at compile time and
rejects programs where concurrent nodes would conflict (see §5 Parallelism).

### `txn` — callable atomic block

A `txn` is identical in semantics to a `node` but is **called explicitly**
from another function. It can have typed parameters and a return value.

```
txn sum_until(n: Int, total: Int) [i < n][i == n] -> Int {
    i = i + 1;
    total = total + i;
    term total;
};
```

The body loops atomically (convergence loop) just like a `node`. The
difference is only in how execution starts: `node` waits for its
precondition, `txn` waits for a caller.

### `defn` — pure function

A `defn` is **pure** — it cannot read or write state outside its body.
It is always safe to fold, inline, or reorder. No convergence loop, no
atomic commits.

```
defn add(a: Int, b: Int) -> Int { a + b };
```

---

## 1. The Contract

Every `node` and `txn` carries a precondition `[pre]` and postcondition
`[post]`:

```
node tick [count < total][count == total] { count = count + 1; };
txn work(args) [pre][post] -> Ret { body; term val; };
```

The contract states: *while the precondition holds, the body executes to
make the postcondition true.* The transaction converges when the
precondition is false — no more useful work remains.

## 2. Atomicity

A tick of a `node` or `txn` is **atomic**. Every statement in the body
reads from the state at the start of the tick:

```
node tick [count < total][count == total] {
    x0 = A00*x0 + A01*x1 + A02*x2;   // reads pre-tick x0, x1, x2
    x1 = A10*x0 + A11*x1 + A12*x2;   // reads pre-tick x0, x1, x2
    x2 = A20*x0 + A21*x1 + A22*x2;   // reads pre-tick x0, x1, x2
    count = count + 1;
};
```

All three reads see the **same pre-tick values**, not sequentially
updated values. This is the classical **reactive / synchronous** model:

- All RHS evaluate from pre-tick state
- All LHS are committed together at the tick boundary

### Why atomicity?

Atomicity is the foundation of all higher-level optimization:

| Optimization | How atomicity enables it |
|---|---|
| **Folding** | Deterministic chain `node A→node B` always produces same result → precompute at compile time |
| **ILP** | Independent assignments have no read-after-write chain (they all read pre-tick) → emit parallel |
| **Reactive parallelism** | Multiple `node` blocks fire concurrently on disjoint state → no race condition because each tick sees a consistent snapshot |
| **Observable hoisting** | `frgn`/intrinsic calls extracted from body, remainder folded to a constant |

Without atomicity, none of these transformations are sound.

### Relation to sequential execution

"Atomic" does **not** mean the body is unordered. Statements execute in
program order, and `let` captures values at evaluation time:

```
node [n < N][n == N] {
    let a = x + 1;     // captures pre-tick x
    x = a * 2;         // writes tick's new x
};
```

The key rule: **all state reads in the RHS of assignments go to pre-tick
state.** Local `let` bindings capture whatever value the expression
produces at the point of evaluation.

This makes the `let`-temporaries pattern work for expressing "read all
old, write all new":

```
node [count < total][count == total] {
    let nx0 = A00*x0 + A01*x1 + A02*x2;   // all read pre-tick
    let nx1 = A10*x0 + A11*x1 + A12*x2;
    let nx2 = A20*x0 + A21*x1 + A22*x2;
    x0 = nx0; x1 = nx1; x2 = nx2;
    count = count + 1;
};
```

The three `let` bindings are independent (no read-after-write chain among
them), so the compiler emits them for parallel execution. The stores to
state all happen at the tick boundary.

## 3. Convergence

The body of a `node` or `txn` loops — each tick evaluates `[pre]`, runs
the body, checks `[post]`, and repeats until the precondition is false:

```
[pre]  { body }  [post]  →  [pre]?
                              ├─ yes → body → post → ...
                              └─ no  → exit
```

This is **natural convergence**. The program exits because no active
construct has a true precondition. There is no `while(true)`, no `break`
— the contract drives termination.

### `term` inside the body

`term` is a swan-song marker: it indicates the point after which the
postcondition must be satisfied. For a `txn` with a return value:

```
txn sum(n: Int) [i < n][i == n] -> Int {
    total = total + i;
    i = i + 1;
    term total;   // postcondition check + return value
};
```

## 4. Reactive vs Callable

`node` and `txn` are semantically identical (same atomicity, same
convergence loop). The only difference is calling convention:

| Form | How it fires |
|------|-------------|
| `node NAME [pre][post]` | **Reactive** — auto-fired by runtime when precondition is true |
| `txn NAME(args) [pre][post] -> Ret` | **Callable** — explicitly invoked from another function |

A `node` has no caller. The runtime engine acts as a predicate-evaluation
loop: it scans all nodes, fires those whose precondition is true, and
converges when all preconditions are false. The order of firing among
eligible nodes is undefined (they run concurrently when their state sets
are disjoint).

## 5. Parallelism

### Concurrent nodes

Two or more nodes that operate on **disjoint state fields** fire
concurrently. The compiler checks at compile time whether their read/write
sets intersect.

```
let a: Int = 0;
let b: Int = 0;

node inc_a [a < N][a == N] { a = a + 1; };
node inc_b [b < N][b == N] { b = b + 1; };
// a and b are disjoint → inc_a and inc_b run in parallel
```

### Conflict detection

If two concurrent nodes would read and write (or write and write) the
same field, the compiler **rejects the program** at compile time. There is
no runtime conflict resolution.

```
node tick_a [a < N][a == N] { a = a + 1; };
node tick_b [a < N][a == N] { a = a + 2; };
// REJECTED: both write `a`
```

### `sync(group)`

`sync(group)` is the escape hatch. Nodes in the same sync group are
serialized — they execute in a defined order rather than concurrently.

```
sync(io) {
    node tick_a [a < N][a == N] { ... };
    node tick_b [a < N][a == N] { ... };
};
// tick_a and tick_b are serialized
```

## 6. Compiler Optimization

The atomicity contract enables aggressive optimization that no
sequential-by-default language can safely perform.

### Folding

If the compiler can prove a node/txn body has no observable side effects
(`frgn` calls or `observable <~ true` intrinsics) and all inputs are
compile-time constants, the entire body is precomputed:

```
const N: Int = 50000000;
node [i < N][i == N] { i = i + 1; };
// → folded to `i = 50000000` at compile time
```

To prevent folding (when the benchmark is meaningful only at runtime):

```
let N: Int = GetEnvInt!("BOUND");   // runtime-determined
node [i < N][i == N] { i = i + 1; };
```

### Observable hoisting

If a node/txn body mixes pure computation with an observable call, the
compiler attempts to hoist the observable out of the convergence loop,
leaving only the foldable computation:

```
node [i < N][i == N] {
    i = i + 1;           // pure — foldable
    PrintLn!(i);         // observable — must execute each tick
};
```

When the observable depends on state the pure computation updates, the
loop must remain. But when the observable can be hoisted (or the pure
fragment extracted), the compiler does so.

### ILP via dependency analysis

Because all RHS read from pre-tick state, the compiler can reorder
independent assignments for parallel execution:

```
node [count < N][count == N] {
    x0 = A0 * x0 + B0 * x1;    // independent — no read-after-write
    x1 = A1 * x0 + B1 * x1;    // reads PRE-TICK x0, not updated x0
    x2 = A2 * x2 + C2;         // independent
    count = count + 1;
};
```

The compiler emits: load all pre-tick → compute all in parallel → store
all. LLVM's scheduler then maps the independent chains to separate
execution units.

## 7. Summary Table

| Concept | `node` | `txn` | `defn` |
|---------|--------|-------|--------|
| Calling convention | Auto (reactive) | Explicit | Explicit |
| Parameters | No | Yes | Yes |
| Return value | No | Yes | Yes |
| State mutation | Yes | Yes | **No** |
| Atomic ticks | Yes | Yes | N/A (no loop) |
| Convergence loop | Yes | Yes | N/A (no loop) |
| `[pre][post]` contract | Yes | Yes | No |
| `term` marker | Yes (no value) | Yes (with value) | No |
| Folding | When pure + const | When pure + const | Always |
| Concurrent firing | Yes (disjoint state) | No (sequential calls) | N/A |
| Conflict detection | Compile-time | N/A | N/A |
| `sync(group)` serializable | Yes | N/A | N/A |
