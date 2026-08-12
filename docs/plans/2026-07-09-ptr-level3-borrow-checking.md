# Level 3 Ptr — Safe Borrow Checking for Briev

**Date:** 2026-07-09
**Status:** Plan — awaiting implementation
**Branch:** (new branch from `main`)
**Supersedes:** The unresolved `Expr::OwnedRef` / `&` semantics in the existing codebase
**Documentation:** Updates `docs/architecture/features/backend-dispatch.md` (pointer handling), `docs/learn/types.md` (Ptr type section), `spec/SPEC.md` (borrow rules)

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Motivation: Why & Needs Purpose](#2-motivation-why--needs-purpose)
3. [Design Overview](#3-design-overview)
4. [The Desugaring Contract](#4-the-desugaring-contract)
5. [Const Inference from Declaration Context](#5-const-inference-from-declaration-context)
6. [Dangling Prevention via Warnings](#6-dangling-prevention-via-warnings)
7. [Pointer Provenance for Parallel Txn Safety](#7-pointer-provenance-for-parallel-txn-safety)
8. [Current State Assessment](#8-current-state-assessment)
9. [Phase 1 — Type System Foundation](#9-phase-1--type-system-foundation)
10. [Phase 2 — Dereference Operator + Expr::Deref](#10-phase-2--dereference-operator--exprderef)
11. [Phase 3 — Borrow Warnings + Provenance Tracking](#11-phase-3--borrow-warnings--provenance-tracking)
12. [Phase 4 — Arrow Operations, Interpreter, Proof Engine](#12-phase-4--arrow-operations-interpreter-proof-engine)
13. [Phase 5 — Webstack + CIRCT Backends](#13-phase-5--webstack--circt-backends)
14. [Phase 6 — Tests, Benchmarks, Documentation](#14-phase-6--tests-benchmarks-documentation)
15. [Verification Gates](#15-verification-gates)
16. [Benchmark Baseline](#16-benchmark-baseline)

---

## 1. Executive Summary

Currently `&name` is a syntactic marker with zero type-level or codegen-level
distinction from `name`. It parses as `Expr::OwnedRef(name)`, types to `T`,
and in the LLVM backend it's immediately converted to `Expr::Identifier(name)`
— identical codegen. The `&` is decorative.

This plan gives `&` real meaning:

| Before | After |
|--------|-------|
| `&name: T` (same as `name`) | `&name: Ptr<T>` or `Ptr<const T>` |
| `Expr::OwnedRef(String)` — type `T` | `Expr::AddrOf(expr)` — type `Ptr<T>` |
| `*` operator doesn't exist | `*ptr: T` — dereference |
| No borrow checking | Warnings for dangling pointers |
| No pointer provenance | Compiler tracks which field each `Ptr` originates from |
| Webstack: `OwnedRef` unhandled (would crash) | Handled via shared `emit_addr_of` helper |
| CIRCT: `OwnedRef` partial | Handled via unified `ptr` path |

### What stays (unchanged)

- Arrow operation syntax: `&list <- val`, `val <- &list`, `<- &list`
- Assignment syntax: `&name = value`
- State field declaration syntax: `state { field: Int }`
- All existing `.bv` files — no syntax changes needed

---

## 2. Motivation: Why & Needs Purpose

The `&` marker is visually clear — it signals "this is a mutation target" at a
glance. But until now, it's purely cosmetic. The compiler infers the same
write sets from a bare `Identifier` as from `OwnedRef`. This creates a
justification gap: why require a syntactic marker that does nothing?

The answer: `&` is **address-of**. `&name` literally evaluates to the memory
address of `name`, typed as `Ptr<T>`. This gives the syntax real weight:
- The type changes (`Ptr<T>` vs `T`)
- The codegen changes (GEP vs value load)
- The safety guarantees emerge from pointer provenance tracking

### Why keep `&name = value` sugar

Without sugar, a mutation would require:
```briev
*(&counter) = counter + 1;   // dereference + assign — messy
&amp;counter = counter + 1;        // ampersand-in-a-row — visually confusing
```

With sugar:
```briev
&counter = counter + 1;       // clear: "I'm mutating counter"
```

The sugar desugars early (parser → AST) so the rest of the pipeline never sees
the complexity. The visual clarity of `&name = value` justifies the desugaring
rule, and the desugaring rule is what makes `&name` produce `Ptr<T>` everywhere
else (rvalue positions, FFI arguments, pointer passing).

---

## 3. Design Overview

### 3.1 Types

```rust
// src/ast.rs
Type::Applied("Ptr".into(), vec![inner_ty])
// vs
Type::Applied("Ptr".into(), vec![Type::Applied("const".into(), vec![inner_ty])])
// or more naturally: for const, use a marker
// For brevity in the type universe, just Ptr<T> and Ptr<const T> as two variants
```

In the TypeUniverse, `Ptr` is recognized as a built-in name (already the case).
The const qualifier is represented as `Ptr.const` or `Ptr<const T>`.

### 3.2 AST Nodes

```rust
pub enum Expr {
    // ...
    AddrOf(Box<Expr>),           // &expr — address of any expression
    Deref(Box<Expr>),            // *expr — dereference a pointer
    // OwnedRef(String) is REMOVED — replaced by AddrOf(Identifier(name))
}
```

- `AddrOf(Identifier("x"))` replaces `OwnedRef("x")` — identical in coverage
- `AddrOf(FieldAccess(...))`, `AddrOf(Index(...))` — address of a sub-expression
- `Deref(expr)` — read/write through a pointer

### 3.3 Parse Changes

```rust
// parser.rs — parse_unary, Token::Ampersand branch
// Before:
&name  →  OwnedRef(name)
// After:
&expr  →  AddrOf(expr)

// New: parse_unary, Token::Star branch
*expr  →  Deref(expr)
```

The `&` token generalizes from `&name` to `&expr`. This enables:
- `&name.field` → `AddrOf(FieldAccess(...))`
- `&name[i]` → `AddrOf(Index(...))`
- `&(a + 1)` → error (can't address an rvalue)

The validity check (`is_valid_addr_of_target`) rejects rvalues:
- `&Identifier` → valid (state field or let binding)
- `&FieldAccess(...)` → valid (field of a named variable)
- `&Index(...)` → valid (element of a named variable)
- `&BinaryOp(...)` → error ("cannot take address of a temporary value")
- `&Deref(...)` → valid (address of a dereferenced pointer — i.e., re-borrow)

### 3.4 Typechecker Changes

```rust
// typechecker.rs — infer_expression
Expr::AddrOf(inner) => {
    let inner_ty = self.infer_expression(inner)?;
    let name = if is_mutable_location(inner) { "Ptr" } else { "PtrConst" };
    Ok(Type::Applied(name.into(), vec![inner_ty]))
}
Expr::Deref(ptr) => {
    let ptr_ty = self.infer_expression(ptr)?;
    let Some(inner) = pointee_type(&ptr_ty) else {
        return Err(TypeError::InvalidDeref(ptr_ty));
    };
    Ok(inner)
}
```

`is_mutable_location` checks (flat — max 2 levels):

```rust
fn is_mutable_location(&self, expr: &Expr) -> bool {
    let Expr::Identifier(name) = expr else {
        return self.is_mutable_subexpr(expr);
    };
    // State fields and txn-scoped variables are mutable.
    // Let bindings are NOT mutable locations.
    self.is_state_field(name) || self.is_txn_variable(name)
}

fn is_mutable_subexpr(&self, expr: &Expr) -> bool {
    match expr {
        Expr::FieldAccess(base, _) => self.is_mutable_location(base),
        Expr::Index(base, _) => self.is_mutable_location(base),
        Expr::Deref(ptr) => !is_const_ptr(&self.infer_expression(ptr)),
        _ => false,
    }
}
```

### 3.5 LLVM Codegen

```rust
// llvm/expr/identifier.rs — emit_addr_of
// &field (state field) → GEP on %State*
// &let_binding → alloca + store (value into alloca, then address)
// &param → alloca copy + address (or use existing alloca if param is already in memory)

// llvm/expr/deref.rs — NEW FILE
// *ptr → load through the pointer (rvalue)
// *ptr = value → store through the pointer (lvalue in assignment context)
```

**SSA mode interaction:** When a field's address is taken (`&field`), the field
is forced out of SSA into memory (alloca + load/store). The `ssa_state_reg`
mode is incompatible with address-of — the loop engine detects this and falls
back to memory mode for fields with pending borrows.

---

## 4. The Desugaring Contract

### 4.1 What gets desugared

| Source expression | Desugared to | When |
|------------------|--------------|------|
| `&name = value` (Statement::Assignment) | `*(&name) = value` | Parser (or early desugar pass) |
| `&list <- value` (ArrowMut) | `*(&list) <- value` | Parser |
| `value <- &list` (ArrowPop) | `value <- *(&list)` | Parser |
| `<- &list` (ArrowDiscard) | `<- *(&list)` | Parser |

The desugaring happens **at the parser level** immediately after constructing
the AST node. The `&name` is parsed as `AddrOf(Identifier(name))` and then
the assignment/arrow node wraps it with an implicit `Deref`.

### 4.2 Why desugaring is essential

Without desugaring, every assignment would look like:
```briev
*(&counter) = counter + 1;
```

This is:
1. **Visually noisy** — `*(&` is three symbols where one (`&`) should suffice
2. **Mentally taxing** — the programmer must parse "dereference the address of counter" for every write
3. **Error-prone** — forgetting the `*` would store the pointer, not the value

The sugar is specifically for **assignment and arrow targets** — the places where
`&name` has historically been used as a mutation marker. In all other positions,
`&name` is a literal address-of and `*ptr` is the literal dereference.

### 4.3 Implementation strategy

```rust
// parser.rs — parse_statement, after parsing assignment
// Once the LHS is parsed as Expr::AddrOf(target), wrap with Deref:
fn parse_assignment_lhs(&mut self) -> Result<Expr, SyntaxError> {
    let mut expr = self.parse_unary()?;  // may produce AddrOf, Identifier, etc.
    // If it's an address-of expression used as LHS, wrap with Deref
    if is_addr_of(&expr) {
        // Keep as AddrOf — the assignment codegen will handle it
        // by dereferencing before storing
    }
    Ok(expr)
}
```

Actually, the cleanest approach is to keep `Statement::Assignment` and the arrow
statement variants working as they do today — the LHS can be `Expr::AddrOf` and
the codegen handles it by computing the address and storing through it. The sugar
is in the parser accepting `&name` on the LHS and wrapping it in `AddrOf`, then
the codegen recognizing that `*(AddrOf(...))` in an assignment context should
store through the pointer.

Re-conceptualization: The sugar is NOT "desugar to Deref(AddrOf(...))". Instead,
it's **the assignment/arrow codegen already knows how to handle AddrOf targets**
by computing the address and emitting a store. This is cleaner because:
- No `Deref(AddrOf(...))` nesting in the AST for the common case
- The `Assignment` / `ArrowMut` nodes keep their clean structure
- The codegen has a single path for "store through this address" regardless of
  whether the LHS is `AddrOf`, `Deref`, or something else

**Revised desugaring rule:**

```
Parser accepts &name = value as:
  Statement::Assignment { lhs: Expr::AddrOf(Identifier("name")), expr: value }

Codegen for Statement::Assignment with AddrOf lhs:
  1. Compute address (via emit_addr_of on the inner expr)
  2. Store value through that address (emit store)

This is exactly the same as:
  Statement::Assignment { lhs: Expr::Deref(Expr::AddrOf(Identifier("name"))), expr: value }
  
But without the Deref indirection in the AST.
```

So the "desugaring" is really a **codegen convention** — the `AddrOf` on the LHS
of an assignment is the "store through address" pattern, not the "dereference"
pattern. This avoids double-wrapping while keeping the semantics correct.

---

## 5. Const Inference from Declaration Context

The compiler infers `Ptr<T>` vs `Ptr<const T>` automatically based on where the
referent is declared:

| Declaration | `&name` produces | Rationale |
|-------------|-----------------|-----------|
| `state { x: Int }` | `Ptr<Int>` | State fields are mutable — writes are expected |
| `node { &x = ... }` | `Ptr<Int>` | Same — txn body mutation targets |
| `let x = 5` | `Ptr<const Int>` | Let bindings are single-assignment — write-through is meaningless |
| `defn f(p: Ptr<Int>)` | `Ptr<Int>` | Parameter declared mutable |
| `defn f(p: Ptr<const Int>)` | `Ptr<const Int>` | Parameter declared read-only |
| `defn f(x: Int)` | `Ptr<const Int>` | Value parameter — address taken is local const |

The inference is a simple declaration-context lookup in the symbol table.
No annotations needed from the programmer.

### 5.1 Pointer mutability rules

| Operation | `Ptr<T>` | `Ptr<const T>` |
|-----------|----------|----------------|
| `*ptr = value` (write) | ✅ | ❌ error: "cannot write through const pointer" |
| `let v = *ptr` (read) | ✅ | ✅ |
| `&*ptr` (reborrow) | ✅ → `Ptr<T>` | ✅ → `Ptr<const T>` |
| Pass to `fn(p: Ptr<Int>)` | ✅ | ❌ error: "expected Ptr<Int>, got Ptr<const Int>" |
| Pass to `fn(p: Ptr<const Int>)` | ✅ (coercion) | ✅ |

This mirrors Rust's `&T` vs `&mut T` rules, but with inference from context
rather than explicit annotations.

---

## 6. Dangling Prevention via Warnings

### 6.1 The only dangerous pattern

```briev
state { saved: Ptr<Int> };

node example {
    [true][true] {
        let temp = 5;
        &saved = &temp;      // ⚠️ DANGER: &temp points to a local
    };
};
```

The compiler detects this by tracking **pointer provenance** at the typechecker
level. When a `Ptr<T>` is created via `&expr`, the compiler records:

```rust
// New data structure — per-function/txn provenance map
struct ProvenanceInfo {
    // Maps a variable name to whether it's a state field or local
    source_kind: HashMap<String, SourceKind>,
    // Maps a pointer expression to its provenance
    ptr_provenance: HashMap<ExprId, Provenance>,
}

enum SourceKind {
    StateField,     // safe — lives forever
    LetBinding,     // ⚠️ local — may dangle
    Parameter,      // depends on caller — conservatively safe within the call
}

enum Provenance {
    PointsTo(ExprId),          // this pointer points to the location of `expr`
    Unknown,                   // provenance not determinable (e.g., from FFI)
}
```

### 6.2 Warning messages

```briev
&saved = &temp;
```

```
warning[W-PTR-LOCAL]: pointer to local may dangle
  ─> example.bv:6:18
  |
6 |         &saved = &temp;
  |                  ^^^^^ this creates Ptr<Int> to `temp`
  |
  = `temp` is a let binding — valid only within the current transaction body
  = `saved` is a state field — persists across transactions
  = storing a pointer to a local variable in a state field creates
    a dangling pointer after the current body exits
  |
  suggestion: restructure — store the value, not the pointer
    &saved_val = temp;    // copy value — always safe
  |
  suggestion: promote `temp` to a state field if it needs a stable address
    state { temp: Int; };
```

For `const` pointers to locals:

```briev
let r = &temp;       // Ptr<const Int> — no warning yet
let v = *r;          // read through — fine
// r's last use
```

No warning, because `Ptr<const T>` to a local is safe within the body. The
warning only fires when a pointer with local provenance **escapes** the
current body (stored in state, returned from txn).

### 6.3 Escape detection

The compiler scans `Statement::Assignment` and `Statement::Expression(ArrowMut)`
for LHS targets that are state fields, and checks whether the RHS is a pointer
with local provenance. This is a simple dataflow analysis within a single txn
body — no inter-procedural analysis needed for the initial implementation.

```rust
fn check_dangling(assignments: &[Statement], provenance: &ProvenanceInfo) -> Vec<Warning> {
    let mut warnings = vec![];
    for stmt in assignments {
        let Some(w) = check_dangling_stmt(stmt, provenance) else { continue; };
        warnings.push(w);
    }
    warnings
}

fn check_dangling_stmt(stmt: &Statement, provenance: &ProvenanceInfo) -> Option<Warning> {
    let Statement::Assignment { lhs, expr } = stmt else { return None; };
    let Expr::AddrOf(lhs_target) = lhs else { return None; };
    // Assigning to a pointer field: check if RHS is a local.
    let Expr::AddrOf(rhs_inner) = expr else { return None; };
    if !provenance.is_local(rhs_inner) { return None; }
    Some(Warning::DanglingPointer {
        pointer_target: lhs_target.clone(),
        local_source: rhs_inner.clone(),
    })
}
```

### 6.4 No hard errors — always restructure

The compiler NEVER produces a hard error for this — always a warning with a
restructure suggestion. The programmer fixes it by:
1. Copying the value instead of storing the pointer
2. Promoting the local to a state field
3. Changing the data flow so the pointer doesn't escape

This approach avoids the annotation burden of Rust's borrow checker while
providing the same safety outcome — the programmer is informed of the risk
and given clear paths to eliminate it.

---

## 7. Pointer Provenance for Parallel Txn Safety

### 7.1 The problem

When two transactions execute in parallel, a write through a pointer may alias
with a direct field write in the other transaction:

```briev
// Txn A                                     // Txn B
node write_via_ptr {                       node write_direct {
    [true][true] {                               [true][true] {
        let p = &counter;                            &counter = compute();
        *p = compute();
    };                                           };
};                                           };
```

The compiler must determine whether `*p` in Txn A aliases with `&counter` in
Txn B. If `p` points to `counter`, they conflict — serialization required.

### 7.2 Provenance tracking for parallel safety

The provenance map from §6 is reused here. When a `Ptr<T>` is created, it
carries its provenance through the expression tree:

```rust
enum Provenance {
    PointsTo(String),        // known target: points to field "counter"
    PointsToField {          // known field access: points to "obj.field"
        base: Box<Provenance>,
        field: String,
    },
    PointsToIndex {          // known index: points to "arr[i]"
        base: Box<Provenance>,
        index: Box<Provenance>,
    },
    Unknown,                 // provenance lost — e.g., read from state or FFI
}
```

When the parallel scheduler analyzes write sets:
- `&counter` → writes to `{counter}`
- `*p` where `p.provenance = PointsTo("counter")` → writes to `{counter}` (conflict)
- `*p` where `p.provenance = Unknown` → writes to `{unknown}` (conservatively conflicts with everything)

### 7.3 Conservative fallback

When provenance is `Unknown` (e.g., a `Ptr<T>` read from a state field whose
source was set by a different txn), the scheduler conservatively serializes
the transactions:

```
note: transaction `write_via_ptr` writes through a pointer with unknown provenance
  → this pointer may alias with fields written by any other transaction
  → serialized with all conflicting transactions for safety
  → this may impact parallel performance
  → to restore parallelism, ensure the pointer's source is traceable
```

This is sound — never incorrect, only potentially slower. The programmer can
restore parallelism by ensuring the pointer's provenance is known (e.g., by
not storing `Ptr<T>` in state fields, or by inlining the borrow).

### 7.4 Implementation strategy

The provenance is threaded through the typechecker as a companion data structure
to the typed AST. It's computed alongside type inference:

```rust
// New component: ProvenanceTracker in typechecker
fn infer_with_provenance(&mut self, expr: &Expr) -> (Type, Provenance) {
    match expr {
        Expr::AddrOf(inner) => {
            let (ty, prov) = self.infer_with_provenance(inner);
            (Type::Applied("Ptr".into(), vec![ty]), prov)
        }
        // ...
    }
}
```

The parallel scheduler (in `src/analysis/transition_graph.rs` or similar) reads
the provenance map to compute write sets.

---

## 8. Current State Assessment

### 8.1 What exists today

| Component | Status | File |
|-----------|--------|------|
| `Type::Applied("Ptr", ...)` | Full — type universe, byte_size, alignment | `type_universe.rs:816` |
| `Expr::OwnedRef(String)` | Full — parser, typechecker, interpreter, LLVM | `ast.rs:1639`, `parser.rs:7282` |
| `&name = value` assignment | Full — parser, typechecker, LLVM codegen | `parser.rs:6366` |
| `&list <- value` arrow ops | Full — parser, arrow handler | `parser.rs:6377-6426` |
| `.#Ptr` projection | Full — typechecker, codegen | `typechecker.rs:2414` |
| Ptr + Int arithmetic | Full — typechecker | `typechecker.rs:3085` |
| Ptr ↔ Int cast | Full — typechecker | `typechecker.rs:3278` |
| Webstack backend | **NOT** handled — `OwnedRef` would crash | `webstack.rs` |
| CIRCT backend | Partial — LHS assignment only | `circt.rs:547,726` |
| Proof engine | Partial — `OwnedRef` → `SymbolicValue::Unknown` | `proof_engine.rs:202` |
| `const` qualifier on Ptr | Does not exist | — |
| `Expr::Deref` | Does not exist | — |
| Borrow warnings | Does not exist | — |
| Provenance tracking | Does not exist | — |

### 8.2 What must change

| Change | Impact | Difficulty |
|--------|--------|------------|
| Replace `Expr::OwnedRef(String)` with `Expr::AddrOf(Box<Expr>)` | All match arms on `OwnedRef` | Medium — ~40 references |
| Add `Expr::Deref(Box<Expr>)` | New AST variant, new match arms | Medium — ~20 references |
| Add `*` token to lexer | New token, `parse_unary` arm | Easy |
| Generalize `&expr` in parser | Minimal — already accepts postfix | Easy |
| Change `OwnedRef` type inference to `Ptr<T>` | Typechecker inference | Medium |
| Add const inference from context | New helper `is_mutable_location` | Medium |
| LLVM codegen: `&field` → GEP | Already works in memory mode | Medium — SSA fallback |
| LLVM codegen: `*ptr` → load/store | New codegen path | Medium |
| Webstack: add `AddrOf` and `Deref` | New match arms | Medium |
| CIRCT: generalize from `OwnedRef` to `AddrOf` | New match arms | Easy |
| Borrow warnings for local→state escape | New analysis pass | Medium |
| Provenance tracking for parallelism | Companion data in typechecker | Hard |
| Proof engine: symbolic evaluation of Ptr | New `SymbolicValue` variant | Medium |
| Arrow ops: update to work with `AddrOf` | Codegen for arrow targets | Medium |

---

## 9. Phase 1 — Type System Foundation

### 9.1 Replace OwnedRef with AddrOf

**Files:** `src/ast.rs`, `src/parser.rs`, `src/typechecker.rs`, `src/interpreter.rs`

```diff
- Expr::OwnedRef(String),
+ Expr::AddrOf(Box<Expr>),
+ Expr::Deref(Box<Expr>),
```

Update ALL match arms on `Expr::OwnedRef(name)`:
- `from_expr` / `format_expr` → handle `AddrOf(inner)` (recursive)
- `extract_deps_recursive` → recurse into inner
- `normalize_to_old` → return None for both new variants
- `collect_identifiers` → recurse into inner
- `is_compile_time_expr` → false for both
- Proof engine → explicit handling

### 9.2 Generalize parser

**File:** `src/parser.rs`

`parse_unary` at the `Token::Ampersand` branch:
```diff
- &name → OwnedRef(name) + parse_postfix_expr
+ &expr → AddrOf(expr)
```

The parser already calls `parse_postfix_expr` after the identifier, so
`&name.field` and `&name[i]` already work as `AddrOf(Index(...))` etc.

Add `*expr` → `Deref(expr)` to `parse_unary`.

### 9.3 Add const to type universe

**File:** `src/type_universe.rs`

```rust
/// Ptr<T> — mutable pointer. PtrConst<T> — read-only pointer.
fn is_const_ptr(ty: &Type) -> bool {
    let Type::Applied(name, _) = ty else { return false; };
    name == "PtrConst"
}

/// Returns the pointee type for Ptr<T> or PtrConst<T>.
fn pointee_type(ty: &Type) -> Option<Type> {
    let Type::Applied(name, args) = ty else { return None; };
    if name != "Ptr" && name != "PtrConst" { return None; }
    args.first().cloned()
}
```

### 9.4 Const inference from context

**File:** `src/typechecker.rs`

```rust
fn is_mutable_location(&self, expr: &Expr) -> bool {
    let Expr::Identifier(name) = expr else {
        return self.is_mutable_subexpr(expr);
    };
    self.is_state_field(name) || self.is_txn_variable(name)
}

fn is_mutable_subexpr(&self, expr: &Expr) -> bool {
    match expr {
        Expr::FieldAccess(base, _) => self.is_mutable_location(base),
        Expr::Index(base, _) => self.is_mutable_location(base),
        Expr::Deref(ptr) => !self.is_const_ptr(ptr),
        _ => false,
    }
}
```

### 9.5 Type inference for AddrOf

```rust
Expr::AddrOf(inner) => {
    let inner_ty = self.infer_expression(inner)?;
    let name = if self.is_mutable_location(inner) { "Ptr" } else { "PtrConst" };
    Ok(Type::Applied(name.into(), vec![inner_ty]))
}
Expr::Deref(ptr) => {
    let ptr_ty = self.infer_expression(ptr)?;
    let inner = pointee_type(&ptr_ty);
    inner.ok_or(TypeError::InvalidDeref(ptr_ty))
}
```

### 9.6 Tests

- `&state_field` → `Ptr<Int>`
- `&let_binding` → `Ptr<const Int>`
- `&param` → `Ptr<const Int>` (value parameter)
- `&param: Ptr<Int>` → `Ptr<Int>` (pointer parameter, reborrow)
- `&field.member` → `Ptr<T>` where field is a state struct
- `*(&counter)` → `Int` (dereference gives back the value)
- `&(a + b)` → type error (cannot take address of rvalue)

---

## 10. Phase 2 — Dereference Operator + Codegen

### 10.1 LLVM codegen: emit_addr_of

**New file:** `src/backend/llvm/expr/addr_of.rs` (or inline in `identifier.rs`)

```rust
fn emit_addr_of(&mut self, out: &mut String, expr: &Expr, indent: usize)
    -> Result<String, String>
{
    match expr {
        Expr::Identifier(name) => {
            if let Some(&idx) = self.ctx.field_index_map.get(name) {
                // State field → GEP on %State*
                let state_ptr = self.fun.state_ptr.as_ref().unwrap();
                let reg = self.gen_reg();
                writeln!(out, "{}%{} = getelementptr inbounds %State, ptr %{}, i32 0, i32 {}",
                    ind, reg, state_ptr, idx)?;
                Ok(reg)  // This is a ptr<State.field>
            } else if let Some(alloca) = self.fun.let_allocas.get(name) {
                // Let binding with alloca → address is the alloca
                Ok(alloca.clone())
            } else {
                // Let binding or param without alloca — create one
                let ty = self.fun.let_types.get(name).unwrap();
                let llvm_ty = self.llvm_type(ty);
                let reg = self.gen_reg();
                let alloca_reg = format!("{}.addr", name);
                writeln!(out, "{}%{} = alloca {}, align {}", ind, alloca_reg, llvm_ty, align)?;
                writeln!(out, "{}store {} %{}, ptr %{}, align {}",
                    ind, llvm_ty, reg, alloca_reg, align)?;
                Ok(alloca_reg)
            }
        }
        Expr::FieldAccess(base, field) => {
            let base_ptr = self.emit_addr_of(out, base, indent)?;
            // GEP into the struct
            let field_idx = self.ctx.struct_field_index(field);
            let reg = self.gen_reg();
            writeln!(out, "{}%{} = getelementptr inbounds %StructTy, ptr %{}, i32 0, i32 {}",
                ind, reg, base_ptr, field_idx)?;
            Ok(reg)
        }
        Expr::Index(base, index) => {
            let base_ptr = self.emit_addr_of(out, base, indent)?;
            let idx_reg = self.emit_expr(out, index, indent, &Type::Int)?;
            let reg = self.gen_reg();
            writeln!(out, "{}%{} = getelementptr inbounds %ElemTy, ptr %{}, i64 %{}",
                ind, reg, base_ptr, idx_reg)?;
            Ok(reg)
        }
        _ => Err("cannot take address of expression".into()),
    }
}
```

### 10.2 LLVM codegen: emit_deref

New match arm in `emit_expression`:

```rust
Expr::Deref(ptr) => {
    let ptr_reg = self.emit_expr(out, ptr, indent, &Type::Void)?;
    let ptr_ty = self.infer_expression(ptr)?;
    let inner_ty = pointee_type(&ptr_ty).unwrap();
    let llvm_inner_ty = self.llvm_type(&inner_ty);
    let reg = self.gen_reg();
    writeln!(out, "{}%{} = load {}, ptr %{}, align {}",
        ind, reg, llvm_inner_ty, ptr_reg, align)?;
    Ok(reg)
}
```

### 10.3 Assignment through AddrOf

In `emit_stmt.rs`, the assignment codegen:

```rust
// For Statement::Assignment { lhs: Expr::AddrOf(target), expr }
// Compute the address, then store through it
let addr = self.emit_addr_of(out, target, indent)?;
let val = self.emit_expr(out, expr, indent, &void)?;
// Store val to addr
writeln!(out, "{}store {} %{}, ptr %{}, align {}",
    ind, llvm_ty, val, addr, align)?;
```

### 10.4 SSA mode fallback

When SSA mode (A005a/A005b) is active and a field's address is taken:

```rust
// In loop_engine.rs, during field analysis:
if self.has_addr_of(&loop_body, field_name) {
    // Force this field out of SSA into memory
    // Create an alloca for it, then GEP for addr_of
    self.fun.ssa_field_fallback.insert(field_name);
}
```

The analysis scans the body for `Expr::AddrOf(Identifier(name))` and marks
those fields for memory-based access.

### 10.5 Tests

- `&counter = 42` → GEP on `%State*` + `store i64 42`
- `*ptr = val` → `load`/`store` through ptr register
- `let x = *(&counter)` → GEP + load (same as direct read for now)
- `&record.field = val` → GEP into struct field
- `&arr[i] = val` → GEP with index

---

## 11. Phase 3 — Borrow Warnings + Provenance Tracking

### 11.1 Provenance data structure

**New file:** `src/analysis/provenance.rs`

```rust
pub enum Provenance {
    /// Points to a known field or variable
    Known(String),
    /// Points to a field access: base.field
    FieldAccess {
        base: Box<Provenance>,
        field: String,
    },
    /// Points to an index: base[index]
    Index {
        base: Box<Provenance>,
        index: Box<Provenance>,
    },
    /// Points to the target of a dereference
    Deref(Box<Provenance>),
    /// Provenance lost — read from opaque source or FFI
    Unknown,
}

pub struct ProvenanceMap {
    /// Maps expression IDs to their provenance
    map: HashMap<ExprId, Provenance>,
}
```

### 11.2 Provenance inference

Alongside type inference in the typechecker:

```rust
fn infer_provenance(&self, expr: &Expr) -> Provenance {
    match expr {
        Expr::Identifier(name) => Provenance::Known(name.clone()),
        Expr::FieldAccess(base, field) => Provenance::FieldAccess {
            base: Box::new(self.infer_provenance(base)),
            field: field.clone(),
        },
        Expr::Index(base, index) => Provenance::Index {
            base: Box::new(self.infer_provenance(base)),
            index: Box::new(self.infer_provenance(index)),
        },
        Expr::AddrOf(inner) => self.infer_provenance(inner),
        Expr::Deref(ptr) => self.deref_provenance(ptr),
        _ => Provenance::Unknown,
    }
}

fn deref_provenance(&self, ptr: &Expr) -> Provenance {
    let inner = self.infer_provenance(ptr);
    match inner {
        Provenance::Unknown => Provenance::Unknown,
        Provenance::Deref(in_inner) => *in_inner,  // *(*p) → p's target
        _ => Provenance::Deref(Box::new(inner)),
    }
}
```

### 11.3 Dangling warning

After typechecking each txn body, scan assignments:

```rust
fn check_dangling_ptrs(txn_body: &[Statement], provenance: &ProvenanceMap) -> Vec<Diagnostic> {
    let mut diags = vec![];
    for stmt in txn_body {
        let Some(ptr_target, ptr_source) = extract_ptr_assign(stmt) else { continue; };
        if is_local_source(ptr_source, provenance) {
            diags.push(build_dangling_warning(ptr_target, ptr_source));
        }
    }
    diags
}

fn extract_ptr_assign(stmt: &Statement) -> Option<(&Expr, &Expr)> {
    let Statement::Assignment { lhs, expr } = stmt else { return None; };
    let Expr::AddrOf(lhs_target) = lhs else { return None; };
    let Expr::AddrOf(rhs_inner) = expr else { return None; };
    Some((lhs_target, rhs_inner))
}

fn is_local_source(expr: &Expr, provenance: &ProvenanceMap) -> bool {
    let Some(prov) = provenance.get(expr) else { return false; };
    matches!(prov, Provenance::Known(_)) && provenance.is_local(expr)
}

fn build_dangling_warning(target: &Expr, source: &Expr) -> Diagnostic {
    Diagnostic::warning("pointer to local may dangle")
        .with_suggestion("store the value, not the pointer")
}
```

### 11.4 Provenance for parallel safety

The transition graph's `compute_live_fields` reads provenance to refine write
sets:

```rust
fn refine_write_set(&self, writes: &mut WriteSet, provenance: &ProvenanceMap) {
    for (txn_id, body) in self.transactions.iter() {
        self.refine_txn_writes(txn_id, body, writes, provenance);
    }
}

fn refine_txn_writes(&self, txn_id: &str, body: &[Statement],
    writes: &mut WriteSet, provenance: &ProvenanceMap)
{
    for stmt in body {
        for write in stmt.writes() {
            self.refine_one_write(txn_id, write, writes, provenance);
        }
    }
}

fn refine_one_write(&self, txn_id: &str, write: &Write,
    writes: &mut WriteSet, provenance: &ProvenanceMap)
{
    let Some(prov) = provenance.get(write.expr_id()) else {
        return writes.add_unknown(txn_id);
    };
    match prov {
        Provenance::Known(name) => writes.add_named(txn_id, name),
        Provenance::Unknown => writes.add_unknown(txn_id),
        _ => writes.add_unknown(txn_id),
    }
}
```

### 11.5 Tests

- `&state_ptr = &local_var` → warning
- `&state_ptr = &state_field` → no warning (both are state)
- `*ptr = val` with unknown provenance → no warning (provenance is already unknown)
- Parallel txn with known provenance → correctly serialized
- Parallel txn with unknown provenance → conservatively serialized

---

## 12. Phase 4 — Arrow Operations, Interpreter, Proof Engine

### 12.1 Arrow ops

Arrow operations (`&list <- val`, `val <- &list`, `<- &list`) currently use
`Expr::OwnedRef` for the target/source. Under the new system, they use
`Expr::AddrOf` from the parser. The codegen for arrow ops extracts the address
from `AddrOf`:

```rust
// In arrow codegen (emit_stmt.rs or helpers.rs):
fn arrow_target_addr(&mut self, out: &mut String, target: &Expr, indent: usize)
    -> Result<String, String>
{
    match target {
        Expr::AddrOf(inner) => self.emit_addr_of(out, inner, indent),
        _ => Err("arrow target must be an address-of expression".into()),
    }
}
```

The parser already handles this — `extract_arrow_target` already expects
`OwnedRef` (now `AddrOf`) at the root. Minimal changes needed.

### 12.2 Interpreter

**File:** `src/interpreter.rs`

```rust
Expr::AddrOf(inner) => {
    // In the interpreter, address-of evaluates the inner expression
    // and returns it wrapped in a Value::Ptr marker
    let val = self.eval_expr(inner)?;
    Ok(Value::Ptr(Box::new(val)))
}
Expr::Deref(ptr) => {
    let val = self.eval_expr(ptr)?;
    match val {
        Value::Ptr(inner) => Ok(*inner),
        _ => Err(RuntimeError::TypeError("cannot dereference non-pointer".into())),
    }
}
```

Add `Value::Ptr(Box<Value>)` to the interpreter's value enum. This is a simple
wrapper — no heap allocation, just a semantic marker.

### 12.3 Proof engine

**File:** `src/proof_engine.rs`

```rust
SymbolicValue::Pointer(Box<SymbolicValue>),
```

```rust
Expr::AddrOf(inner) => {
    let sv = self.from_expr(inner, vars, state_expr)?;
    SymbolicValue::Pointer(Box::new(sv))
}
Expr::Deref(ptr) => {
    let sv = self.from_expr(ptr, vars, state_expr)?;
    match sv {
        SymbolicValue::Pointer(inner) => *inner,
        _ => SymbolicValue::Unknown,
    }
}
```

### 12.4 Tests

- Interpreter: `let p = &x; let v = *p;` → reads value of x
- Interpreter: `*(&x) = 5;` → updates state
- Proof engine: `*p > 0` in postcondition → symbolically evaluated
- Arrow: `&list <- val` in interpreter → push to list

---

## 13. Phase 5 — Webstack + CIRCT Backends

### 13.1 Webstack

**File:** `src/backend/webstack.rs`

Add match arms for `AddrOf` and `Deref`:

```rust
Expr::AddrOf(inner) => {
    // Webstack target: pointer semantics via array index + offset at runtime
    // Emit JS: &state_field → `state.field` (reference, not value)
    let inner_js = self.emit_expr(inner, reg_names, indent)?;
    // Wrap to indicate it's a reference
    format!("{}.ref", inner_js)
}
Expr::Deref(ptr) => {
    let ptr_js = self.emit_expr(ptr, reg_names, indent)?;
    // Strip the .ref marker, emit as value read
    format!("{}.val", ptr_js)
}
```

The webstack backend is a secondary concern — it's acceptable to implement as
a basic translation (`.ref` / `.val` JS wrappers) without full borrow checking.

### 13.2 CIRCT

**File:** `src/backend/circt.rs`

Replace existing `OwnedRef` handling with `AddrOf`:

```rust
Expr::AddrOf(Expr::Identifier(name)) => {
    // Same as current OwnedRef behavior: reference by name
    name.clone()
}
Expr::Deref(ptr) => {
    // In CIRCT, dereference reads the register value
    self.emit_expr(ptr, reg_names, indent, expected_ty)
}
```

### 13.3 Tests

- Webstack: `&x = 5` emits correct JS assignment
- CIRCT: `&x = 5` emits correct `seq.always(posedge %clock)` register update

---

## 14. Phase 6 — Tests, Benchmarks, Documentation

### 14.1 Test categories

| Test | Scope | Count |
|------|-------|-------|
| Parser: `&expr` generalizes beyond `&name` | parser.rs | 3 |
| Parser: `*expr` dereference | parser.rs | 3 |
| Parser: reject `&(a + b)` rvalue | parser.rs | 1 |
| Typechecker: `&field` → `Ptr<Int>` | typechecker.rs | 2 |
| Typechecker: `&let_binding` → `Ptr<const Int>` | typechecker.rs | 2 |
| Typechecker: `*ptr` → inner type | typechecker.rs | 2 |
| Typechecker: `*(&field)` == `field` | typechecker.rs | 2 |
| Typechecker: const → mut coercion rejection | typechecker.rs | 2 |
| LLVM: `&field` → GEP | llvm tests | 3 |
| LLVM: `*ptr` → load/store | llvm tests | 3 |
| LLVM: assignment through AddrOf | llvm tests | 3 |
| LLVM: SSA fallback for borrowed fields | llvm tests | 2 |
| Interpreter: AddrOf/Deref round-trip | interpreter.rs | 3 |
| Proof engine: symbolic Ptr evaluation | proof_engine.rs | 2 |
| Arrow: `&list <- val` with AddrOf | Full integration | 3 |
| Dangling warning: local → state | typechecker/provenance | 3 |
| Dangling: state → state (no warning) | typechecker/provenance | 1 |
| CIRCT: updated match arms | circt.rs | 2 |
| Webstack: AddrOf emission | webstack.rs | 2 |
| All existing tests still pass | — | 1400+ |

### 14.2 Benchmark baseline

Before starting Phase 1, capture benchmarks:

```bash
bash benchmarks/build_and_bench.sh --runtime
```

### 14.3 Documentation

| Doc | Update |
|-----|--------|
| `docs/learn/types.md` | Add `Ptr<T>` and `Ptr<const T>` section: address-of, dereference, const inference, borrow warnings |
| `docs/learn/ffi.md` | Add Ptr FFI section: passing `&buf` to C functions |
| `docs/reference/BRIEV_LANGUAGE_REFERENCE.md` | Add `&expr` and `*expr` to Expressions section |
| `docs/architecture/features/borrow-checking.md` | NEW: design doc for pointer provenance, borrow warnings, const inference |
| `spec/SPEC.md` | Update type system section with Ptr types, dereference, and safety rules |
| `AGENTS.md` | Update "No Magic" table: `Expr::OwnedRef` → `Expr::AddrOf` |

---

## 15. Verification Gates

### 15.1 Flat Control Flow Mandate

Every Rust function written or modified during this project must respect
**max 2 levels of indentation**. Use `?`, guard clauses (`let ... else { }`),
early returns, and extracted helper functions to flatten code:

| Anti-pattern (wrong) | Pattern (right) |
|----------------------|-----------------|
| Nested `if let` chains | Guard clauses + extracted helpers |
| 3+ levels of `match` | Split into named sub-functions |
| Arrow code inside loops | Move body to a helper, call it |
| Cascading `else if` | Guard clauses + early returns |

**LLVM IR emission** is exempt (the `writeln!` lines naturally need match arms
over types), but the dispatch logic around IR emission must still be flat.

This rule is non-negotiable. Code with deeper nesting must be rejected before
commit.

Before each commit:

1. `cargo build` — 0 errors, 0 warnings
2. `cargo test --lib` — all tests pass (1400+)
3. `cargo test --lib -- backend::tests` — backend registration tests
4. `bash benchmarks/build_and_bench.sh --runtime` — no regressions (ratio ≥ 0.97x)

Before merging:

5. All Phase 6 tests written and passing
6. `cargo test --lib` with `-- --test-threads=1` (no flaky failures)
7. Full benchmark suite at or above baseline

---

## 16. Benchmark Baseline

(Taken from `main` at commit `1023ebb`, 2026-07-09, `cargo build --release` + `bash benchmarks/build_and_bench.sh --runtime`)

| Benchmark | Briev | C | Ratio | Winner | Correct |
|-----------|-------|---|-------|--------|---------|
| ring_buffer | .0591s | .0607s | .97x | Briev | MATCH |
| float_math | .0576s | .0743s | .77x | Briev | MATCH |
| float_math_nonzero | .1734s | .1675s | 1.03x | C | MATCH |
| sparse_dispatch | .0060s | .0606s | .09x | Briev | MATCH |
| print_loop | .0591s | .0591s | 1.00x | ~tie | MATCH |
| nbody_newton | 6.4594s | 9.1394s | .70x | Briev | MATCH |
| nbody_sqrt | 2.3248s | 3.4094s | .68x | Briev | MATCH |
| nbody_sqrt_idio | 2.8469s | 3.9685s | .71x | Briev | MATCH |
| fasta | .2068s | .2062s | 1.00x | ~tie | MATCH |
| fannkuch_redux | .0575s | .0635s | .90x | Briev | MATCH |
| mandelbrot | .6489s | .6476s | 1.00x | ~tie | MATCH |
| kalman_filter_runtime | .1802s | .1787s | 1.00x | ~tie | MATCH |
| knucleotide | .1879s | .1880s | .99x | Briev | MATCH |
| cancel_math | .0624s | .0588s | 1.06x | C | MATCH |
| bit_clear | .0007s | .0007s | 1.00x | ~tie | MATCH |
| queue_drain | .0596s | .0582s | 1.02x | C | MATCH |
| queue_drain_sym | .0592s | .0582s | 1.01x | C | MATCH |
| queue_drain_idio | SKIP | — | — | — | SKIP |
| interval_step | .0006s | .0599s | .01x | Briev | MATCH |
