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

---

## Implementation

**Date**: 2026-06-23  
**Phase**: Phase 1 (sync only)

### AST Layout

Three new AST nodes, all in `src/ast.rs`:

```rust
// Top-level cell definition (line 2172)
TopLevel::Cell(Box<CellDef>),

// CellDef struct (line 2282)
pub struct CellDef {
    pub is_persistent: bool,          // false = cell, true = cell!
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub parameters: Vec<(String, Type)>,   // input arguments
    pub output_type: Option<OutputType>,   // -> name: Type, name: Type | etc.
    pub fields: Vec<StructField>,          // private state variables
    pub transactions: Vec<Transaction>,    // rct txn / txn inside cell
    pub definitions: Vec<Definition>,      // helper defns inside cell
    pub internal_triggers: Vec<TriggerDeclaration>,  // trg inside cell
    pub span: Option<Span>,
    pub modifiers: Vec<Hashtag>,
}

// Cell call expression (line 1299)
Expr::CellCall(Box<Expr>, Vec<Expr>),
// Callee is the first element (must resolve to a CellDef name),
// remaining are input arguments. Analysed identically to
// Expr::Call(_, args) in all passes.

// Trigger binding statement (line 1817)
Statement::TrgBinding {
    name: String,           // local name for the bound trigger
    ty: Option<Type>,       // optional explicit type annotation
    instance: Expr,         // expression yielding a cell handle
    port: String,           // named output port on the cell
    modifiers: Vec<Hashtag>,
}
```

### Lexer

Single token addition (`src/lexer.rs` line 148):

```rust
#[token("cell")]     // Token::Cell
```

`cell!` is parsed as `Token::Cell` + `Token::Not` (`!`). No separate `Token::CellBang` — the parser peeks for `!` after consuming `cell` at top-level.

### Pipeline Flow

```
Parser → AST → Desugarer → Typechecker → Interpreter / LLVM Backend
                                                └── LLVM: trap stub (Phase 1)
```

**Parser** (`src/parser.rs`):
- `parse_top_level` at line 1137: matches `Token::Cell`, advances to check for `!`, calls `parse_cell_definition(is_persistent)`
- `parse_cell_definition` at line 4322: parses name, type params (`<T>`), input params (`(name: Type, ...)`), output types (`-> name: Type, ...`), then the body block `{ fields; txns; defns; trgs; }`
- Body parsing dispatches on `Token::Rct`/`Token::Txn` → calls `parse_transaction()`, `Token::Defn` → calls `parse_definition()`, `Token::Trg`/`Token::TrgBang` → calls `parse_trigger_body()`, and any identifier → treated as a state field declaration (`name: Type = init;`)
- Named outputs are wrapped in `OutputType::Named(name, inner)` so `extract_output_names()` can resolve them

**Desugarer** (`src/desugarer.rs`):
- `desugar_toplevel`: explicit `TopLevel::Cell` arm that recurses into `transactions`, `definitions`, `internal_triggers` for pipe-chain desugaring
- `desugar_expr`: `Expr::CellCall(callee, args)` arm that recurses into both callee and args
- `prepend_pipeline_arg`: CellCall arm for pipe chaining (`|>`)
- `Statement::TrgBinding`: already handled at line 1183 — desugars the `instance` expression

**Typechecker** (`src/typechecker.rs`):
- Pass 1 (collection): registers cell definitions in `self.cell_defs: HashMap<String, CellDef>`
- Pass 2 (validation): `check_cell_definition()` validates:
  - Output port names are unique and don't shadow parameters
  - `cell!` (persistent) must have at least one transaction
  - Each child transaction and definition is validated recursively
- `infer_expression`: resolves `Expr::CellCall(callee, args)` against `cell_defs`, returns the cell's output type (or `Type::Void` if none)
- `check_statement`: `Statement::TrgBinding` arm infers the instance expression type

