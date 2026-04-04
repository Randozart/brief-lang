# Brief Compiler

<img src="brief-logo-draft.jpg" alt="Brief" width="400"/>

A transactional, contract-enforced language compiler. Brief treats program execution as verified state transitions with mathematical proofs at compile time.

## Status

**Active development.** The core compiler (lexer, parser, typechecker, interpreter) is complete. Rendered Brief (`.rbv`) UI framework is in progress.

## What is Brief?

Brief is a declarative language where:

- **Transactions are contracts.** Every state change is proven valid before execution.
- **No runtime surprises.** The compiler verifies all state transitions, not the runtime.
- **Lock-free concurrency.** Preconditions act as hardware-level gates — no mutexes needed.
- **Formal verification without boilerplate.** Reactive state machines with pre/post conditions.

```brief
let balance: Int = 100;
let withdrawn: Int = 0;

txn withdraw(amount: Int) [amount > 0 && amount <= balance][balance == @balance - amount] {
  &balance = balance - amount;
  &withdrawn = withdrawn + amount;
  term;
};
```

## Installation

```bash
cargo install --path .
```

## Usage

```bash
# Type check without execution
brief check program.bv

# Build/execute
brief build program.bv

# Initialize a new project
brief init my-project

# Add a dependency
brief import <name> --path <location>
```

## Project Structure

```
src/
├── lexer.rs        # Tokenizer
├── parser.rs       # Recursive descent parser
├── ast.rs          # AST definitions
├── typechecker.rs  # Type inference and verification
├── proof_engine.rs # Contract and reachability proofs
├── interpreter.rs  # Reactive execution engine
├── reactor.rs      # Event-driven reactor loop
├── resolver.rs     # Import resolution
├── manifest.rs     # Dependency management (brief.toml)
├── cache.rs        # Incremental compilation cache
├── watch.rs        # File watching
└── main.rs         # CLI
```

## Documentation

| Spec | Description |
|------|-------------|
| [brief-lang-spec.md](spec/brief-lang-spec.md) | Brief language specification |
| [rendered-brief-spec-v4.md](spec/rendered-brief-spec-v4.md) | Rendered Brief (`.rbv`) UI framework |
| [ARCHITECTURE.md](spec/ARCHITECTURE.md) | Compiler architecture decisions |
| [brief-compiler-build-plan.md](spec/brief-compiler-build-plan.md) | Implementation roadmap |

## VSCode Extension

Syntax highlighting for `.bv` and `.rbv` files is included in `syntax-highlighter/`. Install to VSCodium:

```bash
cp -r syntax-highlighter/ ~/.var/app/com.vscodium.codium/data/vscodium/extensions/brief
```

## Rendered Brief

Rendered Brief (`.rbv`) is a reactive UI framework where Brief logic and HTML coexist in a single file.

<img src="r-brief-logo-draft.jpg" alt="Rendered Brief" width="400"/>

The Brief logic owns all state; HTML and CSS are declarative projections of that state. No virtual DOM, no component tree — just bindings.

```html
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

## License

MIT
