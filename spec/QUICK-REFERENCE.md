# Brief Quick Reference

**Cheat sheet for Brief syntax and patterns.**

## Basics

```brief
# State
let name: Type = value;
const MAX: Int = 100;

# Passive transaction
txn name [pre][post] { body };

# Reactive transaction
rct txn name [pre][post] { body };

# Function
defn name(params) -> Type [pre][post] { body };

# FFI
frgn name(params) -> Result<T, E> from "file.toml";
```

## Types

```brief
Int, Float, String, Bool, Void, Data
Type1 | Type2                    # Union
```

## Statements

```brief
&var = expr;                     # Mutate state
let local: Type = expr;          # Local var
[condition] statement;           # Guard
Pattern(x, y) = expr;           # Unification
term;                           # Success
term value;                     # Return
escape;                         # Rollback
escape value;                   # Return error
```

## Operators

```brief
a + b, a - b, a * b, a / b     # Arithmetic
a == b, a != b                 # Equality
a < b, a <= b, a > b, a >= b   # Comparison
a && b, a || b, !a             # Logic
```

## Contracts

```brief
txn name [precondition][postcondition] { body }

# Prior state
[balance == @balance - amount]

# Syntactic sugar
[~/flag]  →  [~flag][flag]
```

## Common Patterns

### Counter

```brief
let count: Int = 0;

txn increment [count < 100][count == @count + 1] {
  &count = count + 1;
  term;
};
```

### Reactive Loop

```brief
let done: Bool = false;

rct txn process [!done][done] {
  # Do work
  &done = true;
  term;
};
```

### State Machine

```brief
let state: Int = 0;

rct txn step [state == 0][state == 1] {
  &state = 1;
  term;
};
```

### FFI Call

```brief
frgn read_file(path: String) -> Result<String, Error> from "std::io";

defn load(path: String) -> String [true][true] {
  let content: String = read_file(path);
  content;
};
```

### Error Handling

```brief
sig compute: Int -> Int | Error;

txn process [true][true] {
  let result = compute(42);
  
  Int(x) = result;
  &value = x;
  term;
  
  Error(e) = result;
  escape;
};
```

## Native Stdlib

```brief
import std.core;

# Integer Math
absolute(x)                              # |x|
min(a, b)                               # a if a <= b, else b
max(a, b)                               # a if a >= b, else b
clamp(x, min_val, max_val)              # x clamped to [min, max]

# Integer Predicates
is_positive(x)                          # x > 0
is_negative(x)                          # x < 0
is_zero(x)                              # x == 0
is_even(x)                              # x % 2 == 0

# Float Predicates
float_is_positive(x)                    # x > 0.0
float_is_negative(x)                    # x < 0.0
float_is_zero(x)                        # x == 0.0

# Control & Logic
choose_if(condition, true_val, false_val)  # Conditional expression
always_true()                            # Always returns true
always_false()                           # Always returns false
not_equal(a, b)                          # a != b

# State Patterns
get_or_init_with_default(init, default)    # Returns default if not initialized
is_valid_state(state, min, max)             # true if min <= state <= max

# String Utilities
string_is_empty(s)                      # true if s == ""
```

## FFI Modules

### I/O
```brief
read_file(path: String) -> Result<String, IoError>
write_file(path: String, content: String) -> Result<Void, IoError>
file_exists(path: String) -> Result<Bool, IoError>
```

### Math
```brief
sqrt(x: Float) -> Result<Float, MathError>
sin(x: Float) -> Result<Float, MathError>
pow(base: Float, exp: Float) -> Result<Float, MathError>
```

### String
```brief
string_length(s: String) -> Result<Int, StringError>
string_to_upper(s: String) -> Result<String, StringError>
parse_int(s: String) -> Result<Int, ParseError>
```

### Time
```brief
current_timestamp() -> Result<Int, TimeError>
sleep_ms(ms: Int) -> Result<Void, TimeError>
```

See [FFI-GUIDE.md](FFI-GUIDE.md) for FFI details.

## CLI

```bash
brief check program.bv       # Type check & verify
brief build program.bv       # Run
brief init project           # Create project
brief lsp                    # Language server
```

## Syntax Rules

- Statements end with `;`
- Transactions/functions end with `;`
- No semicolon after `}`
- Precondition is `[expr]`
- Postcondition is `[expr]`
- `@variable` is prior state
- `&variable` is mutation
- Comments: `# comment`

## What Compiles

```brief
# ✓ Works
txn work [true][x == 42] {
  &x = 42;
  term;
};

# ✗ Fails: postcondition unsatisfiable
txn fail [true][x == 42] {
  &x = 41;
  term;
};

# ✗ Fails: Error not handled
txn fail [true][true] {
  let r = fetch(1);
  Int(x) = r;        # Missing: Error(e) = r;
  term;
};

# ✗ Fails: Missing mutation
txn fail [true][count == 1] {
  term;              # count never changed
};
```

## Philosophy

- **Precondition**: When can this run?
- **Postcondition**: What must be true after?
- **Compiler proves**: Pre → Post always holds
- **Rollback**: If post fails, undo all mutations
- **Reactive**: Transactions fire when conditions become true
- **Native**: Use `std.core` for proven functions
- **FFI**: Use `std.bindings` for I/O and Rust operations

---

See [LANGUAGE-REFERENCE.md](LANGUAGE-REFERENCE.md) for full details.
