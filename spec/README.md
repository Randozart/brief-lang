# Brief Language Compiler (Rust)

A Rust-based compiler for the Brief language, a declarative, contract-enforced logic language designed for LLM-assisted development.

## What's Built

### 1. Lexer (`src/lexer.rs`)
- Logos-based tokenizer matching the Brief language specification
- Supports all keywords: `sig`, `defn`, `let`, `const`, `txn`, `rct`, `async`, `term`, `escape`, `import`, `from`
- Supports all operators: `=`, `&`, `@`, `==`, `!=`, `<`, `<=`, `>`, `>=`, `|`, `||`, `&&`, `!`, `-`, `~`, `+`, `*`, `/`, `->`
- Supports literals: integers, floats, strings, booleans
- Supports identifiers and comments

### 2. AST (`src/ast.rs`)
- Complete AST definitions for the Brief language
- Supports all statement types: `Assignment`, `Unification`, `Guarded`, `Term`, `Escape`, `Let`
- Supports all expression types: literals, identifiers, binary operators, unary operators, calls
- Supports all type types: `Int`, `Float`, `String`, `Bool`, `Data`, `Void`, `Custom`, `Union`, `ContractBound`

### 3. Parser (`src/parser.rs`)
- Recursive descent parser producing an AST
- Parses top-level constructs: signatures, definitions, transactions, state declarations, constants, imports
- Parses contracts: `[pre][post]` and `~/` syntactic sugar
- Parses expressions with full operator precedence
- Parses types with union and contract bound support

### 4. Interpreter (`src/interpreter.rs`)
- Basic interpreter for the Brief language
- Supports reactive transactions with STM rollback semantics
- Evaluates expressions and statements
- Manages state with `&` write claims

### 5. Sample Source File (`sample.bv`)
- Demonstrates various Brief language features
- Includes signatures, state declarations, transactions, and definitions

## Usage

```bash
# Build the project
cargo build

# Run the compiler on a Brief source file
cargo run --bin brief-compiler sample.bv

# Run the lexer to see tokens
cargo run --bin tokens sample.bv
```

## Project Structure

```
brief-compiler/
├── src/
│   ├── ast.rs          # AST definitions
│   ├── interpreter.rs  # Interpreter implementation
│   ├── lexer.rs        # Logos-based lexer
│   ├── lib.rs          # Library module exports
│   ├── main.rs         # CLI entry point
│   └── parser.rs       # Recursive descent parser
├── sample.bv           # Sample Brief source file
├── Cargo.toml          # Rust project configuration
└── README.md           # This file
```

## What's Still TODO

The following features from the Brief language specification are not yet fully implemented:

1. **Complete Reactor Loop Implementation**: The interpreter has a basic reactive transaction loop, but the full reactor loop with dependency tracking and event-driven execution is not complete.

2. **Async Transaction Support**: While the parser handles `async` keywords, the interpreter doesn't fully support asynchronous execution and mutual exclusion verification.

3. **Proof Engine**: The compiler doesn't include a proof engine for verifying contract exhaustiveness, mutual exclusion, and other properties.

4. **Code Generation**: The compiler only parses and interprets; it doesn't generate machine code or bytecode.

5. **Error Recovery**: The parser stops on the first error; it doesn't attempt error recovery or provide multiple error messages.

6. **Standard Library**: There's no standard library implementation for common operations.

7. **Type Checking**: Basic type checking is present, but full type inference and verification are not implemented.

## Building and Testing

```bash
# Run all tests
cargo test

# Build in release mode
cargo build --release

# Run with debug output
RUST_LOG=debug cargo run --bin brief-compiler sample.bv
```

## License

This is a work in progress for educational purposes.
