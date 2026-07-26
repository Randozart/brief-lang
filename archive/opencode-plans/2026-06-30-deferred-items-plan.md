# Deferred Items Implementation Plan

**Date:** 2026-06-30  
**Status:** Draft  

This plan addresses three deferred items from the LLVM backend refactoring:

1. **Split `rest.rs`** — extract inline handlers into focused submodules
2. **TypeKey optimization** — replace `HashMap<String, ResolvedType>` with index-based access
3. **Promote built-in types** — migrate from hardcoded `init_primitives()` to explicit declarations

---

## Item 1: Split `rest.rs` (2,404 lines -> ~400 lines)

### Current State

`src/backend/llvm/expr/rest.rs` contains the `emit_rest_expr()` function with 35 top-level match arms. 29 of the 35 arms already use explicit `return` statements. Only 6 arms rely on the **fallthrough return pattern** -- they write to `v` and let the function's default return on line 2401-2403 handle the return:

```rust
// Default: treat as Int. Float operations are handled explicitly
// by emit_binop/emit_fcmp which return Type::Float/Bool respectively.
TypedRegister { name: v.to_string(), ty: Type::Int }
```

This return ALWAYS says `Type::Int`, which is wrong for handlers whose value is semantically a different type. Workaround: those handlers store the correct type elsewhere (e.g., `backend.emit_set_type`) or the downstream code ignores the return type.

### Fallthrough Handlers

| Handler | Lines | Risk | Fix |
|---------|-------|------|-----|
| `Expr::ListIndex` (line 675) | ~32 | Low | Add `return TypedRegister { name: v.to_string(), ty: Type::Int }` on `None` path |
| `Expr::Projection` (line 708) | ~209 | **Medium** -- 15/22 inner sub-targets fall through | Add `return` after the outer `ProjectionTarget` match block |
| `Expr::ObjectLiteral` (line 935) | ~15 | **BUG** -- missing `return`, acts like `StructInstance` but returns wrong type | Add `return TypedRegister { name: v.to_string(), ty: Type::Int }` |
| `Expr::FieldAccess` (line 951) | ~74 | Low -- deliberately returns `Type::Int` for non-Float fields | Add `return` with `Type::Int` |
| `Expr::PatternMatch` (line 1026) | ~11 | Low | Add `return` |
| `Expr::MultiSlice` (line 1038) | ~276 | Low | Add `return` |
| `Expr::Slice` (line 1382) | ~187 | Low | Add `return` |

**Note:** For Projection, all 15 fallthrough sub-targets return boxed i64 values, so `Type::Int` is correct. The fix is purely structural -- just needs explicit `return` before extraction.

### Submodule Extraction Plan

Extract into 5 new submodules under `src/backend/llvm/expr/`:

**Phase 1: `identifier.rs`** -- `emit_identifier()` (286 lines), `emit_owned_ref()` (4 lines), `emit_prior_state()` (32 lines), `emit_concat()` (2 lines). **Pure extraction, no risk.**

**Phase 2: `call.rs`** -- `emit_call()` (231 lines). FFI marshalling, pipe-syntax, variant enum construction, defn/txn calls. **Pure extraction, no risk.**

**Phase 3: `projection.rs`** -- `emit_projection()` (209 lines), 22 `ProjectionTarget` sub-handlers. **Must add `return` before extraction.** After fix: pure extraction.

**Phase 4: `arrow.rs`** -- `emit_arrow_mut_push()` (182 lines), `emit_arrow_mut_pop()` (124 lines), `emit_arrow_discard()` (80 lines), `emit_arrow_transfer()` (99 lines). Custom strategies, fast-path prealloc, slow-path alloc+memcpy. **Pure extraction, no risk.**

