# Macro System: `$template` and `$!macro`

**Date:** 2026-06-18  
**Status:** Planned  
**Context:** Design session with lead developer. Full discussion in session transcript.

---

## Design Summary

Two-tier metaprogramming system using Briv's existing `!`-means-"pay attention" convention:

| Sigil | Keyword | Hygiene | I/O | Introspection | Budget |
|-------|---------|---------|-----|---------------|--------|
| `$` | `template` | Automatic (`__gensym_N`) | ❌ | Simple substitution only | Default cap, flag-overridable |
| `$!` | `macro` | Manual (`$gensym()`) | ✅ | Full AST pattern matching | Default cap, flag-overridable |

Phase-locked: both expand **before type checking** (Phase 1), in two sub-phases.

---

## 1. Syntax

### Declaration (no sigil — sigil is calling convention only)

```briv
template unless(cond: Expr, body: Block) -> Stmt {
    return quote { [@cond] { @body } };
};

macro circular_buffer(name: String, size: Int) -> Block {
    [size <= 0] { $error("size must be > 0"); };

    let code: String = "
        state @{name}_data: Buffer;
        state @{name}_head: Int = 0;
        [ @{name}_head < @{size} ]
    ";
    return compile#(code);
};
```

### Call Sites

```briv
// $ sigil for templates — safe, hygienic
$unless(sensor_tripped) { keep_moving(); };

// $! sigil for macros — high power, pay attention
$!circular_buffer("rx", 256);
```

### `quote { }` — Block Form (AST construction)

The parser enters a special parsing mode inside `quote { }`. It parses standard Briv syntax, but `@ident` and `@{expr}` produce `Expr::Interpolate(String)` / `Expr::InterpolateExpr(Box<Expr>)` marker nodes.

Expansion substitutes marker nodes with the corresponding argument AST nodes (capture-avoiding, hygienic for templates).

```briv
// interpolate a typed AST argument (@cond is Expr, @body is Block)
quote { [@cond] { @body } }

// interpolate a computed compile-time value (@{expr}) — @ in any token position
quote { state @{name}_head: Int = 0; }
quote { let x: @type = @init; }
```

### `compile#()` — String Form (string mixin)

An intrinsic function that takes a Briv source string and returns parsed AST. `@{expr}` markers are substituted before parsing.

```briv
compile#("state @{name}_head: Int = 0; [ @{name}_head < @{size} ]")
```

**Always returns `Block`**: `compile#()` always wraps its output in a `Block` — the most general container. A single expression like `compile#("@x + 1")` becomes `{ @x + 1 }`. If the caller needs an `Expr` or `Stmt`, they extract it via projection or index.

### `@` Interpolation

The anchor sigil `@` signals "this is a placeholder that will be filled at expansion time." Two forms:

| Form | Where | Expands to |
|------|-------|------------|
| `@ident` | `quote { }` body | AST variable `ident` (typed `Expr`, `Stmt`, `Block`, `Type`) |
| `@{expr}` | `quote { }` or `compile#()` | Evaluated compile-time expression, stringified or inlined as AST |

---

## 2. AST Additions

### New `Expr` Variants (`src/ast.rs`)

```rust
enum Expr {
    // --- existing variants ---

    /// $name(args) or $name(args) { block } — template/macro call
    TemplateCall {
        name: String,
        args: Vec<Expr>,
        block: Option<Block>,        // trailing block argument
    },
    MacroCall {
        name: String,
        args: Vec<Expr>,
        block: Option<Block>,
    },

    /// @ident or @{expr} inside quote { }
    Interpolate(String),               // @ident
    InterpolateExpr(Box<Expr>),        // @{expr}

    /// quote { body } — AST quasiquoting
    QuoteBlock {
        body: Vec<Statement>,
        interpolations: Vec<(usize, InterpolateKind)>,  // marker positions
    },
}
```

### New `TopLevel` Variants

```rust
enum TopLevel {
    // --- existing variants ---

    TemplateDef {
        name: String,
        params: Vec<(String, MacroArgType)>,  // Expr, Stmt, Block, Type, Int, String, Bool
        return_type: Option<MacroArgType>,
        body: Vec<Statement>,
    },
    MacroDef {
        name: String,
        params: Vec<(String, MacroArgType)>,
        return_type: Option<MacroArgType>,
        body: Vec<Statement>,
    },
}

enum MacroArgType {
    Expr, Stmt, Block, Type,
    Int, String, Bool,
}
```

### New Interpreter Values

