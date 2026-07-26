# Console Cell + `$!console_input` Macro

**Date**: 2026-06-27  
**Status**: Plan (approved, awaiting implementation)  

---

## Overview

Create a `Console` cell in the standard library (`lib/std/console.bv`) that encapsulates TTY input handling, and a `$!console_input` convenience macro that generates the wiring boilerplate.

### Target API

```brief
import "console.bv";

$!console_input(inp) {
    submit(inp);
};
```

This expands to:

```brief
trg inp: String @ Console!;

node __ci_inp [inp != ""]] {
    submit(inp);
    &inp = "";
    term;
};
```

Where `trg inp: String @ Console!` creates an async persistent `Console` cell instance and binds a trigger to its sole output port `line`.

---

## Design Decisions

### 1. Console Cell (`lib/std/console.bv`)

```
cell! Console -> line: String {
    buffer: String = "";
    prev_key: Char = '\0';

    trg raw: Char @ stdin#;

    // Enter → emit buffer as complete line, increment sequence counter
    node emit [raw != '\0' && raw == '\n']] {
        &line = buffer;
        &buffer = "";
        term;
    };

    // Backspace → trim last char
    node backspace [raw != '\0' && raw == '\x7f']] {
        &prev_key = raw;
        [buffer .#Size > 0] {
            &buffer = buffer :> Slice(0, buffer .#Size - 1);
        };
        term;
    };

    // Printable → accumulate
    node type [raw != '\0' && raw >= ' ' && raw != '\n' && raw != '\x7f']] {
        &prev_key = raw;
        &buffer = buffer + (String)raw;
        term;
    };
};
```

Key properties:

- **Internal `trg raw @ stdin#`**: the cell self-polls `stdin#` on every tick.
- **`prev_key`**: tracks the last processed key to detect changes (stdin# returns `'\0'` when nothing is available, so `raw != prev_key` prevents re-processing stale values).
- **`line: String`**: the output port. Updated only on Enter (via the `emit` transaction). This is the port the parent binds to.
- **No auto-clear of `line`**: the cell never resets `line` to `""`. The parent's handler acknowledges receipt via `&inp = "";` in its body (which writes to the parent's trigger variable, not the cell's state). The parent's trigger variable is separate from the cell's output port.
- **Duplicate-line problem**: Since `line` stays at its last emitted value until the next Enter, pressing Enter twice with the same input produces the same `line` value. The output-change trigger won't re-fire because the value didn't change. **Mitigation**: The parent must explicitly clear the trigger variable (`&inp = ""`) to acknowledge. Since the parent's trigger variable is independent of the cell's output port, the cell's next `line` update (even if it's the same string) gets synced to the parent. The output cache in `tick_persistent_cells` compares against the OLD parent state — if the parent cleared `inp`, the cache sees a "new" value even if the cell's line is the same string. Wait — the output cache stores the cell's output, not the parent's trigger variable. So `line` = "hello" → cached. Next Enter: `line` = "hello" again → cache says same → no sync. This means duplicate input IS lost.

  **True solution**: Add a `line_id: Int` output that increments on every emit. Parent binds to `line_id` (guaranteed to change) and reads `line` as a secondary value.

  ```brief
  cell! Console -> line: String, line_id: Int { ... }
  ```

  Parent pattern:
  ```brief
  trg inp_id: Int @ Console.line_id;

  node handle [inp_id != last_id]] {
      // inp_id has changed → new line available
      process_line(inp_line);
      &last_id = inp_id;
      term;
  };
  ```

  The `$!console_input` macro would generate TWO trg bindings.

### 2. `$!console_input` Macro

A `$!macro` that generates top-level items (trg binding + node handler).

**Macro definition** (placed in `lib/std/console.bv`):

```brief
macro console_input(line_var: String, body: Block) -> Block {
    quote {
        trg @line_var: String @ Console!;

        node __ci_@line_var [@line_var != ""]] {
            @body
            &@line_var = "";
            term;
        };
    };
};
```

