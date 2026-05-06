# Brief

<img src="assets/brief-logo.svg" alt="Brief" width="200"/> <img src="assets/r-brief-logo.svg" alt="Rendered Brief" width="200"/> <img src="assets/e-brief-logo.svg" alt="Embedded Brief" width="200"/>

## Brief Doesn't Break

Brief is a declarative language that tries to prove your code works before it runs.

Instead of commanding the computer through a sequence of steps like most imperative languages,
Brief asks: what should be true before this code runs, what must be true after, and where do we stand now?
The compiler verifies that all logical paths actually lead to the conditions defined in the function's _contract_, a short declaration of what the function is expected to do under which conditions.

This catches bugs that many imperative languages let slip through:

- **Race conditions**: Through a Rust-inspired borrow checker, async transactions
  cannot run simultaneously if any writes to a variable the other might be reading.
- **Unintended side effects**: Every mutation is declared in a precondition/postcondition pair.
  Side effects outside declared boundaries are impossible.
- **Logic errors**: The proof engine traces every path through a transaction and
  verifies each reaches the intended postcondition.
- **Type errors**: Full type checking prevents mismatches before runtime.

If a bug still occurs, it's because the contract didn't fully express the intent.
But that makes the bug easy to find, as the contract shows exactly where expectation
and code diverged.

## The Thesis: Topology over Timing

Brief is not so much a software language, as it tries to be a language of variable logic states.

Most programming languages are built around _operations in sequence_. Temporal instructions that require a CPU to keep track of time. This creates an inherent bottleneck where logic must wait in a single-file line, and if the line needs to be wider, this must be explicitly declared, and all variant states resulting from that accounted for by the programmer.

Brief instead describes the _sequence of operations_, the spatial connections between logical states. That which must happen is inferred automatically, and that which logically cannot will be equally found out at compile time.

*   **Logic as a Map:** Brief defines a world where roads exist all at once. The "sequence" is the _connection_, not the _timing_. 
*   **Physical Isomorphism:** Because the logic defines a *shape* rather than a *schedule*, it adapts to the physics of its material:
    *   **In Software:** The compiler hires a worker (the CPU) to walk these roads in order.
    *   **In Hardware:** The compiler builds the roads directly out of copper.
*   **Variable Logic:** The logic remains invariant while the material changes. A square is a square whether it is drawn in the sand or forged in steel.

By moving from _instruction_ to _connection_, Brief allows a single logic to manifest as either a software schedule or a hardware netlist without changing its fundamental shape.

## Syntax Highlighting & Language Server

VS Code / VSCodium syntax highlighting and **Language Server integration** for `.bv`, `.rbv`, and `.ebv` is available in `syntax-highlighter/`.

The extension automatically launches the `brief lsp` server to provide:
- Real-time type-checking and proof verification diagnostics
- Hover information for transactions and state declarations
- Go to definition for top-level items
- Keyword and type completions

### Installation

**1. Install the Extension:**
```bash
# Install to VSCodium
cp -r syntax-highlighter/ ~/.var/app/com.vscodium.codium/data/vscodium/extensions/brief

# Or install the VSIX
code --install-extension syntax-highlighter/brief-language-0.1.0.vsix
```

**2. Install Dependencies:**
Navigate to the extension folder and install the LSP client:
```bash
cd ~/.var/app/com.vscodium.codium/data/vscodium/extensions/brief
npm install
```

**Note:** Ensure the `brief` compiler is in your PATH so the extension can launch it.

## Quick Start

### 1. Install Brief

**Linux / macOS:**
```bash
# Clone this repository
git clone https://github.com/Randozart/brief-lang.git
cd brief-lang

# Run the installer
chmod +x scripts/brief-install
./scripts/brief-install
```

**Windows:**
```powershell
# Clone this repository (or download ZIP)
git clone https://github.com/Randozart/brief-lang.git
cd brief-lang

# Run the installer
.\scripts\brief-install.bat
```

The installer will copy the compiler to your user bin directory:
- Linux/macOS: `~/.local/bin/brief`
- Windows: `%LOCALAPPDATA%\brief\brief.exe`

Add to your PATH if needed:
```bash
# Linux/macOS
export PATH="$HOME/.local/bin:$PATH"

# Windows: Add %LOCALAPPDATA%\brief to your PATH via System Properties
```

### 2. Create a Project

```bash
brief init my-app
cd my-app
```

This creates a project with:
- `main.bv` - Pure Brief (specification only)
- `main.rbv` - Rendered Brief (specification + web UI)

**Note:** Brief supports three file types for different targets:

| Type | File Type | Description |
|------|-----------|-------------|
| **Brief** | `.bv` | Pure declarative logic |
| **Rendered Brief** | `.rbv` | Brief + Web View (HTML/CSS/SVG) |
| **Embedded Brief** | `.ebv` | Brief + Hardware triggers (trg, @ addresses) |

