# Welcome to Brief! 🎉

**Brief** is a declarative, contract-enforced logic language designed for building verifiable state machines.

## What Makes Brief Different?

### 1. Contracts First
Every transaction declares what must be true **before** and **after** it runs:

```brief
txn withdraw(amount: Int) 
    [amount > 0 && balance >= amount]  // Precondition
    [balance == @balance - amount]      // Postcondition
{
    &balance = balance - amount;
    term;
};
```

The compiler **verifies** that your code actually satisfies these contracts.

### 2. Reactive by Default
Transactions fire automatically when their preconditions are met:

```brief
rct txn auto_save() [dirty && !saving][!dirty] {
    save_to_disk();
    &dirty = false;
    term;
};
```

No event handlers. No polling. Just logic.

### 3. Zero-Nesting Logic
No `if/else` chains. Use guards instead:

```brief
// Instead of: if x > 0 { ... } else if x < 0 { ... }
[x > 0] {
    &result = x * 2;
};
[x < 0] {
    &result = x * -1;
};
```

### 4. Compile-Time Verification
The compiler proves:
- ✅ No race conditions
- ✅ No unintended side effects
- ✅ All contracts are satisfied
- ✅ No deadlocks in async code

## Quick Start

### Your First Program

Create `hello.bv`:

```brief
let message: String = "Hello, Brief!";

println(message);     // scripting — no transaction wrapper needed
```

Brief lets you write statements directly at global scope. The compiler
automatically wraps them in a synthesized `rct txn __init` that fires
once on start. No boilerplate needed.

Run it:

```bash
brief check hello.bv
```

For more complex programs, you can still write explicit transactions
(see [01-basics.md](01-basics.md)). Scripting mode is syntactic sugar
for simple programs — all the same safety guarantees apply.

### Learning Path

This folder contains a complete Brief tutorial:

1. **01-basics.md** - Variables, types, transactions
2. **02-contracts.md** - Preconditions, postconditions, @ prior state
3. **03-reactive.md** - Reactive transactions, auto-firing logic
4. **04-functions.md** - Functions with contracts
5. **05-data-types.md** - HashMap, HashSet, Stack, Queue
6. **06-string.md** - String manipulation, StringBuilder
7. **07-ffi.md** - Foreign function interface
8. **08-examples.md** - Complete examples
9. **09-patterns.md** - Common patterns
10. **10-best-practices.md** - Best practices
11. **11-triggers.md** - Triggers and events
12. **12-pragmas.md** - Pragmas and directives
13. **13-projections.md** - Projections (`:>` and `<:` operators)

## Next Steps

Start with [01-basics.md](01-basics.md) to learn the fundamentals!

---

*Last updated: 2026-06-18*  
*Version: Brief v0.16.0*
