# Struct Derivation (`<:`)

**Date added:** 2026-06-12  
**Phase:** 3

## Syntax

```briv
struct Point3D : Pair2D { z: Int; };             // single inheritance
struct BoundedList : Container<Int> { limit: Int; }; // generic parent
struct DeepDerived : Point3D { w: Int; };         // chain inheritance
```

## Rules

| Operation | Behavior |
|-----------|----------|
| **Upcast** `Child → Parent` | Implicitly allowed. Value slice — copies parent fields into a new value. |
| **Downcast** `Parent → Child` | Compile error. Cannot synthesize missing data. |
| **Field name collision** | Compile error: `"field 'x' already defined in parent 'A'"`. |
| **Multiple inheritance** | Not supported. Single parent chain only. |
| **Chain inheritance** | Supported. Fields cascade: `DeepDerived` has x, y, z, w. |

## AST

```rust
pub struct StructDefinition {
    pub parent: Option<Type>,    // NEW — parent type for derivation
    pub fields: Vec<StructField>,
    // ...
}
```

## Parser

`<:` is a single lexer token (`Token::LtColon`). After type parameters
and before `{`, the parser checks for `LtColon` and, if present, calls
`self.parse_type()` to read the parent type.

## Desugarer

A flattening pass in the desugarer recursively resolves the parent chain,
prepends parent fields to the child's field list, and checks for name
collisions. The parent link (`parent: Some(...)`) is preserved for type
system queries (upcast validation). After flattening, every struct's
field list is self-contained — backends see no derivation.
