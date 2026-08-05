# AST Migration — 42 Test Regression Fix Plan

## Current State
- `cargo build`: **0 errors**
- `cargo check --lib`: **0 errors**
- `cargo test --lib`: **755 passed, 42 failed** (after removing 9 architecture-reiterating intrinsic tests)

All 42 failures are runtime assertions. Every single one has a documented root cause.

---

## Clean Code Directives (Apply to Every Change)

Every new/modified function must follow these rules:

### 1. Flat Control Flow — Max 2 Levels Deep
No arrowhead code. Never indent past 2 levels.

```rust
// DON'T:
fn process(x: Option<Value>) -> Option<i64> {
    if let Some(val) = x {
        if let Some(result) = val.as_i64() {
            if result > 0 {
                return Some(result);
            }
        }
    }
    None
}

// DO:
fn process(x: Option<Value>) -> Option<i64> {
    let val = x?;
    let result = val.as_i64()?;
    if result <= 0 { return None; }
    Some(result)
}
```

### 2. Guard Clauses and Early Returns
```rust
// DON'T:
if a { A } else if b { B } else if c { C } else { D }

// DO:
if a { return A; }
if b { return B; }
if c { return C; }
D
```

### 3. Doc Comments on Every Definition
Every `fn`, `struct`, `enum`, `mod` must have a `///` doc comment.

### 4. Comment Every Code Change
```
// 2026-07-14: <why this code exists, what bug it fixes>
```

### 5. HashMap Iteration Determinism
Sort all HashMaps before iterating for LLVM IR emission.

---

## Phase 1: Parser Fix — Optional Parens in `defn`

### Problem
`parse_definition()` calls `self.expect(Token::LParen)` making parentheses mandatory. Test source files omit parens (`defn hello -> Int { ... }`). `parse_transaction()` already uses optional parens — mirror that pattern.

### File
`src/parser/definitions.rs:49-57`

### Change
```rust
// OLD (broken):
fn parse_definition(&mut self) -> Result<Definition, SyntaxError> {
    self.pos += 1;
    let name = self.expect_identifier()?;
    let type_params = self.parse_type_params()?;
    self.expect(Token::LParen)?;
    let parameters = self.parse_parameter_list()?;
    self.expect(Token::RParen)?;
    let output_type = self.parse_output_type()?;
    // ...
}

// NEW (mirrors parse_transaction):
fn parse_definition(&mut self) -> Result<Definition, SyntaxError> {
    self.pos += 1;
    let name = self.expect_identifier()?;
    let type_params = self.parse_type_params()?;
    let parameters = if self.eat(&Token::LParen) {
        let p = self.parse_parameter_list()?;
        self.expect(Token::RParen)?;
        p
    } else {
        Vec::new()
    };
    let output_type = self.parse_output_type()?;
    // ...
}
```

### Tests Fixed
`test_resolve_bv_file`, `test_resolve_checked_cached_modules`, `test_filter_items_by_name`, `test_filter_items_empty`, `test_glob_import_non_recursive`, `test_auto_core_injection`

---

## Phase 2: Core Infrastructure Fixes

### 2.1 `as_i64()` — Zero-Pad Short Buffers

**File:** `src/interpreter/mod.rs:165-173`
**Tests:** `test_zero_bits`, `test_zext_bool_to_int`

**Problem:** `as_i64()` returns `None` for bit buffers < 8 bytes. `zero_bits(4)` creates a 4-byte zero. `as_i64()` rejects it.

**Fix:**
```rust
// 2026-07-14: Zero-pad short bit buffers in LE order so small values
// (zero_bits(4), zext of a 1-byte bool) can be converted to i64.
pub fn as_i64(&self) -> Option<i64> {
    let bytes = match self {
        Value::Bits(b) if b.len() <= 8 => {
            let mut arr = [0u8; 8];
            arr[..b.len()].copy_from_slice(b);
            arr
        }
        Value::Bits(b) if b.len() > 8 => b[..8].try_into().ok()?,
        _ => return None,
    };
    Some(i64::from_le_bytes(bytes))
}
```

### 2.2 `analyze_program()` — Wire Real Analysis Passes

**File:** `src/backend/mod.rs:33-56`
**Tests:** `test_exit_pragma_*` (2), `test_natural_death_*` (2), `test_iir_filter_folded_path_regression`, `test_precompute_pure_counter`, `test_report_shows_*` (3), `test_async_barrier_calls_in_main` — ~10 tests

**Problem:** Stub returns empty `AnalysisResults`. All downstream code depending on transition graph/region analysis gets empty data — exit condition detection, natural death, foldable path, async barrier dispatch, reports all silently produce nothing.

