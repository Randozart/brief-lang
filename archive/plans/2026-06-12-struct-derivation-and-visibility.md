# Plan: Call Argument Check + Struct Derivation + Visibility System

**Created:** 2026-06-12T07:20Z  
**Updated:** 2026-06-12T15:00Z — implementation complete, 769 tests passing  
**Status:** Complete — all phases implemented and merged  
**Source:** officina-cli design session — structural typing analysis, visibility ergonomics, bug audit

---

## Overview

Three pieces addressing correctness gaps and missing ergonomics in Briv's type system:

1. **Bug Fix: Call argument type checking** — the typechecker currently does not validate
   that argument types match parameter types at call sites. A `String` where `TargetOS`
   is expected sails through silently and fails at runtime. (Phase 1 — correctness)
2. **Visibility System (`pvt`, `sed`)** — opt-in file-as-module boundaries for struct fields
   and top-level items. (Phase 2 — encapsulation)
3. **Struct Derivation (`struct B <: A`)** — compile-time zero-cost field flattening for
   layout reuse. (Phase 3 — ergonomics)

Phases 2 and 3 compose naturally — derived structs inherit visibility modifiers on parent
fields, and `sed` on a struct cascades its file-boundary protection to all fields.

---

## Phase 1: Call Argument Type Checking (Bug Fix)

### The Problem

The typechecker does not validate that argument types match parameter types at call sites.

In `check_expr_for_function_calls` (line 890), `Expr::Call(func_name, args)` invokes
`verify_term_function_call` which only does **symbolic postcondition verification** —
it checks whether the function's postcondition can be proven, but never checks whether
the types of the actual arguments are compatible with the declared parameter types.

Similarly, `infer_expression` (line 1315) for `Expr::Call` infers the return type but
never validates the argument types against the function signature.

**Impact:** A `String` passed where `TargetOS` is expected (or any other type mismatch
at a call site) compiles silently and fails at runtime with a panic, match error, or
silent data corruption.

**Briv's philosophy demands this is caught at compile time.** Silent runtime bugs
from preventable type errors are unacceptable.

### The Fix

**One new method** in `src/typechecker.rs`:

```rust
fn check_call_argument_types(&mut self, func_name: &str, args: &[Expr]) {
    // Look up the callee's parameter types
    let params: Option<Vec<Type>> = if let Some(defn) = self.definitions.get(func_name) {
        Some(defn.parameters.iter().map(|(_, t)| t.clone()).collect())
    } else if let Some(sig) = self.signatures.get(func_name) {
        Some(sig.params.iter().map(|(_, t)| t.clone()).collect())
    } else if let Some(fb) = self.foreign_bindings.get(func_name) {
        // Foreign bindings have input_layout with positional type info
        fb.input_layout.as_ref().map(|layout| {
            layout.iter().map(|(_, ty)| ty.clone()).collect()
        })
    } else {
        None // Unknown function or intrinsic — skip
    };

    let params = match params {
        Some(p) => p,
        None => return,
    };

    for (i, param_type) in params.iter().enumerate() {
        if i >= args.len() {
            break; // arg count mismatch handled elsewhere
        }
        let arg_type = self.infer_expression(&args[i]);
        if !self.types_compatible(&arg_type, param_type) {
            self.errors.borrow_mut().push(TypeError::TypeMismatch {
                expected: param_type.clone(),
                found: arg_type,
                span: None,
            });
        }
    }
}
```

**One call site** — in `check_expr_for_function_calls` (line 893), add:

```rust
Expr::Call(func_name, args) => {
    self.verify_term_function_call(func_name, args);
    self.check_call_argument_types(func_name, args);  // NEW
    for arg in args {
        self.check_expr_for_function_calls(arg);
    }
}
```

### What it leverages that already exists

