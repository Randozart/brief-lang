# Brief Language Specification
**Version:** 5.0 Contract Verification & Ownership
**Date:** 2026-04-05
**Status:** Authoritative Reference

---

## 1. Introduction and Philosophy

**Brief** (Compiled Brief / Communication Brief) is a declarative, contract-enforced logic language designed natively for LLM-assisted development. It treats program execution as a series of verified state transitions (Settlements) rather than sequential instructions.

Brief is designed for **Formal Verification without the Boilerplate**. It eliminates imperative control flow (`if`, `else`, `while`) in favor of Goal-Driven Execution, Unification, and Contractual Convergence.

### 1.1 Core Design Principles

1. **The Brief**: Every transaction is a legally binding agreement. If the postcondition isn't met, the transaction never happened (Atomic STM Rollback).
2. **Goal-Driven Execution (Blackboard)**: The runtime is a Reactor. It does not "run" code top-to-bottom; it continuously evaluates the global state and satisfies Briefs whose preconditions are met.
3. **Flat, Zero-Nesting Logic**: To maximize LLM token efficiency and context-window comprehension, nested scopes are abolished. Branching is handled via Unification Guards and state constraints.
4. **Promise Exhaustiveness**: The compiler forces developers (and AIs) to handle every possible outcome of an external capability before the code is allowed to run.
5. **Infallible Signatures**: The host system can mathematically guarantee capabilities to the AI, reducing boilerplate error handling.

### 1.2 File Extension
File extension is `.bv`.

---

## 2. Grammar Specification (Complete BNF)

*Note: Imperative constructs (`if`, `else`, `while`, `switch`) do not exist in Brief.*

### 2.1 Top-Level Program Structure

```bnf
program ::= (signature | definition | foreign_sig | state_decl | constant | transaction | rct_transaction)*

signature ::= "sig" identifier ":" type_spec "->" result_type ("from" namespace_path)? ("as" identifier)? ";"
foreign_sig ::= "frgn" "sig" identifier "(" parameters? ")" "->" output_types ";"
definition ::= "defn" identifier parameter_list? contract "->" output_types "{" body "}" ";"

output_types ::= type ("," type)*          # Multi-output: (A, B, C)
result_type ::= output_list | "true"        # Projection or assertion
output_list ::= type ("," type)*            # Take specific outputs

state_decl ::= "let" identifier ":" type ("=" expression)? ";"
constant ::= "const" identifier ":" type "=" expression ";"

transaction ::= txn_decl
txn_decl ::= ("async")? "txn" identifier contract "{" body "}" ";"

rct_transaction ::= "rct" ("async")? "txn" identifier contract "{" body "}" ";"

body ::= statement*
```

### 2.2 Structs (Stateful Objects)

```bnf
struct_def ::= "struct" identifier "{" struct_member* "}"
struct_member ::= field_decl | transaction
field_decl ::= identifier ":" type ";"
```

**Example:**
```brief
struct Player {
    name: String;
    score: Int;
    position: Int;
    
    txn update_score [score >= 0][score == @score + points] {
        &score = score + points;
        term;
    };
};
```

Structs can contain:
- **Fields**: Named data members with types
- **Transactions**: Methods that operate on the struct's state

**Field Access:**
```brief
let player: Player;
player.name       # Access field
player.score      # Access another field
```

### 2.3 Types and Contracts

```bnf
type_spec ::= simple_type | union_type | contract_bound_type
simple_type ::= "Int" | "Float" | "String" | "Bool" | "Data" | "void" | identifier
union_type ::= type_spec "|" type_spec
contract_bound_type ::= simple_type "[" contract_guard "]"

result_type ::= type_spec ("|" type_spec)* | "true"

contract ::= "[" pre_condition "]" "[" post_condition "]"
pre_condition ::= expression | "~/" identifier
post_condition ::= expression

contract_guard ::= expression
```

