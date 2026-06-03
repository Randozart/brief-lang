# Brief

<img src="assets/brief-logo.svg" alt="Brief" width="200"/><img src="assets/r-brief-logo.svg" alt="Rendered Brief" width="200"/> 

<img src="assets/e-brief-logo.svg" alt="Embedded Brief" width="200"/><img src="assets/d-brief-logo.svg" alt="Embedded Brief" width="200"/>

## Brief Doesn't Break

**Status:** v0.14.0 - Multi-Backend, FFI-Connected, LLVM-Ready

Brief is a declarative, contract-enforced logic language designed for building verifiable state machines. It treats program execution as a series of verified state transitions rather than sequential instructions. Due to this, it transpiles well to many imperative languages by inferring what instructions must happen for each new state to become true, and writing that in the target code. Due to its declarative nature, this means it handles both software transpilation (LLVM, COBOL), hardware transpilation (SystemVerilog, VHDL), embedded transpilation, web transpilation (by combining WASM, HTML, CSS and SVG, and gluing these together with a thing layer of JS. Also TypeScript and TSX).

The main sources of inspiration are Rust (by Graydon Hoare and the Rust community) and Dialog (by Linus Åkesson). Specifically the fact that both have a very strict compiler, that catches bad code before it ever compiles, simply through smart conventions. Especially the declarative nature is inspired by Dialog, as a direct successor of Prolog, since Dialog showed that setting up a series of predicates could be sufficient to have a compiler figure out a complex runtime capable of simulating a world. And the reactor loop? That was inspired by, well... React. As such, everything in Brief is designed to, in some way, aid in predictable runtime cascades. You set up the first billiard ball, and based on the variables present describing the overall "state", the rest of the balls predictably scatter.

Note that much of this language design was inspired by designing a language that would be impossible for an LLM to get wrong. Therefore it feels important to me to disclaim a lot of AI has been used in building this compiler. The design is fully my own (Randy Smits-Schreuder Goedheijt), but much of the programming was handled by LLMs, and the verification by hand and a series of unit tests (which LLMs somehow manage to cut the corners of). As such, you will find comments, markdown files and many more typical signs of LLM usage in this repository. These all exist to help steer the LLM into *correctly* modifying and applying the design decisions I have made, as it would otherwise be prone to hallucinate a novel language like this. Ergo, you will find a veritable library of markdown files written by AI, just to make sure everything got documented as I went.

If you've gotten this far, I thank you for reading, and I hope you will have enjoyed your *Brief* time here so far.

Regards,

**Randy**

## The Thesis: Topology over Timing

Most programming languages are built around _operations in sequence_. Brief describes the _sequence of operations_ - the spatial connections between logical states.

*   **Logic as a Map:** Brief defines a world where roads exist all at once. The "sequence" is the _connection_, not the _timing_.
*   **Physical Isomorphism:** Because the logic defines a _shape_ rather than a _schedule_, it adapts to the physics of its material:
    *   **In Software:** The compiler hires a worker (the CPU) to walk these roads in order.
    *   **In Hardware:** The compiler builds the roads directly out of copper.
*   **Variable Logic:** The logic remains invariant while the material changes. A square is a square whether it's drawn in the sand or forged in steel.

**Deep Dive:** There are several .md documents scattered across the repo with random ideas on optimizing the language. Some are outdated, some aren't, but they should show the development of the Brief philosophy over time, and ways in which the topological approach has allowed backend optimization not otherwise available.

## The Philosophical Pillars

*   **All operations are expressed in transactions, and only transactions can call operations. They either complete fully, or not at all.**
    *   Transactions are inherently cyclical. If you properly define a postcondition a cyclically executed transaction will eventually reach, it automatically starts behaving like a loop, but one that can predictably halt.
*   **Brief doesn't need you to be correct, it just needs you to be right.**
    *   The contract logic often just requires you to declare either the precondition or postcondition, not both.
    *   Contracts are simultaneously specification AND optimization input. In most languages, types/specs are safety rails that constrain what you can do. In Brief, they're also what the optimizer feeds on. The more you declare, the more the compiler can prove, and the faster your program runs.    
    *   The file extension system (.bv → warnings, .sbv → hard errors) embodies the idea that you opt into strictness as your understanding deepens. Partial contracts compile with warnings. Full contracts with strict mode compile with proofs. This is a choice that distinguishes Brief from total languages (Coq, Agda) where you must prove everything upfront, and from mainstream languages where you prove nothing.
