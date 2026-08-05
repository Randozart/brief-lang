# Fix Remaining Gaps

**Date:** 2026-06-26
**Status:** Active

## Fix 1: Rewrite `inop-sadd.bv` — no `else` after guards

Replace `[guard] { body } else { body }` with two guards.

## Fix 2: `let _ = ...` discard — interpreter.rs

`_` is already lexed as `Token::Underscore` and parsed as `Expr::Identifier("_")`.
In `Statement::Let` evaluation, skip state insertion when `name == "_"`.

## Fix 3: Replace `list + [val]` in skiplist.bv

`+` on collections is SIMD, not concatenation. Use a helper defn with `<-`:
```briv
defn sl_append(list: List<Int>, val: Int) -> List<Int> {
    &list <- val;
    term list;
};
```

## Fix 4: Add end-to-end txn + sl_contains test

Parse skiplist.bv from source, call sl_contains through interpreter.

## Fix 5: Remove unprovable contracts from StringBuilder defns

`term .#Size` on struct returns falls through to `SymbolicValue::Unknown`.
Since defns don't require contracts, remove them.

## Verify

```bash
cargo test --lib
cargo run --bin briv-compiler -- check examples/inop-sadd.bv
cargo run --bin briv-compiler -- check examples/inop-side-effect.bv
cargo run --bin briv-compiler -- check lib/std/string_builder.bv
```
