# Brief Documentation Index

Complete documentation for the Brief language and compiler.

## Getting Started

Start here if you're new to Brief.

- **[README.md](../README.md)** - What Brief is and how to install it
- **[LANGUAGE-TUTORIAL.md](LANGUAGE-TUTORIAL.md)** - Step-by-step guide (10 parts)
  - Part 1: Getting started
  - Part 2: Transactions
  - Part 3: Reactive transactions
  - Part 4: Functions
  - Part 5: Pattern matching
  - Part 6: Using Rust (FFI)
  - Part 7: Real example
  - Part 8: Common patterns
  - Part 9: Tips and gotchas
  - Part 10: Debugging

## Reference Documentation

Use these for detailed information about language features.

- **[LANGUAGE-REFERENCE.md](LANGUAGE-REFERENCE.md)** - Complete language specification
  - Overview and core concepts
  - Full syntax reference
  - Types and type system
  - State and variables
  - Transactions (passive and reactive)
  - Definitions and functions
  - Contracts and verification
  - Pattern matching
  - Standard library
  - Foreign functions (FFI)
  - Working examples

- **[QUICK-REFERENCE.md](QUICK-REFERENCE.md)** - Cheat sheet
  - Syntax at a glance
  - Common patterns
  - Native stdlib
  - FFI modules
  - CLI commands

## Standard Library

Learn about built-in functions.

- **[std/core.bv](../std/core.bv)** - Native Brief stdlib (no FFI)
  - Integer math: `absolute()`, `min()`, `max()`, `clamp()`
  - Predicates: `is_positive()`, `is_negative()`, `is_zero()`, `is_even()`
  - State patterns and helpers
  - All functions proven at compile time

- **[std/bindings/README.md](../std/bindings/README.md)** - FFI bindings guide
  - What's native vs FFI
  - When to use each
  - Philosophy

- **[FFI-STDLIB-REFERENCE.md](FFI-STDLIB-REFERENCE.md)** - Complete FFI reference
  - 59 stdlib functions documented
  - I/O module (10 functions)
  - Math module (14 functions)
  - String module (15 functions)
  - Time module (5 functions)
  - Each function with parameters, returns, error codes, examples

## FFI (Foreign Functions)

Use Rust functions from Brief.

- **[FFI-USER-GUIDE.md](FFI-USER-GUIDE.md)** - How to use FFI
  - Creating custom TOML bindings
  - Using stdlib FFI functions
  - Error handling with Result types
  - 7 detailed examples
  - Troubleshooting guide

- **[FFI-STDLIB-REFERENCE.md](FFI-STDLIB-REFERENCE.md)** - Reference for stdlib FFI
  - Complete list of 59 available functions
  - Full documentation for each

- **[std/bindings/](../std/bindings/)** - TOML binding files
  - `io.toml` - File I/O operations
  - `math.toml` - Math functions
  - `string.toml` - String utilities
  - `time.toml` - Time operations

## Architecture and Design

Understand how Brief works internally.

- **[ARCHITECTURE.md](ARCHITECTURE.md)** - Compiler architecture
  - Pipeline: Lexer → Parser → Type Checker → Proof Engine → Interpreter
  - Design decisions

- **[STDLIB-REDESIGN.md](STDLIB-REDESIGN.md)** - Why stdlib is split
  - What Brief handles natively
  - What needs FFI
  - Rationale for each separation

## Examples

Working Brief programs.

- **[examples/](../examples/)** - Example programs
  - `reactive_counter.bv` - Basic reactive system
  - `bank_transfer_system.bv` - Multi-account state
  - `counter.rbv` - Web UI (Rendered Brief)
  - And more...

- **[examples/stdlib_usage.bv](../examples/stdlib_usage.bv)** - Native vs FFI examples

## By Use Case

### I want to learn Brief

1. Read [README.md](../README.md)
2. Follow [LANGUAGE-TUTORIAL.md](LANGUAGE-TUTORIAL.md) parts 1-3
3. Try examples from [examples/](../examples/)
4. Reference [QUICK-REFERENCE.md](QUICK-REFERENCE.md) while coding

### I want to look up syntax

- [QUICK-REFERENCE.md](QUICK-REFERENCE.md) for quick answers
- [LANGUAGE-REFERENCE.md](LANGUAGE-REFERENCE.md) for complete details

### I want to use FFI

1. Read [FFI-USER-GUIDE.md](FFI-USER-GUIDE.md) parts 1-3
2. Check [FFI-STDLIB-REFERENCE.md](FFI-STDLIB-REFERENCE.md) for available functions
3. See [examples/stdlib_usage.bv](../examples/stdlib_usage.bv) for patterns
4. Reference [FFI-USER-GUIDE.md](FFI-USER-GUIDE.md) troubleshooting section if stuck

### I want to create FFI bindings

1. Read [FFI-USER-GUIDE.md](FFI-USER-GUIDE.md) "Creating Bindings" section
2. Look at [std/bindings/](../std/bindings/) for examples
3. Reference [STDLIB-REDESIGN.md](STDLIB-REDESIGN.md) for design philosophy

### I want to understand the compiler