### 2.3 Statements (Zero-Nesting)

```bnf
statement ::= 
    | assignment
    | unification
    | guarded_stmt
    | term_stmt
    | escape_stmt
    | expression ";"

assignment ::= ("&")? identifier "=" expression ";"

# Unification handles exhaustive branching without nesting
unification ::= identifier "(" identifier ")" "=" expression ";"

# Guard clauses replace 'if' statements
guarded_stmt ::= "[" expression "]" statement

# Multi-output term: term a,,b,c; (trailing commas for void slots)
term_stmt ::= "term" expression? ("," expression?)* ";"
escape_stmt ::= "escape" expression? ";"
```

### 2.4 Expressions

```bnf
expression ::= or_expression
or_expression ::= and_expression ("||" and_expression)*
and_expression ::= comparison ("&&" comparison)*
comparison ::= expr_term (("==" | "!=" | "<" | "<=" | ">" | ">=") expr_term)?
expr_term ::= factor (("+" | "-") factor)*
factor ::= unary (("*" | "/") unary)*
unary ::= ("!" | "-" | "~") unary | primary
primary ::= 
    | literal
    | identifier
    | "&" identifier                    # ownership reference
    | "@" identifier                    # prior-state reference
    | call
    | paren_expression

call ::= identifier "(" arguments? ")"
arguments ::= expression ("," expression)*
```

### 2.5 Imports and Namespaces

```bnf
import_stmt ::= "import" ("{" import_item ("," import_item)* "}")? (("from" namespace_path) | namespace_path)? ";"
import_item ::= identifier ("as" identifier)?
namespace_path ::= identifier ("." identifier)*
```

**Examples:**
```brief
import std.io;                           # Import everything from std.io (shorthand)
import { print } from std.io;            # Import specific symbol
import { print as p } from std.io;       # Import with alias
import { map, filter as f } from collections;  # Multiple with aliases
```

**Note:** The `from` keyword is optional when importing an entire namespace (e.g., `import std.io;`). It is required when importing specific items (e.g., `import { print } from std.io;`).

---

## 3. Reactive Runtime Model

### 3.1 Blackboard Execution Model

Brief programs have no `main()` function. They execute using a **Blackboard Architecture**:

1. **The Blackboard**: `let` and `const` variables act as the global truth state.
2. **The Reactor Loop**: The engine continuously evaluates the `[pre]` conditions of all `rct` blocks (`rct txn` and `rct async txn`).
   - The reactor is event-driven, not a polling loop:
     - The blackboard tracks which variables each rct precondition references (its dependency set)
     - When an &variable mutation occurs (via assignment, term, or return binding), the reactor marks only the preconditions that reference that variable as dirty
     - Only dirty preconditions are re-evaluated — not all of them
     - If a mutation doesn't touch a variable in your precondition, your rct is never re-checked
     - At equilibrium (no dirty preconditions), the reactor sleeps — zero CPU cost
   - The blackboard IS the dependency graph. No polling loop needed because the runtime knows exactly which transactions care about which variables.
3. **Execution**: Any `rct` block whose precondition evaluates to `true` is fired. `rct` blocks self-fire when preconditions hold.
4. **Passive Transactions**: `txn` blocks (without `rct`) are passive units of work. They can be called from inside `rct` bodies or other `txn` blocks, but they do NOT self-fire and are NOT evaluated by the reactor loop.
5. **Equilibrium**: The program naturally terminates when no `rct` precondition evaluates to `true`.

### 3.2 STM Rollback Semantics (Software Transactional Memory)

A transaction in Brief is atomic. If a transaction reaches an `escape` statement, fails an inline guard, or fails to satisfy its `[post]` condition upon `term`, it acts as a **No-Op**.

Any mutations to `&variables` made during the failed transaction are instantly rolled back to their original state, keeping the Blackboard pristine.

---

## 4. Contract Semantics and Control Flow