**Phase 5: `remaining.rs`** -- Remaining ~20 inline handlers that don't fit the above categories:
- StructInstance, ObjectLiteral, FieldAccess (struct/field access)
- PatternMatch, Match (pattern matching)
- Slice, MultiSlice (slice operations)
- MapLiteral, SetLiteral (collection literals)
- Cast, SubtypeProjection, IsType, FromCheck, Like, Block (type/metadata)
- CellCall, Within (control flow)

**Must fix 6 fallthrough handlers before extraction.**

### Total Effort

| Phase | Lines to Move | Risk | Est. Time |
|-------|--------------|------|-----------|
| identifier.rs | 324 | None | 15 min |
| call.rs | 231 | None | 10 min |
| projection.rs | 209 | Medium | 20 min |
| arrow.rs | 485 | None | 20 min |
| remaining.rs + bug fixes | 700 | Low | 30 min |
| **Total** | **~1,950** | | **~95 min** |

Final `rest.rs`: ~400 lines (dispatches + glue).

---

## Item 2: TypeKey Optimization

### Current State

`TypeUniverse` uses `HashMap<String, ResolvedType>`. ~33 HashMap operations across 7 files. `types` is `pub` -- external code accesses it directly (`universe.types.get(...)`, `universe.types.contains_key(...)`).

### Assessment: Premature

| Metric | Current | With TypeKey |
|--------|---------|-------------|
| Operations/compilation | ~100-500 | ~100-500 |
| Overhead/operation | ~50-100ns | ~5-10ns (index) |
| Total saved per compilation | ~5-50us | -- |
| Files touched | -- | 7 |
| Bug risk | -- | Moderate |

**Verdict: Defer until programs have >200 types or profiling shows >1% compile time in HashMap lookups.**

### Approach (If/When Implemented)

1. Add `type TypeKey = usize`
2. Add `Vec<ResolvedType> type_vec` -- parallel to HashMap
3. Add `HashMap<String, TypeKey> name_to_key` -- reverse lookup
4. Make `types` private -- force through new methods
5. Add `get_by_key(key: TypeKey) -> Option<&ResolvedType>` -- O(1) access
6. Thread `TypeKey` through `TypedRegister` -- carry key instead of re-looking-up

Files to modify: `type_universe.rs`, `ast.rs`, `typechecker.rs`, `helpers.rs`, `mod.rs` (LLVM), `emit_stmt.rs`, `emit_toplevel.rs`, `bindgen.rs`.

---

## Item 3: Promote Built-in Types to Explicit Declarations

### Current State

14 built-in primitives are hardcoded in `init_primitives()` (lines 178-308). Each `ResolvedType` has ~15 fields duplicated as struct literals. Every new property requires updating all 14 literals.

### Goal

Move properties into `.bv` declaration files. Rust handles bootstrap only.

### Approach

**Phase 1:** Create `lib/std/type_universe/bootstrap.bv` with declarations:

```
// Type annotation syntax (new parser feature):
// LLVM, Storage, TBAA, BoxOp, UnboxOp are annotation bindings
// distinguished from regular bindings by capitalization convention.

type Int <: Bits {
    bytes <~ 8,
    align <~ 8,
    llvm <~ "i64",
    storage <~ "Boxed",
    tbaa <~ "Int",
};

type Int8 <: Bits {
    bytes <~ 1,
    align <~ 1,
    llvm <~ "i8",
    storage <~ "Boxed",
    tbaa <~ "Int",
    box <~ sext.i8.to.i64,
    unbox <~ trunc.i64.to.i8,
};
```

Uses a new `<~` annotation binding syntax to distinguish codegen properties from regular type bindings. This prevents ambiguity with existing `Bytes: 1` style bindings.

**Phase 2:** Extend parser to accept `<~` annotation bindings.

Parser changes in `src/parser.rs`:
- New token `TildeArrow` (`<~`)
- New `annotation_bindings` field on `TypeDefBody`
- During `parse_typedef_body`, when seeing `<~`, parse as annotation (Expr on right side)

