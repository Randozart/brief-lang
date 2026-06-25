# Skip List Standard Library — Compiler Gap Analysis & Implementation Plan

**Date:** 2026-06-25
**Status:** Plan (awaiting execution request)
**Author:** Agent analysis

---

## Problem

The skip list data structure cannot currently be implemented as a first-class
stdlib type because the type definition system (TypeDef) cannot dispatch
custom operations through the `<-` arrow operator.

## Root Cause: Three Gaps

### Gap 1 — Fixed strategy enum only (`src/type_universe.rs:96-117`)

`InsertStrategy` has only 4 variants: `Append`, `Prepend`, `Sorted`, `Hash`.
`ExtractStrategy` has only 3: `Pop`, `Shift`, `Hash`.

The resolver at `insert_strategy()` (line 447) returns `None` for any
unrecognized string — the `_ => None` fallthrough at line 455. There is no
mechanism for a TypeDef to declare "call this function" as its insert strategy.

### Gap 2 — Interpreter strategy lookup uses variable name (`src/interpreter.rs:970-981`)

```rust
pub(crate) fn lookup_insert_strategy(&self, root_name: &str) -> Option<InsertStrategy> {
    let tu = self.type_universe.as_ref()?;
    // root_name is "fifo" (variable name), but type universe has "Fifo" (type name)
    if let Some(s) = tu.insert_strategy(root_name) {
        return Some(s);
    }
    None
}
```

The interpreter does not track `let`-declared type annotations at runtime, so
it uses the variable name as a heuristic. This silently fails for any type
where the variable name differs from the type name (e.g., `let my_skip: SkipList<Int> = ...`).

### Gap 3 — LLVM Pop ignores strategy (`src/backend/llvm/emit_expr.rs:3706`)

The LLVM Push path calls `check_insert_strategy()` (line 3573) to determine
prepend vs append behavior. The Pop path has no equivalent — it always pops
from the end (`len - 1` at line 3721). `check_extract_strategy()` does not
exist.

---

## Fix: Add `Custom(String)` Strategy Variant

### Step 1 — Extend enums (`src/type_universe.rs`)

```rust
pub enum InsertStrategy {
    Append,
    Prepend,
    Sorted,
    Hash,
    Custom(String),  // <-- NEW: user-defined function name
}

pub enum ExtractStrategy {
    Pop,
    Shift,
    Hash,
    Custom(String),  // <-- NEW: user-defined function name
}
```

### Step 2 — Return Custom for unrecognized strings (`src/type_universe.rs:447-471`)

```rust
// In insert_strategy():
"append" => Some(InsertStrategy::Append),
"prepend" => Some(InsertStrategy::Prepend),
"sorted" => Some(InsertStrategy::Sorted),
"hash" => Some(InsertStrategy::Hash),
_ => Some(InsertStrategy::Custom(strat.clone())),  // was: None

// In extract_strategy():
"pop" => Some(ExtractStrategy::Pop),
"shift" => Some(ExtractStrategy::Shift),
"head" => Some(ExtractStrategy::Shift),
"tail" => Some(ExtractStrategy::Pop),
"hash" => Some(ExtractStrategy::Hash),
_ => Some(ExtractStrategy::Custom(strat.clone())),  // was: None
```

No changes needed to parsing or TypeDef resolution — `InsertAt = fn_name;`
already stores `"fn_name"` in `resolved.insert_at` via
`type_universe_expr_to_string`.

### Step 3 — Fix interpreter strategy lookup (`src/interpreter.rs:970-990`)

Change `lookup_insert_strategy` and `lookup_extract_strategy` to accept a
type name parameter instead of deriving it from the variable name. The caller
(`arrow.rs`) must pass the declared type name.

Alternatively, store the type annotation alongside each variable in the
interpreter state, so the lookup can be resolved internally.

### Step 4 — Handle Custom in interpreter dispatch (`src/features/arrow.rs`)

In the Push dispatch (line 39-58), add a match arm:

```rust
InsertStrategy::Custom(fn_name) => {
    let call = Value::Defn(fn_name.clone());
    let result = ctx.call_function(call, vec![collection, v])?;
    ctx.store_arrow_value(&root_name, &field_path, result.clone());
    return Ok(result);
}
```

