# Plan: `#!exit` Safety Warnings & Errors

**Date:** 2026-06-01
**Status:** Planned → In Progress

## Motivation

The `#!exit` mechanism added in the same session has silent failure modes:

1. **Unknown identifiers** → silently coerced to 0 (exit condition never true → infinite loop)
2. **One-shot programs** → exit check code never emitted (dead code, user thinks it works)
3. **No exit path at all** → no warning when program will idle forever after convergence

Three diagnostics fix this:

| # | Severity | Message | Location |
|---|----------|---------|----------|
| 1 | **Error** | `error: #!exit references unknown variable 'X'` | `emit_exit_expr`: identifiers not in `field_index_map` or `constants` |
| 2 | **Warning** | `warning: #!exit declared but program has no wake loop — condition never checked` | `generate()` or `emit_enum_main` when `!has_wake && exit_condition.is_some()` |
| 3 | **Warning** | `warning: program has wake triggers but no exit path — will idle forever after convergence` | `generate()` when `has_wake_triggers && exit_condition.is_none()` |

## Implementation

### 1. Unknown identifier error

`emit_exit_expr` currently writes `add i64 0, 0` for unknown identifiers.
Instead: store a `Vec<String>` of unknown exit identifiers, emit error after generation.

**Problem**: `emit_exit_expr` is called during code generation, where we can't easily
return an `Err`. It emits LLVM IR directly. But the identifier is a simple lookup —
we can verify ALL identifiers in the exit condition upfront in `generate()`.

**Approach**: Add a pre-check for exit condition identifiers in `generate()`:

```rust
// After setting self.exit_condition, verify all identifiers exist
if let Some(ref cond) = self.exit_condition {
    self.check_exit_condition_idents(cond);
}

fn check_exit_condition_idents(&self, expr: &Expr) {
    match expr {
        Expr::Identifier(name) => {
            if !self.field_index_map.contains_key(name)
                && !self.constants.contains_key(name)
            {
                eprintln!("error: #!exit references unknown variable '{}'", name);
                std::process::exit(1);
            }
        }
        Expr::Eq(l, r) | Expr::Ne(l, r) | Expr::Lt(l, r) | Expr::Le(l, r)
        | Expr::Gt(l, r) | Expr::Ge(l, r) | Expr::And(l, r) | Expr::Or(l, r) => {
            self.check_exit_condition_idents(l);
            self.check_exit_condition_idents(r);
        }
        Expr::Not(e) => self.check_exit_condition_idents(e),
        _ => {} // Integer, Bool, etc. are fine
    }
}
```

### 2. One-shot warning

In `generate()`, after the dispatch path is selected:
- If `exit_condition.is_some()` and the program goes through a one-shot path
  (either `emit_folded_main`, `emit_precomputed_main`, or `emit_enum_main` with `!has_wake`):

```rust
if self.exit_condition.is_some() {
    if folded || precomputed || (!has_wake_triggers && !enumerable.is_some()) {
        // One-shot path — #!exit is unreachable
    }
}
```

More precisely:
- **Folded path** (single counter, while-loop, then `ret i32 0`): one-shot. `#!exit` never reached.
- **Precompute path** (stores + `ret i32 0`): one-shot. `#!exit` never reached.
- **Enum dispatch without wake**: each case arm `ret i32 0`. `#!exit` check never emitted.
- **Standard reactor without wake**: `emit_main` with `has_wake_triggers=false` — this actually DOES emit the exit check (my implementation in `emit_main` doesn't gate on `has_wake_triggers`). But it's still a one-shot program that spins forever with no `__rt_wait()`. The exit check is the ONLY thing that can stop it.

Wait — for `emit_main` with `has_wake_triggers=false` and no exit check, the program spins in `tick: ... br label %tick`. That's a busy-loop with no wait. This path is only reached when there are no wake triggers (line 762). This is essentially a degenerate case — the program has no way to make progress between ticks.

So the warning should be:
- If `exit_condition.is_some()` AND the selected dispatch path NEVER checks it:
  - Folded one-shot: `emit_folded_main` exits before any loop
  - Precompute one-shot: `emit_precomputed_main` exits before any loop  
  - Enum dispatch without wake: `emit_enum_main` doesn't emit `exit_check`
  - Standard reactor without wake: `emit_main` DOES check it (line 2120 code path), so NO warning here

The clearest approach: warn when `has_wake_triggers == false && exit_condition.is_some()`.
The exception is the standard-reactor-no-wake path which DOES check it, but that program
spins in a busy loop which is fundamentally unhealthy anyway.

Actually, the simplest correct approach: warn in `generate()` when we know the exit check won't be emitted:

```rust
// After dispatch path selection:
if exit_condition.is_some() {
    let will_check = has_wake_triggers  // emit_main (wake) always checks
        || true;  // emit_main (no wake) also checks
    // Only enum dispatch without wake skips the check
    if let Some(ref _en) = enumerable {
        if !has_wake_triggers {
            eprintln!("warning: #!exit declared but program has no tick loop (one-shot enum dispatch)");
        }
    }
}
```

Hmm, this gets complicated. Let me simplify: just warn whenever `!has_wake_triggers && exit_condition.is_some()`, UNLESS we're on the `emit_main` path (which handles it). The `emit_main` path is only reached when `!folded && !precomputed && !enumerable`.

```rust
let is_one_shot = !has_wake_triggers;
let uses_enum = enumerable.is_some();
let is_folded_or_precomputed = folded || precomputed_final_values.is_some();

if is_one_shot && exit_condition.is_some() {
    if is_folded_or_precomputed || uses_enum {
        eprintln!("warning: #!exit declared but program has no tick loop — condition never checked");
        eprintln!("  help: add a @link trigger to make it reactive, or remove #!exit");
    }
    // emit_main (no wake) does check exit_condition, so no warning there
}
```

### 3. No-exit-path warning

In `generate()`, after the dispatch path is selected:

```rust
if has_wake_triggers && exit_condition.is_none() {
    eprintln!("warning: program has wake triggers but no #!exit — will idle forever after convergence");
    eprintln!("  help: add `#!exit <condition>;` at the top of the file");
}
```

This covers all wake programs (standard reactor, enum dispatch, async) that don't have an exit condition.

## Files Changed

- `src/backend/llvm.rs:generate()` — all three diagnostics
- `src/backend/llvm.rs` — new `check_exit_condition_idents()` method

## Tests

Update existing exit condition tests to:
- Verify warning text for one-shot programs
- Verify error text for unknown identifiers
- Verify warning text for no-exit-path programs
