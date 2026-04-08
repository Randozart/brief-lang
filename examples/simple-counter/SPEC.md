# Simple Counter Specification

## Overview

A minimal counter program demonstrating Brief's contract system.

## Program Structure

### State Variables

| Variable | Type | Initial Value | Description |
|----------|------|---------------|-------------|
| `count` | Int | 0 | The current counter value |

### Transactions

#### increment

- **Precondition**: `count < 100` - Counter cannot exceed 100
- **Postcondition**: `count == @count + 1` - Increases by exactly 1
- **Effect**: Adds 1 to count

#### decrement

- **Precondition**: `count > 0` - Counter cannot go below 0
- **Postcondition**: `count == @count - 1` - Decreases by exactly 1
- **Effect**: Subtracts 1 from count

#### reset

- **Precondition**: `count != 0` - Cannot reset if already at 0
- **Postcondition**: `count == 0` - Sets count to 0
- **Effect**: Resets count to 0

## Contract Semantics

### Preconditions

Preconditions define when a transaction may run. If the precondition is false, the transaction will not execute.

### Postconditions

Postconditions define what must be true after a transaction completes. Brief mathematically verifies that the postcondition can be satisfied from the precondition.

### The @ Symbol

`@count` refers to the value of `count` **before** the transaction ran. This allows expressing state changes as relationships.

## Design Decisions

1. **Bounds at 0 and 100** - Arbitrary but reasonable limits
2. **No set transaction** - Keeping it simple; see counter.rbv for full example
3. **Sequential constraints** - Each transaction is independent
