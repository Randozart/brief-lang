# RStruct — Rendered Struct Components

**Date:** 2026-06-24
**Phase:** TBD
**Status:** Fully parsed, desugared, and emitted in the web pipeline

## Overview

`rstruct` (Rendered Struct) is Brief's component system for `.rbv` files. It defines a self-contained UI component with its own state, transactions, and HTML view. An `rstruct` desugars into separate state declarations, top-level transactions, and a render block.

## Syntax

```brief
rstruct ComponentName {
    // State fields with optional defaults
    field1: Type1 = default1;
    field2: Type2 = default2;

    // Transactions (become ComponentName.method)
    txn ComponentName.method [pre][post] {
        &field1 = new_value;
        term;
    };

    // View HTML with b-* directives
    <div class="component">
        <span b-text="field1">Default</span>
        <button b-trigger:click="method">Click</button>
    </div>
};
```

## Desugaring

An `rstruct Counter { count: Int = 0; txn ...; <div>...</div> }` desugars into:

1. **State declarations**: `let count: Int = 0;`
2. **Top-level transactions**: `txn Counter.increment ...`
3. **Struct definition**: `struct Counter { count: Int }` with the view HTML attached
4. **Render block**: Maps the struct name to its HTML template

## Features

- **Self-contained**: State, logic, and view in one block
- **Reusable**: Multiple instances via `<Component />` tags
- **Encapsulated**: Field names are namespaced within the component
- **Desugared early**: After parsing, the compiler sees flat state and transactions

## View Directives

RStruct views support the same `b-*` directives as top-level `<view>` blocks:

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

- `examples/rstruct-demo.rbv` — Simple greeter component
- `examples/counter.rbv` — Counter with increment/decrement/reset
- `examples/shopping_cart.rbv` — Multi-step shopping cart with product selection