```rust
enum Value {
    // --- existing variants ---

    /// Compile-time AST tokens for macro return values
    Expr(Box<Expr>),
    Stmt(Box<Statement>),
    Block(Vec<Statement>),
    Type(crate::typing::Type),
}
```

---

## 3. Phase Architecture

### Updated Pipeline

```
Source text
  │
  ▼
Lexer → Parser
  │       Creates TemplateCall/MacroCall nodes, stores TemplateDef/MacroDef
  ▼
ImportResolver
  │       Populates template/macro registry from imports
  ▼
synthesize_types() + synthesize_init_txn()
  │
  ▼
Desugarer
  │
  ├──────────────────────────────────┐
  │  PHASE 1a: Template Expansion    │  ← NEW
  │  ─────────────────────           │
  │  Walk AST for TemplateCall nodes │
  │  Resolve to stored TemplateDef   │
  │  Execute template body in        │
  │    sandboxed interpreter context │
  │  Substitite @ markers with args  │
  │  Output: expanded AST nodes      │
  │  Applied recursively until fix   │
  ├──────────────────────────────────┤
  │  PHASE 1b: Macro Expansion       │  ← NEW
  │  ────────────────────            │
  │  Walk AST for MacroCall nodes    │
  │  Resolve to stored MacroDef      │
  │  Execute macro body (full power) │
  │  Re-expand Phase 1a on output    │
  │  Output: expanded AST nodes      │
  ├──────────────────────────────────┤
  │  (One pass: 1a→1b, then done)    │
  │  No re-entrant type checking     │
  └──────────────────────────────────┘
  │
  ▼
TypeChecker — all macros already expanded
  │
  ▼
ProofEngine → analyze → simplify → codegen
```

### Phase 1a Details (Templates)

- **Budget**: `--macro-budget <N>` (default 10,000 steps) or `--unlimited-macros`
- **Context**: `MacroContext { interpreter, gensym_counter, budget, call_site_span }`
- **Hygiene**: Any `Ident` node the template creates that wasn't in its input args gets auto-prefixed `__gensym_N`. This prevents variable capture.
- **I/O**: Disabled. No `read_file#`, `print#`, etc. Templates are pure AST transformations.
- **Recursion**: Static recursion check at definition time. Self-calls and mutual recursion detected → compile error. Templates have no data to consume, so recursion is always an infinite loop by construction.
- **Fixpoint**: Run until no new `TemplateCall` nodes appear (max 5 iterations to prevent infinite loops).

### Phase 1b Details (Macros)

- **Budget**: Same as 1a (default 10,000, overridable)
- **Context**: `MacroContext` plus I/O access and `$gensym()` intrinsic
- **Intrinsics available to macro bodies**:
  - `$error(msg: String)` — emit compiler error at call site
  - `$warn(msg: String)` — emit compiler warning at call site
  - `$gensym() -> String` — guaranteed-unique identifier
  - `compile#(code: String) -> Block | Stmt | Expr` — string mixin path
  - `sys#(command: String) -> String` — shell execution (behind `--unsafe-macros` flag)
- **Re-expansion**: After Phase 1b, walk the output for new `TemplateCall` nodes and re-run Phase 1a. Macros can emit template calls; templates cannot emit macro calls (enforced by context).

### Resolution Order

Both templates and macros resolve **lexically** (at definition site), not by scope (at expansion site). A template/macro can only refer to other templates/macros defined earlier in the same file or imported via `import`. This avoids circular expansion issues.

---

## 4. Feature Directory Structure

```
src/features/macro/
  mod.rs          — Module root, re-exports
  context.rs      — MacroContext (shared state machine)
  expand.rs       — Phase 1a/1b driver: walk AST, dispatch, substitute
  template.rs     — Template expansion logic
  macro_.rs       — Macro expansion logic
  quote.rs        — Quote block desugaring + interpolation substitution
  hygiene.rs      — Gensym counter, capture-avoiding substitution
  intrinisics.rs  — $error, $warn, $gensym, compile#() implementations
```

### Integration Points

| File | Change |
|------|--------|
| `src/lexer.rs` | New tokens `Dollar`, `DollarBang`, `At` (for `@interpolate`) |
| `src/ast.rs` | New `Expr`, `TopLevel`, `Value` variants (listed above) |
| `src/parser.rs` | `parse_template_call()`, `parse_macro_call()`, `parse_quote_block()`, `parse_template_def()`, `parse_macro_def()` |
| `src/parser.rs` | `parse_top_level()` and `parse_expr()` dispatch to new parsers |
| `src/backend/mod.rs` | `expand_templates(ast)` / `expand_macros(ast)` calls in pipeline |
| `src/backend/llvm/mod.rs` | Handle new `Expr` variants in codegen (can stub as `todo!()` initially — all macros expanded before LLVM sees them) |
| `src/interpreter.rs` | `Value::Expr` / `Value::Stmt` / `Value::Block` / `Value::Type` eval support |
| `src/features/traits.rs` | Register `MacroContext` if needed for dispatch |
| `docs/architecture/channel-map.md` | Update pipeline diagram |