*   **Execution is inferred, not prescribed.**
    *   Programs are declared through a combination of variables, definitions and transactions.
    *   The entire program runs on a non-polling reactor loop. It indexes which variable changes lead to which `rct txn` preconditions to be fulfilled, and fires them automatically when it's their time to act.
    *   Because these paths are laid out predictably, the compiler has great leeway in folding these paths. If X through A, B and C will always lead to Y with side-effect Z, the compiler will simply draw a short route from X to YZ.
*   **No magic.**
    *   Every function and keyword in Brief must be traceable to a source following the same rules as every other definition.
    *   The compiler is not allowed to have any baked in function calls. These must all trace to a library file.
    *   Anything interacting with an external language or interrupt source must be declared explicitly through the Metropolitan FFI interface.

## Quick Start

### 1. Build the Compiler

```bash
# Build in debug mode
cargo build

# Build in release mode
cargo build --release

# Run tests
cargo test --lib
```

### 2. Create Your First Program

Create `counter.bv`:

```brief
let counter: Int = 0;

txn increment() [counter < 100][counter == @counter + 1] {
    &counter = counter + 1;
    term;
};

txn main() [true][true] {
    increment();
    increment();
    increment();
    term;
};
```

### 3. Compile and Run

```bash
# Type-check only (fast)
./target/debug/brief-compiler check counter.bv

# Check in strict mode (.sbv files or --strict flag)
./target/debug/brief-compiler check counter.sbv
./target/debug/brief-compiler check --strict counter.bv

# Compile to Rust
./target/debug/brief-compiler rust counter.bv

# Compile to C
./target/debug/brief-compiler c counter.bv

# Compile to LLVM IR
./target/debug/brief-compiler llvm counter.bv

# Compile to AArch64 assembly
./target/debug/brief-compiler compile counter.bv --target aarch64.dbvs

# Compile to x86-64 assembly
./target/debug/brief-compiler compile counter.bv --target x86_64.dbvs

# Generate FFI bindings for a foreign library
./target/debug/brief-compiler bind mylib.h

# Connect to a Metropolitan shared memory service
./target/debug/brief-compiler metrod connect WeatherApi

# Start the LSP server
./target/debug/brief-compiler lsp
```

## Language Variants

| Type | File Extension | Description | Targets |
|------|----------------|-------------|---------|
| <img src="assets/brief-icon.svg" alt="Brief" width="25" style="vertical-align: middle;"/> **Brief** | `.bv` | Pure declarative logic | LLVM into native binary, COBOL |
| <img src="assets/r-brief-icon.svg" alt="Brief" width="25" style="vertical-align: middle;"/> **Rendered Brief** | `.rbv` | Brief + Web UI (HTML/CSS/SVG) | Browser (WASM + JS + HTML + CSS) |
| <img src="assets/e-brief-icon.svg" alt="Brief" width="25" style="vertical-align: middle;"/> **Embedded Brief** | `.ebv` | Brief + Hardware triggers | FPGA (VHDL/SystemVerilog), ARM bare-metal |
| <img src="assets/d-brief-icon.svg" alt="Brief" width="25" style="vertical-align: middle;"/> **Data Brief** | `.dbv` | Configuration data | All targets |
| <img src="assets/d-brief-icon.svg" alt="Brief" width="25" style="vertical-align: middle;"/> **Data Brief Schema** | `.dbvs` | Schema/FFI bindings | All targets |
| <img src="assets/d-brief-icon.svg" alt="Brief" width="25" style="vertical-align: middle;"/> **Data Brief Lines** | `.dbvl` | Line-based databases | All targets |

## Key Features

### 1. Contracts First

Every transaction declares what must be true **before** and **after**:

```brief
txn withdraw(amount: Int) 
    [amount > 0 && balance >= amount]  // Precondition
    [balance == @balance - amount]      // Postcondition
{
    &balance = balance - amount;
    term;
};
```

The compiler **verifies** that your code satisfies these contracts.

### 2. Reactive by Default

Transactions fire automatically when preconditions are met:

```brief
rct txn auto_save() [dirty && !saving][!dirty] {
    save_to_disk();
    &dirty = false;
    term;
};
```

No event handlers. No polling. Just logic.

### 3. Zero-Nesting Logic

No `if/else` chains. Use guards:

```brief
[x > 0] {
    &result = x * 2;
};
[x < 0] {
    &result = x * -1;
};
[x == 0] {
    escape;  // Rollback
};
```

