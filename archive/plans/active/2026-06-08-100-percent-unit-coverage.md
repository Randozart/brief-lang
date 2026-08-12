# 100% Unit Coverage Drive

**Date**: 2026-06-08
**Status**: Implementation started
**Context**: 584 tests pass, 0 fail. 28 of 91 source files have zero tests (~76,742 total LOC). This plan fills all gaps.

## Convention

All tests go inline in each source file as `#[cfg(test)] mod tests { ... }`, matching existing codebase convention. Use AST construction (not parser round-trips) for interpreter/typechecker tests; use `tempfile` for filesystem tests (import_resolver, dbvl).

## Execution Order (by dependency + risk, ascending)

### Week 1: Small isolated modules (~34 tests)

```
Step 1  signal_graph      70 LOC   4 tests  ~20 min
Step 2  target_spec/loader 119 LOC   5 tests  ~30 min
Step 3  ffi/protocol       56 LOC   2 tests  ~15 min
Step 4  ffi/mapper         95 LOC   3 tests  ~20 min
Step 5  ffi/mappers       191 LOC   6 tests  ~45 min
Step 6  errors            645 LOC   8 tests  ~45 min
Step 7  annotator         589 LOC   6 tests  ~45 min
```

### Week 2: FFI infrastructure + core runtime (~52 tests)

```
Step 8   ffi/native_mapper  194 LOC   8 tests  ~1 hr
Step 9   ffi/orchestrator   223 LOC   8 tests  ~1 hr
Step 10  ffi/dynamic        233 LOC  10 tests  ~1.5 hr
Step 11  reactor            345 LOC  12 tests  ~1.5 hr
Step 12  import_resolver    502 LOC  14 tests  ~2 hr
```

### Week 3: Syntax gaps + backends (~30 tests)

```
Step 13  parser.rs <: syntax          14 tests  ~1.5 hr
Step 14  interpreter SubtypeOp gaps     7 tests  ~1 hr
Step 15  backend/llvm.rs gaps           9 tests  ~2 hr
```

### Week 4: Heavy modules (~42 tests)

```
Step 16  typechecker  1,947 LOC  30 tests  ~3 hr
Step 17  view_compiler  950 LOC  12 tests  ~1.5 hr
```

**Total: ~160 tests across 28 file changes**

---

## Detailed Test Specifications

### Step 1 — `src/signal_graph.rs` (70 LOC, 4 tests)

| Test | What It Verifies |
|------|------------------|
| `test_subscribe_adds_subscriber` | Signal → subscriber mapping created |
| `test_update_signal_notifies_subscribers` | Returns list of dependent txn names |
| `test_get_value_returns_stored_value` | After update, value retrievable |
| `test_clear_subscribers_removes_all` | All subscribers cleared |

### Step 2 — `src/target_spec/loader.rs` (119 LOC, 5 tests)

| Test | What It Verifies |
|------|------------------|
| `test_find_without_toml_ext` | `find("default")` appends `.toml` |
| `test_find_with_toml_ext` | `find("default.toml")` does not double-ext |
| `test_find_not_found_returns_none` | Unknown name → `None` |
| `test_project_root_default_paths` | `project_root()` returns default paths |
| `test_loader_with_custom_path` | `add_path` includes custom path in search |

### Step 3 — `src/ffi/protocol.rs` (56 LOC, 2 tests)

| Test | What It Verifies |
|------|------------------|
| `test_mapper_trait_object_safety` | Trait can be used as `Box<dyn Mapper>` |
| `test_ffi_value_debug_clone` | Derive traits compile and work |

### Step 4 — `src/ffi/mapper.rs` (95 LOC, 3 tests)

| Test | What It Verifies |
|------|------------------|
| `test_find_mapper_no_custom_path` | Delegates to registry |
| `test_create_mapper_registry_succeeds` | Default registry created |
| `test_describe_mapper_type` | Briev/Rust strings returned |

### Step 5 — `src/ffi/mappers.rs` (191 LOC, 6 tests)

| Test | What It Verifies |
|------|------------------|
| `test_registry_new_has_empty_mappers` | `MapperRegistry::new()` has zero mappers |
| `test_registry_add_search_path` | Search path appended correctly |
| `test_find_mapper_builtin_name` | Looking up known mapper (if any registered) |
| `test_find_mapper_not_found` | Unknown mapper → `None` |
| `test_mapper_info_debug_clone` | Derive traits work |
| `test_mapper_type_equality` | `MapperType::Briev == MapperType::Briev` |

### Step 6 — `src/errors.rs` (645 LOC, 8 tests)

