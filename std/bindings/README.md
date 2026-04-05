# Brief Standard Library FFI Bindings

These are Rust FFI bindings for operations Brief cannot do natively.

## Organization

- **io.toml** - File I/O and system operations
- **math.toml** - Transcendental math (sin, sqrt, log, etc.)
- **string.toml** - String utilities (parsing, manipulation)
- **time.toml** - Time and timing operations

## What's NOT Here (Use Native Brief Instead)

For these, use functions from `std/core.bv`:

- Integer math: `absolute()`, `min()`, `max()`, `clamp()`
- State patterns: `lazy initialization`, `state validation`
- Conditionals: `choose_if()` for select logic
- Predicates: `is_positive()`, `is_negative()`, `is_zero()`, `is_even()`

Native Brief functions are proven at compile time and have no FFI overhead.

## Usage

```brief
# Use native Brief for simple math
let x: Int = -5;
let abs_x: Int = absolute(x);  # Uses std/core.bv

# Use FFI for complex math
frgn sqrt(value: Float) -> Result<Float, MathError> from "std::math";
let root: Float = sqrt(16.0);  # Uses FFI to Rust
```

## Philosophy

- **Native Brief**: State management, transactions, simple computation
- **FFI Rust**: I/O, OS operations, transcendental math, string utilities
