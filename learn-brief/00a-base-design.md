# The Brief Mindset

**How to read and think in Brief**

---

## Why This Document Exists

Most language tutorials teach you the **syntax** - what to write. This document teaches you how to **see** Brief code. It's the mental model you need to read Brief the way a Brief developer reads it, understanding intent at a glance.

If you're the kind of learner who picks up languages by "getting a feel" for them, this is for you.

---

## The Core Philosophy

Brief removes familiar imperative constructs **by design**. No `if/else`, no `while`, no `for`.

This is **not a limitation**. It's the foundation of what makes Brief work. When code can't branch unpredictably, the compiler can **prove** properties about it. No races. No unhandled cases. No deadlocks.

**Never-nesting** - Treat each block of code as an individual building block to be strung together, not nested inside other blocks.

---

## What Symbols Mean

### `[ ]` - Brackets for Logic

Square brackets are for **logical checks and verification**. This is the primary way Brief makes decisions.

```brief
[x > 0] {        // Guard: only runs if x is positive
    &result = x;
};
```

**Vector format**: `Vector<T, dim1, dim2, ...>` with commas, not angle brackets for dimensions

```brief
let items: Vector<Int, 10>;   // Type declaration - obviously not a check
let x = items[5];             // Index access
let matrix: Vector<Int, 10, 20>;    // 2D vector
```

---

### `< >` - Angle Brackets for Types

When you see angle brackets, think **Type**. This is almost always a generic or comparison.

```brief
HashMap<String, Int>;    // Generic type
Option<String>;         // Optional type
Result<Int, Error>;    // Result type
```

**Exception**: Rendered Brief's inline HTML uses `<tag>` for actual HTML elements:

```brief
<div class="card">     // HTML element in .rbv view block
    <span>Hello</span>
</div>
```

---

### `{ }` - Curly Braces for Code Blocks

Curly braces **divvy up code** - they group statements into logical units:

```brief
txn increment [true][count == @count + 1] {
    &count = count + 1;   // Block 1
    term;                  // Block 2
};
```

Unlike languages where `{` starts a new scope, in Brief each block is just organization.

---

### `( )` - Parentheses for Arguments

Parentheses are for **arguments** - parameters to functions, transactions, or calls:

```brief
txn withdraw(amount: Int) (...) { ... };
defn max(a: Int, b: Int) -> Int (...) { ... };
io.println("hello");
```

---

### `.` - The Accessor

When you see `.`, you're accessing something **inside** a struct or type:

```brief
account.balance         // Field access
map.get("key")      // Method call
result.value         // Result unwrapping
```

**What this means**: Brief tries to be transparent. If you see `something.field`, that struct exists **somewhere** in the standard library. Nothing is hidden magic.

---

### `:>` - The Metadata Lens

`:>` reads compile-time-known metadata from a value. Think of it as "ask the
compiler for a property of this thing":

```brief
list :> Size;       // How many elements?
str :> Bytes;       // How many bytes does this occupy?
&x :> Ptr;          // Get a verified pointer to x
val :> Popcount;    // How many set bits (via @llvm.ctpop)?
val :> Absolute;    // Absolute value (via @llvm.fabs)
val :> Type;        // What type is this at compile time?
x :> Ptr!;          // Raw address — dangerous, no safety envelope
```

`:>` is the "cheat code" — it lets the compiler handle operations it can
prove or optimize, rather than requiring the programmer to write them in
user-space. Every `:>` target maps to either a compile-time constant or a
zero-cost LLVM intrinsic.

See `learn-brief/13-projections.md` for the complete reference.

---

### `@` - The Prior State

`@` always means "value at the **start** of this transaction":

```brief
txn withdraw(amount: Int)
    [balance >= amount]
    [balance == @balance - amount]    // @balance = balance BEFORE this txn ran
{
    &balance = balance - amount;
    term;
};
```

Also used for memory-mapped addresses in Embedded Brief:

```brief
let led: Bool @ 0x40020000;    // Address, not prior state
```

---

### `&` - The Mutation Marker

**You must use `&` to mutate state.** This is mandatory and deliberate.

```brief
&count = count + 1;    // Correct - mutates state
count = count + 1;     // Wrong - won't compile
```

This explicit marker exists because mutations are verification-critical. The compiler needs to know exactly what's changing.

---

### `~` - Boolean Toggle

`~` is shorthand for flipping a boolean:

```brief
// These are equivalent:
[~/ready]                // Shorthand
[~ready][ready]          // Full form
// Means: "fire when ready is false, ensure ready becomes true"
```

---