---

## 5. Hygiene

### Template Hygiene (Automatic)

`quote { }` in a template body tracks which `Ident` nodes were part of the input args vs. generated by the template:

```briv
template wrap_counter(name: String) -> Stmt {
    return quote {
        state @name: Int = 0;     // @name is from input → not prefixed
        let temp: Int = 0;        // "temp" is template-generated → __gensym_1
        [ @name < 100 ]           // @name is from input → not prefixed
    };
};
```

Expansion of `$wrap_counter("count")` produces:
```briv
state count: Int = 0;
let __gensym_1: Int = 0;
[ count < 100 ]
```

### Macro Hygiene (Manual)

Macros use `$gensym()` explicitly when they need unique names:

```briv
macro declare_pair(prefix: String) -> Block {
    let sym: String = $gensym();
    let code: String = "
        state @{prefix}_left: Int = 0;
        state @{prefix}_right: Int = @{sym};
    ";
    return compile#(code);
};
```

---

## 6. Budget & Safety Flags

| Flag | Effect |
|------|--------|
| `--macro-budget <N>` | Set macro/template expansion step limit (default: 10,000) |
| `--unlimited-macros` | Remove all budget limits |
| `--safe-compile` | Disable `$!macro` execution entirely; `$template` still runs |
| `--unsafe-macros` | Enable `sys#()` intrinsic in macro bodies (shell execution) |

Budget is tracked per macro invocation, not globally. A macro that exceeds its budget produces a compiler error with the call site span.

---

## 7. Implementation Milestones

### M1 — Lexer & Parser Foundations (3 days)

- Add `Dollar`, `DollarBang`, `At` tokens to lexer
- Add `Expr::TemplateCall`, `Expr::MacroCall`, `Expr::Interpolate`, `Expr::InterpolateExpr`, `Expr::QuoteBlock`
- Add `TopLevel::TemplateDef`, `TopLevel::MacroDef`
- Add `MacroArgType` enum
- Implement `parse_template_def()`, `parse_macro_def()` in parser
- Implement `parse_template_call()`, `parse_macro_call()` with optional trailing block
- Implement `parse_quote_block()` — special parsing mode inside `quote { }` that accepts `@ident`/`@{expr}` as interpolation markers
- Add `Value::Expr`, `Value::Stmt`, `Value::Block`, `Value::Type` to interpreter
- Add `$error()`, `$warn()` as interpreter intrinsics for compile-time use

### M2 — Template Expansion (Phase 1a) (3 days)

- Implement `MacroContext` (budget tracking, gensym counter, call site span)
- Implement `hygiene.rs` — gensym generation, capture-avoiding identifier substitution
- Implement `template.rs` — template storage, argument binding, body execution
- Implement `expand.rs` — walk AST, find `TemplateCall` nodes, expand them
- Wire Phase 1a into `backend/mod.rs` pipeline (after Desugarer, before TypeChecker)
- Implement `quote { }` evaluation — parse block, bind args, substitute @ markers
- **Tests**: template with args, template returning `Stmt`/`Block`/`Expr`, hygiene verification

### M3 — Macro Expansion (Phase 1b) (3-4 days)

- Implement `macro_.rs` — macro storage, argument binding, body execution with full I/O
- Implement `compile#()` intrinsic — parse string, substitute @ markers, return AST
- Implement `$gensym()` intrinsic — return unique identifier
- Implement re-expansion of Phase 1a on macro output
- Wire Phase 1b into pipeline after Phase 1a
- **Tests**: macro with string mixin, macro calling template, macro with `$gensym()`, nested expansion

### M4 — Error Reporting & Spans (2 days)

- Propagate call site span through `MacroContext` to all generated AST nodes
- Implement error messages: "error: generated from macro call here: $!name(args)"
- Implement `$error()` / `$warn()` with correct span attachment
- **Tests**: verify error messages point to correct macro call sites

### M5 — Flags & Safety (1 day)

- `--macro-budget <N>`, `--unlimited-macros`
- `--safe-compile` (disables Phase 1b)
- `--unsafe-macros` (enables `sys#()`)
- **Tests**: budget exhaustion → compile error, safe-compile + macro → compile error