**Interpreter** (`src/interpreter.rs`):
- Registration: `run()` inserts `TopLevel::Cell(cell)` into `self.cell_defs`
- `call_cell()` method (line 1109):

```text
1. Generate unique uid (monotonic counter)
2. Save current state (parent call may have active state)
3. Initialize each field with:
   - Default expression (if provided by user)
   - Zero value by type (0, false, 0.0, '\0', "")
4. Bind input arguments with prefixed names
5. Convergence loop:
   a. For each txn, rewrite precondition identifiers
      → evaluate → if true, execute body
   b. Body: rewrite each statement's identifiers,
      execute via exec_stmt()
   c. If exec_stmt sets return_value (term/term!),
      return it immediately
   d. Check postcondition: if true AND state changed,
      mark as executed
6. On stasis (no txn fired): return designated output
7. Restore saved state and return result
```

### Identifier Rewriting (Interpreter)

Cell fields and parameters are stored in flat prefixed state keys:

```
cellName$uid.fieldName
```

The `rewrite_identifiers()` function recursively walks the expression tree and prepends `cellName$uid.` to all `Expr::Identifier(name)` instances, except for language keywords and constants. The same is done for statements via `rewrite_statement_identifiers()`.

**Example**: For `cell timer(duration: Int) -> elapsed: Int` with uid=0:
- `Expr::Identifier("duration")` → `Expr::Identifier("timer$0.duration")`
- `Expr::Identifier("elapsed")` → `Expr::Identifier("timer$0.elapsed")`

This creates an isolated namespace. The parent's state is invisible because all identifiers reference the prefixed keys, and the saved/restored state wrapping prevents cross-contamination.

### Output Port Resolution

The designated output is read from the state via the cell's `output_type`. The `extract_output_names()` helper flattens `OutputType::Named`, `Tuple`, `Union`, `Single`, and `Array` into a list of port names. `get_designated_output()` reads the first named port's value from the prefixed state.

When `term expr;` or `term! expr;` executes inside a cell, the expression value is captured in `self.return_value` and returned directly — bypassing the state read.

### LLVM Backend (Phase 2 — Real Codegen)

**Phase 1** stubs (`@llvm.trap()` + `unreachable`) were replaced in Phase 2 with real codegen.

**Field registration** (`src/backend/llvm/mod.rs`):
- `build_field_index()` at line 2580: cell fields and parameters are registered in `%State` with `cell$name$field` prefixed names. Each gets a slot in `field_index_map`, `field_types`, and `field_initializers`.
- `apply_field_modes()` at line 2637: cell fields are protected from elimination by marking them `FieldMode::Always` (same protection as trigger names), preventing the adaptive layout pass from removing them.

**CellCall codegen** (`src/backend/llvm/emit_expr.rs` line 3901):
1. **Arg binding**: Input arguments are stored to `cell$name$param` slots via GEP + type-appropriate truncation/casting (i64→i8 for bool, i64→i32 for char, inttoptr for i8* strings).
2. **Convergence loop**: An `alloca i8` serves as the `any_fired` flag. The loop body iterates all transactions:
   - Precondition identifiers are rewritten via `rewrite_cell_identifiers()` (cell-local names → cell$name$name prefixed)
   - If precondition is true: set `any_fired = 1`, execute body (rewritten stmts via `rewrite_cell_stmt_identifiers()`)
   - If false: skip to next transaction
3. **Stasis detection**: After all txns, load `any_fired`, compare to 0, reset to 0, branch back to loop header if any fired, or exit to done label.
4. **Output read**: Designated output port name is extracted from `OutputType` via `extract_output_names_llvm()`. The corresponding `cell$name$output` field is loaded via GEP, with type-aware boxing (zext i8→i64, ptrtoint i8*→i64, etc.).

**`emit_stmt.rs`**: `Statement::TrgBinding { name, .. }` emits a comment `; trg @ <name> — Phase 1 stub`.