### 4. Compile-Time Verification

The compiler proves:
- No race conditions
- No unintended side effects  
- All contracts are satisfied
- No deadlocks in async code

## Compiler Architecture

```
Source (.bv/.sbv/.rbv/.ebv)
    ↓
Lexer (lexer.bv) → List<Token>
    ↓
Parser (parser.bv, ast.bv) → AST
    ↓
Import Resolver (import_resolver.rs) → Resolved AST
    ↓
Desugarer (desugarer.rs) → Desugared AST
    ↓
Type Checker (typechecker.bv) → Typed AST
    ↓
Proof Engine (proof_engine.bv) → Verified AST
    ↓
Shared Analysis
├── CallGraph (call_graph.bv) — Cycle detection, acyclic optimization
├── Range Analysis (range.bv) — Parameter bounds inference
├── Dataflow (analysis/dataflow.rs) — Read/write dependencies
└── Protocol (analysis/protocol.rs) — Control register prerequisites
    ↓
FFI Layer (Metropolitan)
├── DBVS Bindings (std/bindings/*.dbvs) — Interface definitions
├── Registry (ffi/registry.rs) — 60+ Rust implementation functions
├── Orchestrator (ffi/orchestrator.rs) — Native + Metropolitan dispatch
├── Sentinel (ffi/sentinel.rs) — Pre/post-condition validation
├── NativeMapper (ffi/native_mapper.rs) — Byte serialization
└── Metropolitan Hub (ffi/metropolitan.rs) — Shared memory IPC + codegen
    ↓
Backends
├── AArch64 (aarch64.rs) — ARM64 assembly
├── x86-64 (x86_64.rs) — AMD64 assembly
├── Rust (rust.rs) — Rust source
├── C (c.rs) — C source
├── LLVM (llvm.rs) — LLVM IR
├── WASM (wasm.rs) — WebAssembly text format
├── Webstack (webstack.rs) — Rust + wasm-bindgen + JS
├── COBOL (cobol.rs) — COBOL source
├── VHDL (vhdl.rs) — FPGA
├── SystemVerilog (verilog.rs) — FPGA/ASIC
└── TCL Generator (tcl_generator.rs) — Vivado build scripts
    ↓
LSP Server (lsp.rs)
├── Hover — Type information
├── Definition — Go-to-definition
├── Completions — Context-aware keyword/field/hashtag suggestions
├── Document Symbols — Outline view
├── Workspace Symbols — Cross-file symbol search
├── Strict Mode Detection — .sbv/.sebv/.srbv extension handling
└── Diagnostics — Typechecker + proof engine errors
```

## Standard Library

### Core Types
- **Char** - Unicode codepoints
- **HashMap<K,V>** - O(1) lookup
- **HashSet<T>** - O(1) membership
- **StringBuilder** - O(n) string building
- **Stack<T>** - LIFO structure
- **Queue<T>** - FIFO structure
- **Result/Option** - Error handling with combinators

### String Processing
- Character classification (`is_whitespace`, `is_digit`, `is_alpha`)
- Case conversion (`to_upper`, `to_lower`, `capitalize`)
- String manipulation (`trim`, `reverse`, `split`, `join`)
- 95% native functions (no FFI)

### Collections
- `List<T>` - Dynamic arrays
- Vector operations
- Sorting, filtering, mapping

### IO & Process
- File I/O (`read_file`, `write_file`, `file_exists`)
- Path operations (`join_path`, `split_path`, `file_extension`)
- Process spawning (`spawn`, `spawn_with_output`)
- Environment access (`env_var`, `current_dir`)

### Iterators
- `map`, `filter`, `fold`
- `take`, `skip`, `zip`, `chain`
- `sum`, `product`, `min`, `max`
- `find`, `any`, `all`

**Total:** 300+ native functions across 15+ modules

## Learning Brief

Start with our comprehensive tutorial:

```bash
cd learn-brief
```

1. **[00-welcome.md](learn-brief/00-welcome.md)** - What is Brief?
2. **[01-basics.md](learn-brief/01-basics.md)** - Variables, types, transactions
3. **[02-contracts.md](learn-brief/02-contracts.md)** - Preconditions & postconditions
4. **[03-reactive.md](learn-brief/03-reactive.md)** - Reactive transactions
5. **[11-triggers.md](learn-brief/11-triggers.md)** - Triggers and reactive I/O (`trg`, `trg!`)

