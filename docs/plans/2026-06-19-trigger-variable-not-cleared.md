# Plan: Keyboard Input — Immediate Fix + Macro Decorator Architecture

**Date:** 2026-06-19  
**Status:** Written (awaiting execution request)  

## Overview

Two-phased work: (A) immediate fixes for the officina character-repeat bug, (B) architectural addition to let macros emit top-level items so keyboard input becomes a one-line decorator.

---

## Phase A — Immediate Fixes (ready now)

### A1: `loop_engine.rs:1264-1272` — type-correct load/store in `step()`

`emit_trg_step` hardcodes `load volatile i64` / `store volatile i64` for all trigger fields regardless of their actual LLVM type in `%State`:

| Brief type | LLVM type |
|------------|-----------|
| `Char`     | `i32`     |
| `Bool`     | `i8`      |
| `String`   | `i8*`     |
| `Int`      | `i64`     |
| `Float`    | `float`   |

Match on `self.field_types[idx]` and emit the correct typed load/store. Three sub-locations: trigger volatile loads (1264-1272), dependency field loads (1318-1328), and proxy store (1329-1339).

### A2: `officina.bv:78-80` — clear trigger after consumption

Add `&keypress = '\0';` unconditionally before `term;` so the guard `[booted && keypress != '\0']` converges to false after one firing.

### A3: Verification

```bash
cargo test --lib
./target/release/brief-compiler build ~/Projects/officina-cli/officina.bv
printf "hello\x03" | timeout 3 ./officina
```

---

## Phase B — Macro Decorator Architecture

### B1: Architecture

**One macro pattern, one syntax**: `$!name { overrides? }` before a `rct txn` = decorator. The macro receives the following declaration's AST parts (name, guard, body) as implicit arguments and emits augmented top-level items.

### B2: AST changes (`src/ast.rs`)

| Change | Detail |
|--------|--------|
| `Value::TopLevels(Vec<TopLevel>)` | New Value variant — macro return type for top-level items |
| `Expr::QuoteTopBlock { items: Vec<TopLevel> }` | `quote_top { }` syntax inside macro bodies — produces top-level items |
| Named block args on MacroCall | `on_char: (ch) { ... }` in trailing `{ }` parsed as `HashMap<String, (Vec<String>, Block)>` |
| `TopLevel::DecoratedMacro` | Parser wires `$!name { overrides }` + following `rct txn` into one AST node: `DecoratedMacro(MacroCall, Box<TopLevel>)` |

### B3: Parser changes (`src/parser.rs`)

| Change | Detail |
|--------|--------|
| Decorator detection | In `parse_top_level()`, if `$!` call is followed by a declaration keyword (`rct`, `defn`), parse as `TopLevel::DecoratedMacro` |
| Named block args | Inside `$!name { }` trailing block, parse `label: (params) { body },` pairs instead of a single raw Block |
| `quote_top { }` | `QuoteTopBlock` parser — parses top-level items inside macro definitions |
| Param-aware `@` interpolation | Inside `quote_top { }`, the parser knows which identifiers are macro params. `@name` → interpolation; `@stdin` → literal (not a param, falls through to trigger source token). This is controlled by a `macro_param_names: Option<HashSet<String>>` flag on the parser context. |
| `gensym#()` in macro context | Already works — returns fresh `__gensym_N` identifiers |

### B4: Expander changes (`src/features/macros/expand.rs`)

| Change | Detail |
|--------|--------|
| `TopLevel::DecoratedMacro` expansion | Extract `rct txn`'s name, guard, body as AST; pass them as implicit args to the macro call; call macro expansion; splice returned `Vec<TopLevel>` into program |
| `Value::TopLevels` handling | If macro returns `Value::TopLevels(items)`, splice items directly (not wrapped in `TopLevel::Statement`) |
| `QuoteTopBlock` execution | Inside interpreter, `QuoteTopBlock` evaluates to `Value::TopLevels` |

### B5: Hygiene changes (`src/features/macros/hygiene.rs`)

Extend `apply_hygiene` to walk top-level items (not just statements), so `let trg_name = gensym#()` in macro body produces unique names.

### B6: `lib/std/tty.bv` — `keyboard_input` macro

```brief
macro keyboard_input(body, guard, name) {
    let trg_name = gensym#();
    term quote_top {
        trg @trg_name: Char @stdin#;
        rct txn @name [@guard && @trg_name != '\0']] {
            let __ch = @trg_name;
            [__ch == '\n'] { @on_enter; &@trg_name = '\0'; term; };
            [__ch == '\x7f'] { @on_backspace; &@trg_name = '\0'; term; };
            [__ch == '\x03'] { @on_ctrl_c; &@trg_name = '\0'; term; };
            [__ch != '\n' && __ch != '\x7f' && __ch != '\x03'] {
                @on_char(__ch);
                &@trg_name = '\0';
                term;
            };
            &@trg_name = '\0';
            @body
            term;
        };
    };
};
```

