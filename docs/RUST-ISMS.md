# Rust-isms to Avoid in Briev

This document tracks patterns from Rust that accidentally leaked into Briev code but violate Briev's philosophy.

## ❌ `panic()` - Runtime Error Handling

**Status**: Found in `lib/std/option.bv` lines 50, 52 - **BEING FIXED**

### The Mistake

```briev
// WRONG - Rust thinking
defn unwrap<T>(opt: Option<T>) [opt.is_some()][true] -> T {
    uni opt(Some(v)) = { term v; };
    uni opt(None) = {
        term panic("called unwrap on None");  // ← Runtime crash!
    };
    term panic("unreachable");
};
```

### Why This Violates Briev Philosophy

1. **Contracts, Not Crashes**: Briev uses contracts to guarantee safety at compile-time, not runtime checks
2. **No `Never` Type**: Briev has no `!` (diverges) type - every function must return
3. **Lazy Error Handling**: `panic` is a shortcut that avoids proper error propagation
4. **Philosophy Violation**: "Contracts are the source of truth" - if contract says `[opt.is_some()]`, the None case is impossible

### The Correct Approaches

**Option 1: Trust the Contract** (Preferred for internal code)
```briev
defn unwrap<T>(opt: Option<T>) [opt.is_some()][result == @opt.Some_value] -> T {
    uni opt(Some(v)) = { term v; };
    // None case omitted - contract guarantees Some
    // Compiler proves this path is complete
};
```

**Option 2: Use Result** (Preferred for public APIs)
```briev
defn try_unwrap<T>(opt: Option<T>) [true][result.is_ok() || result.is_err()] -> Result<T, String> {
    uni opt(Some(v)) = { term Ok(v); };
    uni opt(None) = { term Err("unwrap on None"); };
};
```

**Option 3: Unwrap Or Default**
```briev
defn unwrap_or<T>(opt: Option<T>, default: T) [true][true] -> T {
    uni opt(Some(v)) = { term v; };
    uni opt(None) = { term default; };
};
```

### How to Fix Existing Code

1. **Remove all `panic()` calls**
2. **Strengthen preconditions** if the function should never fail
3. **Return `Result<T, E>`** if failure is possible
4. **Use `unwrap_or` / `unwrap_or_else`** for defaults

### Related Rust-isms

- ❌ `.unwrap()` without contract - Use `unwrap_or` or strengthen precondition
- ❌ `expect("message")` - Same as panic, just with a message
- ❌ `unreachable!()` macro - If truly unreachable, omit the branch
- ❌ `Option::None` in postcondition - Should be `result.is_some()` or `result.is_ok()`

---

## ❌ `[true][true]` Contracts

**Status**: Fixed in `lib/compiler/main.bv` - see commit 66cbcc6

### The Mistake

```briev
// WRONG - No contract!
defn compile_file(path: String) -> Result<String, String> [true][true]
```

### Why This Violates Briev Philosophy

- Provides zero compile-time guarantees
- Equivalent to no contract at all
- Defeats the purpose of contract-first design

### The Fix

```briev
// RIGHT - Meaningful contracts
defn compile_file(path: String) [path .#Size > 0][result.is_ok() || result.is_err()] -> Result<String, String>
```

---

## Checklist for Auditing Code

- [ ] Search for `panic(` - replace with proper error handling
- [ ] Search for `[true][true]` - replace with meaningful contracts
- [ ] Search for `.unwrap()` - add precondition or use `unwrap_or`
- [ ] Search for `unreachable` - prove unreachable or handle the case
- [ ] Check all `defn` have non-trivial postconditions
- [ ] Check all `txn` have non-trivial preconditions

# Rust-isms to Avoid in Briev

This document tracks patterns from Rust that accidentally leaked into Briev code but violate Briev's philosophy.

## ❌ `panic()` - Runtime Error Handling

**Status**: Found in `lib/std/option.bv` lines 50, 52 - **BEING FIXED**

### The Mistake

```briev
// WRONG - Rust thinking
defn unwrap<T>(opt: Option<T>) [opt.is_some()][true] -> T {
    uni opt(Some(v)) = { term v; };
    uni opt(None) = {
        term panic("called unwrap on None");  // ← Runtime crash!
    };
    term panic("unreachable");
};
```

### Why This Violates Briev Philosophy

1. **Contracts, Not Crashes**: Briev uses contracts to guarantee safety at compile-time, not runtime checks
2. **No `Never` Type**: Briev has no `!` (diverges) type - every function must return
3. **Lazy Error Handling**: `panic` is a shortcut that avoids proper error propagation
4. **Philosophy Violation**: "Contracts are the source of truth" - if contract says `[opt.is_some()]`, the None case is impossible

### The Correct Approaches

**Option 1: Trust the Contract** (Preferred for internal code)
```briev
defn unwrap<T>(opt: Option<T>) [opt.is_some()][result == @opt.Some_value] -> T {
    uni opt(Some(v)) = { term v; };
    // None case omitted - contract guarantees Some
    // Compiler proves this path is complete
};
```

**Option 2: Use Result** (Preferred for public APIs)
```briev
defn try_unwrap<T>(opt: Option<T>) [true][result.is_ok() || result.is_err()] -> Result<T, String> {
    uni opt(Some(v)) = { term Ok(v); };
    uni opt(None) = { term Err("unwrap on None"); };
};
```

**Option 3: Unwrap Or Default**
```briev
defn unwrap_or<T>(opt: Option<T>, default: T) [true][true] -> T {
    uni opt(Some(v)) = { term v; };
    uni opt(None) = { term default; };
};
```

### How to Fix Existing Code

1. **Remove all `panic()` calls**
2. **Strengthen preconditions** if the function should never fail
3. **Return `Result<T, E>`** if failure is possible
4. **Use `unwrap_or` / `unwrap_or_else`** for defaults

### Related Rust-isms

- ❌ `.unwrap()` without contract - Use `unwrap_or` or strengthen precondition
- ❌ `expect("message")` - Same as panic, just with a message
- ❌ `unreachable!()` macro - If truly unreachable, omit the branch
- ❌ `Option::None` in postcondition - Should be `result.is_some()` or `result.is_ok()`

---

## ❌ `[true][true]` Contracts

**Status**: Fixed in `lib/compiler/main.bv` - see commit 66cbcc6

### The Mistake

```briev
// WRONG - No contract!
defn compile_file(path: String) -> Result<String, String> [true][true]
```

### Why This Violates Briev Philosophy

- Provides zero compile-time guarantees
- Equivalent to no contract at all
- Defeats the purpose of contract-first design

### The Fix

```briev
// RIGHT - Meaningful contracts
defn compile_file(path: String) [path .#Size > 0][result.is_ok() || result.is_err()] -> Result<String, String>
```

---

## Checklist for Auditing Code

- [ ] Search for `panic(` - replace with proper error handling
- [ ] Search for `[true][true]` - replace with meaningful contracts
- [ ] Search for `.unwrap()` - add precondition or use `unwrap_or`
- [ ] Search for `unreachable` - prove unreachable or handle the case
- [ ] Check all `defn` have non-trivial postconditions
- [ ] Check all `txn` have non-trivial preconditions

