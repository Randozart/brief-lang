# Simple Counter

A Briev program that tracks a number you can increment, decrement, and reset. Demonstrates state variables and contracts.

## Run It

From the `briev-compiler` directory:

```bash
briev run examples/simple-counter/src/main.bv
```

## What It Does

This program manages a single number (`count`). You can:

1. **Increment** - Add 1 to the count (but not above 100)
2. **Decrement** - Subtract 1 from the count (but not below 0)
3. **Reset** - Set the count back to 0

## The Code Explained

### State Variable

```briev
let count: Int = 0;
```

This creates a variable called `count` that holds an integer, starting at 0. Unlike local variables inside transactions, this persists between transactions.

### Increment Transaction

```briev
txn increment [count < 100][count == @count + 1] {
    &count = count + 1;
    term;
};
```

**Precondition**: `count < 100`
- The increment won't run if count is already 100 or higher
- This prevents the counter from going above 100

**Postcondition**: `count == @count + 1`
- After incrementing, the new count should equal the old count plus 1
- The `@` symbol means "the previous value of count"

**Body**: `&count = count + 1;`
- `&` means "modify the state variable"
- Without it, the calculation would happen but the result wouldn't be saved

### Decrement Transaction

```briev
txn decrement [count > 0][count == @count - 1] {
    &count = count - 1;
    term;
};
```

Same pattern as increment, but:
- **Precondition**: `count > 0` - Can't go below 0
- **Postcondition**: `count == @count - 1` - Decreases by 1

### Reset Transaction

```briev
txn reset [count != 0][count == 0] {
    &count = 0;
    term;
};
```

- **Precondition**: `count != 0` - Can't reset if already at 0
- **Postcondition**: `count == 0` - Must end at 0

## Key Concepts

### The `&` Symbol

Use `&` before a variable name to modify a state variable:

```briev
&count = count + 1;  // Modifies count
count + 1;           // Just a calculation, result is discarded
```

### The `@` Symbol

Use `@` before a variable name to refer to its previous value:

```briev
// In the postcondition:
count == @count + 1  // "new count equals old count plus 1"
```

### Contracts Catch Bugs

Try removing the precondition from increment:

```briev
txn increment [true][count == @count + 1] {  // BAD!
    &count = count + 1;
    term;
};
```

Briev will warn you:

```
error[P009]: trivial precondition
```

Briev noticed that `[true]` has no requirements, which is suspicious for an increment operation that could exceed 100.

## Next Steps

After understanding this example:

1. **Add a `set` transaction** - Set count to any value (with contract checking bounds)
2. **Learn about structs** - See `examples/counter.rbv` for a counter with a visual display
3. **Read the Getting Started guide** - `spec/GETTING-STARTED.md` for comprehensive coverage
