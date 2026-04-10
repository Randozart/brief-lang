# Brief Language Tutorial

Learn Brief by building real systems, step by step.

---

## Part 1: Getting Started

### Installation

```bash
cargo build --release
./target/release/brief --help
```

### Your First Program

Create `hello.bv`:

```brief
let message: String = "hello world";
```

Run it:

```bash
brief check hello.bv
brief build hello.bv
```

Brief programs don't need `main()` - they declare state, and transactions describe how state changes.

### State is Everything

```brief
let counter: Int = 0;
let name: String = "Alice";
let active: Bool = true;
let balance: Float = 1000.50;
```

State is declared with `let`. You can:
- Give it a type and initial value
- Give it a type without a value (defaults to 0, "", false)
- Brief infers types where possible

---

## Part 2: Transactions - Making Changes

### Your First Transaction

```brief
let count: Int = 0;

txn increment [count < 100][count == @count + 1] {
    &count = count + 1;
    term;
};
```

**Breaking this down:**

- `txn` - declares a transaction
- `increment` - the name
- `[count < 100]` - precondition: when can this run?
- `[count == @count + 1]` - postcondition: what must be true after?
- `&count = count + 1;` - mutate state (the `&` is required)
- `term;` - complete successfully

**The key insight**: You're not describing code that runs. You're declaring:
- When it's allowed to run (precondition)
- What must be true after it runs (postcondition)

The compiler proves the code actually satisfies the postcondition.

### The Prior State Operator

```brief
let balance: Int = 100;

txn withdraw(amount: Int) 
    [amount > 0 && amount <= balance]
    [balance == @balance - amount]
{
    &balance = balance - amount;
    term;
};
```

`@balance` means "the value of balance when this transaction started".

### Guards: Conditional Execution

```brief
txn process [true][true] {
    let value = compute();
    
    [value > 0] &positive = true;
    [value < 0] &negative = true;
    [value == 0] escape;  // Rollback if zero
    
    term;
};
```

`[condition] statement` only executes if the condition is true.

### Escape: Rollback

```brief
txn validate(x: Int) 
    [x >= 0][state == @state]
{
    [x > 1000] {
        escape;  // Rollback, nothing changes
    };
    &state = x;
    term;
};
```

`escape` rolls back all mutations and terminates the transaction.

---

## Part 3: Reactive Transactions

### Auto-Firing Transactions

```brief
let count: Int = 0;
let done: Bool = false;

rct txn increment [count < 10 && !done]
    [count == @count + 1]
{
    &count = count + 1;
    term;
};

rct txn finish [count >= 10 && !done]
    [done == true]
{
    &done = true;
    term;
};
```

`rct txn` (reactive transaction) automatically runs whenever its precondition becomes true.

**How it works:**
1. You change `count` from 9 to 10
2. Reactor sees `count >= 10 && !done` is now true
3. `finish` fires automatically
4. `done` becomes true
5. `increment` can't fire anymore (precondition now false)
6. Program reaches equilibrium

### Reactive State Machines

This is Brief's superpower - describe state transitions, compiler handles the rest:

```brief
let state: Int = 0;

rct txn step_1 [state == 0][state == 1] {
    &state = 1;
    term;
};

rct txn step_2 [state == 1][state == 2] {
    &state = 2;
    term;
};

rct txn reset [state == 2][state == 0] {
    &state = 0;
    term;
};
```

When you set `state = 0`, the machine automatically cycles through all three steps.

---

## Part 4: Functions (Definitions)

### Writing Functions

```brief
defn double(x: Int) -> Int [true][result == x * 2] {
    term x * 2;
};
```

**Parts:**
- `defn` - define a function
- `double` - function name
- `(x: Int)` - parameter
- `-> Int` - return type
- `[true]` - precondition (always runnable)
- `[result == x * 2]` - postcondition
- `term x * 2;` - return the value

### Multiple Return Values

```brief
defn divide(a: Int, b: Int) -> Int, Int, Bool [b != 0][true] {
    term a / b, a % b, true;
};
```

**Using it:**
```brief
let quotient, remainder, ok = divide(17, 5);
```

### Functions Can Call Other Functions

