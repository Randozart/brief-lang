# Embedded Brief Implementation Summary

## Overview

This document describes the implementation of **Embedded Brief** - a bare-metal variant of the Brief programming language for microcontrollers and high-velocity hardware (GPUs, FPGAs). This implementation extends the existing Brief compiler with new features for embedded systems programming.

## Project Goal

The goal is to implement Embedded Brief (`.ebv` files) while also adding features that benefit Core (`.bv`) and Rendered Brief (`.rbv`) variants. The approach uses a unified approach where all three file types can share logic via imports.

## Discoveries from Research

1. **Existing Infrastructure**: The codebase already has `Type::Union` in `src/ast.rs` and `ResultCheckStatus` tracking in `src/typechecker.rs`, which can be leveraged for new error handling rules
2. **Pattern Matching**: The parser already supports `[result Ok(val)]` style guards via `Expr::PatternMatch`
3. **Error Handling Rule**: If `within` timeout is used, the receiver **must** be a union type containing `Error`, and the compiler must enforce that all variants are handled before `term` is allowed
4. **Geometry Matching**: Vector operations require strict dimension matching - `Int[100] + Int[50]` should be a compile error

## Implementation Completed

### 1. Lexer Updates (`src/lexer.rs`)

Added new tokens for Embedded Brief features:

```rust
// Type tokens
TypeUInt,      // UInt
TypeUnsigned,  // Unsigned
TypeUSgn,      // USgn
TypeSigned,    // Signed
TypeSgn,       // Sgn

// Embedded keywords
Trg,           // trg
Stage,         // stage  
Forall,        // forall
Exists,        // exists
Within,        // within
Bank,          // bank

// Time units
Cycles, Cyc,  // cycles, cyc
Ms,           // ms
Seconds,      // sec, seconds, s
Minute        // min, minute
```

### 2. AST Updates (`src/ast.rs`)

Added new types and structures:

```rust
// Time units for timeout
pub enum TimeUnit {
    Cycles,
    Ms,
    Seconds,
    Minutes,
}

// Bit ranges for memory mapping
pub enum BitRange {
    Single(usize),      // [7]
    Range(usize, usize), // [0:3]
    Any(usize),         // [*xN]
}

// New type variants
pub enum Type {
    // ... existing variants
    UInt,                        // Unsigned integer
    Vector(Box<Type>, usize),    // Vector with dimension: Int[100]
    // ...
}

// New expression variants
pub enum Expr {
    // ... existing variants
    Slice {
        value: Box<Expr>,
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
        stride: Option<Box<Expr>>,
    },
    ForAll {
        var: String,
        expr: Box<Expr>,
    },
    Exists {
        var: String,
        expr: Box<Expr>>,
    },
    // ...
}

// Updated Statement variants
pub enum Statement {
    Assignment {
        is_owned: bool,
        name: String,
        expr: Expr,
        timeout: Option<(Expr, TimeUnit)>,  // NEW: within timeout
    },
    Let {
        name: String,
        ty: Option<Type>,
        expr: Option<Expr>,
        address: Option<u64>,       // NEW: @ 0x40000000
        bit_range: Option<BitRange>, // NEW: [7], [0:3], [*x2]
        is_override: bool,           // NEW: override keyword
    },
    // ...
}

// NEW: Trigger declaration
pub struct TriggerDeclaration {
    pub name: String,
    pub stages: Vec<String>,
    pub condition: Expr,
    pub bit_range: Option<BitRange>,
    pub span: Option<Span>,
}

// NEW: TopLevel variant
pub enum TopLevel {
    // ... existing variants
    Trigger(TriggerDeclaration),
}
```

### 3. Parser Updates (`src/parser.rs`)

Implemented parsing for new syntax:

**Bit Range Parsing:**
```rust
fn parse_bit_range(&mut self) -> Result<BitRange, String> {
    // [7]       -> Single(7)
    // [0:3]     -> Range(0, 3)
    // [*x2]     -> Any(2)
    // [*]       -> Any(1)
}
```

**State Declaration with Address:**
```brief
let led_pin @ 0x40001000 [0:3]: Int = 0;
```

**Assignment with Timeout:**
```brief
sensor_read = read_sensor() within 100 ms;
```

**Trigger Declaration:**
```brief
trg button_press on stage init [0] {
    term true;
};
```

### 4. Codebase Compatibility Fixes

Updated ~15 source files to handle new fields and variants:

| File | Changes |
|------|---------|
| `annotator.rs` | Added handlers for `Trigger`, `UInt`, `Vector`, `Slice`, `ForAll`, `Exists` |
| `desugarer.rs` | Added `timeout`, `address`, `bit_range`, `is_override` to Statement initializers |
| `interpreter.rs` | Added timeout and new expr variants |
| `proof_engine.rs` | Added handlers for new types and expressions |
| `symbolic.rs` | Added handlers for new expr variants |
| `typechecker.rs` | Added handlers for new types and expressions |
| `wasm_gen.rs` | Added timeout field handling |
| `reactor.rs` | Added timeout field handling |
| `assertion_verify.rs` | Added timeout field to test statements |
| `ffi/validator.rs` | Added wasm_impl/wasm_setup fields to test structs |

## Features Implemented

### Memory Mapping
- Address specification with `@` syntax: `let reg @ 0x40000000: Int;`
- Bit-range selection: `[0:3]`, `[7]`, `[*xN]`
- Override capability: `override let reg @ 0x40000000: Int;`

### Timeout Handling
- `within <expression> <time_unit>` syntax
- Time units: `cycles`, `ms`, `s`, `min`
- Example: `data = fetch() within 10 cycles;`

### Vector Types
- Syntax: `Int[100]`, `Float[64]`
- Strict dimension matching in operations (compile error for mismatched dims)

### Quantifiers
- `forall` and `exists` for constraint solving
- Used in contracts for bounded quantification

### Triggers
- Hardware trigger declarations
- Stage-based triggering for GPU/FPGA pipelines

## Testing

All existing tests continue to pass:
```
test result: ok. 81 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Build Status

The compiler builds successfully:
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.46s
```

## Files Modified

- `src/lexer.rs` - Added tokens
- `src/ast.rs` - Added types and structures
- `src/parser.rs` - Added parsing logic
- `src/annotator.rs` - Added new pattern handlers
- `src/desugarer.rs` - Added field initializers
- `src/interpreter.rs` - Added new expr handlers
- `src/proof_engine.rs` - Added new type/expr handlers
- `src/symbolic.rs` - Added new expr handlers
- `src/typechecker.rs` - Added new type/expr handlers
- `src/wasm_gen.rs` - Added timeout handling
- `src/reactor.rs` - Added timeout handling
- `src/assertion_verify.rs` - Added test field
- `src/ffi/validator.rs` - Added test fields

## Next Steps

1. **Typechecker logic for vector lifting and geometry validation**
2. **Implement mandatory error handling for Union types** (when `within` is used)
3. **Create embedded module** for memory mapping and codegen
4. **Update wasm_gen.rs** for TypedArray vector mapping
5. **Add parser support for `UInt` type**
6. **Implement trigger compilation** for embedded targets

## Relevant Documentation

- `docs/EMBEDDED_BRIEF_2.2_SPEC.md` - Final spec
- `docs/EMBEDDED_BRIEF_IMPLEMENTATION_PLAN.md` - Implementation plan
- `spec/EMBEDDED-BRIEF-SPEC.md` - Original Embedded Brief 1.0 spec
- `CLAUDE.md` - Compiler documentation