### 4.1 Preconditions `[pre]` and Postconditions `[post]`
- **Precondition**: Determines *when* a Pact is allowed to fire. Must evaluate to true. Cannot contain mutations.
- **Postcondition**: Determines *if* a Pact was successful. Evaluated when `term` is called.

### 4.2 The `@` Prior-State Operator
Postconditions often need to verify relative changes. The `@` symbol references the value of a variable at the exact moment the transaction began.
```brief
txn increment [count < 10][count == @count + 1] {
  &count = count + 1;
  term;
};
```

### 4.3 Syntactic Sugar `~/`
`[~/ready]` is compiled directly into `[~ready][ready]`. It instructs the runtime: *"This transaction fires when `ready` is false, and must result in `ready` being true."*

### 4.4 Transaction Loop Model

**All transactions loop until successful termination.** This is a fundamental property of Brief:

1. Transaction executes top-to-bottom through guards `[condition] { ... }`
2. On reaching `term`, the runtime evaluates the postcondition
3. If postcondition is satisfied → transaction completes, state mutates
4. If postcondition fails → rollback, loop to top and try again
5. For `rct` transactions, reactor triggers automatically when pre-condition becomes true

The transaction will continue looping until the postcondition is satisfied. This is why **termination reachability** must be proven at compile time — if there's no path from pre-condition to a satisfying `term`, the transaction would loop forever.

### 4.5 Flat Logic Guards `[condition]`
Instead of nested `if` blocks, Brief uses inline logic gates. If a guard evaluates to `false`, the rest of the line is skipped.
```brief
let result = attempt();
[result == true] &successes = @successes + 1;
[result == false] &failures = @failures + 1;
```

---

## 5. Definitions and Signatures

### 5.1 Signatures (`sig`)
Signatures define the boundary between Brief logic and the external host system (I/O, Network, OS).

*   **Fallible Signatures**: `sig fetch: Int -> User | Error;` (The AI must handle both outcomes).
*   **Infallible Signatures**: `sig print: String -> true;` (The host guarantees execution. Error handling is abstracted away).
*   **Contract-Bound Signatures**: `sig get_id: String -> Int[~/0];` (Guarantees the return value mathematically satisfies the constraint, e.g., never returning zero).

### 5.2 Exhaustive Unification
When a `sig` returns a union type (`User | Error`), the compiler forces the developer to handle all possible branches using flat unification.

```brief
txn load_user [~/has_user] {
  let result = fetch(1);
  
  # Path 1: Success. Updates state and settles.
  User(u) = result; &active_user = u; &has_user = true; term;
  
  # Path 2: Failure. Logs error and escapes (STM rollback).
  Error(e) = result; log(e.msg); escape;
};
```
*If the `Error` line is omitted, the compiler rejects the program: `Compile Error: Unhandled outcome 'Error' for signature 'fetch'.`*

### 5.3 Definitions (`defn`) vs Signatures (`sig`)
- `defn` is a **White Box**: Local logic that the compiler rigorously proves.
- `sig` is a **Black Box**: External logic that the compiler trusts, but forces the caller to handle exhaustively.
- **Delegation**: A program can import a `defn` from a library and cast it as a `sig` to filter outcomes.
- **Defn is non-reactive**: `defn` runs linearly start-to-end when explicitly called. It never fires from the reactor loop. Only txn blocks are reactive.
- **Sig as local cast / contract projection**: `sig` can be cast over a local defn to narrow its output contract. The compiler verifies the narrow path is reachable and strips unreachable branches.

### 5.4 Definitions as Functions

**Definitions (`defn`) are the primary function mechanism in Brief.** They are named, callable functions with contracts:

```brief
defn add(a: Int, b: Int) [true][result == a + b] -> Int {
  let result: Int = a + b;
  term result;
};
```

**Signatures (`sig`) are constrained function declarations** that can:
- Declare the interface to external (frgn) functions
- Project specific outputs from a defn
- Assert properties about outputs (e.g., `-> true`)