| Component | Location | Used for |
|-----------|----------|----------|
| `definitions: HashMap<String, Definition>` | typechecker.rs:56 | `Definition.parameters: Vec<(String, Type)>` |
| `signatures: HashMap<String, Signature>` | typechecker.rs:55 | `Signature.params: Vec<(String, Type)>` |
| `foreign_bindings: HashMap<String, ForeignSignature>` | typechecker.rs:59 | `input_layout: Option<Vec<(String, Type)>>` |
| `infer_expression(&self, expr: &Expr) -> Type` | typechecker.rs:1207 | Infers type of each argument |
| `types_compatible(&self, a: &Type, b: &Type) -> bool` | typechecker.rs:1955 | Compares arg type vs param type |
| `TypeError::TypeMismatch { expected, found, span }` | — | Error variant for reporting |

### Error Reporting

When `types_compatible` returns `false`, the error message is:

```
Type mismatch in call to 'translate_to_spanish':
  argument 0: expected TargetOS, found String
```

This uses the existing `TypeError::TypeMismatch` infrastructure and the type's
`Display` implementation (which prints user-friendly type names).

### Tests

| Test | What it checks |
|------|---------------|
| `test_typecheck_call_arg_mismatch` | Passing `String` where `TargetOS` expected → error |
| `test_typecheck_call_arg_match` | Passing correct type → no error |
| `test_typecheck_call_unknown_fn` | Calling undeclared function → no crash (skip) |
| `test_typecheck_frgn_call_arg_types` | frgn call with type mismatch → error |

### Files Changed

| File | Change | Lines |
|------|--------|-------|
| `src/typechecker.rs` | Add method + call site | ~30 |
| Tests | New test module | ~25 |

---

## Phase 2: Struct Field + Top-Level Visibility

### Syntax

```briv
// Top-level items — file-as-module boundary
sed defn helper() -> Int { ... }         // not importable
sed let BOUND: Int = 100;                // not importable
sed txn increment(...) -> Int { ... }    // not importable
sed node on_tick [...] { ... }        // triggers only in this file
sed trg internal_event;                  // fires only in this file
sed struct Buffer { ... };               // type name unexported

// Struct fields — two levels of restriction
struct BTree<T> {
    pvt  root: Ptr<Node<T>>;      // struct boundary only
    sed  cache: HashMap<T, Int>;   // file boundary
    size: Int;                     // public (default)
};
```

### Keyword Reference

| Keyword | Full Forms | Scope | Applies to |
|---------|-----------|-------|------------|
| `pvt` | `pvt` `PVT` `private` `PRIVATE` | Struct boundary (nested txns/defns only) | Struct fields |
| `sed` | `sed` `SED` `sedentary` `SEDENTARY` | File boundary (same `.bv` file) | Struct fields, top-level defn/txn/trg/let/struct |

### File-as-Module Model

- Every `.bv` file is implicitly a module.
- Names declared without `sed` are public — importable from other files.
- Names declared with `sed` are file-private — the import resolver filters them
  out of the exported symbol table.
- `sed` items CAN call public code from other files. The restriction is on
  *being called from* outside, not on *calling out*.
- `sed` on a struct cascades to all its fields. If a struct is file-private,
  all its fields are implicitly `sed` (unless `pvt`, which is stricter).
  The compiler does not error on redundant `sed` field annotations — it just
  marks them `sed`.

### Lexer Changes

**`src/lexer.rs`** — Add two new token definitions (~line 175):

```rust
#[token("pvt")]
#[token("PVT")]
#[token("private")]
#[token("PRIVATE")]
Pvt,

#[token("sed")]
#[token("SED")]
#[token("sedentary")]
#[token("SEDENTARY")]
Sed,
```

Add Display variants:
```rust
Token::Pvt => write!(f, "pvt"),
Token::Sed => write!(f, "sed"),
```

`pvt` and `sed` are reserved keywords — using them as general identifiers
produces a parse error, matching `txn`, `defn`, `rct`, etc.

### AST Changes

**`src/ast.rs`** — Add `Visibility` enum and field on `StructField`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Sedentary,     // file boundary
    Private,       // struct boundary
}