| Test | What It Verifies |
|------|------------------|
| `test_span_new_and_dummy` | `Span::new()` sets fields; `dummy()` is 0,0,0,0 |
| `test_span_display` | `format!("{}", span)` → `"line:col"` |
| `test_span_format_with_source` | Line number, column pointer, source line |
| `test_diagnostic_verbose_format` | `format_verbose` includes all sections |
| `test_diagnostic_whisper_format` | `format_whisper` is one-liner |
| `test_diagnostic_builder_methods` | Chained `with_*` methods populate fields |
| `test_diagnostic_empty_code_format` | Empty code handled gracefully |
| `test_error_mode_enum` | `ErrorMode::Verbose != ErrorMode::Whisper` |

### Step 7 — `src/annotator.rs` (589 LOC, 6 tests)

| Test | What It Verifies |
|------|------------------|
| `test_analyze_empty_program` | Empty program → empty `call_paths` |
| `test_analyze_definition_no_calls` | Definition with no calls → empty calls vec |
| `test_analyze_definition_with_call` | Definition calling another function → `call_paths` populated |
| `test_analyze_nested_call` | Definition with nested call expressions |
| `test_analyze_guarded_call` | Call inside `[guard] { ... }` extracted |
| `test_analyze_guarded_assignment_call` | Call in assignment expression inside guard |

### Step 8 — `src/ffi/native_mapper.rs` (194 LOC, 8 tests)

| Test | What It Verifies |
|------|------------------|
| `test_drop_int_little_endian` | Writes `i64` value at correct offset |
| `test_drop_int_big_endian` | Writes `i64` with endian byte order reversed |
| `test_drop_buffer_overflow_error` | Offset + size > buffer → `Err` |
| `test_drop_bool_true_and_false` | Writes 1 for true, 0 for false |
| `test_drop_string_truncation` | Copies min(str.len, size) bytes |
| `test_fetch_field_by_size` | 1 byte → Bool; 4 bytes → Int; 8 bytes → Int |
| `test_fetch_field_underflow_error` | Offset + size > buffer → `Err` |
| `test_fetch_empty_layout_returns_void` | Empty fields → `FfiValue::Void` |

### Step 9 — `src/ffi/orchestrator.rs` (223 LOC, 8 tests)

| Test | What It Verifies |
|------|------------------|
| `test_orchestrator_new` | Default construction succeeds |
| `test_orchestrator_with_metro_hub` | `with_metro_hub()` sets hub |
| `test_is_metropolitan_target_match` | `ForeignTarget::Metropolitan` → true |
| `test_is_metropolitan_target_mismatch` | Other targets → false |
| `test_orchestrator_call_with_state` | Call with `&mut state` param populates state |
| `test_orchestrator_call_sentinel_validated` | Precondition fails → sentinel blocks call |
| `test_orchestrator_metro_hub_accessor` | `metro_hub()` returns the Arc |
| `test_orchestrator_sentinel_creation` | Sentinel created with default config |

### Step 10 — `src/ffi/dynamic.rs` (233 LOC, 10 tests)

| Test | What It Verifies |
|------|------------------|
| `test_frgn_type_from_name_valid` | `"Int"` → `FrgnType::Int`, `"Float"` → `FrgnType::Float`, etc. |
| `test_frgn_type_from_name_invalid` | `"Invalid"` → `None` |
| `test_wrap_ok_creates_result_enum` | `wrap_ok(Int, Value::Int(42))` → `Value::Enum("Result", "Ok", {value: 42})` |
| `test_wrap_err_creates_result_enum` | `wrap_err("msg")` → `Value::Enum("Result", "Err", {error: "msg"})` |
| `test_frgn_registry_register_and_lookup` | `register` + `call` with known name |
| `test_frgn_registry_unknown_function` | Unknown name → `RuntimeError` |
| `test_call_foreign_by_name_unsupported_sig` | Unsupported signature pattern → error |
| `test_frgn_registry_declaration_not_found` | Missing declaration → proper error message |
| `test_frgn_decl_display` | Verify `FrgnDecl` debug/display formatting |
| `test_frgn_type_equality_and_clone` | Derive traits work correctly |

### Step 11 — `src/reactor.rs` (345 LOC, 12 tests)

