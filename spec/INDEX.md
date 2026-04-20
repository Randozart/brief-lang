# Brief Documentation Index

## Quick Navigation

### Getting Started
- [README](README.md) - What Brief is and how to install
- [QUICK-REFERENCE](QUICK-REFERENCE.md) - Syntax at a glance, common patterns
- [LANGUAGE-TUTORIAL](LANGUAGE-TUTORIAL.md) - Step-by-step guide (legacy reference)

### Language Specification
- [SPEC](SPEC.md) - **Master specification** - All language features in one document
  - [Core Language](SPEC.md#1-introduction-and-philosophy)
  - [FFI System](SPEC.md#4-foreign-function-interface-ffi)
  - [Type System](SPEC.md#5-type-system)
  - [Standard Library](SPEC.md#6-standard-library)
  - [Implementation Status](SPEC.md#7-implementation-status)

### Language Variants
- [RENDERED-BRIEF](RENDERED-BRIEF-GUIDE.md) - Web UI components (`rstruct`, directives)
- [EMBEDDED BRIEF](EMBEDDED_BRIEF_2.1_SPEC.md) - Float types, vectors, bit ranges

### Reference
- [QUICK-REFERENCE](QUICK-REFERENCE.md) - Cheat sheet
- [FFI GUIDE](FFI-GUIDE.md) - FFI guide (legacy, superseded by SPEC)
- [WASM SETUP](lib/ffi/wasm/README.md) - WASM backend guide

### Examples
- [examples/](examples/) - Working code examples
  - [bank_transfer_system.bv](examples/bank_transfer_system.bv) - Multi-account state
  - [reactive_counter.bv](examples/reactive_counter.bv) - Reactive transactions
  - [test_ffi.bv](examples/test_ffi.bv) - FFI patterns
  - [stdlib_usage.bv](examples/stdlib_usage.bv) - Standard library usage

### Build & Test
- [CLI GUIDE](CLI-GUIDE.md) - Command line interface
- [CONTRIBUTING](#contributing) - Development setup

---

## Feature Cross-Reference

| Feature | Spec Section | Examples |
|---------|-------------|----------|
| Transactions | SPEC.md §3 | reactive_counter.bv |
| FFI (`frgn`, `syscall`) | SPEC.md §4 | test_ffi.bv |
| Fire-and-forget FFI | SPEC.md §4.1 | (see FFI guide) |
| Resource System | SPEC.md §4.3 | (see FFI guide) |
| Float Types | EMBEDDED_BRIEF §2.4 | (see embedded spec) |
| Bit Packing | SPEC.md §4.2 | (advanced examples) |
| Async Transactions | SPEC.md §3 | (see examples) |
| Pattern Matching | SPEC.md §3 | (see language tutorial) |

---

## Contributing

This documentation is generated from the compiler's understanding of the language. To update the spec, modify `SPEC.md` directly. Key files:

- `spec/SPEC.md` - Master specification (this file)
- `src/` - Compiler source code (truth reference for implementation)
- `spec/old_docs/` - Archived legacy specifications
- `examples/` - Working code examples

To test examples:
```bash
cd examples/bank_transfer_system
brief run
```

---

*Generated from compiler state v0.10.0 (2026-04-20)*