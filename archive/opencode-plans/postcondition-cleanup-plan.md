# Postcondition Cleanup: Expr::Term + result → term sweep

## Goal
Replace the magic string `"result"` in postconditions with a proper `Expr::Term` AST node. Remove all method-call syntax (`.len()`, `.is_some()`, etc.) from postconditions, replacing with function-call style. Fix `parse_pattern_fields()` to handle literal tokens. Fix `range.bv` contracts.

## Phase A — Rust Compiler (5 files)

### A1: `src/ast.rs` — Add `Expr::Term` variant
```diff
     Bool(bool),
+    Term,
     Identifier(String),
```

### A2: `src/parser.rs` — Token::Term → Expr::Term in `parse_primary()`
Add before `Token::Identifier` match (~line 4282):
```rust
Some(Ok(Token::Term)) => {
    self.advance();
    Ok(Expr::Term)
}
```

### A3: `src/parser.rs` — Literal tokens in `parse_pattern_fields()`
Add before the `_ => { /* try identifier */ }` catch-all (~line 3025):
```rust
Some(Ok(Token::Integer(val))) => {
    fields.push(val.to_string());
    self.advance();
}
Some(Ok(Token::Float(val))) => {
    fields.push(val.to_string());
    self.advance();
}
Some(Ok(Token::BoolTrue)) => {
    fields.push("true".to_string());
    self.advance();
}
Some(Ok(Token::BoolFalse)) => {
    fields.push("false".to_string());
    self.advance();
}
Some(Ok(Token::Char(c))) => {
    fields.push(format!("'{}'", c));
    self.advance();
}
```

### A4: `src/typechecker.rs` — Replace `name == "result"` with `Expr::Term`
Line 953:
```diff
-            Expr::Identifier(name) => name == "result",
+            Expr::Term => true,
```

### A5: `src/desugarer.rs` — Replace `name != "result"` with `Expr::Term`
Line 25-28:
```diff
-                // Skip 'result' - that's a special output variable for definitions
-                if name != "result" && !vars.contains(name) {
+                if !vars.contains(name) {
                     vars.push(name.clone());
                 }
```
And add to the catch-all at line 57:
```diff
-            Expr::Bool(_) | Expr::Integer(_) | Expr::Float(_) | Expr::String(_) => {}
+            Expr::Bool(_) | Expr::Term | Expr::Integer(_) | Expr::Float(_) | Expr::String(_) => {}
```

### NOT touched (correct as-is):
- `src/dbriv/parser.rs:829` — `"result"` is the Dbriv `Result<S,E>` type keyword, unrelated
- `src/assertion_verify.rs:278,283` — `"result"` is a regular variable name in test helper, unrelated
- `src/analysis/dataflow.rs:195` — `"result"` is a heuristic string search, unrelated
- `src/ffi/error.rs:197` — `"result"` is a generated FFI variable name, unrelated

---

## Phase B — All `.bv` files: `result` → `term` in postconditions

### B1: `lib/std/list.bv`
Line with `result` in postcondition: `len(result) == 0`.
→ `[len(term) == 0]` (already function-call style, just rename)

### B2: `lib/std/string.bv`  
Line with `result` in postcondition: `len(result) >= 0`.
→ `[len(term) >= 0]` (or simply `[true]` since len ≥ 0 always)

### B3: `lib/std/map.bv`
Line with `result` in postcondition: `len(result) == len(list)`.
→ `[len(term) == len(list)]`

### B4: `lib/std/iterator.bv`
Line with `result` in postcondition: `len(result) >= 0`.
→ `[len(term) >= 0]` (or `[true]`)

### B5: `lib/std/option.bv`
Lines with `result` in postcondition: `is_some(result)`, `is_none(result)`.
→ `[is_some(term)]`, `[is_none(term)]` (already function-call style)

### B6: `lib/std/hashmap.bv`
Line with `result` in postcondition: `len(result) >= 0`.
→ `[len(term) >= 0]` (or `[true]`)

### B7: `lib/compiler/call_graph.bv`
Multiple `result` references in postconditions (7+):
- `result.graph` → `term.graph` (struct field access — OK)
- `result.txn_names` → `term.txn_names` (struct field access — OK)
- `result.entry` → `term.entry` (struct field access — OK)
- `len(result) >= 0` → `len(term) >= 0` or `[true]`
- `result == false || result == true` → `[true]` (tautology)

### B8: `lib/compiler/main.bv`
- `is_ok(result) || is_err(result)` → `[true]` (tautology for Result type)
- `len(result) >= 0` → `[true]`

### B9: `lib/ffi/mappers/*.bv`
Various postconditions referencing `result`:
- `len(result) == len(name)` → `len(term) == len(name)`
- `result == ...` → `term == ...`

### B10: `examples/*.bv`
Any postconditions referencing `result`.

---

## Phase C — `range.bv` contract fixes

### C1: Fix 6 function contracts: change `[len(program.items) >= 0]` to `[name != ""]`
- `has_lower_bound(ranges: Map<String, ParamRange>, name: String)`
- `has_upper_bound(ranges: Map<String, ParamRange>, name: String)`
- `min_value(ranges: Map<String, ParamRange>, name: String)`
- `max_value(ranges: Map<String, ParamRange>, name: String)`
- `set_lower_bound(ranges: Map<String, ParamRange>, name: String, value: Int)`
- `set_upper_bound(ranges: Map<String, ParamRange>, name: String, value: Int)`

### C2: Remove explicit brackets from 3 functions without `name` parameter
- `extract_bounds_from_expr(e: Expr, ranges: Map<String, ParamRange>)`
- `apply_comparison(op: ComparisonOp, ...)`
- `analyze_parameter_ranges(expr: Expr, ...)`

### C3: Fix 1 postcondition rename
- `new_param_range`: `result` → `term` (or remove tautology)

---

## Phase D — Verify

1. `cargo build`
2. `./target/debug/briv-compiler check lib/compiler/*.bv`
3. `./target/debug/briv-compiler check lib/std/*.bv`
4. `./target/debug/briv-compiler check lib/ffi/mappers/*.bv`
5. `./target/debug/briv-compiler check lib/ffi/mappers/**/*.bv`
6. `cargo test --lib`

## Files NOT to touch
- `src/dbriv/parser.rs` — `"result"` is the Dbriv Result<S,E> type keyword
- `src/assertion_verify.rs` — `"result"` is a regular variable in test helper
- `src/analysis/dataflow.rs` — string heuristic
- `src/ffi/error.rs` — FFI variable name generation
- `src/fuzzing/concolic.rs` — unrelated test assertion
- `lib/ffi/mappers/error.bv` — already fixed