**Full documentation:**
- [spec/SPEC.md](spec/SPEC.md) - Complete language specification
- [spec/LANGUAGE-TUTORIAL.md](spec/LANGUAGE-TUTORIAL.md) - Detailed tutorial
- [spec/QUICK-REFERENCE.md](spec/QUICK-REFERENCE.md) - Syntax cheat sheet
- [lib/std/README.md](lib/std/README.md) - Standard library guide
- [lib/compiler/README.md](lib/compiler/README.md) - Compiler architecture

---

## Examples

### Counter (Basic)
```brief
let counter: Int = 0;

txn increment() [counter < 100][counter == @counter + 1] {
    &counter = counter + 1;
    term;
};
```

### Bank Account (Contracts)
```brief
let balance: Int = 1000;

txn deposit(amount: Int) 
    [amount > 0]
    [balance == @balance + amount]
{
    &balance = balance + amount;
    term;
};

txn withdraw(amount: Int) 
    [amount > 0 && balance >= amount]
    [balance == @balance - amount]
{
    &balance = balance - amount;
    term;
};
```

### Shopping Cart (Reactive)
```brief
let items: Int = 0;
let total: Float = 0.0;
let discount_applied: Bool = false;

rct txn add_item(price: Float) [true][items == @items + 1] {
    &items = items + 1;
    &total = total + price;
    term;
};

rct txn apply_discount() 
    [items > 10 && total > 100.0 && !discount_applied]
    [total < @total && discount_applied == true]
{
    let discount = total * 0.1;
    &total = total - discount;
    &discount_applied = true;
    term;
};
```

See [examples/](examples/) for more.

## Performance

| Component | Complexity | Optimization |
|-----------|------------|--------------|
| **HashMap** | O(1) | Hash lookup |
| **StringBuilder** | O(n) | Amortized append |
| **Lexer** | O(n) | Single pass |
| **Parser** | O(n) | Recursive descent |
| **Type Checker** | O(n·α(n)) | Union-Find |
| **Proof Engine** | O(2^b) pruned | BFS with pruning |
| **Code Generation** | O(n) | Single pass |

**See:** [OPTIMIZATIONS.md](OPTIMIZATIONS.md) for complete analysis.

## Testing

```bash
# Run all library tests
cargo test --lib

# Run specific test module
cargo test --lib lexer::tests
cargo test --lib analysis::call_graph::tests
cargo test --lib backend::llvm::tests

# Check a Brief file
./target/debug/brief-compiler check examples/counter.rbv

# Check in strict mode
./target/debug/brief-compiler check --strict counter.sbv
```

**Test Suite (269 tests):**
- `tests/tier1/` - Core data type tests
- `tests/tier2/` - String processing tests
- `tests/backends/` - Backend generation tests
- `tests/ffi_*.rs` - FFI parser, typechecker, stdlib, proof engine tests
- `tests/fuzz_frontend.rs` - AST roundtrip fuzzing
- `tests/fuzz_backend.rs` - Backend codegen fuzzing
- `tests/fuzz_fault_injection.rs` - Error recovery fuzzing
- `src/analysis/*/tests/` - CallGraph, Range analysis, etc.
- `src/ffi/metropolitan.rs/tests` - Shared memory IPC tests

**See:** [docs/reports/TESTING_SUMMARY.md](docs/reports/TESTING_SUMMARY.md) for complete results.

---

## Self-Hosting Status

The Brief compiler can now:
- Parse itself
- Type-check itself
- Verify its own contracts
- Generate code for itself (all 11 backends)
- Run shared analysis (CallGraph, range inference) in both Rust and Brief

**Implementation:**
- 9 architectural tiers (100% complete)
- 11 production backends
- 300+ standard library functions
- ~46,000 lines of Rust bootstrap compiler
- ~8,500 lines of Brief self-hosted compiler
- ~7,500 lines of documentation
- 269 passing tests

**Key v0.14.0 additions:**
- Metropolitan FFI: shared memory IPC with C/Python/JS/Rust interop
- Strict Brief: full pre/postcondition verification with `--strict` flag
- LSP server: hover, definition, completions, document/workspace symbols
- `brief bind` + `brief metrod connect`: one-command FFI binding generation
- LLVM IR backend with acyclic optimization
- Backend syncs: all 11 backends have full statement/expression coverage
- Phase 0 shared analysis: CallGraph + ParameterRanges across all backends

**See:** [docs/milestones/SELF_HOSTING_COMPLETE.md](docs/milestones/SELF_HOSTING_COMPLETE.md) for the full story.

