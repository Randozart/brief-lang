# Ellipsis — Range Expansion in Multi-Slice Coordinates

**Date:** 2026-06-24
**Status:** Parse-time only; evaluation produces an error

## Overview

The ellipsis (`..`) in multi-slice expressions represents "all remaining dimensions." It is expanded at parse time into the full set of range coordinates.

## Syntax

```briev
// Ellipsis captures remaining slice axes
matrix[.., 0]            // all rows, column 0
tensor[0, ..]            // first row, all remaining dims
list[..]                 // full range (entire list)
```

## Semantics

- `..` must appear at most once in a multi-slice bracket
- If appearing at the start: `..` expands to `0..len` for the first N axes, leaving the rest explicit
- If appearing in the middle or end: explicit coordinates fill the first axes, `..` fills the remainder
- If `..` is the only expression: equivalent to the full range `0..len`

## Parse-Time Resolution

The ellipsis `Expr::Ellipsis` is never evaluated at runtime. It is resolved during parsing into concrete `SliceCoordinate` values:

```rust
// Before resolution:
matrix[.., 0]

// After resolution (for a 3D tensor):
matrix[0..N, 0..M, 0]
```

## Error Cases

```briev
[.., ..]   // Error: multiple ellipses not allowed
..          // Error: ellipsis must be inside brackets
```

## Backend Status

| Backend | Status |
|---------|--------|
| Parser | ✅ Resolved into concrete coordinates |
| Interpreter | ❌ Always returns error at evaluation |
| LLVM | ⚠️ Stub — emits `%elp: Void` |