### M6 — Documentation & Polish (1 day)

- `docs/architecture/features/macro.md` — syntax, phase architecture, hygiene rules
- `docs/architecture/channel-map.md` — update pipeline diagram with Phase 1a/1b
- Code review, edge case handling, benchmark that no existing functionality regressed

---

## 8. Design Decisions (Resolved)

### Q1 — `@` anywhere a token goes

`@` is valid in **any token position** — identifier, type, expression, field name. The parser inside `quote { }` uniformly produces `Expr::Interpolate(String)` regardless of position. The kind check (is this `String` where an identifier is expected? `Type` in type position?) happens at expansion time, not parse time. Error messages are descriptive: *"expected String for identifier interpolation, got Expr"*.

### Q2 — Template/macro import story

Import resolution populates the registry. `import "std/wrappers.bv"` makes its templates and macros available globally before any expansion begins. This front-loads "undefined template" errors and avoids pipeline violations — import resolution is already a pass that runs before everything else.

### Q3 — `compile#()` always returns `Block`

No variadic detection. `compile#()` always returns `Value::Block(Vec<Statement>)`. The most general container. Single expressions are wrapped: `compile#("@x + 1")` becomes `{ @x + 1 }`. If the caller needs an `Expr` or `Stmt`, they extract via projection: `compile#("@x + 1") :> 0`.

### Q4 — Global gensym counter

A single global `u64` for the entire compilation. `__gensym_42` is unique across the whole program. Two separate expansions of the same template can never collide. Simple, provably correct count.

### Q5 — Recursive templates: compile error

Static recursion check at definition time. Self-calls and mutual recursion between templates → **compile error**. Templates operate on syntax only — they have no data to consume, so recursion is always an infinite loop. Budget exhaustion is the wrong safety net for something provably wrong.

**Macros**: recursion allowed with budget safety net. A macro can recurse on a decreasing compile-time integer (`n - 1`) and terminate legitimately.

### Q6 — Parser doesn't distinguish positions

The parser treats `@name`, `@type`, and `@init` identically — all become `Expr::Interpolate(String)`. The macro context at expansion time checks:

| Interpolation position | Required arg kind |
|------------------------|-------------------|
| Identifier | `String` (or `Int` convertible) |
| Type annotation | `Type` |
| Expression value | `Expr` or embeddable value |
| Field name | `String` |

Single parse path, clear error messages.

---

## 9. Implementation Gotchas (External Review)

### G1 — AST Traversal Memory: Use `std::mem::replace`

When walking the AST to expand `TemplateCall` nodes, replace the call node with a lightweight placeholder (`Expr::Term`), expand, then swap the result back. This avoids clone-heavy recursive walks and keeps compile times fast.

```rust
let placeholder = Expr::Term;
let call = std::mem::replace(expr, placeholder);
let expanded = expand_call(call);
let _ = std::mem::replace(expr, expanded);
```

### G2 — `Block` Return + Variable Scoping

`compile#()` returning `Block` means the block's natural scoping handles variable isolation. Variables declared inside a returned `Block` are scoped to that block by Briv's existing lexical analysis — no special cleanup needed. The `:> 0` projection for expression extraction is only needed when the caller explicitly wants to unwrap.

### G3 — Hygiene Exceptions for State/Struct Fields

Automatic hygiene (`__gensym_N`) must apply only to **local `let` bindings**. State declarations (`state count: Int`), struct fields, and function/transaction names **must NOT be hygienized** — the programmer needs to reference them by name across the program.

| Construct | Hygienic? | Reason |
|-----------|-----------|--------|
| `let x = ...` | ✅ `__gensym_N` | Local, no cross-reference needed |
| `state count: Int` | ❌ keep name | Referenced in other transactions |
| `fn f() { ... }` | ❌ keep name | Called from other code |
| `txn t { ... }` | ❌ keep name | Referenced in dispatch chain |

### G4 — ProofEngine Integration Is Automatic

Since Phase 1a/1b runs before TypeChecker, all macro/template output is fully expanded into standard AST by the time ProofEngine runs. Contract optimization (dead-field elimination, linear-solver) works on generated code exactly as if handwritten — no special macro support needed in the verification pipeline.

---

**Total estimated effort**: 13–17 days

**Kani requirement**: `MacroContext` budget tracking, `hygiene.rs` gensym counter, and `expand.rs` substitution logic must have proof harnesses.

**Praetor**: Each new file in `src/features/macro/` must pass complexity ≤ 15, lines ≤ 100, params ≤ 6.
