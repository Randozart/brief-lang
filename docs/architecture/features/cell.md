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

### LLVM Backend (Phase 1 Stubs)

`src/backend/llvm/`: All three backend points are stubs for Phase 1:

| File | Addition | Behavior |
|------|----------|----------|
| `mod.rs` | `cell_defs: HashMap<String, CellDef>` | Registration + string collection |
| `emit_expr.rs` | `Expr::CellCall(_, _)` arm | `call void @llvm.trap(); unreachable` |
| `emit_stmt.rs` | `Statement::TrgBinding { name, .. }` arm | Comment-only `; trg @ <name>` |

Cell-using programs work in the interpreter (test mode) but crash at runtime when LLVM-compiled. Real LLVM codegen (prefixed state functions, convergence loop in IR) is Phase 2.

### Tests

| File | Tests | What it covers |
|------|-------|----------------|
| `src/parser.rs` | 3 | Parse `cell`, `cell!`, no outputs |
| `src/interpreter.rs` | 4 | Simple arithmetic, multiple fields, loop convergence, `term!` early exit |

Run with `cargo test --lib`.

### Known Gaps and Future Work

| Item | Priority | Status |
|------|----------|--------|
| `cell!` persistent (async) | High | Not started |
| `trg @` trigger binding (full) | High | Stub only |
| LLVM convergence loop codegen | High | Stub only |
| CIRCT hardware synthesis | Medium | Not started |
| Cell isolation check in typechecker (no parent state refs) | Medium | Not implemented |
| Sub-state GEP optimization (Phase 2) | Low | Not started |
| `cell` keyword in expression context (`let x = cell timer(1000)`) | Low | Parser emits `Expr::CellCall` on explicit `cell` keyword prefix |
| Multi-output named ports | Low | Parser + interpreter handle via `OutputType::Named` |
| In-cell `defn` with output ports | Low | Parser parses, interpreter needs to handle defn calls within cell scope |