**Calling defn from signatures:**
- A `sig` can wrap a `defn` to narrow its contract
- Signatures can be derived from defn's output types
- The compiler verifies the projection is valid

### 5.5 Multi-Output Functions

A `defn` can declare multiple output types, and each `term` provides values for all of them.

**Syntax:**
```brief
defn print(msg: String) -> String, Void, Bool, Bool {
    [msg.len() == 0] term "",,false, true;
    [msg.len() > 0] term msg,, true, true;
};
```

**Rules:**
- `-> A, B, C` declares three output positions (0, 1, 2)
- `term a,,b, c;` provides values for all outputs
- Empty slots use trailing commas: `term a,,,d` means output[0]=a, output[1]=void, output[2]=void, output[3]=d
- Each exit path must provide values for all output positions

### 5.6 Signature Projection

A `sig` projects specific outputs from a multi-output `defn`.

**Projection by type:**
```brief
sig print: String -> Bool as safe_print;   # Takes first Bool output
sig print: String -> Bool, Bool as both;   # Takes both Bool outputs
```

**Assertion with `-> true`:**
```brief
sig print: String -> true;
```

`-> true` is an **assertion**, not a type. It tells the compiler: "I assert that the projected Bool output is always `true`. Prove it."

### 5.7 The `-> true` Assertion

When you write `sig fn: T -> true`, the compiler must **prove** that the projected Bool output is always `true` given:
1. The `defn`'s code paths
2. The actual inputs passed at call sites in your program

**Example: Guaranteed true**
```brief
defn always_true(x: Int) -> Bool {
    term true;
};

sig always_true: Int -> true;  # ✅ Approved - defn always returns true
```

**Example: Conditional - rejected**
```brief
defn maybe_true(b: Bool) -> Bool {
    term b;
};

sig maybe_true: Bool -> true;  # ❌ Rejected - b could be false
```

### 5.8 Sugared Signature Inference

When a `term` contains a function call expression without an explicit signature, the compiler **infers** the signature:

**Sugared:**
```brief
import { print } from std.io;

rct txn hello [~/done] {
  term print("Hello, World!");
}
```

**Desugars to:**
```brief
let done: Bool = false;

sig print: String -> true;  # Auto-generated

rct txn hello [~done][done] {
  &done = true;
  term;
}
```

---

## 6. Reactive Transactions

### 6.1 Synchronous Reactive Transactions (`rct`)
Reactive transactions (`rct`) are the core reactive execution units. Unlike regular `txn` blocks which are passive and do not self-fire, `rct` blocks are part of the Blackboard reactor loop and fire synchronously when their preconditions hold.

**Syntax:**
```bnf
rct_transaction ::= "rct" ("async")? "txn" identifier contract "{" body "}" ";"
```

**Example:**
```brief
rct txn process_order [order_ready][order_processed] {
  &order_processed = true;
  term;
};
```

### 6.2 Asynchronous Reactive Transactions (`rct async`)
Asynchronous reactive transactions run concurrently but enforce compiler-verified safety. The compiler proves no conflicting variable access between concurrent `rct async` transactions.

**Syntax:**
```brief
rct async txn fetch_data [~/data_loaded][data_loaded] {
  let result = network_request();
  Data(d) = result; &data = d; &data_loaded = true; term;
};
```

### 6.3 Entry Point and Equilibrium
Brief programs have no `main()` function. Entry is whichever `rct`'s preconditions hold first. The program reaches equilibrium when no `rct` preconditions evaluate to true.

**Flow:**
1. Reactor evaluates all `[pre]` conditions for `rct` blocks only
2. First matching `rct` fires synchronously (blocks others)
3. For `rct async`, the compiler verifies mutual exclusion of write claims
4. Program continues until equilibrium (no active `rct` preconditions)

---

## 7. Borrow and Scope Rules