**Implementation risks** (to be verified):

| Risk | Failure mode | Fallback |
|------|-------------|----------|
| `$!macro` can't be called at top level | Macro call only valid inside functions | Document the explicit pattern |
| `node` inside `quote` doesn't parse as `Statement` | Macro can't generate handler | Split: macro only generates `trg` binding |
| `trg @ Console!` shorthand fails in `Statement::TrgBinding` | Shorthand only works at top level | Macro generates `let __inst = cell Console; trg @ __inst.line;` |
| `@"$!Console"` string interpolation for cell name fails | Can't generate cell name in quote | Use explicit cell reference |

### 3. Duplicate-Line Handling

The `$!console_input` macro must handle the case where the user enters the same input twice.

**Approach**: Use `line_id` counter. The macro generates:

```brief
trg @line_var: String @ Console!;

node __ci_@line_var [@line_var != ""]] {
    @body
    &@line_var = "";
    term;
};
```

With `line: String` as the sole output, the parent's trigger fires when the cell's output changes. After the parent processes and clears the trigger variable (`&inp = ""`), the trigger won't re-fire until the cell outputs a new line. If the user enters the same line again, `line` gets the same value from the cell. The output-change detection compares against the cached cell output (not the parent's trigger variable). Since cell output didn't change, the sync doesn't fire.

**True fix**: Use a `line_id` counter guaranteed to change each emit. Parent binds to `line_id` and reads `line` as a side channel:

```brief
trg inp_id: Int @ Console.line_id;
let last_inp_id: Int = -1;

node handle [inp_id != last_inp_id]] {
    // The line is available via the other trg:
    // trg inp_line: String @ Console.line;
    process_line(inp_line);
    &last_inp_id = inp_id;
    term;
};
```

The `$!console_input` macro would generate both trg bindings and the handler.

---

## Files to Create / Modify

| File | Action | Purpose |
|------|--------|---------|
| `lib/std/console.bv` | Create | Console cell definition + `console_input` macro |
| `src/interpreter.rs` | Modify | Ensure internal triggers evaluated during `tick_persistent_cells` |
| `src/parser.rs` | Potentially modify | If `trg @ Console!` shorthand at statement level doesn't work |

## Files to NOT Modify

- `src/ast.rs` — all needed AST nodes exist
- `src/backend/llvm/` — existing cell codegen handles this pattern
- `src/typechecker.rs` — existing cell validation is sufficient

## Tests

| Test | Location | What it covers |
|------|----------|----------------|
| Parser: cell with internal trg | `src/parser.rs` | `trg raw: Char @ stdin#;` inside cell body |
| Interpreter: Console cell | `src/interpreter.rs` | Internal trigger eval, key accumulation, emit on Enter, backspace, line_id counter |
| Interpreter: Duplicate input | `src/interpreter.rs` | Same input twice → line_id changes |
| LLVM: Console cell fields | `src/backend/llvm/tests.rs` | Fields registered in `%State` |
| LLVM: Console cell codegen | `src/backend/llvm/tests.rs` | Convergence loop, output read |
| Macro: `$!console_input` | `src/features/macros/expand.rs` | Macro expansion produces correct AST |
| Integration: officina | `officina.bv` (optional) | End-to-end TUI replacement |

## Prerequisite: Internal Trigger Evaluation

The cell system has an architectural gap: `internal_triggers` are **parsed** and **stored** in `CellDef` but **never evaluated** during cell execution. Both interpreter paths — `call_cell` (sync) and `tick_persistent_cells` (persistent) — iterate only over `transactions` and skip internal trigger evaluation.

A top-level Brief program has this flow each tick:

```
1. Evaluate all trg sources (stdin#, timer#, mmio) → write to state
2. For each node: check precondition → execute body → check postcondition
3. Repeat until stasis
4. Sync output ports
```

