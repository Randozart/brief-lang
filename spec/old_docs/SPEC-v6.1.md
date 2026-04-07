# Brief Language Specification
**Version:** 6.1 Multi-Output Functions, Sig Polymorphism & Adaptive Reactor Scheduling  
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
6. **Polymorphism Through Constraints**: Functions can declare multiple possible outputs; callers must handle all, OR sig casting constrains them to one type with compiler verification.

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

output_types ::= output_union ("," output_union)*        # Multi-output with precedence
output_union ::= type ("|" type)*                        # Union: pick one
result_type ::= output_union | "true" | output_projection
output_projection ::= type ("," type)*                   # Sig projection to specific types

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
player.name       // Access field
player.score      // Access another field
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

### 2.4 Statements (Zero-Nesting)

```bnf
statement ::= 
    | assignment
    | unification
    | guarded_stmt
    | guarded_block
    | term_stmt
    | escape_stmt
    | expression ";"

assignment ::= ("&")? identifier "=" expression ";"

unification ::= identifier "(" identifier ")" "=" expression ";"

guarded_stmt ::= "[" expression "]" statement
guarded_block ::= "[" expression "]" "{" statement* "}"

term_stmt ::= "term" expression? ("," expression?)* ";"
escape_stmt ::= "escape" expression? ";"
```

### 2.5 Expressions

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
    | "&" identifier                    // ownership reference
    | "@" identifier                    // prior-state reference
    | call
    | paren_expression

call ::= identifier "(" arguments? ")"
arguments ::= expression ("," expression)*
```

### 2.6 Comments

```bnf
comment ::= "//" [^\n]*

# Comments can appear anywhere except inside string literals
# Lexer filters comments; parser never sees them
```

**Rules:**
- Syntax: `// text to end of line`
- Can appear in transaction bodies, defn bodies, expressions
- Cannot appear inside string literals
- Multiple comments per line OK: `x = 1; // assign // comment`

**Examples:**
```brief
// This is a comment at statement level
txn transfer [pre][post] {
    &balance = balance - 10;  // Deduct amount
    term;                      // Settle transaction
};

defn add(a: Int, b: Int) [true][true] -> Int {
    // Add two numbers
    term a + b;  // Return sum
};
```

### 2.7 Imports and Namespaces

```bnf
import_stmt ::= "import" ("{" import_item ("," import_item)* "}")? (("from" namespace_path) | namespace_path)? ";"
import_item ::= identifier ("as" identifier)?
namespace_path ::= identifier ("." identifier)*
```

**Examples:**
```brief
import std.io;                           // Import everything from std.io (shorthand)
import { print } from std.io;            // Import specific symbol
import { print as p } from std.io;       // Import with alias
import { map, filter as f } from collections;  // Multiple with aliases
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

### 3.3 Adaptive Reactor Scheduling with `@Hz` Declarations

Brief uses a **single global reactor** that adapts to the polling needs of all loaded files. This optimization eliminates unnecessary polling while maintaining reactive responsiveness.

#### 3.3.1 Default Polling Rate

```brief
reactor @10Hz;
```

The file-level `reactor` directive sets the **default polling speed** for all `rct` blocks in that file. If not specified, the default is `@10Hz`.

**Rules:**
- `@10Hz` means the reactor evaluates preconditions 10 times per second
- This applies to all `rct` blocks in the file that don't have explicit speed declarations
- Pure files without `rct` blocks never activate the reactor (zero CPU cost)

#### 3.3.2 Per-rct Speed Declaration

```brief
rct [condition] txn name [pre][post] { ... } @60Hz;
```

Individual reactive transactions can declare their own polling speed, overriding the file-level default.

**Examples:**
```brief
reactor @10Hz;  // File default: 10Hz