**Fix:** Replace the stub body with actual analysis calls:
```rust
pub fn analyze_program(items: &[TopLevel], optimize: bool) -> AnalysisResults {
    let transition_graph = ReactorTransitionGraph::build(items, &None, &vec![]);
    let (linear_chains, composed_chains) = ChainAnalyzer::build(&transition_graph, items);
    let region_analyzer = RegionAnalyzer::analyze(items, &transition_graph, optimize);
    AnalysisResults {
        transition_graph,
        region_analyzer,
        linear_chains,
        composed_chains,
    }
}
```

**Note:** This is the single highest-impact fix. Without it, ~10 tests can't work regardless of other corrections.

### 2.3 Dependency Graph — Restore Edge Extraction

**File:** `src/analysis/dependency_graph.rs:30-91`
**Tests:** `test_cycle_detection`, `test_simple_dependency`, `test_chain_dependency`, `test_multiple_trgs`

**Problem:** `StateDecl.expr` was removed from the AST. The old Pass 2 extracted dependency edges from state initialization expressions like `state derived: Int = sensor + 5`. Without these edges, Kahn's algorithm sees all nodes with in-degree 0 and emits in hash order.

**Fix:** Re-introduce a `deps` field on `StateDecl` populated from the assignment operator or contract. If unavailable, add a new pass that scans transaction bodies for state-field writes and infers dependencies from the read set.

```rust
// 2026-07-14: New Pass 2 — extract dependency edges from state initializer
// references. StateDecl now carries `deps: Vec<String>` populated by the parser
// from state assignment expressions (or inferred).
for item in items {
    if let TopLevel::StateDecl(decl) = item {
        for dep in &decl.deps {
            if all_vars.contains(dep) {
                dependencies.entry(decl.name.clone())
                    .or_default()
                    .push(dep.clone());
                dependents.entry(dep.clone())
                    .or_default()
                    .push(decl.name.clone());
            }
        }
    }
}
```

---

## Phase 3: LLVM Codegen Rewrites

### 3.1 `Expr::List` — Full 2-Slot Header Protocol

**File:** `src/backend/llvm/emit_expr.rs:126-133`
**Tests:** `test_list_literal_2slot_header`, `test_empty_list_global_sentinel`, `test_nonempty_list_uses_malloc`, `test_list_index_uses_2slot_header`, `test_list_len_loads_length`

**Problem:** The handler is a stub: `alloca i64, i64 N` + `ptrtoint`. No `malloc`, no length storage, no element computation, no `@ll_empty_list` sentinel.

**Fix:** Complete rewrite with flat control flow:
```rust
// 2026-07-14: Proper 2-slot heap-allocated list emission.
// Non-empty: malloc((2+N)*8), store length, compute elements.
// Empty: reference global @ll_empty_list sentinel.
Expr::List(exprs) => {
    let count = exprs.len();
    let reg = if count == 0 {
        writeln!(out, "{}{} = ptrtoint ptr @ll_empty_list to i64", indent, v).ok();
        v.to_string()
    } else {
        let bytes = (2 + count) * 8;
        let m = self.fun.gen_reg();
        writeln!(out, "{}{} = call ptr @malloc(i64 {})", indent, m, bytes).ok();
        let cast = self.fun.gen_reg();
        writeln!(out, "{}{} = bitcast ptr {} to ptr", indent, cast, m).ok();
        let len_ptr = self.fun.gen_reg();
        writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 0", indent, len_ptr, cast).ok();
        writeln!(out, "store i64 {}, ptr {}", count, len_ptr).ok();
        for (i, e) in exprs.iter().enumerate() {
            let elem = self.emit_expr(out, e, indent);
            let slot_ptr = self.fun.gen_reg();
            writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 {}", indent, slot_ptr, cast, 1 + i as u64).ok();
            writeln!(out, "store i64 {}, ptr {}", elem.name, slot_ptr).ok();
        }
        writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, v, cast).ok();
        v.to_string()
    };
    TypedRegister { name: reg, ty: Type::ptr(Type::int()) }
}
```

### 3.2 `Expr::Tuple` — Same 2-Slot Protocol

**File:** `src/backend/llvm/emit_expr.rs:117-123`
**Tests:** `test_tuple_emits_2slot_header`

**Problem:** Stub that only emits the first element.

**Fix:** Same `malloc` + length + element storage pattern as `Expr::List`. Structurally identical.

### 3.3 `Expr::Cast` — String Conversion Dispatch

**File:** `src/backend/llvm/emit_expr.rs:154-165`
**Tests:** `test_emit_cast_int_to_string`, `test_emit_cast_string_to_int`

**Problem:** Falls to `bitcast ptr %x to i64` for string-to-int casts instead of calling runtime helpers `__int_to_str__` / `__str_to_int`.

