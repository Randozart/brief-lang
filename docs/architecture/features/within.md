# Within Timeout — Assignment with Cycle Deadline

**Date:** 2026-06-24
**Status:** Implemented in interpreter

## Overview

The `within` keyword on assignments specifies a timeout bound for the operation. The assignment must complete within the given number of reactive cycles or it times out.

## Syntax

```briev
let result = fetch() within 10 cycles;
```

The timeout applies to the entire assignment expression, not just the function call. If the expression does not complete within the specified cycles, the assignment produces its default/zero value.

## Semantics

- The cycle count is evaluated as an integer expression
- If the inner expression completes before the cycle budget is exhausted, the result is stored normally
- If the cycle budget is exhausted, the result becomes the type default (`0` for `Int`, `false` for `Bool`, `""` for `String`, etc.)
- Used primarily in Embedded Briev (`.ebv`) for hardware-timed operations

## Examples

```briev
// Hardware fetch with timeout
sig fetch: () -> Int;
let res: Int = fetch() within 10 cycles;

// Reactive read with deadline
let port_val: Int = read_port() within 5 cycles;
```

## Embedded Briev Context

In `.ebv` files, `within` is particularly useful for MMIO operations where hardware might not respond:

```briev
// Wait for hardware ready with timeout
let status: Int = read_register(0x4000) within 100 cycles;
[status == 0] {
    // hardware not ready — handle timeout
};
```

## Backend Status

| Backend | Status |
|---------|--------|
| Interpreter | ✅ Timeout tracking with cycle counter (stubbed — always succeeds) |
| LLVM | ⚠️ Not implemented |
| Webstack | ⚠️ Not implemented |