rct [data_ready] txn process [pre][post] { ... };      // Uses @10Hz
rct [frame_tick] txn render [pre][post] { ... } @60Hz;  // Overrides to @60Hz
rct [event] txn handle [pre][post] { ... } @5Hz;        // Overrides to @5Hz
```

#### 3.3.3 Global Reactor Adaptation

When multiple files are loaded, Brief uses a **single global reactor** that adapts to requirements:

1. **Collector phase:** Compiler collects all `@Hz` declarations from all files
2. **Adaptation:** Global reactor runs at `max(@Hz)` across all files
3. **Intelligent skipping:** Files declaring slower speeds are checked only when needed
   - Example: If global runs at `@60Hz` but file needs `@10Hz`, that file is checked every 6 ticks
   - Zero overhead for slower files

**Example:**
```
File A (mainMenu.rbv):     reactor @10Hz;
File B (gameLogic.bv):     reactor @60Hz;
File C (counter.bv):       no rct blocks (no reactor)

Global reactor: Runs at @60Hz
- mainMenu.rbv preconditions checked every 6 ticks (60/10)
- gameLogic.bv preconditions checked every tick
- counter.bv never participates (pure library)
```

#### 3.3.4 R.rbv Optimization

Components without `rct` blocks have **zero reactor overhead**:

```brief
<script type="brief">
  // No rct blocks = no reactor = no connection overhead
  let count: Int = 0;
  
  txn increment [true][count == @count + 1] {
    &count = count + 1;
    term;
  };
</script>
```

This is critical for R.rbv files served over connections (browsers, mobile). Passive components consume zero polling bandwidth.

#### 3.3.5 Compiler Warnings

The compiler emits warnings for suspicious polling rates:

```
⚠️ WARNING: Reactor speed @10000Hz is extremely aggressive
   Did you intend to set your PC aflame or crash to desktop?
   Consider using a more reasonable speed (e.g., @60Hz for games, @10Hz for UI)
```

**Warning thresholds:**
- `@10000Hz` and above: Aggressive warning
- Suggests reasonable alternatives based on use case

#### 3.3.6 Performance Characteristics

| Scenario | Reactor Speed | CPU Cost | Notes |
|----------|---------------|----------|-------|
| Pure library (defn only) | Inactive | ~0% | No rct blocks |
| UI app + passive display | @10Hz | Low | Most browser apps |
| Game with logic + render | @60Hz | Medium | gameLogic + render |
| Real-time system | @1000Hz | High | Requires careful tuning |

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

### 4.5 Flat Logic Guards `[condition]` and `[condition] { }`
Instead of nested `if` blocks, Brief uses inline logic gates. 

**Flat syntax:** If a guard evaluates to `false`, the rest of the line is skipped.
```brief
let result = attempt();
[result == true] &successes = @successes + 1;
[result == false] &failures = @failures + 1;
```

**Block syntax:** Multiple statements can be guarded with braces.
```brief
let amount = 50;
[amount > 100] {
  &large_transfers = large_transfers + 1;
  &total = total + amount;
};
[amount <= 100] {
  &small_transfers = small_transfers + 1;
  &total = total + amount;
};
```

Both syntaxes are equivalent and can be mixed. Choice is stylistic.

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

A `defn` can declare multiple output types. The caller **must handle all declared outputs**.

#### 5.5.1 Output Type Syntax

**Union type** (pick one): `-> Bool | String | Int`
- Execution can produce any one of these types
- Caller must handle ALL possibilities

**Tuple type** (all of these): `-> Bool, String, Int`
- Execution produces all these types in sequence
- Caller must bind variables for all slots

**Mixed** (union then tuple): `-> Bool | String, Int, String`
- Output slot 0: `Bool OR String` (pick one)
- Output slot 1: `Int` (always)
- Output slot 2: `String` (always)
- Caller must handle: (Bool|String, Int, String)

**Precedence**: Comma binds tighter than pipe
- `A | B, C` = `(A | B), C` (not `A | (B, C)`)

#### 5.5.2 Union Exhaustiveness

When a `defn` declares union outputs, **the caller must handle all types**:

```brief
defn get_status(id: Int) -> Bool | String {
  [id > 0] term true;
  [id <= 0] term "error";
};

