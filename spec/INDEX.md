# Brief Documentation Index

Complete documentation for the Brief language and compiler.

## Getting Started

Start here if you're new to Brief.

- **[README.md](../README.md)** - What Brief is and how to install it
- **[LANGUAGE-TUTORIAL.md](LANGUAGE-TUTORIAL.md)** - Step-by-step guide
  - Part 1: Getting started
  - Part 2: Transactions
  - Part 3: Reactive transactions
  - Part 4: Functions
  - Part 5: Pattern matching
  - Part 6: Structs
  - Part 7: Foreign Functions (FFI)
  - Part 8: Real example
  - Part 9: Common patterns
  - Part 10: Syntactic sugar
  - Part 11: Multi-return functions
  - Part 12: Tips and gotchas
  - Part 13: Debugging

## Reference Documentation

Use these for detailed information about language features.

- **[SPEC.md](SPEC.md)** - Language specification
  - Complete BNF grammar
  - Formal semantics
  - Type system
  - Transaction model
  - FFI system

- **[LANGUAGE-REFERENCE.md](LANGUAGE-REFERENCE.md)** - Complete reference
  - Syntax reference
  - Types
  - State and variables
  - Transactions
  - Definitions
  - Structs
  - Pattern matching
  - FFI
  - Standard library
  - Error messages

- **[QUICK-REFERENCE.md](QUICK-REFERENCE.md)** - Cheat sheet
  - Syntax at a glance
  - Common patterns
  - Standard library
  - FFI modules
  - CLI commands

## Foreign Function Interface (FFI)

Learn how to call external functions from Brief.

- **[FFI-GUIDE.md](FFI-GUIDE.md)** - Complete FFI guide
  - When to use FFI
  - TOML bindings format
  - Brief declarations
  - Error handling
  - Creating custom bindings
  - Type system
  - Examples

- **[std/bindings/](../std/bindings/)** - TOML binding files
  - `io.toml` - File I/O
  - `math.toml` - Math functions
  - `string.toml` - String utilities
  - `time.toml` - Time operations

## Rendered Brief

Build reactive web interfaces with Brief.

- **[RENDERED-BRIEF-GUIDE.md](RENDERED-BRIEF-GUIDE.md)** - Web UI guide
  - rstruct components
  - b-text, b-show, b-trigger directives
  - Component composition
  - Examples

## Examples

Working Brief programs.

- **[examples/](../examples/)** - Example programs
  - `reactive_counter.bv` - Basic reactive system
  - `bank_transfer_system.bv` - Multi-account state
  - `stdlib_usage.bv` - Native vs FFI examples
  - And more...

## By Use Case

### I want to learn Brief

1. Read [README.md](../README.md)
2. Follow [LANGUAGE-TUTORIAL.md](LANGUAGE-TUTORIAL.md)
3. Try examples from [examples/](../examples/)
4. Reference [QUICK-REFERENCE.md](QUICK-REFERENCE.md) while coding

### I want to look up syntax

- [QUICK-REFERENCE.md](QUICK-REFERENCE.md) for quick answers
- [LANGUAGE-REFERENCE.md](LANGUAGE-REFERENCE.md) for complete details
- [SPEC.md](SPEC.md) for formal specification

### I want to use FFI

1. Read [FFI-GUIDE.md](FFI-GUIDE.md)
2. Check [std/bindings/](../std/bindings/) for available functions
3. See [examples/stdlib_usage.bv](../examples/stdlib_usage.bv) for patterns

### I want to create FFI bindings

1. Read [FFI-GUIDE.md](FFI-GUIDE.md) "Creating Custom Bindings" section
2. Look at [std/bindings/](../std/bindings/) for examples

### I want to build web UIs

1. Read [RENDERED-BRIEF-GUIDE.md](RENDERED-BRIEF-GUIDE.md)
2. Look at [examples/*.rbv](../examples/) for patterns

## File Organization

```
spec/
├── INDEX.md (you are here)
├── README.md                    # What is Brief?
├── SPEC.md                      # Language specification
├── LANGUAGE-REFERENCE.md        # Complete reference
├── LANGUAGE-TUTORIAL.md         # Step-by-step guide
├── QUICK-REFERENCE.md           # Cheat sheet
├── FFI-GUIDE.md                # FFI guide
├── RENDERED-BRIEF-GUIDE.md     # Web UI guide
└── old_docs/                   # Archived documents

std/
├── core.bv                      # Native Brief stdlib
└── bindings/
    ├── io.toml                  # File I/O
    ├── math.toml                # Math functions
    ├── string.toml             # String utilities
    └── time.toml               # Time operations

lib/
├── ffi/
│   └── mappers/                 # FFI mapper system
└── std/
    ├── io.bv                    # I/O module
    ├── math.bv                   # Math module
    └── ...

examples/
├── reactive_counter.bv           # Reactive transactions
├── bank_transfer_system.bv       # State management
├── stdlib_usage.bv               # Native vs FFI
└── ...

src/
├── lexer.rs                     # Tokenization
├── parser.rs                    # Parsing
├── ast.rs                       # AST
├── typechecker.rs               # Type checking
├── proof_engine.rs              # Verification
├── interpreter.rs                # Execution
├── ffi/                         # Foreign function interface
└── ...
```

## Quick Navigation

| I want to... | Read this |
|------|----------|
| Install Brief | [README.md](../README.md) |
| Learn the language | [LANGUAGE-TUTORIAL.md](LANGUAGE-TUTORIAL.md) |
| Look up syntax | [QUICK-REFERENCE.md](QUICK-REFERENCE.md) |
| Deep dive into features | [LANGUAGE-REFERENCE.md](LANGUAGE-REFERENCE.md) |
| Formal specification | [SPEC.md](SPEC.md) |
| Use FFI | [FFI-GUIDE.md](FFI-GUIDE.md) |
| Build web UIs | [RENDERED-BRIEF-GUIDE.md](RENDERED-BRIEF-GUIDE.md) |
| See working code | [examples/](../examples/) |

## Topics

### State Management
- [LANGUAGE-TUTORIAL.md Part 2](LANGUAGE-TUTORIAL.md#part-2-transactions---making-changes)
- [LANGUAGE-REFERENCE.md State](LANGUAGE-REFERENCE.md#state-and-variables)

### Reactive Systems
- [LANGUAGE-TUTORIAL.md Part 3](LANGUAGE-TUTORIAL.md#part-3-reactive-transactions)
- [LANGUAGE-REFERENCE.md Reactive](LANGUAGE-REFERENCE.md#transactions)

### Functions
- [LANGUAGE-TUTORIAL.md Part 4](LANGUAGE-TUTORIAL.md#part-4-functions-definitions)
- [LANGUAGE-REFERENCE.md Definitions](LANGUAGE-REFERENCE.md#definitions)

### Syntactic Sugar
- [LANGUAGE-TUTORIAL.md Part 10](LANGUAGE-TUTORIAL.md#part-10-syntactic-sugar)
- [LANGUAGE-REFERENCE.md Syntactic Sugar](LANGUAGE-REFERENCE.md#syntactic-sugar)

### Foreign Functions
- [LANGUAGE-TUTORIAL.md Part 7](LANGUAGE-TUTORIAL.md#part-7-foreign-functions-ffi)
- [FFI-GUIDE.md](FFI-GUIDE.md)

## Version

- **Language**: Brief 8.0
- **Status**: Production-ready
- **Last Updated**: 2026-04-08

---

**Start with [README.md](../README.md) or [LANGUAGE-TUTORIAL.md](LANGUAGE-TUTORIAL.md).**