A cell currently does steps 2–4 but not step 1. This means `trg raw: Char @ stdin#;` inside a cell never actually calls `stdin#`.

### Fix: Add internal trigger evaluation loop

In **`tick_persistent_cells`** (`src/interpreter.rs`, before line 1635):

```rust
// Evaluate internal triggers before running transactions
for trg in &instance.cell_def.internal_triggers {
    let trg_key = format!("{}${}.{}", cell_name, 0, trg.name);
    let trg_val = match &trg.address {
        LinkRef::Stdin => self.eval_expr(&Expr::IntrinsicCall("stdin".to_string(), vec![]))?,
        LinkRef::Timer(hz) => { /* eval timer# */ Value::Int(0) },
        LinkRef::Explicit(addr) => { /* mmio read */ Value::Int(0) },
        LinkRef::Linked(name) => { /* linked source */ Value::Int(0) },
        _ => Value::Void,
    };
    self.state.insert(trg_key, trg_val);
}
```

Same pattern in **`call_cell`** (sync path, before line 1541).

The `prev_key` tracking in the Console cell's transactions handles the dirty-flag equivalent — no need for a full bitmask system in Phase 1.

## Implementation Order

1. **Fix interpreter**: Add internal trigger evaluation to `tick_persistent_cells` + `call_cell`
2. **Test**: Existing cell tests still pass + new test for internal `trg @ stdin#`
3. **Create**: `lib/std/console.bv` with the Console cell
4. **Test**: Parser + interpreter tests for Console cell (accumulate, emit, backspace, line_id)
5. **Test**: LLVM backend tests for Console cell field layout + convergence loop  
6. **Investigate**: Can `$!console_input` macro generate top-level items?
7. **Create**: Macro + tests (or document fallback)
8. **Optional**: Convert officina to use Console cell

---

## Eliminating Recursive AST Walks in Region Analyzer

### Problem

Four recursive functions in `src/analysis/region.rs` accumulate enough stack
depth to overflow the debug build on large programs (officina-cli):

| Function | Lines | Walk type | Depth risk |
|----------|-------|-----------|-----------|
| `expr_has_call` | 1431-1472 | Expression AST | High — deep Match/Slice nesting |
| `count_statements_recursive` | 1347-1365 | Statement body | Low — shallow nesting |
| `has_ffi_or_terminator_stmt` | 1367-1394 | Stmt + expr | Medium — combined |
| `has_ffi_or_trigger_stmt` | 1400-1428 | Stmt + expr | Medium — combined |

### Fix pattern

Each function is converted from recursion to an explicit `Vec` work stack.
No tree reconstruction is needed (unlike `substitute_expr`) — they are
boolean or count queries.

**`expr_has_call`**: Walk AST with a `Vec<&Expr>` stack. Return `true`
immediately on `Expr::Call`. Push children for composite nodes.

**`count_statements_recursive`**: Walk with `Vec<&Statement>` stack.
Accumulate count, push nested statement bodies.

**`has_ffi_or_terminator_stmt` / `has_ffi_or_trigger_stmt`**: Walk with
`Vec<&Statement>` stack. Return `true` on any terminating/ffi statement.
Push nested bodies. Call `expr_has_call` on assignment expressions.

### Files modified

`src/analysis/region.rs` — four functions, ~180 lines total.

### Verification

- `cargo test --lib` — all 1300 tests pass
- `brief build officina.bv` — no stack overflow in debug build

---

## Option A: Top-Level Macro Code Generation via `Value::Items`

### Problem

`compile#()` cannot generate top-level items (defn, txn, struct, enum, etc.).
The root cause is twofold:

1. `Value::Block(Vec<Statement>)` can only hold statements.
2. The macro expansion system (`expand_macro_calls_in_items`) unconditionally wraps
   results in `TopLevel::Statement`.

### Solution: `Value::Items(Vec<TopLevel>)`

**Step 1: Add `Items` variant to `Value` enum**