// INVALID - doesn't handle String:
let b: Bool = get_status(5);
// Compiler error: "get_status declares Bool | String; missing String"

// VALID - handles both:
let b: Bool;
let s: String = get_status(5);

// VALID - explicit binding:
let result = get_status(5);
Bool(b) = result; &my_bool = b;
String(s) = result; &my_string = s;
```

#### 5.5.3 Output Variable Names in Postconditions

Postconditions can reference output variable names:

```brief
defn divide(a: Int, b: Int) [b != 0][result == a / b] -> Int {
  let result: Int = a / b;
  term result;
};

defn maybe_succeed(should_work: Bool) [true][output == true] -> Bool {
  term should_work;
};
```

The compiler maps `result` and `output` to the `term` statement values and verifies the postcondition is satisfiable.

### 5.6 Signature Projection

A `sig` can project specific outputs from a multi-output `defn`.

#### 5.6.1 Type Projection

```brief
defn get_pair(id: Int) -> Bool, String {
  term true;
  term "success";
};

sig get_bool: Int -> Bool;       // Extract Bool from tuple
sig get_string: Int -> String;   // Extract String from tuple
sig get_tuple: Int -> Bool, String;  // Take full tuple
```

**Verification Rule:** For each output type in the sig:
- At least one execution path in the defn must produce that type
- No path can fail the postcondition for that projection

#### 5.6.2 Assertion with `-> true`

A sig can assert that the Bool output is **always `true`** given all constraints:

```brief
sig always_succeeds: Args -> true;
```

This casts the Bool output AND asserts it is **always true** on all execution paths.

#### 5.6.3 The `-> true` Assertion

When you write `sig fn: T -> true`, the compiler must **prove** that the projected Bool output is always `true` given:
1. The `defn`'s code paths
2. The actual inputs passed at call sites in your program
3. The defn's postcondition

**Example: Guaranteed true**
```brief
defn always_true(x: Int) [true][result == true] -> Bool {
    term true;
};

sig always_true: Int -> true;  // ✅ Approved - all paths produce true
```

**Example: Conditional - rejected**
```brief
defn maybe_true(b: Bool) [true][true] -> Bool {
    term b;
};

sig maybe_true: Bool -> true;  // ❌ Rejected - b could be false
```

**Example: Context-aware - valid**
```brief
defn maybe_true(b: Bool) [true][true] -> Bool {
    term b;
};

txn caller [~success] {
  let result = maybe_true(true);  // Always called with true
  &success = result;
  term;
};

sig contextualized: Bool -> true;  // ✅ Valid - given call context, always true
```

### 5.7 Sugared Signature Inference

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

sig print: String -> true;  // Auto-generated

rct txn hello [~done][done] {
  &done = true;
  term;
}
```

---

## 6. Path Selection & Output Buffering

### 6.1 Default Behavior (No Casting)

When calling a `defn` without explicit sig casting, the first happy path wins:

```brief
defn multi_path(x: Int) -> Bool | String {
  [x > 0] term true;           // Path 1: returns Bool
  [x < 0] term "negative";     // Path 2: returns String
};

let result = multi_path(5);
// Execution: x=5 > 0, reaches term true
// Returns: true (Bool)
// Path 2 is never reached
```

### 6.2 Output Buffering for Tuples

When a `defn` declares tuple outputs, execution buffers values until all slots are filled:

```brief
defn pair_output() -> Bool, String {
  term true;        // Fills slot 0 (Bool)
  term "msg";       // Fills slot 1 (String)
};

let (b, s) = pair_output();
// Execution: reaches term true, buffers as slot[0]
// continues to term "msg", buffers as slot[1]
// All slots filled, returns (true, "msg")
```

### 6.3 Smart Type Inference

When caller context requires specific types, buffering adapts:

```brief
defn flexible() -> Int | Bool, String {
  term 42;          // Could fill slot 0 (Int from union)
  term "text";      // Fills slot 1 (String)
};

// Caller expects: Int, String tuple
let (i, s) = flexible();
// Compiler infers: buffer 42 as Int, then "text" as String
// Result: (42, "text")

// Caller expects: Bool, String tuple
let (b, s) = flexible();
// Compiler error: Path produces Int, not Bool in slot 0
```

---

## 7. Reactive Transactions

### 7.1 Synchronous Reactive Transactions (`rct`)
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

### 7.2 Asynchronous Reactive Transactions (`rct async`)
Asynchronous reactive transactions run concurrently but enforce compiler-verified safety. The compiler proves no conflicting variable access between concurrent `rct async` transactions.

**Syntax:**
```brief
rct async txn fetch_data [~/data_loaded][data_loaded] {
  let result = network_request();
  Data(d) = result; &data = d; &data_loaded = true; term;
};
```

### 7.3 Entry Point and Equilibrium
Brief programs have no `main()` function. Entry is whichever `rct`'s preconditions hold first. The program reaches equilibrium when no `rct` preconditions evaluate to true.

**Flow:**
1. Reactor evaluates all `[pre]` conditions for `rct` blocks only
2. First matching `rct` fires synchronously (blocks others)
3. For `rct async`, the compiler verifies mutual exclusion of write claims
4. Program continues until equilibrium (no active `rct` preconditions)

---

## 8. Borrow and Scope Rules

### 8.1 Variable Scoping
Brief uses explicit scoping rules to manage variable lifetime and ownership.

**Syntax:**
```bnf
assignment ::= ("&" | "const")? identifier "=" expression ";"
```

### 8.2 Local Scope (`let`)
`let` creates a local variable with block scope. Variables declared with `let` are safe and automatically garbage collected when out of scope.

**Example:**
```brief
let temp_value = 5;  # Local to current transaction
&global_var = temp_value;  # Write to higher-scope variable
```

### 8.3 Explicit Write Claims (`&`)
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

### 8.4 Function Return Assignment
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

## 9. Ownership and Concurrency

### 9.1 Exclusive Access (`&`)
Variables declared via `let` are read-only by default. To mutate a variable, a transaction must claim exclusive write ownership using the `&` prefix.
```brief
&balance = balance - amount;
```

### 9.2 Lock-Free Concurrency via Pre-condition Exclusivity
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

### 9.3 Async Transaction Ownership Rules

For async transactions that may run concurrently, the following ownership rules apply:

**Read Access:**
- Many transactions may read the same variable simultaneously (shared read)
- Reading never blocks other readers or writers

**Write Access:**
- Only one transaction may hold write access (`&variable`) at a time
- If one transaction has write access, no other transaction may read OR write that variable

**Pre-condition Exclusivity Requirement:**
Two async transactions that both claim write access to the same outer-scope variable must satisfy ONE of:
1. Their pre-conditions are mutually exclusive (can never both be true)
2. They can never be called simultaneously (enforced by caller)

Multiple readers are always allowed simultaneously. Writes require exclusive access.

```brief
# ILLEGAL: Both write to &counter, pre-conditions can overlap
async txn inc_a [true][...] { &counter = ... }
async txn inc_b [true][...] { &counter = ... }

# LEGAL: Pre-conditions are mutually exclusive
async txn inc_a [counter == 0][...] { &counter = ... }
async txn inc_b [counter == 1][...] { &counter = ... }

# LEGAL: Multiple readers allowed (no blocking)
async txn reader_a [true][...] { let x = counter; ... }
async txn reader_b [true][...] { let y = counter; ... }

# ILLEGAL: Writer blocks all reads and writes
async txn reader [true][...] { let x = counter; ... }
async txn writer [x == 0][...] { &counter = ... }
```

### 9.4 Await as Ownership Claim
`await` suspends the transaction and claims exclusive ownership of a variable for the duration. The compiler must prove no other transaction can claim that variable while the await is pending — same mutual exclusion proof as synchronous `&` claims, extended across the async window. If the compiler can't prove exclusion, it's a compile error.