See [Brief Languages](#brief-languages) below for details on when to use each.

### 3. Build and Run Your App

The `brief build` command is the new standard for compiling. Its behavior depends on the file type:

```bash
# For .bv files (pure logic) -> builds a native Rust executable
brief build main.bv

# For .rbv files (with a UI) -> builds a full web application
brief build main.rbv
```

To run the web app, use `brief run`:
```bash
brief run main.rbv
```
This command builds the `.rbv` file, starts a local server, and opens your app in a browser.

### New `brief wasm` Command
For specific WASM output, use the `wasm` command:
- **`.bv` file**: Generates a pure, standalone WASM binary.
- **`.rbv` file**: Generates a full web application (WASM + JS + Frontend).

```bash
brief wasm main.bv --out ./dist
```

## Commands

Brief uses a unified command structure. While older commands like `brief c` and `brief rust` still work, the recommended commands are:

| Command | Description |
|---|---|
| `brief build <file>` | Compiles a file to its default target. `.bv` -> native executable, `.rbv` -> web app. |
| `brief compile <file> --target <spec>` | Compiles a file to a specific target using a `.toml` spec file. |
| `brief wasm <file>` | Generates a pure WASM binary (`.bv`) or a full web app (`.rbv`). |
| `brief run <file.rbv>` | Builds and serves a `.rbv` file, opening it in your browser. |
| `brief check <file>` | Type checks and verifies a file without compiling. |
| `brief init [name]` | Creates a new Brief project. |
| `brief lsp` | Starts the Language Server for IDE integration. |

### Direct Compilation Commands

These commands provide shortcuts for compiling to a specific language target without needing a full `--target` spec file. They are useful for quick tests and single-file projects.

| Command | Description |
|---|---|
| `brief c <file>` | Compiles to C. Handles `.bv` and `.ebv` for hosted or bare-metal targets. |
| `brief rust <file>` | Compiles to native Rust (std). |
| `brief arm <file>` | Compiles to ARM bare-metal Rust. |
| `brief verilog <file.ebv>` | Compiles to SystemVerilog. Requires `--hw` config. |
| `brief vhdl <file.ebv>` | Compiles to VHDL. Requires `--hw` config. |
| `brief cobol <file>` | Compiles to IBM Enterprise COBOL. |

### Data Brief for Configuration

Hardware configuration has been modernized with **Data Brief**, a schema-enforced configuration system using `.dbv` and `.dbvs` files, replacing the older `hardware.toml` files. This allows for safer and more reliable hardware definitions.

See `DATABRIEF.md` for more details.

## Why Declarative?

The specific declarative logic in Brief is inspired by several domains:

- **Prolog** and other pure logic languages showed that logical inference can replace explicit control flow
- **Dialog & Inform** (both interactive fiction languages) demonstrated how declarative
  state machines can elegantly express complex behavior
- **Rust** influenced how Brief handles state and correctness through strict
  boundaries and smart design principles baked into the language itself
- **React** inspired Rendered Brief's component model
- **SQL transactions** inspired Brief's transaction syntax and atomicity semantics

The insight: if you declare what must be true before and after a code block
(known more formally as a Hoare triple), the compiler can verify the transition is valid. This turns
assertions from optional boilerplate into mandatory first-class citizens.

## How the Reactor Works

Brief runs on a reactor loop that continuously checks if transactions are ready to fire:

1. All transactions declare preconditions (what must be true to run)
2. When a variable changes, the reactor re-evaluates affected transactions
3. Any _reactive_ transaction with a satisfied precondition fires automatically
4. Each firing updates state, potentially triggering more transactions
5. When nothing can fire, the reactor reaches equilibrium and waits

This replaces manual polling and event dispatchers with logical evaluation.
Instead of: _"check this condition, then fire this handler, then check that condition..."_
Brief says: _"When A is true, do B. When B is true, do C."_ The reactor and logical sequence figure out the rest.

## Example: A Complete Lifecycle

```brief
let counter: Int = 0;
let ready: Bool = false;

// Passive transaction (must be explicitly called)
txn initialize [~/ready] {
  &ready = true;
  term;
};

// Reactive transaction (fires automatically when precondition met)
rct txn increment [ready && counter < 5][counter > @counter] {
  &counter = counter + 1;
  term;
};

// Another reactive that depends on the first
rct txn notify_complete [ready && counter == 5][true] {
  log("Count complete!");
  term;
};
```

Walkthrough:
- `initialize` must be explicitly called, as it does not have the `rct` keyword. Its contract `[~/ready]` is synctactic sugar for `[~ready][ready]`, which means
  _"precondition: ready is false, postcondition: ready is true"_
- Once `initialize` fires, `ready` is mutated to become true inside the transaction
- Since `ready` is out of scope for initialise, it claims exclusive write access using the `&` symbol
- `term;` checks whether the postcondition is fulfilled, and exits the transaction if it does. If it does not, it keeps looping the transaction until it does, unless no path to the postcondition will be possible, which would have been caught at compile time. Here, we just need a single iteration to set `ready` to true 
- This, in turn, satisfies `increment`'s precondition: `ready && counter < 5`
- When `increment` fires, `counter` increases by 1. Its postcondition `[counter > @counter]`
  verifies the compiler that counter actually increased (@ refers to the value at transaction start)
- After 5 increments, `counter == 5`, satisfying `notify_complete`'s precondition
- The reactor handles the cascade automatically

Each transaction's postcondition is a guarantee the compiler verifies.
If any path through the transaction could violate it, compilation fails.

## Example of Caught Error: Broken Reactive Cascade

The real power comes when the compiler detects conflicts between reactive transactions:

```brief
let counter: Int = 0;
let ready: Bool = false;

rct txn initialize [~ready][ready] {
  &ready = true;
  term;
};

rct txn increment [ready && counter < 5][counter > @counter] {
  &counter = counter + 1;
  term;
};

// This transaction depends on counter reaching 5
rct txn notify_complete [counter == 5][true] {
  log("Done!");
  term;
};

// This is where we get offending code:
rct txn bad_reset [counter > 0][counter == 0] {
  &counter = 0;
  term;
};
```

The problem: `bad_reset` fires whenever `counter > 0`, immediately resetting it to 0.
This violates the logical chain needed to reach `counter == 5`, making `notify_complete` unreachable.

The compiler catches this and reports:

```
P001: ownership conflict in reactive cascade

transactions 'increment' and 'bad_reset' have conflicting reactive paths.
The postcondition of 'increment' ([counter > @counter]) would be immediately 
violated by 'bad_reset' ([counter == 0]), making 'notify_complete' unreachable.

Proof chain:
1. 'increment' fires when ready && counter < 5
2. 'increment' increments counter, satisfying postcondition counter > @counter
3. But 'bad_reset' fires when counter > 0, resetting counter to 0
4. This violates the logical chain needed to reach counter == 5
5. Therefore 'notify_complete' cannot fire: unreachable postcondition

Hint: resolve the conflict by either:
- Adding a guard to prevent 'bad_reset' from interfering with 'increment'
- Removing the conflicting transaction
- Reordering the reactive chain to be logically consistent
```

The compiler forces you to think about the entire reactive system as a coherent whole,
not just individual transactions. Every transaction's postcondition must flow logically 
into the next, or compilation fails.

## Brief Languages

Brief comes in three variants, each targeting different output:

| Language | File Type | Description | Transpilation Targets |
|----------|-----------|-------------|----------------------|
| **Brief** | `.bv` | Pure declarative logic | C, Rust, WebAssembly, COBOL |
| **Rendered Brief** | `.rbv` | Brief + Web View (HTML/CSS/SVG) | Browser (HTML/JS/CSS) |
| **Embedded Brief** | `.ebv` | Brief + Hardware triggers | SystemVerilog + TCL, ARM Rust, C bare-metal |
| **Data Brief** | `.dbv` | Provides concrete hardware configuration values | Configuration |
| **Data Brief Schema** | `.dbvs` | Defines hardware schemas, templates, and aliases | Configuration |
| **Data Brief Lines** | `.dbvl` | Line-based mutable database for large datasets | Data Storage & Verification |

### Key Differences

- **`.bv`** — Pure specification. No `@` address bindings, no `trg` hardware triggers.
  Compiles to software targets (C, Rust, WASM, COBOL).

- **`.rbv`** — `.bv` syntax plus a `view` block with HTML/CSS/SVG components.
  Generates a web UI; the logic compiles to WebAssembly.

- **`.ebv`** — `.bv` syntax plus `trg name: Type @ address` hardware triggers
  and `@` memory-mapped I/O bindings. Targets:
  - **FPGA/ASIC**: SystemVerilog via `brief verilog` (with optional TCL build scripts)
  - **ARM bare-metal**: Rust via `brief arm` or C via `brief c`



## Full Language

- **Transactions**: `txn` and `rct txn` blocks with contracts
- **State**: Global variables (`let`, `const`)
- **Types**: String, Int, UInt, Float, Bool, Void, Vector, custom structs
- **Contracts**: Preconditions `[pre]` and postconditions `[post]`
- **Prior state**: `@variable` references the value at transaction start
- **Pattern matching**: Unification for handling multiple outcomes
- **Imports**: Modular code
- **Definitions**: Named functions with contracts (`defn`)
- **FFI**: Call other languages from Brief

## Documentation

- [SPEC.md](spec/SPEC.md) - Full language spec (v7.0)
- [LANGUAGE-REFERENCE.md](spec/LANGUAGE-REFERENCE.md) - Detailed language manual
- [FFI-GUIDE.md](spec/FFI-GUIDE.md) - Writing FFI interfaces
- [RENDERED-BRIEF-GUIDE.md](spec/RENDERED-BRIEF-GUIDE.md) - Web UI framework
- [QUICK-REFERENCE.md](spec/QUICK-REFERENCE.md) - Syntax cheat sheet
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
