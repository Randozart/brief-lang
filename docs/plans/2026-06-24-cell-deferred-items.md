# Deferred Cell Items — Sub-State GEP + Cell-to-Cell Communication

**Date**: 2026-06-24  
**Status**: specification  

---

## 1. Sub-State GEP Optimization

### Current Approach

Persistent cell fields are stored as flat prefixed slots in `%State`:

```
%State = type { i8, i8, i64, i64, i64, ... }
                ^      ^     ^     ^
                |      |     |     cell$adder$b
                |      |     cell$adder$sum
                |      cell$adder$done
                boot_done
```

The `cell$adder$done` field at index 1 is accessed via `getelementptr %State, ptr %state, i32 0, i32 1`. This works but means `%State` grows linearly with the number of cell definitions, and cell field accesses go through the same GEP path as program state fields — no type-level distinction.

### Goal

Emit a separate `%CellState.adder = type { i8, i64, i64, i64 }` struct for each cell. The cell's tick function operates on its own `%CellState*` instead of `%State*`. This:

- **Isolates cell memory** — cell fields are in a separate allocation, not interleaved with program state
- **Enables threading** — each cell thread owns its `%CellState` without sharing `%State`
- **Improves alias analysis** — LLVM can prove GEPs on different `%CellState` pointers don't alias each other or `%State`

### Design

**`build_field_index` in `mod.rs`:**

```rust
// Instead of:
let prefixed = format!("cell${}${}", cell.name, field.name);
self.field_index_map.insert(prefixed, self.field_types.len());

// Emit a separate struct:
writeln!(out, "%CellState.{} = type {{ {} }}", cell.name, field_types.join(", ")).ok();
```

**`emit_cell_thread` in `emit_toplevel.rs`:**

```rust
// Instead of receiving a %State*:
define i8* @cell_thread_counter(ptr %state) { ... }

// Receive a %CellState.counter*:
define i8* @cell_thread_counter(ptr %cell_state) {
  %cs = bitcast ptr %cell_state to ptr
  %val = getelementptr %CellState.counter, ptr %cs, i32 0, i32 0
  store i64 %new_val, ptr %val
}
```

**Allocation in `emit_main` / `emit_ssa_main`:**

```rust
// Instead of:
%cell_state_counter = alloca %State

// Allocate the cell's own struct:
%cell_state_counter = alloca %CellState.counter
```

**Channel globals stay the same** — `@chan_val_<cell>_<port>` and `@chan_dirty_<cell>` are independent of the struct layout.

### Impact

| File | Lines | Change |
|------|-------|--------|
| `src/backend/llvm/mod.rs` | +30 | `build_field_index` emits `%CellState.*` types; skips persistent cell fields in `%State` |
| `src/backend/llvm/emit_toplevel.rs` | +20 | `emit_cell_thread` uses `%CellState.cellName*` instead of `%State*` |
| `src/backend/llvm/emit_expr.rs` | +10 | CellCall codegen uses separate access path for persistent cells |
| `src/backend/llvm/loop_engine.rs` | +10 | Main loop allocates `%CellState.*` instead of `%State` for cell threads |

Total: ~70 lines.

### Risk

The CellCall convergence loop codegen (emit_expr.rs) uses `self.state_reg_name` and `self.field_index_map` for GEPs. If the cell state is a separate type, the codegen needs a different GEP path. The current flat approach works correctly and is proven. This optimization is purely structural — no correctness benefit, only performance and alias-analysis improvements.

---

## 2. Cell-to-Cell Communication (Phase 4)

### Goal

Allow one cell's output port to feed into another cell's input argument without parent mediation:

```brief
cell! filter(input: Int) -> out: Int { ... };
cell! smoother(input: Int) -> out: Int { ... };

// Wire: filter.out → smoother.input
trg smoothed: Int @ smoother(filter.out).out;
```

### Design