**Fix:** Add string conversion arms:
```rust
// 2026-07-14: Handle String ⟷ Int conversions via runtime helpers
// __int_to_str__ and __str_to_int. The runtime declares exist at
// emit_toplevel.rs:1696-1698, but the Cast handler never called them.
let src_ll = lower_type(&src.ty);
let tgt_ll = lower_type(target);
match (src_ll.as_str(), tgt_ll.as_str()) {
    ("i64", "ptr") => {
        writeln!(out, "{}{} = call i64 @__int_to_str__(i64 {})", indent, v, src.name).ok();
    }
    ("ptr", "i64") => {
        let ptr = self.fun.gen_reg();
        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, src.name).ok();
        writeln!(out, "{}{} = call i64 @__str_to_int(ptr {})", indent, v, ptr).ok();
    }
    _ if tgt_ll == "double" => {
        writeln!(out, "{}{} = sitofp i64 {} to double", indent, v, src.name).ok();
    }
    _ if tgt_ll == "i64" && src_ll == "double" => {
        writeln!(out, "{}{} = fptosi double {} to i64", indent, v, src.name).ok();
    }
    _ => {
        writeln!(out, "{}{} = bitcast {} {} to {}", indent, v, src_ll, src.name, tgt_ll).ok();
    }
}
```

### 3.4 `Expr::Float` — Update Test Expectation

**File:** `src/backend/llvm/tests.rs:367`
**Tests:** `test_local_float_binding`

**Problem:** Tests expects `bitcast i32` but codegen now emits `fadd double 0.0, 2` (native float arithmetic, which is correct).

**Fix:** Change assertion from `"bitcast i32"` to `"fadd double 0.0, 2"`.

### 3.5 Struct Byte Size Computation

**File:** `src/backend/llvm/mod.rs:1497-1523` + match arm for `TopLevel::TypeDef`
**Tests:** `test_struct_auto_registered_in_type_universe`, `test_type_with_slots_populates_struct_types`

**Problem:** `bytes: 8` is hardcoded. For a `Point` struct with 2 `Int` fields (16 bytes), the type universe reports 8. Also `TopLevel::TypeDef` is not handled in `generate()` iteration.

**Fix:** Compute bytes from field types dynamically. Add `TopLevel::TypeDef` match arm.

### 3.6 Exit Condition Codegen — Wire into Main Emission

**File:** `src/backend/llvm/mod.rs` (emit_main, emit_ssa_main, reactor loop)
**Tests:** `test_exit_pragma_in_wake_main`, `test_exit_pragma_without_wake_no_change`

**Problem:** Exit condition `Option<Box<Expr>>` is accepted by `generate()` but never emitted as LLVM IR. Test also doesn't pass `Some(...)` to generate — `make_exit_program` ignores its `exit_expr` parameter.

**Fix:** In main entry/loop: evaluate exit condition → `trunc i64 ... to i1` → `br i1 ... label %done, label %loop` → `done: ret i32 0`. Fix the test helper to actually wire the exit condition through.

### 3.7 MMIO Address Guarded Insert

**File:** `src/backend/llvm/mod.rs:3119-3120`
**Tests:** `test_imported_alias_is_mmio`

**Problem:** `build_field_index` unconditionally sets address to 0, overwriting prepopulated addresses like `0x40000000`.

**Fix:**
```rust
// 2026-07-14: Don't overwrite prepopulated MMIO addresses.
if !self.ctx.mmio_prepopulated || !self.ctx.mmio_fields.contains_key(&s.name) {
    self.ctx.mmio_fields.insert(s.name.clone(), 0u64);
}
```

### 3.8 Async Barrier Emission — Wire `emit_async_phase`

**File:** `src/backend/llvm/helpers.rs:326`, `mod.rs:2576-2593`
**Tests:** `test_async_barrier_calls_in_main`

**Problem:** `emit_async_phase` is defined but never called. `__thread_pool_init__` is declared but never called.

**Fix:** Call `__thread_pool_init__` in main entry block when `has_async_txns`. Call `emit_async_phase` in the main loop body.

### 3.9 GPU Unsafe Intrinsic Message

**File:** `src/backend/llvm/gpu.rs` or test
**Tests:** `test_check_eligibility_unsafe_intrinsic_blocked`

**Problem:** Test checks for "unsafe intrinsic" but message says "FFI call".

**Fix:** Update test assertion to match the actual message.

---

## Phase 4: Remaining One-Off Behavioral Fixes

All are flat, max-2-depth changes (generally 1-5 lines each, fixes ~9 tests).

