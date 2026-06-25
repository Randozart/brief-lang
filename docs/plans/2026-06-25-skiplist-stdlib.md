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

*(Note: Line numbers below are from the pre-implementation analysis. As of June 25, all three gaps have been fixed — see commit `[current HEAD]`.)*

### Gap 1 — Fixed strategy enum only (`src/type_universe.rs:95-118`)

`InsertStrategy` had only 4 variants: `Append`, `Prepend`, `Sorted`, `Hash`.
`ExtractStrategy` had only 3: `Pop`, `Shift`, `Hash`.

The resolver at `insert_strategy()` (line 455) returned `None` for any
unrecognized string — the `_ => None` fallthrough at line 455. No mechanism
for a TypeDef to declare "call this function" as its insert strategy.

### Gap 2 — Interpreter strategy lookup used variable name (`src/interpreter.rs:986-1006`)

`lookup_insert_strategy` passed the *variable name* (e.g. `"fifo"`) to
`tu.insert_strategy()`, but the type universe had `"Fifo"` (declared type name).
No `let_types` tracking existed at runtime.

### Gap 3 — LLVM Pop ignored strategy (`src/backend/llvm/emit_expr.rs:3795`)

The LLVM Push path called `check_insert_strategy()`. The Pop path had no
equivalent — always popped from the end (`len - 1`). `check_extract_strategy()`
did not exist.

---

## Fix Applied: `Custom(String)` Strategy Variant

All changes below have been implemented in the June 25 commits. The sections
document the as-built design.

### Step 1 — Extended enums (`src/type_universe.rs:95-128`)

```rust
#[derive(Debug, Clone, PartialEq)]  // Copy removed — String is not Copy
pub enum InsertStrategy {
    Append,
    Prepend,
    Sorted,
    Hash,
    Custom(String),  // NEW: user-defined function name
}

pub enum ExtractStrategy {
    Pop,
    Shift,
    Hash,
    Custom(String),  // NEW: user-defined function name
}
```

### Step 2 — Return Custom for unrecognized strings (`src/type_universe.rs`)

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

### Step 3 — Fixed interpreter strategy lookup (`src/interpreter.rs`)

Added `let_types: HashMap<String, Type>` to the `Interpreter` struct,
populated on `Statement::Let` when a type annotation is present.
`lookup_insert_strategy` and `lookup_extract_strategy` now resolve the
declared type by name, then look up the strategy by type name.

```rust
pub(crate) fn lookup_insert_strategy(&self, root_name: &str) -> Option<InsertStrategy> {
    let tu = self.type_universe.as_ref()?;
    let type_name = self.let_types.get(root_name).and_then(|t| match t {
        Type::Custom(n) => Some(n.as_str()),
        Type::Applied(n, _) => Some(n.as_str()),
        _ => None,
    })?;
    tu.insert_strategy(type_name)
}
```

### Step 4 — Custom dispatch in interpreter (`src/features/arrow.rs`)

Push Custom: calls `call_custom_fn(fn_name, vec![collection, value])` which
returns the new collection. Pop Custom: calls with `vec![collection]` and
expects a 2-element list `(popped, new_collection)`.

`call_custom_fn` is a new helper (`interpreter.rs`) that tries inop first
(fallback expression), then defn (body execution), with runtime Value args.

### Step 5 — LLVM backend (`src/backend/llvm/emit_toplevel.rs`, `emit_expr.rs`)

- `check_extract_strategy()` added (mirrors `check_insert_strategy`)
- Pop path uses `should_shift` to pop from front when strategy is Shift
- Push Custom: emits `call i64 @fn_name(i64, i64)`
- Pop Custom: emits `call { i64, i64 } @fn_name(i64)` and extracts results

### Step 6 — Stdlib files

- `lib/std/skiplist.bv` — type definition, inop declarations, public API
- `lib/std/core/skiplist.bv` — copy in core/ for import resolution
- `lib/std/from-bits.bv` — educational SkipList entry added

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

## Testing Strategy (all implemented)

1. **Unit test**: `test_insert_strategy_resolution_unknown` now asserts
   `Custom("custom_strat")` instead of `None` (`type_universe.rs`)
2. **Interpreter test (inop)**: `test_custom_insert_strategy_dispatch` —
   Push dispatches through `Custom("my_insert")` and calls inop fallback
3. **Interpreter test (defn)**: `test_custom_insert_strategy_with_defn` —
   Push dispatches through `Custom("sl_insert_fn")` and calls defn body
4. **Interpreter test (pop)**: `test_custom_extract_strategy_dispatch` —
   Pop dispatches through `Custom("my_extract")` and returns tuple
5. All 1306 tests pass (`cargo test --lib`)
