# `trg` Reactive Dirty-Flag Architecture + CIRCT Backend

**Date:** 2026-06-15
**Status:** Plan

## Motivation

A `trg` (trigger) is a top-level variable that the compiler assumes can change
at any time — an asynchronous input from outside the program's control flow.

| Target | `trg` semantics |
|--------|----------------|
| **LLVM** (native) | `volatile` load from a memory location; program blocks on `epoll` waiting for OS to write |
| **CIRCT** (hardware) | Input port on a `hw.module` — a physical wire |
| **Webstack** (WASM) | Reactive signal in the JS runtime, updated by DOM event handlers |

The compiler must:

1. **Prevent optimization on `trg` reads** — `load volatile` in LLVM, input ports in CIRCT, reactive signal accessors in webstack.
2. **Propagate changes efficiently** — compile-time dependency graph + bitmask dirty flags enable a flat `step()` function that recomputes only what changed.

## The Three Canonical Backends

After this plan is implemented, only three backends will be actively developed:

| Backend | Output | Domain |
|---------|--------|--------|
| **LLVM** | Native binary via `.ll` + `llc` | OS programs, CLI, servers |
| **Webstack** | WASM + JS glue via Rust/WASM | Browser, web apps |
| **CIRCT** | `.mlir` in HW+Comb+Seq dialects → `circt-opt` + `circt-translate` → Verilog | Hardware (ASIC, FPGA) |

All other current backends (`verilog.rs`, `vhdl.rs`, `c.rs`, `rust.rs`, `cobol.rs`,
`x86_64.rs`, `aarch64.rs`, `wasm.rs`, `tcl_generator.rs`) are **dead code** —
preserved in tree but receiving zero fixes, zero features, zero attention.
They remain only as a reference; deleting them is acceptable if they break.

## Architecture

### Compile-Time Dependency Graph

```
trg sensor_a: Int @ 0x4000;        // Bit 0  — external input
trg sensor_b: Int @ 0x4004;        // Bit 1  — external input
let derived_x: Int = sensor_a + 5; // Bit 2  — depends on sensor_a
let derived_y: Int = sensor_b * 2; // Bit 3  — depends on sensor_b
let final: Int = derived_x + derived_y; // Bit 4 — depends on derived_x, derived_y
```

The compiler builds a DAG during analysis:

```
sensor_a ──→ derived_x ──→ final
sensor_b ──→ derived_y ──↑
```

Topological order: `[sensor_a, sensor_b, derived_x, derived_y, final]`
Bit assignments: `sensor_a=0, sensor_b=1, derived_x=2, derived_y=3, final=4`

### Dirty-Flag Bitmask

A single `u64` register `dirty_flags` tracks which variables need recomputation:

