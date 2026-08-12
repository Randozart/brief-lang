# Briev Compiler - Development Guidelines

## Project Overview

**Briev** is a pure declarative specification language for reactive state machines. It defines valid states, transitions, and contracts.

**Rendered Briev** is Briev with embedded view/UI bindings for frontend integration. It adds:
- HTML/JSX-like view templates
- Signal bindings (b-text, b-show, b-trigger)
- Compiles to WebAssembly + JavaScript glue

**File Types**:
- `.br` - Pure Briev (specification only)
- `.rbv` - Rendered Briev (Briev + View) - like how `.tsx` relates to `.ts`

The project consists of:
- A Rust compiler (`src/`)
- Example applications (`examples/`)
- Specification documents (`spec/`)

## Build/Test Commands

### Standard Development
- **Build**: `cargo build`
- **Build Release**: `cargo build --release`
- **Run Tests**: `cargo test`
- **Run Library Tests Only**: `cargo test --lib`
- **Typecheck**: `cargo check`

### Running the Compiler
- **Unified Compile**: `./target/release/briev-compiler compile <file> --target <spec.toml>`
- **Build (Default)**: `./target/release/briev-compiler build <file>`
- **WASM Generation**: `./target/release/briev-compiler wasm <file>`
- **Compile RBV File**: `./target/release/briev-compiler rbv <file.rbv>`
- **Run with Server**: `./target/release/briev-compiler run <file.rbv>`
- **Install to PATH**: `cp target/release/briev-compiler ~/.local/bin/briev`

### New CLI Commands & Build System (as of May 2026)

The build and compilation system has been refactored to be more explicit and powerful.

#### `briev build`

The `build` command is now the primary way to compile a Briev file to its default target. The behavior depends on the file extension:

-   **`.bv` (Briev Volume)**: Transpiles to Rust and attempts to compile to a native executable using `rustc`.
    -   *Default Target*: Native Rust application.
-   **`.rbv` (Rendered Briev Volume)**: Compiles to a full web application (WASM + JS + Frontend), similar to the old `run` command.
    -   *Default Target*: Web application.
-   **`.ebv` (Embedded Briev Volume)**: **No default target**. These files are hardware-specific and must be compiled with an explicit target using `briev compile`.

#### `briev wasm`

This new command is dedicated to WASM generation:

-   **`.bv` file**: Generates a pure, standalone WASM binary. No JS glue or frontend is created.
-   **`.rbv` file**: Generates a full web application (WASM + JS + Frontend), identical to `briev build` for `.rbv` files.

#### `briev compile` (Unified)

This is the most flexible command, allowing you to compile any Briev file to any supported target by specifying a target specification file.

`briev compile <file> --target <spec.toml>`

**New Target Specs**:
-   `vhdl_fpga.toml`: Compiles to VHDL for FPGAs.
-   `react_native.toml`: Generates a React Native component.
-   `nextjs.toml`: Generates a Next.js page.
-   `vite.toml`: Generates a React component for a Vite project.

See `lib/targets/` for all available target specifications.

### Data Briev for Configuration

The old `hardware.toml` files are being replaced by **Data Briev** (`.dbv`, `.dbvs`). This allows for schema-enforced hardware configuration and validation. See `DATABRIEV.md` for more details.


### Example Files
- **Shopping Cart**: `examples/shopping_cart.rbv`
- **Counter**: `examples/counter.rbv`
- **Todo**: `examples/todo.rbv`

## Architecture

### Key Source Files
- `src/parser.rs` - Briev language parser (handles both .br and .rbv)
- `src/lexer.rs` - Tokenization
- `src/ast.rs` - Abstract syntax tree definitions
- `src/typechecker.rs` - Type checking, contract verification, FFI error enforcement
- `src/desugarer.rs` - Desugaring (implicit term true, etc.)
- `src/symbolic.rs` - Symbolic execution for contract verification
- `src/proof_engine.rs` - Proof generation, mutual exclusion checking, contract proofs
- `src/wasm_gen.rs` - WASM code generation
- `src/rbv.rs` - .rbv file parsing (Rendered Briev view extraction)
- `src/view_compiler.rs` - View/HTML compilation with bindings
- `src/reactor.rs` - Reactor runtime

### Generated Output
- WASM artifacts go to `/tmp/briev-run-<name>/`
- Includes: `.rbv` → Rust → WASM → Browser

