# Pattern Matching — Literal, Variant, Wildcard, and Field Patterns

**Date:** 2026-06-24
**Status:** Fully implemented in interpreter; LLVM backend limited

## Overview

Pattern matching tests a value against a pattern, binding variables from the match. Briev supports several pattern forms used in `match` expressions and `uni` blocks.

## Pattern Types

### Wildcard

```briev
_  // matches anything, binds nothing
```

### Literal Patterns

```briev
42        // matches integer 42
"hello"   // matches string "hello"
true      // matches boolean true
3.14      // matches float 3.14
'A'       // matches char 'A'
```

### Variant Patterns

```briev
Some(val)     // matches Value::Enum where variant == "Some"
Ok(result)    // matches Result::Ok variant
None          // matches fieldless variant
```

### Tuple Patterns

```briev
(1, "a")      // matches 2-element tuple with specific values
(x, y)        // matches 2-element tuple, binds to x, y
```

### Field Patterns

```briev
{ x: 1, y: _ }  // matches struct-like value with fields
```

## Match Expressions

```briev
match value {
    pattern1 => expr1,
    pattern2 if guard => expr2,
    pattern3 => expr3,
    _ => default_expr
};
```

Arms are tried in order. The first matching arm evaluates its body expression. Guards are optional boolean expressions evaluated after a pattern matches.

## Uni Blocks

`uni` blocks are statement-level pattern matching on enum values:

```briev
uni result(Ok(val)) = {
    println#("Success: " + val);
};
uni result(Err(msg)) = {
    println#("Error: " + msg);
};
```

## Evaluation

- `pattern_match(pat, value, state)` recursively matches `pat` against `value`, inserting bindings into `state`
- Literal patterns compare structural equality
- Variant patterns match the variant name and recursively match fields
- Wildcards always succeed with no bindings
- Tuple patterns match element-by-element

## Backend Status

| Backend | Status |
|---------|--------|
| Interpreter | ✅ Full match + uni evaluation |
| LLVM | ⚠️ PatternMatchExpr delegates to old AST form; MatchExpr is stub |
| Webstack | ✅ PatternMatch, Match both return `JsValue::TRUE` |
