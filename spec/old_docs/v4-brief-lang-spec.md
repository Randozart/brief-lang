# Brief Language Specification - Definitive Reference
**Version:** 4.0 Multi-Output & Proof Engine
**Date:** 2026-04-04
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

**Examples:**
```brief
import std.io;                           # Import everything from std.io
import { print } from std.io;            # Import specific symbol
import { print as p } from std.io;       # Import with alias
import { map, filter as f } from collections;  # Multiple with aliases
```

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
```acr
txn increment [count < 10][count == @count + 1] {
  &count = count + 1;
  term;
};
```

### 4.3 Syntactic Sugar `~/`
`[~/ready]` is compiled directly into `[~ready][ready]`. It instructs the runtime: *"This transaction fires when `ready` is false, and must result in `ready` being true."*

### 4.4 Term (The Convergence Operator)
`term` is **not** a return statement. It is a "Settle Instruction".
When `term` is invoked, the runtime evaluates the postcondition. If the postcondition is false, the runtime **loops** the transaction body. It continuously attempts to converge on the truth. If it succeeds, the state mutates globally.

### 4.5 Flat Logic Guards `[condition]`
Instead of nested `if` blocks, Brief uses inline logic gates. If a guard evaluates to `false`, the rest of the line is skipped.
```acr
let result = attempt();
[result == true] &successes = @successes + 1;
[result == false] &failures = @failures + 1;
```

---

## 5. Promises, Signatures, and Exhaustiveness

### 5.1 Signatures (`sig`)
Signatures define the boundary between Brief logic and the external host system (I/O, Network, OS). 

*   **Fallible Signatures**: `sig fetch: Int -> User | Error;` (The AI must handle both outcomes).
*   **Infallible Signatures**: `sig print: String -> true;` (The host guarantees execution. Error handling is abstracted away).
*   **Contract-Bound Signatures**: `sig get_id: String -> Int[~/0];` (Guarantees the return value mathematically satisfies the constraint, e.g., never returning zero).

### 5.2 Exhaustive Unification
When a `sig` returns a union type (`User | Error`), the compiler forces the developer to handle all possible branches using flat unification.

```acr
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
- **Sig as local cast / contract projection**: `sig` can be cast over a local defn to narrow its output contract. The compiler verifies the narrow path is reachable and strips unreachable branches. Example: given defn `apirequest` returning `Data | null | timeout | autherror`, you can declare `sig api_request: string -> Data` to project only the happy path.

---

## 5.4 Multi-Output Functions

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

### 5.5 Signature Projection

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

### 5.6 The `-> true` Assertion

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

**Example: Context-dependent - approved if callers use it safely**
```brief
defn bool_return(b: Bool) -> Bool {
    term b;
};

# If program only calls bool_return(true), assertion holds
sig bool_return: Bool -> true;  # ✅ If all call sites pass true
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

sig print: String -> true;  # Auto-generated

rct txn hello [~done][done] {
  &done = true;
  term;
}
```

The `term expression;` form implicitly:
1. Generates `sig fn: Args -> true;`
2. Adds `&done = true;` before `term;`
3. The `done` variable is auto-declared at top level with `let done: Bool = false;`

**Only works for `rct txn` with `[~done][done]` pattern.**

---

## 6. Reactive Transactions (New)

### 6.1 Synchronous Reactive Transactions (`rct`)
Reactive transactions (`rct`) are the core reactive execution units. Unlike regular `txn` blocks which are passive and do not self-fire, `rct` blocks are part of the Blackboard reactor loop and fire synchronously when their preconditions hold.

**Syntax:**
```bnf
rct_transaction ::= "rct" ("async")? "txn" identifier contract "{" body "}" ";"
```

**Example:**
```acr
rct txn process_order [order_ready][order_processed] {
  &order_processed = true;
  term;
};
```

### 6.2 Asynchronous Reactive Transactions (`rct async`)
Asynchronous reactive transactions run concurrently but enforce compiler-verified safety. The compiler proves no conflicting variable access between concurrent `rct async` transactions.

**Syntax:**
```acr
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

## 7. Borrow and Scope Rules (New)

### 7.1 Variable Scoping
Brief uses explicit scoping rules to manage variable lifetime and ownership.

**Syntax:**
```bnf
assignment ::= ("&" | "const")? identifier "=" expression ";"
```

### 7.2 Local Scope (`let`)
`let` creates a local variable with block scope. Variables declared with `let` are safe and automatically garbage collected when out of scope.

**Example:**
```acr
let temp_value = 5;  # Local to current transaction
&global_var = temp_value;  # Write to higher-scope variable
```

### 7.3 Explicit Write Claims (`&`)
The `&` prefix on the left side of an assignment creates an explicit write claim on a higher-scope variable.

**Syntax:**
```acr
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
```acr
defn compute(): Int [true][result > 0] {
  term 42;
};

