# Plan: Keyboard Input — Immediate Fix + Macro Decorator Architecture

**Date:** 2026-06-19  
**Status:** Written (awaiting execution request)  

## Overview

Three-phased work: (A) immediate fixes for the officina character-repeat bug, (B) macro decorator architecture for painless keyboard input, (C) interactive-only spurious epoll wakeup fix.

---

## Phase A — Immediate Fixes (committed at 28e2195)

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

### Phase C — Spurious Epoll Wakeup Guard (committed)

### C1: Root cause

The stdin handler in `emit_trg_event_epoll_wait` (`loop_engine.rs:1436`) discards `read()`'s return value:

```rust
// loop_engine.rs:1436 — return value assigned but NEVER checked:
writeln!(out, "  %rd_{}_{} = call i64 @read(i32 0, i8* {}, i64 1)", ...)
// ...
// loop_engine.rs:1440-1441 — loads garbage if read() returned -1 or 0:
let ch_ld = format!("%chld_{}_{}", tc, name);
writeln!(out, "  {} = load i8, i8* {}, align 1", ch_ld, ch_slot)
```

On some Linux kernels, `epoll_wait` on a TTY fd with `O_NONBLOCK` + raw mode can return spurious wakeups. `read()` returns `-1/EAGAIN`, but the handler stores the **uninitialized `alloca` garbage** into the `keypress` state field. This garbage is non-zero, so `process_input` fires with it every tick — creating a tight 100% CPU loop.

When the user presses Enter, the byte `\n` finally succeeds on `read()`, triggering `[k == '\n'] { &running = false; }` which causes the render txn to exit the program.

### C2: Fix — `loop_engine.rs:1432-1484`

**Before**: All trigger arms (`Stdin`, `Timer`, `Signal`, `_`) share a common `step()` + `br %t_skip` after the match block. `Stdin` never validates `read()`'s result.

**After**: Each arm gets its own `step()` + `br %t_skip`. `Stdin` checks `read() > 0` before storing the byte or calling `step()`.

| Arm | Change |
|-----|--------|
| `Stdin` | After `read()`: `icmp sgt i64 %rd, 0` → if true, store byte + step() + br t_skip; if false, br t_skip (skip store and step entirely) |
| `Timer` | Add `step()` + `br %t_skip` inside arm (moved from post-match) |
| `Signal` | Add `step()` + `br %t_skip` inside arm (moved from post-match) |
| `_` | Add `step()` + `br %t_skip` inside arm (moved from post-match) |
| Post-match (lines 1480-1484) | Remove step/br lines, keep only `t_skip:` label |

**Stdin arm code after**:

```rust
crate::ast::LinkRef::Stdin => {
    let ch_slot = format!("%ch_{}_{}", tc, name);
    writeln!(out, "  {} = alloca i8, i64 1, align 1", ch_slot).ok();
    let rd_res = format!("%rd_{}_{}", tc, name);
    writeln!(out, "  {} = call i64 @read(i32 0, i8* {}, i64 1)", rd_res, ch_slot).ok();
    let rd_ok = format!("%rdok_{}_{}", tc, name);
    writeln!(out, "  {} = icmp sgt i64 {}, 0", rd_ok, rd_res).ok();
    let store_lbl = format!("rds_{}_{}", tc, name);
    writeln!(out, "  br i1 {}, label %{}, label %{}", rd_ok, store_lbl, t_skip).ok();
    writeln!(out, "{}:", store_lbl).ok();
    if let Some(&idx) = backend.field_index_map.get(name) {
        // ... existing typed store logic unchanged ...
    }
    let drx = format!("%drx_{}_{}", tc, name);
    writeln!(out, "  {} = add i64 {}, {}", drx, 1u64 << bit, bit).ok();
    writeln!(out, "  call void @step(%State* %state, i64 {})", drx).ok();
    writeln!(out, "  br label %{}", t_skip).ok();
}
```

### C3: What happens on a spurious wakeup

1. `epoll_wait` returns (spurious — no actual data on fd 0)
2. Handler matches bit → enters Stdin arm
3. `read(0, &ch, 1)` returns -1 (EAGAIN)
4. `icmp sgt i64 %rd, 0` → false
5. `br %t_skip` → **skip store, skip step()**
6. Back to `tick:` → guard `keypress != '\0'` still false (unchanged)
7. `epoll_wait` called again — blocks normally

On a REAL wakeup (user types a key), `read()` returns 1, store+step+process_input fire normally. One character, one fire, no repeat.

### C4: Verification

```bash
cargo test --lib             # all tests pass
./target/release/brief-compiler build ~/Desktop/Projects/officina-cli/officina.bv
# Interactive: run ./officina, type characters — no repeats, no freeze
printf "hello\x03" | timeout 3 ./officina    # pipe test still works
```

### Execution Order

1. ~~Phase A fixes (committed at 28e2195)~~
2. ~~Phase C — spurious epoll guard (committed)~~
3. ~~Phase D — `const trg` design (committed at 5e9d757)~~
4. Phase B architecture (requires AST + parser + expander + stdlib changes)
5. Future: per-instance reactive structs (when game-engine work begins)

---

## Phase D — `const trg` Design

### Philosophy

A `trg` is a **mailbox** — the outside world drops a value, Brief reads it. Writing to the `trg` variable mutates the local copy; the external source neither sees nor cares.

For software triggers (`@stdin#`), writing is fine — you consumed the event and clear the latch. For hardware triggers (`@0x...`, `@mmio`), the register is **sovereign** — the program must never pretend to own what the hardware holds.

Hence `const trg` exists: "I, the code, cannot mutate this."

### Syntax

```brief
trg  keypress: Char @stdin#;            // software — writable by code
const trg status: Int @0xFFFF0000;      // hardware — read-only from code
```

`trg` without `const` = mutable local copy (software triggers).
`const trg` = read-only (hardware triggers, or any trigger you want to guard).

### Compiler Errors

| Code | Error |
|------|-------|
| `&const_trg_name = expr` | `"cannot write to const trigger 'name'"` |
| `trg name @{literal} ...` (address literal without `const`) | `"hardware-addressed triggers must be declared 'const trg'"` |

The first catches any write to a const trigger. The second catches the common embedded bug: declaring a hardware trigger as mutable, then writing to the shadow field thinking you're writing to the register.

### Rendered Brief (`.rbv`)

Same rule applies. a front-end button press emits a one-time signal into a `trg`. Brief can read and optionally write back (`&trg = ...`), but the front end never listens. The mailbox is one-directional regardless.

### Implementation

| Change | File | Detail |
|--------|------|--------|
| AST | `src/ast.rs` | Add `is_const: bool` to `TriggerDeclaration` |
| Parser | `src/parser.rs` | Accept `const` before `trg` in `parse_trigger()` |
| Resolver/Analysis | `src/analysis/` | Validate address-bound triggers have `const` |
| Codegen | `src/backend/llvm/emit_stmt.rs` | Error on write to `const` trigger in assignment |
| Docs | `docs/architecture/glossary.md` | Document `const trg` |
| Plan | `docs/plans/...` | This document |
