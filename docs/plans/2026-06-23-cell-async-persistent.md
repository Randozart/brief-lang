# True Async Persistent Cells — `cell!` with Independent Ticking

**Date**: 2026-06-23  
**Status**: specification  
**Depends on**: Phase 1–3 cell primitive (committed)

---

## 1. Core Idea

The current `cell!` persistent cell preserves state across calls but only runs when the parent explicitly invokes a `CellCall`. **True async** means the persistent cell ticks independently on the main reactor loop — it fires its internal transactions every tick (or at a configured rate) without the parent needing to call it. The parent reads the latest output via `trg @` bindings, and the output only propagates when it changes between ticks.

```briv
// Current (sync persistent — parent must call):
let r = cell! counter();  // tick happens NOW, returns current value

// Desired (true async — ticks independently):
// Register on boot:
trg count: Int @ counter();          // ticks every main loop iteration at @1kHz
trg smoothed: Int @ filter(raw).out; // ticks at @10kHz, filters sensor input
```

### Why This Matters

Without true async, `cell!` is indistinguishable from `cell` except for state persistence. The motivating use case — `Console!` reading stdin, `Timer!` counting independently, sensor fusion pipelines — all require the cell to tick on its own schedule, not the caller's schedule.

---

## 2. Syntax

### `@Hz` syntax for reactor speed

After a cell call in a trigger binding, an optional `@Hz` annotation sets the tick rate:

```briv
trg count: Int @ counter() @1kHz;
trg smoothed: Int @ filter(raw).out @10kHz;
```

Supported units:
- `@NHz` — N ticks/second (e.g. `@1kHz` = 1000 ticks/s, `@10MHz` = 10M ticks/s)
- `@1Hz` — once per second (minimum for reactor-based tick)
- Default (no annotation): tick every main loop iteration (same as `node`)

The `@Hz` annotation is **informational** in the interpreter (all cells tick every iteration) and **binding** in the LLVM backend (determines whether the cell function is inlined or called periodically).

### Registration syntax

Persistent cells can also be registered at top level:

```briv
cell! timer(duration: Int) -> elapsed: Int @1kHz;

defn main() -> Int {
    trg e: Int @ timer.elapsed;
    // ...
};
```

The top-level `cell!` declaration registers a persistent instance with the given initial arguments (from state defaults or `#!init` pragma).

---

## 3. Architecture

### 3.1 Interpreter — Cell Registry

Add to `Interpreter` struct:

```rust
/// Registered persistent cell instances. Keyed by instance name.
pub persistent_cells: HashMap<String, PersistentCellInstance>,

pub struct PersistentCellInstance {
    pub cell_def: CellDef,
    pub state: HashMap<String, Value>,       // cell's private state (`cell$name$field`)
    pub prior_state: HashMap<String, Value>,  // for postcondition comparison
    pub output_cache: HashMap<String, Value>, // last-known output per port name
    pub tick_hz: Option<u64>,                // from @Hz annotation, None = every tick
    pub tick_counter: u64,                   // current tick count (for rate limiting)
    pub tick_interval: u64,                  // ticks between firings (derived from Hz)
    pub outputs_changed: bool,               // true if any output changed this tick
}
```

### 3.2 Registration Flow

Persistent cells are registered via two paths:

**Path A: `trg name: Type @ cell!(args)` at statement level**
```
1. Parse `trg X: Int @ counter() @1kHz`
2. Typechecker resolves `counter` to a CellDef
3. At registration time (desugaring or first interpretation):
   - Create PersistentCellInstance with fresh state
   - Initialize fields from defaults
   - Store tick_hz from @ annotation
   - Register in persistent_cells map
   - Bind output port name "X" to the cell's first output
```