let result: Int;
result = compute();  # Implicit write claim on 'result'
```

### 7.5 Compiler Enforcement for Async Transactions
The compiler enforces strict borrow rules for `rct async` transactions:

1. **Conflicting Write Claims**: Two `rct async` transactions cannot claim write access (`&var`) to the same variable
2. **Conflicting Read/Write Claims**: An `rct async` transaction cannot read a variable while another claims write access to it
3. **Concurrent Reads**: Multiple `rct async` transactions can read the same variable simultaneously (shared read access)

**Example: Valid Async Transactions:**
```acr
# These can run concurrently because they don't conflict
rct async txn reader1 [~read1][read1] {
  let val = data;  # Read only
  &read1 = true;
  term;
};

rct async txn reader2 [~read2][read2] {
  let val = data;  # Read only
  &read2 = true;
  term;
};
```

**Example: Invalid Async Transactions (Compiler Error):**
```acr
# These cannot run concurrently - conflicting write claims
rct async txn writer1 [~write1][write1] {
  &data = "first";  # Write claim
  &write1 = true;
  term;
};

rct async txn writer2 [~write2][write2] {
  &data = "second";  # Conflicting write claim
  &write2 = true;
  term;
};
```

---

## 8. Ownership and Concurrency (Existing)

### 8.1 Exclusive Access (`&`)
Variables declared via `let` are read-only by default. To mutate a variable, a transaction must claim exclusive write ownership using the `&` prefix.
```acr
&balance = balance - amount;
```

### 8.2 Lock-Free Concurrency
Brief achieves concurrent thread safety without `Mutexes` or explicit locks. 
If two `async txn` blocks mutate the same `&variable`, the compiler proves safety by ensuring their preconditions are **mutually exclusive**.

```acr
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

### 8.3 Await as Ownership Claim
`await` suspends the transaction and claims exclusive ownership of a variable for the duration. The compiler must prove no other transaction can claim that variable while the await is pending — same mutual exclusion proof as synchronous `&` claims, extended across the async window. If the compiler can't prove exclusion, it's a compile error.

---

## 9. Compilation and Proof Engine

Brief utilizes a two-stage pipeline: a fast AST Linter for development, and a rigorous Proof Engine for deployment.

### 9.1 DAG-Based Dead Code Detection
During semantic analysis, the compiler maps all `[pre]` and `[post]` contracts into a Directed Acyclic Graph (DAG).
- If a transaction requires a state (e.g., `[step == 5]`), but no initial state or postcondition ever produces `step == 5`, the compiler throws an `Unreachable State` error.
- This mathematically guarantees that no "Dead Logic" can be deployed.

### 9.2 Proof Obligations
The compiler verifies:
1. **Promise Exhaustiveness**: All union branches of a `sig` are unified.
2. **Mutual Exclusion**: No two concurrent transactions share overlapping preconditions if they mutate the same `&variable`.
3. **Contract Implication**: For all `defn` blocks, the logic body provably leads to the `[post]` condition.
4. **Borrow Safety**: For `rct async` transactions, no conflicting write claims or read/write conflicts exist.
5. **True Assertion**: For `sig fn: T -> true`, the compiler proves the projected Bool output is always `true` given actual usage.

### 9.3 Dependency-Tracking Optimization
Instead of evaluating all preconditions every tick, the compiler already computes which variables each transaction reads. At runtime, only re-evaluate transactions when a variable they depend on changes. Same semantics, fewer wasted cycles. This makes browser deployment viable.

---

## 10. Comprehensive Examples (The Brief Way)

### 10.1 API Fetch with Fallbacks
```acr
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

### 10.2 Reactive Transaction Example
```acr
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

### 10.3 Borrow Rules Example
```acr
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

### 10.4 Multi-Output Function Example
```brief
defn validate_input(input: String) -> Bool, String, String {
    [input.len() == 0] term false,, "Error: empty", "Please provide input";
    [input.len() < 3] term false,, "Error: too short", "Minimum 3 characters";
    [input.len() >= 3] term true,, "OK", input;
};

# Project only the success flag
sig validate_input: String -> Bool as is_valid;

# Project success and message
sig validate_input: String -> Bool, String as check_input;

# Assert success is always true (only safe if all callers guarantee valid input)
sig validate_input: String -> true;
```

### 10.5 Sugared Transaction Example
```brief
import { print } from std.io;

rct txn hello [~/done] {
  term print("Hello, World!");
}
```

Desugars to:
```brief
let done: Bool = false;
sig print: String -> true;

rct txn hello [~done][done] {
  &done = true;
  term;
}
```

---

## 11. Common Patterns (Declarative Logic)

These patterns demonstrate how Brief rejects imperative spaghetti code in favor of contract-driven logic.

### Pattern 1: Guarded Concurrency (The Contract as a Mutex)
No manual locks. The contract acts as a hardware-level gate.
```acr
let busy: Bool = false;

txn worker [~/busy] {
  term do_work();
};
```

### Pattern 2: Atomic Swap (No Temp Variable)
The postcondition enforces the swap. The runtime handles the atomic transition.
```acr
let a: Int = 1;
let b: Int = 2;