`call_function` is a new helper that resolves `fn_name` to a `defn` or `inop`
and evaluates it.

Similarly for Pop:

```rust
ExtractStrategy::Custom(fn_name) => {
    let call = Value::Defn(fn_name.clone());
    let result = ctx.call_function(call, vec![collection])?;
    ctx.store_arrow_value(&root_name, &field_path, ...)?;
    return Ok(result);
}
```

### Step 5 — Add extract strategy check to LLVM backend (`src/backend/llvm/emit_toplevel.rs`)

```rust
pub(super) fn check_extract_strategy(&self, target: &Expr) -> Option<ExtractStrategy> {
    let tu = self.type_universe.as_ref()?;
    let var_name = match target {
        Expr::OwnedRef(n) | Expr::Identifier(n) => n,
        _ => return None,
    };
    let ty = self.let_original_types.get(var_name)?;
    let type_name = match ty {
        Type::Custom(n) => n,
        Type::Applied(n, _) => n,
        _ => return None,
    };
    tu.extract_strategy(type_name)
}
```

For `Custom(fn_name)` in the LLVM Pop path, emit a function call to `fn_name`
with the list handle argument.

---

## Skip List Stdlib (`lib/std/skiplist.bv`)

After the compiler changes, the skip list is defined using:

```brief
import option from "std/option.bv";

// ---- Type definition --------------------------------
type SkipList<T> <: List<T> {
    InsertAt = _sl_insert;
    ExtractFrom = _sl_remove;
};

// ---- Inop declarations for arrow dispatch -----------
inop _sl_insert<T>(list: SkipList<T>, val: T) -> SkipList<T>
{ term %res; }
fallback {
    // Pure-Brief skip list insert using index-based node pool
    // stored as a flat List in the flat buffer.
};

inop _sl_remove<T>(list: SkipList<T>, val: T) -> (Option<T>, SkipList<T>)
{ term %res; }
fallback {
    // Pure-Brief skip list remove
};

// ---- Public API -------------------------------------
defn new_skiplist<T>() -> SkipList<T> { term [] :> ...
defn sl_contains<T>(list: SkipList<T>, val: T) -> Bool { ... };
defn sl_to_list<T>(list: SkipList<T>) -> List<T> { ... };
```

The **BILD bodies** are left as stubs initially (`term %res;`) — the
interpreter uses the pure-Brief fallback, and the LLVM backend emits the
fallback since no BILD is provided.

---

## File Change Summary

| File | What | Risk |
|------|------|------|
| `src/type_universe.rs` | Add `Custom(String)` to enums, return `Custom` for unrecognized strings | Low — additive, no existing paths touched |
| `src/interpreter.rs` | Fix `lookup_insert_strategy`/`lookup_extract_strategy` to use type name | Medium — needs type info plumbing |
| `src/features/arrow.rs` | Handle `Custom(fn_name)` dispatch, use corrected strategy lookup | Medium — new dispatch path |
| `src/backend/llvm/emit_toplevel.rs` | Add `check_extract_strategy()` | Low — mirrors existing `check_insert_strategy` |
| `src/backend/llvm/emit_expr.rs` | Handle Custom in Push; wire `check_extract_strategy` in Pop | Medium — new codegen path |
| `lib/std/skiplist.bv` | New file: type definition + inop declarations + public API | Low — new file, no existing code touched |
| `src/typechecker.rs` | Possibly: register `SkipList` type | Low |
| `src/backend/llvm/mod.rs` | Possibly: collect inop decls for skiplist | Low |

---

## Testing Strategy

1. **Unit test**: `InsertStrategy::Custom("_sl_insert")` resolves from
   `InsertAt = _sl_insert;` in type universe
2. **Interpreter test**: `let sl: SkipList<Int> = ...; &sl <- 42;` dispatches
   through `Custom("_sl_insert")` and calls the fallback
3. **LLVM test**: Emit IR for `<-` on `SkipList<Int>` target with custom
   strategy, verify function call is generated
4. **Stdlib test**: Skip list operations (insert, search, delete, contains)
   work end-to-end via interpreter
