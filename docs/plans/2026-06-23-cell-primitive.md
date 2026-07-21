# `cell` — Cybernetic Cell Primitive

**Date**: 2026-06-23  
**Author**: Brief Compiler Architecture  
**Status**: Plan (ready for implementation)  

---

## Table of Contents

1. [Core Concept](#1-core-concept)
2. [Full Specification](#2-full-specification)
3. [Syntax Reference](#3-syntax-reference)
4. [Desugaring Model](#4-desugaring-model)
5. [Implementation Plan — Phase 1](#5-implementation-plan--phase-1)
6. [File-by-File Implementation Guide](#6-file-by-file-implementation-guide)
7. [Test Plan](#7-test-plan)
8. [Future Phases](#8-future-phases)
9. [Edge Cases and Gotchas](#9-edge-cases-and-gotchas)
10. [Motivating Use Case: officina-cli Console Component](#10-motivating-use-case-officina-cli-console-component)
11. [Error Messages](#11-error-messages)

---

## 1. Core Concept

A `cell` (component) is an **intentionally isolated Brief-in-Brief state space**. It wraps a complete Brief program (private state, reactive transactions, triggers) into a sealed box with a well-defined output interface. The outside world interacts only through:

- **Input arguments**: ephemeral values passed per invocation
- **Output ports**: named variables declared in the return type, continuously updated and trigger-able
- **Sync return**: `cell` (auto-terminating) returns on stasis or `term!`
- **Trigger binding**: `cell!` (persistent) output ports can be bound to parent triggers

### Cybernetic Isolation Principle

A `cell`'s `node` **cannot** see the parent's `%State`. They react only to:
- The cell's own private state fields
- The cell's own input arguments
- The cell's own `trg` variables (internal triggers)

This makes it safe to compose complex systems (console I/O, protocol stacks, hardware modules) without accidental coupling to the outside state space.

---

## 2. Full Specification

### 2.1 Two Variants

| Property | `cell` | `cell!` |
|----------|--------|---------|
| Lifecycle | Auto-terminates on stasis (nothing can fire) or `term!` | Persistent — stays alive until `term!` or parent exit |
| Sync call `let r = c(args)` | ✓ Blocks parent reactor, runs to completion, returns | ✗ Compile error |
| Async `async c(args)` | ✓ Runs to completion in background | ✓ Standard invocation form |
| `trg @ c().port` one-liner | ✗ Requires explicit `let t = async cell` + `trg @ t.port` | ✓ Creates implicit async instance |
| Meaning of `!` | — | "Altered control flow — pay attention" |

### 2.2 Output Ports

```
cell name(args) -> name: Type [, name: Type]*     // Tuple — all ports simultaneously active
cell name(args) -> name: Type [| name: Type]*     // Union — exactly one port active at a time
```

| Separator | OutputType | Meaning | Binding |
|-----------|-----------|---------|---------|
| `,` | `Tuple(Named...)` | All ports are active every tick | Each port can have its own `trg` |
| `\|` | `Union(Named...)` | Exactly one port carries a meaningful value | Caller must match to extract |

### 2.3 Lifecycle Rules

**`cell` (auto-terminating):**
1. Sync call `let r = c(args)` — allocates instance state, sets inputs, runs reactor loop
2. Reactor loop: all `node` fire while preconditions are true
3. On each tick: `term` = normal contribution; `term!` = stop immediately
4. Stasis = no `node` can fire (all preconditions false) → stop
5. On stop: return value of the designated output port (first named output in `-> Type: name`)
6. Instance state is deallocated

**`cell!` (persistent):**
1. Only `async` instantiation: `let c = async cell!(args)` or `trg @ cell!(args).port`
2. Starts independent reactor loop in background
3. Continuously updates output ports via `node`
4. `term!` inside → component stops and is collected
5. Parent exit → component stops and is collected
6. Output ports are readable by parent ONLY through `trg` bindings

### 2.4 `term` vs `term!` Inside cell

| Statement | Effect |
|-----------|--------|
| `term <expr>` | Normal tick — contributes to convergence cycle, cell continues |
| `term! <expr>` | Terminate component immediately. Return the value of the designated output variable (NOT the expression). |
| `term!;` | Terminate component immediately. Return the current value of the designated output variable. |

**Note**: In a `cell` (sync), `term!` returns to the parent. In a `cell!`, `term!` stops the component (the parent sees the trigger stop firing).

### 2.5 Trigger Binding

```brief
// One-liner with explicit port (cell! only):
trg X: Type @ cell!(args).port_name;

// One-liner shorthand (single output — port name omitted):
trg X: Type @ CellName!;
// Only valid when the cell! has exactly ONE output port.
// Desugars to: let __anon = async CellName!(); trg X @ __anon.<sole_port>;

// Explicit multi-port:
let c = async cell!(args);
trg A @ c.port1;
trg B @ c.port2;

// For cell (auto-terminating), always explicit:
let c = async cell(args);
trg A @ c.port;
```

**Rules for `trg @ instance.port`:**
- `.port` must be a named output declared in the cell's `-> Type: name` return
- The trigger fires when the cell writes to that output variable
- For `cell!`, the trigger fires continuously until the component stops
- For `cell` used with `async`, the trigger fires during the cell's lifetime

**Rules for `trg @ CellName!` shorthand:**
- `CellName!` must be a `cell!` (persistent) definition with exactly one output port
- Desugars to: create an anonymous async instance of `CellName!`, bind the trigger to its sole output port
- The anonymous instance lives as long as the trigger binding exists (parent scope lifetime)
- This is the primary ergonomic form for std library components (e.g., `trg buf: String @ Console!;`)

### 2.6 Direct Reads — FORBIDDEN

```brief
let c = async cell!(args);
let x = c.port;              // COMPILE ERROR — can't read cell variables directly
trg X @ c.port;              // OK — trigger binding
```

This prevents polling, stale reads, and timing coupling.

---

## 3. Syntax Reference

### 3.1 EBNF

```ebnf
cell_decl      ::= "cell" ["!"] ident "(" params ")" ["->" outputs] "{" body "}"
params         ::= param ("," param)*
param          ::= ident ":" type                       (* input argument *)
outputs        ::= output ("," | "|") output            (* tuple or union *)
output         ::= ident ":" type                       (* named output port *)
body           ::= decl*
decl           ::= field_decl | trans_decl | trg_decl
field_decl     ::= ident ":" type ["=" expr] ";"
trans_decl     ::= "rct" "txn" ident contract block     (* reactive transitions *)
                 | "txn" ident params contract block   (* non-reactive transitions *)
                 | "defn" ident params contract block   (* helper functions *)
trg_decl       ::= "trg" ident "@" source ";"          (* internal triggers *)
```

### 3.2 Full Example

```brief
// ── Timer component ──
cell! timer(duration: Int) -> elapsed: Int, done: Bool {
    elapsed: Int = 0;
    done: Bool = false;

    node tick [elapsed < duration] {
        &elapsed = elapsed + 1;
    };

    node finish [elapsed >= duration && !done] {
        &done = true;
        term!;        // Stop component
    };
};

// ── Safe division (auto-terminating) ──
cell safe_div(a: Int, b: Int) -> result: Int | error: String {
    result: Int = 0;
    error: String = "";

    node ok [b != 0] {
        &result = a / b;
        term!;
    };

    node err [b == 0] {
        &error = "division by zero";
        term!;
    };
};

// ── Parent ──
let t = async timer(duration: 100);
trg E @ t.elapsed;
trg D @ t.done;

let r = safe_div(10, 2);
term! r;
```

---

## 4. Desugaring Model

### 4.1 Cell Definition → Prefixed State + Defn

A `cell timer(@elapsed, @done, duration: Int)` with body declarations becomes:

**Before (cell AST):**
```
CellDef {
    is_persistent: false,
    name: "timer",
    inputs: [("duration", Int)],
    output_type: Tuple([
        Named("elapsed", Single(Int)),
        Named("done", Single(Bool)),
    ]),
    fields: [
        ("elapsed", Int, Some(Expr::Integer(0))),
        ("done", Bool, Some(Expr::Bool(false))),
    ],
    transactions: [...],
    internal_triggers: [...],
}
```

**After desugaring (flat TopLevel items):**

```
// 1. State declarations with instance-name prefix
// These are generated at INSTANTIATION time, not definition time.

// For each field, generate:
StateDecl { name: "timer$<uid>.elapsed", ty: Int, expr: Some(Integer(0)), ... }
StateDecl { name: "timer$<uid>.done", ty: Bool, expr: Some(Bool(false)), ... }

// Also generate state slots for each output port (for trigger binding metadata):
// Output ports are state slots with trigger-binding annotation

// 2. Transition definitions
// Each node / txn / defn inside the cell body is rewritten with prefixed
// state references:

Definition {
    name: "timer$<uid>.tick",
    parameters: [("duration", Int)],
    // Body references: elapsed → timer$<uid>.elapsed, done → timer$<uid>.done
    body: [
        // Original: &elapsed = elapsed + 1;
        Assignment { lhs: StateRef("timer$<uid>.elapsed"),
                     rhs: Add(Identifier("timer$<uid>.elapsed"), Integer(1)) },
        // Original: [pre] { ... }
        Guarded { guard: /* rewritten */, body: /* rewritten */ },
    ],
}

// 3. Binding sync wrapper (for exposed output ports)
// The sync wrapper copies bound parent variables into/out of component state
// before and after invocation. Since the only external binding is via triggers,
// the sync is: load parent trigger-slot → store into cell state (pre-call),
// load from cell state → store into parent trigger-slot (post-tick).
```

### 4.2 Instantiation → State Allocation + Binding Record

```
// Source: let t = async timer(duration: 100);

// Desugars to:
let __timer_uid_alloc = CellInstance {
    uid: unique_id(),
    cell_def: &timer_def,
    state_base: state_alloc_count,   // Starting index in %State
    field_count: 2,                  // elapsed, done
    bindings: [
        ("elapsed", trigger_slot_id),   // If bound to parent trigger
        ("done", trigger_slot_id),
    ],
};

// State fields are allocated in the parent's %State:
// timer$<uid>.elapsed at index N
// timer$<uid>.done at index N+1

// The instance handle becomes a reference to this allocation record.
```

### 4.3 Sync Invocation → Reactor Loop with Prefixed State

```
// Source: let r = timer(duration: 100);
// (where timer is a cell, not cell!)

// Desugars to:
{
    // 1. Allocate prefixed state (on stack or in parent %State)
    // 2. Set inputs
    let timer$<uid>.duration = 100;
    
    // 3. Run convergence loop
    loop {
        let fired = false;
        // For each node in definition order:
        if /* tick precondition rewritten with prefixed vars */ {
            // Execute body with rewritten identifiers
            &timer$<uid>.elapsed = timer$<uid>.elapsed + 1;
            fired = true;
        }
        if /* finish precondition */ {
            // Execute body
            &timer$<uid>.done = true;
            term!;  // stop loop, return
        }
        if !fired { break; }  // stasis
    }
    
    // 4. Return designated output value
    let r = timer$<uid>.elapsed;  // first named output
}
```

### 4.4 Async Instantiation → Background Runtime

```
// Source: let t = async timer(duration: 100);

// Desugars to:
// The async runtime creates a component instance with:
// 1. A heap-allocated copy of the component's state slots
// 2. A background reactor task (for the interpreter, a coroutine)
// 3. Shared-memory slots for output ports (for trigger binding)

// The async runtime handles:
// - Polling the component's reactor loop (one iteration per parent tick)
// - Writing output port changes to shared slots
// - Trigger propagation (if bound)

// The handle t:
// - Identifies the component instance in the runtime
// - References output port slots for trigger binding
// - Is invalidated when the component terminates
```

### 4.5 Trigger Binding

```
// Source: trg E @ t.elapsed;

// Desugars to:
// 1. Look up component instance t
// 2. Find output port "elapsed" in t's definition's output_type
// 3. Get the state slot index for elapsed (timer$<uid>.elapsed)
// 4. Register a trigger on that state slot:
//    Trigger {
//        name: "E",
//        source: StateSlot(timer$<uid>.elapsed),
//        // fires when the slot value changes
//    }

// This reuses the existing trigger infrastructure — the same
// mechanism as trg name @ field_name, but the field is a
// component instance's state slot.
```

---

## 5. Implementation Plan — Phase 1

### Scope

Phase 1 implements:
- `cell` and `cell!` definitions (parser + AST)
- Sync invocation for `cell` (blocks reactor, runs to completion, returns)
- Async instantiation for `cell!` (separate runtime, independent loop)
- Output port declaration via `-> name: Type [, | name: Type]`
- Internal `node`, `txn`, `defn`, `trg` inside cell body
- `term` and `term!` semantics inside cell
- Trigger binding to output ports (`trg @ instance.port`)
- No direct reads of cell variables (compile error)
- Flat prefixed state layout (no sub-state GEP — Phase 2)

### Pipeline Changes

```
Parser → AST → Desugarer → Typechecker → Interpreter / LLVM Backend
         ↑                              ↗
    CellDef              Cell handled as
    TopLevel::Cell        prefixed StateDecls
    Expr::CellCall             + Definitions
```

### Touch Points

| File | Change Type | Estimated Lines |
|------|------------|-----------------|
| `src/lexer.rs` | Add `Token::Cell`, `Token::At` | 10 |
| `src/ast.rs` | Add `CellDef`, `TopLevel::Cell`, `Expr::CellCall`, `Statement::TrgBinding` | 80 |
| `src/parser.rs` | Parse `cell`/`cell!`, params, outputs, body | 150 |
| `src/desugarer.rs` | Transform CellDef → StateDecl + Definition, instance allocation, binding sync | 150 |
| `src/typechecker.rs` | Validate isolation, output uniqueness, field privacy | 80 |
| `src/interpreter.rs` | `call_cell()` sync, async runtime, trigger binding, `term!` handling | 200 |
| `src/backend/llvm/mod.rs` | `build_field_index` for cell instances, collect definition params | 50 |
| `src/backend/llvm/emit_toplevel.rs` | Emit cell definitions as functions with prefixed state access | 80 |
| `src/backend/llvm/emit_expr.rs` | `Expr::CellCall` → prefixed state + convergence loop | 100 |
| `src/features/toplevel/cell.rs` | Pattern-B wrapper following existing conventions | 30 |
| `src/features/toplevel/mod.rs` | `pub mod cell;` | 2 |
| `tests/` | Parser tests, interpreter E2E, LLVM IR assertions | 200 |
| `docs/architecture/features/cell.md` | Feature documentation | 80 |

**Total: ~1,212 lines**

---

## 6. File-by-File Implementation Guide

### 6.1 `src/lexer.rs`

**Changes:**
1. Add `#[token("cell")]` → `Token::Cell`
2. Add `#[token("@")]` → `Token::At` (check if `@` already exists)

**Token display:**
```rust
Token::Cell => write!(f, "cell"),
Token::At => write!(f, "@"),
```

### 6.2 `src/ast.rs`

**Add `CellDef` struct:**
```rust
#[derive(Debug, Clone)]
pub struct CellDef {
    pub is_persistent: bool,          // false = cell, true = cell!
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub parameters: Vec<(String, Type)>,    // input arguments
    pub output_type: Option<OutputType>,    // -> name: Type, ...
    pub fields: Vec<StructField>,           // private state fields
    pub transactions: Vec<Transaction>,     // node / txn inside cell
    pub definitions: Vec<Definition>,       // defn inside cell
    pub internal_triggers: Vec<TriggerDeclaration>,  // trg inside cell
    pub span: Option<Span>,
    pub modifiers: Vec<Hashtag>,
}
```

**Add to `TopLevel` enum:**
```rust
pub enum TopLevel {
    // ... existing variants ...
    Cell(Box<CellDef>),
}

**Add `Expr::CellCall`:**
```rust
pub enum Expr {
    // ... existing variants ...
    /// cell_name(args) — synchronous call to a cell.
    /// First element is the cell instance expression (identifier or field),
    /// remaining are the input arguments.
    CellCall(Vec<Expr>),
}
```

**Add `Statement::TrgBinding`:**
```rust
pub enum Statement {
    // ... existing variants ...
    /// trg name @ instance.port;
    TrgBinding {
        name: String,
        ty: Option<Type>,         // optional explicit type
        instance: Expr,           // expression yielding a component handle
        port: String,             // named output port on the component
        modifiers: Vec<Hashtag>,
    },
}
```

### 6.3 `src/parser.rs`

**In `parse_top_level()` (around line 1125, before `Struct`):**
```rust
Some(Ok(Token::Cell)) => {
    // Check for '!' after 'cell'
    let is_persistent = self.peek_is(Token::Bang);
    if is_persistent { self.advance(); }  // consume '!'
    self.parse_component_definition(is_persistent)
}
```

**New method `parse_component_definition(&mut self, is_persistent: bool)`:**
```rust
fn parse_component_definition(&mut self, is_persistent: bool) -> Result<TopLevel, SyntaxError> {
    let span = self.current_span();
    // Parse name
    let name = self.expect_identifier()?;
    // Parse type params if any
    let type_params = self.parse_type_params()?;
    // Parse '(' params ')'
    self.expect(Token::OpenParen)?;
    let parameters = self.parse_cell_params()?;
    self.expect(Token::CloseParen)?;
    // Parse '-> outputs' if present
    let output_type = if self.peek_is(Token::Arrow) {
        self.advance();
        Some(self.parse_output_type_structure()?)
    } else {
        None
    };
    // Parse '{' body '}'
    self.expect(Token::OpenBrace)?;
    let (fields, transactions, definitions, internal_triggers) = self.parse_cell_body()?;
    self.expect(Token::CloseBrace)?;
    
    Ok(TopLevel::Cell(Box::new(CellDef {
        is_persistent,
        name,
        type_params,
        parameters,
        output_type,
        fields,
        transactions,
        definitions,
        internal_triggers,
        span: Some(span),
        modifiers: Vec::new(),
    })))
}
```

**In `parse_top_level()` `is_sed` handling (around line 1174):**
Add `TopLevel::Cell(c) => Some(c.name.clone())` to the name extraction.

**Parse `Statement::TrgBinding`** (in statement parsing, around line 5000):
```rust
// trg name @ instance.port;
// Detect: current token is "trg", next token is identifier, then '@'
fn parse_trg_binding(&mut self) -> Result<Statement, SyntaxError> {
    self.expect(Token::Trg)?;
    let name = self.expect_identifier()?;
    let ty = if self.peek_is(Token::Colon) {
        self.advance();
        Some(self.parse_type()?)
    } else {
        None
    };
    self.expect(Token::At)?;
    // Parse instance expression . port_name
    let instance = self.parse_primary_expression()?;  // e.g., identifier or cell call
    self.expect(Token::Dot)?;
    let port = self.expect_identifier()?;
    self.expect(Token::Semicolon)?;
    Ok(Statement::TrgBinding { name, ty, instance, port, modifiers: vec![] })
}
```

### 6.4 `src/desugarer.rs`

**In `desugar()` method (around line 191), add a new match arm:**
```rust
TopLevel::Cell(cell) => {
    // 1. At definition time: validate and register the component definition
    //    (definitions are stored for use at instantiation sites)
    // 2. Generate the component's "template" — a set of state declarations and
    //    transition definitions that will be instantiated with a unique UID.
    
    // Store the component definition for later instantiation
    self.cell_defs.insert(cell.name.clone(), cell.clone());
    
    // Generate the transition functions as definitions with $<uid> parameterization.
    // These are registered but parameterized — they need a UID to be complete.
    // The actual state declarations are generated at each instantiation site.
    
    // For each transaction in the cell body, generate a parameterized definition:
    for txn in &cell.transactions {
        let param_name = format!("{}__$uid", cell.name);
        items.push(TopLevel::Definition(/* ... */));
    }
}
```

Actually, there's a design choice here. The desugarer can either:

**Approach A: Definition-time desugaring (generate parameterized templates)**
- At definition: generate definitions with a `$uid` parameter that's filled in at instantiation
- At instantiation: generate state declarations with concrete UID and wire up calls

**Approach B: Instantiation-time desugaring (generate everything at let/trg site)**
- At definition: just register the CellDef in a lookup table
- At instantiation: lookup the definition, generate all state decls + definitions with concrete UID

Approach B is simpler because the definitions contains concrete state references rather than parameterized ones. But it means the definitions are generated in the middle of expression/statement desugaring, not at the top level.

For Phase 1, use **Approach B** — keep `CellDef` as a pass-through at the top level, and desugar at instantiation sites (in `Expr::CellCall` and `Statement::TrgBinding` processing).

**Desugaring `Expr::CellCall` (the `cell(args)` call expression):**

The desugarer needs a new method:
```rust
fn desugar_cell_call(&mut self, args: &[Expr], span: Option<Span>) -> Vec<TopLevel> {
    // 1. Generate a unique UID for this instance
    // 2. Look up the CellDef from self.cell_defs
    // 3. Generate StateDecl for each field with prefixed name
    // 4. Generate a Block expression that:
    //    a. Sets input arguments
    //    b. Runs the convergence loop
    //    c. Returns the designated output value
    // 5. Return the generated items + the expression
}
```

This is complex because the desugarer needs to generate statements and expressions inline. An alternative simpler approach:

**Simpler Approach B':** Keep `TopLevel::Cell` through the pipeline. Don't desugar it into other forms. The typechecker, interpreter, and LLVM backend all handle it directly. This preserves the semantic information (isolation boundary, output ports, lifecycle) that would be lost in a full desugar.

This is what we'll do. **The component is a first-class AST node throughout the pipeline.** No desugaring into lower-level forms.

### 6.5 `src/typechecker.rs`

**Pass 1 (collection, around line 520):**
```rust
TopLevel::Cell(cell) => {
    // Register the component definition
    // Collect output port names and types
}
```

**Pass 2 (validation, around line 644):**
```rust
TopLevel::Cell(cell) => {
    // 1. Verify all output port names reference existing fields
    let output_vars = cell.output_type.as_ref().map(|ot| {
        // Extract Named slot names from OutputType
    });
    // 2. Verify no external state references in any transaction body
    // 3. Verify 'term!' is valid within cell context
}
```

**Typechecking `Expr::CellCall`:**
```rust
Expr::CellCall(args) => {
    // 1. First arg is the component instance expression — must resolve to a CellDef
    // 2. Remaining args are input arguments — typecheck against CellDef.parameters
    // 3. Return type is the CellDef's output type
    // 4. Verify CellDef.is_persistent == false (cell! can't be sync-called)
}
```

**Typechecking `Statement::TrgBinding`:**
```rust
Statement::TrgBinding { instance, port, ty, .. } => {
    // 1. instance expression must resolve to a component handle
    // 2. port must be a named output in the component's output_type
    // 3. ty must match the port's type (or be inferrable)
    // 4. If component is cell (not cell!), warn that trg on cell is unusual
}
```

### 6.6 `src/interpreter.rs`

**Registration (in `run()`, around line 952):**
```rust
TopLevel::Cell(cell) => {
    self.cell_defs.insert(cell.name.clone(), cell.clone());
    // Register internal triggers (scoped to the component type)
}
```

**Sync call (new method `call_cell`):**
```rust
fn call_cell(&mut self, cell_def: &CellDef, args: &[Value]) -> Result<Value, RuntimeError> {
    // 1. Generate unique instance ID
    // 2. Push a new scope for component state
    // 3. Initialize fields from initializers
    // 4. Bind input arguments
    // 5. Run convergence loop (same as reactor loop in run())
    // 6. On stasis or term!, pop scope and return designated output value
    
    let uid = self.next_cell_uid;
    self.next_cell_uid += 1;
    
    // Push scoped state
    self.state_scope.push();
    
    // Initialize fields
    for field in &cell_def.fields {
        let prefixed_name = format!("{}${}.{}", cell_def.name, uid, field.name);
        let value = if let Some(expr) = &field.expr {
            self.eval_expr(expr)?
        } else {
            match field.ty {
                Type::Int => Value::Int(0),
                Type::Bool => Value::Bool(false),
                Type::Float => Value::Float(0.0),
                Type::String => Value::String(String::new()),
                _ => Value::Void,
            }
        };
        self.state.insert(prefixed_name, value);
    }
    
    // Bind input arguments
    for ((param_name, _), arg) in cell_def.parameters.iter().zip(args) {
        let prefixed = format!("{}${}.{}", cell_def.name, uid, param_name);
        self.state.insert(prefixed, arg.clone());
    }
    
    // Convergence loop
    let mut executed = true;
    while executed {
        executed = false;
        for txn in &cell_def.transactions {
            // Check precondition (rewrite identifiers with prefix)
            let pre = rewrite_identifiers(&txn.contract.pre_condition, &cell_def.name, uid);
            let pre_val = self.eval_expr(&pre)?;
            if pre_val == Value::Bool(true) {
                self.prior_state = self.state.clone();
                for stmt in &txn.body {
                    // Rewrite identifiers in each statement
                    let rewritten = rewrite_identifiers_in_stmt(stmt, &cell_def.name, uid);
                    match self.exec_stmt(&rewritten) {
                        Ok(_) => {}
                        Err(RuntimeError::Terminated) => {
                            // term! hit — exit convergence loop
                            self.state_scope.pop();
                            // Return designated output value
                            return self.get_designated_output(cell_def, uid);
                        }
                        Err(e) => return Err(e),
                    }
                }
                // Check postcondition
                let post = rewrite_identifiers(&txn.contract.post_condition, &cell_def.name, uid);
                let post_val = self.eval_expr(&post)?;
                if post_val == Value::Bool(true) && self.state != self.prior_state {
                    executed = true;
                }
            }
        }
    }
    
    // Stasis — return designated output value
    let result = self.get_designated_output(cell_def, uid);
    self.state_scope.pop();
    Ok(result)
}

fn get_designated_output(&self, cell_def: &CellDef, uid: usize) -> Value {
    // Extract the first named output from the output_type
    // Read its value from the state
    if let Some(ref ot) = cell_def.output_type {
        if let OutputType::Named(name, _) = ot {
            // Actually need to flatten the output structure to find the first named slot
            // For OutputType::Tuple([Named("a", Single(Int)), Named("b", Single(Bool))])
            // the "first" is named "a" — return that value
        }
        // For Union, also return the first — caller discriminates
    }
    Value::Void
}
```

**Handling `Statement::TrgBinding`:**
```rust
Statement::TrgBinding { name, instance, port, .. } => {
    // Evaluate instance to get a component handle
    let handle = self.eval_expr(&instance)?;
    // Register a trigger on the specified output port
    // Uses same trigger infrastructure as trg @ state_field
}
```

### 6.7 `src/backend/llvm/mod.rs`

**In `build_field_index()` (line 2520):**
```rust
TopLevel::Cell(cell) => {
    // At definition time: nothing to add to state
    // At instantiation time (when Expr::CellCall is seen during codegen):
    // The LLVM backend tracks pending cell instances and allocates their fields
}
```

**In `generate()` (line 1220):**
```rust
TopLevel::Cell(cell) => {
    // 1. Register the component definition with its output types
    // 2. Generate the transition functions as @llvm functions with
    //    a parameterized state prefix (filled in at call sites)
    
    // For now, generate nothing — the component's transitions are emitted
    // inline at each call site (or as separate functions called via GEP).
}
```

**New method `emit_cell_transition()`:**

fn emit_cell_transition(&mut self, out: &mut String, cell: &CellDef, uid: usize) {
    // For each node in the component, emit an LLVM function that:
    // - Takes a pointer to the component's sub-state region
    // - Takes input arguments
    // - Returns void (reactive txn) or the output value (non-reactive txn)
    // - Runs the convergence loop
    
    // Phase 1: flat prefixed state — the function takes %State* and a base index
    // Phase 2: sub-state GEP — the function takes %CellState* (nested struct)
}
```

### 6.8 `src/backend/llvm/emit_toplevel.rs`

**New method `emit_cell_call()`:**

For `Expr::CellCall`, emit:
```llvm
; Allocate component state fields in the current function
%cell_base = getelementptr inbounds %State, ptr %state, i32 0, i32 <base_index>

; Initialize fields
store i64 0, ptr %cell_base

; Set input arguments
store i64 %arg0, ptr %cell_base + 1

; Convergence loop
br label %cell_loop

cell_loop:
  ; For each node in the cell body:
  ; 1. Check precondition
  ; 2. Execute body with prefixed state references
  ; 3. Check postcondition
  ; 4. Branch back to cell_loop or exit
  
  ; Precondition check for txn "tick":
  %pre_cond = ...  ; rewritten with cell$uid. prefix
  br i1 %pre_cond, label %tick_body, label %tick_done

tick_body:
  ; Original: &elapsed = elapsed + 1
  %elapsed_ptr = getelementptr i64, ptr %cell_base, i32 0
  %elapsed_val = load i64, ptr %elapsed_ptr
  %new_elapsed = add i64 %elapsed_val, 1
  store i64 %new_elapsed, ptr %elapsed_ptr
  br label %tick_done

tick_done:
  ; Check if any txn fired
  ; If none fired: stasis → exit loop
  ; If term! was hit: br exit
  br i1 %any_fired, label %cell_loop, label %cell_done

cell_done:
  ; Return designated output value
  %result = load i64, ptr %cell_base  ; first output
  ret i64 %result
```

### 6.9 `src/backend/llvm/emit_expr.rs`

**Handle `Expr::CellCall`:**

```rust
Expr::CellCall(args) => {
    // args[0] = component instance expression
    // args[1..] = input arguments
    
    // 1. Evaluate the instance expression to get the component UID
    // 2. Emit the convergence loop (see emit_toplevel.rs section)
    // 3. Return the designated output value
    
    let uid = self.next_cell_instance_uid;
    self.next_cell_instance_uid += 1;
    
    // Allocate state slots for this instance
    // (pre-allocated in build_field_index or dynamically via alloca)
    
    // Emit convergence loop IR
    self.emit_cell_convergence_loop(out, cell_def, uid, args, indent);
    
    // Return the register holding the output value
    format!("%cell_{}_result", uid)
}
```

### 6.10 `src/features/toplevel/cell.rs`

Following the Pattern-B convention (thin wrapper):

```rust
pub struct CellItem(pub CellDef);

impl CellItem {
    pub fn name(&self) -> &str { &self.0.name }
    pub fn is_persistent(&self) -> bool { self.0.is_persistent }
    pub fn parameters(&self) -> &[(String, Type)] { &self.0.parameters }
    pub fn output_type(&self) -> &Option<OutputType> { &self.0.output_type }
    pub fn fields(&self) -> &[StructField] { &self.0.fields }
    pub fn transactions(&self) -> &[Transaction] { &self.0.transactions }
    pub fn internal_triggers(&self) -> &[TriggerDeclaration] { &self.0.internal_triggers }
}
```

**Add to `src/features/toplevel/mod.rs`:**
```rust
pub mod cell;
```

### 6.11 Identifier Rewriting

The most critical implementation detail. When desugaring a component's body, every identifier reference must be rewritten to use the `cell$uid.` prefix.

```rust
fn rewrite_identifiers(expr: &Expr, cell_name: &str, uid: usize) -> Expr {
    let prefix = format!("{}${}.", cell_name, uid);
    match expr {
        Expr::Identifier(name) => {
            // Only rewrite identifiers that are component state fields or params
            // (not builtins, not intrinsic names, not external triggers)
            Expr::Identifier(format!("{}{}", prefix, name))
        }
        Expr::Add(l, r) => Expr::Add(
            Box::new(rewrite_identifiers(l, cell_name, uid)),
            Box::new(rewrite_identifiers(r, cell_name, uid)),
        ),
        // ... recurse into all expression variants ...
        _ => expr.clone(),  // terminals (Integer, Bool, String, etc.) unchanged
    }
}
```

**Important**: The identifier rewriter must NOT rewrite:
- Intrinsic names (`print_int#`, `get_global_id#`)
- Foreign function names (declared via `frgn`)
- Trigger names that reference external sources (`@stdin`, etc.)
- Type names
- Anything that's not a component field, parameter, or local variable

---

## 7. Test Plan

### 7.1 Parser Tests

| Test | Input | Expected |
|------|-------|----------|
| Parse cell | `cell empty() {}` | `TopLevel::Cell` with no fields, no txns |
| Parse cell! | `cell! persistent() {}` | `is_persistent: true` |
| Parse params | `cell add(a: Int, b: Int) {}` | Two input params |
| Parse outputs | `cell sensor() -> temp: Float, ready: Bool {}` | Tuple output with two named slots |
| Parse union outputs | `cell div() -> val: Int \| err: String {}` | Union output |
| Parse body fields | `cell c() { x: Int = 0; }` | One field |
| Parse body txn | `cell c() { node t [true] { term; }; }` | One reactive transaction |
| Parse internal trg | `cell! c() { trg k @ stdin; }` | One internal trigger |
| Reject sync cell! | `let r = cell!(){}; r()` | Compile error |

### 7.2 Interpreter Tests

| Test | Description |
|------|-------------|
| Basic sync cell | `cell add(a: Int) -> r: Int { r: Int = a; node go [true] { &r = r + 1; term!; }; }; let a = add(5);` → `a == 6` |
| Cell convergence | `cell count(n: Int) -> c: Int { c: Int = 0; node inc [c < n] { &c = c + 1; }; }; let r = count(5);` → `r == 5` |
| Cell with term! early exit | `cell early() -> v: Int { v: Int = 0; node exit [true] { &v = 99; term!; }; }; let r = early();` → `r == 99` |
| Cell! persistent lifecycle | `cell! timer(d: Int) -> e: Int { e: Int = 0; node t [e < d] { &e = e + 1; }; }; let t = async timer(3);` → instance runs, e increments to 3 |
| Cell privacy | Accessing cell field from outside → error |
| Union output match | `cell div(a: Int, b: Int) -> v: Int \| e: String { ... }; let r = div(10, 2); match r { ... }` |
| Internal trigger isolation | `cell! reader() { trg k @ stdin; };` internal trg doesn't affect parent |
| Nested cells | A `cell` containing another `cell` instance |

### 7.3 LLVM Backend Tests

| Test | Assertion |
|------|-----------|
| State layout | Cell fields allocated with prefixed names in `%State` |
| Convergence loop | LLVM IR contains loop structure with precondition checks |
| Return value | Final output value loaded and returned |
| `term!` handling | Early exit branch emitted |
| Input argument passing | Args stored to prefixed state slots |

### 7.4 Error Tests

| Test | Expected Error |
|------|---------------|
| Direct read | `t.port` outside `trg` → compile error |
| Sync call cell! | `let r = cell!(...)` → compile error |
| External state access in cell | Cell `node` referencing parent field → compile error |
| Unknown output port | `trg @ t.nonexistent` → error |
| Type mismatch on output | `trg X: Int @ t.str_port` → type error |

---

## 8. Future Phases

### Phase 2 — Sub-State GEP + Verilog Mapping

- Cell state becomes a nested LLVM struct (`%CellState = type { ... }`)
- Transition functions take `ptr %CellState` (not `ptr %State`)
- Caller computes GEP from `%State*` to sub-state
- CIRCT backend: `cell` → `hw.module` with input/output ports
- Enables Verilog generation without flattening

### Phase 3 — `trg @ cell!` One-liner

- `trg X: Type @ cell!(args).port` syntax
- Implicit async instance creation
- Instance lifecycle tied to trigger lifetime (persistent `cell!`)
- Instance cleanup when trigger is unregistered

### Phase 4 — Cell-to-Cell Communication

- Exposed output ports of one cell feed into input arguments of another
- Static wiring at compile time (like hardware signals)
- Compiler can statically schedule or parallelize independent cells

---

## 9. Edge Cases and Gotchas

### 9.1 Identifier Rewriting Must Be Selective

The `rewrite_identifiers` function must NOT rewrite:
- Intrinsic function names (e.g., `print_int#`)
- `frgn` function names
- External trigger source names
- Builtin types
- Keywords used as identifiers

**Strategy**: Maintain a set of "known global" identifiers (intrinsics, frgns, types) that are excluded from rewriting. Only rewrite identifiers that are:
1. Declared as component fields in `cell.fields`
2. Declared as component parameters in `cell.parameters`
3. Declared as `let` or `state` inside the component's transactions

### 9.2 Stasis Detection in Sync Calls

The convergence loop must detect when NO `node` fires in a full pass. This is the "nothing can fire" condition. Implementation:
- Track `any_fired: bool` per iteration
- If `any_fired == false` after iterating all transactions → stasis → exit loop

### 9.3 Postcondition Check in Sync Calls

Each `node` inside the cell still checks its postcondition. If the postcondition fails:
- In a `cell` sync call: rollback that transaction's state changes, continue loop
- In a `cell!` async: same behavior (it's a reactor loop)

### 9.4 `term!` Propagation

Inside a cell:
- `term!` inside a `node` → stop the convergence loop, return to parent
- `term!` inside a nested `txn` or `defn` → same behavior (stop the cell)
- `term!` MUST NOT terminate the parent program when inside a cell
- `term!` outside a cell → unchanged behavior (program exit)

Implementation: The interpreter flags `RuntimeError::Terminated` is repurposed for cell-level `term!`. The cell's convergence loop catches it and returns. The parent's reactor loop does NOT catch it (it's the cell's internal signal).

### 9.5 Async Runtime for cell!

The async runtime needs:
- A registry of running component instances
- Each instance has its own state snapshot and tick counter
- On each parent reactor tick, each async cell gets one tick of its own reactor loop
- If a cell reaches stasis (or `term!`), it's removed from the registry

For the interpreter, this can be a simple cooperative coroutine model:
```rust
struct AsyncCellInstance {
    cell_def: CellDef,
    uid: usize,
    state: HashMap<String, Value>,
    alive: bool,
}

impl Interpreter {
    fn tick_async_instances(&mut self) {
        for inst in &mut self.async_instances {
            if !inst.alive { continue; }
            // Run one iteration of the cell's reactor loop
            // Update state, check stasis, check term!
        }
    }
}
```

### 9.6 Trigger Binding Implementation

`trg @ instance.port` needs to:
1. Evaluate `instance` to get the component handle (UID)
2. Look up the component definition to find the output port's state slot
3. Register a trigger on that state slot in the trigger system

The trigger infrastructure already handles `trg @ state_field`. The component port binding just maps the output port name to a state slot index (the port's field in the component's prefixed state).

### 9.7 `trg @ CellName!` Shorthand Desugaring

When the user writes `trg buf: String @ Console!;`, the desugaring is:

1. Look up `Console` in the component definitions table — must be a `cell!`
2. Verify `Console` has exactly one output port (e.g., `-> buffer: String`)
3. Generate an anonymous async instance with a compiler-generated UID:
   ```
   let __console_<uid> = async Console;
   trg buf: String @ __console_<uid>.buffer;
   ```
4. The instance is owned by the trigger runtime; it lives until the trigger is unregistered or the parent scope exits

If `Console` has zero or multiple output ports, it's a compile-time error — the user must use the explicit `trg @ instance.port` form.

---

## 10. Motivating Use Case: officina-cli Console Component

### 10.1 The Problem

The `officina-cli` project at `~/Desktop/Projects/officina-cli` requires complex TTY interaction: reading raw keystrokes from `stdin#`, stripping escape sequences, handling backspace/delete, buffering input, and surfacing clean strings to the application layer. Currently this requires manually wiring `trg` to `stdin@`, writing input-cleaning `node`s, and managing buffer state — a significant amount of boilerplate in every program that needs console input.

### 10.2 The Solution: `std/system_comp.bv`

A standard library component that encapsulates all TTY plumbing:

```brief
// lib/std/system_comp.bv
// Console component — reads raw stdin, produces clean strings

cell! Console -> buffer: String {
    trg raw @ stdin;              // Raw keystroke input (internal)
    buffer: String = "";
    saved: Char = '\0';

    node accumulate [raw != saved] {
        &saved = raw;

        // Handle special keys:
        //   '\n' → emit buffer, reset
        //   '\x7f' (backspace) → trim last char
        //   '\x1b' prefix → skip escape sequence
        //   printable → append

        [raw == '\n'] {
            // Emit buffer (trigger fires, parent consumes)
            // Reset for next line
            // term! would stop the component — instead let buffer update fire the trigger
        };

        [raw == '\x7f' && buffer != ""] {
            &buffer = buffer :> Slice(0, buffer :> Size - 1);
        };

        [raw != '\n' && raw != '\x7f' && raw >= ' '] {
            &buffer = buffer + char_to_string(raw);
        };
    };

    // Internal helper: clear buffer after parent consumes it
    node consume [buffer != ""] {
        // The trigger binding in the parent reads buffer.
        // After the parent's node fires (which reads buffer),
        // we reset.
        // This works because the cell's reactor runs its own convergence
        // independently of the parent's.
        &buffer = "";
    };
};
```

### 10.3 Parent Usage

```brief
import { Console } from "std/system_comp";

// One-liner: creates async Console instance, binds trigger to buffer output
trg consoleBuffer: String @ Console!;

// Reactive transaction: fires when Console updates its buffer
node displayOutput [consoleBuffer != "" && consoleBuffer != savedBuffer]
    [savedBuffer == @consoleBuffer]
{
    &savedBuffer = consoleBuffer;
    print_int#(savedBuffer);
};
```

### 10.4 What This Demonstrates

| Concept | How It's Used |
|---------|---------------|
| `cell!` persistence | Console runs for the entire program lifetime, processing keystrokes |
| Output port binding | `-> buffer: String` becomes the trigger source |
| `trg @ CellName!` shorthand | `trg consoleBuffer: String @ Console!;` — one line |
| Isolation | Internal `trg raw @ stdin` doesn't leak to parent |
| Reactive consumption | Parent's `node` fires when consoleBuffer changes |
| Convergence | Console's internal `accumulate` + `consume` converge independently |
| No direct reads | Parent only sees `consoleBuffer` through the trigger |

### 10.5 Future: Other System Components

The same pattern works for many system interaction points:

```brief
// lib/std/system_comp.bv (extended)
cell! FileReader(path: String) -> line: String;
cell! Clock -> tick: Int;
cell! Clipboard -> content: String;
cell! Network(url: String) -> data: String, status: Int;
cell! SignalHandler(sig: Int) -> fired: Bool;
```

Each component encapsulates a system interaction point (file I/O, timer, clipboard, network, signal) behind a clean trigger-able output port. The parent program stays simple — just `trg @ CellName!` + reactive consumption.

---

## 11. Error Messages

| Situation | Error Message |
|-----------|---------------|
| Sync call to cell! | `error: 'cell! {name}' is persistent and cannot be called synchronously. Use 'async {name}(args)' instead.` |
| Direct read of cell variable | `error: cannot directly read component variable '{name}'. Use 'trg @ instance.{name}' for trigger binding.` |
| External state access | `error: component '{name}' cannot access parent state field '{field}'. Components are isolated state spaces.` |
| Unknown output port | `error: component '{name}' has no output port '{port}'. Available ports: {list}.` |
| `trg @ CellName!` with 0 outputs | `error: component '{name}' has no output ports. Cannot bind trigger to a void component.` |
| `trg @ CellName!` with >1 outputs | `error: component '{name}' has multiple output ports. Use explicit 'trg @ instance.port_name' form.` |
| `trg @ CellName!` on cell (not cell!) | `error: component '{name}' is not persistent (use 'cell!' for persistent trigger binding).` |
| `trg @ CellName!` undefined | `error: unknown component '{name}'. Did you forget to import it?` |
| `term!` outside any cell | (existing behavior — program exit, no change) |
