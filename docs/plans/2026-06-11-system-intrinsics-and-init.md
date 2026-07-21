# System Intrinsics, Top-Level `__init`, and Universal Bracket

**Date:** 2026-06-11 14:00 UTC

## Overview

Three passes of additive language features:

- **Pass A:** 14 new intrinsics (syscall layer + data layer), bringing total from 15 → 29
- **Pass B:** Top-level `Statement` → implicit `__init` transaction
- **Pass C:** Universal Bracket Syntax (from `docs/plans/2026-06-11-universal-bracket.md`)

---

## Pass A: 14 New Intrinsics

### Design Rule

> If LLVM handles it best, make it an intrinsic. If a `frgn` declaration suffices, keep it in stdlib.

### Syscall Layer (11)

| Intrinsic | Signature | LLVM emission | Why intrinsic, not frgn |
|-----------|-----------|---------------|------------------------|
| `println#(val)` | `T -> Bool` | `printf` with per-type format specifier | Type→format dispatch; auto-declares `printf` |
| `readln#()` | `-> String` | `fgets(stdin)` / `syscall read(0)` | Stdin buffer managed; target-aware |
| `exit#(code)` | `Int -> !` | `syscall exit(60)` | `noreturn` — affects CFG, dead-code elimination |
| `time#()` | `-> Int` | `clock_gettime` / `rdtsc` | Target chooses the timer |
| `read_file#(path)` | `String -> Result<String, Int>` | `open` + `mmap`; constant path → preload at compile time | Compile-time constant path → embed in binary |
| `write_file#(path, data)` | `String, String -> Result<Int, Int>` | `open` + `write` | Constant path → compile-time existence check |
| `sleep#(ms)` | `Int -> Bool` | `nanosleep` / `Sleep` | Constant 0 → eliminate entirely at compile time |
| `socket#(domain, type, protocol)` | `Int, Int, Int -> Int` | `syscall socket(41)` | Platform sockaddr layout, syscall emission |
| `bind#(fd, port)` | `Int, Int -> Bool` | `syscall bind(49)` | Intrinsic avoids libc linking |
| `listen#(fd, backlog)` | `Int, Int -> Bool` | `syscall listen(50)` | Same |
| `accept#(fd)` | `Int -> Int` | `syscall accept(43)` | Same |

### Data Layer (3)

| Intrinsic | Signature | LLVM emission | Why intrinsic |
|-----------|-----------|---------------|---------------|
| `sort#(list)` | `List<T> -> List<T>` | `qsort` call or SIMD sorting network | 15+ line convergent `txn` vs one call |
| `reverse#(list)` | `List<T> -> List<T>` | Tight `for` loop, in-place swap | Verbose loop pattern |
| `range#(end)` | `Int -> List<Int>` | Constant `end` → `.rodata` array; runtime → allocated loop | Compile-time constant precomputation |

### Total: 29 intrinsics (15 existing + 14 new)

### Existing 15
sqrt, fabs, ceil, floor, ctpop, ctlz, cttz, abs, bitreverse, pop, size, bytes, contains, keys, values

---

## Pass B: Top-Level `__init`

### Motivation

Top-level statements (e.g., `println#("hello")`) should be allowed at the global scope. They desugar to a single implicit `node __init` that fires once at program start. This provides:

1. **Low-friction scripting**: write statements directly without wrapping in a `main` txn
2. **Atomic booting**: the entire startup is a transaction — `escape` aborts cleanly with zero partial state
3. **Compiler-enforced safety**: declarations must precede statements; no interleaving

### AST

```rust
pub enum TopLevel {
    // ...
    Statement(Box<Statement>),
}
```

### Parser Rules

1. All `TopLevel::Let`, `TopLevel::Const`, `TopLevel::Struct`, `TopLevel::Enum`, `TopLevel::Defn`, `TopLevel::Txn`, `TopLevel::ForeignBinding` declarations must precede any `TopLevel::Statement`
2. A `Statement` after the first `Statement` is sequential execution
3. A declaration after a `Statement` is a compile error: "Declarations must precede executable statements"

### Pass 2 Synthesis

The compiler collects all `TopLevel::Statement` nodes in program order and synthesizes:

```brief
let __booted_N: Bool = false;

node __init [!__booted_N][__booted_N] {
    // all top-level statements in order
    &__booted_N = true;
    term;
};
```

Where `__booted_N` uses N = first integer where `__booted_N` is not already declared (collision avoidance: check 0, 1, 2, ...).

### Semantics

- `escape` inside top-level code → clean atomic boot abort: all state rolled back, program never partially configured
- `term!` inside top-level code → compile error (program exit is a txn concern)
- Top-level `Statement` can use `let` for local bindings within the `__init` body (scope is the synthesized txn)
- FFI calls (`println#`, `read_file#`, etc.) inside `__init` are fine and expected

---

## Pass C: Universal Bracket

See `docs/plans/2026-06-11-universal-bracket.md` for the full spec.

**Summary:**
- Bracket syntax works on every type, decomposing to `Char` fragments for atomics
- `@"pattern"` regex literal → `Expr::RegexLiteral` → DFA-compiled at parse time
- `BracketOp::Mask` evaluates both boolean predicates and regex values
- Type-directed desugar: string literal in bracket on atomic type → regex filter

---

## File Change Summary

### Per Intrinsic
| File | Change |
|------|--------|
| `src/ast.rs` | `Intrinsic` enum: +14 variants |
| `src/interpreter.rs` | `Expr::IntrinsicCall` match arms: +14 eval branches |
| `src/typechecker.rs` | Intrinsic type signatures: +14 entries |
| `src/backend/llvm/emit_expr.rs` | `Intrinsic::*` codegen: +14 emission branches |
| `src/backend/llvm/emit_toplevel.rs` | `emit_declares`: +11 syscall declares (printf, etc.) |
| All match-arm files | +14 match arms (or 1 wildcard if already handled) |

### Per Pass

**Pass A:** ~12-15 files modified
**Pass B:** `ast.rs`, `parser.rs`, `pass2` / `codegen` module, `spec/SPEC.md`
**Pass C:** ~15-20 files (see universal-bracket.md)

---

## Verification

- `cargo test --lib` after each pass
- Benchmarks: `nbody_sqrt` should still pass, performance unaffected
- New test cases for each intrinsic (interpreter AST-construction tests)
