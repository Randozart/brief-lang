# Brief Compiler - Development Guidelines

## Project Overview

**Brief** is a pure declarative specification language for reactive state machines. It defines valid states, transitions, and contracts.

**Rendered Brief** is Brief with embedded view/UI bindings for frontend integration. It adds:
- HTML/JSX-like view templates
- Signal bindings (b-text, b-show, b-trigger)
- Compiles to WebAssembly + JavaScript glue

**File Types**:
- `.br` - Pure Brief (specification only)
- `.rbv` - Rendered Brief (Brief + View) - like how `.tsx` relates to `.ts`

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
- **Compile RBV File**: `./target/release/brief-compiler rbv <file.rbv>`
- **Run with Server**: `./target/release/brief-compiler run <file.rbv>`
- **Install to PATH**: `cp target/release/brief-compiler ~/.local/bin/brief`

### Example Files
- **Shopping Cart**: `examples/shopping_cart.rbv`
- **Counter**: `examples/counter.rbv`
- **Todo**: `examples/todo.rbv`

## Architecture

### Key Source Files
- `src/parser.rs` - Brief language parser (handles both .br and .rbv)
- `src/lexer.rs` - Tokenization
- `src/ast.rs` - Abstract syntax tree definitions
- `src/typechecker.rs` - Type checking, contract verification, FFI error enforcement
- `src/desugarer.rs` - Desugaring (implicit term true, etc.)
- `src/symbolic.rs` - Symbolic execution for contract verification
- `src/proof_engine.rs` - Proof generation, mutual exclusion checking, contract proofs
- `src/wasm_gen.rs` - WASM code generation
- `src/rbv.rs` - .rbv file parsing (Rendered Brief view extraction)
- `src/view_compiler.rs` - View/HTML compilation with bindings
- `src/reactor.rs` - Reactor runtime

### Generated Output
- WASM artifacts go to `/tmp/brief-run-<name>/`
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

When writing Brief code (.rbv files) or modifying the compiler:
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

## Brief-Specific Rules

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

## Contact

This file is used by AI coding assistants (Claude Code, OpenCode) when working in the Brief compiler project. All changes should maintain the Contract-First Philosophy.
