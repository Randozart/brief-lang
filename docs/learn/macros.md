# Macros and Templates in Brief — Learning Guide

**Last updated:** 2026-06-18

Brief's metaprogramming system has two tiers:

| Sigil | Keyword | Hygiene | I/O | Use case |
|-------|---------|---------|-----|----------|
| `$` | `template` | Automatic (`__gensym_N`) | ❌ | Safe boilerplate reduction, wrappers |
| `$!` | `macro` | Manual (`gensym#()`) | ✅ (behind `--unsafe-macros`) | Code generation, DSLs, compile-time file I/O |

Both run **before type-checking** — expanded output is standard Brief AST by the time the type checker sees it.

---

## 1. Templates (`$`)

### Declaration

```brief
template unless(cond: Expr, body: Block) -> Stmt {
    return quote { [@cond] { @body } };
};
```

Parameters are typed using `Expr`, `Stmt`, `Block`, `Type`, `Int`, `String`, or `Bool`.

### Call site

```brief
$unless(sensor_tripped) { activate_alarm(); };
```

The `$` sigil tells the reader: "this is a safe syntactic transformation."

### `quote { }` and `@`-interpolation

Inside `quote { }`, `@ident` is replaced with the corresponding argument's AST:

```brief
template wrap_counter(name: String) -> Stmt {
    return quote {
        state @name: Int = 0;
        [ @name < 100 ];
        term;
    };
};

// $wrap_counter("count") expands to:
// state count: Int = 0;
// [ count < 100 ];
// term;
```

For computed interpolations, use `@{expr}`:

```brief
template declare(n: Int) -> Stmt {
    let size: Int = @{n * 2};
    return quote { let arr: List<Int> = List<Int>::new(@{size}); };
};
```

### Hygiene

Templates are **hygienic by default**. Local `let` bindings are automatically renamed to `__gensym_N` to prevent variable capture. `state`, `fn`, and `txn` names are preserved.

```brief
template wrap() -> Block {
    return quote {
        let temp: Int = 0;     // → __gensym_0
        temp + 1;              // → __gensym_0 + 1 (reference updated)
        state counter: Int;    // preserved — not renamed
    };
};
```

---

## 2. Macros (`$!`)

### Declaration

```brief
macro assert_nonzero(val: Expr) -> Stmt {
    return quote { [@val == 0] { error#("assertion failed: nonzero"); }; };
};
```

Macros accept the same parameter types as templates.

### Call site

```brief
$!assert_nonzero(x);
```

The `$!` sigil means "warning: this macro may do unexpected things."

### String mixins with `compile#()`

The `compile#()` intrinsic parses a Brief source string at compile time:

```brief
macro circular_buffer(name: String, size: Int) -> Block {
    [size <= 0] { error#("size must be > 0"); };
    return compile#("
        state @{name}_data: Buffer;
        state @{name}_head: Int = 0;
        [ @{name}_head < @{size} ]
    ");
};
```

`compile#()` always returns a `Block`. Use `:> 0` to extract a single expression.

---

## 3. Compile-Time Intrinsics

These intrinsics are only valid during macro/template expansion. If they survive past Phase 1b, the compiler emits an error.

| Intrinsic | Signature | Purpose |
|-----------|-----------|---------|
| `compile#(code)` | `compile#(String) -> Block` | Parse string as Brief code at compile time |
| `error#(msg)` | `error#(String)` | Halt compilation with error message |
| `warn#(msg)` | `warn#(String)` | Print warning, continue compilation |
| `gensym#()` | `gensym#() -> String` | Generate unique identifier |

Example:

```brief
macro check_positive(name: String, val: Expr) -> Stmt {
    [@val <= 0] { error#("expected positive value for " ++ @name); };
    return quote { let @name: Int = @val; };
};
```

---

## 4. Phase Architecture

```
Parser → ImportResolver → synthesizers → Desugarer
  │
  ├── Phase 1a: Template expansion (hygienic, no I/O)
  │     └── $unless(...) → [cond] { body }
  │
  ├── Phase 1b: Macro expansion (full I/O, then re-run 1a)
  │     └── $!circular_buffer("rx", 256) → state rx_data: Buffer; ...
  │
  ▼
TypeChecker → ProofEngine → analyze → simplify → codegen
```

Macros can emit template calls (they're re-expanded in a second Phase 1a pass). Templates cannot emit macro calls.

---

## 5. Compiler Flags

| Flag | Effect |
|------|--------|
| `--macro-budget <N>` | Override default 10,000 step budget |
| `--unlimited-macros` | Remove all budget limits |
| `--safe-compile` | Disable `$!macro` execution entirely |
| `--unsafe-macros` | Enable `sys#()` shell execution in macros |

---

## 6. Common Patterns

### Conditional compilation

```brief
macro if_target(name: String, body: Block) -> Block {
    let current_target: String = sys#("uname -s");
    [current_target == @name] { return @body; };
    return quote { /* no-op */ };
};
```

### Lookup table generation

```brief
macro sine_lut(n: Int) -> Block {
    let code: String = "state sine_table: Array<Float> = [";
    let i: Int = 0;
    [i < @n] {
        let val: Float = math#.sin(2.0 * 3.14159 * i / @n);
        code = code ++ float_to_str(val) ++ ", ";
        &i = i + 1;
    };
    code = code ++ "];";
    return compile#(code);
};
```

### Error-checking wrappers

```brief
template must_succeed(call: Expr) -> Stmt {
    return quote {
        let result = @call;
        [result == -1] { error#("syscall failed"); };
    };
};
```
