# Brief Basics

## 1. State Declarations

All state in Brief is declared globally with `let`:

```brief
let counter: Int = 0;
let name: String = "Alice";
let active: Bool = true;
let score: Float = 95.5;
let initial: Char = 'A';
```

**Type Inference:**
```brief
let x = 42;          // Inferred: Int
let s = "hello";     // Inferred: String
let b = true;        // Inferred: Bool
```

**Without Initial Value:**
```brief
let counter: Int;    // Defaults to 0
let name: String;    // Defaults to ""
let active: Bool;    // Defaults to false
```

## 2. Constants

Immutable values with `const`:

```brief
const MAX_SIZE: Int = 100;
const VERSION: String = "1.0.0";
const PI: Float = 3.14159;
```

## 3. Transactions

Transactions are how state changes in Brief:

```brief
txn increment() [counter < 100][counter == @counter + 1] {
    &counter = counter + 1;
    term;
};
```

**Parts of a transaction:**
- `txn` - Keyword
- `increment` - Name
- `[counter < 100]` - **Precondition** (when it can run)
- `[counter == @counter + 1]` - **Postcondition** (what it guarantees)
- `&counter = counter + 1` - Mutation (note the `&`)
- `term` - Terminate successfully

**The `@` operator** refers to the value **before** the transaction:
- `@counter` = counter's value at transaction start
- `counter` = counter's current value

## 4. Calling Transactions

Transactions can be called explicitly:

```brief
txn main() [true][true] {
    increment();  // Call the transaction
    increment();
    term;
};
```

Or they can be **reactive** (fire automatically when precondition is met):

```brief
rct txn auto_increment() [counter < 10][counter == @counter + 1] {
    &counter = counter + 1;
    term;
};
```

We'll cover reactive transactions in detail in [03-reactive.md](03-reactive.md).

## 5. Guards (Conditional Execution)

Instead of `if/else`, Brief uses guards:

```brief
txn process(x: Int) [true][result != 0] {
    let result: Int = 0;
    
    // Guard: only executes if condition is true
    [x > 0] {
        &result = x * 2;
    };
    
    [x < 0] {
        &result = x * -1;
    };
    
    [x == 0] {
        escape;  // Rollback
    };
    
    term;
};
```

**Key difference from if/else:**
- Multiple guards can execute (not mutually exclusive)
- Guards are evaluated in order
- No nesting required

## 6. Escape (Rollback)

Use `escape` to rollback a transaction:

```brief
txn validate(x: Int) [x >= 0][state == @state] {
    [x > 1000] {
        escape;  // Rollback - nothing changes
    };
    &state = x;
    term;
};
```

## 7. Complete Example

```brief
// counter.bv
let counter: Int = 0;

txn increment() [counter < 100][counter == @counter + 1] {
    &counter = counter + 1;
    term;
};

txn decrement() [counter > 0][counter == @counter - 1] {
    &counter = counter - 1;
    term;
};

txn reset() [true][counter == 0] {
    &counter = 0;
    term;
};

txn main() [true][true] {
    increment();  // counter = 1
    increment();  // counter = 2
    decrement();  // counter = 1
    term;
};
```

## Exercises

1. Create a `balance` variable and `deposit`/`withdraw` transactions
2. Add a precondition that prevents negative balances
3. Create a `reset` transaction that sets balance back to 0

---

*Next: [02-contracts.md](02-contracts.md) - Master preconditions and postconditions*
