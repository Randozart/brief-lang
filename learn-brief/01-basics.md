# Brief Basics

> **Operator quick reference:** Brief's operators fall into three groups
> — **Reflection** (`.^` — `s.^Len` runtime; `.^^` — `x.^^Bytes` compile-time), **Partition
> Operators** (`[]`, `@/`), and the **Transfer Operator** (`<-`). The
> **Anchor** (`@`) prefixes prior state in contracts. See `00a-base-design.md`
> for the full taxonomy.

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

## 3. Reactive Nodes and Callable Transactions

State changes happen in `node`s (reactive) and `txn`s (callable). A reactive
`node` fires whenever its precondition is met:

```brief
node increment [counter < 100][counter == @counter + 1] {
    &counter = counter + 1;
    term;
};
```

**Parts of a node:**
- `node` - Keyword (reactive: no parameters, no return value)
- `increment` - Name (no parentheses)
- `[counter < 100]` - **Precondition** (when it can run)
- `[counter == @counter + 1]` - **Postcondition** (what it guarantees)
- `&counter = counter + 1` - Mutation (note the `&`)
- `term` - Terminate successfully

A callable `txn` may take parameters and return a value, but only runs when
another transaction calls it (see §5).

**The `@` operator** refers to the value **before** the transaction:
- `@counter` = counter's value at transaction start
- `counter` = counter's current value

## 4. Scripting Mode (Top-Level Statements)

Instead of wrapping every program in a `txn`/`node`, you can write top-level
`let`/`const` bindings (or a plain `defn main()`). The flat-scripting plugin
synthesizes a one-shot opening node that runs them exactly once:

```brief
let message: String = "Hello, Brief!";
let x: Int = 42;
```

This is equivalent to:

```brief
let message: String = "Hello, Brief!";
let x: Int = 42;
let __script_done: Bool = false;
node __script_main [__script_done == false][__script_done] {
    let message: String = "Hello, Brief!";
    let x: Int = 42;
    __script_done = true;
};
```

A `defn main() -> Int { ... }` with no `entry!` is also wired to run exactly
once via the same synthesized node (it is renamed to `brief_main`).

**Rules:**
- All declarations (`let`, `const`, `struct`, `enum`, `defn`, `txn`) must come
  **before** any executable statements
- Once a top-level statement appears, no more declarations are allowed
- Statements execute in order, exactly once, then the program exits
- `__script_main` / `__script_done` are compiler-reserved — a user binding
  with either name is a compile error
- `escape` inside a top-level statement atomically rolls back all changes
- Reactive programs (with `node`/`txn`/`entry!`) are NOT wrapped

**Optimization behavior:** Scripting code goes through the same optimizer as
any other transaction. If a script is pure (no FFI calls) with all-const
inputs, the compiler may fully precompute it. Scripts with FFI calls always
emit runtime code.

**When to use scripting vs explicit transactions:**
- **Scripting**: quick scripts, one-shot initialization, simple programs
- **Explicit `txn`/`node`**: programs with loops, state machines, reactive chains,
  or multiple independent operations that need their own contracts
- **CLI subcommands**: use `entry!("<cmd>")` / `args!("--flag")` in a node's
  contract (see the entry-point tutorial)

## 5. Calling Transactions

Callable `txn`s run only when called. A reactive `node` can drive them:

```brief
node run [counter < 100][counter == 100] {
    increment();
    decrement();
    counter = counter + 1;
    term;
};

txn increment() [true][counter == @counter + 1] {
    &counter = counter + 1;
    term;
};

txn decrement() [counter > 0][counter == @counter - 1] {
    &counter = counter - 1;
    term;
};
```

`increment`/`decrement` are callable (they may take parameters and return
values); `run` is reactive and drives the program from the main loop.

Or they can be **reactive** (fire automatically when precondition is met):

```brief
node auto_increment [counter < 10][counter == @counter + 1] {
    &counter = counter + 1;
    term;
};
```

We'll cover reactive transactions in detail in [03-reactive.md](03-reactive.md).

## 6. Guards (Conditional Execution)

Instead of `if/else`, Brief uses `when` guards:

```brief
let x: Int = 5;

node process [x > 0 || x <= 0][result >= 0] {
    let result: Int = 0;

    // Guard: only executes if condition is true
    when x > 0 {
        &result = x * 2;
    };

    when x < 0 {
        &result = x * -1;
    };

    // For x == 0, result stays 0 (satisfies result >= 0)
    term;
};
```

**Key difference from if/else:**
- Multiple guards can execute (not mutually exclusive)
- Guards are evaluated in order
- No nesting required
- Postconditions must be satisfied on ALL paths

## 7. Escape (Rollback)

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

## 8. Pipe Chaining

Brief supports pipe chaining (`|>`) for chaining function calls in dataflow
order — like Unix pipes, but at the expression level:

```brief
// Instead of: g(f(x))
// Write:
x |> f() |> g()
```

The pipeline value is automatically passed as the first argument to each
function. Existing arguments follow:

```brief
x |> f(a, b)    // f(x, a, b)
```

### Dot-Skip

The dot-skip variants (`.|>`, `..|>`, `.N|>`) let a downstream step read
from an earlier position in the pipeline, not just the immediately
preceding one:

```brief
a |> f() |> g() .|> h()
// h receives f(a) — the same value g received (skip=1)

a |> f() |> g() .2|> h()
// h receives the initial a (skip=2 — skips f and g)
```

This is equivalent to `h(f(a))` or `h(a)`, skipping the intervening
results. The skip count cannot exceed the number of preceding steps —
that's a compile-time error.

### Auto-Wrap

If the target is a bare identifier, it is auto-wrapped as a function call:

```brief
x |> f           // same as: x |> f()
```

### Starting with a Function

A pipe chain can start with a function call (no initial value):

```brief
f() |> g()       // initial value is the result of f()
```

### Desugaring

Pipe chains are syntactic sugar — they desugar to flat let-bindings
before typechecking, with zero runtime overhead:

```brief
// x |> f() |> g() desugars to:
{
    let __pipe_0 = x;
    let __pipe_1 = f(__pipe_0);
    let __pipe_2 = g(__pipe_1);
    __pipe_2
}
```

## 9. Complete Example

```brief
// counter.bv
let counter: Int = 0;
const TOTAL: Int = 100;

txn increment() [counter < TOTAL][counter == @counter + 1] {
    &counter = counter + 1;
    term;
};

txn decrement() [counter > 0][counter == @counter - 1] {
    &counter = counter - 1;
    term;
};

node run [counter < TOTAL][counter == TOTAL] {
    increment();  // counter = 1
    increment();  // counter = 2
    decrement();  // counter = 1
    counter = counter + 1;
    term;
};
```

## Exercises

1. Create a `balance` variable and `deposit`/`withdraw` transactions
2. Add a precondition that prevents negative balances
3. Create a `reset` transaction that sets balance back to 0

---

*Next: [02-contracts.md](02-contracts.md) - Master preconditions and postconditions*
