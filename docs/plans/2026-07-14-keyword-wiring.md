# Keyword Token Wiring Plan

## Problem
40 keyword tokens in the lexer are not matched by any parser arm. Many of these (`frgn`, `struct`, `enum`, `Ok`, `Err`, `Some`, `None`, `async`, `uni`) are essential language features used throughout benchmarks and stdlib. Others (`syscall`, `bank`, `link`, `stage`, `on`) are removed syntax that should be cleaned up.

## Architecture

```
Lexer token ──► parse_top_level() ──► dedicated arm (syntax introducer)
              │                  └──► keyword_as_identifier() (fallback)
              ▼
         expect_identifier() accepts keyword tokens as identifiers
```

Every keyword token is handled by exactly one of:
1. **Dedicated parser arm** — for syntax-introducing keywords (`frgn`, `struct`, `enum`, `async`, `uni`, `defn`, `let`, etc.)
2. **`keyword_as_identifier()` bridge** — for keywords that are also used as identifiers (`reg`, `op`, `bank` before removal, `Ok`/`Err` in patterns)
3. **Removed from lexer** — for syntax that was removed from the language (`syscall`, `bank`, `link`, `stage`, `on`, `inlineasm`)

## Phase 1 — Immediate Stopgap

Implement `keyword_as_identifier(&tok) -> Option<String>` in `src/parser/helpers.rs`. This maps every keyword token to its string representation. The function is called by `expect_identifier()` when the current token is not `Token::Identifier`.

**Critical for compilation:** Without this, the entire project fails to build (the function is already referenced at `helpers.rs:83`).

## Phase 2 — Wire Critical Keywords

Add dedicated parser arms for keywords that are syntax introducers:

| Keyword | Parser arm | Location | Feature module |
|---------|-----------|----------|----------------|
| `async` | Match in `parse_reactive_transaction()` | `definitions.rs` | Active in benchmarks |
| `frgn` | Add `Token::Frgn` arm in `parse_top_level()` | `definitions.rs` | `features/toplevel/foreign.rs` |
| `struct` | Add `Token::Struct` arm in `parse_top_level()` | `definitions.rs` | `features/toplevel/struct_def.rs` |
| `enum` | Add `Token::Enum` arm in `parse_top_level()` | `definitions.rs` | `features/toplevel/enum_def.rs` |
| `uni` | Add parsing in expression parser | `expressions.rs` | Pattern matching |
| `Op` | Match in `parse_type_definition()` slot loop | `definitions.rs` | Our new op syntax |

## Phase 3 — Wire Remaining Features

| Keyword | Priority | Notes |
|---------|----------|-------|
| `Is` | Low | `Expr::IsType` exists in AST, backend, interpreter — just needs parser |
| `Within` | Low | `Expr::Within` exists in AST, backend, interpreter — just needs parser |
| `Pvt` | Low | Visibility modifier |
| `Sed` | Low | Observability marker |
| `Render` | Low | `TopLevel::RenderBlock` exists in AST |
| `Sig` | Low | `TopLevel::Signature` exists in AST |
| `PtrBang` | Low | Stub implementation exists |
| Timing units | Low | Cycles, Ms, Seconds, etc. for watchdog expressions |
| Template/Macro | Low | Macro system exists but uses `$` syntax, not keyword |

## Phase 4 — Cleanup Removed Syntax

Remove these tokens from the lexer — they were explicitly removed from the language:

- `Syscall`, `SyscallBang` — replaced by `#` intrinsics
- `Bank` — removed
- `Link` — superseded by `import "link/..."` string imports
- `Stage` — removed
- `On` — removed
- `Asm` — removed (inline assembly)
- `Wfi` — removed (wait-for-interrupt was never implemented)

## Files Modified

| File | Phase | Change |
|------|-------|--------|
| `src/parser/helpers.rs` | 1 | Add `keyword_as_identifier()` function |
| `src/parser/definitions.rs` | 2 | Add `async`, `frgn`, `struct`, `enum`, `Op` parser arms |
| `src/parser/expressions.rs` | 2 | Add `uni` expression parsing |
| `src/lexer.rs` | 4 | Remove `Syscall`, `SyscallBang`, `Bank`, `Link`, `Stage`, `On`, `Asm` tokens |
| `src/lexer.rs` | 4 | Remove corresponding Display and test entries |
