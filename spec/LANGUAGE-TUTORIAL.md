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

## Appendix A: Complete Example - Shopping Cart

Here's a complete Rendered Brief application:

```brief
// shopping_cart.rbv
import std.math;

rstruct ShoppingCart {
    items: Int = 0,
    total: Float = 0.0,
    last_added: String = "";
    
    txn add_item(name: String, price: Float, quantity: Int)
        [price > 0 && quantity > 0]
        [items == @items + quantity && total == @total + (price * quantity)]
    {
        &items = items + quantity;
        &total = total + (price * quantity);
        &last_added = name;
        term;
    };
    
    txn remove_item(quantity: Int)
        [quantity > 0 && quantity <= items]
        [items == @items - quantity]
    {
        &items = items - quantity;
        term;
    };
    
    txn clear_cart() [items > 0][items == 0 && total == 0.0] {
        &items = 0;
        &total = 0.0;
        &last_added = "";
        term;
    };
    
    rct txn apply_discount() [items > 10 && total > 100.0][total < @total] {
        let discount: Float = total * 0.1;
        &total = total - discount;
        term;
    };
    
    view {
        <div class="shopping-cart">
            <h1>Shopping Cart</h1>
            
            <div class="stats">
                <p>Items: <span b-text="items"></span></p>
                <p>Total: $<span b-text="total"></span></p>
                <p b-show="last_added != ''">
                    Last added: <span b-text="last_added"></span>
                </p>
            </div>
            
            <div class="actions">
                <button b-trigger:click="add_item('Laptop', 999.99, 1)">
                    Add Laptop
                </button>
                <button b-trigger:click="add_item('Mouse', 29.99, 1)">
                    Add Mouse
                </button>
                <button b-trigger:click="add_item('Keyboard', 79.99, 1)">
                    Add Keyboard
                </button>
            </div>
            
            <div class="controls">
                <button b-trigger:click="remove_item(1)">Remove One</button>
                <button b-trigger:click="clear_cart()">Clear Cart</button>
            </div>
            
            <p b-show="items > 10 && total > 100.0" class="discount">
                Bulk discount applied! (10% off)
            </p>
        </div>
    }
}
```

**Key features demonstrated:**
- State management (`items`, `total`, `last_added`)
- Transactions with contracts (pre/post conditions)
- Reactive transaction (`apply_discount` fires automatically)
- UI bindings (`b-text`, `b-show`, `b-trigger`)
- Guard-based logic (discount only when conditions met)

---

## Appendix B: Complete Example - Embedded LED Blinker

An Embedded Brief example for FPGA/ARM:

```brief
// led_blinker.ebv
import std.time;

// Hardware configuration
let led_state: Bool = false @ 0x40020000;
let timer_value: Int @ 0x40001000;
let timer_reload: Int = 1000000;  // 1 second at 1MHz

// Hardware triggers
trg timer_interrupt: Int @ 0x40001004;

rct txn handle_timer() [timer_value == 0][led_state != @led_state] {
    // Toggle LED
    [led_state == false] {
        &led_state = true;
    };
    [led_state == true] {
        &led_state = false;
    };
    
    // Reload timer
    timer_value = timer_reload;
    
    term;
};

// Inline assembly for low-level control
txn init_hardware() [true][timer_value == timer_reload] {
    // Enable timer peripheral
    asm "ldr r0, =0x40001000; mov r1, #1; str r1, [r0]";
    
    // Set timer reload value
    asm "ldr r0, =0x40001004; ldr r1, timer_reload; str r1, [r0]";
    
    // Enable interrupts
    asm "cpsie i";
    
    term;
};
```

**Key embedded features:**
- Memory-mapped I/O (`@ address`)
- Hardware triggers (`trg`)
- Inline assembly (`asm`)
- Reactive interrupt handling
- Bit-level hardware access

---

## Appendix C: Complete Example - FFI with Python

Using Python libraries from Brief:

```brief
// python_example.bv
import std.io;

// Python FFI signatures
frgn sig py_init() -> Result<Void, String> from "python.toml";
frgn sig py_eval(code: String) -> Result<String, String> from "python.toml";
frgn sig py_finalize() -> Result<Void, String> from "python.toml";

// Wrapper functions
defn python_init() -> Bool {
    let result = py_init();
    [result.is_ok()] {
        term true;
    };
    term false;
};

defn python_run(code: String) -> String {
    let result = py_eval(code);
    [result.is_ok()] {
        term result.value;
    };
    [result.is_err()] {
        term "Error: " + result.error.message;
    };
    term "";
};

// Usage
txn main() [true][true] {
    [python_init()] {
        let code = "import math; str(math.sqrt(16))";
        let result = python_run(code);
        io.println("Python result: " + result);
        
        py_finalize();
    };
    term;
};
```