---

## 10. Contract Verification

This section defines the compile-time verification that the Brief compiler must perform to ensure program correctness.

### 10.1 Pre-condition Satisfiability

The compiler must verify that a transaction's pre-condition is **satisfiable** — there exists at least some state where pre-condition evaluates to true.

If pre-condition is unsatisfiable (e.g., `[count > 0 && count < 0]`), the transaction can never fire, and the compiler should emit a warning or error.

### 10.2 Path Verification: Pre → Post

For every transaction and definition, the compiler must verify that for **all paths** from the pre-condition through guards to termination, the post-condition holds.

**Transaction Model:**
- Transaction executes top-to-bottom
- Guards `[condition] { ... }` branch based on condition
- On reaching `term`, post-condition is evaluated

**Verification Process:**
1. Enumerate all possible paths through guards
2. For each path, collect path constraints (guard conditions that must be true)
3. Verify that (pre-condition AND path constraints) implies post-condition
4. Use symbolic execution to track variable assignments through paths

**Example:**
```brief
txn increment [counter >= 0][counter == @counter + 1] {
  [counter < 10] &counter = counter + 1;
  [counter >= 10] &counter = counter;  # No-op path
  term;
};
```

The compiler must verify both paths (counter < 10 and counter >= 10) lead to post-condition satisfaction.

### 10.3 Termination Reachability

The compiler must verify that there exists at least one path from the pre-condition to a terminating `term`.

This is critical because **all transactions loop** — if there's no path to termination, the transaction would loop forever.

**Verification:**
1. Build control flow graph through guards
2. Verify at least one path from pre-condition to `term`
3. For definitions with iteration constructs `[i < n]`, verify the iteration is bounded and terminates

### 10.4 Contract Implication for Definitions

Definitions (`defn`) are verified as **Hoare triples**: `{pre} body {post}`

Given the pre-condition holds when the defn is called, the post-condition must hold after the defn executes.

**Key Difference from Transactions:**
- defn runs once (no looping)
- No prior-state (`@`) in defn contracts (no state mutation)
- Pre-condition constrains valid inputs

### 10.5 Multi-Output Verification

For defn with multiple outputs, the compiler must verify:

1. **Union Exhaustiveness**: All declared output types are reachable
2. **Caller Obligation**: Caller must bind variables for all output types
3. **Output Buffering**: Tuple outputs are collected correctly
4. **Output Variable Names**: Postconditions using output names are satisfiable

**Example:**
```brief
defn safe_fetch(url: String) -> JSON | Bool {
  [valid_url] term fetch_json(url);
  [~valid_url] term false;
};

// INVALID - doesn't handle Bool:
let json: JSON = safe_fetch(url);

// VALID - handles both:
let json: JSON;
let success: Bool = safe_fetch(url);
```

### 10.6 Sig Casting Verification

For a sig cast `sig func: Args -> OutputType`:

**Type Projection Verification:**
- At least one path in the defn must produce OutputType
- All other paths can produce other types (will be filtered out)
- Postcondition must be satisfiable on the selected path

**Assertion Verification (`-> true`):**
- At least one path must produce `Bool = true`
- That path's guards and assignments must constrain the Bool to true
- Consider:
  - Guard constraints on that path
  - Postcondition of the defn
  - All inputs passed at call sites

**Example:**
```brief
defn print(msg: String) -> Void | Bool | String {
  &output = msg;
  term Void;
  term true;
  [msg.len() > 0] term msg;
};

sig print_success: String -> true;
// ✅ Path 2 produces true; assertion satisfied

sig print_by_id: String -> Void;
// ✅ Path 1 produces Void; type projection satisfied

defn endpoint(id: Int) -> JSON | Error {
  [id > 0] term fetch(id);
  [id <= 0] term Error("Invalid");
};

sig guaranteed_json: Int -> true;
// ❌ No path produces Bool; assertion fails
```