### 7.1 Variable Scoping
Brief uses explicit scoping rules to manage variable lifetime and ownership.

**Syntax:**
```bnf
assignment ::= ("&" | "const")? identifier "=" expression ";"
```

### 7.2 Local Scope (`let`)
`let` creates a local variable with block scope. Variables declared with `let` are safe and automatically garbage collected when out of scope.

**Example:**
```brief
let temp_value = 5;  # Local to current transaction
&global_var = temp_value;  # Write to higher-scope variable
```

### 7.3 Explicit Write Claims (`&`)
The `&` prefix on the left side of an assignment creates an explicit write claim on a higher-scope variable.

**Syntax:**
```brief
&bar = value;  # Explicit write claim on higher-scope variable 'bar'
```

**Rules:**
- `&` can only appear on the left side of an assignment
- `&bar = value` claims write access to `bar` (higher scope)
- Bare reference `bar` reads the higher-scope variable
- `const` creates an immutable binding (cannot be reassigned)

### 7.4 Function Return Assignment
When a function return is assigned to a higher-scope variable, it creates an implicit write claim.

**Example:**
```brief
defn compute(): Int [true][result > 0] {
  term 42;
};

let result: Int;
result = compute();  # Implicit write claim on 'result'
```

---

## 8. Ownership and Concurrency

### 8.1 Exclusive Access (`&`)
Variables declared via `let` are read-only by default. To mutate a variable, a transaction must claim exclusive write ownership using the `&` prefix.
```brief
&balance = balance - amount;
```

### 8.2 Lock-Free Concurrency via Pre-condition Exclusivity
Brief achieves concurrent thread safety without `Mutexes` or explicit locks.

If two `async txn` blocks mutate the same `&variable`, the compiler proves safety by ensuring their preconditions are **mutually exclusive**.

```brief
# These txn blocks are auto-threaded. They can never cause a race condition 
# because 'access' cannot be 0 and 1 simultaneously.
txn reader [access == 0] {
  log(data);
  &access = 1;
  term;
};

txn writer [access == 1] {
  &data = "updated";
  &access = 0;
  term;
};
```

### 8.3 Async Transaction Ownership Rules

For async transactions that may run concurrently, the following ownership rules apply:

**Read Access:**
- Many transactions may read the same variable simultaneously (shared read)
- Reading does not block other readers or writers

**Write Access:**
- Only one transaction may hold write access (`&variable`) at a time
- If one transaction has write access, no other transaction may read OR write that variable

**Pre-condition Exclusivity Requirement:**
Two async transactions that both claim write access to the same outer-scope variable must satisfy ONE of:
1. Their pre-conditions are mutually exclusive (can never both be true)
2. They can never be called simultaneously (enforced by caller)

```brief
# ILLEGAL: Both write to &counter, pre-conditions can overlap
async txn inc_a [true][...] { &counter = ... }
async txn inc_b [true][...] { &counter = ... }

# LEGAL: Pre-conditions are mutually exclusive
async txn inc_a [counter == 0][...] { &counter = ... }
async txn inc_b [counter == 1][...] { &counter = ... }

# LEGAL: One reads, one writes (reader doesn't block writer, but writer blocks reader)
async txn reader [true][...] { let x = counter; ... }
async txn writer [x == 0][...] { &counter = ... }
```

### 8.4 Await as Ownership Claim
`await` suspends the transaction and claims exclusive ownership of a variable for the duration. The compiler must prove no other transaction can claim that variable while the await is pending — same mutual exclusion proof as synchronous `&` claims, extended across the async window. If the compiler can't prove exclusion, it's a compile error.

---

## 9. Contract Verification

This section defines the compile-time verification that the Brief compiler must perform to ensure program correctness.

### 9.1 Pre-condition Satisfiability

The compiler must verify that a transaction's pre-condition is **satisfiable** — there exists at least some state where pre-condition evaluates to true.