**Python FFI setup (`python.toml`):**
```toml
[ffi.python]
version = "3.9"
init = "Py_Initialize()"
finalize = "Py_Finalize()"
eval = "PyRun_SimpleString()"

[types]
String = "PyObject*"
```

---

## Appendix D: Complete Example - Data Brief Configuration

Using Data Brief for hardware configuration:

```brief
// hardware.dbvs (schema)
schema Board {
    name: String,
    cpu: CPU,
    memory: [MemoryRegion],
    peripherals: [Peripheral]
};

schema CPU {
    architecture: String,
    frequency: Int,
    flash_size: Int,
    ram_size: Int
};

schema MemoryRegion {
    name: String,
    start: Int,
    size: Int,
    type: String  // "flash", "ram", "peripheral"
};

schema Peripheral {
    name: String,
    type: String,
    address: Int,
    irq: Option<Int>
};
```

```brief
// hardware.dbv (data)
import "hardware.dbvs";

Board {
    name: "STM32F4 Discovery",
    cpu: CPU {
        architecture: "ARM Cortex-M4",
        frequency: 168000000,
        flash_size: 1048576,
        ram_size: 131072
    },
    memory: [
        MemoryRegion {
            name: "Flash",
            start: 0x08000000,
            size: 1048576,
            type: "flash"
        },
        MemoryRegion {
            name: "SRAM",
            start: 0x20000000,
            size: 131072,
            type: "ram"
        }
    ],
    peripherals: [
        Peripheral {
            name: "GPIOA",
            type: "gpio",
            address: 0x40020000,
            irq: Some(0)
        },
        Peripheral {
            name: "USART1",
            type: "uart",
            address: 0x40011000,
            irq: Some(37)
        }
    ]
};
```

```brief
// main.ebv (using the configuration)
import "hardware.dbv";

let gpioa_base: Int = hardware.peripherals[0].address;
let usart1_base: Int = hardware.peripherals[1].address;

txn init_gpio() [true][true] {
    // Enable GPIOA clock
    let rcc_ahb1enr = 0x40023830;
    asm "ldr r0, =0x40023830; ldr r1, [r0]; orr r1, r1, #1; str r1, [r0]";
    
    term;
};
```

**Validation:**
```bash
# Check schema validity
brief check hardware.dbv

# Compile with hardware config
brief compile main.ebv --target hardware.dbv
```

---

## Appendix E: Language Quick Reference

### Keywords

**Transactions:**
- `txn` - Transaction
- `rct` - Reactive
- `async` - Async
- `term` - Terminate
- `escape` - Rollback

**Definitions:**
- `defn` - Function
- `sig` - Signature
- `frgn` - Foreign
- `syscall` - Syscall

**Types:**
- `struct` - Structure
- `rstruct` - Rendered struct
- `enum` - Enumeration
- `let` - Mutable state
- `const` - Constant

**Control:**
- `import` - Import
- `from` - From path
- `as` - Alias
- `asm` - Assembly

**View (`.rbv`):**
- `view` - View block
- `b-text` - Text binding
- `b-show` - Visibility binding
- `b-trigger` - Event binding

### Operators

**Arithmetic:**
- `+` - Addition
- `-` - Subtraction/Negation
- `*` - Multiplication
- `/` - Division
- `%` - Modulo

**Comparison:**
- `==` - Equal
- `!=` - Not equal
- `<` - Less than
- `>` - Greater than
- `<=` - Less or equal
- `>=` - Greater or equal

**Logical:**
- `&&` - And
- `||` - Or
- `!` - Not

**Bitwise:**
- `&` - Bitwise and
- `|` - Bitwise or
- `^` - Bitwise xor
- `~` - Bitwise not
- `<<` - Shift left
- `>>` - Shift right

**Special:**
- `@` - Prior state / Address
- `&` - Mutation
- `.` - Field access
- `[]` - Index/slice
- `()` - Call

### Standard Library

**Math:**
```brief
math.abs(n)
math.min(a, b)
math.max(a, b)
math.sqrt(n)
math.pow(base, exp)
math.sin(n)
math.cos(n)
```

**String:**
```brief
string.len(s)
string.concat(a, b)
string.find(s, needle)
string.split(s, delim)
string.replace(s, old, new)
string.trim(s)
```

**Collections:**
```brief
list.len()
list.contains(x)
list.find(x)
list[i]
list[i..j]
map.get(key)
map.insert(key, value)
set.contains(x)
```

**IO:**
```brief
io.print(msg)
io.println(msg)
io.input()
io.read_file(path)
io.write_file(path, content)
```

---

## Appendix F: Common Patterns