### `!` - "Something Weird"

The `!` suffix signals **this does something unusual to control flow**:

```brief
frgn! log_message(msg);   // Fire-and-forget - no Result to check, runs and forgets
syscall! exit(code);      // Kernel call that never returns
trg! interrupt();        // Trigger that can fire during any function
```

When you see `!`, pause and think: "What makes this call unusual?"

---

### `?` - Watchdog / Timeout

`?` marks a **watchdog** - a timeout or external condition:

```brief
txn long_operation() [true][done] ?[5000ms] {   // Must finish in 5 seconds
    do_work();
    &done = true;
    term;
};
```

---

### `->` - Return Type

`->` always introduces a **return type**:

```brief
defn double(x: Int) -> Int [true][result == x * 2] {
    term x * 2;
};
```

---

## Keywords and Their Abbreviations

Brief has many **full forms** and **abbreviated forms**. The abbreviated ones are used by default. You can write them in all lowercase or all uppercase, but never mixed case.

| Abbrev | Full | Meaning |
|-------|------|--------|
| `txn` | `transaction` | State-changing operation |
| `rct` | `reactive` | Auto-fires when precondition met |
| `defn` | `definition` | Function |
| `frgn` | `foreign` | FFI call (returns Result) |
| `frgn!` | `foreign!` | FFI fire-and-forget |
| `syscall` | `syscall` | Kernel call |
| `let` | `let` | Mutable state |
| `const` | `const` | Constant |
| `term` | `terminate` | Successful end |
| `escape` | `escape` | Rollback |

---

## What Keywords Signify

### `txn` - A Transaction

Something **will change state**. Transactions are atomic - they either complete fully or roll back.

```brief
txn deposit(amount) [amount > 0][balance == @balance + amount] {
    &balance = balance + amount;
    term;
};
```

### `rct` - Reactive

This **fires automatically** when its precondition becomes true. No caller needed.

```brief
rct txn auto_save() [dirty && !saving][!dirty] {
    save_to_disk();
    &dirty = false;
    term;
};
```

### `defn` - A Pure Function

A calculation that doesn't mutate state (except via return).

```brief
defn absolute(x: Int) -> Int [true][result >= 0] {
    [x < 0] term -x;
    term x;
};
```

### `frgn` / `frgn!` - Foreign

Calls **outside** Brief. Requires error handling unless using `!` (fire-and-forget).

```brief
frgn sig sqrt(x: Float) -> Result<Float, MathError> from "math.dbvs";

frgn! sig log(msg: String) -> void from "io.dbvs";
```

---

## Reading Patterns

### Guards vs. If/Else

**Old thinking**: "If X, do Y. Otherwise, do Z."

**Brief thinking**: "When X is true, Y fires. When not-X is true, Z fires. Both can fire. I need to ensure my postcondition holds regardless."

```brief
// NOT if/else - these are guards, both CAN fire
[x > 0] &positive = true;
[x < 0] &negative = true;
```

### Contracts as Documentation

A transaction's contract is its **documentation**:

```brief
txn withdraw(amount)
    [amount > 0 && balance >= amount]      // When can this run?
    [balance == @balance - amount]          // What must be true after?
```

That precondition says: "You can withdraw if and only if the amount is positive AND you have enough balance." The postcondition says: "After withdrawing, your balance is exactly what it was minus the amount."

### The Postcondition Lies

If you see a weak postcondition like `[true][true]`, something is wrong:

```brief
// Suspicious - promises nothing
txn do_something [true][true] {
    // What does this actually guarantee?
};
```

A strong contract tells you what changed. A weak one is a red flag.

---

## Why `term` and `escape`, Not `return` and `break`

Brief deliberately uses different words for ending a transaction: `term` (terminate) and `escape` (rollback).

### The Reasoning

Most languages use `return` and `break` because they assume:
- Code runs once from start to finish
- You might want to exit early from a loop
- The compiler doesn't need to verify your loop will end

Brief assumes:
- Transactions can **loop** (they run until their postcondition is satisfied)
- You might need to exit early AND rollback all changes
- The compiler **must verify** termination

When a transaction says `[count < 100][count == @count + 1]`, it loops: count goes 99→100→101→102. The postcondition says count must increase by exactly 1, but the transaction only adds 1 each iteration. It loops until it hits exactly +1 from the start value.

Thus `term` means "I succeeded, here's my return value." Not "I'm done, get out."

And `escape` means "Rollback everything - pretend this never happened." Not "break early."

### Why Reactive Transactions Are Inherent Loops

`rct txn` can self-verify when to end:

```brief
rct txn fill_buffer() [buffer :> Size < 100][buffer :> Size == 100] {
    &buffer = buffer + [new_item];
    term;
};
```

This transaction will fire automatically when the buffer has fewer than 100 items. It keeps adding until the buffer has exactly 100. The **postcondition itself verifies termination** - the compiler proves the loop will end.

That's why `rct txn` is different from a `while` loop: the reactor pattern includes its own exit condition in the contract.

---

## Why Lists and Vectors, Not Regular Arrays

Brief provides **`List<T>`** and **`Vector<T, dim1, dim2, ...>`**, not traditional arrays. This is deliberate.

### Spatial Thinking, Not Sequential

Regular arrays force you to think sequentially: `array[0]`, `array[1]`, `array[2]`. This pattern is hard to parallelize - SIMD operations need to see multiple elements at once.

Lists and Vectors encourage **spatial thinking**:

```brief
let items: List<Int> = [1, 2, 3, 4];                      // List - growable
let buffer: Vector<Int, 100>;                             // 1D vector - fixed size
let matrix: Vector<Int, 10, 20>;                         // 2D matrix
let tensor: Float, 3, 32, 32>;                         // 3D tensor
let persons: Vector<Person, width:50, height:50>;           // Named dimensions
```

When Brief compiles to hardware (`.ebv` → SystemVerilog/VHDL) or uses SIMD operations, the compiler can reason about the **entire structure at once**, not just iterate sequentially.

### The Difference

- **`List<T>`** - Dynamic, growable, heap-allocated
- **`Vector<T, dims...>`** - Fixed-size, contiguous memory, multidimensional, hardware-friendly

### Vector Declaration Syntax

Vectors use **angle brackets** with commas for multiple dimensions:
- First argument is the **element type**
- Remaining arguments are **dimensions**
- Dimensions can be **anonymous** (just numbers) or **named** (`name:size`)

```brief
Vector<Int, 10, 20>                             // 2D, 10x20
Vector<Person, width:50, height:50, time:10>      // Named dimensions
```

Choosing between List and Vector forces you to think about your access pattern upfront.

---

## Why Multiple File Extensions

Brief splits into **file extensions by target** to keep syntax clean and bake in base assumptions:

| Extension | Purpose | Assumptions |
|----------|---------|-----------|
| `.bv` | Pure Brief | Universal - assumes some architecture exists |
| `.rbv` | Rendered Brief | MUST run in browser - HTML/CSS/SVG embeddable |
| `.ebv` | Embedded | Bare-metal or FPGA - memory-mapped I/O, hardware triggers |
| `.dbv` | Data Brief | Configuration data - cleaner to audit than regular Brief |
| `.dbvs` | Data Brief Schema | Schema definitions for `.dbv` |

Each extension bakes in **what the target expects**:

- `.rbv` knows it's going to the browser - so view blocks use HTML syntax
- `.ebv` knows it's going to hardware - so addresses and triggers are first-class
- `.dbv` knows it's data - so syntax is streamlined for auditability

When you compile, these assumptions are already baked in. You don't need to specify "this is for the browser" - the file extension already says it.

---

## The Feel Summary

When you see **square brackets** `[ ]` → think **verification check**

When you see **angle brackets** `< >` → think **type**

When you see **curly braces** `{ }` → think **code block**

When you see **parentheses** `( )` → think **arguments**

When you see **dot** `.` → think **struct field/method**

When you see **ampersand** `&` → think **mutation (required)**

When you see **at sign** `@` → think **prior state**

When you see **tilde** `~` → think **boolean toggle**

When you see **exclamation** `!` → think **control flow anomaly**

When you see **question mark** `?` → think **timeout/watchdog**

When you see **`txn`** → think **state change (atomic)**

When you see **`rct`** → think **auto-fire on condition (inherent loop)**

When you see **`defn`** → think **pure calculation**

When you see **`frgn`** → think **external call**

When you see **`term`** → think **succeeded, return value**

When you see **`escape`** → think **rollback everything**

When you see **`List`** → think **dynamic, spatial**

When you see **`Vector`** → think **fixed, contiguous, hardware-friendly**

When you see **`.bv`** → think **pure Brief (universal)**

When you see **`.rbv`** → think **rendered (browser)**

When you see **`.ebv`** → think **embedded (hardware)**

When you see **`.dbv`** → think **data (audit-friendly)**

---

## Next Steps

Now that you have the feel, continue to [01-basics.md](01-basics.md) to learn the syntax.

---

*Last updated: 2026-05-08  
Version: Brief v0.12.0*