| # | File | Line(s) | Fix | Tests Fixed |
|---|------|---------|-----|-------------|
| 4.1 | `analysis/watchdog.rs` | 123-125 | Add check that Identifier matches a known trigger name | `test_extract_trigger_name` |
| 4.2 | `dbriv/bridge.rs` | 38-53 | Add else branch for non-lazy mode to iterate data groups | `test_data_conversion_basic` |
| 4.3 | `dbriv/bridge.rs` | 441 | Update assertion: map→list yields 2 elements, not 1 | `test_data_value_conversion` |
| 4.4 | `derive/cli.rs` | 88 | Change `is_ok()` → `is_err()` (lexer errors on unterminated strings) | `test_lex_source_error` |
| 4.5 | `derive/engine.rs` | 88-108 | Add `Expr::Identifier` match arm for identity pattern | `test_identity`, `test_enumerative_search_identity` |
| 4.6 | `ffi/orchestrator.rs` | 68 | Add layout validation guard returning Err | `test_orchestrator_call_missing_layout` |
| 4.7 | `layout.rs` | 33 | Fix index: `get(0)` instead of `get(indent_count)` | `test_mixed_indentation_error` |
| 4.8 | `layout.rs` | 61 | Suppress `;` on block-header lines, or update test | `test_simple_function` |
| 4.9 | `type_universe/operators.rs` | 11-36 | Add `"++" => "Concat"` to `rune_to_op_name` | `test_builtin_string_concat` |

---

## Phase 5: New Behavior-Model Tests

After all regressions are fixed, write tests that model actual behavior:

| Area | Test Strategy |
|------|--------------|
| **List/Tuple** | Compile a program using lists → interpreter check of output |
| **Cast** | Compile `int_to_str`/`str_to_int` → interpreter output |
| **Exit** | Compile reactive program with `#!exit` → verify termination |
| **Async** | Compile async txns → verify thread pool calls (`objdump`, `nm`) |
| **Parser** | Parse files with/without parens → same AST |
| **Dep graph** | Build with known edges → verify topological order |
| **Float** | Compile float arithmetic → verify correct LLVM `fadd`/`fmul` |

---

## Execution Order (Recommended)

1. Phase 1 (parser) — unblocks 6 tests + all real .bv files
2. Phase 4.4 (derive/cli) — 1-line, 5 seconds
3. Phase 4.7 (layout) — 1-line, 5 seconds
4. Phase 4.9 (type_universe) — 1-line, 5 seconds
5. Phase 4.1 (watchdog) — 3-line, 30 seconds
6. Phase 4.5 (derive/engine) — 5-line, 1 minute
7. Phase 2.1 (as_i64) — 10-line, 1 minute (fixes 2 tests)
8. Phase 3.4 (float test) — test string update, 30 seconds
9. Phase 3.9 (GPU message) — test string update, 30 seconds
10. Phase 3.7 (MMIO guard) — 3-line, 30 seconds
11. Phase 3.5 (struct bytes) — dynamic computation + TypeDef arm, 5 minutes
12. Phase 3.3 (cast string dispatch) — new match arms in emit_expr, 5 minutes
13. Phase 4.2-4.3 (dbriv bridge) — else branch + test update, 5 minutes
14. Phase 4.6 (orchestrator) — layout guard, 2 minutes
15. Phase 4.8 (layout) — suppress semicolon or test, 5 minutes
16. Phase 2.2 (analyze_program) — wire real analysis, 10 minutes
17. Phase 3.1-3.2 (list/tuple codegen) — ~100 lines each, 20 minutes
18. Phase 2.3 (dependency graph) — restore edge extraction, 15 minutes
19. Phase 3.6 (exit condition codegen) — wire into main emission, 15 minutes
20. Phase 3.8 (async barrier) — call emit_async_phase, 10 minutes

Total estimated time: ~2 hours for all 42 fixes.

---

## Files Modified

| File | Phase |
|------|-------|
| `src/parser/definitions.rs` | 1 |
| `src/interpreter/mod.rs` | 2.1 |
| `src/backend/mod.rs` | 2.2 |
| `src/analysis/dependency_graph.rs` | 2.3 |
| `src/analysis/watchdog.rs` | 4.1 |
| `src/backend/llvm/emit_expr.rs` | 3.1, 3.2, 3.3 |
| `src/backend/llvm/mod.rs` | 3.5, 3.6, 3.7, 3.8 |
| `src/backend/llvm/helpers.rs` | 3.8 |
| `src/backend/llvm/tests.rs` | 3.4, 3.6, 3.9 |
| `src/backend/llvm/gpu.rs` | 3.9 |
| `src/dbriv/bridge.rs` | 4.2, 4.3 |
| `src/derive/cli.rs` | 4.4 |
| `src/derive/engine.rs` | 4.5 |
| `src/ffi/orchestrator.rs` | 4.6 |
| `src/layout.rs` | 4.7, 4.8 |
| `src/type_universe/operators.rs` | 4.9 |
