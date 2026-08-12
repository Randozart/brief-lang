# Event Model: `@ link` as the Universal Doorbell

**Date:** 2026-05-29
**Status:** Specification
**Version:** 1.0 — Final (Zero Magic)

## 1. Philosophy: No Magic

Briev's event model has zero compiler-generated functions, zero hidden keywords,
zero implicit behavior, and zero intrinsic symbols. Everything is composed from
five existing language primitives. Nothing is known to the compiler by name:

| Primitive | Role | Magic Level |
|-----------|------|-------------|
| `trg name: Type @ link sym` | Declare a volatile global — the runtime writes to it | Zero — the backend just emits `@sym = external global` for whatever name it sees |
| `trg name: Type @ 0x...` | Declare an MMIO location — hardware writes to it | Zero |
| `node [pre] { ... term; }` | "Sleep until precondition is true" — the event handler | Zero |
| `frgn name(args) -> Ret from "lib"` | FFI boundary to the outside world | Zero — the backend just emits `declare` + `call` |
| `defn name(args) -> Ret { ... term val; }` | Pure accessor on cached state | Zero |

The operating principle: **`@ link` is the universal doorbell.** The runtime (or
hardware) writes to a known global address. The `trg` declaration is a volatile
window onto that address. The `node` gates on it. That is the entire event
model. Everything else is library code.

### How "Sleep" Works Without Magic

There is no `__wait_for_event` intrinsic. The equilibrium path in the LLVM IR
just does `ret void`. The user (or stdlib) provides sleep as an explicit
`frgn` + `node [true]` pattern:

```briev
// User code:
frgn __wait_for_event() -> Void from "libruntime";
// ^ A regular FFI declaration. The linker resolves it.

node my_work [some_trigger] { ... term; };

// Last in dispatch order — only fires when nothing else is true:
node sleep [true] { __wait_for_event(); term; };
```

Because `node` uses **fall-through dispatch** (all preconditions evaluated
sequentially in one tick), and `sleep` is declared last, it fires only when
no earlier transaction's precondition was true. Each transaction's side
effects are visible to the precondition evaluation of the next transaction
in the same tick — so `__io_pump` can set `io_ready` and a downstream
consumer can read it within the same tick.

This is the "equilibrium sleep" pattern, composed entirely from existing
primitives.

The same symbol `__wait_for_event` is provided by `runtime/briev_rt.c` (one C
file, `#ifdef` handles platforms). The user links it once. No compiler knows
its name.

## 2. The Reactor Tick

The reactor tick never changes. It is always:

```
reactor_tick():
  1. Sample phase: load volatile from every @ link / @ address trigger
  2. Evaluate phase: check each node's precondition using sampled values
  3. Execute phase: fire the first true-precondition transaction
  4. If none fired: ret void (tick loop continues, re-evaluates next tick)
```

Step 4 is a simple `ret void`. The tick loop (`main` → `tick:` → `call
@reactor_tick()` → `br %tick`) handles the repetition. If the user wants to
block the thread between ticks, they declare a `frgn __wait_for_event()` and
write a `node [true] { __wait_for_event(); term; }` as their last
transaction. This pattern is provided by `lib/std/io.bv`.

There is no compiler-generated `__wait_for_event()` call. The backend emits
only what the user's `frgn` declarations and `node` bodies tell it to.

## 3. `@ link` Semantics

### Declaration

```briev
trg io_pending: Bool @ link __io_pending;
trg sigint_flag: Bool @ link __sigint_flag;
```

The `@ link name` tells the compiler:
- Emit `@name = external global <type>, align <align>` in the LLVM module
- Access it via `load volatile` — never cache, never hoist, never eliminate

The linker is responsible for resolving `@name`. The runtime (a small C/asm
shim shipped with the compiler) defines these symbols.

### Name Convention

Reserved runtime symbols use a `__` prefix:

