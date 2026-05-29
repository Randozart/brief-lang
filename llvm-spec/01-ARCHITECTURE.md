# Architecture: AST → Lowering → LLVM IR

## Pipeline

```
Brief Source (.bv)
    ↓
Lexer → Parser
    ↓
Type Checker
    ↓
Proof Engine (symbolic execution, contract verification)
    ↓
Backend Analysis (call graph, parameter ranges)
    ↓
Lowering Pass (AST → lowered IR — constant folding, guard simplification)
    ↓
LLVM IR Generator
    ├── Module header (target triple, datalayout)
    ├── %State type (all rstruct fields flattened)
    ├── Transaction functions (define void @txn_name(%State* noalias nocapture))
    ├── Definition functions (define i64 @defn_name(i64 %arg0, ...))
    ├── Reactor loop (main → tick, trigger sampling, dispatch)
    └── FFI declarations (declare i64 @strlen(i8*))
    ↓
LLVM IR (.ll file)
```

## Key Design Decisions

1. **Text IR emission** — The backend emits `.ll` text, not LLVM bitcode. This makes debugging trivial (`llc -O3 file.ll` produces the final binary) and avoids the `inkwell` dependency.

2. **Single `%State` type** — All program state is flattened into one LLVM struct. Each `rstruct` field becomes a member. Transactions receive `%State*` and access their fields by GEP.

3. **Acyclic inlining** — If the call graph has no cycles, the backend emits all transaction bodies inline in the reactor loop. LLVM's inliner then sees the entire state machine as one SSA graph, maximizing optimization surface.

4. **`noalias` as default** — Every `%State*` parameter is `noalias` + `nocapture`. Brief's memory model guarantees no pointer aliasing, so this is always safe.

## Lowering Pass

Before LLVM emission, the AST runs through the peephole optimizer (`src/backend/mod.rs`):

- **Constant folding**: `3 + 4` → `7`, `true && false` → `false`
- **Redundant elimination**: `let x = y; let z = x;` → `let z = y;`
- **Guard simplification**: `[true] { ... }` → inline the block, `[false] { ... }` → delete it
- **Select conversion**: `[cond] { x = a }; ~[cond] { x = b };` → `x = select cond, a, b`
- **Transition fusing**: If `Txn_A`'s postcondition implies `Txn_B`'s precondition, and no async `trg` can preempt, fuse their bodies into a single atomic transition (see `08b-TRANSITION-FUSING.md`)
- **Trigger sampling**: All volatile `trg` variables are sampled once at tick entry into immutable SSA registers, enforcing deterministic execution across the tick (see `08a-TRIGGERS.md`)

The lowered AST is then passed to the LLVM IR generator.