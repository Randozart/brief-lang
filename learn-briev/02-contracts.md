# Contracts: Preconditions & Goals

Contracts are the heart of Briev. They're not just documentation — the compiler **verifies** them.

## 1. Preconditions `[pre]`

The precondition declares **when** a node can fire:

```briev
let balance: Int = 100;
let amount: Int = 10;

node withdraw [balance >= amount][balance >= 0] {
    balance = balance - amount;
    term;
};
```

**This node can ONLY fire when:** `balance >= amount`. If the precondition is false, the node simply doesn't fire.

### Common Precondition Patterns

**Range checks:**
```briev
[age >= 18]                    // Must be adult
[index >= 0 && index < len]    // Valid array index
```

**State checks:**
```briev
[done == false]                // Must not be completed yet
[counter < max]               // Has remaining work
```

## 2. Goals `[post]`

The goal (postcondition) declares the **termination state**:

```briev
node count_up [count < 10][count == 10] {
    count = count + 1;
    term;
};
```

The reactor fires this node repeatedly. Each firing increments `count`. When `count` reaches 10, the goal is satisfied and the node stops firing. The compiler proves this goal is reachable.

### Goal-Based Contracts

Instead of expressing "what changed" (which requires prior-state reads), express "what the final state looks like":

```briev
// Old style (removed): [counter == @counter + 1]
// New style (goal):     [counter == target]
node process [count < target][count == target] {
    count = count + 1;
    term;
};
```

## 3. Contracts on txns

Callable transactions also take contracts:

```briev
txn double_it [val >= 0][term == val * 2] -> Int {
    term val * 2;
};
```

For txns, the postcondition can reference `term` (the return value).

## 4. Contract Verification

The compiler verifies:
- **Goal reachability**: can the postcondition ever become true?
- **Contract satisfaction**: does the body satisfy the declared contracts?
- **Termination**: will the reactive loop eventually stop?

If verification fails, you get a compile error with a diagnostic explaining what couldn't be proven and why.

## 5. Trivial Contracts

A contract that asserts nothing (`[true][true]`) is rejected — it's indistinguishable from having no contract at all. If you don't need a contract on a defn, just omit it (contracts are optional on defns, mandatory on nodes/txns).

Pre-only contracts (`[pre]` with no post) are fine for defns — the post defaults to true.

## 6. Strict Mode

In `.s.bv` files (strict profile), representation fallbacks become hard errors:
- Unresolved memory lifetimes fail instead of warning
- Missing proofs are rejected

This lets you opt into full verification as your understanding deepens.
