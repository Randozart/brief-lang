# Make `wake` the Default for All Triggers

**Date:** 2026-06-24
**Status:** Plan

## Current State

| Trigger Source | Default `is_wake` | `#wake` modifier | 
|---|---|---|
| `@ link <symbol>` | ✅ true | allowed (no-op) |
| `@ stdin#` | ✅ true | N/A (builtin) |
| `@ timer#(hz)` | ✅ true | N/A (builtin) |
| `@ signal#(name)` | ✅ true | N/A (builtin) |
| `@ <MMIO addr>` | ❌ **false** | ❌ Errors: "redundant" |

The inconsistency: MMIO is the only trigger that defaults to non-waking. There's no way to make an MMIO trigger wake.

## Proposal

1. **Default `is_wake = true` unconditionally** — removes the conditional logic
2. **Introduce `#nowake` modifier** — for the edge case of read-only/passive MMIO
3. **Remove the `#wake` modifier** — no longer needed (it's the default)
4. **Update codegen** — `emit_trg_event_epoll_wait()` must handle MMIO addresses

## Changes

### `src/parser.rs` — 3 changes

**1. Default `is_wake` to `true` (line ~3923):**
```rust
// Before:
let is_builtin = matches!(address, LinkRef::Stdin | LinkRef::Timer(_) | LinkRef::Signal(_));
let mut is_wake = matches!(address, LinkRef::Linked(_)) || is_builtin;

// After:
let mut is_wake = true;
```

**2. Replace `#wake` modifier with `#nowake` (lines ~3926–3942):**
```rust
// Before:
if !is_builtin {
    if let Some(Ok(Token::Hash)) = self.current_token() {
        // parse #wake → is_wake = true
    }
}
if is_wake && matches!(address, LinkRef::Explicit(_)) {
    return self.spanned_err("MMIO triggers are natively wake-capable; #wake is redundant");
}

// After:
if let Some(Ok(Token::Hash)) = self.current_token() {
    if let Some(Ok(Token::Identifier(n))) = self.current_token() {
        if n == "nowake" {
            self.advance();
            is_wake = false;
        } else {
            return self.spanned_err("Expected #nowake".to_string());
        }
    }
}
```

Note: `#nowake` applies to ALL trigger types, not just MMIO. A `@ link` can also be `#nowake`.

**3. Update parser tests:**
- `test_wake_modifier` → `test_nowake_modifier` — parses `trg x: Bool @ link __x #nowake;`, asserts `is_wake == false`
- `test_wake_on_mmio_error` → remove (MMIO with `#nowake` is now valid)
- Add `test_wake_default` — parses `trg x: Bool @ 0x4000;`, asserts `is_wake == true`

### `src/backend/llvm/loop_engine.rs` — epoll-wait for MMIO

`emit_trg_event_epoll_wait()` (line 1676) currently only handles Stdin/Timer/Signal. MMIO addresses fall through to a plain `step()` call (line 1774). Since MMIO now defaults to wake:

- If the MMIO is on a memory-mapped address that supports interrupts (e.g., GPIO rising edge), epoll won't work for bare addresses — they're not file descriptors.
- **Fix**: Add a `LinkRef::Explicit(_)` branch that calls `__rt_wait()` (a blocking sleep) when only MMIO wake triggers exist. This prevents busy-waiting.

```rust
LinkRef::Explicit(addr) => {
    // MMIO wake triggers can't use epoll — fall back to blocking wait
    writeln!(out, "{}call void @__rt_wait()", indent).ok();
}
```

### `src/backend/llvm/emit_expr.rs` — wake metadata

`emit_wake_metadata()` (line 4599) already correctly filters to only `LinkRef::Linked` symbols for the `@llvm.wake_triggers` metadata. MMIO triggers won't appear there, which is correct (MMIO doesn't need `-lrt`).

### `src/backend/llvm/optimizer.rs` — no change needed

`has_wake_triggers = triggers.values().any(|t| t.is_wake)` works identically — just the default changed from `false` to `true` for MMIO.

### `src/backend/circt.rs` — propagate the default

The CIRCT backend already propagates `is_wake` correctly. Test helpers that set `is_wake: false` may need updating if they test MMIO triggers.

### Test helpers (analysis/*.rs, reactor.rs, fuzzing)

All `make_trigger()` / `arb_trigger_decl()` helpers create triggers with `is_wake: false` by default. These should be updated to `is_wake: true` to match the new parser default. Tests that specifically test non-wake behavior should use `is_wake: false` explicitly with a comment.

### LLVM backend tests (src/backend/llvm/tests.rs)

21 wake-related tests exist (see exploration). About half test `is_wake = true` scenarios (which still work) and half test `is_wake = false` scenarios (which need updating since the default changes).

Tests to update: any that set `is_wake = false` to test non-wake behavior now need to use `#nowake` in their trigger declaration or create the TriggerDeclaration with `is_wake: false` explicitly.

## Migration — No Deprecation Window

Since `#wake` was never widely used (it was redundant for most trigger types and errored on MMIO), there's no real migration burden. The only change users will see is:

1. MMIO triggers now wake the reactor by default (correct behavior)
2. `#wake` keyword is removed (it's the default)
3. `#nowake` keyword available for passive MMIO reads

## File Manifest

| File | Change |
|------|--------|
| `src/parser.rs` | Default `is_wake = true`, replace `#wake` with `#nowake` |
| `src/backend/llvm/loop_engine.rs` | Add MMIO path in `emit_trg_event_epoll_wait()` |
| `src/backend/llvm/tests.rs` | Update ~21 tests for new default |
| `src/reactor.rs` | Update test helper `is_wake: false` → `true` |
| `src/analysis/watchdog.rs` | Update `make_trigger()` default |
| `src/analysis/region.rs` | Update `make_trigger()` default |
| `src/analysis/dependency_graph.rs` | Update `make_trigger()` default |
| `src/fuzzing/ast_generator.rs` | Update `arb_trigger_decl()` default |
| `src/backend/circt.rs` | Update test helper default |
| `docs/architecture/features/trg-dirty-flag.md` | Update wake section to reflect new default |
| `AGENTS.md` | Update if needed |

## Test Plan

- `cargo test --lib` — all existing tests pass after updating defaults
- Add new tests: `test_nowake_modifier`, `test_wake_default`, `test_mmio_nowake`
- LLVM codegen tests verify MMIO wake triggers produce `__rt_wait()` instead of busy-loop