```brief
defn absolute(x: Int) -> Int [true][result >= 0] {
    [x < 0] term -x;
    [x >= 0] term x;
};

defn is_positive(x: Int) -> Bool [true][true] {
    let abs_x = absolute(x);
    [abs_x > 0] term true;
    term false;
};
```

### Pure Brief vs FFI

Pure functions that Brief can express should use `defn`:

```brief
defn min(a: Int, b: Int) -> Int [true][result == a || result == b] {
    [a <= b] term a;
    [a > b] term b;
};
```

Functions requiring system access use `frgn` (see Part 7).

---

## Part 5: Pattern Matching

### Handling Multiple Outcomes

```brief
let result: Int | String;

[Int(n) = result] &int_val = n;
[String(s) = result] &str_val = s;
```

### Guards for Branches

```brief
let value: Int = get_value();

[value > 0] &positive = true;
[value == 0] &zero = true;
[value < 0] &negative = true;
```

### Enum Pattern Matching

Enums let you define types with named variants. Pattern matching in guards destructures them:

```brief
enum Result<T, E> {
    Ok(T),
    Err(E)
}

let result: Result<Int, String> = from_json("42");

// Match on Ok - bind inner value to 'n'
[result Ok(n)] {
    &parsed_value = n;
};

// Match on Err - bind error to 'e'
[result Err(e)] {
    &error_msg = e;
};
```

The syntax is `[variable Variant(field1, field2)]` where the fields bind to the variant's inner values.

---

## Part 6: Structs

### Plain Struct

```brief
struct BankAccount {
    balance: Int;
    overdraft_limit: Int;
    
    txn deposit(amount: Int)
        [amount > 0]
        [balance == @balance + amount]
    {
        &balance = balance + amount;
        term;
    };
    
    txn withdraw(amount: Int)
        [amount > 0 && amount <= balance + overdraft_limit]
        [balance == @balance - amount]
    {
        &balance = balance - amount;
        term;
    };
};
```

### Using Structs

```brief
let account: BankAccount;
account.deposit(100);
account.withdraw(50);
```

### Render Struct

```brief
rstruct Counter {
    count: Int;
    
    rct txn increment [count < 100][count == @count + 1] {
        &count = count + 1;
        term;
    };
} -> "
<div class='counter'>
    <span>{count}</span>
    <button onclick='increment()'>+</button>
</div>
";
```

---

## Part 7: Foreign Functions (FFI)

### When to Use FFI

FFI is for operations Brief genuinely cannot do:
- File I/O
- Network access
- Console input/output
- Hardware math (sqrt, sin, etc.)

FFI is NOT for things Brief can express natively:
- Arithmetic
- Comparisons
- String operations Brief can handle

### TOML Binding

Create a file `lib/std/io.toml`:

```toml
[[functions]]
name = "read_file"
description = "Read file contents"
location = "std::fs::read_to_string"
target = "native"
mapper = "rust"

[functions.input]
path = "String"

[functions.output.success]
content = "String"

[functions.output.error]
type = "IoError"
code = "Int"
message = "String"
```

### Brief Declaration

```brief
frgn read_file(path: String) -> Result<String, IoError> from "lib/std/io.toml";
```

### Using FFI

```brief
frgn read_file(path: String) -> Result<String, IoError> from "lib/std/io.toml";

defn load_config() -> String [true][result.len() >= 0] {
    let result = read_file("config.txt");
    term "default";
};
```

### Generic FFI

```brief
frgn<T> identity(value: T) -> Result<T, Error> from "lib/std/util.toml";
```

---

## Part 8: Real Example - Bank System

```brief
// State
let alice_balance: Int = 1000;
let bob_balance: Int = 500;
let in_transfer: Bool = false;

txn transfer_to_bob(amount: Int)
    [!in_transfer && alice_balance >= amount]
    [alice_balance == @alice_balance - amount && bob_balance == @bob_balance + amount && !in_transfer]
{
    &in_transfer = true;
    &alice_balance = alice_balance - amount;
    &bob_balance = bob_balance + amount;
    &in_transfer = false;
    term;
};

rct txn alert_low_balance [alice_balance < 100][alice_balance == @alice_balance] {
    // Send alert
    term;
};
```

