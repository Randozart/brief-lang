# Subtype Projections — Collection Query Pipeline

**Date:** 2026-06-24
**Phase:** TBD
**Status:** Fully implemented in interpreter; LLVM backend is a stub

## Overview

Subtype projections (`<:` syntax) are Briev's built-in collection query pipeline. They let you filter, transform, sort, join, and aggregate collections with a SQL-like chain of operations, all without explicit loops.

## Syntax

```briev
let result : source { OP1(args); OP2(args); ... };
```

The source is evaluated, then each operation is applied in sequence. Each item in the pipeline is bound to `_` for expression arguments.

### Operations

| Op | Arity | Terminal? | Semantics |
|----|-------|-----------|-----------|
| `FILTER(predicate)` | 1 expr | No | Keep items where predicate evaluates to `true` |
| `MAP(transform)` | 1 expr | No | Transform each item |
| `SORT(key)` | 1 expr | No | Sort by key expression |
| `LIMIT(n)` | integer | No | Take first N items |
| `SKIP(n)` | integer | No | Drop first N items |
| `UNIQUE` | 0 | No | Remove adjacent duplicates |
| `JOIN(other, key)` | 2 exprs | No | Inner join with another collection on key |
| `GROUP(key)` | 1 expr | No | Group by key into `(key, items)` tuples |
| `COUNT` | 0 | **Yes** | Number of items (returns Int) |
| `SUM(expr)` | 1 expr | **Yes** | Sum of expression over items (returns Int) |
| `AVG(expr)` | 1 expr | **Yes** | Average of expression over items (returns Float) |
| `MIN(expr)` | 1 expr | **Yes** | Minimum of expression over items |
| `MAX(expr)` | 1 expr | **Yes** | Maximum of expression over items |
| `MATCH(regex)` | 1 expr | N/A | Regex match on String source |

**Terminal ops** produce a scalar value and end the pipeline. Non-terminal ops produce a `List` as their output.

### Examples

```briev
// Filter and map
let result : items { FILTER(active); MAP(x * 2); };

// Sort by a field and take top 5
let top5 : scores { SORT(_.value); LIMIT(5); };

// Unique values
let unique : tags { UNIQUE; };

// String regex match
let match : email["^(.+)@(.+)$"];

// Aggregate
let count : items { FILTER(active); COUNT; };
let sum : items { SUM(_.price); };
let avg : items { AVG(_.score); };

// Join two collections on a key
let joined : left { JOIN(right, _[0]); };

// Group by key
let groups : items { GROUP(_.category); };

// Chained pipeline
let result : orders {
    FILTER(status == "paid");
    SORT(date);
    MAP(order_total);
    LIMIT(10);
};
```

## Evaluation

Each item in the source collection is bound to `_` when evaluating operation expressions. The `_` binding works like an implicit element variable.

The pipeline is evaluated left-to-right:
1. Evaluate source expression
2. For each op: apply to current items
3. If terminal op: return scalar, stop
4. Otherwise: output becomes input to next op

### DBVL Optimization

When the source is a `DbvlTable` and the first op is `FILTER(_.field_0 == "key")`, the interpreter uses an O(1) indexed lookup instead of materializing all rows.

### Source Types

| Source | Behavior |
|--------|----------|
| `List<T>` | Iterate elements directly |
| `Tuple` | Iterate fields |
| `HashMap<K,V>` | Iterate values (not keys) |
| `HashSet<T>` | Iterate elements as Strings |
| `String` | Only `MATCH` is valid; returns captures |
| `DbvlTable` | Lazy evaluation with key optimization |

## Backend Status

| Backend | Status |
|---------|--------|
| Interpreter | ✅ Full evaluation, all 14 ops |
| LLVM | ⚠️ Stub — returns `%sub: Void` |
| Webstack | ⚠️ Not implemented |
| CIRCT | ⚠️ Not implemented |