pub struct StructField {
    pub name: String,
    pub ty: Type,
    pub default: Option<Expr>,
    pub visibility: Visibility,     // NEW
}
```

Top-level items do NOT need a new AST field. The parser groups `sed` as a
modifier that the compiler driver reads during symbol registration.

### Parser Changes

**`src/parser.rs`** — In all three struct field parsing paths:

1. **Regular fields** (line 1561-1586): Before `self.expect_identifier()`, peek
   for `Pvt`/`Sed` token. If found, consume and set `visibility`.
2. **`let`-prefix fields** (line 1596-1624): Same pattern — parse `pvt`/`sed`
   before the identifier (but after `let`, before field name).
3. **Struct variant fields** (line 1683-1733): Same pattern.

For top-level items, the parser checks for `sed` at the start of
`parse_top_level()` before dispatching to `parse_defn`/`parse_txn`/etc.
A boolean flag `is_sed` is passed through, stored as a modifier on the item.

### Typechecker Changes

**`src/typechecker.rs`**:

New tracking state:
```rust
struct_field_visibility: HashMap<String, HashMap<String, Visibility>>,
struct_files: HashMap<String, PathBuf>,
current_struct: Option<String>,
```

Populated alongside `struct_fields` at line 618-624. When a struct is `sed`,
all its fields are forced to `Sedentary` (unless `Private` on the field itself).

At field access (line 1427-1438), after field type lookup:
- `Public` → always allowed
- `Sedentary` → check `struct_files[struct_name] == current_file`; reject with
  `"field '{field}' of '{struct}' is sedentary"` on mismatch
- `Private` → check `current_struct == Some(struct_name)`; reject with
  `"field '{field}' of '{struct}' is private to the struct"` on mismatch

### Import Resolver Changes

**`src/import_resolver.rs`** — When building the symbol table for an imported
file, filter out any item marked `sed`. This prevents cross-file reference
at the symbol resolution level, before the typechecker even runs on uses.

### Constructor Sites

Every `StructField { name, ty, default }` in the codebase needs
`visibility: Visibility::Public`:

| File | Lines | Count |
|------|-------|-------|
| `src/parser.rs` | 1582, 1620, 1706, 1733, 1794, 1838 | 6 |
| `src/backend/llvm/tests.rs` | 2018-2019 | 2 |
| `src/fuzzing/ast_generator.rs` | 219 | 1 |
| `src/dbriv/bridge.rs` | 106 | 1 |

### Tests

| Test | What it checks |
|------|---------------|
| `test_visibility_default_public` | No keyword → `Visibility::Public` |
| `test_visibility_pvt_keyword` | `pvt` field → `Visibility::Private` |
| `test_visibility_sed_keyword` | `sed` field → `Visibility::Sedentary` |
| `test_visibility_sed_same_file` | Same-file access to sed field → allowed |
| `test_visibility_sed_cross_file` | Cross-file access to sed field → error |
| `test_visibility_pvt_same_struct` | Nested txn accesses pvt field → allowed |
| `test_visibility_pvt_cross_struct` | Other struct's txn accesses pvt → error |
| `test_visibility_sed_struct_cascade` | `sed struct` → all fields are Sedentary |
| `test_visibility_sed_top_level` | `sed defn` not present in imported symbols |
| `test_visibility_sed_calls_out` | `sed defn` can call cross-file public code |

---

## Phase 3: Struct Derivation

### Syntax

```briv
struct Point3D <: Pair2D { z: Int; };             // single inheritance
struct BoundedList <: Container<Int> { limit: Int; }; // generic parent
struct DeepDerived <: Point3D { w: Int; };         // chain inheritance
```

### Rules

| Operation | Behavior |
|-----------|----------|
| **Upcast** `Child → Parent` | Implicitly allowed. Value slice — copies parent fields into a new value, discards extras. |
| **Downcast** `Parent → Child` | Compile error. Cannot synthesize missing data. |
| **Field name collision** | Compile error: `"field 'x' already defined in parent 'A'"`. |
| **Multiple inheritance** | Not supported. Single parent chain only. |
| **Chain inheritance** | Supported. Fields cascade: `DeepDerived` has x, y, z, w. |
| **Derived struct capabilities** | Full: fields, nested txns/defns, variants, visibility modifiers. |
| **`pvt`/`sed` on parent fields** | Preserved through flattening. A `sed` field in the parent remains `sed` in the child. |

### AST Changes

**`src/ast.rs`** — Add `parent` to `StructDefinition`:

```rust
pub struct StructDefinition {
    pub name: String,
    pub type_params: Vec<String>,
    pub parent: Option<Type>,       // NEW
    pub fields: Vec<StructField>,
    pub transactions: Vec<Transaction>,
    pub view_html: Option<String>,
    pub span: Option<Span>,
    pub modifiers: Vec<Hashtag>,
    pub variants: Vec<StructVariant>,
}
```

The parent is stored as `Option<Type>` so generic instantiation works
(e.g., `Container<Int>` is `Type::Custom("Container")` with type args).

### Parser Changes

**`src/parser.rs`** — In `parse_struct()` (line 1530), after parsing type params
(line ~1548) and before the opening brace, check for `<:`:

```
struct Name<Params>  <:  ParentType  {  fields  }
                     ^^
                Token::LtColon
