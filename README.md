# Brief

A language for verified state machines. The compiler proves all state transitions are valid before your code runs.

## Install

```bash
cargo install --path .
```

## Usage

```bash
brief check program.bv          # Type check
brief build program.bv          # Run program
brief init my-project           # Create project
brief import lib --path ./lib   # Add dependency
brief lsp                       # Start language server
```

## Language

Brief programs declare state and transactions on that state. Each transaction has a pre-condition (when it can run) and a post-condition (what must be true after it runs). The compiler verifies these conditions are actually satisfied.

```brief
let balance: Int = 100;

txn withdraw(amount: Int) 
  [amount > 0 && amount <= balance]      # Pre-condition
  [balance == @balance - amount]         # Post-condition
{
  &balance = balance - amount;
  term;
};
```

The compiler proves: if the pre-condition is true, the code will execute and make the post-condition true.

## Features

- **Transactions**: Named state transitions with pre/post conditions
- **Signals**: Reactive state variables
- **Type system**: String, Int, Float, Bool, Void, custom structs
- **Pattern matching**: Destructure data safely
- **Imports**: Modular code organization
- **FFI**: Call Rust functions (59 stdlib functions included)
- **Proof engine**: Verifies all transaction contracts
- **Incremental compilation**: Fast feedback during development

## Project Structure

```
src/
├── lexer.rs       Tokenization
├── parser.rs      Parsing
├── ast.rs         AST definitions
├── typechecker.rs Type checking
├── proof_engine.rs Contract verification
├── interpreter.rs Execution
├── reactor.rs     Event loop
├── ffi/           Foreign function interface
└── main.rs        CLI

std/bindings/      Standard library FFI bindings
spec/              Documentation
examples/          Example programs
```

## Documentation

- [brief-lang-spec.md](spec/brief-lang-spec.md) - Language spec
- [FFI-USER-GUIDE.md](spec/FFI-USER-GUIDE.md) - Creating FFI bindings
- [FFI-STDLIB-REFERENCE.md](spec/FFI-STDLIB-REFERENCE.md) - Stdlib functions
- [ARCHITECTURE.md](spec/ARCHITECTURE.md) - Compiler architecture
- [examples/](examples/) - Example programs

## Building

```bash
cargo build --release
```

## Testing

```bash
cargo test --lib          # Unit tests
cargo test                # All tests
```

## Rendered Brief (Web UI)

Brief can compile to WebAssembly for reactive web components:

```brief
<script type="brief">
  let count: Int = 0;
  txn increment [true][count == @count + 1] {
    &count = count + 1;
    term;
  };
</script>

<view>
  <p b-text="count">0</p>
  <button b-trigger="increment">+</button>
</view>
```

Compile and serve:

```bash
brief rbv component.rbv --out dist/
cd dist && wasm-pack build --target web
python3 -m http.server 8080
```

## Status

| Component | Status |
|-----------|--------|
| Core Language | Production |
| Type System | Production |
| Proof Engine | Production |
| FFI System | Production |
| Rendered Brief (UI) | In Progress |

## How It Works

1. **Lexer** tokenizes input
2. **Parser** builds AST
3. **Type checker** ensures type correctness
4. **Proof engine** verifies transaction contracts
5. **Interpreter** executes verified code

At each step, errors are caught and reported clearly. If the code compiles, the proof engine has verified all state transitions are valid.

## Contributing

Issues: bug fixes, documentation, more stdlib bindings, performance improvements, Rendered Brief completion.

## License

MIT
