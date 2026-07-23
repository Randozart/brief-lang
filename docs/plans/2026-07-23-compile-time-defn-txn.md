# Compile-Time `$defn` and `$txn` — Brief Functions in Stage Blocks

**Date:** 2026-07-23
**Status:** Plan

---

## Problem

Stage blocks (`$(Stage) { ... }`) can only use `$` intrinsics and basic
flow control (`let`, `when`, `foreach`). Any nontrivial logic requires
chaining intrinsics or adding new Rust code. The GLUE bridge generator's
template rendering shows the pain point: 10+ `StrReplace$` calls per
function, per template — a pure string algorithm expressed awkwardly.

## Solution

**`$defn`** and **`$txn`** are compile-time-only function definitions.
They live inside `$(Stage)` blocks, are extracted before codegen, and
can call `$` intrinsics freely. They push logic from Rust into Brief.

### Syntax

```brief
$(Normalized @ highest) {
    $defn replace_all(s: String, pairs: List) -> String {
        // Can call $ intrinsics: StrReplace$, StrLen$, etc.
    };

    $txn converge(state: List) [changed > 0][changed == 0] {
        // Convergent loop: re-runs body until postcondition met
    };

    // Regular calls inside stage block:
    let result = replace_all(tmpl, [["{{x}}", "a"]]);
    converge(my_state);
};
```

### Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Prefix | `$defn` not `defn$` | `$` at front signals "compile-time" instantly; grep-friendly |
| Lexing | `$defn` is `Identifier("$defn")` | Lexer already absorbs `$` + text into one identifier token |
| Dispatch | `check_identifier("$defn")` in parser | No lexer changes needed |
| AST | `Statement::InlineDefn(Definition)` | Stage body is `Vec<Statement>`; new variants hold the parsed definitions |
| Calls | No `$` needed at call site | Just `name(args)`; evaluator checks fn_registry for non-`$` calls |
| Execution | Body via existing `evaluate_stage_stmt` | Function bodies reuse the same statement evaluator |
| Returns | `Statement::Term(expr)` → signals value | Currently silently ignored; now causes function to return |
| Scope | Fresh scope per call | Parameters bound as `NavValue`; outer scope unchanged |
| Runtime | Never reaches codegen | Stage blocks extracted before LLVM; `$` intrinsics undefined at runtime |

---

## Implementation

### Step 1: AST — `Statement::InlineDefn`, `Statement::InlineTxn`

Add two new variants to `Statement` in `src/ast/top.rs`:

```rust
pub enum Statement {
    // ...
    InlineDefn(Definition),
    InlineTxn(Transaction),
}
```

### Step 2: Parser — detect `$defn` / `$txn` at statement level

In `src/parser/statements.rs`, `parse_statement()`, add to the `_` arm:

```rust
else if self.check_identifier("$defn") { self.parse_inline_defn() }
else if self.check_identifier("$txn")  { self.parse_inline_txn() }
```

Implement `parse_inline_defn()`:
- Consume the `$defn` identifier token
- Call `self.parse_definition()` (reuse from definitions.rs)
- Wrap result in `Statement::InlineDefn`

`parse_inline_txn()`:
- Consume the `$txn` identifier token
- Call `self.parse_transaction(false, false)`
- Wrap in `Statement::InlineTxn`

### Step 3: PluginManager — function registry

Add to `src/plugin/mod.rs`:

```rust
pub enum FnDef {
    Defn(Definition),
    Txn(Transaction),
}

pub struct PluginManager {
    // ...
    pub fn_registry: HashMap<String, FnDef>,
}
```

Registered when the evaluator encounters `InlineDefn` / `InlineTxn`.

### Step 4: Evaluator — register, call, return

In `src/macros/eval.rs`:

**Registration**: in `evaluate_stage_stmt`, handle:
- `Statement::InlineDefn(d)` → `registry.insert(d.name, FnDef::Defn(d))`
- `Statement::InlineTxn(t)` → `registry.insert(t.name, FnDef::Txn(t))`

**Call dispatch**: in `eval_nav_chain`, change the `Expr::Call` arm:
- If `name.ends_with('$')` → existing intrinsic path
- Else if `name` in `fn_registry` → execute function body
- Else → error

**Function execution** (`exec_defn`):
1. Create fresh scope with parameter bindings from call args
2. Execute body statements via `evaluate_stage_stmt`
3. `Statement::Term(expr)` → evaluate expr, return value
4. Return the term value as `NavValue`

**`txn` execution** (`exec_txn`):
1. Enter loop:
   a. Evaluate precondition → if false, return current state
   b. Execute body
   c. Evaluate postcondition → if true, return
   d. Repeat
2. Guard against infinite loops (max iteration bound)

**`Statement::Term` handling**: modify `evaluate_stage_stmt` to return
`Result<Option<NavValue>, String>` instead of `Result<(), String>`:
- Normal statements → `Ok(None)` (continue)
- `Statement::Term(Some(expr))` → evaluate expr, `Ok(Some(val))` (return)
- `Statement::Term(None)` → `Ok(Some(NavValue::Void))` (return void)
- `Statement::TermBang(opt)` → evaluate expr for side effects, return Void

**`Statement::If` handling**: evaluate guard, execute matching branch.

### Step 5: Tests

- `$defn` with pure arithmetic body, call and get result
- `$defn` calling a `$` intrinsic
- `$txn` convergent loop (count to N)
- `terminated` body (using `term` to return early)
- Scope isolation (outer `let` not visible inside `$defn`)
- Error on unknown non-`$` function name