txn swap [a == @b && b == @a] {
  &a = b;
  &b = @a;
  term;
};
```

### Pattern 3: The Smart Retry (Automatic Convergence)
The `term` instruction automatically loops the transaction body until the `[post]` constraint is met (or until it escapes).
```acr
let status: Bool = false;
let tries: Int = 0;

txn resilient_op [~status][status || tries == 5] {
  &status = attempt_action();
  &tries = @tries + 1;
  term;
};
```

### Pattern 4: Circuit Breaker (Flat Logic Gates)
Replacing nested `if/else` with flat, un-nested evaluation.
```acr
let fails: Int = 0;
let open: Bool = false;
const LIMIT: Int = 5;

txn circuit_checker [~open] {
  let res = risky_op();
  
  [res == true] &fails = 0;
  [res == false] &fails = @fails + 1;
  [fails >= LIMIT] &open = true;
  
  term;
};
```

---

## 12. Design Decisions

This section covers specific syntax and semantic choices made to balance flexibility, safety, and LLM token efficiency.

### 12.1 Import Syntax
Imports bring names into scope. The compiler tracks imported symbols.

**Syntax:**
```bnf
import_stmt ::= "import" ("{" import_item ("," import_item)* "}")? ("from" namespace_path)? ";"
import_item ::= identifier ("as" identifier)?
namespace_path ::= identifier ("." identifier)*
```

**Examples:**
```brief
import std.io;                           # Import everything
import { print } from std.io;            # Import specific
import { print as p } from std.io;       # Import with alias
import { map, filter as f } from collections;  # Multiple
```

**Note:** `import path;` (without `from`) imports everything from the path.

### 12.2 Signature Syntax (`sig`)
Signatures define strict contract boundaries. They project specific outputs from multi-output functions and can assert properties.

**Syntax:**
```bnf
sig identifier ":" type_spec "->" result_type ("from" namespace_path)? ("as" alias)? ";"
result_type ::= type ("," type)* | "true"   # Projection or assertion
```

**Examples:**
```brief
sig get_user: Int -> User | Error from db.lib as db_user;
sig print: String -> Bool as safe_print;      # Project first Bool
sig print: String -> Bool, Bool as both;      # Project both Bools
sig print: String -> true;                    # Assert: Bool is always true
```

**Key Features:**
- **Projection**: `-> Type` projects the first matching type from defn's outputs
- **Multi-projection**: `-> A, B` projects multiple outputs in order
- **Assertion**: `-> true` asserts the projected Bool is always true (proof required)
- **Aliasing**: `as alias` creates a local name for the function
- **From clause**: Specifies source library (required for external functions)

### 12.3 Three Tiers of Contract Declaration
Brief supports three levels of contract strictness:

1. **Inferred Contracts**: Term position implies contract based on usage
   ```acr
   let result = fetch(url);  # Compiler infers return type from signature
   ```

2. **Explicit `sig`**: Overrides inference with explicit contract boundary
   ```acr
   sig fetch: String -> Data from "net.lib";
   let result = fetch(url);  # Strict type checking
   ```

3. **Raw Import**: No contract needed for direct library access
   ```acr
   import { helper } from "utils.lib";  # No type constraints
   helper(data);  # Loose typing, compiler trusts the source
   ```

### 12.4 Term Fall-Through
The `term` statement evaluates its expression and commits only if truthy. If falsy, execution falls through to the next statement. This enables priority-ordered outcome matching.

**Syntax:**
```bnf
term_stmt ::= "term" expression ";"
```

**Example:**
```acr
txn process_order [order_ready] {
  term validate(order);    # If true, commit and exit
  term retry(order);       # If previous was false, try this
  term abort(order);       # Final fallback
};
```

**Behavior:**
- Each `term` evaluates its expression
- If truthy: transaction commits and exits
- If falsy: execution continues to next statement
- Multiple `term` statements act as priority-ordered guards

### 12.5 Total-Path Checking
The compiler proves every `rct txn` has an accepting path — at least one `term` expression on every control flow through the body that evaluates to truthy. If no guaranteed commit path exists, the transaction is rejected.

**Proof Requirements:**
1. All branches must lead to a `term` statement
2. At least one `term` must evaluate to truthy in every possible execution path
3. If control flow can reach a point without a truthy `term`, compilation fails

**Example (Valid):**
```acr
rct txn handle_result [result_ready] {
  let res = get_result();
  term res.success;      # Path 1: Success commits
  term !res.success;     # Path 2: Failure also commits (truthy)
};
```

**Example (Invalid - Rejected):**
```acr
rct txn incomplete [true] {
  let data = fetch();
  [data != null] term data;  # Only commits if data is not null
  # Missing: what happens if data IS null? No accepting path.
};
# Compiler Error: "No guaranteed accepting path in rct txn 'incomplete'"
```

---

## 13. Conclusion

Brief forces a paradigm shift: programs are verified legal settlements between the developer and the runtime. 

By eliminating imperative loops, nested blocks, and manual error tracking, Brief provides a radically token-efficient canvas. It is designed from the ground up for LLMs to generate provably safe, mathematically sound, and concurrently resilient autonomous agents.