## Project Structure

```
brief-compiler/
├── src/                          # Rust bootstrap compiler
│   ├── main.rs                   # CLI: check, build, compile, bind, metrod, lsp
│   ├── lib.rs                    # Crate root
│   ├── ast.rs                    # AST definitions
│   ├── parser.rs                 # Rust parser
│   ├── import_resolver.rs        # Module import resolution
│   ├── desugarer.rs              # AST desugaring
│   ├── typechecker.rs            # Type checker
│   ├── proof_engine.rs           # Proof engine + CallGraph integration
│   ├── lsp.rs                    # LSP server (hover, definition, completions, symbols)
│   ├── reactor.rs                # Reactive runtime
│   ├── signal_graph.rs           # Signal dependency tracking
│   │
│   ├── analysis/                 # Shared program analysis
│   │   ├── call_graph.rs         # Transaction call graph + cycle detection
│   │   ├── range.rs              # Parameter bounds inference
│   │   ├── dataflow.rs           # Read/write dependency analysis
│   │   ├── protocol.rs           # Control register prerequisites
│   │   ├── address_space.rs      # Memory address classification
│   │   ├── cross_reference.rs    # Address validation
│   │   ├── entry_point.rs        # Triggerable transaction discovery
│   │   └── struct_generator.rs   # State struct generation
│   │
│   ├── backend/                  # Code generation backends
│   │   ├── aarch64.rs            # ARM64 assembly (577 lines)
│   │   ├── x86_64.rs             # AMD64 assembly (598 lines)
│   │   ├── rust.rs               # Rust source (789 lines)
│   │   ├── c.rs                  # C source (872 lines)
│   │   ├── llvm.rs               # LLVM IR (500 lines) — NEW
│   │   ├── wasm.rs               # WebAssembly text format (600 lines)
│   │   ├── webstack.rs           # Rust + wasm-bindgen + JS (2087 lines)
│   │   ├── cobol.rs              # COBOL source (710 lines)
│   │   ├── verilog.rs            # SystemVerilog (1805 lines)
│   │   ├── vhdl.rs               # VHDL (1042 lines)
│   │   ├── tcl_generator.rs      # Xilinx Vivado Tcl (369 lines)
│   │   └── mod.rs                # Backend registry + analysis helper
│   │
│   ├── ffi/                      # Foreign Function Interface
│   │   ├── metropolitan.rs       # Shared memory IPC (876 lines)
│   │   ├── orchestrator.rs       # Native + Metropolitan dispatch
│   │   ├── registry.rs           # 60+ Rust impl functions (892 lines)
│   │   ├── sentinel.rs           # Pre/post-condition validation
│   │   ├── native_mapper.rs      # Byte serialization
│   │   ├── loader.rs             # DBVS binding file loader
│   │   ├── resolver.rs           # Binding path resolution
│   │   ├── metro_cli.rs          # `brief metrod connect` CLI (661 lines)
│   │   ├── types.rs              # FfiValue, MemoryLayout, FfiType
│   │   ├── error.rs              # Error conventions
│   │   ├── protocol.rs           # Mapper trait
│   │   ├── mapper.rs             # Mapper registry
│   │   ├── mappers.rs            # Built-in mappers
│   │   ├── script.rs             # Script function resolution
│   │   ├── validator.rs          # Binding validation
│   │   └── mod.rs                # FFI crate root
│   │
│   ├── dbrief/                   # Data Brief (DBVS) subsystem
│   │   ├── ast.rs                # DBVS AST + Fn/Trigger/Result types
│   │   ├── parser.rs             # DBVS parser
│   │   └── ...                   # DBVS compiler
│   │
│   ├── wrapper/                  # Library wrapper/bindings generator
│   │   ├── generator.rs          # DBVS + bridge.bv + foreign stub gen
│   │   ├── c_analyzer.rs         # C header parser
│   │   ├── rust_analyzer.rs      # Rust source analyzer
│   │   ├── python_analyzer.rs    # Python function analyzer
│   │   ├── js_analyzer.rs        # JavaScript function analyzer
│   │   ├── wasm_analyzer.rs      # WASM module analyzer
│   │   └── mod.rs                # Wrapper dispatch
│   │
│   ├── backend/                  # (see above)
│   ├── ffi/                      # (see above)
│   └── ...                       # Other modules
│
├── lib/
│   ├── std/                      # Standard Library (15+ modules)
│   │   ├── char.bv               # Character operations
│   │   ├── string.bv             # String manipulation
│   │   ├── math.bv               # Math functions
│   │   ├── collections.bv        # Collections
│   │   ├── io.bv                 # File I/O
│   │   ├── process.bv            # Process spawning
│   │   ├── iterator.bv           # Iterator adapters
│   │   ├── metro_bridge.bv       # Metropolitan FFI frgn declarations
│   │   └── ...                   # 15+ modules total
│   │
│   └── compiler/                 # Brief Self-Hosted Compiler
│       ├── token.bv              # Token definitions
│       ├── lexer.bv              # Lexer
│       ├── parser.bv             # Parser (strict mode aware)
│       ├── ast.bv                # AST
│       ├── typechecker.bv        # Type checker (strict [true] rejection)
│       ├── proof_engine.bv       # Proof engine (strict escalation)
│       ├── call_graph.bv         # CallGraph analysis (mirrors Rust)
│       ├── range.bv              # Range analysis (mirrors Rust)
│       ├── main.bv               # CLI entry point (--strict flag)
│       └── backends/             # Brief code backends
│           ├── aarch64.bv        # ARM64 binary
│           ├── x86_64.bv         # AMD64 binary
│           ├── rust.bv           # Rust source
│           └── c.bv              # C source
│
├── std/bindings/                 # DBVS binding definitions
│   ├── metropolitan.dbvs         # 23 Metropolitan primitives
│   ├── io.dbvs                   # I/O bindings
│   ├── math.dbvs                 # Math bindings
│   ├── string.dbvs               # String bindings
│   ├── time.dbvs                 # Time bindings
│   ├── system_triggers.dbvs      # System trigger bindings
│   ├── collections.dbvs          # Collections bindings (NEW)
│   ├── encoding.dbvs             # Encoding bindings (NEW)
│   ├── json.dbvs                 # JSON bindings (NEW)
│   └── http.dbvs                 # HTTP bindings (NEW)
│
├── plans/active/                 # Active plans
│   └── ROADMAP.md                # Comprehensive roadmap (supersedes all others)
│
├── learn-brief/                  # Tutorial
│   ├── 00-welcome.md
│   ├── 01-basics.md
│   ├── 02-contracts.md
│   ├── 03-reactive.md
│   └── README.md
│
├── examples/                     # Example programs
├── spec/                         # Language specification
├── tests/                        # Test files (269 tests)
└── docs/                         # Documentation
    ├── milestones/               # Milestone reports
    └── reports/                  # Status reports
```

