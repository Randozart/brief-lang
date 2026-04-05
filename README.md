# Brief

<img src="brief-logo-draft.jpg" alt="Brief" width="400"/>

**Brief doesn't break.** A language where state transitions are mathematically verified at compile time — eliminating entire categories of runtime errors.

## The Problem

State management is hard. Your app crashes because:
- A state transition was invalid but nobody checked before executing
- Race conditions corrupt state in concurrent code
- Edge cases slip through testing
- Nobody can prove the system is correct

**Brief fixes this by making the compiler verify all state transitions before your code runs.**

## How It Works

Every state change in Brief is a **transaction** with proven pre- and post-conditions:

```brief
let balance: Int = 100;

txn withdraw(amount: Int) 
  [amount > 0 && amount <= balance]           # Pre: amount is valid
  [balance == @balance - amount]              # Post: balance decreases by amount
{
  &balance = balance - amount;
  term;
};
```

The compiler proves:
- If the pre-condition holds, the code will execute
- When it executes, the post-condition will be true
- All execution paths are reachable and valid

This happens at **compile time**, not runtime.

## Key Benefits

**Correctness**: State transitions are mathematically proven before execution  
**Concurrency**: Lock-free, safe concurrent operations (preconditions act as gates)  
**Simplicity**: No boilerplate — just declare what should be true, compiler verifies it  
**Performance**: All verification happens at compile time, zero runtime overhead  
**Confidence**: Deploy knowing your state machine is provably correct  

## Quick Start

### Install

```bash
cargo install --path .
```

### Usage

```bash
# Type check and verify
brief check program.bv

# Run the program
brief build program.bv

# Create a new project
brief init my-project

# Add a library dependency
brief import my_lib --path ./lib/my_lib.bv
```

### Example Program

```brief
# State
let count: Int = 0;
let locked: Bool = false;

# Increment transaction
txn increment 
  [!locked && count < 1000000]           # Only increment if not locked and under limit
  [count == @count + 1]                  # Guarantee count increases by 1
{
  &count = count + 1;
  term;
};

# Lock transaction  
txn lock 
  [true]                                 # Can always lock
  [locked == true]                       # Guarantee locked is true after
{
  &locked = true;
  term;
};
```

## What Brief Includes

### Core Language
- **Transactions**: Named blocks with proven pre/post conditions
- **Signals**: Reactive state variables
- **Type System**: String, Int, Float, Bool, Void
- **Pattern Matching**: Destructure and validate data
- **Imports**: Reusable modules and libraries

### Compiler Pipeline
- Lexer → Parser → Type Checker → Proof Engine → Interpreter
- Incremental compilation with caching
- Live file watching for development
- Clear error messages with diagnostic hints

### UI Framework (Rendered Brief)
Build reactive web UIs where Brief logic owns the state:

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
  <button b-trigger="increment">Click me</button>
</view>
```

Compiles to WebAssembly with automatic JS bindings.

### Foreign Function Interface (FFI)
Call external Rust functions safely. 59 stdlib functions included:

```brief
frgn read_file(path: String) -> Result<String, IoError> from "std::io";

defn load_config(path: String) -> String [true][true] {
  let content: String = read_file(path);
  content;
};
```

See [FFI-USER-GUIDE.md](spec/FFI-USER-GUIDE.md) for details.

## Documentation

| Document | Purpose |
|----------|---------|
| [brief-lang-spec.md](spec/brief-lang-spec.md) | Language specification |
| [FFI-USER-GUIDE.md](spec/FFI-USER-GUIDE.md) | Creating and using FFI bindings |
| [FFI-STDLIB-REFERENCE.md](spec/FFI-STDLIB-REFERENCE.md) | Reference for 59 stdlib functions |
| [ARCHITECTURE.md](spec/ARCHITECTURE.md) | Compiler architecture |

## Project Structure

```
src/
├── lexer.rs         # Tokenization
├── parser.rs        # Syntax analysis
├── ast.rs           # AST definitions
├── typechecker.rs   # Type checking and inference
├── proof_engine.rs  # Contract verification
├── interpreter.rs   # Execution engine
├── reactor.rs       # Event loop for reactive execution
├── resolver.rs      # Module import resolution
├── ffi/             # Foreign function interface
│   ├── mod.rs
│   ├── loader.rs
│   ├── validator.rs
│   ├── resolver.rs
│   └── types.rs
├── errors.rs        # Error types and diagnostics
└── main.rs          # CLI interface

std/bindings/        # Standard library FFI bindings
├── io.toml          # File I/O
├── math.toml        # Math functions
├── string.toml      # String manipulation
└── time.toml        # Timing functions

spec/                # Documentation
tests/               # Integration tests
examples/            # Example programs
```

## Development

### Run Tests

```bash
# Unit tests
cargo test --lib

# All tests
cargo test
```

### Build the Compiler

```bash
cargo build --release
```

### LSP Server (IDE Integration)

Brief includes a Language Server Protocol implementation:

```bash
brief lsp
```

Integrates with VSCode, Neovim, Emacs, and other LSP-compatible editors.

### VSCode Syntax Highlighting

Copy the syntax highlighter extension:

```bash
cp -r syntax-highlighter/ ~/.config/Code/User/extensions/brief
```

## Rendered Brief (Web UI)

Create reactive web components where Brief logic drives HTML:

```bash
# Compile .rbv file to WebAssembly
brief rbv component.rbv --out dist/

# Outputs:
# - component.rs (Brief logic compiled to Rust)
# - component_glue.js (event bridge)
# - component.html (page)
# - component.css (styles)

# Build WebAssembly
cd dist && wasm-pack build --target web

# Serve
python3 -m http.server 8080
```

Open `http://localhost:8080/component.html` in your browser.

## Status

| Component | Status |
|-----------|--------|
| Core Language | Production |
| Type System | Production |
| Proof Engine | Production |
| FFI System | Production |
| Stdlib Bindings | Production |
| Rendered Brief (UI) | In Progress |

## Philosophy

Brief starts with a simple principle: **don't let runtime errors happen**.

Rather than catching errors after they occur, Brief prevents entire categories of errors from existing. State transitions are proven correct before execution. Concurrency is safe by design. Type mismatches are impossible.

This is practical formal verification — not academic, but built into the language from the ground up.

## Influence & Inspiration

Brief draws ideas from:
- **Reactive programming**: State as the source of truth
- **Proof systems**: Formal verification for correctness
- **Transactional semantics**: ACID properties for state
- **Functional languages**: Immutability and pure functions
- **Rust**: Type safety and memory efficiency

(Yes, code generation and symbolic execution are used during compilation, but the language itself is straightforward.)

## Contributing

Contributions welcome. See issues for areas needing work. Most valuable contributions:

1. **Stdlib bindings** - Add more useful FFI functions
2. **Documentation** - Clear examples and guides
3. **Bug fixes** - Issues labeled "bug"
4. **Performance** - Compilation speed and code generation
5. **Rendered Brief** - UI framework completion

## License

MIT

---

**Learn more**: Start with [examples/](examples/) or read [spec/brief-lang-spec.md](spec/brief-lang-spec.md).

**Questions?** Open an issue or start a discussion.