If pre-condition is unsatisfiable (e.g., `[count > 0 && count < 0]`), the transaction can never fire, and the compiler should emit a warning or error.

### 9.2 Path Verification: Pre → Post

For every transaction and definition, the compiler must verify that for **all paths** from the pre-condition through guards to termination, the post-condition holds.

**Transaction Model:**
- Transaction executes top-to-bottom
- Guards `[condition] { ... }` branch based on condition
- On reaching `term`, post-condition is evaluated

**Verification Process:**
1. Enumerate all possible paths through guards
2. For each path, collect path constraints (guard conditions that must be true)
3. Verify that (pre-condition AND path constraints) implies post-condition

**Example:**
```brief
txn increment [counter >= 0][counter == @counter + 1] {
  [counter < 10] &counter = counter + 1;
  [counter >= 10] &counter = counter;  # No-op path
  term;
};
```

The compiler must verify both paths (counter < 10 and counter >= 10) lead to post-condition satisfaction.

### 9.3 Termination Reachability

The compiler must verify that there exists at least one path from the pre-condition to a terminating `term`.

This is critical because **all transactions loop** — if there's no path to termination, the transaction would loop forever.

**Verification:**
1. Build control flow graph through guards
2. Verify at least one path from pre-condition to `term`
3. For definitions with iteration constructs `[i < n]`, verify the iteration is bounded and terminates

### 9.4 Contract Implication for Definitions

Definitions (`defn`) are verified as **Hoare triples**: `{pre} body {post}`

Given the pre-condition holds when the defn is called, the post-condition must hold after the defn executes.

**Key Difference from Transactions:**
- defn runs once (no looping)
- No prior-state (`@`) in defn contracts (no state mutation)
- Pre-condition constrains valid inputs

### 9.5 True Assertion Verification

For `sig fn: T -> true`, the compiler must prove the projected Bool output is always `true` given:
1. The defn's code paths
2. The actual inputs passed at call sites in the program

This requires analyzing all call sites and verifying the Bool output is provably true on all paths.

---

## 10. Proof Engine Requirements

The Brief compiler's Proof Engine must verify the following obligations:

### 10.1 Promise Exhaustiveness
All union branches of a `sig` return type must be handled via unification.

### 10.2 Mutual Exclusion
No two concurrent transactions (async or sync with overlapping execution) may have overlapping pre-conditions if they both claim write access to the same variable.

### 10.3 Contract Implication (NEW in v5.0)
For all transaction and definition bodies, verify that pre-condition + path constraints implies post-condition. This requires symbolic execution.

### 10.4 Borrow Safety (NEW in v5.0)
For async transactions:
- No conflicting write claims to the same variable
- No read while another transaction holds write access
- Pre-conditions must be mutually exclusive for overlapping writes

### 10.5 True Assertion
For `sig fn: T -> true`, verify the projected Bool output is always true.

### 10.6 Termination Reachability (NEW in v5.0)
Verify that every reactive transaction has at least one path from pre-condition to termination.

---

## 11. Compilation and Proof Engine Pipeline

Brief utilizes a two-stage pipeline: a fast AST Linter for development, and a rigorous Proof Engine for deployment.

### 11.1 DAG-Based Dead Code Detection
During semantic analysis, the compiler maps all `[pre]` and `[post]` contracts into a Directed Acyclic Graph (DAG).
- If a transaction requires a state (e.g., `[step == 5]`), but no initial state or postcondition ever produces `step == 5`, the compiler throws an `Unreachable State` error.
- This mathematically guarantees that no "Dead Logic" can be deployed.

### 11.2 Dependency-Tracking Optimization
Instead of evaluating all preconditions every tick, the compiler already computes which variables each transaction reads. At runtime, only re-evaluate transactions when a variable they depend on changes. Same semantics, fewer wasted cycles. This makes browser deployment viable.

---

## 12. Standard Library Guidelines

