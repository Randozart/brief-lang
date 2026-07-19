# DRY Consolidation — LLVM Backend

**Date:** 2026-07-19
**Problem:** The LLVM backend has 5+ independent copies of the same code patterns (float boxing/unboxing, state field GEP+load, arena alloc consumption). Bugs fixed in one copy are missed in the others, causing LLVM IR type errors and wrong output.

**Discovered via:** The `emit_arena_alloc` return type changed from `ptr` to `i64`. 3 call sites were fixed, but 3 more still treat the result as `ptr` — confirmed buggy.

---

## Fix Order

### Phase 1: Fix 3 Live Bugs (CRITICAL — LLVM IR type errors)

These will produce LLVM verification errors at clang time. Fix immediately.

| # | File:Line | Bug | Fix |
|---|-----------|-----|-----|
| 1 | `helpers.rs:864,879` | `emit_concat_strings` heap path: `getelementptr i8, ptr %heap_buf, i64 0` + `ptrtoint ptr %heap_buf to i64` where `heap_buf` is i64 from `emit_arena_alloc` | `inttoptr i64 %heap_buf to ptr` before ptr use |
| 2 | `helpers.rs:971,1080` | `emit_write_header` + `emit_box_concat_result`: `ptrtoint ptr %result to i64` where `result` is i64 from `emit_arena_alloc` | Fix is in the CALLER — do the `inttoptr` before passing to these functions |
| 3 | `intrinsics.rs:308` | `_result = emit_arena_alloc(...)` — result discarded, register `v` never assigned | Assign `v = add i64 0, _result` or return `_result` directly |

### Phase 2: Consolidate Float Boxing/Unboxing

**Current state:** 3 independent functions do `trunc i64 → bitcast i32 → float`:

| Function | File | Pattern |
|----------|------|---------|
| `i64_to_float_reg` | `helpers.rs:585` | `next_reg_with_prefix` + cache |
| `native_float_or_box` | `emit_toplevel.rs:366` | manual counter + no cache |
| `ensure_typed_value("i64"→"float")` | `helpers.rs:2057` | `gen_reg` |

**Fix:** Delete `native_float_or_box` and `i64_to_float_reg`. Route all callers through the surviving function.

| Calls `i64_to_float_reg` | Calls `native_float_or_box` |
|--------------------------|----------------------------|
| `emit_expr.rs:104` (phi path) | `emit_toplevel.rs:373` (marshaling) |
| `emit_expr.rs:127` (field path) | `emit_toplevel.rs:388` (return) |
| `ssa.rs:528` (load_last_val_temps) | `emit_toplevel.rs:1047` (marshal param) |

**Plan:**
1. Pick a survivor: `ensure_typed_value` (most general, uses `gen_reg`, handles all types)
2. Re-route callers from `i64_to_float_reg` and `native_float_or_box` to `ensure_typed_value`
3. Delete the dead functions
4. Same for Float64 (`bitcast i64 to double`)

### Phase 3: Centralized State Field Access

**Current:** `emit_state_gep` exists at `helpers.rs:2031` but called only 2×. There is no `emit_state_field_load` or `emit_state_field_store`. The GEP+load+unbox pattern is hand-rolled ~44× across 8 files.

**Fix:** Add `emit_state_field_load(name)` and `emit_state_field_store(name, val)` that handle:
- GEP from field index
- Load i64
- Unbox float/double/bool (optional for load, auto for store)
- Return register name

Start with the loop engine files (counter.rs, ssa.rs, mod.rs) since these have the most duplication.

### Phase 4: Consolidate All Callers

After the helpers exist, migrate the remaining hand-rolled instances.
