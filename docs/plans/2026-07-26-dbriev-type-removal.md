# Remove `DbrievType` / `schema_aliases` — Clean Break from `.dbvs` Legacy

**Date:** 2026-07-26
**Status:** Plan

---

## 1. Motivation

`DbrievType`, `DbrievAlias`, `DbrievStruct`, `DbrievEnum`, and the entire
`schema_aliases: HashMap<String, DbrievType>` mechanism are architectural debt
from the `.dbvs` era. With `.dbvs` removed (Phase 4), these types exist solely
to carry forward a design that no longer serves its purpose.

The key insight: `cross_validate` (`src/analysis/schema_validator.rs`) uses
`schema_aliases` only as a **set of names** — the `DbrievType` values are
constructed purely to satisfy the HashMap type parameter, never read in the
validation logic. The same is true in the LLVM backend: `schema_aliases` is
threaded through the builder pipeline but only its key set is ever consulted.

The `.dbvs` era is over. These types should be replaced with V2-native
`SchemaDef` / `FieldType` / `DbrievDocument`.

---

## 2. What Changes

### 2.1 Remove Types from `src/dbriev/ast.rs`

Remove the entire `.dbvs`-era type hierarchy:

| Type | Lines | Replaced by |
|------|-------|-------------|
| `DbrievType` | 65-83 | V2 `FieldType` (already exists) |
| `DbrievAlias` | 124-127 | V2 `SchemaDef` + `DataEntry` |
| `DbrievStruct` | 218-221 | V2 `SchemaDef` |
| `DbrievEnum` | 224-228 | V2 `SchemaDef` (optional: not used) |
| `DbvsProgram` | 267-273 | V2 `DbrievDocument` |
| `DbrievRegister` | 52-63 | Not needed — hardware schema via `SchemaDef` |
| `DbvlProgram` | 278-280 | V2 `DbrievDocument` |
| `DbvlRecord` | 286-287 | Not needed — line entries via `DataGroup` |
| `ImportStmt` | 299 | V2 `doc.imports: Vec<String>` |

**Keep** in `ast.rs`:
- `DbrievContract` — still used by hardware tests
- `DbrievExpr` / `DbrievRecord` — referenced by the old parser V1 tests
  (actually, check if still needed after parser cleanups)

### 2.2 Remove `parse_dbvs` from `src/dbriev/parser.rs`

| Line | Code |
|------|------|
| 1242-1276 | Function `pub fn parse_dbvs` — remove entirely |
| 1280-1427 | Module `mod dbvs_tests` with 4 test functions — remove entirely |

### 2.3 Replace `schema_aliases` in LLVM Backend (`src/backend/llvm/context.rs`)

Current:
```rust
pub schema_aliases: HashMap<String, DbrievType>,
```

Replace with:
```rust
/// 2026-07-26: Replaced DbrievType with SchemaRef. The type annotation
/// was never read in production — only names matter for cross-validation.
/// Full schema information is available through the resolved DbrievDocument.
pub schema_alias_names: HashSet<String>,
```

### 2.4 Update `with_schema_aliases` (`src/backend/llvm/mod.rs`)

Change signature to accept `HashSet<String>` instead of `HashMap<String, DbrievType>`.

### 2.5 Update `cross_validate` (`src/analysis/schema_validator.rs`)

Change signature:
```rust
// Before:
pub fn cross_validate(
    schema_aliases: &HashMap<String, crate::dbriev::DbrievType>,
    target_addresses: &HashMap<String, u64>,
) -> Vec<Diagnostic> {

// After:
pub fn cross_validate(
    schema_alias_names: &HashSet<String>,
    target_addresses: &HashMap<String, u64>,
) -> Vec<Diagnostic> {
```

Update all test helpers to pass `HashSet<String>` instead of `HashMap<..., DbrievType>`.

### 2.6 Update `src/backend/llvm/tests.rs`

Remove or rename:
- `test_dbvs_import_aliases_loaded` — rename to `test_schema_aliases_loaded`, remove all
  `DbrievType` value constructions, use `HashSet<String>` instead

### 2.7 Update `src/dbriev/mod.rs`

Remove remaining re-exports of removed types. The `pub use` line currently
(re)exports `parse_dbvs` — already removed in Phase 4, but verify no stale
lines remain.

### 2.8 Comment fixes

| File | Line | Current | Replace |
|------|------|---------|---------|
| `src/target_spec/mod.rs` | 51 | `// .dbvs schema file` | `// .dbv schema file` |

---

## 3. Files Modified (Summary)

| File | Change |
|------|--------|
| `src/dbriev/ast.rs` | ~230 lines removed — remove `DbrievType`, `DbrievAlias`, `DbrievStruct`, `DbrievEnum`, `DbvsProgram`, `DbrievRegister`, `DbvlProgram`, `DbvlRecord`, `ImportStmt` |
| `src/dbriev/parser.rs` | ~190 lines removed — remove `parse_dbvs` function and `dbvs_tests` module |
| `src/backend/llvm/context.rs` | 1 line changed — `schema_aliases: HashMap<String, DbrievType>` → `schema_alias_names: HashSet<String>` |
| `src/backend/llvm/mod.rs` | 1 line changed — `with_schema_aliases` parameter type |
| `src/analysis/schema_validator.rs` | ~15 lines changed — function signature, test helpers, remove `DbrievType` imports |
| `src/backend/llvm/tests.rs` | ~100 lines changed — remove `DbrievType` constructions, rename test, use `HashSet` |
| `src/dbriev/mod.rs` | ~5 lines changed — remove re-exports of removed types |
| `src/target_spec/mod.rs` | 1 line changed — comment update |

---

## 4. Dependencies and Risks

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| `DbrievContract` is still used somewhere | Low | Keep it in `ast.rs` — it's separate from the `DbrievType` hierarchy |
| `DbrievExpr` / `DbrievRecord` used in V1 parser tests | Medium | Check if V1 parser tests still exist after parser.rs cleanup; if so, keep those types with a note |
| LLVM backend test fixtures reference types directly | Medium | All test fixtures construct `DbrievType` values inline; replace with `HashSet<String>` constructions |
| `cross_validate` caller signature mismatch | Low | Only called from `hardware_validator.rs` (already migrated in Phase 4) and from its own tests |

---

## 5. Commit Order

1. **`ast.rs`**: Remove old types, keep needed ones
2. **`parser.rs`**: Remove `parse_dbvs` and tests
3. **`context.rs` + `mod.rs`**: Replace `HashMap<String, DbrievType>` with `HashSet<String>`
4. **`schema_validator.rs`**: Update signature and tests
5. **`llvm/tests.rs`**: Update tests to use `HashSet<String>`
6. **`mod.rs`**: Clean re-exports
7. **`target_spec/mod.rs`**: Comment fix

Build + `cargo test --lib` after each commit. No commit may break the build.

---

## 6. Post-Merge

After this plan is committed and merged to `feat/data-briev`, the `.dbvs` legacy
will be fully excised from the compiler — no `DbrievType`, no `parse_dbvs`,
no `schema_aliases`, no comment references. The V2 parser (`v2.rs`) and its
native types (`FieldType`, `SchemaDef`, `DbrievDocument`) will be the sole
data representation throughout the compiler.
