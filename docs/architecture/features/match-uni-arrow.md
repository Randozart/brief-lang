# Match/Uni Arrow Syntax (`= → ->`)

**Date added:** 2026-06-12  
**Phase:** 1.5

## Rationale

The `=` separator between pattern and body in `match` and `uni` was
semantically ambiguous with assignment. The `->` arrow reads more
naturally as "pattern maps to body" and is consistent with Briv's use
of `->` for return types and swan songs.

## Syntax

```briv
match x {
    Some(v) -> expr1,
    None    -> expr2,
    _       -> fallback,
};

uni val(Some(v)) -> result;
uni x -> expression;
```

## Changes

| Location | File | Change |
|----------|------|--------|
| Match arm separator | `parser.rs` | `self.expect(Token::Eq)` → `self.expect(Token::Arrow)` |
| Uni wildcard pattern | `parser.rs` | same |
| Uni named variant pattern | `parser.rs` | same |
| Uni simple pattern | `parser.rs` | same |
| 6 match test strings | `parser.rs` | `= ` → `-> ` in source strings |
| 9 uni test strings | `parser.rs` | `= ` → `-> ` in source strings |