File: [`src/interpreter.rs:64-102`](../../../src/interpreter.rs)

Add `Items(Vec<TopLevel>)` as a new variant after `Block(Vec<Statement>)`.
Every match on `Value` needs a handler — most can use `_ =>` fallthrough or
explicitly map to `Value::Block(...)` for backward compat.

**Step 2: Update `compile#()` intrinsic to return `Value::Items`**

File: [`src/interpreter.rs:5079-5108`](../../../src/interpreter.rs)

Change from extracting only `Statement`/`TriggerBinding`:

```rust
Ok(Value::Items(prog.items))   // return ALL items
```

**Step 3: Add `value_to_items` conversion**

File: [`src/features/macros/template.rs`](../../../src/features/macros/template.rs)

Add `pub fn value_to_items(value: &Value) -> Vec<TopLevel>` that handles
`Value::Items(items)` and falls back to `value_to_statements` for other variants.

**Step 4: Update `expand_macro_calls_in_items` to accept `Value::Items`**

File: [`src/features/macros/expand.rs:257-288`](../../../src/features/macros/expand.rs)

When the macro returns `Value::Items`, insert the items directly (not wrapped
in `TopLevel::Statement`). When it returns other values (including `Block`),
continue to wrap the result in `TopLevel::Statement` as before.

**Step 5: `value_to_statements` fallback for `Value::Items`**

If someone calls a macro that returns `Items` from statement context (inside a
function body), the expansion should convert each `TopLevel::Statement` to a
`Statement` and skip non-statement items. This maintains backward compatibility.

### Impact

| Aspect | Before | After |
|--------|--------|-------|
| `compile#("let x: Int = 0;")` | Silently dropped | Injected as `TopLevel::StateDecl` |
| `compile#("trg inp @ Console!;")` | Injected as `Statement::TrgBinding` | Injected as `TopLevel::TriggerBinding` |
| `compile#("defn foo() { ... }")` | Silently dropped | Injected as `TopLevel::Definition` |
| Match arms to update | 0 | ~35 (all `Value` matches) |
| Risk | Low — fallthrough handles unknown | Low — `_ =>` fallthrough for unmigrated matches |

### Why not do this now

The officina refactor only needs the Console cell (which works via direct
`trg @ Console!` syntax). The `compile#()` fix enables macros to generate
arbitrary top-level items, which is valuable but orthogonal to the immediate
goal of trying out the Console cell.

---

## Risk Register

| Risk | Impact | Mitigation |
|------|--------|------------|
| Internal `trg` not evaluated during cell ticks | Cell can't self-poll stdin | Add eval loop in `tick_persistent_cells` |
| `$!macro` can't generate top-level items | Macro approach fails | Fall back to documented 5-line pattern |
| `node` inside `quote` doesn't parse as statement | Handler can't be macro-generated | Split: macro generates trg only, user writes handler |
| Duplicate-input loss despite line_id counter | Incorrect behavior | Ensure output comparison uses the latest cell output, not cached |

---

## Remaining Gaps (post-commit)

### Gap 1: `trg @ CellName!` shorthand parser support

**Root cause**: `parse_trigger_body` (4092) handles `@` by matching `LinkRef` variants (`stdin#`, `timer#`, `link`, integer address). `Console!` parses as `Identifier("Console")` + `Token::Not(!)`, falling through to `LinkRef::Linked("Console")` then erroring on the unparsed `!`.

**Fix**: Add a `trg @ CellName!` shorthand branch. After `@`, if token is an identifier followed by `!`, consume both, look up the cell def, verify single output port, create binding.

**File**: `src/parser.rs` — `parse_trigger_body` `@` handler (~line 4119)
**Est**: ~20 lines

### Gap 2: `cell Console` return type inference

**Root cause**: Typechecker `infer_expression` for `Expr::CellCall` (line 2279) returns the cell's `OutputType`. For multi-output cells like `Console -> line: String, line_id: Int`, the type isn't structurally resolvable as a let-binding type.

