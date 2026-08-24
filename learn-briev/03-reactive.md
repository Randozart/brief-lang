# Reactive Nodes

Reactive nodes fire **automatically** when their precondition becomes true. The compiler proves they can terminate, then optimizes them to loop until termination.

## 1. The `node` Keyword

A `node` fires automatically when its precondition is met:

```briev
let count: Int = 0;

// Reactive (fires automatically)
node auto_count [count < 10][count == 10] {
    count = count + 1;
    term;
};
```

**How it works:**
1. Compiler verifies the goal can be satisfied (proves termination)
2. At runtime, node fires when precondition is true
3. **Loops until goal is met** (not just once!)
4. Only stops when `term` is reached and the goal holds

A `txn` is the same shape but callable — it takes parameters and only runs when called.

## 2. Termination Verification

The compiler proves that the goal state is reachable from the precondition. If not, you get a compile error:

```briev
// ERROR: goal [count == -1] is unreachable from [count >= 0]
// because count only increases
node broken [count >= 0][count == -1] {
    count = count + 1;
    term;
};
```

## 3. Multiple Nodes

Nodes form reactive chains — one node's postcondition enables another's precondition:

```briev
let data_ready: Bool = false;
let processed: Bool = false;

node load_data [data_ready == false][data_ready == true] {
    // Load data...
    data_ready = true;
    term;
};

node process_data [data_ready == true && processed == false]
    [processed == true]
{
    // Process...
    processed = true;
    term;
};
```

`load_data` fires first, setting `data_ready = true`. This enables `process_data`'s precondition, so it fires next.

## 4. When Guards Inside Nodes

Use `when` for conditional execution inside a node body:

```briev
node classify [score >= 0][done == true] {
    when score >= 90 {
        grade = "A";
    };
    when score >= 80 && score < 90 {
        grade = "B";
    };
    done = true;
    term;
};
```

For value-based branching, use exhaustive `match`:

```briev
match status {
    Ok(v) => Print#(v),
    Err(msg) => Print#(0 - 1),
};
```

## 5. Deferred Cleanup

```briev
defer {
    Cleanup#();
};
```

Deferred bodies run LIFO when the enclosing scope exits (normally or via rollback).