| Symbol | Type | Written By |
|--------|------|------------|
| `__io_pending` | `i8` (Bool) | Any event source (interrupt, signal, timer) |
| `__sigint_flag` | `i8` (Bool) | SIGINT handler |
| `__sigterm_flag` | `i8` (Bool) | SIGTERM handler |
| `__timer_1hz` | `i64` | Timer interrupt, incremented once per second |
| `__timer_100hz` | `i64` | Timer interrupt, incremented 100×/sec |
| `__stdin_line_avail` | `i8` (Bool) | Stdin data available |
| `__stdin_line_buf` | `i8*` (ptr) | Pointer to line buffer |

Users are free to declare `@ link` to any symbol name. The runtime only
reserves the `__` prefix. Custom `@ link` symbols must be provided by the
user's own runtime or linker script.

### MMIO vs Link

```briev
// MMIO — bare-metal, FPGA: hardware writes to 0x40001000
trg button: Bool @ 0x40001000;

// Link — OS, WASM: runtime writes to @__io_pending
trg io_pending: Bool @ link __io_pending;
```

Both produce identical LLVM IR except for how the pointer is obtained:
- `@ 0x...` → `inttoptr (i64 0x40001000 to i8*)`
- `@ link sym` → the global `@sym` itself

Both use `load volatile`. Both are sampled once per tick.

## 4. The Standard IO Library Pattern

The standard library (`lib/std/io.bv`) demonstrates the pattern:

### Layer 1: FFI Boundary

```briev
// io/internal.bv — the raw FFI contract
frgn __raw_poll() -> Vector<u8> from "libruntime";
```

`__raw_poll()` is guaranteed non-blocking because the runtime has already
determined (via `__wait_for_event` / interrupt handler) that data is available.

### Layer 2: Pump Transaction

```briev
// io.bv
import system from "std/system.bv";
// system.bv declares: trg __io_pending: Bool @ link __io_pending;

let __io_buffer: Vector<u8> = [];
let __io_ready: Bool = false;

// Fires whenever the runtime signals an event.
// Guaranteed first in dispatch order because no other txn gates on __io_pending
// (the pump clears __io_pending and sets __io_ready for consumers).
node __io_pump [__io_pending] {
    &__io_buffer = __raw_poll();
    &__io_ready = true;
    term;
};
```

The pump runs when `__io_pending` is true. It calls the non-blocking FFI poll,
fills the buffer, and signals `__io_ready` for downstream consumers.

### Layer 3: Accessor Defns

```briev
// io.bv — pure functions on the cached buffer, zero FFI
defn key_pressed(key: String) -> Bool {
    let i: Int = 0;
    guard [i < len(__io_buffer)] {
        if __io_buffer[i] == key {
            term true;
        };
        &i = i + 1;
    };
    term false;
};

defn get_char() -> String {
    if len(__io_buffer) > 0 {
        term __io_buffer[0];
    };
    term "";
};

defn get_mouse_position() -> (Int, Int) {
    // Parse mouse event from __io_buffer
    // ...
    term (0, 0);
};
```

### Layer 4: User Code

```briev
import io from "std/io.bv";
import system from "std/system.bv";

let jumping: Bool = false;
let moving: Bool = false;

node handle_input [io.__io_ready] {
    [io.key_pressed("Space")] { &jumping = true; };
    [io.key_pressed("W")]    { &moving = true; };
    &io.__io_ready = false;    // consume the event
    term;
};

node physics [true] {
    [jumping] { &velocity = -10; &jumping = false; };
    [moving]  { &x = x + 1; };
    term;
};
```

The user never calls `__raw_poll`. They call `key_pressed("Space")` which is a
pure scan of the cached buffer.

## 5. Multiple Event Sources

Independent reactive transactions can gate on independent triggers:

```briev
trg mouse_click: Bool @ link __mouse_click;
trg timer_tick: Bool @ link __timer_100hz;

node handle_mouse [mouse_click] { ... term; };
node handle_timer [timer_tick]  { ... term; };
```

Both preconditions are evaluated each tick using sampled values. The dispatch
order determines priority (first-true-wins by default). If neither is true, the
reactor reaches equilibrium and sleeps.

### Parallel Dispatch Consideration

For video games and UIs, "first-true-wins" is too restrictive — you want
*mouse* and *timer* and *keyboard* to all fire in the same tick if all are
ready. The proof engine already has `check_mutual_exclusion` (proof_engine.rs)
which can verify that two transactions write to disjoint fields.

