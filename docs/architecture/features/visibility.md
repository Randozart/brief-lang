# Visibility System (`pvt`, `sed`)

**Date added:** 2026-06-12  
**Phase:** 2

## Syntax

```brief
// Struct fields
struct BTree<T> {
    pvt  root: Ptr<Node<T>>;      // struct boundary only
    sed  cache: HashMap<T, Int>;   // file boundary
    size: Int;                     // public (default)
};

// Top-level items
sed defn helper() -> Int { ... }         // not importable
sed let BOUND: Int = 100;                // not importable
sed txn increment(...) -> Int { ... }    // not importable
sed struct Buffer { ... };               // type name unexported
```

## Keywords

| Keyword | Scope | Applies to |
|---------|-------|------------|
| `pvt` (struct-private) | Struct boundary (nested txns/defns only) | Struct fields |
| `sed` (file-private) | File boundary (same `.bv` file) | Struct fields, top-level items |

## Typechecking

- `enforce_field_visibility()` checks `Sedentary` fields at field access
- Cross-file access to `sed` fields emits `TypeMismatch` error
- `Private` enforcement is stubbed (requires `current_struct` tracking)

## Import Resolver

- Top-level `sed` items are filtered from exported symbols
- `filter_items()` excludes sed items from wildcard and named imports
- Sed names tracked via `Parser.sed_item_names` (no AST changes)

## Evaluation

- `Visibility::Public` — accessible from anywhere
- `Visibility::Sedentary` — accessible only within defining file
- `Visibility::Private` — accessible only from within the struct

## Example

See `examples/visibility-demo.bv` for a complete walkthrough of all three visibility levels.
