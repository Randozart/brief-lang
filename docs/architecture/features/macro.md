# Macro/Template System (`$` / `$!`)

**Date added:** 2026-06-18  
**Phase:** 1 (complete)

## Purpose

Two-tier metaprogramming system for compile-time code generation. 
`$template` provides safe, hygienic AST substitution. `$!macro` provides 
full-power, unhygienic-capable AST transformation with I/O access.

## Syntax

### Declaration (no sigil — sigil is calling convention only)

```brief
template unless(cond: Expr, body: Block) -> Stmt {
    return quote { [@cond] { @body } };
};

macro circular_buffer(name: String, size: Int) -> Block {
    [size <= 0] { error#("size must be > 0"); };
    return compile#("state @{name}_head: Int = 0; [ @{name}_head < @{size} ]");
};
```

### Call Sites

```brief
$unless(sensor_tripped) { keep_moving(); };    // template (safe, hygienic)
$!circular_buffer("rx", 256);                   // macro (high-power)
```

### `quote { }` — Block Form (AST quasiquoting)

`@`-interpolation markers produce `Expr::Interpolate(String)` / `Expr::InterpolateExpr(Box<Expr>)`.

```brief
quote { [@cond] { @body } }
quote { state @{name}_head: Int = 0; }
```

### `compile#()` — String Form (string mixin)

Parses a Brief source string at compile time. Always returns `Value::Block`.

```brief
compile#("state @{name}_head: Int = 0;")
```

### Compile-Time Intrinsics

| Intrinsic | Syntax | Purpose |
|-----------|--------|---------|
| `compile#(code)` | `compile#("...")` | Parse string → Block |
| `error#(msg)` | `error#("bad arg")` | Emit compiler error |
| `warn#(msg)` | `warn#("deprecated")` | Emit compiler warning |
| `gensym#()` | `gensym#()` | Generate unique identifier |

## Phase Architecture

```
Parser → ImportResolver → synthesizers → Desugarer
  │
  ├── Phase 1a: Template expansion (hygienic, no I/O)
  │   ├── Collect TemplateDef into registry
  │   ├── Expand TemplateCall nodes via @-substitution
  │   └── Hygiene: local let → __gensym_N; state/fn names preserved
  │
  ├── Phase 1b: Macro expansion (full power, I/O allowed)
  │   ├── Collect MacroDef into registry
  │   ├── Execute macro bodies in sandboxed Interpreter
  │   ├── Re-run Phase 1a on macro output
  │   └── validate_no_compile_time_intrinsics() check
  │
  ▼
TypeChecker → ProofEngine → analyze → simplify → codegen
```

## Hygiene Rules

| Construct | Template | Macro |
|-----------|----------|-------|
| `let x = ...` | ✅ `__gensym_N` | Manual via `gensym#()` |
| `state count: Int` | ❌ name preserved | ❌ name preserved |
| `fn / txn` names | ❌ name preserved | ❌ name preserved |

## AST Additions

| Variant | Type | Purpose |
|---------|------|---------|
| `Expr::TemplateCall` | Expr | `$name(args) { block }` |
| `Expr::MacroCall` | Expr | `$!name(args) { block }` |
| `Expr::Interpolate(String)` | Expr | `@ident` in quote blocks |
| `Expr::InterpolateExpr(Box<Expr>)` | Expr | `@{expr}` in quote blocks |
| `Expr::QuoteBlock` | Expr | `quote { ... }` block |
| `TopLevel::TemplateDef` | TopLevel | `template name(...) { ... }` |
| `TopLevel::MacroDef` | TopLevel | `macro name(...) { ... }` |
| `Value::Expr` | Value | Compile-time Expr value |
| `Value::Stmt` | Value | Compile-time Stmt value |
| `Value::Block` | Value | Compile-time block |
| `Value::Type` | Value | Compile-time type |

## Feature Files (`src/features/macros/`)

| File | Responsibility |
|------|---------------|
| `context.rs` | MacroContext, TemplateDef, MacroDef, gensym counter |
| `expand.rs` | Phase 1a/1b AST walker + validate pass |
| `template.rs` | @-interpolation substitution, macro body execution |
| `hygiene.rs` | Gensym-based let-renaming (state/fn names exempt) |
| `macro_.rs` | Macro expansion stubs (reserved for future use) |

## Compiler Flags

| Flag | Effect |
|------|--------|
| `--macro-budget <N>` | Override default 10,000 step budget |
| `--unlimited-macros` | Set budget to u64::MAX |
| `--safe-compile` | Disable `$!macro` execution entirely |

## Kani

`MacroContext` budget tracking and `hygiene.rs` gensym counter must have proof harnesses.

## Praetor

Each file in `src/features/macros/` must pass complexity ≤ 15, lines ≤ 100, params ≤ 6.