**Parser**: The expression `smoother(filter.out)` is parsed as `Call("smoother", [FieldAccess(Identifier("filter"), "out")])`. The typechecker resolves this to a cell-to-cell wire if `filter` is a cell and `out` is its output port.

**Typechecker**: When a `TrgBinding` instance contains an expression referencing another cell's output port, validate that:
- The source cell (`filter`) exists and is a `cell!`
- The port name (`out`) exists on the source cell
- The source port type matches the target param type
- No circular dependencies exist (self-loop, A→B→A)

**Dataflow**: During registration, establish a static wiring graph:
```rust
struct CellWire {
    from_cell: String,
    from_port: String,
    to_cell: String,
    to_param: usize,
}
```

At runtime, after each tick of `filter`, propagate its `out` value to `smoother`'s input param. Both cells are registered persistent instances; the wire just copies the output value from one instance's state to the other's param slot.

**Interpreter**: In `tick_persistent_cells`, after sync'ing a cell's outputs, check the wire table. If any wire originates from this cell, copy the output value to the target cell's state under the parameter key:

```rust
for wire in &self.cell_wires {
    if wire.from_cell == cell_name {
        let src_key = format!("{}${}.{}", wire.from_cell, 0, wire.from_port);
        let dst_key = format!("{}${}.{}", wire.to_cell, 0, cell_def.parameters[wire.to_param].0);
        if let Some(val) = cell_state.get(&src_key) {
            if let Some(target) = self.persistent_cells.get_mut(&wire.to_cell) {
                target.state.insert(dst_key, val.clone());
            }
        }
    }
}
```

**LLVM Backend**: In `@cell_persistent_ticks` or the main loop, after each cell's convergence pass, emit GEP+load from the source cell's output slot, then GEP+store to the target cell's param slot. Since both are `%State` slots in the current flat scheme, this is straightforward.

### Impact

| File | Lines | Change |
|------|-------|--------|
| `src/typechecker.rs` | +50 | Validate cell wires, check circular deps, infer types |
| `src/interpreter.rs` | +30 | Wire propagation in `tick_persistent_cells`, `cell_wires` registry |
| `src/parser.rs` | +10 | Accept `cell(param.field)` syntax in `trg @` expression |
| `src/backend/llvm/dispatch.rs` | +20 | Wire propagation in `@reactor_tick` after cell ticks |

Total: ~110 lines.

### Open Questions

1. **Wire ordering**: If A→B and B→C, should A's update propagate to C in the same tick? (Transitive propagation) Or should C see B's previous value until next tick? (One-hop per tick)
   - **Recommendation**: One-hop per tick. Simpler, avoids iteration-ordering dependencies.

2. **Partial wiring**: Can a cell with 3 input params receive 2 from wires and 1 from a `trg @` literal? Yes — unwired params use their default values.

3. **Dynamic rewiring**: Can wires change at runtime? No — Phase 4 is static wiring only. Dynamic rewiring is Phase 5+.

---

## 3. CIRCT Transaction Body Synthesis

### Current State

The CIRCT backend emits `hw.instance` for `Expr::CellCall` and registers cell fields as `seq.firreg` registers. What's missing is full synthesis of the cell's transaction bodies — the `node` blocks inside the cell definition should emit `comb.and`/`comb.add`/etc. logic rather than being opaque placeholders.

### Remaining Work

| Item | Priority | Status |
|------|----------|--------|
| `Expr::IntrinsicCall` → `comb` operations | Medium | Not started |
| Cell state fields → `seq.firreg` with reset values | Medium | Partially done |
| `Statement::Assignment` → combinational wiring | Medium | Not started |
| `Contract` precondition → `assert` or `when` guard | Low | Not started |

---

## 4. Implementation Order

1. **Sub-state GEP** — smallest change, purely structural, unblocks cleaner threading
2. **Cell-to-cell communication** — adds new capability, ~110 lines
3. **CIRCT body synthesis** — fills in the remaining CIRCT stubs, requires understanding of `comb` and `seq` MLIR ops

Each item is independent and can be tackled in any order.
