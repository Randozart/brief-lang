# Block Expressions — Statement-Sequence as Expression

**Date:** 2026-06-24
**Status:** Fully implemented in interpreter; LLVM backend is a stub

## Overview

A block expression (`{ stmt1; stmt2; last_expr }`) groups a sequence of statements followed by a final expression. The value of the block is the value of the last expression. Block expressions scope the statements — state changes inside the block are discarded when the block exits.

## Syntax

```brief
let result = {
    &temp = temp + 1;
    temp * 2
};
```

The block evaluates each statement in order, then evaluates the final expression and returns its value. The preceding semicolons terminate each statement; the final expression has no trailing semicolon.

## Semantics

- **Scoped execution**: Statements in the block execute with the current state, but any state mutations are rolled back after the block ends
- **Value is the last expression**: The block evaluates to the final expression's value
- **Zero or more statements**: Any valid `Statement` can appear inside (assignment, let, guarded, term, etc.)
- **Void blocks**: A block with no statements and no final expression is not valid; blocks must have at least a final expression

## Evaluation

The interpreter:
1. Clones the current state
2. Executes each statement in sequence via `exec_stmt`
3. Evaluates the final expression
4. Restores the original state
5. Returns the final expression's value

This means block expressions are **pure** from the perspective of the enclosing scope — they can read state but mutations are discarded.

## Examples

```brief
// Scoped computation
let x: Int = 10;
let result = {
    &x = x + 5;    // mutation is discarded
    x * 2
};
// x is still 10, result is 30

// Guarded block
let score = {
    [temp > 0] {
        temp * 2
    };
    temp
};
```

## Backend Status

| Backend | Status |
|---------|--------|
| Interpreter | ✅ Full evaluation with scoped state |
| LLVM | ⚠️ Stub — emits `%blk: Void` |
| Webstack | ✅ Always returns `JsValue::TRUE` |