**Fix**: Ensure `check_cell_definition` registers the structural tuple type, and `infer_expression` returns `Type::Tuple(...)` for multi-output cells.

**File**: `src/typechecker.rs` — `Expr::CellCall` inference (~line 2279)
**Est**: ~15 lines

### Gap 3: LLVM backend internal trigger codegen

**Root cause**: `emit_persistent_cell_ticks` (emit_toplevel.rs:1592), `emit_cell_thread` (1710), and `CellCall` codegen (emit_expr.rs:4156) don't emit IR for internal triggers before the transaction loop.

**Fix**: Add IR emission for `LinkRef::Stdin` at the start of each cell's convergence pass: call `@tty_read_key`, type-convert i64→i8 for Char, store to the cell's prefixed GEP slot.

**Files**: `src/backend/llvm/emit_toplevel.rs`, `src/backend/llvm/emit_expr.rs`
**Est**: ~40 lines

---

## Pre-Existing Officina Bugs Fixed

### Bug 1: `FieldAccess` — struct field via `ListIndex` returns wrong type

**Root cause**: `Expr::ListIndex` in the LLVM backend always returns
`TypedRegister { ty: Type::Int }`, even when the list's element type is
a struct like `UnderstandRule`. When `let rule = rules[i];` is emitted,
`let_binding_types["rule"]` stores `Type::Int` instead of
`Type::Custom("UnderstandRule")`. Subsequent `rule.slot_count` fails
because `FieldAccess` can't find the struct.

**Fix** (`src/backend/llvm/emit_expr.rs:2672`): After emitting the
ListIndex GEP+load, check the list expression's type. If it's
`Type::Applied("List", [el_ty])`, propagate `el_ty` as the result type
instead of defaulting to `Type::Int`.

**Secondary fix** (`src/backend/llvm/emit_toplevel.rs`): Register function
parameter names in `let_binding_types` so `defn foo(x: StructType)` can
access `x.field` without a `let` binding in between.

### Bug 2: Debug build stack overflow

**Root cause**: Debug builds have ~4x larger stack frames per function call
(debu-info, no inlining). The officina project's 14 modules + complex
expression trees create a call chain deep enough to overflow the default
2MB stack in debug mode but not in release mode.

**Fix** (`.cargo/config.toml`): Set linker stack size to 8MB for debug
builds. This matches the Linux default `ulimit -s` and gives headroom for
any project without changing the compiler's architecture.

### Files modified

| File | Change |
|------|--------|
| `.cargo/config.toml` | Added `-C link-args=-Wl,-z,stack-size=8388608` for debug builds |
| `src/backend/llvm/emit_expr.rs` | `ListIndex` returns element type from list's `Applied("List", [T])` |
| `src/backend/llvm/emit_toplevel.rs` | Register `defn`/`txn` parameter names in `let_binding_types` |

---

---

## Remaining Items (Post-Gap-Close)

### Item 1: Restore line buffering in `console.bv`

**What**: Change `&line = (String)raw;` back to `&line = line + (String)raw;`.
The string concat fix in the LLVM backend now handles this correctly.

**File**: `lib/std/console.bv` — 1 line change.

### Item 2: Fix officina `action` field on `Option<Match>`

**What**: `matched.action` fails because `matched: Option<Match>` doesn't forward
field access. Fix: add `unwrap#(matched)` before accessing fields, or teach the
LLVM backend `FieldAccess` code to unwrap `Option<T>` → `T` when the object
type is `Option<Custom(name))`.

**Two approaches**:
- **Source fix** (1 line in officina): `let m = unwrap#(matched); term m.action;`
- **Compiler fix** (~30 lines in `emit_expr.rs`): In the `FieldAccess` handler,
  if `obj_val.ty` is `Type::Option(inner)` or `Type::Custom("Option")`, check
  the Option's inner type and unwrap before field lookup.