**How it works:**
1. You call `transfer_to_bob(100)`
2. If precondition is true, code executes
3. If postcondition is satisfied, state changes
4. If postcondition fails, entire transaction rolls back
5. Reactive transactions fire automatically based on state

---

## Part 9: Common Patterns

### Lazy Initialization

```brief
let initialized: Bool = false;
let value: Int = 0;

txn initialize [~initialized][initialized] {
    &initialized = true;
    &value = 100;
    term;
};

rct txn use_value [initialized][initialized] {
    term;
};
```

### State Machine

```brief
let state: Int = 0;  // 0=idle, 1=processing, 2=done

rct txn process [state == 0][state == 1] {
    &state = 1;
    term;
};

rct txn complete [state == 1][state == 2] {
    &state = 2;
    term;
};

rct txn reset [state == 2][state == 0] {
    &state = 0;
    term;
};
```

### Synchronization with Flags

```brief
let ready: Bool = false;
let busy: Bool = false;

txn start_work [ready && !busy][busy == true] {
    &busy = true;
    term;
};

txn finish_work [busy][busy == false] {
    &busy = false;
    term;
};
```

---

## Part 10: Syntactic Sugar

Brief provides several syntactic shortcuts that make code more concise.

### Boolean Toggle (`~/`)

`~/condition` is shorthand for `[~condition][condition]`:

```brief
// These are equivalent:
txn initialize [~/ready] {
    &ready = true;
    term;
};

txn initialize [~ready][ready] {
    &ready = true;
    term;
};
```

This reads as: "Fire when ready is false, ensure ready becomes true."

### Implicit State Declaration

When you use `~/condition`, the variable is automatically declared:

```brief
// No need to write: let ready: Bool = false;
// Brief infers it from the contract
rct txn start [~/ready] {
    &ready = true;
    term;
};
```

### Implicit Termination

When the postcondition is literal `true`, `term;` is implicitly treated as `term true;`:

```brief
// Postcondition is literal true - term; becomes term true;
txn activate [ready][true] {
    term;  // implicitly: term true;
};
```

When the postcondition is a Bool expression, `term;` checks if it is satisfied:

```brief
// Postcondition is an expression - term; checks if it is met
txn set_flag [true][flag == true] {
    &flag = true;
    term;  // checks: is flag == true satisfied?
};
```

Note: `term true;` must obey borrowing rules since it implicitly performs a state mutation.

### Lambda-Style Declarations

For simple transactions where the body is just `term`, you can omit the body:

```brief
// Full form:
txn increment [count < 100][count == @count + 1] {
    &count = count + 1;
    term;
};

// Lambda form - body is just term:
txn inc [count < 100][count == @count + 1];

// Full form:
defn double(x: Int) -> Int [true][result == x * 2] {
    term x * 2;
};

// Lambda form:
defn double(x: Int) -> Int [true][result == x * 2];
```

### Term with Function Call

`term functionCall();` means "call the function and use its return value in the postcondition":

```brief
defn addOne(x: Int) -> Int [true][result == x + 1] {
    term x + 1;
};

// The compiler verifies that addOne() produces exactly what the postcondition requires
txn increment [count < 100][count == @count + 1] {
    term addOne(@count);  // Compiler checks: addOne(@count) == @count + 1
};

// If addOne() does NOT satisfy the postcondition, compiler throws error
```

---

## Part 11: Multi-Return Functions

Brief supports powerful multi-return functions with union types.

### Single Return

```brief
defn get_value() -> Int [true][result >= 0] {
    term 42;
};

let x: Int = get_value();  // x = 42
```

### Multi-Return with Accumulation

A function can have multiple `term` statements. Each `term` adds to the accumulated return type:

```brief
defn try_parse(s: String) -> Int | Bool | String [true][true] {
    term 1;        // Can return Int
    term true;     // Can return Bool
    term "error";  // Can return String
};

let result: Int | Bool | String = try_parse("hello");
```

The function returns a union type containing all possible termination values.

### Type Inference with Multi-Return

When calling a multi-return function, the type determines which term is used:

```brief
defn try_parse(s: String) -> Int | Bool | String [true][true] {
    term 1;        // Int term
    term true;     // Bool term
    term "error";  // String term
};

// Type inference selects the appropriate term:
let integer: Int = try_parse("hello");  // Returns 1
let boolean: Bool = try_parse("hello");  // Returns true
let str: String = try_parse("hello");  // Returns "error"
```

### Accumulating Multi-Return with Tuples

For multiple return slots, use explicit tuple type notation:

```brief
defn multi() -> Int | Int, Int | Int, Int, Int [true][true] {
    term 1;        // Slot 1: returns Int
    term 2;        // Slot 2: returns Int
    term 3;        // Slot 3: returns Int
};

// How many slots you request determines which term is used:
let n1: Int = multi();              // Returns 1
let n1, n2: Int, Int = multi();    // Returns 1, 2
let n1, n2, n3: Int, Int, Int = multi();  // Returns 1, 2, 3
```

### Tuple Returns

Functions can return tuples:

```brief
defn divide(a: Int, b: Int) -> Int, Int, Bool [b != 0][true] {
    term a / b, a % b, true;
};

let quotient, remainder, ok = divide(17, 5);
```

---

## Part 12: Tips and Gotchas

### Transaction Loop Behavior

Transactions loop until the postcondition is satisfied. They continue mutating until the postcondition holds.

```brief
// This terminates - each iteration accumulates until postcondition is met
txn increment_by_2 [count < 100][count == @count + 2] {
    &count = count + 1;
    term;
};
// Starting at count=99, @count=99: 99->100->101->102 (stops at 102)
```

### The @ Operator

The `@` operator captures the value at the START of the transaction:

```brief
txn increment [count < 100][count == @count + 1] {
    &count = count + 1;
    term;
};
// @count is captured once at start. As transaction loops, @count stays the same
// but &count accumulates: 99->100->101->102...
```

### Mutations Need `&`

```brief
let count: Int = 0;

&count = count + 1;    // Correct - use &
count = count + 1;     // Wrong - & required
```

### Reactive vs Passive Transactions

```brief
// Reactive transaction - fires automatically when preconditions are met
// Return values are meaningless (no caller to receive them)
rct txn process [ready][done] {
    &done = true;
    term;
};

// Passive transaction with no return value
txn do_work [true][true] {
    // do something
    term;
};

// Passive transaction with return value
txn compute() -> Int [true][true] {
    term 42;  // Caller receives this value
};

// Lambda-style passive transaction
txn increment [count < 100][count == @count + 1];  // No body needed
```

### Guards Skip Execution

```brief
txn example [true][true] {
    [false] &never_runs = true;  // This never executes
    [true] &always_runs = true;   // This always executes
    term;
};
```

---

## Part 13: Debugging

### Type Checking

```bash
brief check program.bv
```

Shows all type errors before running.

### Proof Verification

The proof engine checks:
1. Precondition can be true (satisfiable)
2. Code reaches `term` or `escape` (termination)
3. Postcondition is satisfied (correctness)

### Common Errors

#### "Precondition not satisfiable"

```brief
// Precondition is contradictory
txn bad [x > 0 && x < 0][...] {
    term;
};
```

#### "Postcondition violation"

```brief
// Code doesn't achieve postcondition
txn bad [true][count == @count + 1] {
    &count = count;  // Doesn't change count
    term;
};
```

#### "Termination unreachable"

```brief
// No path to term
txn bad [true][false] {
    escape;  // Always escapes
};
```

---

## Next Steps

1. Try the examples in `examples/`
2. Read the language reference
3. Learn FFI for system access
4. Build your own reactive system

---

## Key Takeaways

- **State first**: Everything is about state transitions
- **Contracts**: Preconditions and postconditions on transactions
- **Reactive**: Transactions fire automatically when conditions are met
- **Atomic**: Transactions complete or roll back completely
- **Loop until satisfied**: Transactions continue until postcondition is met
- **Syntactic sugar**: `~/`, implicit `term`, lambda-style declarations
- **Multi-return**: Functions can return union types via multiple `term` statements
- **Native when possible**: Use `defn` for things Brief can do
- **FFI when needed**: Use `frgn` for system access
- **Compiler verifies**: If it compiles, the state machine is correct
