# Tuple Expressions — Multi-Value Groups

**Date:** 2026-06-24
**Status:** Fully implemented

## Overview

Tuples group multiple values into a single compound value. They are used for multi-return functions, heterogeneous data groups, and pattern matching.

## Syntax

### Tuple Literal

```briv
let pair = (1, "hello");
let triple = (true, 42, "world");
let singleton = (1,);     // trailing comma required for single-element
```

### Tuple Type

```briv
defn split_name(s: String) -> (String, String);
defn process() -> (Bool, Int, String);
```

### Tuple Destructuring

```briv
let (first, second) = split_name("John Doe");
let (a, b, c) = process();
let (_, important) = split("skip_this,keep_this");  // _ discards
```

### Tuple Indexing via Projection

```briv
let pair = (42, "hello");
let first: Int = pair :> 0;      // 42
let second: String = pair :> 1;  // "hello"
```

### Multi-Return Functions

```briv
defn split(s: String, sep: String) -> (String, String) {
    let pos = find(s, sep);
    term (s[..pos], s[pos+1..]);
};

txn test {
    let (left, right) = split("a,b", ",");
    term;
};
```

## Evaluation

Tuple literals evaluate each sub-expression and wrap the results in a `Value::Tuple`. Destructuring extracts values by position and binds them to named variables. Multi-return `defn`/`txn` are syntax sugar for tuple returns.

## Value Representation

- `Value::Tuple(Vec<Value>)` — ordered, heterogeneous
- Tuple fields are accessed by integer index via `ProjectionTarget::Index(usize)`

## Related

- `multi_output.bv` — demonstrates multi-return functions
- Arrow pop/discard: tuples can be used as structured return types