### Item 3: `Value::Items(Vec<TopLevel>)` for `compile#()`

**Goal**: Allow macros to generate arbitrary top-level items
(`StateDecl`, `Definition`, `Transaction`, `TriggerBinding`, etc.)
via `compile#()`.

**Implementation**:
1. Add `Items(Vec<TopLevel>)` variant to the `Value` enum in `src/interpreter.rs`
2. Update `compile#()` handler to return `Value::Items(prog.items)` instead
   of filtering only `Statement` and `TriggerBinding` items
3. Add `value_to_items` in `src/features/macros/template.rs` —
   converts `Value::Items(items)` to `Vec<TopLevel>`; falls back to
   `value_to_statements` wrapped in `TopLevel::Statement` for other values
4. Update `expand_macro_calls_in_items` in `src/features/macros/expand.rs`
   to detect `Value::Items` return and inject items directly into the
   program (not wrapped in `TopLevel::Statement`)
5. Handle `Value::Items` in all `Value` match arms that need it —
   `value_to_statements` can convert items to statements lossily

**Files**: `src/interpreter.rs`, `src/features/macros/template.rs`,
`src/features/macros/expand.rs`, `src/ast.rs` (Value enum)

**Result**: `$!console_input("inp");` expands `trg inp: String @ Console!;` as
a `TopLevel::TriggerBinding` (not wrapped in `TopLevel::Statement`), and
`compile#("let x: Int = 0; defn foo() -> Int { term x; };");` generates
proper top-level items.

---

## Officina Source Fixes (2026-06-28)

### Bug 6: Missing `Option` import

Added `import "std/option";` to `rules.bv`, `understand.bv`, and `prompt.bv`.
Also copied `lib/std/option.bv` to the officina-cli project's `lib/std/` directory
and fixed wrong `uni` syntax (`=` → `->`).

### Bug 7: `split` name shadow

Renamed local `split(input)` to `words(input)` in `understand.bv`. Updated all
call sites (`tokenize` and pattern matching).

---

## `%t{N}` Duplicate Register Bug

### Symptoms

`opt/llc` rejects `officina.ll` with:
```
error: multiple definition of local value named 't26'
%t26 = add i64 0, 0
```

### Root Cause

The compiler emits `%t{N}` register names from `self.txn_counter` in `emit_expr.rs:30`.
When a callable txn returns a tuple, the destructuring code in `emit_stmt.rs:234` creates
`%td*` names but the subsequent `emit_expr` for the destructured variable can produce
a `%t{N}` name with a leading indent of `""` (empty). This causes two `%t26`
definitions in the same function — one with no indent (from the empty-indent path)
and one with `"  "` indent (from the normal path).

### Unsuccessful Fix Attempts

1. **Removing `txn_counter = 0` resets** (lines 932, 980, 1111) — didn't help because
   the duplicate is within a SINGLE function, not across functions.
2. **Removing `alwaysinline`** — not the cause, the collision is in the raw IR.
3. **Searching for empty-indent `emit_expr` calls** — no production code passes `""`.

### Next Steps

The fix requires finding the code path that emits `%t{N}` with empty indent.
Most likely in `emit_stmt.rs` or `emit_expr.rs` in the tuple destructure path
where `indent` is derived from a context that doesn't pass it through correctly.
Alternatively, a post-processing pass can scan the IR for duplicate `%t{N}` names
and rename them.

### Priority
2. **Item 2**: Unblocks officina compilation. Source fix is 1 line; compiler
   fix is cleaner but larger.
3. **Item 3**: Largest change, unlocks macro potential. Requires `Value` enum
   change (impacts ~35 match arms across codebase).

---

1. **Gap 1** (parser shorthand) — unlocks `trg inp @ Console!` syntax and `$!console_input` macro
2. **Gap 3** (LLVM codegen) — makes Console cell work in compiled binaries
3. **Gap 2** (type inference) — affects variable typing, lowest priority