## Code Style

- **Runtime**: Rust with wasm-bindgen
- **Imports**: Use crate modules (e.g., `crate::parser::Parser`)
- **Error Handling**: Return `Result<T, String>` for parsing, `Box<dyn Error>` for IO
- **Naming**: snake_case for functions, PascalCase for structs/enums

---

# CONTRACT-FIRST PHILOSOPHY

**This is the most important principle for this project.**

## Core Principle

> **Contracts are the source of truth. Code is generated to satisfy contracts. Never weaken contracts to match lazy implementation code.**

When writing Briev code (.rbv files) or modifying the compiler:
1. Write/improve the CONTRACT first
2. Generate CODE that satisfies the contract
3. If code cannot satisfy the contract, PROVE it's impossible
4. Only modify the contract as a LAST RESORT with full justification

## The Three Coercion Strategies

### 1. Contract-First Generation
Don't write code and bolt on contracts. Write contracts FIRST, then generate code that satisfies them.

**Example - Bad (lazy)**:
```
// Write transaction first, then weak contract
txn add_to_cart [true] { ... }  // ← Lazy contract!
```

**Example - Good (correct)**:
```
// Contract defines valid state transitions
txn add_to_cart [cart.has_valid_product == true] { ... }
// THEN generate code that ensures precondition is met
```

### 2. Failure-Driven Contracts
Write contracts in response to actual bugs. A contract written to prevent a specific failure is never lazy.

**Before**: `items > 0` (generic, lazy)
**After (based on bug report "cart shows negative items")**: `items >= 0 && items <= max_cart_size` (specific, rigorous)

### 3. Adversarial Review
Before accepting any contract, ask: "What inputs could pass this contract but cause wrong behavior?"

**Questions to ask**:
- What happens if `product == 0` in `[product > 0]`?
- What if signal is uninitialized?
- Can the pre/post condition be satisfied trivially (e.g., `[true]`)?

## Escalation Hierarchy

When code cannot satisfy a contract:

1. **First**: Modify the CODE to satisfy the contract
2. **Second**: If impossible, PROVE the contract is unsatisfiable (show specific input that makes fulfillment impossible)
3. **Third**: ONLY THEN modify the contract - and the modification MUST include:
   - The original contract
   - The proof of unsatisfiability
   - The new contract
   - Justification for why the original was wrong

**NEVER silently weaken a contract** (e.g., changing `[product > 0]` to `[true]` just because code doesn't set product).

## Briev-Specific Rules

### For .rbv Files (Shopping Cart, Counter, etc.)

When modifying example files:

1. **Preserve contracts exactly** - If a transaction has `[product > 0]`, that's correct (can't add "nothing" to cart)
2. **Fix the button/trigger bindings** - If contract requires product > 0, ensure buttons call transactions that set product first, OR call product-specific transactions directly
3. **Don't weaken to test** - Never change `[product > 0]` to `[true]` for convenience

### Transaction Design Patterns

**Pattern 1: Direct Action (Preferred)**
```
// Each button calls specific transaction
<button b-trigger:click="ShoppingCart.add_laptop">Add Laptop</button>
<button b-trigger:click="ShoppingCart.add_keyboard">Add Keyboard</button>

txn ShoppingCart.add_laptop [true][...] { &product = 1; &items = items + 1; ... }
txn ShoppingCart.add_keyboard [true][...] { &product = 2; &items = items + 1; ... }
```

**Pattern 2: Selection Then Action**
```
// Two-step: select first, then add
<button b-trigger:click="select_laptop">Select Laptop</button>
<button b-trigger:click="add">Add to Cart</button>

txn select_laptop [true] { &product = 1; term; }
txn add [product > 0][...] { ... }
```

**NEVER**:
```
// Lazy - tries to use single add for all products
<button b-trigger:click="add">Add</button>  // Calls add with precondition [product > 0]
txn add [product > 0] { ... }  // Requires product to be set, but button doesn't set it!
```

## Anti-Patterns to Avoid

### 1. Contract Weakening
```rust
// WRONG - Lazy fix
txn add [true] { ... }  // Changed from [product > 0]

// CORRECT - Keep contract, fix code
<button b-trigger:click="add_laptop">Add</button>
txn add_laptop [true] { &product = 1; ... }
```