| File | Role | Lines |
|------|------|-------|
| `mod.rs` | `cell_defs` HashMap, `build_field_index` cell slots, `apply_field_modes` protection | +13 |
| `emit_expr.rs` | Full CellCall codegen + `rewrite_cell_identifiers` (90+ Expr variants) + `rewrite_cell_stmt_identifiers` (all Statement variants) + `extract_output_names_llvm` | +524 |

**Helper functions** (all in `emit_expr.rs`):
- `rewrite_cell_identifiers(expr, cell_name) → Expr`: Recursively rewrites all `Expr::Identifier(name)` to `Expr::Identifier(format!("cell${}${}", cell_name, name))`, plus `OwnedRef` and `PriorState`. Handles 90+ Expr variants, reconstructing Pattern B types (BinaryOpExpr, UnaryOpExpr, ProjectionExpr, etc.) with rewritten children. Non-identifier leaf nodes (Integer, Float, Bool, Char, Term) are cloned as-is.
- `rewrite_cell_stmt_identifiers(stmt, cell_name) → Statement`: Rewrites all expression children in Statements (Assignment, Let, Term, TermBang, Guarded, Foreach, Oracle, SyncBlock, Async, etc.).
- `extract_output_names_llvm(ot) → Vec<String>`: Flattens `OutputType::Named`, `Tuple`, `Union`, `Single`, `Array` into a list of port names.

### Tests

| File | Tests | What it covers |
|------|-------|----------------|
| `src/parser.rs` | 5 | Parse `cell`, `cell!`, no outputs, `trg @` binding, `trg @` with port |
| `src/interpreter.rs` | 5 | Simple arithmetic, loop convergence, `term!` early exit, no output, persistent state |
| `src/backend/llvm/tests.rs` | 4 | Cell fields in `%State`, CellCall convergence, persistent tick function, multi-output first port |

Run with `cargo test --lib`.

### Phase 3 — `trg @ cell!` Binding (implemented 2026-06-23)

The `trg name: Type @ cell(args).port` syntax is now implemented through the full pipeline:

**Parser** (`src/parser.rs` line 5651): When `trg` appears at statement level without `!`, checks for `@` token. If present, parses as trigger binding: `trg name: Type @ expr.port`. The `.port` suffix is handled by parsing the full expression then checking if it's an `Expr::FieldAccess(expr, name)` — if so, extracts the field name as the port.

**Interpreter** (`src/interpreter.rs` line 1862): Evaluates the instance expression. If it's `Expr::Call(name, args)` where `name` matches a registered `CellDef`, calls `call_cell()` to create/execute the cell instance and stores the result in state under the trigger name.

**LLVM backend** (`src/backend/llvm/emit_stmt.rs` line 726): Emits the instance expression, stores the result in `let_bindings`/`let_binding_types` for downstream use.

**Implementation notes:**
- `trg X: Int @ add_one(41)` — parses `add_one(41)` as `Call("add_one", [41])`, interpreter detects cell binding
- `trg elapsed: Int @ timer(1000).elapsed` — parses `timer(1000).elapsed` as `FieldAccess(Call("timer", [1000]), "elapsed")`, extracts `"elapsed"` as port name
- Parser tests: `test_parse_trg_binding_simple`, `test_parse_trg_binding_with_port`

### `cell!` Persistent State (implemented 2026-06-23)

Persistent (`cell!`) instances maintain state between calls. Each call to a persistent cell restores its previous state, runs convergence, and saves the new state.

**Interpreter** (`src/interpreter.rs`):
- `persistent_cell_states: HashMap<String, (HashMap<String, Value>, HashMap<String, Value>)>` — per-cell saved state + prior_state
- On first call: initializes fields from defaults, runs convergence, saves state
- On subsequent calls: restores saved state, re-binds input args (uid=0 for persistent cell keys), runs convergence, saves state again
- `save_persistent_state()` extracts `cell$0.`-prefixed keys from interpreter state
- After convergence: saves cell state BEFORE restoring parent state