| Test | What It Verifies |
|------|------------------|
| `test_build_from_program_empty` | Empty program → zero transactions |
| `test_build_from_program_with_reactive_txn` | One `node` registered in reactor |
| `test_build_from_program_skips_non_reactive` | Regular `txn`/`defn` skipped |
| `test_dependency_map_populated` | Dependencies from txn inserted in `dependency_map` |
| `test_mark_dirty_propagates` | Variable update marks dependent txn dirty |
| `test_get_dirty_transactions` | Returns correct indices |
| `test_run_executes_txn` | Precondition true → body executes state changes |
| `test_run_skips_txn_pre_false` | Precondition false → body skipped |
| `test_escape_guard_detection` | Escape guard returns true when escape condition true |
| `test_run_escape_triggers_rollback` | Escape → state rolled back to `prior_state` |
| `test_run_max_iterations_rollback` | Exceeds max iterations → state rolled back |
| `test_run_returns_any_executed` | `run()` returns `Ok(true)` when txn fires |

### Step 12 — `src/import_resolver.rs` (502 LOC, 14 tests)

Uses `tempfile::TempDir` to create real file structures.

| Test | What It Verifies |
|------|------------------|
| `test_resolve_empty_import` | Import with empty path returns empty program |
| `test_resolve_bv_file` | Temp `.bv` file resolved and parsed |
| `test_resolve_checked_cached_modules` | Second resolve returns cached program |
| `test_import_css_file` | `.css` import returns `TopLevel::Stylesheet` |
| `test_import_svg_file` | `.svg` import returns parsed SVG content |
| `test_import_dbv_file` | `.dbv` DBriev file parsed by dbriev v2 parser |
| `test_import_dbvl_file` | `.dbvl` creates `Expr::DbvlTable` with path + fields |
| `test_strict_mode_propagation` | `with_strict_mode(true)` propagates to all programs |
| `test_filter_items_by_name` | `filter_items` selects only matching definitions |
| `test_filter_items_empty` | No import items returns full program |
| `test_search_path_ordering` | First search path wins |
| `test_resolve_ambiguous_extensions` | Both `.bv` and `.ebv` → error |
| `test_resolve_module_not_found` | Missing module → error message |
| `test_dbvl_path_injection` | `DbvlTable` path field populated by inject path |

### Step 13 — Parser `<:` Subtype Projection (in `src/parser.rs`, 14 tests)

Add to existing `mod tests` in `parser.rs`:

| Test | What It Parses |
|------|---------------|
| `test_parse_subtype_filter` | `items <: { FILTER(.active); }` |
| `test_parse_subtype_map` | `items <: { MAP(.x * 2); }` |
| `test_parse_subtype_sort` | `items <: { SORT(.name); }` |
| `test_parse_subtype_limit` | `items <: { LIMIT(10); }` |
| `test_parse_subtype_skip` | `items <: { SKIP(5); }` |
| `test_parse_subtype_unique` | `items <: { UNIQUE; }` |
| `test_parse_subtype_join` | `items <: { JOIN(other, .key); }` |
| `test_parse_subtype_group` | `items <: { GROUP(.category); }` |
| `test_parse_subtype_count` | `items <: { COUNT; }` → returns Int |
| `test_parse_subtype_sum` | `items <: { SUM(.price); }` |
| `test_parse_subtype_avg` | `items <: { AVG(.score); }` |
| `test_parse_subtype_min` | `items <: { MIN(.age); }` |
| `test_parse_subtype_max` | `items <: { MAX(.height); }` |
| `test_parse_subtype_match` | `email <: { MATCH("^(.+)@(.+)$"); }` |

### Step 14 — Interpreter SubtypeOp Gaps (in `src/interpreter.rs`, 7 tests)

| Test | What It Evaluates |
|------|------------------|
| `test_subtype_skip_on_list` | `Skip(2)` on `[1,2,3,4,5]` → `[3,4,5]` |
| `test_subtype_unique_on_list` | `Unique` on `[1,1,2,2,3]` → `[1,2,3]` |
| `test_subtype_sort_on_list` | `Sort(_)` sorts tuples by key |
| `test_subtype_join_two_lists` | Merge two lists on matching key |
| `test_subtype_avg_on_list` | `Avg(_)` on `[1,2,3,4]` → `2.5` |
| `test_subtype_min_on_list` | `Min(_)` on `[3,1,4,1,5]` → `1` |
| `test_subtype_max_on_list` | `Max(_)` on `[3,1,4,1,5]` → `5` |

### Step 15 — LLVM Backend Gaps (in `src/backend/llvm.rs`, 9 tests)

Each constructs a `Program`, calls `LLVMBackend::generate()`, asserts IR string output.