Defaults for each handler are defined inside the macro as fallback blocks:
- `on_char(ch)`: appends `(String)ch` to a state variable `current_input`
- `on_enter()`: submits current input, clears it
- `on_backspace()`: trims current_input by 1
- `on_ctrl_c()`: restores terminal, exits

### B7: User API

```brief
// With defaults only — works as-is:
$!keyboard_input
rct txn process_input [needs_redraw]] {
    &needs_redraw = true;
    term;
};

// With overrides:
$!keyboard_input {
    on_char: (ch) { &current_input = &current_input + (String)ch; },
    on_ctrl_c: () { tty_raw_mode#(false); term! -> exit#(0); },
}
rct txn process_input [needs_redraw]] {
    &needs_redraw = true;
    term;
};
```

### B8: What the user does NOT write

- No `trg keypress: Char @stdin#;` — macro gensyms it
- No `keypress != '\0'` guard — macro appends it
- No `&keypress = '\0'` — macro injects it
- No escape sequences (`\x7f`, `\x03`, `\n`) — macro's default handlers handle them
- No trigger variable name — macro gensyms it

### B9: Files changed

| File | Phase | Change |
|------|-------|--------|
| `src/backend/llvm/loop_engine.rs` | A | Type-correct step() loads/stores |
| `officina.bv` (user's project) | A | Add `&keypress = '\0'` |
| `src/ast.rs` | B | `Value::TopLevels`, `QuoteTopBlock`, named block args, `DecoratedMacro` |
| `src/parser.rs` | B | Decorator detection, `quote_top { }`, param-aware `@`, named blocks |
| `src/features/macros/expand.rs` | B | Decorator expansion, `TopLevels` splicing |
| `src/features/macros/template.rs` | B | `QuoteTopBlock` → `TopLevels` in interpreter |
| `src/features/macros/hygiene.rs` | B | Top-level item hygiene |
| `lib/std/tty.bv` | B | `keyboard_input` macro |
| `docs/architecture/features/macro.md` | B | Update architecture doc |

### B10: Verification

```bash
cargo test --lib                                          # all existing tests pass
# New test: decorator macro expands trg + augmented txn
cargo test --lib macro_decorator_test
# Build officina using $!keyboard_input decorator
./target/release/brief-compiler build officina.bv
printf "hello\x03" | timeout 3 ./officina                 # one "hello", no repeats
```

---

## Future Work: Per-Instance Reactive Structs

### Vision

Currently, a struct's `rct txn` is promoted once at compile time — all instances share one reactor-slot. The vision for independent reactivity per instance:

```brief
struct Enemy {
    hp: Int,
    position: Vec2,
    state: AIState,

    rct txn ai_tick [__ticker > 0]] {
        // reads/writes this instance's fields
        [state == Idle] { &state = Patrol; };
        [state == Patrol] { patrol(); };
        [hp <= 0] { spawn_particles(position); term!; };
    };
};

// Each instance gets its own state slot in the reactor:
let goblin = Enemy{ hp: 10, ... };
let orc    = Enemy{ hp: 20, ... };
// goblin.ai_tick and orc.ai_tick are independent reactor participants
```

### What would need to change

| Component | Change |
|-----------|--------|
| **State allocation** | Dynamic instance registry, not one static `%State` struct. Pool allocator or sparse set. |
| **Reactor dispatch** | Per-instance precondition evaluation. Linear scan → indexed dispatch for entity-scale counts. |
| **Trigger wiring** | `@stdin#` fan-out: one epoll fd, N listeners. Or per-instance trigger via a different mechanism (event bus, not epoll). |
| **Desugarer** | No longer promotes struct txns to global. Instance txns are registered at instantiation time. |
| **Backend** | Codegen for instance-aware state access. GEP through instance offset, not hardcoded field index. |

### Not needed yet

This is future work. The `$!keyboard_input` decorator (Phase B) works within the current single-instance reactor model. Per-instance reactivity only becomes relevant when we need hundreds of independently ticking entities — a game engine use case.

### Execution Order

1. Phase A fixes (proven, scoped, ready now)
2. Phase B architecture (requires AST + parser + expander + stdlib changes)
3. Future: per-instance reactive structs (when game-engine work begins)
