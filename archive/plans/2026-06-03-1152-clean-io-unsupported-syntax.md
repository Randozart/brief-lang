# Clean Unsupported Syntax from lib/std/io.bv

**Timestamp**: 2026-06-03 11:52
**Status**: Done

## Problem
`lib/std/io.bv` contains unsupported, unimplemented syntax that no Briev parser handles:

| Construct | Lines | Problem |
|-----------|-------|---------|
| `guard [cond] { ... }` | 122 | Intended as while-loop. No parser supports `guard` keyword. |
| `if cond { ... }` | 123, 132 | Intended as conditional. No parser supports `if` keyword. |
| `Vector<u8>` | 67 | `Vector` type has no definition anywhere in stdlib or compiler. |

RBV is just HTML `<script>` + `<view>` wrapping the same `.bv` parser (`src/rbv.rs:44-59`). No parser in this codebase handles `guard` or `if` keywords.

## Design Decision

**Remove convenience lookup functions entirely.** Users gate on `io_ready` and compose with `std/string.bv` methods in their own handlers. This is both more performant (O(1) per tick) and more Briev-idiomatic (the user owns handler logic; the IO module provides infrastructure).

### Why not txn-based scan
A reactive txn scanning `__io_buffer` character-by-character costs 1 reactor tick per char + 3 state fields. The buffer from `__raw_poll()` is small (a few events), so this overhead is disproportionate. Users know what key they're looking for and can `char_at()` directly.

### Why not `contains()`
Wrong abstraction — `__io_buffer` contains raw event data, not text strings. `contains("Space")` would conflate key names with raw scan codes.

## Changes

### 1. `__io_buffer` type: `Vector<u8>` → `String`
- `__raw_poll()` already returns `String`
- `String` has `char_at()`, `len()` — sufficient for event buffer inspection
- No undeclared types remain

### 2. Remove `key_pressed` (lines 120-128)
- Used `guard` (while-loop) and `if` (conditional)
- Users write their own handler: `[char_at(io.__io_buffer, 0) == 'a'] { ... };`

### 3. Remove `get_char` (lines 131-136)
- Used `if` (conditional)
- Users write: `let c = char_at(io.__io_buffer, 0);`

### 4. Fix imports
- Add `import { char_at, len } from "std/string.bv";` if any internal functions need them
- The pump txn itself doesn't use these — only the removed convenience functions did

## Result Matrix

| Benchmark | Status |
|-----------|--------|
| All 372 tests | Must pass |
| `lib/std/io.bv` | Must parse (no `guard`/`if`/`Vector<u8>`) |
| `print_loop.bv` | Unaffected (uses `frgn __print_int` directly) |
| IO infrastructure | `__io_pump`, `__io_sleep`, `io_ready`, `consume()` preserved |

## Implementation
1. Change `__io_buffer` type from `Vector<u8>` to `String`
2. Remove `key_pressed` function (lines 120-128)
3. Remove `get_char` function (lines 131-136)
4. Update docs/usage example at top of file
5. Verify `cargo test --lib` passes
6. Update AGENTS.md Known Issue section