1. Read [ARCHITECTURE.md](ARCHITECTURE.md)
2. Look at compiler pipeline in [src/](../src/)
3. See [LANGUAGE-REFERENCE.md](LANGUAGE-REFERENCE.md) "Verification" section

## File Organization

```
spec/
├── INDEX.md (you are here)
├── README.md                           # What is Brief?
├── LANGUAGE-REFERENCE.md               # Complete language spec
├── LANGUAGE-TUTORIAL.md                # Step-by-step guide (10 parts)
├── QUICK-REFERENCE.md                  # Cheat sheet
├── FFI-USER-GUIDE.md                  # How to use FFI
├── FFI-STDLIB-REFERENCE.md            # Stdlib functions reference
├── STDLIB-REDESIGN.md                 # Why stdlib is designed this way
├── ARCHITECTURE.md                     # Compiler internals
└── (other design docs)

std/
├── core.bv                             # Native Brief stdlib
└── bindings/
    ├── README.md                       # FFI bindings guide
    ├── io.toml                         # File I/O
    ├── math.toml                       # Math functions
    ├── string.toml                     # String utilities
    └── time.toml                       # Time operations

examples/
├── stdlib_usage.bv                     # Native vs FFI examples
├── reactive_counter.bv                 # Reactive transactions
├── bank_transfer_system.bv             # State management
├── counter.rbv                         # Web UI
└── (other examples)

src/
├── lexer.rs                            # Tokenization
├── parser.rs                           # Parsing
├── ast.rs                              # AST
├── typechecker.rs                      # Type checking
├── proof_engine.rs                     # Verification
├── interpreter.rs                      # Execution
├── reactor.rs                          # Event loop
├── ffi/                                # Foreign function interface
└── (other modules)
```

## Quick Navigation

| I want to... | Read this |
|------|----------|
| Install Brief | [README.md](../README.md) |
| Learn the language | [LANGUAGE-TUTORIAL.md](LANGUAGE-TUTORIAL.md) |
| Look up syntax | [QUICK-REFERENCE.md](QUICK-REFERENCE.md) |
| Deep dive into features | [LANGUAGE-REFERENCE.md](LANGUAGE-REFERENCE.md) |
| Use FFI functions | [FFI-STDLIB-REFERENCE.md](FFI-STDLIB-REFERENCE.md) |
| Create FFI bindings | [FFI-USER-GUIDE.md](FFI-USER-GUIDE.md) |
| See working code | [examples/](../examples/) |
| Understand the compiler | [ARCHITECTURE.md](ARCHITECTURE.md) |
| Understand stdlib design | [STDLIB-REDESIGN.md](STDLIB-REDESIGN.md) |

## Topics

### State Management
- [LANGUAGE-TUTORIAL.md Part 2](LANGUAGE-TUTORIAL.md#part-2-transactions---making-changes)
- [LANGUAGE-REFERENCE.md State and Variables](LANGUAGE-REFERENCE.md#state-and-variables)
- [LANGUAGE-TUTORIAL.md Part 8](LANGUAGE-TUTORIAL.md#part-8-common-patterns)

### Reactive Systems
- [LANGUAGE-TUTORIAL.md Part 3](LANGUAGE-TUTORIAL.md#part-3-reactive-transactions)
- [LANGUAGE-REFERENCE.md Reactive Transactions](LANGUAGE-REFERENCE.md#reactive-transactions-rct-txn)
- [examples/reactive_counter.bv](../examples/reactive_counter.bv)

### Functions and Definitions
- [LANGUAGE-TUTORIAL.md Part 4](LANGUAGE-TUTORIAL.md#part-4-functions-definitions)
- [LANGUAGE-REFERENCE.md Definitions](LANGUAGE-REFERENCE.md#definitions-and-functions)

### Error Handling
- [LANGUAGE-TUTORIAL.md Part 6](LANGUAGE-TUTORIAL.md#part-6-using-rust-ffi)
- [FFI-USER-GUIDE.md Error Handling](FFI-USER-GUIDE.md#error-handling)

### Pattern Matching
- [LANGUAGE-TUTORIAL.md Part 5](LANGUAGE-TUTORIAL.md#part-5-pattern-matching)
- [LANGUAGE-REFERENCE.md Pattern Matching](LANGUAGE-REFERENCE.md#pattern-matching)

### Type System
- [LANGUAGE-REFERENCE.md Types](LANGUAGE-REFERENCE.md#types)
- [LANGUAGE-TUTORIAL.md Part 1](LANGUAGE-TUTORIAL.md#part-1-getting-started)

### Formal Verification
- [LANGUAGE-REFERENCE.md Contracts](LANGUAGE-REFERENCE.md#contracts-and-verification)
- [LANGUAGE-TUTORIAL.md Part 9](LANGUAGE-TUTORIAL.md#part-9-tips-and-gotchas)

## Version

- **Language**: Brief 6.2
- **Status**: Production-ready
- **Last Updated**: 2026-04-05

## Contributing

Found an error in the docs? Have a suggestion? Open an issue at:
https://github.com/anomalyco/brief-compiler/issues

---

**Start with [README.md](../README.md) or [LANGUAGE-TUTORIAL.md](LANGUAGE-TUTORIAL.md).**
