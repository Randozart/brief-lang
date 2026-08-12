# Plan: Fix Benchmark Syntax, Parser Error, and `is_trigger_gated` Gap

**Date:** 2026-06-02  
**Status:** Plan → Implementation

## Motivation

Three calibration benchmarks (`float_math.bv`, `const_heavy.bv`, `sparse_dispatch.bv`) have syntax errors that prevent compilation: missing `};` after `node` blocks, missing `;` on `let` declarations, missing `[postcondition]` contracts. Additionally, the parser error for a missing `;` after `node { body }` is cryptic: `expected Semicolon, found end of file at 0:0`.

Worse: a compiler gap in `is_trigger_gated()` at `src/backend/llvm.rs:139` prevents the enum dispatch optimizer from recognizing preconditions with `Eq(trigger, literal)` patterns (e.g., `t == 101`). This means any dispatch benchmark using `trigger == value` syntax never enters the enum dispatch path — a critical optimization gap that defeats Briev's declarative advantage.

## Tasks

### Task 1 — Better Parser Error (`src/parser.rs:2678`)

Replace generic `self.expect(Token::Semicolon)?;` with a custom match providing a clear, actionable error message. Capture the `}` span at line 2634 so the error shows the correct source location even at EOF.

**Before:** `expected Semicolon, found end of file at 0:0`  
**After:** `expected ';' after node block — all node declarations must end with '};', at 55:1`

### Task 2 — Fix `benchmarks/float_math.bv`

| Line | Issue | Change |
|------|-------|--------|
| 41 | Missing postcondition | `[x0 == x0]` → `[x0 == x0][count == total]` |
| 55 | Missing `;` after `}` | `}` → `};` |

### Task 3 — Fix `benchmarks/const_heavy.bv`

| Line | Issue | Change |
|------|-------|--------|
| 21 | Missing postcondition | `[count < total]` → `[count < total][count == total]` |
| 27 | Missing `;` after `}` | `}` → `};` |

### Task 4 — Fix Compiler Gap (`src/backend/llvm.rs:139`)

`is_trigger_gated()` currently matches only `Expr::Identifier` and `Expr::And`. Add `Expr::Eq(l, r)` arm to recognize trigger comparisons:

```rust
Expr::Eq(l, r) => {
    matches!(l.as_ref(), Expr::Identifier(name) if trigger_names.contains(name.as_str()))
        || matches!(r.as_ref(), Expr::Identifier(name) if trigger_names.contains(name.as_str()))
}
```

Without this, any txn with precondition `t == 101` is invisible to the enum dispatch optimizer. This was the root cause of `is_trigger_gated` failing for Int-triggered dispatch.

### Task 5 — Redesign `benchmarks/sparse_dispatch.bv` (Briev-Native)

The original benchmark tried to mimic C's `io_pending = keys[idx % 8]` pattern — impossible in Briev because `io_pending` is a `trg` (read-only OS event flag). Replace with a Briev-native cyclic dispatch:

```briev
#!exit count == total;

import { io_pending } from "std/briev_rt.bv";
import "link/briev_rt.o";

let count: Int = 0;
let total: Int = __get_env_int("BOUND");

node ping [io_pending && count % 8 == 0][count == total] { &count = count + 1; };
node ack  [io_pending && count % 8 == 1][count == total] { &count = count + 1; };
node err  [io_pending && count % 8 == 2][count == total] { &count = count + 1; };
node debug[io_pending && count % 8 == 3][count == total] { &count = count + 1; };
node data [io_pending && count % 8 == 4][count == total] { &count = count + 1; };
node ctrl [io_pending && count % 8 == 5][count == total] { &count = count + 1; };
node sync [io_pending && count % 8 == 6][count == total] { &count = count + 1; };
node stat [io_pending && count % 8 == 7][count == total] { &count = count + 1; };
```

Exercises: 8 precondition evaluations per tick, exactly 1 fires via `(count % 8) == N`, cyclic dispatch. `io_pending` is the Bool trigger (region analyzer size=2). Enum dispatch activates. The C equivalent is a straightforward `switch (count % 8)`.

### Task 6 — Update C Reference `benchmarks/sparse_dispatch_c.c`

Match the new benchmark design — direct `count % 8` switch, no key-driven io_pending:

```c
while (count < bound) {
    switch (count % 8) {
        case 0: break; ... case 7: break;
    }
    count++;
}
```

### Task 7 — Verify

1. `cargo build --release`
2. `cargo test --lib` (all 368+ tests pass — verify `is_trigger_gated` change doesn't break existing tests)
3. `bash benchmarks/build_and_bench.sh float_math`
4. `bash benchmarks/build_and_bench.sh const_heavy`
5. `bash benchmarks/build_and_bench.sh sparse_dispatch`
6. Manual test: broken `.bv` file produces new clear error message

## Risk Assessment

- **`is_trigger_gated` change**: Adding `Expr::Eq` matching changes enum categorization. Existing tests use `Identifier` and `And` preconditions — should be unaffected. But any test with `Eq(trigger, literal)` preconditions will now be classified differently. Check `git diff` on test outputs.
- **sparse_dispatch.bv redesign**: Complete rewrite, but the old file couldn't compile anyway. No regression risk.
