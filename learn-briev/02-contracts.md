# Contracts: Preconditions & Postconditions

Contracts are the heart of Briev. They're not just documentation - the compiler **verifies** them.

## 1. Preconditions `[pre]`

The precondition declares **when** a transaction can run:

```briev
node withdraw [amount > 0 && balance >= amount][balance == @balance - amount] {
    &balance = balance - amount;
    term;
};

let balance: Int = 100;
let amount: Int = 10;
```

> A zero-parameter operation that the main loop drives is a **`node`**
> (reactive). A **`txn`** is the same shape but callable — it may take
> parameters and only runs when another transaction calls it. See
> [01-basics.md](01-basics.md) §3.

**This transaction can ONLY run when:**
- `amount > 0` (can't withdraw negative or zero)
- `balance >= amount` (must have enough funds)

If the precondition is false, the transaction simply doesn't run.

### Common Precondition Patterns

**Range checks:**
```briev
[age >= 18 && age <= 65]      // Must be working age
[index >= 0 && index < len]    // Valid array index
```

**State checks:**
```briev
[!locked]                      // Must be unlocked
[status == "active"]           // Must be active
[items .^Len > 0]             // Must have items
```

**Resource checks:**
```briev
[available >= needed]          // Must have enough
[!in_progress]                 // Must not be running
```

## 2. Postconditions `[post]`

The postcondition declares **what must be true after** the transaction:

```briev
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
```briev
[counter == @counter + 1]      // Counter incremented
[items .^Len == @items .^Len + 1]  // One item added
[active == false]              // Now inactive
```

**Relationships:**
```briev
[total == sum(items)]          // Total equals sum
[sorted(items)]                // Items are sorted
[unique(items)]                // No duplicates
```

**Preservation:**
```briev
[total == @total]              // Total unchanged
[count >= @count]              // Count didn't decrease
```

## 2.5 The `[!/X]` Invert Form

A bracket that begins with `!/` **inverts the contract** — one bracket expands
to both the precondition and the postcondition:

| Form | Precondition | Postcondition |
|---|---|---|
| `[!/X]` | `!X` | `X` |
| `[!/!X]` | `X` | `!X` |

```briev
// The node fires only while the queue is NOT full, and leaves it full.
node refill [!/ queue.^Len < capacity] {
    queue <- item;
    term;
};
```

This is the successor of the old `~/` term-until token (removed) — a contract
bracket is the honest place for "until this holds", since it carries both sides
of the condition.

## 3. The `@` Prior State Operator
`@` gives you the value **before** the transaction started:

```briev

[counter == counter + 1]  // Always false!
```

**With `@`:**
```briev
[counter == @counter + 1]  // counter after = counter before + 1
```

## 4. Contract Verification

The compiler checks that your code **actually satisfies** the contract:

```briev
// ❌ This FAILS verification
txn broken_increment() [true][counter == @counter + 1] {
    &counter = counter + 2;  // Oops! Adds 2, not 1
    term;
};

// Error: Postcondition not satisfied
// Proof: counter = @counter + 2, but postcondition requires @counter + 1
```

```briev
// ✅ This PASSES verification
txn correct_increment() [true][counter == @counter + 1] {
    &counter = counter + 1;
    term;
};
```

## 5. Multiple Paths

All paths must satisfy the postcondition:

```briev
txn conditional(x: Int) [true][result >= 0] {
    let result: Int = 0;
    
    when x > 0 {
        &result = x;  // result = x > 0 ✓
    };
    
    when x <= 0 {
        &result = -x;  // result = -x >= 0 ✓
    };
    
    term;
};
```

**Both paths** must satisfy `result >= 0`.

## 6. Escape and Contracts

`escape` rolls back the transaction - the postcondition doesn't need to hold:

```briev
txn safe_divide(a: Int, b: Int) 
    [b != 0]
    [result == a / b]
{
    when b == 0 {
        escape;  // Rollback - postcondition not checked
    };
    let result = a / b;
    term;
};
```

## 7. Watchdog Timers

A watchdog is a **liveliness** contract: the loop continues while the
condition holds, and **fires** the moment it stops — or when a deadline
expires. `?[...]` is optional (the fire is graceful); `![...]` is required
(firing is an error exit).

```briev
// Optional liveliness: fires when `x` reaches 5 (the condition stops holding)
txn converge() [true][done] ?[x < 5] {
    x = x + 1;
    term;
};

// The `-> handler(val)` on-fire callback receives the LAST COMPUTED VALUE
defn report(v: Int) -> Int { println!(v); term v; };
txn converge() [true][done] ?[x < 5] -> report(x) {
    x = x + 1;
    term;
};

// The `within N <unit>` deadline fires even if the condition never stops
// holding — after N milliseconds or N loop cycles:
txn poll() [true][done] ?[ready] within 10 ms -> report(x) { ... };
txn sweep() [true][done] ?[done]   within 1000 cyc { ... };
```

Units: `ms` / `seconds` / `minute` (a time deadline via the `Now#` monotonic
clock) and `cyc` (a cycle/fuel deadline). The `within` clause comes before the
`-> handler`.

## 8. Complete Example

```briev
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

txn enable_overdraft
    [!overdraft_protection]
    [overdraft_protection == true]
{
    &overdraft_protection = true;
    term;
};

node run [balance < 10000][balance >= 10000] {
    amount = 50;
    deposit();
    amount = 30;
    withdraw();
    term;
};
```

The operations are callable `txn`s; `run` is the reactive `node` that drives
them from the main loop.

## Exercises

1. Write a `multiply` transaction with pre/post conditions
2. Add a precondition that prevents integer overflow
3. Create a `swap` transaction that exchanges two values

---

*Next: [03-reactive.md](03-reactive.md) - Reactive transactions that fire automatically*