### 12.1 defn vs frgn

The standard library should prefer `defn` for pure functions that can be verified by the compiler, and `frgn` for operations requiring external capabilities.

**defn (native, verified):**
- Pure computations with verifiable contracts
- Data structure operations without callbacks
- Functions whose contracts can be proven at compile time

**frgn (external, trusted):**
- I/O operations (file, network, console)
- Cryptographic operations
- Functions requiring function parameters (callbacks)
- Time-dependent operations

### 12.2 Iteration in defn vs txn

**Definitions should NOT contain internal loops.** If iteration is needed, it should be expressed as a transaction that can be verified for termination.

**Incorrect:**
```brief
# defn should not contain iteration constructs
defn find<T>(list: List<T>, item: T) -> Bool {
  let i: Int = 0;
  [i < list.len() && list[i] != item] { &i = i + 1; }
  term i < list.len();
};
```

**Correct:**
```brief
# Iteration expressed as transaction
txn find<T>(list: List<T>, item: T, i: Int) [i < list.len() && list[i] != item][found == (list[i] == item)] {
  [list[i] == item] &found = true; term;
  [list[i] != item] term find(list, item, i + 1);
};
```

---

## 13. Comprehensive Examples

### 13.1 API Fetch with Fallbacks
```brief
sig fetch_data: String -> Data | Error;
sig log: String -> true;

let data: Data;
let loaded: Bool = false;

txn initialize [~/loaded] {
  let res = fetch_data("https://api");
  
  Data(d) = res; &data = d; &loaded = true; term;
  Error(e) = res; log(e.message); escape;
};
```

### 13.2 Reactive Transaction Example
```brief
rct async txn process_event [event_ready][event_processed] {
  let event = get_event();
  &event_processed = true;
  term;
};

rct async txn log_event [event_processed][logged] {
  log("Event processed");
  &logged = true;
  term;
};
```

### 13.3 Borrow Rules Example
```brief
let global_counter: Int = 0;

rct txn increment [true][global_counter > 0] {
  &global_counter = global_counter + 1;
  term;
};

rct async txn read_counter [global_counter > 0][read_complete] {
  let local_copy = global_counter;  # Read higher-scope var
  &read_complete = true;
  term;
};
```

### 13.4 Async Transaction Ownership Example
```brief
let access: Int = 0;
let data: String = "";

# LEGAL: Pre-conditions are mutually exclusive
async txn writer_a [access == 0][access == 1] {
  &data = "first";
  &access = 1;
  term;
};

async txn writer_b [access == 1][access == 0] {
  &data = "second";
  &access = 0;
  term;
};
```

### 13.5 Contract Verification Example
```brief
let counter: Int = 0;

# Compiler verifies: from pre (counter < 3), all paths lead to post (counter == @counter + 1)
rct txn increment [counter < 3][counter == @counter + 1] {
  &counter = counter + 1;
  term;
};

# Compiler verifies: termination is reachable (path exists from counter < 3 to term)
# Compiler verifies: pre-condition is satisfiable (counter can be 0, 1, or 2)
```

---

## 14. Error Messages

Brief compiler errors should teach the programmer Brief. Error messages should include:

1. **What failed**: The specific check that failed (e.g., "contract implication", "termination reachability")
2. **Why it failed**: The logical path or constraint that caused the failure
3. **How to fix it**: Suggested changes in Brief terms

**Example - Contract Verification Failure:**
```
[E001] Transaction 'increment' violates post-condition

Path analysis:
  1. Pre-condition: counter < 3
  2. Guard entered: (no additional guards)
  3. Assignment: counter' = counter + 1
  4. Post-condition expected: counter == @counter + 1
  
Why this happened:
  - After increment, counter can be 3 (when starting from 2)
  - But loop continues and counter can exceed 3
  
To fix:
  - Add guard before term: [counter < 3] term;
```

---

*End of Specification v5.0*