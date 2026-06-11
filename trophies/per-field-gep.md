## Per-Field GEP Loop Codegen

**What**: Replaced the monolithic `load %State`, `extractvalue`, `insertvalue`,
`store %State` pattern with direct per-field GEP loads and stores in the LLVM
backend's reactor tick codegen.

**Why it matters**: The old pattern forced LLVM to optimize through an entire
typed struct every tick, which blocked SROA promotion for most fields. With
per-field GEP access, each state variable becomes an independent SSA value,
enabling LLVM to promote hot fields to registers (phi nodes) and fold cold
fields away entirely.

**How**: The field index map (`self.field_index_map`) is built during
`emit_declares` for each `StateDecl`. Codegen emits `getelementptr` for each
read and write operation, keyed by the field name. The `emit_expr` handler for
`Expr::Identifier` and `Expr::OwnedRef` uses the index map to emit a GEP +
load. Arrow operations (`<-`, `=` on fields) also GEP-direct the target.

**Before/After**: Hot-loop state access went from `load %State` + `extractvalue
i64, %state, 3` → `getelementptr %State, %State* %state, i32 0, i32 3` + `load
i64`. LLVM's SROA now promotes these to phi nodes in the tick loop.
