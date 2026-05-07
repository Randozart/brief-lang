# Contracts: Preconditions & Postconditions

Contracts are the heart of Brief. They're not just documentation - the compiler **verifies** them.

## 1. Preconditions `[pre]`

The precondition declares **when** a transaction can run:

```brief
txn withdraw [amount > 0 && balance >= amount][balance == @balance - amount] {
    &balance = balance - amount;
    term;
};

let balance: Int = 100;
let amount: Int = 10;
```

**This transaction can ONLY run when:**
- `amount > 0` (can't withdraw negative or zero)
- `balance >= amount` (must have enough funds)

If the precondition is false, the transaction simply doesn't run.

### Common Precondition Patterns

**Range checks:**
```brief
[age >= 18 && age <= 65]      // Must be working age
[index >= 0 && index < len]    // Valid array index
```

**State checks:**
```brief
[!locked]                      // Must be unlocked
[status == "active"]           // Must be active
[items.len() > 0]             // Must have items
```

**Resource checks:**
```brief
[available >= needed]          // Must have enough
[!in_progress]                 // Must not be running
```

## 2. Postconditions `[post]`

The postcondition declares **what must be true after** the transaction:

```brief
txn withdraw(amount: Int) 
    [amount > 0 && balance >= amount]
    [balance == @balance - amount]  // Postcondition
{
    &balance = balance - amount;
    term;
};
```

**The compiler verifies that:**
- After the transaction, `balance` equals the old balance minus amount
- This must be true for ALL execution paths

### Common Postcondition Patterns

**State changes:**
```brief
[counter == @counter + 1]      // Counter incremented
[items.len() == @items.len() + 1]  // One item added
[active == false]              // Now inactive
```

**Relationships:**
```brief
[total == sum(items)]          // Total equals sum
[sorted(items)]                // Items are sorted
[unique(items)]                // No duplicates
```

**Preservation:**
```brief
[total == @total]              // Total unchanged
[count >= @count]              // Count didn't decrease
```

## 3. The `@` Prior State Operator

`@` gives you the value **before** the transaction started:

```brief
txn increment() [true][counter == @counter + 1] {
    &counter = counter + 1;
    term;
};
```

**Without `@`:**
```brief
[counter == counter + 1]  // Always false!
```

**With `@`:**
```brief
[counter == @counter + 1]  // counter after = counter before + 1
```

## 4. Contract Verification

The compiler checks that your code **actually satisfies** the contract:

```brief
// ❌ This FAILS verification
txn broken_increment() [true][counter == @counter + 1] {
    &counter = counter + 2;  // Oops! Adds 2, not 1
    term;
};

// Error: Postcondition not satisfied
// Proof: counter = @counter + 2, but postcondition requires @counter + 1
```

```brief
// ✅ This PASSES verification
txn correct_increment() [true][counter == @counter + 1] {
    &counter = counter + 1;
    term;
};
```

## 5. Multiple Paths

All paths must satisfy the postcondition:

```brief
txn conditional(x: Int) [true][result >= 0] {
    let result: Int = 0;
    
    [x > 0] {
        &result = x;  // result = x > 0 ✓
    };
    
    [x <= 0] {
        &result = -x;  // result = -x >= 0 ✓
    };
    
    term;
};
```

**Both paths** must satisfy `result >= 0`.

## 6. Escape and Contracts

`escape` rolls back the transaction - the postcondition doesn't need to hold:

```brief
txn safe_divide(a: Int, b: Int) 
    [b != 0]
    [result == a / b]
{
    [b == 0] {
        escape;  // Rollback - postcondition not checked
    };
    let result = a / b;
    term;
};
```

## 7. Watchdog Timers

Optional or required timeouts:

```brief
// Optional timeout (warns if exceeded)
txn slow_operation() [true][done] ?[5000ms] {
    do_work();
    &done = true;
    term;
};

// Required timeout (error if exceeded)
txn critical_operation() [true][done] ![1000ms] {
    do_critical_work();
    &done = true;
    term;
};
```

## 8. Complete Example

```brief
// bank_account.bv
let balance: Int = 1000;
let overdraft_protection: Bool = true;
let amount: Int = 0;

txn deposit
    [amount > 0]
    [balance == @balance + amount]
{
    &balance = balance + amount;
    term;
};

txn withdraw
    [amount > 0 && (balance >= amount || overdraft_protection)]
    [balance == @balance - amount]
{
    &balance = balance - amount;
    term;
};

txn transfer
    [amount > 0 && balance >= amount]
    [balance == @balance - amount]
{
    &balance = balance - amount;
    term;
};

txn enable_overdraft
    [!overdraft_protection]
    [overdraft_protection == true]
{
    &overdraft_protection = true;
    term;
};
```

## Exercises

1. Write a `multiply` transaction with pre/post conditions
2. Add a precondition that prevents integer overflow
3. Create a `swap` transaction that exchanges two values

---

*Next: [03-reactive.md](03-reactive.md) - Reactive transactions that fire automatically*
