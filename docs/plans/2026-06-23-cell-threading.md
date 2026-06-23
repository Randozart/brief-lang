# True Threading for Persistent Cells — `cell!` with Independent Execution

**Date**: 2026-06-23  
**Status**: specification  
**Depends on**: true async persistent cells (committed, Phase `cell-async-persistent.md`)

---

## 1. Core Idea

The current async `cell!` ticks all persistent cells cooperatively in the main reactor loop — one convergence pass per cell per iteration, single-threaded. True threading means each persistent cell runs its convergence loop in its own thread (or async task), communicating output changes to the parent via atomic channels.

```brief
// Current (cooperative — all cells tick in main loop):
trg count: Int @ counter();     // ticks every main loop iteration
trg buffer: String @ console(); // ticks every main loop iteration

// Desired (threaded — each cell ticks independently):
trg count: Int @ counter() @1MHz;     // ticks at 1MHz in its own thread
trg buffer: String @ console();       // ticks on stdin events in epoll thread
```

### Why Threading Matters

The cooperative model works for I/O-bound cells (console, timers) where each convergence pass is fast. It breaks for:

- **CPU-bound cells**: Audio synthesis running alongside a UI reactor — one blocks the other
- **Different time domains**: `@1kHz` timer and `@10kHz` sensor filter need different tick rates that don't align with the main loop rate
- **Real-time constraints**: A PID controller must tick at precise intervals regardless of main loop load

---

## 2. Architecture

### 2.1 Memory Model — Per-Cell State Copies

The main blocker for threading is the `%State` struct — all state fields are in one flat LLVM struct, and the cell's fields are just prefixed slots within it (`cell$name$field`). Threading requires:

- **Dedicated per-cell state**: Each persistent cell gets its own `%CellState` allocation (either on the thread's stack or as a heap allocation). This is the plan's original Phase 2 "sub-state GEP" — a LLVM struct type per cell containing only its fields.
- **No shared writes**: The cell thread writes only to its own `%CellState`. Output port values are communicated to the parent via atomic channels.
- **Parent reads**: The parent reactor reads output values from the channel's latest-value slot, not from `%State`.

### 2.2 Thread Lifecycle

```
┌─ Main Reactor Loop ─────────────────────┐
│  trg count: Int @ counter() @1MHz;      │
│                                          │
│  On program start:                       │
│    spawn_thread(counter_tick, args)      │
│    // counter runs independently         │
│                                          │
│  Each reactor iteration:                 │
│    check_channel("counter.count")        │
│    if changed: update state["count"]     │
│    process local transactions            │
│                                          │
│  On program exit:                        │
│    join_thread(counter)                  │
└──────────────────────────────────────────┘

┌─ Cell Thread ────────────────────────────┐
│  void* counter_tick(void* cell_state) {  │
│    while (!terminated) {                 │
│      sleep_ns(tick_interval_ns);         │
│      convergence_pass(cell_state);       │
│      atomic_send(output_channel, val);   │
│    }                                     │
│    return NULL;                          │
│  }                                       │
└──────────────────────────────────────────┘
```

### 2.3 Channel Protocol

Output communication uses a **latest-value channel** (single producer, single consumer):

```rust
pub struct CellChannel {
    /// Latest output value per port name. Written by cell thread,
    /// read by parent thread. Atomic u64 for lock-free read/write.
    pub values: HashMap<String, AtomicU64>,
    /// Dirty flags — set to 1 by cell thread on write, cleared by
    /// parent after read. Avoids polling overhead.
    pub dirty: HashMap<String, AtomicBool>,
}
```

**Writer (cell thread):**
1. Compute new output value
2. `atomic_store(dirty[port], 0)` — clear while writing (prevents partial-read)
3. `atomic_store(values[port], new_val)` — write the value
4. `atomic_store(dirty[port], 1)` — signal availability

**Reader (parent reactor):**
1. `if atomic_load(dirty[port]) != 0` — check if new value available
2. `val = atomic_load(values[port])` — read the value
3. `atomic_store(dirty[port], 0)` — acknowledge

### 2.4 Termination Protocol

- **Cell-initiated**: When a cell's transaction executes `term!`, the cell thread sets a `terminated` flag, does a final channel send, and exits.
- **Parent-initiated**: When the program exits, the main loop sets a global `program_exit` flag. All cell threads check this after each convergence pass and exit cleanly.
- **Force kill**: After a timeout (configurable via `--cell-kill-timeout`), `pthread_cancel` for unresponsive cells.

---

## 3. Implementation Plan

### Step 1: Cell State Isolation (days 1–3)

**Problem**: Current `cell$name$field` slots live in `%State`, which is shared. Threading requires per-cell state.

**Solution**: For each persistent cell, allocate a separate heap struct containing only its fields. In the LLVM backend:

1. In `build_field_index`, DON'T add persistent cell fields to `%State`. Instead, build a separate mapping of `cell_name -> Vec<(field_name, Type)>`.
2. Emit `%CellState.cellName = type { i64, i64, ... }` for each persistent cell.
3. Emit `@cell_name_init(ptr %cell_state, ...)` that initializes fields from defaults + args.
4. Emit `@cell_name_tick(ptr %cell_state)` — the convergence loop (single pass), reads/writes `%cell_state` instead of `%State`.

**Files**: `src/backend/llvm/emit_toplevel.rs`, `mod.rs`, `emit_expr.rs`

**Estimated**: 200 lines

### Step 2: Thread Spawning Infrastructure (days 4–6)

**Problem**: LLVM IR has no built-in thread spawning. Need to emit `pthread_create` calls.

**Solution**:

1. Add `declare i32 @pthread_create(ptr, ptr, ptr, ptr)` to `emit_declares` in `emit_toplevel.rs`.
2. Add `declare void @pthread_exit(i32)` and `declare i32 @pthread_join(ptr, ptr)`.
3. In `@main`, after `emit_inline_init_stores`, emit a `pthread_create` call for each persistent cell:

```llvm
; Allocate thread struct and cell state
%cell_state = call ptr @malloc(i64 16)           ; 2 x i64 fields
call void @cell_name_init(ptr %cell_state, ...)
%thread = alloca i64
%tp = bitcast ptr @cell_name_tick to ptr
call i32 @pthread_create(ptr %thread, ptr null, ptr %tp, ptr %cell_state)
```

4. Before program exit, emit `pthread_join` for each thread.

**Alternative**: Use the C11 threads API (`thrd_create`) if available, or POSIX threads. The LLVM backend already assumes POSIX (`fcntl`, `timerfd_create`, etc.), so `pthread` is consistent.

**Files**: `src/backend/llvm/emit_toplevel.rs`, `loop_engine.rs`

**Estimated**: 150 lines

### Step 3: Channel Implementation (days 6–8)

**Problem**: Need atomic communication between cell threads and the main reactor without mutexes.

**Solution**:

1. Add `CellChannel` struct to the interpreter (Rust side) and emit equivalent LLVM IR.
2. Each channel is a pair of `i64` slots in `%State`: one for the value, one for the dirty flag. These are NOT used by the cell's own codegen (which uses `%cell_state`), but the main loop reads them.
3. In the cell tick function: after the convergence pass, `atomic store` the output value + dirty flag.
4. In `@reactor_tick` or `@main`: check dirty flag for each persistent cell's channel, load new value if dirty, update the corresponding trigger variable in `%State`.

**LLVM atomics**: Use `store atomic i64 %val, ptr %chan_val, seq_cst` and `load atomic i64, ptr %chan_val, seq_cst` for memory ordering.

**Files**: `src/backend/llvm/emit_toplevel.rs`, `dispatch.rs`, `loop_engine.rs`

**Estimated**: 250 lines

### Step 4: Rate Limiting with Wall-Clock Timing (days 8–10)

**Problem**: Current `tick_interval` is based on main loop iteration count, not real time. Threading enables proper sleep-based timing.

**Solution**:

1. In the cell tick function (running in its own thread), compute sleep duration from `@Hz`:
```llvm
%period_ns = udiv i64 1000000000, <hz_value>
call void @nanosleep(ptr %ts, ptr null)
```
2. Add `declare i32 @nanosleep(ptr, ptr)` to `emit_declares`.
3. For cells without `@Hz` (default): sleep for a configurable default period (e.g., 1ms = `@1kHz`).
4. For cells that should tick on every main loop iteration (no `@Hz`, event-driven): use a condition variable or eventfd instead of sleep.

**Files**: `src/backend/llvm/emit_toplevel.rs`, `mod.rs`

**Estimated**: 100 lines

### Step 5: Interpreter Threading (days 10–12)

**Problem**: The interpreter needs threading support for consistency with the LLVM backend. Current `tick_persistent_cells` is synchronous.

**Solution**:

1. Add `persistent_threads: HashMap<String, JoinHandle<()>>` to `Interpreter`.
2. On `register_persistent_cell`: spawn a thread that runs the convergence loop in a loop with sleep-based timing.
3. Channels use `Arc<Mutex<HashMap<String, Value>>>` for the interpreter (simpler than atomic since we're in Rust).
4. On `tick_persistent_cells`: check channels for changed outputs (instead of running convergence).
5. On `unregister_persistent_cell`: set a termination flag and join the thread.

**Files**: `src/interpreter.rs`

**Estimated**: 200 lines

---

## 4. File-by-File Change Summary

| File | Lines | What |
|------|-------|------|
| `src/backend/llvm/emit_toplevel.rs` | +300 | `%CellState` type emission, cell init/tick functions, pthread_create, nanosleep |
| `src/backend/llvm/mod.rs` | +50 | `build_field_index` separates persistent cell fields from %State, CellChannel fields |
| `src/backend/llvm/emit_expr.rs` | +20 | Cell tick codegen uses `%cell_state` instead of `%State` for persistent cells |
| `src/backend/llvm/dispatch.rs` | +20 | Main loop reads cell output channels after reactor_tick |
| `src/backend/llvm/loop_engine.rs` | +20 | pthread_join at program exit, channel check in main loop |
| `src/backend/llvm/emit_declares.rs` | +10 | pthread_create/join/nanosleep declare statements |
| `src/interpreter.rs` | +200 | Thread spawning, channel-based output sync, termination |
| `src/interpreter.rs` (test) | +50 | Threading E2E test |

---

## 5. Open Questions

1. **Thread count limits**: If a program has 100 persistent cells, should we spawn 100 threads? Or use a thread pool (one thread per CPU core, round-robin cell assignment)?
   - **Recommendation**: Start with 1 thread per cell. Thread pools can be added later as an optimization.

2. **Memory ordering**: `seq_cst` is safest but slowest. Can we use `acquire`/`release` for the channel protocol?
   - **Recommendation**: Start with `seq_cst` for correctness, optimize to `acquire`/`release` after benchmarking.

3. **Cell-initiated vs parent-initiated termination**: Which takes priority?
   - **Recommendation**: Parent-initiated (program exit) should always override cell `term!`. If the program exits, all cells die regardless of their internal state.

4. **Debugging**: How should the LLVM backend handle a cell thread that panics/crashes?
   - **Recommendation**: Wrap the cell tick function in a signal handler. On crash, set the channel dirty flag to a sentinel value (e.g., `i64 -1`) and let the main loop detect the failure.
