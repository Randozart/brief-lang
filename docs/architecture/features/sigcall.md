# SigCall — Executable Signatures

**Date:** 2026-06-24
**Phase:** TBD
**Status:** Fully parsed (top-level sig declarations); expression-level SigCall is stubbed

## Overview

`sig` (signature) declarations are Briev's mechanism for declaring an interface contract. They serve two roles:

1. **Top-level declaration**: `sig name: params -> output_type;` declares a callable signature
2. **Higher-order function types**: `sig_name` used as a type constrains which functions can be passed

## Syntax

### Top-Level Declaration

```briev
// Simple signature
sig print: String -> Bool;

// With modifier
sig #out fetch_user: Int -> Bool | Int;

// With union return type (| means "one of")
sig fetch_user: Int -> Bool | Int;

// With tuple return type (, means "all")
sig split: String -> String, String;

// With array return type ([] means "list of")
sig find_all: String -> Bool[];

// Bind to a specific defn for path verification
sig my_sig: Int -> Bool = some_defn;
```

### Sig Modifiers

| Modifier | Meaning | Effect |
|----------|---------|--------|
| `#out` | Has observable side effects | Compiler preserves ordering, no folding |
| `#inline` | Pure, no side effects | Safe to fold, CSE, reorder |
| `#export("name")` | Emit globally-visible symbol | C ABI, visible to linker |

### Sig as Type

Signatures can be used as first-class function types:

```briev
sig print_func: String -> Bool;

defn my_printer(msg: String) [msg != ""][term == true] -> Bool {
    term true;
};

defn apply(msg: String, func: sig_print_func) -> Bool {
    term func(msg);
};

txn test {
    let result = apply("hello", my_printer);
    term;
};
```

Here `sig_print_func` (the sig name prefixed by `sig_`) acts as a function type constraint. Any `defn` or `txn` with matching signature can be passed.

### Union Return Types

```briev
sig fetch: Int -> Int | Error;

txn load [status == false][status == true] {
    let result = fetch(1);
    uni result(Int(val)) = { &status = true; };
    uni result(Error(e)) = { &error_log = e; };
    term;
};
```

## Evaluation

The interpreter evaluates sig calls by dispatching to the bound `defn` or `txn`. Sig call modifiers (`#out`, `#inline`) are currently ignored at evaluation time but influence optimization decisions.

## Backend Status

| Backend | Status |
|---------|--------|
| Interpreter | ✅ Full evaluation |
| LLVM | ⚠️ Stub — SigCallExpr returns `%sig: Void` |
| Webstack | ⚠️ Not implemented |
| CIRCT | ⚠️ Not implemented |

The LLVM backend codegen for `Expr::SigCall` is a stub. Top-level sig declarations (used as documentation/interface contracts) are handled at parse time and do not generate code.