### 2. Trivial Assertions
```rust
// WRONG - Always passes
[true]

// CORRECT - Specific invariant
[items >= 0 && items <= 100]
```

### 3. Missing Postconditions
```rust
// WRONG - No guarantee of outcome
txn add [product > 0] { &items = items + 1; }

// CORRECT - Defines outcome
txn add [product > 0][items == @items + 1] { &items = items + 1; }
```

---

## Recent Changes

### Parser Bugs Fixed (2025-04)
1. Nested block elements - depth tracking for HTML nesting
2. Unicode/Emoji - UTF-8 safe character iteration  
3. WASM method export - `#[wasm_bindgen]` on transaction methods
4. Cache invalidation - rebuilds WASM when source changes
5. Show bindings - poll_dispatch evaluates visibility expressions
6. Duplicate JS function - fixed code generator outputting applyInstructions twice

### Shopping Cart Status
The shopping cart now works but demonstrates lazy contract patterns. Fix it by:
1. Keeping `[product > 0]` contract (CORRECT)
2. Adding product-specific transactions: `add_laptop`, `add_keyboard`, etc.
3. Binding buttons directly to product-specific transactions

---

# RESEARCH, PLAN, EXECUTE

Three-phase problem solving framework used for all significant tasks.

## Phase 1: Research

Investigate and understand the problem before acting:
- Gather all relevant information
- Read existing code, tests, and documentation
- Understand the current state and context
- Ask questions if anything is unclear

**Never start coding until you understand what you're building.**

## Phase 2: Plan