**Path B: Top-level `cell!` declaration with state**
```
1. Parse `cell! timer(duration: Int) -> elapsed: Int @1kHz { ... }`
2. Registration creates the instance immediately
3. Initial arguments come from defaults or `#!init` pragma
```

### 3.3 Main Loop Ticking (`Interpreter::run()`)

```rust
// In the main reactive loop, after processing all program transactions:
for (name, instance) in &mut self.persistent_cells {
    instance.tick_counter += 1;
    
    // Check if this cell should tick this iteration (rate limiting)
    if instance.tick_interval > 0 && instance.tick_counter % instance.tick_interval != 0 {
        continue;
    }
    
    // Run one convergence pass on the cell's state
    let changed = self.tick_persistent_cell(instance);
    
    if changed {
        instance.outputs_changed = true;
        // Copy new output values to parent's state under trigger names
        self.sync_persistent_outputs(name, instance);
    }
}
```

### 3.4 Single Tick Function

```rust
fn tick_persistent_cell(&mut self, instance: &mut PersistentCellInstance) -> bool {
    let mut any_fired = false;
    
    for txn in &instance.cell_def.transactions {
        let pre = self.rewrite_identifiers(&txn.contract.pre_condition, 0, &instance.cell_def.name);
        let pre_val = self.eval_expr_in_state(&pre, &instance.state)?;
        
        if pre_val == Value::Bool(true) {
            instance.prior_state = instance.state.clone();
            self.eval_stmts_in_state(&txn.body, &mut instance.state, &instance.cell_def.name);
            
            let post = self.rewrite_identifiers(&txn.contract.post_condition, 0, &instance.cell_def.name);
            let post_val = self.eval_expr_in_state(&post, &instance.state)?;
            if post_val == Value::Bool(true) && instance.state != instance.prior_state {
                any_fired = true;
            }
        }
    }
    
    any_fired
}
```

Key detail: `eval_expr_in_state` and `eval_stmts_in_state` operate on the cell's private state, not the parent's. The cell's identifiers are rewritten with `cell$name$` prefix (uid=0 for persistent cells), so they access the cell's state HashMap.

### 3.5 Output Sync

After each tick, compare the cell's output port values against the cache. Copy changed values to the parent's state:

```rust
fn sync_persistent_outputs(&mut self, name: &str, instance: &PersistentCellInstance) {
    let names = extract_output_names(&instance.cell_def.output_type);
    for port_name in names {
        let key = format!("{}${}.{}", instance.cell_def.name, 0, port_name);
        let new_val = instance.state.get(&key).cloned().unwrap_or(Value::Void);
        let old_val = instance.output_cache.get(&port_name);
        
        if Some(&new_val) != old_val {
            // Output changed — store in parent's state under trigger name
            // The trigger name is looked up from the trg binding registry
            for (trg_name, bound_port) in &self.trg_bindings {
                if bound_port.cell_name == name && bound_port.port_name == port_name {
                    self.state.insert(trg_name.clone(), new_val.clone());
                }
            }
            instance.output_cache.insert(port_name, new_val);
        }
    }
}
```

### 3.6 `term!` Inside Persistent Cells

When a persistent cell's transaction executes `term!`:
- The cell terminates: remove from `persistent_cells` map
- The program continues normally (the cell's output just stops updating)
- To terminate the program: the cell must call `term!` on the parent's behalf, or set an exit condition

This is consistent with the principle that `term!` inside a cell terminates the **cell**, not the program.

---

## 4. Implementation Plan

### Step 1: Interpreter — Helper Methods (days 1–2)

**Files**: `src/interpreter.rs`

1. Add `PersistentCellInstance` struct and `persistent_cells` field
2. Add `eval_expr_in_state(expr, state) -> Result<Value, RuntimeError>` — evaluates an expression against a specific state HashMap instead of `self.state`
3. Add `eval_stmts_in_state(stmts, state, cell_name) -> Result<()>` — executes statements against specific state
4. Add `tick_persistent_cell(instance) -> Result<bool>` — single convergence pass on cell state
5. Add `sync_persistent_outputs(name, instance)` — copy changed outputs to parent state

**Estimated**: 150 lines

### Step 2: Interpreter — Registration (day 2–3)

**Files**: `src/interpreter.rs`, `src/parser.rs`, `src/typechecker.rs`

1. In `run()`: after processing all program TopLevel items, register top-level `cell!` declarations
2. In `exec_stmt` for `TrgBinding`: if instance calls a `cell!`, register as persistent instead of sync call
3. Parser: parse optional `@Hz` suffix on instance expression in `trg @` bindings
4. Typechecker: validate that `@Hz` is only used with `cell!` definitions

**Estimated**: 100 lines

### Step 3: Interpreter — Main Loop Integration (day 3–4)

**Files**: `src/interpreter.rs`

1. In `run()` reactive loop: after each main tick iteration, iterate `persistent_cells` and tick each one
2. Handle rate limiting (tick_interval)
3. Modify the stasis detection in `run()`: the program reaches stasis when NO program transaction fires AND no persistent cell output changes
4. When a persistent cell exits via `term!`: clean up its registration, optionally set `trg` to a sentinel value

**Estimated**: 100 lines

### Step 4: LLVM Backend — Tick Functions (days 4–6)

**Files**: `src/backend/llvm/emit_toplevel.rs`, `emit_expr.rs`, `mod.rs`

1. For each persistent cell, emit a `define void @cell_name_tick(ptr %State)` function that:
   - Loads the cell's fields from their `cell$name$` prefixed %State slots
   - Evaluates all transaction preconditions
   - Fires matching transactions
   - Stores updated field values back to %State
   - Returns a flag indicating whether output changed
2. In the main `@main` function's reactive loop: call each persistent cell's tick function
3. Use the `tick_interval` to gate tick function calls (skip every N iterations based on Hz)
4. After tick, compare output slot values and update trigger variables

**Estimated**: 400 lines

### Step 5: Tests (day 6–7)

**Files**: `src/interpreter.rs` test module, `src/backend/llvm/tests.rs`

1. Interpreter test: register persistent cell, run main loop, verify cell ticks independently
2. Interpreter test: verify output sync — trg binding captures changed outputs
3. Interpreter test: verify rate limiting — cell with @1kHz only ticks every N iterations
4. Interpreter test: verify term! inside persistent cell terminates the cell (not the program)
5. LLVM test: verify tick function is emitted for persistent cells

**Estimated**: 150 lines

---

## 5. Edge Cases

### 5.1 Multiple Calls to the Same Cell

```briv
trg a: Int @ counter();
let x = cell counter();  // sync call to the SAME persistent cell?
```

**Decision**: A persistent cell is a singleton identified by name. Multiple `trg @ cell_name()` bindings to the same cell read the same instance's outputs. A sync `cell cell_name()` call to a persistent cell should error at compile time — you cannot synchronously call a persistent cell. Use `trg @` instead.

### 5.2 Persistence Across Program Restarts

Persistent cell state lives for the duration of the program. Not persisted to disk (that's a separate feature — state serialization).

### 5.3 Cell-Cell Communication

If cell A needs to read cell B's output:
- Cell B's output must be a `trg` in the parent
- Cell A must receive it as an input argument (set at registration or updated via parent)

Direct cell-to-cell wiring is Phase 4 (deferred).

### 5.4 `@Hz` Without Trigger Binding

```briv
cell! timer(duration: Int) -> elapsed: Int @1kHz;
```

The `@Hz` applies to the cell's tick rate even without a `trg @` binding. The cell ticks at 1kHz regardless. Its output updates are available to any `trg @ timer.elapsed` bindings.

### 5.5 Stasis vs Persistent Ticking

A persistent cell **always ticks** — it does not converge to stasis like a transient cell. The convergence loop is ONE pass per tick (not repeated until stasis). This means:
- The cell fires exactly the set of transactions whose preconditions are true on this tick
- Those that fire run once and check their postconditions
- The cell does NOT loop until stasis
- Next tick: evaluate all preconditions again with the updated state

This is the key difference between `cell` (converge to stasis in one call) and `cell!` (tick each iteration, one pass per tick).

---

## 6. File-by-File Change Summary

| File | Lines | What |
|------|-------|------|
| `src/interpreter.rs` | +300 | PersistentCellInstance struct, registration, tick loop, output sync, eval helpers |
| `src/parser.rs` | +15 | `@Hz` suffix parsing in `trg @` bindings |
| `src/typechecker.rs` | +10 | Validate `@Hz` only on `cell!`, sync call to persistent cell is error |
| `src/ast.rs` | +5 | Optional `tick_hz: Option<u64>` on `CellDef` or `TrgBinding` |
| `src/backend/llvm/emit_toplevel.rs` | +200 | Emit `@cell_name_tick` functions |
| `src/backend/llvm/emit_stmt.rs` | +50 | TrgBinding emits tick function call in main loop |
| `src/backend/llvm/tests.rs` | +60 | LLVM IR tests for persistent cell tick |

---

## 7. Questions for Implementation

1. Should `@Hz` be stored on `CellDef` (persistent attribute) or `Statement::TrgBinding` (per-binding modifier)?
   - **Recommendation**: Both. `CellDef.tick_hz` for the cell's natural rate, `TrgBinding.tick_hz` for per-binding override.
2. Should the interpreter's main loop stasis detection account for persistent cells? I.e., should the program stay alive as long as any persistent cell is still firing?
   - **Recommendation**: Yes. The program stays alive while any persistent cell has fired since the last stasis check. Only exit when all cells are quiet AND the main program is at stasis.
3. Thread safety: Since `cell!` ticks happen in the main reactor loop, they're single-threaded by default. True parallelism (separate thread per cell) is deferred to a future phase.
