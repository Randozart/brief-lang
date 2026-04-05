# Brief

<img src="brief-logo-draft.jpg" alt="Brief" width="200"/> <img src="r-brief-logo-draft.jpg" alt="Brief" width="200"/>


**Brief doesn't break.**

A compiled language for building reactive systems where the compiler proves all state transitions work correctly.

## What Brief Actually Is

Brief is a reactive, declarative, blackboard-based language. You declare state as global variables. You write transactions (blocks of code) that specify what conditions must be true before they can run, and what must be true after. The runtime continuously checks if transaction conditions are met and fires them automatically.

The compiler verifies two things:
1. Every transaction will actually reach its termination condition
2. When a transaction runs, the postcondition will be satisfied

## Example

```brief
let balance: Int = 100;
let ready: Bool = false;

txn initialize [~ready] {
  &ready = true;
  term;
};

rct txn process [ready && balance > 0] {
  &balance = balance - 10;
  term;
};
```

- `initialize` is a passive transaction. It runs when called.
- `process` is reactive (`rct`). It fires automatically whenever `ready` is true and `balance > 0`.
- `[~ready]` means: fire when `ready` is false, and must make it true.
- `[ready && balance > 0]` means: only fire when both conditions hold.

The compiler proves `balance` will actually decrease, and that the postcondition will be satisfied.

## How It Works

Brief runs on a reactor loop. The runtime:
1. Tracks which variables each reactive transaction cares about
2. When a variable changes, marks only the affected transactions as dirty
3. Re-evaluates their preconditions
4. Fires any that now have true preconditions
5. When nothing can fire, the program is at equilibrium and the reactor sleeps

This means polling and event dispatchers are replaced by simple logical evaluation. The state changes → check affected transactions → fire them if their preconditions hold.

## Current Status

This is an early implementation. Core features work:
- Transactions with pre/post conditions
- Reactive (`rct`) auto-firing transactions
- Proof engine verifies termination and postconditions
- Type checking
- FFI (call Rust functions)
- 59 stdlib functions included

Known incomplete:
- Rendered Brief (web UI framework)
- Some edge cases in termination proofs
- Complex generics

## Install

```bash
cargo install --path .
```

## Usage

```bash
brief check program.bv          # Type check and verify
brief build program.bv          # Run
brief init my-project           # Create project
brief lsp                       # Start language server
```

## Full Language

- **Transactions**: `txn` and `rct txn` blocks with contracts
- **State**: Global variables (`let`, `const`)
- **Types**: String, Int, Float, Bool, Void, custom structs
- **Contracts**: Preconditions `[pre]` and postconditions `[post]`
- **Prior state**: `@variable` references the value at transaction start
- **Pattern matching**: Unification for handling multiple outcomes
- **Imports**: Modular code
- **Definitions**: Named functions with contracts (`defn`)
- **FFI**: Call Rust from Brief

## Documentation

- [SPEC.md](spec/SPEC.md) - Full language spec
- [FFI-USER-GUIDE.md](spec/FFI-USER-GUIDE.md) - Using Rust functions from Brief
- [FFI-STDLIB-REFERENCE.md](spec/FFI-STDLIB-REFERENCE.md) - Available functions
- [examples/](examples/) - Example programs

## Building and Testing

```bash
cargo build --release
cargo test --lib          # Unit tests
cargo test                # All tests
```

## How It's Built

```
Lexer → Parser → Type Checker → Proof Engine → Interpreter
```

- **Lexer**: Tokenizes input
- **Parser**: Builds AST
- **Type Checker**: Verifies type correctness
- **Proof Engine**: Verifies each transaction reaches its postcondition
- **Interpreter**: Runs the reactive loop

## License

Apache 2.0