### Pattern 1: State Machine

```brief
enum State {
    Idle,
    Running,
    Paused,
    Done
};

let state: State = State::Idle;

rct txn start() [state == State::Idle][state == State::Running] {
    &state = State::Running;
    term;
};

rct txn pause() [state == State::Running][state == State::Paused] {
    &state = State::Paused;
    term;
};

rct txn resume() [state == State::Paused][state == State::Running] {
    &state = State::Running;
    term;
};

rct txn finish() [state == State::Running][state == State::Done] {
    &state = State::Done;
    term;
};

rct txn reset() [state == State::Done][state == State::Idle] {
    &state = State::Idle;
    term;
};
```

### Pattern 2: Resource Pool

```brief
const POOL_SIZE: Int = 10;
let available: Int = POOL_SIZE;
let in_use: List<Int> = [];

rct txn acquire() [available > 0][available == @available - 1] {
    let id = find_free_id();
    &available = available - 1;
    &in_use = in_use + [id];
    term;
};

rct txn release(id: Int) [in_use.contains(id)][available == @available + 1] {
    let idx = in_use.find(id);
    &in_use = in_use.remove(idx);
    &available = available + 1;
    term;
};
```

### Pattern 3: Observer Pattern

```brief
let observers: List<String> = [];
let subject_value: Int = 0;

defn notify_observers() -> Bool {
    let i: Int = 0;
    [i < observers.len()] {
        notify(observers[i], subject_value);
        &i = i + 1;
    };
    term true;
};

txn subscribe(observer: String) [true][observers.contains(observer)] {
    &observers = observers + [observer];
    term;
};

txn unsubscribe(observer: String) [observers.contains(observer)][!observers.contains(observer)] {
    let idx = observers.find(observer);
    &observers = observers.remove(idx);
    term;
};

txn set_value(value: Int) [true][subject_value == value] {
    &subject_value = value;
    notify_observers();
    term;
};
```

### Pattern 4: Retry with Backoff

```brief
let attempts: Int = 0;
const MAX_ATTEMPTS: Int = 5;

rct txn try_operation() [attempts < MAX_ATTEMPTS][true] {
    let result = external_call();
    [result.is_ok()] {
        &attempts = 0;  // Reset on success
    };
    [result.is_err()] {
        &attempts = attempts + 1;
        [attempts < MAX_ATTEMPTS] {
            sleep(backoff(attempts));
        };
    };
    term;
};

defn backoff(attempt: Int) -> Int {
    term math.pow(2, attempt) * 1000;  // Exponential backoff
};
```

---

## Appendix G: Debugging Tips

### 1. Use Contracts for Debugging

```brief
txn debug_example(x: Int) [x > 0][result > 0 && result < 1000] {
    let result = compute(x);
    
    // Add intermediate contracts
    [result >= 0] {
        // Invariant: result is non-negative
    };
    
    term result;
};
```

### 2. Logging Transactions

```brief
rct txn log_state() [true][true] {
    io.println("Counter: " + String(counter));
    io.println("Active: " + String(active));
    term;
};
```

### 3. Watchdog Timers

```brief
txn long_operation() [true][done] ?[5000ms] {
    // Must complete within 5 seconds
    do_work();
    &done = true;
    term;
};
```

### 4. Assertion Checking

```brief
defn safe_div(a: Int, b: Int) -> Int {
    [b != 0] {  // Guard acts as assertion
        term a / b;
    };
    term 0;  // Fallback
};
```

---

## Appendix H: Performance Tips

### 1. Use Reactive Transactions

```brief
// Bad: Polling
rct txn check_sensor() [true][true] {
    let value = read_sensor();
    [value > threshold] {
        handle_event();
    };
    term;
};

// Good: Event-driven
rct txn handle_event() [sensor_value > threshold][handled] {
    process_event();
    &handled = true;
    term;
};
```

### 2. Minimize State Mutations

```brief
// Bad: Multiple mutations
txn bad_example() [true][true] {
    &x = x + 1;
    &y = y + 1;
    &z = z + 1;
    term;
};

// Good: Batch mutations
txn good_example() [true][true] {
    let new_x = x + 1;
    let new_y = y + 1;
    let new_z = z + 1;
    &x = new_x;
    &y = new_y;
    &z = new_z;
    term;
};
```

### 3. Use Guards Early

```brief
// Bad: Check after work
txn bad_check() [true][result != 0] {
    let result = expensive_computation();
    [result == 0] {
        escape;
    };
    term result;
};

// Good: Check before work
txn good_check() [input != 0][result != 0] {
    let result = expensive_computation();
    term result;
};
```

---

*Last updated: Brief v0.11.0 (2026-05-06)*
