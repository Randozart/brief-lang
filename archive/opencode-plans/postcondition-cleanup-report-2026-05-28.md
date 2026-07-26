# Postcondition Cleanup — Completion Report

**Report timestamp:** 2026-05-28T09:55:47Z  
**Author:** OpenCode agent (Plan+Build mode)  
**Session:** Postcondition `result` → `Expr::Term` migration + contract cleanup  

---

## Overview

Replaced the ad-hoc magic-string `"result"` convention for referring to function return values in postconditions with a proper `Expr::Term` AST node. Cleaned up all `.bv` postcondition contracts to use `term` instead of `result`. Fixed `range.bv` contracts to reference parameters actually in scope. Added literal-token support to `parse_pattern_fields()` in the Rust parser.

---

## Phase A — Rust Compiler Changes

### Files modified (6 files):

| File | Change |
|------|--------|
| `src/ast.rs` | Added `Expr::Term` variant to `Expr` enum |
| `src/parser.rs` | `Token::Term` → `Expr::Term` in `parse_primary()`; added `Integer`, `Float`, `BoolTrue`, `BoolFalse`, `Char` handlers to `parse_pattern_fields()` |
| `src/typechecker.rs` | Added `Expr::Term => true` to `expr_has_result()` (backward-compatible: `Expr::Identifier("result")` also still matched) |
| `src/desugarer.rs` | Removed `"result"` skip; added `Expr::Term` to leaf-expr catch-all |
| `src/annotator.rs` | Added `Expr::Term => "term"` to `format_expr()` |
| `src/interpreter.rs` | Added `Expr::Term` case looking up `"term"` from state |
| `src/proof_engine.rs` | Added `Expr::Term` to leaf-expressions in `collect_identifiers()` |
| `src/symbolic.rs` | `Expr::Term` → `SymbolicValue::Unknown` |

### Not changed (correct as-is):
- `src/dbrief/parser.rs:829` — `"result"` is the Dbrief `Result<S,E>` type keyword, unrelated
- `src/assertion_verify.rs:278,283` — `"result"` is a regular variable in test helper, unrelated
- `src/ffi/error.rs:197` — `"result"` is a generated FFI variable name, unrelated

---

## Phase B — `.bv` Postcondition Sweep

Replaced every `result` in postcondition position with `term` across all `.bv` files. The Rust parser's `expr_has_result()` is backward-compatible — both `Expr::Term` and `Expr::Identifier("result")` are recognized — so this is a source-level cleanup for language purity, not a breaking change.

### Files changed:

**Compiler:**
- `lib/compiler/call_graph.bv` — 7 postconditions (incl. tautology `result == false || result == true` → `true`)
- `lib/compiler/main.bv` — 3 postconditions

**Standard Library:**
- `lib/std/char.bv` — 26 postconditions
- `lib/std/collections.bv` — 2 postconditions
- `lib/std/encoding.bv` — 9 postconditions
- `lib/std/hashmap.bv` — 14 postconditions
- `lib/std/hashset.bv` — 14 postconditions
- `lib/std/http.bv` — 2 postconditions
- `lib/std/io.bv` — 18 postconditions
- `lib/std/iterator.bv` — 14 postconditions
- `lib/std/json.bv` — 11 postconditions
- `lib/std/math.bv` — 23 postconditions
- `lib/std/metro_bridge.bv` — 1 postcondition
- `lib/std/option.bv` — 13 postconditions
- `lib/std/process.bv` — 9 postconditions
- `lib/std/queue.bv` — 8 postconditions
- `lib/std/result.bv` — 7 postconditions
- `lib/std/shm.bv` — 1 postcondition
- `lib/std/stack.bv` — 8 postconditions
- `lib/std/string.bv` — ~50 postconditions
- `lib/std/string_builder.bv` — 12 postconditions
- `lib/std/time.bv` — 5 postconditions

**FFI Mappers:**
- `lib/ffi/mappers/c_mapper.bv` — 8 postconditions
- `lib/ffi/mappers/python_mapper.bv` — 14 postconditions
- `lib/ffi/mappers/rust_mapper.bv` — 4 postconditions
- `lib/ffi/mappers/template.bv` — 12 postconditions
- `lib/ffi/mappers/wasm_mapper.bv` — 9 postconditions

**Core lib:**
- `std/core.bv` — 4 postconditions (multi-line contract format)

**Examples:**
- `examples/main.bv`, `sig_as_type.bv`, `stdlib_usage.bv`, `test_ffi.bv`, `hello-world/src/main.bv`
- `tests/instances_test.bv`

### Tautologies removed:
- `result == false || result == true` → `true` (Bool return — always true)
- `result.is_ok() || result.is_err()` → `true` (Result return — always true)

### `result` references NOT changed (legitimate variable/parameter names):
- `let result = ...` in function bodies (iterator.bv, hashmap.bv, math.bv, etc.)
- `source_result`, `token_result`, `parse_result`, `cg_result`, `has_cycle_result`, `edge_result` — prefixed compound names
- `result` as parameter name in `lib/std/result.bv` and guards writing `[result.is_ok()]`
- `result.value`, `result.is_ok()` in function bodies (variable field access/guards)
- String literals containing `"result"` (token.bv, error messages)

---

## Phase C — `range.bv` Contract Fixes

| Function | Change |
|----------|--------|
| `has_lower_bound, has_upper_bound, min_value, max_value, set_lower_bound, set_upper_bound` | `[len(program.items) >= 0]` → `[name != ""]` (these take `name: String`, not `program`) |
| `extract_bounds_from_expr, apply_comparison` | Removed explicit `[len(program.items) >= 0][true]` brackets (these take `Expr` + `Map`, not `program`) |
| `analyze_parameter_ranges` | Kept `[len(program.items) >= 0]` — **valid**, takes `program: Program` |
| `new_param_range` | `result.min` → `term.min` |

---

## Phase D — Verification

```
cargo build                   → OK
cargo test --lib              → 269/269 passed (30.68s)
```

---

## Naming Conflict Scan

Checked all `.bv` files for `term` used as a regular variable name (which would conflict with the `Term` keyword now parsed as `Expr::Term`):

| Pattern | Matches | Verdict |
|---------|---------|---------|
| `let term` | 0 | Safe |
| `term:` (field def) | 0 | Safe |
| `.term` (member access) | 0 | Safe |
| `"term"` as string | `token.bv:177` | String literal, not identifier — safe |
| `[true][term]` as postcondition | `main.bv:265` | Correct use of `Expr::Term` keyword — safe |

**Result: No naming conflicts.**

---

## Remaining Work

1. **Brief-in-Brief parser (`parser.bv`)**: Does not yet recognize `term` in postconditions — it still expects `result`. When the self-hosted compiler reaches parity with the Rust parser, this needs updating.
2. **Method-call syntax in postconditions**: Postconditions like `term.len()` use method-call syntax (parsed as `Call("len", [Term])` at the AST level). Per the language philosophy, these should be function-call style `len(term)`. This is cosmetic (AST is identical) and was deferred.

---

## Files Changed (count)

- **Rust source files:** 8 modified
- **`.bv` source files:** ~40 modified  
- **Total:** ~48 files