```

If `Token::LtColon` matches, consume it and call `self.parse_type()`.
Store result in `parent`. No `parent` → `parent: None`.

### Desugarer Changes

**`src/desugarer.rs`** — The `desugar()` method already builds a
`struct_defs: HashMap<String, &StructDefinition>` at line 122. Add a new pass
before the existing item loop:

1. **Topological resolve**: For each struct with a parent, recursively resolve
   the parent chain. Cycle detection → compile error.
2. **Monomorphize**: If parent is generic (e.g., `Container<Int>`), substitute the
   child's type arguments into the parent's field types.
3. **Flatten**: Prepend parent fields to the child's field list — shallow copy.
   Do NOT modify the parent struct definition itself (parent may be reused by
   multiple children).
4. **Collision check**: If any parent field name matches a child field name,
   emit compile error.
5. **Preserve parent link**: Keep `parent: Some(...)` in the output AST for
   type system queries (upcast validation, `:> Type`).

Child structs with no parent are untouched. After this pass, every struct field
list is flat and self-contained.

### Typechecker Changes

**`src/typechecker.rs`** — Upcast validation:

When checking assignment or argument passing, if expected type is `A` and actual
type is `B`, walk `B`'s parent chain. If `A` is an ancestor of `B`, allow the
assignment (implicit upcast slice).

Downcast (`A` → `B` where `B <: A`) is a type mismatch — compile error.

### Interpreter / Backend Impact

Minimal. Fields are flat after desugaring. Struct creation, field access, and
field assignment all work on the same layout as always. The desugarer runs
before codegen, so every backend sees a flat struct.

### Tests

| Test | What it checks |
|------|---------------|
| `test_struct_derivation_basic` | `B <: A { z }` → B has fields from A + z |
| `test_struct_derivation_generic` | `Container<T>`, `BC <: Container<Int>` → field monomorphized |
| `test_struct_derivation_chain` | `C <: B <: A` → C has all fields |
| `test_struct_derivation_collision` | Field name matches parent → error |
| `test_struct_upcast_implicit` | Child value assigned to parent var → allowed |
| `test_struct_downcast_rejected` | Parent value assigned to child var → error |

---

## Interaction Between Features

```briv
struct Pair2D {
    sed internal_tag: Int;
    x: Int;
    y: Int;
};