## Roadmap

### ✅ Complete (v0.14.0)
- [x] 11 production backends (AArch64, x86-64, Rust, C, LLVM, WASM, Webstack, COBOL, Verilog, VHDL, TCL)
- [x] Full statement/expression coverage across all backends (13/13 statement variants, 22-36 expression variants)
- [x] 300+ standard library functions
- [x] Self-hosting capable (both Rust bootstrap and Brief self-hosted)
- [x] Metropolitan FFI: shared memory IPC with C/Python/JS/Rust clients
- [x] DBVS binding schema with `Fn()`, `Trigger()`, `Result[]` type support
- [x] `brief bind` — one-command FFI binding generation
- [x] `brief metrod connect` — interactive shared memory CLI
- [x] Strict Brief (.sbv/.sebv/.srbv) with full contract verification
- [x] LSP server: hover, definition, completions, symbols, strict mode detection
- [x] Shared analysis: CallGraph + ParameterRanges across all backends
- [x] Acyclic optimization — static dispatch in backends when graph is cycle-free
- [x] Sentinel pre/post-condition validation
- [x] 269 passing tests

### 📋 Planned
- [ ] AArch64 FFI support + linkage config
- [ ] Integration tests for all 11 backends
- [ ] Bootstrap process (compile compiler with itself)
- [ ] LSP ghost text (inlay hints with call graph, trigger dependencies, ranges)
- [ ] LLVM backend inkwell bindings (programmatic IR generation)
- [ ] Performance profiler
- [ ] Debugger integration

## Contributing

1. Read [CONTRIBUTING.md](CONTRIBUTING.md) (planned)
2. Check [OPTIMIZATIONS.md](OPTIMIZATIONS.md) for CS guidelines
3. See [lib/compiler/README.md](lib/compiler/README.md) for architecture
4. Run tests: `cargo test --lib`

## License

Apache 2.0 with explicit runtime exception

---

*Last updated: 2026-05-28*  
*Version: Brief v0.14.0*