AST changes in `src/ast.rs`:
- Add `annotation_bindings: Vec<(String, Expr)>` to `TypeDefBody`
- Add `get_annotation(&self, name: &str) -> Option<&Expr>` helper

**Phase 3:** Shift `init_primitives()` to load from bootstrap file:

```rust
fn init_primitives(&mut self) {
    let bootstrap_src = include_str!("../../lib/std/type_universe/bootstrap.bv");
    let program = parse(bootstrap_src);
    for item in program.items {
        if let TopLevel::TypeDef(td) = item {
            self.resolve_type_def_as_primitive(td);
        }
    }
}
```

New `resolve_type_def_as_primitive()` reads annotations and constructs `ResolvedType` from them.

**Phase 4:** Delete hardcoded `Vec<ResolvedType>` literals.

### Implicit Import (Prelude Requirement)

The bootstrap file must be auto-loaded for every `.bv` compilation, equivalent to an implicit:

```
import "lib/std/type_universe/bootstrap.bv"
```

at the top of every file. This means:

1. **No explicit `import` needed** for built-in types (Int, Float, Bool, etc.) — they are always in scope
2. **The bootstrap types function as a prelude** — available in all programs without ceremony
3. **User-defined types can shadow** built-in names (the language allows it, though uncommon)
4. **No circular import issues** — bootstrap is loaded before user code, before any other imports
5. **Implementation approach**: During `build()`, process bootstrap declarations first (as `init_primitives()` does today), then process user TypeDefs. The bootstrap types remain the root of the type inheritance tree.

This preserves the current ergonomics where `Int`, `Float`, `Bool` etc. "just work" without imports, while moving their definitions from hardcoded Rust struct literals to proper source-level declarations.

### Risk Analysis

| Risk | Impact | Mitigation |
|------|--------|------------|
| Parser fails on `<~` syntax | Blocking | Add step by step; test each token |
| Circular dep: parser needs TypeSystem | Blocking | Bootstrap file uses only primitive syntax |
| Missing annotation causes crash | High | Validate every primitive has required annotations |
| Backward compat | Low | Annotations are additive |

### Effort

| Phase | Time |
|-------|------|
| bootstrap.bv file | 15 min |
| Parser: <~ annotation support | 45 min |
| Universe builder: read annotations | 30 min |
| Shift init_primitives() | 15 min |
| Remove hardcoded literals | 10 min |
| Tests | 30 min |
| **Total** | **~2.5 hr** |

---

## Implementation Order

### Phase A (Immediate): Fix rest.rs fallthroughs + ObjectLiteral bug
**Effort:** 30 min | **Depends on:** Nothing  
Fix all 6 fallthrough handlers with explicit `return`. Particularly the `ObjectLiteral` bug (missing return since initial Phase 6 extraction).

### Phase B (Next): Extract rest.rs submodules
**Effort:** 95 min | **Depends on:** Phase A  
rest.rs: 2,404 -> ~400 lines. 5 new submodules.

### Phase C (Optional): Promote built-in types
**Effort:** 2.5 hr | **Depends on:** Nothing architecturally  
14 primitives declared in `.bv` file. `init_primitives()` loads from source.

### Phase D (Deferred): TypeKey optimization
**Effort:** ~3 hr | **Depends on:** Profiling evidence  
Deferred until >200 types or profiling shows >1% compile time in lookups.

---

## Risk Register

| Risk | Phase | Likelihood | Impact | Mitigation |
|------|-------|-----------|--------|------------|
| Missing `return` in fallthrough handler | B | Medium | High | Fix returns BEFORE extraction |
| ObjectLiteral fix changes behavior | A | Low | Low | Verify with existing tests |
| Parser rejects <~ annotations | C | Medium | High | Limit to simple key:value pairs |
| include_str path breaks in release | C | Low | Medium | Path resolved at compile time |
| TypeKey causes off-by-one | D | Low | High | Exhaustive fuzz before commit |