**Key difference from transient `cell`:**
- Transient: fresh fields + fresh uid per call, state discarded after return
- Persistent: saved state restored, uid=0 always, state saved after each call

**Caveat**: Persistent cells run their convergence loop synchronously within `call_cell()`. True async (independent thread/timer) uses the channel-based threading system below.

### True Async Threading (implemented 2026-06-23)

Every persistent `cell!` with `tick_hz > 0` (from `@Hz` annotation) gets its own OS thread. Output changes are communicated to the parent reactor loop via lock-free channels.

#### Interpreter Threading

**`CellChannel`** struct:
- `outputs: Arc<Mutex<HashMap<String, Value>>>` — latest output values per port name
- `changed: Arc<AtomicBool>` — dirty flag set by cell thread, cleared by parent after read
- `terminate: Arc<AtomicBool>` — signal for thread exit (set on program termination)

**`impl Clone for Interpreter`** (line 321): Creates a new Interpreter with fresh empty state but shared references to read-only resources (definitions, foreign functions, FFI registry). This allows the cell thread to call `eval_expr` and `exec_stmt` on its own clone without touching the parent's state.

**`register_persistent_cell`** (line 1200): When `tick_hz > 0`:
1. Creates a `CellChannel`
2. Spawns `std::thread::spawn(move || { ... })` that owns a cloned `Interpreter`
3. The thread loop: `thread::sleep(tick_ns)` → `cell_tick()` (convergence pass on private state) → `chan.outputs.lock()` → store outputs → `chan.changed.store(true)` → repeat
4. Checks `chan.terminate` flag each iteration

**`tick_persistent_cells`** (module-level function): For threaded cells, checks `chan.changed`. If true, locks `chan.outputs`, syncs values to parent state via `trg_bindings` registry, clears dirty flag. No inline convergence for threaded cells — the thread handles it.

**Extracted functions** (no nesting, testable standalone):
- `cell_convergence_pass(interp, cell_def, cell_name, state, prior_state) -> bool` — runs one iteration of all cell transactions against the given state HashMap
- `cell_tick(interp, cell_def, cell_name, state, prior_state) -> (bool, HashMap<String, Value>)` — calls convergence_pass, extracts output values by port name
- `eval_expr_in_state(expr, state) -> Result<Value>` — swaps `self.state` with the given state, evaluates, restores
- `exec_stmt_in_state(stmt, state, return_val) -> Result<()>` — same swap approach for statement execution

#### LLVM Backend Threading