- **Write path:** When a `trg` fires (epoll returns / DOM event fires / wire changes), its bit is set in `dirty_flags`.
- **`step()` function:** Iterates variables in topological order. For each variable, if its dirty bit is set (or any dependency's bit is set), it recomputes the variable, marks downstream bits, clears its own bit.
- **Zero-allocation:** No queues, no pointer chasing, no heap. Just bitwise ops on a u64.

### Event-Driven Wake (No Polling)

The program never spin-waits. It blocks on `epoll` (LLVM), sleeps on a WASM event (webstack), or reacts to wire transitions (CIRCT).

## Implementation Phases

### Phase 1 — Dependency Graph Analysis

**New file:** `src/analysis/dependency_graph.rs`

- Walk all `TopLevel::StateDecl`, `TopLevel::Trigger`, `TopLevel::Transaction`
- For each variable identified in expressions, trace which other variables it reads
- Build DAG: `trg` inputs → intermediate variables → outputs
- Topological sort using Kahn's algorithm (detect cycles → compiler error)
- Assign bit indices (bits 0..63 for u64; extend to multiple u64s with hierarchical summary bits for larger graphs)
- Expose as `DependencyGraph`:

```rust
pub struct DependencyGraph {
    pub topo_order: Vec<String>,          // topological variable order
    pub bit_index: HashMap<String, usize>,// variable → bit position
    pub dependencies: HashMap<String, Vec<String>>, // variable → deps
    pub dependents: HashMap<String, Vec<String>>,   // variable → downstream
    pub is_trg: HashSet<String>,          // variables declared as `trg`
}
```

### Phase 2 — Dirty Flag Infrastructure

**New types in `src/backend/mod.rs`:**

```rust
pub struct DirtyFlags(pub u64);

impl DirtyFlags {
    pub fn mark(&mut self, var_bit: usize) { self.0 |= 1 << var_bit; }
    pub fn is_set(&self, var_bit: usize) -> bool { self.0 & (1 << var_bit) != 0 }
    pub fn clear(&mut self, var_bit: usize) { self.0 &= !(1 << var_bit); }
    pub fn any_set(&self) -> bool { self.0 != 0 }
    pub fn mark_downstream(&mut self, var_bit: usize, graph: &DependencyGraph) {
        if let Some(deps) = graph.dependents.get(&var_name) {
            for dep in deps {
                if let Some(bit) = graph.bit_index.get(dep) {
                    self.mark(*bit);
                }
            }
        }
    }
    pub fn merge(&mut self, other: &DirtyFlags) { self.0 |= other.0; }
}
```

### Phase 3 — LLVM Backend: `step()` + Event-Driven Epoll Loop

**Changes to `src/backend/llvm/`:**

#### 3a. New `emit_step_fn` in `emit_toplevel.rs`

Generate the flat `step()` function using topological order from the
dependency graph. Pattern per variable:

```llvm
define void @step() {
  %df = load volatile i64, i64* @dirty_flags

  ; --- sensor_a (bit 0) ---
  %b0 = and i64 %df, 1
  %t0 = icmp ne i64 %b0, 0
  br i1 %t0, label %eval_sensor_a, label %check_sensor_b

eval_sensor_a:
  %val_a = load volatile i64, i64* @sensor_a
  store i64 %val_a, i64* @derived_x
  ; mark derived_x (bit 2) dirty
  %df1 = or i64 %df, 4
  ; clear sensor_a
  %df2 = and i64 %df1, -2
  store volatile i64 %df2, i64* @dirty_flags
  br label %check_sensor_b

  ; ... continue for all variables in topological order
}
```

`trg` variable loads inside `step()` use `load volatile` to prevent LLVM
from caching values across iterations.

#### 3b. Replace `emit_trg_init` / `emit_trg_load` with epoll loop

Replace the current polling `__trg_stdin_read` etc. with an event loop:

Initialization:
- Create epoll fd
- Register stdin fd, timer fds (`timerfd_create`), signal fds (`signalfd`)
- User-provided `@ link` symbols register custom fds

Event loop at `main()` entry:
```llvm
define void @main() {
  call void @init_trg_epoll()
  br label %event_loop

event_loop:
  %fd = call i32 @epoll_wait(...)  ; blocks until something fires
  ; determine which trg(s) fired, set their dirty bits
  call void @set_trg_dirty(...)
  call void @step()
  br label %event_loop
}
```

#### 3c. Remove old `__trg_*` infrastructure

- Delete `emit_trg_init()` (timerfd/signalfd open — replaced by epoll registration)
- Delete `emit_trg_load_finish()` (no longer needed)
- Remove `__trg_stdin_read`, `__trg_timerfd_open`, `__trg_timerfd_read`,
  `__trg_signalfd_open`, `__trg_signalfd_read` declares from `mod.rs`
- Remove `briev_trg` section variables from `briev_rt.c` (`__io_pending`,
  `__sigint_flag`, `__sigterm_flag`, `__sighup_flag`, `__timer_1hz`,
  `__timer_100hz`, `__stdin_ready`, `__stdin_buffer`, `__tty_read_key`)

#### 3d. Keep existing dispatch as fallback

The old snapshot-then-dispatch reactor (`dispatch.rs`) remains for programs
with NO `trg` declarations — where classic `node` convergence is the
only active pattern.

### Phase 4 — CIRCT Backend: Hardware via HW + Comb + Seq

**New file:** `src/backend/circt.rs`

Emit MLIR text in CIRCT's HW, Comb, and Seq dialects. The pattern mirrors
the LLVM backend's `.ll` text emission.

#### 4a. Dialect mapping

| Briev construct | CIRCT dialect | Example |
|---|---|---|
| `trg x: Int @ 0x4000` | HW | `hw.module port %x : !hw.inout<i64>` |
| `trg y: Bool @ link btn` | HW | `hw.module port %y : i1` (input) |
| `let sum = a + b` | Comb | `%sum = comb.add %a, %b : i64` |
| `let flag = x > 0` | Comb | `%flag = comb.icmp gt %x, %c0_i64 : i64` |
| `let sel = cond ? a : b` | Comb | `%sel = comb.mux %cond, %a, %b : i64` |
| `node [pre][post] { body }` | Comb + Seq | Guard → `comb.icmp`, state → `seq.compreg` |
| `let state: Int = 0` | Seq | `%reg = seq.compreg %clk, %d` |

#### 4b. Module structure

```mlir
hw.module @top(%clk: i1, %rst: i1, %sensor_a: i64 [@0x4000], %sensor_b: i64 [@0x4004])
    -> (out: i64) {

  // Combinatorial: derived_x = sensor_a + 5
  %c5 = hw.constant 5 : i64
  %derived_x = comb.add %sensor_a, %c5 : i64

  // Combinatorial: derived_y = sensor_b * 2
  %c2 = hw.constant 2 : i64
  %derived_y = comb.mul %sensor_b, %c2 : i64

  // Combinatorial: final = derived_x + derived_y
  %final = comb.add %derived_x, %derived_y : i64

  // output
  hw.output %final : i64
}
```

`trg` inputs are combinatorial (pure function of inputs) — no dirty-flag step()
needed in hardware. The compiler verifies the dependency graph is acyclic
(a cyclic combinational path is a hardware bug).

#### 4c. Clocked state

For state variables (`let` with initializer and `node` updates):

```mlir
  // let counter: Int = 0;
  %counter_init = hw.constant 0 : i64
  %counter = seq.compreg %clk, %counter_next, %rst, %counter_init

  // txn increment [counter < 10][counter == 10] { &counter = counter + 1; };
  %c10 = hw.constant 10 : i64
  %guard = comb.icmp ult %counter, %c10 : i64
  %c1 = hw.constant 1 : i64
  %counter_next = comb.add %counter, %c1 : i64  ; only valid when guard true
```

#### 4d. Pipeline integration

The compiler invokes `circt-opt` and `circt-translate` externally:

```
briev-compiler --target circt program.bv -o program.mlir
circt-opt --lower-hw --lower-seq program.mlir -o program.lowered.mlir
circt-translate --export-verilog program.lowered.mlir -o program.v
```

The user can customize the lowering pipeline or skip it to get raw `.mlir`.

### Phase 5 — Webstack: Dirty-Flag Reactive Signals

**Changes to `src/backend/webstack.rs`:**

#### 5a. Wire `TopLevel::Trigger` into signal system

Replace the silent `_ => {}` drop at `webstack.rs:427` with proper
registration in the signal map:

```rust
TopLevel::Trigger(trg) => {
    let signal_id = signals.len();
    signal_map.insert(trg.name.clone(), signal_id);
    signals.push(trg.ty.clone());
    dirty_signals.push(false);
    // Dependency tracking: which reactive txns depend on this trg
    reactive_dependency_map.insert(trg.name.clone(), Vec::new());
}
```

#### 5b. Connect `Directive::Trigger` to dirty flags

When a DOM event fires (via existing `Directive::Trigger` → `addEventListener`),
the handler sets the corresponding `trg`'s dirty flag:

```js
// Generated JS:
const TRG_BITS = { sensor_a: 0, sensor_b: 1 };
let dirtyFlags = 0n;

// From Directive::Trigger:
el.addEventListener('click', () => {
  dirtyFlags |= (1n << BigInt(TRG_BITS.sensor_a));
  step();
});
```

#### 5c. Generate `step()` function in JS/WASM

Same pattern as LLVM — topological order, bit checks:

```js
function step() {
  if (dirtyFlags & (1 << 0)) {
    derived_x = wasm.sensor_a + 5;
    dirtyFlags |= (1 << 2);
    dirtyFlags &= ~(1 << 0);
  }
  if (dirtyFlags & (1 << 1)) {
    derived_y = wasm.sensor_b * 2;
    dirtyFlags |= (1 << 3);
    dirtyFlags &= ~(1 << 1);
  }
  if (dirtyFlags & ((1 << 2) | (1 << 3))) {
    final = derived_x + derived_y;
    dirtyFlags |= (1 << 4);
    dirtyFlags &= ~((1 << 2) | (1 << 3));
  }
  if (dirtyFlags & (1 << 4)) {
    render(final);
    dirtyFlags &= ~(1 << 4);
  }
}
```

#### 5d. Extend `reactive_dependency_map`

Currently tracks at transaction granularity. Extend to variable-level
granularity using the dependency graph from Phase 1.

### Phase 6 — Remove Polling `__trg_*` C Runtime

**Changes to `lib/runtime/briev_rt.c` and `src/backend/llvm/mod.rs`:**

Delete:
- `__trg_stdin_read()` — C function in `briev_rt.c`
- `__trg_timerfd_open()` / `__trg_timerfd_read()` — C functions in `briev_rt.c`
- `__trg_signalfd_open()` / `__trg_signalfd_read()` — C functions in `briev_rt.c`
- All `__trg_*` `declare` stubs in `src/backend/llvm/mod.rs` lines 767–771
- `briev_trg` section variables: `__io_pending`, `__sigint_flag`, `__sigterm_flag`,
  `__sighup_flag`, `__timer_1hz`, `__timer_100hz`, `__stdin_ready`, `__stdin_buffer`,
  `__tty_read_key`

These are replaced by:
- `epoll` loop in LLVM backend (Phase 3)
- `ast`-based `#`-intrinsics for the underlying OS operations (`epoll_create1#`,
  `epoll_ctl#`, `epoll_wait#`) — or direct syscall inlining

## Backend Phase-Out Strategy

| Backend | Status | Replacement |
|---------|--------|-------------|
| LLVM (`src/backend/llvm/`) | **Active** | Canonical native target |
| Webstack (`webstack.rs`) | **Active** | Canonical web target |
| CIRCT (`circt.rs` — new) | **Active** | Canonical hardware target |
| Verilog (`verilog.rs`) | **Dead** | Kept for reference, CIRCT supersedes |
| VHDL (`vhdl.rs`) | **Dead** | Kept for reference, CIRCT supersedes |
| C (`c.rs`) | **Dead** | Kept for reference |
| Rust (`rust.rs`) | **Dead** | Kept for reference |
| WASM (`wasm.rs`) | **Dead** | Webstack supersedes |
| x86\_64 / aarch64 | **Dead** | LLVM supersedes |
| COBOL (`cobol.rs`) | **Dead** | Kept for novelty |
| TCL (`tcl_generator.rs`) | **Dead** | Kept for reference |

Dead backends receive zero fixes, zero features, zero attention.
If a dead backend breaks due to an API change, delete the broken code or
use `#[allow(...)]` — never implement new behavior.

## Files Summary

| Phase | Files | Change |
|-------|-------|--------|
| 1 | `src/analysis/dependency_graph.rs` (new) | +~300 lines |
| 1 | `src/backend/mod.rs` | +DependencyGraph struct |
| 1 | `src/analysis/mod.rs` | Register new module |
| 2 | `src/backend/mod.rs` | +DirtyFlags struct |
| 3 | `src/backend/llvm/emit_toplevel.rs` | +emit_step_fn, +epoll loop init |
| 3 | `src/backend/llvm/emit_stmt.rs` | Remove LocalTrigger stub |
| 3 | `src/backend/llvm/mod.rs` | Remove __trg_* declares |
| 3 | `src/backend/llvm/dispatch.rs` | Keep as fallback for no-trg programs |
| 4 | `src/backend/circt.rs` (new) | +~1500–2000 lines |
| 4 | `src/backend/router.rs` | Add --target circt dispatch |
| 4 | `src/main.rs` | Add circt target flag, circt-opt/circt-translate pipeline |
| 5 | `src/backend/webstack.rs` | Wire TopLevel::Trigger, step() gen |
| 6 | `lib/runtime/briev_rt.c` | Remove ~80 lines of __trg_* |
| 6 | `src/backend/llvm/mod.rs` | Remove __trg_* declare stubs |

## Dependencies Between Phases

```
Phase 1 (Dependency Graph)
  ├── Phase 2 (Dirty Flags)
  │     ├── Phase 3 (LLVM step() + epoll)
  │     ├── Phase 4 (CIRCT — uses dep graph for acyclicity)
  │     └── Phase 5 (Webstack — uses dep graph + dirty flags)
  └── Phase 6 (Cleanup — only after Phase 3 completed)
```

Phases 3, 4, and 5 are parallelizable after Phase 1+2 are done.

## Execution Priority

1. **Phase 1 + 2** — foundational, unblocks everything
2. **Phase 3 (LLVM)** — replaces the current polling loop, immediate
   correctness + performance payoff
3. **Phase 5 (Webstack)** — relatively contained, existing reactive
   infrastructure to build on
4. **Phase 4 (CIRCT)** — largest scope, but mostly independent
5. **Phase 6 (Cleanup)** — only after Phase 3 is verified

## Test Strategy

- **Phase 1 tests:** Build a known program with `trg` inputs and derived
  variables; verify `DependencyGraph.topo_order`, `bit_index`, cycle detection.
- **Phase 3 tests:** Compile a program with `trg @ link` to `.ll`; verify
  presence of `load volatile`, `@dirty_flags`, and `@step` in the IR.
- **Phase 4 tests:** Compile a program with `trg` to `.mlir`; verify
  `hw.module`, `comb.add`, `seq.compreg` ops. Run `circt-opt --verify` on output.
- **Phase 5 tests:** Compile a `trg`-based program to webstack; verify
  generated JS contains `dirtyFlags`, `step()`, and correct event wiring.
- **All phases:** Existing 881+ tests must continue to pass (regression).