### 10.7 True Assertion Verification

For `sig fn: T -> true`, the compiler must prove the projected Bool output is always `true` given:
1. The defn's code paths
2. The actual inputs passed at call sites in the program

This requires analyzing all call sites and verifying the Bool output is provably true on all paths.

---

## 11. Proof Engine Requirements

The Brief compiler's Proof Engine must verify the following obligations:

### 11.1 Promise Exhaustiveness
All union branches of a `sig` return type must be handled via unification.

### 11.2 Mutual Exclusion
No two concurrent transactions (async or sync with overlapping execution) may have overlapping pre-conditions if they both claim write access to the same variable.

### 11.3 Contract Implication (Symbolic Execution)
For all transaction and definition bodies, verify that pre-condition + path constraints implies post-condition. This requires **symbolic executor** to track variable assignments through paths.

### 11.4 Borrow Safety
For async transactions:
- No conflicting write claims to the same variable
- No read while another transaction holds write access
- Pre-conditions must be mutually exclusive for overlapping writes

### 11.5 Multi-Output Verification
- Union exhaustiveness at call sites
- Output buffering correctness for tuples
- Output type projection validity

### 11.6 True Assertion
For `sig fn: T -> true`, verify the projected Bool output is always true.

### 11.7 Termination Reachability
Verify that every reactive transaction has at least one path from pre-condition to termination.

---

## 12. Compilation and Proof Engine Pipeline

Brief utilizes a two-stage pipeline: a fast AST Linter for development, and a rigorous Proof Engine for deployment.

### 12.1 DAG-Based Dead Code Detection
During semantic analysis, the compiler maps all `[pre]` and `[post]` contracts into a Directed Acyclic Graph (DAG).
- If a transaction requires a state (e.g., `[step == 5]`), but no initial state or postcondition ever produces `step == 5`, the compiler throws an `Unreachable State` error.
- This mathematically guarantees that no "Dead Logic" can be deployed.

### 12.2 Dependency-Tracking Optimization
Instead of evaluating all preconditions every tick, the compiler already computes which variables each transaction reads. At runtime, only re-evaluate transactions when a variable they depend on changes. Same semantics, fewer wasted cycles. This makes browser deployment viable.

---

## 13. Standard Library Guidelines

### 13.1 defn vs frgn

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

### 13.2 Iteration in defn vs txn

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

## 14. Comprehensive Examples

### 14.1 Multi-Output API with Fallback

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

### 14.2 Multi-Output Function with Polymorphism

```brief
defn safe_json(url: String) -> JSON | Bool {
  let json: JSON;
  let success: Bool = fetch_json(url, json);
  [success] term json;
  term false;
};

sig json_data: String -> JSON;     // Extract JSON
sig endpoint_ok: String -> Bool;   // Extract Bool

txn use_polymorphism [~done] {
  let data = json_data("https://api");
  let ok = endpoint_ok("https://api");
  &done = true;
  term;
};
```

### 14.3 Reactive Transaction Example
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

### 14.4 Borrow Rules Example
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

### 14.5 Async Transaction Ownership Example
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

### 14.6 Contract Verification with Symbolic Execution
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

### 14.7 Output Variable Names in Postconditions
```brief
defn sufficient_funds(amount: Int) [amount > 0][result == true] -> Bool {
  term true;
};

defn get_pair() [true][first > 0 && second > 0] -> Int, Int {
  term 10;
  term 20;
};
```

---

## 15. Error Messages

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

**Example - Union Exhaustiveness Failure:**
```
[E002] Incomplete union handling for 'fetch_json'

Function 'fetch_json' declares:
  -> JSON | Bool

Your code handles:
  ✓ JSON
  ✗ Bool (missing)

To fix:
  Add handler for Bool type:
  let json: JSON;
  let success: Bool = fetch_json(url);
```

---

*End of Specification v6.0*
