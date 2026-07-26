# Render Struct — UI Component System (was RStruct)

**Date:** 2026-06-24 (updated 2026-07-26)
**Status:** Active — `rstruct` syntax deprecated, use `render struct`/`render obj`

> **⚠️ DEPRECATED 2026-07-26:** The `rstruct` keyword is deprecated. Use
> `render struct <name> { <html> }` or `render obj <name> { <html> }` instead.
> See `docs/architecture/features/rendered-brief-wasm.md` for the current spec.
> The old examples are archived in `archive/examples/`.

## Overview

The `render struct` / `render obj` keywords attach view HTML to existing
data types. Together with top-level `let` state declarations and `txn` blocks,
they form Brief's component system for `.rbv` files.

## Syntax

```brief
// Define state
let count: Int = 0;

// Define transactions
txn increment [count < 100][@count + 1 == count] {
    count = count + 1;
    term;
};

// Attach view HTML to a struct pattern
render struct Counter {
    <div class="counter">
        <span b-text="count">0</span>
        <button b-trigger:click="increment">+</button>
    </div>
};
```

## How It Works

A `render struct Counter { <html> }` block:

1. Associates the HTML template with the name `Counter` for use in `<view>`
2. The view compiler processes the `b-*` directives into typed bindings
3. During codegen, the compiler analyzes which `txn` blocks modify which
   state fields, and generates the minimal JS to update the DOM at commit points

## View Directives

Render struct views support the same `b-*` directives:

| Directive | Purpose |
|-----------|---------|
| `b-text="expr"` | Bind text content |
| `b-trigger:click="txn"` | Bind click handler |
| `b-show="cond"` | Show/hide with CSS |
| `b-if="cond"` | Conditional render |
| `b-class="{ ... }"` | Dynamic CSS classes |
| `b-style="expr"` | Dynamic inline styles |
| `b-each:item="list"` | Iterate over list |
| `b-bind:attr="val"` | Two-way binding |

## Examples

- `examples/rstruct-demo.rbv` (migrated to `render struct` — see `archive/examples/rstruct-demo.rbv.old-rstruct` for the original)
- `examples/counter.rbv`
- `examples/shopping_cart.rbv`

## Deprecated: `rstruct` Syntax

The old `rstruct` keyword bundled state declarations, transactions, and view
HTML in a single block. The parser still accepts it for backward compatibility,
with a deprecation warning. Migrate to the new pattern:

| Old (`rstruct`) | New (`render struct`) |
|-----------------|----------------------|
| `rstruct X { field: T = val; txn ...; <html> };` | `let field: T = val; txn ...; render struct X { <html> };` |
| `rstruct` bundles state + logic + view | State (`let`), logic (`txn`), and view (`render struct`) are separate |
| Transactions prefixed with `rstruct` name | Transactions are top-level (no prefix) |
