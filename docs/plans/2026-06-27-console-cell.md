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

rct txn __ci_inp [inp != ""]] {
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
    rct txn emit [raw != '\0' && raw == '\n']] {
        &line = buffer;
        &buffer = "";
        term;
    };

    // Backspace → trim last char
    rct txn backspace [raw != '\0' && raw == '\x7f']] {
        &prev_key = raw;
        [buffer :> Size > 0] {
            &buffer = buffer :> Slice(0, buffer :> Size - 1);
        };
        term;
    };

    // Printable → accumulate
    rct txn type [raw != '\0' && raw >= ' ' && raw != '\n' && raw != '\x7f']] {
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

  rct txn handle [inp_id != last_id]] {
      // inp_id has changed → new line available
      process_line(inp_line);
      &last_id = inp_id;
      term;
  };
  ```

  The `$!console_input` macro would generate TWO trg bindings.

### 2. `$!console_input` Macro

A `$!macro` that generates top-level items (trg binding + rct txn handler).

**Macro definition** (placed in `lib/std/console.bv`):

```brief
macro console_input(line_var: String, body: Block) -> Block {
    quote {
        trg @line_var: String @ Console!;

        rct txn __ci_@line_var [@line_var != ""]] {
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
| `rct txn` inside `quote` doesn't parse as `Statement` | Macro can't generate handler | Split: macro only generates `trg` binding |
| `trg @ Console!` shorthand fails in `Statement::TrgBinding` | Shorthand only works at top level | Macro generates `let __inst = cell Console; trg @ __inst.line;` |
| `@"$!Console"` string interpolation for cell name fails | Can't generate cell name in quote | Use explicit cell reference |

### 3. Duplicate-Line Handling

The `$!console_input` macro must handle the case where the user enters the same input twice.

**Approach**: Use `line_id` counter. The macro generates:

```brief
trg @line_var: String @ Console!;

rct txn __ci_@line_var [@line_var != ""]] {
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

rct txn handle [inp_id != last_inp_id]] {
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
2. For each rct txn: check precondition → execute body → check postcondition
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

## Risk Register

| Risk | Impact | Mitigation |
|------|--------|------------|
| Internal `trg` not evaluated during cell ticks | Cell can't self-poll stdin | Add eval loop in `tick_persistent_cells` |
| `$!macro` can't generate top-level items | Macro approach fails | Fall back to documented 5-line pattern |
| `rct txn` inside `quote` doesn't parse as statement | Handler can't be macro-generated | Split: macro generates trg only, user writes handler |
| Duplicate-input loss despite line_id counter | Incorrect behavior | Ensure output comparison uses the latest cell output, not cached |