Future work: a parallel dispatch mode where the backend emits:

```llvm
; Instead of evaluating A, then B if A is false:
; Evaluate all preconditions, then fire all non-conflicting:
%a_ready = ...
%b_ready = ...
%c_ready = ...
br i1 %a_ready, label %exec_a, label %try_b
try_b:
  br i1 %b_ready, label %exec_b, label %try_c
; ...but exec_a and exec_b can fire in the same tick if conflict-free
```

This is not yet implemented. The current backend uses first-true-wins. The
infrastructure (mutual exclusion check, write-set analysis) already exists.

## 6. Portability

| Platform | `@ link` Provider | `__wait_for_event()` | `__raw_poll()` |
|----------|------------------|---------------------|----------------|
| Linux (x86_64/AArch64) | Runtime C shim uses `epoll` + signal handlers | `epoll_wait(epfd, ..., -1)` | `read()` from epoll-mapped fds |
| FreeBSD/macOS | Runtime C shim uses `kqueue` + signal handlers | `kevent(kq, NULL, 0, events, nevents, NULL)` | `read()` from kqueue-mapped fds |
| Windows | Runtime C shim uses `WaitForMultipleObjects` | `WaitForMultipleObjects(n, handles, FALSE, INFINITE)` | `ReadFile()` on ready handles |
| Bare-metal ARM | Interrupt vector table writes to `@ link` address | `wfi` | No-op (data is already in the MMIO register) |
| Bare-metal x86 | IDT handler writes to `@ link` address | `sti; hlt` | No-op |
| WASM | Host environment writes to shared memory | Asyncify yield | Synchronous read from shared memory |
| FPGA | Hardware writes to `@ address` | No-op (tick runs continuously) | N/A (no OS buffers) |

The compiler emits the same IR for all targets. The runtime shim and linker
script handle platform differences.

## 7. Without IO Import

If the user does not import `std/io.bv`, they must either:

1. **Use MMIO triggers directly** (`trg btn: Bool @ 0x40001000` with `node`)
2. **Write their own FFI poll** (`frgn my_poll() -> ... from "lib"`) inside a
   custom `node [true]` — polling busy-loop
3. **Define custom `@ link` symbols** and provide a linker script that maps
   them

The `__wait_for_event()` intrinsic is always available (linker-provided), but
without a `trg @ link __io_pending` declaration, the reactor will always reach
equilibrium and sleep. On OS targets this means the process blocks until a
signal kills it. On bare-metal with MMIO triggers, the MMIO address is the
interrupt source — no `@ link` needed.

## 8. `trg!` Inside Transactions

**The `trg!` statement is deprecated for event-driven programming.**

The original concept — a mid-transaction async yield point — was never
implemented in any backend. Every backend emits `trg!` as a comment.

The correct pattern is:

```briev
// Instead of:
node bad [true] {
    phase1();
    trg! wait: Bool = ready;  // yield here — never implemented
    phase2();
    term;
};

// Use:
trg ready: Bool @ link __something;  // or @ 0x...
node phase1 [ready] {
    phase1();
    term;
};
node phase2 [phase1_done] {       // gate on state set by phase1
    phase2();
    term;
};
```

The parser still accepts `trg!` (backward compatibility), but the LLVM backend
emits it as a no-op comment. New code should use the top-level `trg` + `node` pattern.

## 9. Implementation Plan

### Phase 0: Documentation (this spec + LLVM lowering spec)

### Phase 1: Runtime shim
- Create `runtime/` directory
- Linux shim: `epoll`-based `__wait_for_event()`, signal handlers for `@ link` symbols
- Bare-metal stubs: `wfi`/`hlt` implementations

### Phase 2: Update stdlib
- Add `@ link` bindings to `lib/std/system.bv` trigger declarations
- Add pump transaction + `__raw_poll()` FFI + accessor defns to `lib/std/io.bv`

### Phase 3: LLVM backend fixes (15 bugs in PHASE-REOPT-LLVM.md)
- Phases A–F, starting with SSA correctness, ending with optimization enhancement
