# 12. Compiler Pragmas — The Complete Reference

> These directives are Brief's complete set of compiler-intrinsic behavior.
> Everything else — imports, FFI, transactions, contracts — is standard library or user code.
> No hidden magic. If it doesn't appear on this page, the compiler doesn't know about it by name.

## Overview

Pragmas use the `#` prefix to signal "this is a compiler instruction, not application logic."
They are Brief's single, universal escape hatch, exhaustively documented here.

| Directive | Scope | Purpose |
|-----------|-------|---------|
| `#!dispatch(parallel)` | File-level | Enable parallel reactor dispatch |
| `#io` | File-level | Declare an OS-linked trigger |
| `#wake` | Trigger modifier | Mark a trigger as wake-capable |

---

## `#!dispatch(parallel)` — Parallel Reactor Mode

**Syntax at top of file:**
```brief
#!dispatch(parallel)
```

**Effect:** Changes the reactor from sequential (one transaction per tick) to parallel
(multiple non-conflicting transactions per tick). The compiler:
1. Evaluates ALL transaction preconditions upfront
2. Allocates a `fired_mask` accumulator
3. Fires each transaction whose precondition is true AND whose write-mask
   doesn't overlap with already-fired transactions

**Conflict detection:** Two transactions conflict if they write to the same state variable.
The compiler computes a `u64` bitmask for each transaction (bit N = writes field N)
and checks masks at runtime before firing.

**Backward compatibility:**
```brief
#pragma dispatch(parallel)       // Item-level (backward compat)
#!pragma dispatch(parallel)]     // File-level (backward compat)
```

---

## `#io` — OS Trigger Declarations

Declares an OS-linked trigger without needing to know the underlying C runtime symbol.

**Implicit form** (concept name = trigger name):
```brief
#io sigint;                     // Creates trg sigint: Bool @ link __sigint_flag
#io stdin_ready;                // Creates trg stdin_ready: Bool @ link __stdin_ready
#io stdin_line;                 // Creates trg stdin_line: String @ link __stdin_buffer
```

**Explicit form** (user-chosen name, type validation):
```brief
#io timer(1hz) -> trg clock_tick: Int;    // Validates type, renames trigger
```

**Parametrized concepts:**
```brief
#io timer(1hz);                  // 1-second timer
#io timer(100hz);                // 10ms timer
```

**Duplicates:** Error — each concept may only appear once per file.

**Type mismatch:** Error — if explicit form uses a different type than the registry declares.

### Canonical IO Concepts

| Concept | Type | Runtime Symbol | Description |
|---------|------|----------------|-------------|
| `sigint` | Bool | `__sigint_flag` | SIGINT interrupt (Ctrl+C) |
| `sigterm` | Bool | `__sigterm_flag` | SIGTERM termination signal |
| `sighup` | Bool | `__sighup_flag` | SIGHUP hangup signal |
| `stdin_ready` | Bool | `__stdin_ready` | Stdin has data available |
| `stdin_line` | String | `__stdin_buffer` | Current stdin line buffer |
| `timer(1hz)` | Int | `__timer_1hz` | 1-second timer tick |
| `timer(100hz)` | Int | `__timer_100hz` | 10ms timer tick |
| `io_pending` | Bool | `__io_pending` | Generic IO pending flag |
| `mouse_click` | Bool | `__io_mouse_click` | Mouse button click |
| `key_press` | Char | `__io_key_press` | Keyboard key press |

### Why `#io` exists

OS environments require platform-specific signal handling, timer setup, and event
demultiplexing. The `#io` pragma abstracts these details: the compiler maps each
concept name to the correct runtime symbol per target, and the runtime provides
the actual implementation signal handlers, epoll/kqueue integration, etc.

Embedded/Rendered Brief targets use raw `trg name: Type @ 0xADDRESS` instead,
where triggers are natively wake-capable via hardware interrupt lines.

---

## `#wake` — Trigger Wake Modifier

Marks a `@ link` trigger as wake-capable, meaning it can wake the reactor from
a blocking sleep.

**Syntax:**
```brief
trg sigint: Bool @ link __sigint_flag #wake;
```

**Rules:**
- `#wake` is valid only on `@ link` triggers (not MMIO)
- `trg x: Bool @ 0x4000 #wake;` — **error** (MMIO is natively wake-capable)
- `#io` implies `#wake` automatically (all OS triggers are wake-capable)

**Semantics:** The `#wake` flag is metadata for the runtime. When at least one
wake-capable trigger exists, the reactor may use blocking waits
(`epoll_wait`, `signalfd`, `kqueue`, `WFI`) instead of busy-looping.

---

## Migration Guide: `trg @ link` → `#io`

Old syntax (still works, fully supported):
```brief
trg sigint: Bool @ link __sigint_flag;
trg clock_tick_1hz: Int @ link __timer_1hz;
```

New syntax (preferred):
```brief
#io sigint;
#io timer(1hz) -> trg clock_tick_1hz: Int;
```

**Why migrate:**
1. No need to remember C runtime symbol names
2. Type validation — the compiler checks your declared type matches the registry
3. `#wake` is implied automatically
4. Platform portability — the registry can differ per target

**When NOT to migrate:**
- Custom runtime symbols not in the IO registry
- FPGA/bare-metal with hardware addresses
- Maximum portability (the `@ link` form works on any target without registry support)