Develop a clear roadmap before implementation:
- Break down the task into specific, actionable steps
- Identify dependencies and potential issues
- Define success criteria (how will you know it's done?)
- Create a todo list to track progress

**Never execute without a clear plan.**

## Phase 3: Execute

Implement the solution:
- Follow the plan
- Run tests frequently to verify progress
- Update the plan if new information emerges
- Document changes

## Application to This Project

1. **Research** - Read CLAUDE.md, IMPLEMENTATION-*.md, and relevant source files
2. **Plan** - Create a todo list, identify what needs to change
3. **Execute** - Make changes, run tests (`cargo test`), update documentation

---

## Changes Made

### 2026-05-01 - Phase 2: TargetSpec refactor
- Files: `src/backend/c.rs`, `src/main.rs`, `src/target_spec/`
- Change: Replaced CBackend boolean flags with TOML-based TargetSpec
- Now uses `--target <spec.toml>` for kernel modules, bare-metal, etc.

## Contact

This file is used by AI coding assistants (Claude Code, OpenCode) when working in the Briev compiler project. All changes should maintain the Contract-First Philosophy.

---

## v0.14 Implementation Log

### 2026-05-16 — Engineering Log

#### What was built
- **`Hashtag` struct** + `parse_hashtag_modifiers()` parser (handles `#tag`, `#!tag`, `#!A|B|C`, `#[target]#tag`)
- **`AlkaBlock`** AST struct + `parse_alka_block()` parser (handles `alka { ... };` and `alka! { ... };`)
- **Dynamic `@` address** — parser stores non-literal `@ expr` in `address_expr: Option<Box<Expr>>`
- **AST fields** — `modifiers` on Let/Assignment/Term/Transaction/Definition, `variant_bodies` on Transaction/Definition, `variants` on StructDefinition, `address_expr` on Let
- **10 new tests** covering alka, hashtags (scoped, mandatory, fallback chain, with value), dynamic `@`, and term/assignment modifiers

#### Bugs fixed during implementation
1. **Semicolon before hashtags on assignments** — `self.expect(Semicolon)` was called BEFORE `parse_hashtag_modifiers()` in the assignment parsing path, meaning `&x = 1 #!sfence;` would fail. Fixed by moving semicolon check into each match arm.
2. **Term hashtags not parsed** — `parse_term_outputs()` would try to parse `#retry` as an expression. Fixed by adding early-return for `Hash`/`HashBang` tokens.
3. **`Statement::Term` construction used `.. }` pattern** in expression context (struct update syntax without base), which is a Rust compiler error. Changed all construction sites to `modifiers: vec![]`.
4. **`Statement::Assignment { .. }` missing explicit modifiers** in construction sites (~15 locations across backends, fuzzer, proof engine, annotator).

#### Not implemented (deferred from v0.14 draft)
| Feature | Reason |
|---------|--------|
| Multi-body dispatch `[pre]{body}[pre2]{body2}` | Syntax conflicts with current `[pre][post]{body}` contract model — needs spec clarification |
| `#on_exit { ... };` block pragma | Requires new `parse_hashtag_block_pragma()` method + proof engine integration for LIFO cleanup |
| `+/-` struct variants | `StructVariant` struct exists but no parser support for differential member syntax |
| Backend registry for hashtag validation | Backends need to declare supported tags and reject unsupported `#!` tags |
| Backend codegen for dynamic `@` | C/Rust backends still emit `&var = ADDR` literal, not pointer-deref from `address_expr` |
| Backend codegen for `alka {}` | Currently parsed but all backends treat `Alka` as no-op via catch-all match arms |

#### Key design decisions
- **`alka` is NOT a Token** — It's matched as `Identifier("alka")` in the parser, avoiding a lexer change for what's essentially a contextual keyword
- **Brace-matching for `alka`** uses the same span-based approach as `parse_render_block()`: record `{` position, track depth through token stream, extract `source[lbrace+1..end_rbrace]`
- **Hashtags live AFTER expressions**, before `;` — this means the expression parser never sees `#` tokens, avoiding ambiguity
- **`#!A|B|C` fallback** — `|` is a pipe token (`Token::Pipe`), stored as `fallback: Vec<String>` in `Hashtag`

#### Caught myself being lazy
1. **First pass of `Statement::Term` construction** — used `.. }` everywhere thinking it was like a pattern. It IS a pattern in `if let`/`match`, but a struct-update-expression in constructors. Rust caught this, rightfully.
2. **Added AlkaBlock but didn't wire match arms** — thought most would already have `_ =>` catch-alls. Only 5 were exhaustive. `cargo check` caught them all.
3. **Tests for scoped hashtags** — wrote `#[cpp]volatile` instead of `#[cpp]#volatile` initially. The spec says `"#[" identifier "]" hashtag`, meaning the `#` prefix is required after the bracket. Fixed test.
4. **Almost skipped semicolon reordering** — considered leaving assignment hashtags as a "parser limitation". But the `#` tokens are consumed by expression parser in the wrong order. Had to fix it properly.

#### What I'd do differently
- Multi-body dispatch should have been designed WITH the contract parsing, not bolted on. The current `[pre][post]{body}` is incompatible with per-body `[pre]{body}[post]{body2}`. A proper design would be: `[post]` is always on the LAST body, or adopt the Rust-like `match` syntax.
- I should have written tests FIRST before implementing the fix for semicolon ordering. Would have caught the design flaw earlier.
- The amount of `modifiers: vec![]` boilerplate across 15+ construction sites suggests this should have been a default. Consider using `#[derive(Default)]` patterns or builder methods for Statement.

### 2026-05-16 Session 2 — Multi-body dispatch + recovery from misplaced code

#### Built
- **Multi-body dispatch** for transactions and definitions. Added `parse_variant_bodies()` method that reads `[pre]{body}` and bare `{body}` (catch-all) pairs after the main body. Wired into both `parse_transaction()` and `parse_definition()`.
- **3 new tests** for multi-body (transaction, definition, single-body backward compat).
- **204 total tests** (up from 201).

#### Deferred (still pending)
| Feature | Reason |
|---------|--------|
| `#on_exit { ... };` block pragma | Needs new parser method for hashtag block pragmas |
| `+/-` struct variants | `StructVariant` exists but no parser |
| Backend registry, alka codegen, dynamic @ emits | All no-ops via catch-all match arms |

#### Bugs and recoveries during session
1. **`[post ready]` in test input is invalid** — the spec example uses `[post ready]` as shorthand, but the contract parser reads sequential bracketed expressions. `ready` alone works (`[post == ready]` or `[ready]`). The spec `[post condition]` is aspirational sugar, not current grammar.
2. **Misplaced test code** — I accidentally placed `#[test] fn test_multi_body...` inside the `impl Parser` block after `parse_statement`. This caused 94 cascading errors. The correct location is inside the `mod parser_tests { }` block at the end of the file. Three rounds of cleanup needed (removing duplicate, fixing extra closing braces, restoring `None` return in `parse_map_pair`).
3. **`return` not a keyword in Briev** — used `return x * 2;` in a test which failed because `return` is an identifier token. Replaced with `&result = x * 2;` (standard Briev assignment).
4. **Editor edit was too greedy** — my `String.replace("oldText", "newText")` pattern matched a larger region than intended, copying test functions into the wrong location while also removing them from the right one. Should have verified the edit region before applying.

#### What I'd do differently this session
- Verify test syntax against actual tokenizer BEFORE writing tests (the `[post ready]` and `return` issues are obvious in hindsight)
- When removing code, use a smaller match window and verify with `git diff` before building
- Place `#[test]` functions ONLY inside the `#[cfg(test)] mod parser_tests` block from the start

### 2026-05-16 Session 3 — #on_exit block pragma + +/- struct variants

#### Built
- **`#on_exit { ... };` block pragma** — Added `Statement::OnExit` variant to AST. Parser reads `#identifier{body};` as a block pragma statement (new match arm in `parse_statement()`). Stores cleanup body statements.
- **`+/-` struct variants** — Added `parse_struct_variants()` and `parse_struct_variant_fields()` methods. After the main `struct { fields }`, parser checks for `[discriminant]{ +addition; -removal; field; }` variant bodies. Fields prefixed with `+` go into `StructVariant::additions`, fields with `-` go into `removals`.
- **4 new tests** — 2 for `#on_exit` (basic and bare form), 2 for struct variants (`+` add and `-` remove).
- **208 total tests** (up from 204).

#### Bugs and recoveries
1. **Misplaced test code (round 2)** — despite cleaning up in session 2, remnant `#[test]` functions were still inside `impl Parser` at ~line 449. These closed `impl Parser` prematurely at line 481, causing ALL subsequent methods to be outside the impl block. Fixed by removing the full remaining test block.
2. **`type` not a keyword** — test used `type GPU { ... }` but Briev uses `struct` keyword. Fixed test input.
3. **`spanned_err` return type** — in `parse_struct_variant_fields`, the `_ =>` error arm calls `spanned_err` which returns `Err(SyntaxError)`. The function's `Result<..., SyntaxError>` return type means `return self.spanned_err(...)` works directly (no `.unwrap_err()` needed) since the `?` operator coerces the error variant.

#### Commits
- `da90458` — v0.14 hashtag modifiers, alka hatch, dynamic @
- `b24e7dc` — #on_exit block pragma, +/- struct variants
- `e2fc2c2` — typecheck #on_exit bodies, add Alka/OnExit to fuzzer
- `984afb6` — backend hashtag registry with validation pipeline

#### Still deferred
- ~~Backend registry for hashtag support checking~~ **DONE**
- Backend codegen for dynamic `@`, alka, #on_exit
- Multi-body struct type dispatch (only `+/-` field syntax is parsed, no type-check semantics)

### 2026-05-16 Session 4 — Backend hashtag registry

#### Built
- **`supported_hashtags(backend)`** — returns supported tags per backend (C, Rust, WASM, Verilog, VHDL, Cobol, x86_64, aarch64)
- **`validate_hashtags(tags, backend)`** — checks each tag against backend support, handles mandatory/fallback/scoped
- **`validate_hashtags_in_program(program, backend, strict)`** — walks entire AST, collects hashtags from all statements/transactions/definitions/structs, validates them
- **Wired into C and Rust pipelines** — called after typechecking, before codegen
- **7 unit tests** — supported tag, unknown advisory warning, unknown mandatory error, fallback chain success, fallback chain failure, scoped skip, scoped validate

#### Key findings
- `supported_hashtags()` is a simple string list, but the tags themselves are not standardized. `volatile` is used by C/Rust/Cobol but not by Verilog (`clock`, `register`). If a Briev file has `#!volatile` targeting Verilog, the error message tells the user exactly that.
- Fallback chain `#!A|B|C` only works if at least one alternative is supported. The validation correctly skips the primary `name` when checking fallbacks.
- Scoped tags `#[verilog]clock` are only validated when `scope == backend`. This means a C target never sees the Verilog-specific `clock` tag.
- `StateDecl` (top-level let) doesn't have a `modifiers` field — hashtags on top-level lets are not yet supported. This is fine because top-level state declarations don't need backend-specific modifiers in practice.
- The validation is only wired into `run_c` and `run_rust` in main.rs. Other backends (Cobol, Verilog, VHDL, WASM) would need the same call added in their `run_*` functions.