struct Point3D <: Pair2D {
    z: Int;
};
// → Point3D has { internal_tag (sed), x, y, z }
// → internal_tag retains its sed visibility from Pair2D
// → Same-file code accessing p3.internal_tag is fine
// → Cross-file access blocked
```

No special interaction logic needed — visibility is a per-field attribute
that survives field flattening intact.

---

## Implementation Order

| Phase | Step | Description | Files | Est. lines |
|-------|------|-------------|-------|-----------|
| **1** | 1 | Typechecker: `check_call_argument_types()` method + call site | typechecker.rs | 30 |
| **1** | 2 | Tests: argument type mismatch/match/unknown/frgn | tests | 25 |
| **2** | 3 | Lexer: Pvt, Sed tokens + Display | lexer.rs | 12 |
| **2** | 4 | AST: `Visibility` enum, field on `StructField` | ast.rs | 10 |
| **2** | 5 | Constructor sites: add `visibility: Public` to all 10 StructField usages | 4 files | 10 |
| **2** | 6 | Parser: visibility peek in struct field parsing (3 paths) | parser.rs | 28 |
| **2** | 7 | Parser: top-level `sed` flag for defn/txn/trg/let/struct | parser.rs | 15 |
| **2** | 8 | Typechecker: `struct_field_visibility`, `struct_files` maps + enforcement | typechecker.rs | 35 |
| **2** | 9 | Import resolver: filter `sed` items from exported symbols | import_resolver.rs | 12 |
| **2** | 10 | Tests: visibility tests (11 tests) | tests | 80 |
| **3** | 11 | AST: `parent: Option<Type>` on `StructDefinition` | ast.rs | 6 |
| **3** | 12 | Parser: `<:` parent type in `parse_struct()` | parser.rs | 8 |
| **3** | 13 | Desugarer: struct flattening pass | desugarer.rs | 45 |
| **3** | 14 | Typechecker: upcast validation (walk parent chain) | typechecker.rs | 20 |
| **3** | 15 | Tests: derivation tests (6 tests) | tests | 45 |
| — | 16 | Docs: architecture updates | 2 files | 40 |

**Total: ~16 steps, ~420 lines**

Note: Phases can be partially parallelized. Steps 3-5 (lexer/AST/sites) are independent
of any other phase. Steps 11-12 (derivation AST/parser) are independent of visibility
semantics. Only steps 1-2 (Phase 1) must be done first — correctness fix.

---

## Open Questions (Deferred)

1. **Pointer-level zero-cost upcast**: `let p2: &Pair2D = &p3;` — a pointer
   reinterpret that avoids the value copy. Useful for hot loops but not needed
   for initial implementation. Deferred.

2. **`sed` on enum variants or enum definitions**: Should an enum be markable
   as `sed`? The file-as-module model suggests yes — but enum derivation is
   not yet designed. Deferred until enum derivation work begins.

3. **Compile-time upcast cost**: The implicit upcast slice is a memcpy of the
   parent fields. For small structs this is trivially optimized by LLVM. For
   large structs in hot paths, a pointer reinterpret may eventually matter.
   Document as a known cost; revisit when benchmarks show a problem.

---

## Phase 1.5: Match/Uni Arrow Syntax (`=` → `->`)

**2026-06-12T15:00Z** — Implemented during Phase 2.4 work.

### Rationale

The `=` separator between pattern and body in `match` and `uni` was semantically
ambiguous with assignment. The `->` arrow reads more naturally as "pattern maps to
body" and is consistent with Briv's use of `->` for return types and swan songs.

### Changes

| Location | File | Lines |
|----------|------|-------|
| Match arm separator | `parser.rs:6140` | `self.expect(Token::Eq)` → `self.expect(Token::Arrow)` |
| Uni wildcard pattern | `parser.rs:4314` | same |
| Uni named variant pattern | `parser.rs:4376` | same |
| Uni simple pattern | `parser.rs:4391` | same |
| 6 match test strings | `parser.rs:7539-7651` | `= ` → `-> ` in source strings |
| 9 uni test strings | `parser.rs:7382-7523` | `= ` → `-> ` in source strings |

### Syntax After

```briv
match x {
    Some(v) -> expr1,
    None    -> expr2,
    _       -> fallback,
};

uni val(Some(v)) -> result;
uni x -> expression;
```
