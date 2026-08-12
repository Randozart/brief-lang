# Macros and Templates in Briev — Learning Guide

**Last updated:** 2026-07-23

Briev's metaprogramming system has three tiers:

| Tier | Mechanism | Use case |
|------|-----------|----------|
| `$(Stage)` blocks | `$` intrinsics (compile-time DSL) | AST navigation, file I/O, code generation at any pipeline stage |
| `template $` | Hygienic `quote { }` substitution | Safe boilerplate reduction |
| `macro $!` | String mixin via `compile#()` | Full-power AST transformation with I/O |

The `$` intrinsic system is the **primary** mechanism. The old `template`/`macro`
system is maintained for backward compatibility but new code should use
`$(Stage)` blocks.

---

## 0. `$` Intrinsic System (`$(Stage) { }`)

Run at compile time at a specified pipeline stage.

### Stage Block Syntax

```briev
$(Parsed) {
    let imports = Tag$("import");
    when imports.IsEmpty$() {
        EmitWarning$("no imports");
    };
};
```

### Key Intrinsics

| Intrinsic | Purpose |
|-----------|---------|
| `Tag$("defn")` | Select all definitions |
| `Named$("main")` | Select item by name |
| `TypeInfo$(sel, "name")` | Extract type info from AST node |
| `ConfigGet$("rust", "templates.x")` | Read glue.toml config |
| `StrReplace$(tmpl, "{{x}}", val)` | Template substitution |
| `FileWrite$("path", content, true)` | Write file to disk |
| `EmitInfo$(msg)` | Print compile-time diagnostic |
| `Insert$(pos, nodes...)` | Insert AST nodes |
| `Delete$(sel)` | Remove AST nodes |
| `SysQuery$("cpu.cores")` | Query host hardware |

### Flow Control

| Construct | Description |
|-----------|-------------|
| `let x = expr;` | Bind value |
| `x = expr;` | Reassign value |
| `when cond { body };` | Conditional (comparison with `>`, `<`, `==`, etc.) |
| `foreach(item in selection) { body };` | Iterate over selected AST nodes |
| `"a" + x + "b"` | String concatenation (`+` works for Str, Int, Str+Int) |

### Diagnostics

```briev
EmitInfo$("found " + count + " exports");
EmitWarning$("deprecated pattern detected");
EmitError$("fatal: " + reason);   // halts compilation
```

See `docs/architecture/macro-system.md` for the full reference.

---

## 1. Templates (`$`)

### Declaration

```briev
template unless(cond: Expr, body: Block) -> Stmt {
    return quote { [@cond] { @body } };
};
```

Parameters are typed using `Expr`, `Stmt`, `Block`, `Type`, `Int`, `String`, or `Bool`.

### Call site

```briev
$unless(sensor_tripped) { activate_alarm(); };
```

The `$` sigil tells the reader: "this is a safe syntactic transformation."

### `quote { }` and `@`-interpolation

Inside `quote { }`, `@ident` is replaced with the corresponding argument's AST:

```briev
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

```briev
template declare(n: Int) -> Stmt {
    let size: Int = @{n * 2};
    return quote { let arr: List<Int> = List<Int>::new(@{size}); };
};
```

### Hygiene

Templates are **hygienic by default**. Local `let` bindings are automatically renamed to `__gensym_N` to prevent variable capture. `state`, `fn`, and `txn` names are preserved.

```briev
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

```briev
macro assert_nonzero(val: Expr) -> Stmt {
    return quote { [@val == 0] { error#("assertion failed: nonzero"); }; };
};
```

Macros accept the same parameter types as templates.

### Call site

```briev
$!assert_nonzero(x);
```

The `$!` sigil means "warning: this macro may do unexpected things."

### String mixins with `compile#()`

The `compile#()` intrinsic parses a Briev source string at compile time:

```briev
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
| `compile#(code)` | `compile#(String) -> Block` | Parse string as Briev code at compile time |
| `error#(msg)` | `error#(String)` | Halt compilation with error message |
| `warn#(msg)` | `warn#(String)` | Print warning, continue compilation |
| `gensym#()` | `gensym#() -> String` | Generate unique identifier |

Example:

```briev
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

### Lookup table generation

```briev
macro sine_lut(n: Int) -> Block {
    let code: String = "state sine_table: Array<Float> = [";
    let i: Int = 0;
    [i < @n] {
        let val: Float = sin#(2.0 * 3.14159 * i / @n);
        code = code ++ float_to_str#(val) ++ ", ";
        &i = i + 1;
    };
    code = code ++ "];";
    return compile#(code);
};
```

### Error-checking wrappers

```briev
template must_succeed(call: Expr) -> Stmt {
    return quote {
        let result = @call;
        [result == -1] { error#("syscall failed"); };
    };
};
```