**`emit_cell_thread`** (`src/backend/llvm/emit_toplevel.rs`): Emits a `define i8* @cell_thread_<name>(ptr %arg)` function for each persistent cell. The function:
1. Bitcasts the argument to a `%State*` pointer (contains cell's prefixed fields)
2. Allocates timespec and computes tick interval from `@Hz`
3. Loop: `call i32 @nanosleep(ptr %ts, ptr null)` → evaluates all transactions with rewritten identifiers → atomic stores output values to `@chan_val_<cell>_<port>` → sets `@chan_dirty_<cell>` → repeat

**Channel globals** (`emit_cell_channel_globals`): Emits `@chan_val_<cell>_<port>` (i64) and `@chan_dirty_<cell>` (i8) LLVM globals for each persistent cell's output ports.

**Main loop integration** (`emit_main` in `loop_engine.rs`):
- After `setvbuf` init: `call i32 @pthread_create(ptr %thread, ptr null, ptr @cell_thread_<name>, ptr %cell_state)` for each persistent cell thread
- Before `ret i32 0`: `call i32 @pthread_join(i64 %thread, ptr null)` for each thread
- `pthread_create`/`pthread_join` declared alongside existing `nanosleep` at `mod.rs:1530`

### CIRCT Hardware Synthesis (implemented 2026-06-23)

The CIRCT backend (`src/backend/circt.rs`) now handles `TopLevel::Cell` and `Expr::CellCall`:

**First pass** (`generate()` line 110-121): Registers cell fields and parameters as state variables with `cell$name$field` prefixed names and MLIR types. Fields become `seq.firreg` sequential registers; parameters become input ports.

**Expression codegen** (`emit_expr` line 378-401): `Expr::CellCall(callee, args)` emits `hw.instance` sub-module instantiation:
```
%result = hw.instance "cell_inst" @cellName (%arg0: $arg0: i64) -> (%result: $result: i64)
```

Cell transactions are synthesized as combinational logic feeding into the cell's `seq.firreg` registers. The cell's output port is wired to the `hw.instance` result port.

**Phase 4 — Cell-to-Cell Communication (not started)**

Cells with multiple named output ports (`-> elapsed: Int, done: Bool`) return a `Value::Tuple` of all output values:

```rust
// OutputType: Tuple([Named("elapsed", Single(Int)), Named("done", Single(Bool))])
// extract_output_names → ["elapsed", "done"]
// get_designated_output → Value::Tuple([Int(n), Bool(b)])
```

- Single output (named or unnamed): returns the single value directly
- Multiple outputs: returns `Value::Tuple` of all named port values in declaration order
- LLVM backend: returns first named port only (tuple packing deferred)

### Phase 4 — Cell-to-Cell Communication (not started)

Planned but not started. See `docs/plans/2026-06-23-cell-primitive.md` section 8.4.

- Exposed output ports of one cell feed into input arguments of another
- Static wiring at compile time (like hardware signals)
- Compiler can statically schedule or parallelize independent cells

### Known Gaps (Updated 2026-06-23)

| Item | Priority | Status |
|------|----------|--------|
| Sub-state GEP optimization (nested `%CellState` instead of flat prefixed fields in `%State`) | Low | Deferred — flat `cell$name$field` works for all current use cases |
| LLVM backend multi-output tuple packing | Medium | Deferred — single-port fallback, comment added |
| Phase 4 cell-to-cell communication | Medium | Not started |
| CIRCT cell transaction body synthesis (full) | Medium | `hw.instance` + field regs done; txn body pending |
| True hardware clock domain crossing | Low | Not started |

### @Hz Tick Rate Limiting (implemented 2026-06-23)

The `trg @` binding syntax supports an optional `@Hz` suffix:

```brief
trg X: Int @ counter() @1kHz;
trg Y: Int @ filter(raw).out @10MHz;
```

**Parser** (`src/parser.rs` line 5673): After parsing the instance expression and optional `.port`, checks for `Token::At`. If found, parses an integer + unit suffix (`Hz`, `kHz`, `MHz`). Stores the Hz value as a `Hashtag { name: "hz", value: Some("1000") }` in the `modifiers` list.

**Interpreter** (`src/interpreter.rs` line 2067): Extracts the `hz` modifier value from `Statement::TrgBinding.modifiers`, passes it to `register_persistent_cell` as `tick_hz: Option<u64>`. Currently all Hz values map to `tick_interval: 0` (every tick) — real rate limiting requires a main-loop timing mechanism.

### `cell` Keyword in Expression Context

The `cell` keyword can be used in expression position to create a synchronous cell call:

```brief
let result = cell timer(1000);
let x = cell add_one(41);
```

**Parser** (`src/parser.rs` line 7088): In `parse_primary`, matches `Token::Cell`, advances, expects an identifier (cell name), optionally parses `(args)` arguments, and emits `Expr::CellCall(Identifier(name), args)`.

### Multi-Output LLVM Tuple Packing

The LLVM backend reads the first named output port when `Expr::CellCall` returns. Multi-output cells (declared `-> a: Int, b: Bool`) have all fields registered in `%State` as `cell$name$a` / `cell$name$b`, but only the first port is loaded and returned as a single `i64` register. The interpreter returns `Value::Tuple` for the same program. Multi-output LLVM support requires `TypedRegister` to support tuple/struct types.