| Test | What It Verifies |
|------|------------------|
| `test_slice_full_range_emitted` | `list[2..5]` produces GEP + loop |
| `test_slice_with_stride_emitted` | `list[::2]` produces stride logic |
| `test_slice_with_mask_emitted` | `list[; x > 0]` produces filter loop |
| `test_multislice_2d_emitted` | `matrix[row][col]` produces 2D index |
| `test_map_literal_emitted` | `{"a": 1}` produces LLVM init (or fallback stub) |
| `test_set_literal_emitted` | `{1, 2, 3}` produces LLVM init (or fallback stub) |
| `test_arrow_transfer_emitted` | `&dest <- &source` produces transfer loop |
| `test_projection_keys_stub` | `map :> Keys` falls to stub (returns 0) |
| `test_projection_contains_stub` | `map :> Contains("k")` falls to stub (returns 0) |

### Step 16 — `src/typechecker.rs` (1,947 LOC, 30 tests)

| Test | What It Verifies |
|------|-----------------|
| `test_check_program_empty` | Empty program produces no errors |
| `test_check_basic_definition` | `defn foo -> Int { term 42; }` passes |
| `test_check_definition_type_mismatch` | `defn foo -> Bool { term 42; }` → `TypeMismatch` |
| `test_check_undefined_variable` | `let x = y` with `y` undefined → error |
| `test_check_assignment_type_mismatch` | `let x: Int = "hello"` → error |
| `test_check_state_decl_initial_value` | `let x: Int = 5;` passes |
| `test_check_state_decl_uninitialized_warning` | `let x: Int;` → warning (no error) |
| `test_check_signature_registration` | `sig foo(x: Int) -> Int` registers in signatures |
| `test_check_transaction_basic` | `node [x > 0][x == 0] { ... }` passes |
| `test_check_transaction_invalid_precondition` | Precondition type not Bool → error |
| `test_check_transaction_invalid_postcondition` | Postcondition type not Bool → error |
| `test_check_frgn_binding_basic` | `frgn foo(x: Int) -> Int from "test";` passes |
| `test_check_frgn_binding_location_override` | Regression guard: `from "real::path"` not overwritten |
| `test_check_enum_variant_registration` | Enum variants tracked in `enum_variants` |
| `test_check_struct_field_registration` | Struct fields tracked in `struct_fields` |
| `test_check_foreign_binding_from_signature` | Correct `check_frgn_binding` call with TOML path |
| `test_check_type_inference_binary` | Type inference for `+`, `-`, `*`, etc. |
| `test_check_type_inference_comparison` | Comparison produces Bool |
| `test_check_contract_bound_types` | `Int[0..100]` type is validated |
| `test_check_geometry_compatible` | `check_geometry` with matching types |
| `test_check_geometry_incompatible` | `check_geometry` with type mismatch |
| `test_check_stdlib_signatures_registered` | `to_json`, `from_json`, `new_builder` etc. present |
| `test_check_expression_ffi_errors` | `check_expr_for_ffi_errors` traverses calls |
| `test_check_statement_let` | Check `let` binding type inference |
| `test_check_statement_guarded` | Guarded statement types checked |
| `test_check_statement_term` | `term` return type matches signature |
| `test_check_program_with_import_types` | Type from resolved imports resolved |
| `test_check_subtype_projection_basic` | `<:` SUBTYPE ops type-checked |
| `test_check_constant_declaration` | `const X: Int = 5;` registers in scope |
| `test_check_diagnostics_collection` | `get_diagnostics()` returns accumulated warnings |

### Step 17 — `src/view_compiler.rs` (950 LOC, 12 tests)

| Test | What It Verifies |
|------|-----------------|
| `test_register_signal_and_transaction` | Register + lookup round-trip |
| `test_compile_basic_html_no_directives` | Plain HTML passes through unmodified |
| `test_compile_b_text_directive` | `b-text="signal"` creates `Text` binding |
| `test_compile_b_show_directive` | `b-show="expr"` creates `Show` binding |
| `test_compile_b_hide_directive` | `b-hide="expr"` creates `Hide` binding |
| `test_compile_b_trigger_directive` | `b-trigger:click="txn"` creates `Trigger` binding |
| `test_compile_b_class_directive` | `b-class` creates `Class` binding |
| `test_compile_b_attr_directive` | `b-attr:src="val"` creates `Attr` binding |
| `test_compile_b_style_directive` | `b-style:color="red"` creates `Style` binding |
| `test_compile_b_each_directive` | `b-each="item in list"` creates `Each` binding |
| `test_compile_inject_ids_adds_id_attr` | Elements with directives get auto-generated IDs |
| `test_compile_empty_html` | Empty string → no bindings, empty result |
