# `cell` — Cybernetic Cell Primitive

**Date**: 2026-06-23  
**Phase**: Phase 1 (sync)  
**Status**: specification

---

## Anatomy of a Cell

```brief
cell! timer(duration: Int) -> elapsed: Int, done: Bool {
    elapsed: Int = 0;
    done: Bool = false;

    rct txn tick [elapsed < duration] {
        &elapsed = elapsed + 1;
    };

    rct txn finish [elapsed >= duration && !done] {
        &done = true;
        term!;
    };
};
```

A `cell` is not a class. A class is a passive data structure with imperative accessors. A cell is an **intentionally isolated Brief-in-Brief state space** — a sealed state machine with a defined interface, its own reactor loop, and no external coupling.

---

## Why "Cell" Instead of "Component"

**"Component" is a mechanical metaphor.** A gear in a machine. It implies a passive part turned by other parts. It invites thinking about static assemblies and tight imperative wiring.

**"Cell" is a biological metaphor.** A cell has a membrane (the boundary), its own metabolism (the reactor loop), and cannot be directly forced or polled. It communicates with its environment through structural coupling — controlled exchanges across a sealed boundary.

The keyword tells the programmer: *you are designing an autonomous, self-regulating unit. Respect its boundary.*

---

## The Membrane: Operational Closure at Compile Time

A `cell`'s `rct txn` **cannot** see the parent's `%State`. They react only to:
- The cell's own private state fields
- The cell's own input arguments
- The cell's own `trg` variables (internal triggers)

This is enforced by the compiler, not by convention. In hardware, physical epoxy enforces the boundary. In Brief, the compiler **is the epoxy casing** — it rejects any attempt to read cell internals from outside, or reference parent state from inside.

---

## Cognitive Transparency vs. Operational Closure

| Aspect | For the Developer | For the Program |
|--------|------------------|-----------------|
| Source | Open — edit any cell's `.bv` file | Not visible |
| State | Inspectable via the source | Sealed — no direct reads |
| Behavior | Understandable via the contracts | Observable only through output ports |

The cell is a **white box to the human**, but a **black box to the compiling program**. This is the ideal balance: intellectual openness for debugging and reasoning, computational closure for robustness.

When you open `std/system_cell.bv` to modify the `Console!` state machine, you step *outside* the system. You become the meta-designer, rewriting the cell's "laws of nature." Once you save and recompile, you step back *inside*, and the compiler enforces those new laws strictly.

---

## Cells Are Not Objects

Alan Kay's original vision of Object-Oriented Programming was biological — cells communicating by passing messages, sharing no memory. Mainstream OOP (C++, Java, C#) abandoned this for hierarchical class inheritance and shared mutable state. A `cell` reconstructs Kay's original vision:

| Property | Mainstream Class | Brief `cell` |
|----------|-----------------|--------------|
| State | Public/private fields, mutable from outside | Private, sealed — no direct reads |
| Mutation | Setter methods (imperative, synchronous) | Input arguments (perturbations) |
| Observation | Getter methods (synchronous polling) | Trigger binding (event-driven) |
| Lifecycle | Passive — dead until called | Autonomous — owns its reactor loop |
| Composition | Inheritance (fragile base class) | Composition via structural coupling |

A cell is not a mechanism for organizing code. It is a mechanism for organizing state spaces.

---

## Cell and Cell! — Two Lifecycle Modes

| Property | `cell` | `cell!` |
|----------|--------|---------|
| Lifecycle | Auto-terminating | Persistent |
| Call semantics | Sync only (blocks) | Async only (runs in background) |
| Convergence | Stasis or `term!` causes return | Stays alive until `term!` or parent exit |
| Trigger binding | Not supported (returns then dies) | Supported (`trg @ Cell!`) |
| `term` inside body | Normal tick (continue) | Normal tick (continue) |
| `term!` inside body | Early return to caller | Early exit — component terminates |

**`cell`** is a goal-seeking regulator: converge to a stable output, return it, deallocate. Use it for computations that produce a result.

**`cell!`** is an allostatic agent: maintain internal homeostasis while processing signals. Use it for console I/O, protocol handlers, hardware drivers, sensor fusion.

The `!` signals "altered control flow — pay attention," consistent with Brief's `!` semantics (e.g. `term!` for program exit).

---

## Inputs as Perturbations

You cannot imperatively force a state change on a cell. You pass input arguments at creation:

```brief
let t = cell timer(1000);           // sync, blocking
let t = async cell! console(path);  // async, non-blocking
```

The cell's internal reactor loop decides *if* and *how* to transition based on its own private contracts. The caller sends signals; the cell responds autonomously.

---

## Outputs as Observable Differences

You cannot read a cell's internal fields directly. You must bind a trigger to an output port:

```brief
trg elapsed: Int @ timer.elapsed;
trg done: Bool @ timer.done;
```

The cell only communicates when it has executed a state transition that produces a difference on an output port. This is Bateson's definition of information — "a difference that makes a difference" — encoded in the type system.

---

## The Hardware Mapping

"Cell" is already a standard term in digital design: **standard cells** are the basic building blocks of silicon. A Brief `cell` maps directly:

| Brief Concept | Hardware Equivalent |
|--------------|---------------------|
| `cell` (auto-terminating) | Combinational logic |
| `cell!` (persistent) | Sequential logic (clocked) |
| Input arguments | Input pins |
| Output ports | Output pins |
| Private state | Flip-flops / registers |
| `trg` binding | Wire connecting output pin to input |
| `cell` invocation | Submodule instantiation |

Whether compiled to native code via LLVM or synthesized to Verilog via CIRCT, the name and structure remain accurate.

---

## Cognitive Load Management

Without the shield, a developer must hold the state machines of *all* components in their head simultaneously. With the shield:

- Editing `system_cell.bv`: focus on the console state machine
- Editing the parent: forget the console internals entirely

The compiler enforces the boundary, freeing cognitive attention. This is the ultimate utility of the `cell` primitive — not mystery, but **disciplined attention management**.

---

## Relationship to Brief Philosophy

Cells embody every core Brief principle:

- **Contract-First**: the output port types and `->` interface are the cell's contract with the world
- **No Magic**: all cell behavior is implemented in Brief, not in hardcoded Rust
- **Self-Documenting Failure**: a mistyped `trg` is a compile error, not a runtime segfault
- **Reactive Transactions**: the cell body is a set of `rct txn` blocks — the same primitive as the top-level program
- **Composition over Inheritance**: cells compose via triggers, not class hierarchies
