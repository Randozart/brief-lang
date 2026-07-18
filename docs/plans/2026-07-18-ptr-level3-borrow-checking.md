# Ptr Level 3 — Safe Borrow Checking (Followup Plan)

**Date:** 2026-07-18
**Status:** Plan — ~60% implemented, ~40% remaining
**Prerequisite:** `docs/plans/2026-07-18-allocation-strategy-system.md` (the `Alloc#` escape analysis in this plan builds on the provenance infrastructure perfected here)
**Supersedes:** `docs/plans/2026-07-09-ptr-level3-borrow-checking.md` (original vision, 1169 lines), `docs/plans/2026-07-17-ptr-borrow-and-native-ring-fns.md` (merged interim plan, 224 lines)
**See also:**
  - `src/analysis/provenance.rs` (existing provenance infrastructure — 316 lines, 7 tests)
  - `src/ast/expr.rs` (Expr::AddrOf, Expr::Deref — already committed)
  - `src/backend/llvm/emit_expr.rs` (LLVM codegen for AddrOf/Deref — already committed)
  - `lib/std/core/ring_buffer.bv` (native ring_push/pop in pure Brief — already committed)

---

## Executive Summary

Finish the Ptr Level 3 borrow checker system. The foundational work (AST variants, parser, basic codegen, `Provenance` enum, escape warning infrastructure, native ring functions) is already committed. What remains:

1. **Make provenance actually work** — `is_local_provenance()` is a `false` stub. Fixing it enables dangling pointer warnings and the `Alloc#` escape analysis from the allocation strategy plan.
2. **Thread provenance through typechecker** — return `(Type, Provenance)` from `infer_expression` so every expression carries its pointer origin. This makes escape detection trivial.
3. **Add `PtrConst`** — read-only pointer variant for let-bindings. Enables const-correctness and documents intent.
4. **Const inference from context** — `&state_field` → `Ptr<T>` (mutable), `&let_binding` → `Ptr<const T>` (read-only).
5. **Wire provenance into parallel txn safety** — refine write-set analysis in `transition_graph.rs` to detect pointer aliasing between parallel txns.
6. **SSA fallback for borrowed fields** — when a field's address is taken, force it out of SSA into memory mode.
7. **Interpreter + proof engine** — `Value::Ptr`, `SymbolicValue::Pointer`.
8. **Webstack + CIRCT backends** — `AddrOf`/`Deref` match arms (active backends only).
9. **Comprehensive tests** — behavioral tests for every new feature.

### What's Already Done (roughly Phase 0 of the original plan)

| Component | Status | File |
|-----------|--------|------|
| `Expr::AddrOf(Box<Expr>)` | ✅ Committed | `src/ast/expr.rs:51` |
| `Expr::Deref(Box<Expr>)` | ✅ Committed | `src/ast/expr.rs:50` |
| `&` unary parser (`&expr`) | ✅ Committed | `src/parser/expressions.rs:152-155` |
| `*` unary parser (`*expr`) | ✅ Committed | `src/parser/expressions.rs:146-149` |
| `<-` arrow statement parser | ✅ Committed | `src/parser/statements.rs:291-309` |
| Property binding (`InsertAt <~ fn`) | ✅ Committed | `src/parser/definitions.rs:949` |
| `Type::Ptr(Box<Type>)` | ✅ Exists | `src/ast/types.rs:26` |
| `AddrOf` type inference → `Ptr<T>` | ✅ Done | `src/typechecker/mod.rs:120-123` |
| `Deref` type inference → strip `Ptr` | ✅ Done | `src/typechecker/mod.rs:125-134` |
| LLVM: `Expr::AddrOf` codegen | ✅ Done | `src/backend/llvm/emit_expr.rs:349-372` |
| LLVM: `Expr::Deref` codegen | ✅ Done | `src/backend/llvm/emit_expr.rs:375-385` |
| LLVM: `*ptr = val` (deref-assign) | ✅ Done | `src/backend/llvm/emit_stmt.rs:91-95` |
| Strategy-based `<-` dispatch | ✅ Done | `src/backend/llvm/emit_stmt.rs:35-65` |
| `emit_strategy_fn_call` generic | ✅ Done | `src/backend/llvm/emit_stmt.rs:236-282` |
| `check_insert_strategy` | ✅ Done | `src/backend/llvm/emit_toplevel.rs:107-117` |
| `check_extract_strategy` | ✅ Done | `src/backend/llvm/emit_toplevel.rs:122-132` |
| Bug: key casing mismatch (InsertAt vs insert_at) | ⚠️ Fix in Phase 1 of allocation plan | `normalizer.rs:102` vs `emit_toplevel.rs:115` |
| Bug: normalizer strips InsertAt/ExtractFrom | ⚠️ Fix in Phase 1 of allocation plan | `normalizer.rs:99-105` |
| `Provenance` enum | ✅ Done | `src/analysis/provenance.rs:6-23` |
| `infer_provenance()` | ✅ Done | `src/analysis/provenance.rs:27-40` |
| `deref_provenance()` | ✅ Done | `src/analysis/provenance.rs:45-52` |
| `check_dangling_ptrs()` | ✅ Done (but stub) | `src/analysis/provenance.rs:102-112` |
| `is_local_provenance()` | ❌ **Stub — always false** | `src/analysis/provenance.rs:57-70` |
| `check_convergence_safety()` | ✅ Done | `src/analysis/provenance.rs:187-237` |
| `ring_push`/`ring_pop` in pure Brief | ✅ Done | `lib/std/core/ring_buffer.bv:23-47` |
| `Intrinsic::RingPush`/`RingPop` removed | ✅ Done | — |
| 7 provenance unit tests | ✅ Done | `src/analysis/provenance.rs:239-316` |
| `PtrConst` type | ❌ **Not implemented** | — |
| Const inference (`is_mutable_location`) | ❌ **Not implemented** | — |
| Provenance threaded through typechecker | ❌ **Not implemented** | — |
| SSA fallback for borrowed fields | ❌ **Not implemented** | — |
| Interpreter: `Value::Ptr` | ❌ Not implemented | `src/interpreter/eval.rs:98-100` |
| Proof engine: `SymbolicValue::Pointer` | ❌ Not implemented | `src/symbolic.rs:274-276` |
| Webstack: `AddrOf`/`Deref` | ❌ Not implemented | `src/backend/webstack.rs` |
| CIRCT: `AddrOf`/`Deref` | ❌ Not implemented | `src/backend/circt.rs` |

---

## Phase 1: Fix Provenance Escape Analysis

The single most impactful change. `is_local_provenance()` is a stub that always returns `false`, which means `check_dangling_ptrs()` never fires. Fixing this enables the entire dangling-pointer warning system and provides the escape analysis needed by `Alloc#` in the allocation strategy plan.

### 1a. Make is_local_provenance actually work

**File:** `src/analysis/provenance.rs:57-70`

The function needs access to a set of local variable names. Currently it receives only a `Provenance` — it needs context. Change signature to accept a set of local names:

```rust
/// 2026-07-18: Check if a provenance refers to a local (non-state) variable.
/// `local_names` is the set of let-bindings and txn-local variables in scope.
pub fn is_local_provenance(prov: &Provenance, local_names: &HashSet<String>) -> bool {
    match prov {
        Provenance::Known(name) => local_names.contains(name),
        Provenance::FieldAccess { base, .. } | Provenance::Index { base, .. } => {
            // If the base is local, the field/index is local.
            is_local_provenance(base, local_names)
        }
        Provenance::Deref(_) => {
            // Deref of an unknown provenance could point anywhere.
            // Don't warn — the pointer's origin is not traceable.
            false
        }
        Provenance::Unknown => false,
    }
}
```

**Callers must be updated.** `check_dangling_ptrs` must now receive `local_names`:

```rust
pub fn check_dangling_ptrs(body: &[Statement], local_names: &HashSet<String>) -> Vec<String> {
    let mut warnings = Vec::new();
    for stmt in body {
        let Some((target, source)) = extract_ptr_assign(stmt) else { continue; };
        let source_prov = infer_provenance(source);
        if is_local_provenance(&source_prov, local_names) {
            warnings.push(build_dangling_warning(target, source));
        }
    }
    warnings
}
```

**Test:** `test_is_local_provenance_with_locals`
```rust
let locals: HashSet<String> = ["temp", "local_x"].iter().map(|s| s.to_string()).collect();
let prov = Provenance::Known("temp".to_string());
assert!(is_local_provenance(&prov, &locals));

let prov = Provenance::Known("state_val".to_string());
assert!(!is_local_provenance(&prov, &locals));
```

**Test:** `test_dangling_warning_fires`
```rust
let body = vec![
    Statement::Assign(
        Expr::Identifier("state_field".to_string()),
        Expr::Deref(Box::new(Expr::AddrOf(Box::new(Expr::Identifier("local_var".to_string()))))),
    ),
];
let locals: HashSet<String> = ["local_var"].iter().map(|s| s.to_string()).collect();
let warnings = check_dangling_ptrs(&body, &locals);
assert!(!warnings.is_empty());
assert!(warnings[0].contains("local_var"));
```

### 1b. Wire check_dangling_ptrs into the compilation pipeline

**File:** `src/compile.rs` — after typechecking, before codegen

```rust
// 2026-07-18: Dangling pointer check
let local_names: HashSet<String> = txn.let_bindings.iter()
    .map(|lb| lb.name.clone())
    .collect();
let warnings = analysis::provenance::check_dangling_ptrs(&txn.body, &local_names);
for w in &warnings {
    eprintln!("{}", w);  // or use the diagnostic system
}
```

Or wire it into the typechecker pass: after type-checking each txn body, run the dangling check with the txn's local variable set.

**Test:** `test_dangling_warning_in_compile`
- Compile a program with `&state_field = &local_var`
- Assert stderr contains "may dangle"
- Compile a program without the pattern (value copy instead)
- Assert no warning

### 1c. Build the local variable set

**File:** `src/typechecker/mod.rs` (or a new helper in `src/analysis/`)

Extract local variable names from a txn body:

```rust
/// 2026-07-18: Collect all let-binding names from a transaction body.
/// Also includes txn parameters (which are local to the txn invocation).
pub fn collect_local_names(body: &[Statement], params: &[String]) -> HashSet<String> {
    let mut locals: HashSet<String> = params.iter().cloned().collect();
    for stmt in body {
        if let Statement::Let { name, .. } = stmt {
            locals.insert(name.clone());
        }
        // Recursively collect from nested blocks
        match stmt {
            Statement::Guarded(_, body) | Statement::Block(body) => {
                locals.extend(collect_local_names(body, &[]));
            }
            _ => {}
        }
    }
    locals
}
```

This set is passed to `check_dangling_ptrs` and `is_local_provenance`.

---

## Phase 2: Thread Provenance Through Typechecker

Currently `infer_provenance` is a standalone function that walks the expression tree independently of type inference. This means provenance is computed after the fact, which:
1. Misses provenance from intermediate expressions
2. Requires re-walking the tree
3. Cannot inform type inference (const inference)

The fix: thread provenance alongside type inference, returning `(Type, Provenance)` from `infer_expression`.

### 2a. Change infer_expression to return (Type, Provenance)

**File:** `src/typechecker/mod.rs`

```rust
/// 2026-07-18: Infer both type and provenance for an expression.
/// Provenance tracks where pointer values originate, enabling
/// escape detection and parallel-txn write-set refinement.
fn infer_expression(&mut self, expr: &Expr) -> Result<(Type, Provenance>, TypeError> {
    match expr {
        Expr::Identifier(name) => {
            let ty = self.lookup_type(name)?;
            Ok((ty.clone(), Provenance::Known(name.clone())))
        }
        Expr::AddrOf(inner) => {
            let (inner_ty, inner_prov) = self.infer_expression(inner)?;
            // Const inference: state fields and txn vars → Ptr<T>,
            // let bindings and const params → Ptr<const T>
            let ptr_ty = if self.is_mutable_location(inner) {
                Type::ptr(inner_ty)  // Ptr<T>
            } else {
                Type::ptr_const(inner_ty)  // Ptr<const T>  (Phase 3)
            };
            Ok((ptr_ty, inner_prov))
        }
        Expr::Deref(ptr) => {
            let (ptr_ty, ptr_prov) = self.infer_expression(ptr)?;
            let inner = pointee_type(&ptr_ty).ok_or_else(|| TypeError::InvalidDeref(ptr_ty.clone()))?;
            // Deref provenance: strip one level
            let inner_prov = match ptr_prov {
                Provenance::Deref(inner) => *inner,
                p => Provenance::Deref(Box::new(p)),
            };
            Ok((inner, inner_prov))
        }
        // ... all other arms return (ty, Provenance::Unknown)
    }
}
```

This is the largest single change: every match arm in `infer_expression` needs to return a provenance. The general rule:

| Expression | Provenance |
|------------|-----------|
| `Identifier(name)` → | `Known(name)` |
| `Decimal(n)`, `Bool(b)`, `String(s)` → | `Unknown` (literals have no pointer origin) |
| `BinaryOp(op, a, b)` → | `Unknown` (compound expressions lose provenance) |
| `UnaryOp(op, a)` → | same as `a`'s provenance |
| `Field(base, field)` → | `FieldAccess { base, field }` |
| `Index(base, idx)` → | `Index { base, idx_prov }` |
| `AddrOf(inner)` → | same as `inner`'s provenance |
| `Deref(ptr)` → | unwrap one `Deref` or wrap with `Deref(ptr_prov)` |
| `Cast(inner, target)` → | same as `inner`'s provenance (pointer identity preserved) |
| `Call(name, args)` → | `Unknown` (function calls lose provenance) |
| `If(cond, then, else)` → | provenance of taken branch (or `Unknown` if different) |
| `Block(stmts)` → | provenance of last expression |
| `Tuple(elems)`, `List(elems)`, `Match(_, arms)` → | `Unknown` |

**Key principle:** Most expressions return `Provenance::Unknown`. Only `Identifier`, `AddrOf`, `Deref`, `Field`, and `Index` produce meaningful provenance. This is fine — the escape analysis is conservative for `Unknown` (assumes it might be a state field, i.e., no warning).

### 2b. Update all callers of infer_expression

Every site that calls `infer_expression` (or the older `infer_type`) needs to:
1. Destructure the `(Type, Provenance)` pair
2. Use the type for type-checking (unchanged behavior)
3. Thread the provenance through to compound expressions (new behavior)

Sites to update (found via grep for `infer_expression` and `infer_type`):

| Site | File | Change |
|------|------|--------|
| Type inference for let-bindings | `typechecker/mod.rs` | Use `(ty, _prov)` for type check, ignore provenance |
| Type inference for function params | `typechecker/mod.rs` | Same |
| Type inference for state fields | `typechecker/mod.rs` | Same |
| Contract precondition/postcondition checking | `typechecker/mod.rs` | Same |
| `infer_contract_expression` | `typechecker/mod.rs` | May need to thread provenance for contract-bound escape checks |

### 2c. Preserve backward compatibility

Add a convenience wrapper:

```rust
/// Convenience: infer type only (discard provenance).
/// Used by callers that don't need provenance tracking.
pub fn infer_type_only(&mut self, expr: &Expr) -> Result<Type, TypeError> {
    self.infer_expression(expr).map(|(ty, _)| ty)
}
```

Replace existing `infer_expression` calls that don't need provenance with `infer_type_only`. This minimizes churn.

**Test:** `test_provenance_threaded_addrof`
- `&field` with field being state → provenance = `Known("field")`
- `&local_x` → provenance = `Known("local_x")`
- `*(&field)` → provenance = `Deref(Known("field"))`

**Test:** `test_provenance_unknown_for_literals`
- `42` → provenance = `Unknown`
- `true` → provenance = `Unknown`

---

## Phase 3: Add PtrConst Type Variant

Read-only pointers (`Ptr<const T>`) for let-bindings and value parameters. Enables the const-correctness rules from the original plan §5.

### 3a. Add PtrConst to the Type enum

**File:** `src/ast/types.rs`

```rust
pub enum Type {
    // ... existing variants ...
    Ptr(Box<Type>),             // mutable pointer: *T
    PtrConst(Box<Type>),        // const pointer: *const T  — 2026-07-18
    PtrN(u64, u64),             // raw pointer: Ptr64, Ptr32, etc.
    LayoutPtr(LayoutConstraint), // layout-constrained pointer (MMIO)
}
```

Add convenience constructors:

```rust
impl Type {
    pub fn ptr(inner: Type) -> Type { Type::Ptr(Box::new(inner)) }
    pub fn ptr_const(inner: Type) -> Type { Type::PtrConst(Box::new(inner)) }
}
```

### 3b. Update all Type match arms

Every file that matches on `Type` needs a `PtrConst` arm. Files to update:

| File | New arms needed |
|------|-----------------|
| `src/ast/types.rs` | `Type::PtrConst` in `bytes()`, `alignment()`, `Display` |
| `src/type_universe.rs` | Register `PtrConst` as a primitive type |
| `src/backend/llvm/types.rs` | LLVM type mapping for `PtrConst` (same as `Ptr` → `"ptr"`) |
| `src/backend/llvm/helpers.rs` | `is_ptr_ty` should return true for `PtrConst` |
| `src/typechecker/mod.rs` | `pointee_type()`: `PtrConst(inner)` → `Some(*inner)` |
| `src/backend/llvm/intrinsics.rs` | Handle `PtrConst` in Deref#/Index# |
| `src/backend/llvm/emit_expr.rs` | Handle `PtrConst` in AddrOf/Deref codegen |
| `src/interpreter/mod.rs` | Handle `PtrConst` in type dispatch |
| `src/annotator.rs` | Handle `PtrConst` in match arms |
| `src/ast/display.rs` | Display `PtrConst(inner)` as `Ptr<const T>` |
| `src/backend/llvm/emit_toplevel.rs` | Handle in field declarations |

**Pattern for each match:** `PtrConst` is treated identically to `Ptr` for:
- LLVM type mapping (`"ptr"`)
- Size/alignment (8 bytes)
- Pointee type extraction
- Codegen (load/store through pointer)

The only difference is:
- **No write-through:** `*ptr = val` on `PtrConst` → type error
- **Const inference:** `&let_binding` → `PtrConst` instead of `Ptr`

### 3c. Const inference from context

**File:** `src/typechecker/mod.rs` — add `is_mutable_location` helper

```rust
/// 2026-07-18: Determine if an expression refers to a mutable location.
/// Returns true for state fields and txn-scoped variables.
/// Returns false for let bindings (single-assignment) and const params.
fn is_mutable_location(&self, expr: &Expr) -> bool {
    let Expr::Identifier(name) = expr else {
        return self.is_mutable_subexpr(expr);
    };
    // State fields and txn parameters are mutable.
    // Let bindings are NOT mutable locations.
    self.is_state_field(name) || self.is_txn_variable(name)
}

fn is_mutable_subexpr(&self, expr: &Expr) -> bool {
    match expr {
        Expr::Field(base, _) => self.is_mutable_location(base),
        Expr::Index(base, _) => self.is_mutable_location(base),
        Expr::Deref(ptr) => {
            // Deref of a PtrConst → still const (no mutation through const pointer)
            // Deref of a Ptr → mutable (if the base was mutable)
            if let Ok((ty, _)) = self.infer_expression(ptr) {
                !matches!(ty, Type::PtrConst(_))
            } else {
                false
            }
        }
        _ => false,
    }
}
```

Used in `AddrOf` type inference (Phase 2a):

```rust
Expr::AddrOf(inner) => {
    let (inner_ty, inner_prov) = self.infer_expression(inner)?;
    let ptr_ty = if self.is_mutable_location(inner) {
        Type::ptr(inner_ty)
    } else {
        Type::ptr_const(inner_ty)
    };
    Ok((ptr_ty, inner_prov))
}
```

### 3d. Write-through guard on PtrConst

**File:** `src/typechecker/mod.rs` — in the assignment type-check

```rust
// 2026-07-18: Check for write through const pointer
if let Expr::Deref(ptr) = &stmt.lhs {
    if let Ok((ptr_ty, _)) = self.infer_expression(ptr) {
        if matches!(ptr_ty, Type::PtrConst(_)) {
            return Err(TypeError::WriteThroughConstPointer {
                ptr_expr: *ptr.clone(),
                span: stmt.span,
            });
        }
    }
}
```

**Test:** `test_write_through_const_pointer_errors`
```brief
let x = 5;
let p = &x;  // Ptr<const Int>
*p = 6;      // error: cannot write through const pointer
```

**Test:** `test_write_through_mutable_pointer_ok`
```brief
state { x: Int };
let p = &x;  // Ptr<Int>
*p = 6;      // ok
```

---

## Phase 4: Wire Provenance into Parallel Txn Safety

### 4a. Provenance-aware write sets

**File:** `src/analysis/transition_graph.rs`

The parallel scheduler determines which transactions can execute concurrently by computing write sets. Currently, all writes through pointers are conservatively assumed to conflict with everything.

Add a function that computes the write set of an expression given its provenance:

```rust
/// 2026-07-18: Compute the write set of an expression using its provenance.
/// Returns None if the write set is unknown (conservative — conflicts with all).
fn write_set_from_expr(expr: &Expr, prov: &Provenance) -> Option<HashSet<String>> {
    match prov {
        Provenance::Known(name) => {
            let mut set = HashSet::new();
            set.insert(name.clone());
            Some(set)
        }
        Provenance::FieldAccess { base, field } => {
            let base_set = write_set_from_prov(base)?;
            Some(base_set.into_iter().map(|b| format!("{}.{}", b, field)).collect())
        }
        Provenance::Index { base, .. } => {
            // Index access — conservatively assume all elements
            write_set_from_prov(base)
        }
        Provenance::Deref(inner) => {
            // Deref — use the provenance of the pointed-to expression
            write_set_from_prov(inner)
        }
        Provenance::Unknown => None, // conservative: conflicts with everything
    }
}

fn write_set_from_prov(prov: &Provenance) -> Option<HashSet<String>> {
    match prov {
        Provenance::Known(name) => Some([name.clone()].into()),
        Provenance::FieldAccess { base, field } => {
            write_set_from_prov(base).map(|b| b.into_iter().map(|x| format!("{}.{}", x, field)).collect())
        }
        Provenance::Index { base, .. } => write_set_from_prov(base),
        Provenance::Deref(inner) => write_set_from_prov(inner),
        Provenance::Unknown => None,
    }
}
```

### 4b. Refine write-set computation in transition_graph

**File:** `src/analysis/transition_graph.rs`

In the function that computes write sets for parallel txn scheduling, check provenance before falling back to conservative:

```rust
/// 2026-07-18: Compute the fields written by a transaction body,
/// using provenance to refine pointer-based writes.
fn compute_write_set(txn: &Transaction, provenance_map: &HashMap<ExprId, Provenance>) -> WriteSet {
    let mut write_set = WriteSet::new();
    for stmt in &txn.body {
        let written = match stmt {
            Statement::Assign(Expr::Identifier(name), _) => {
                Some(HashSet::from([name.clone()]))
            }
            Statement::Assign(Expr::Deref(ptr), _) => {
                // Write through pointer — use provenance
                let ptr_prov = provenance_map.get(&expr_id(ptr));
                match ptr_prov {
                    Some(prov) => write_set_from_prov(prov),
                    None => None, // conservative
                }
            }
            // ... other statement types
            _ => continue,
        };
        match written {
            Some(fields) => write_set.add_all(fields),
            None => write_set.set_conservative(true), // conflicts with everything
        }
    }
    write_set
}
```

### 4c. Provenance-carrying register annotation

**File:** `src/backend/llvm/mod.rs` — extend `TypedRegister` with optional provenance

```rust
pub struct TypedRegister {
    pub reg: String,
    pub ty: Type,
    pub strategy: Option<AllocStrategy>,  // 2026-07-18 (allocation strategy plan)
    pub provenance: Option<Provenance>,   // 2026-07-18
}
```

The provenance is set during expression emission and read during parallel txn scheduling.

**Test:** `test_provenance_write_set_known`
- Txn A writes `*p` where `p` provenance is `Known("counter")`
- Txn B writes `&counter`
- Assert write sets conflict (both write to `"counter"`)

**Test:** `test_provenance_write_set_unknown`
- Txn A writes `*p` where `p` provenance is `Unknown`
- Txn B writes `&counter`
- Assert conflict (conservative — unknown conflicts with everything)

---

## Phase 5: SSA Fallback for Borrowed Fields

### 5a. Detect address-of for state fields

**File:** `src/backend/llvm/mod.rs` or a new analysis pass

When a field's address is taken (`&state_field`), the field must be forced out of SSA into memory mode. The SSA codegen path (A005c, A006) assumes all fields are SSA registers — a borrowed field must fall back to memory.

Add a scan before codegen path selection:

```rust
/// 2026-07-18: Check if any state field has its address taken.
/// If so, those fields must be forced into memory mode (not SSA).
fn find_borrowed_fields(body: &[Statement]) -> HashSet<String> {
    let mut borrowed = HashSet::new();
    for stmt in body {
        match stmt {
            Statement::Assign(Expr::AddrOf(inner), _) => {
                if let Expr::Identifier(name) = inner.as_ref() {
                    borrowed.insert(name.clone());
                }
            }
            Statement::Guarded(_, body) | Statement::Block(body) => {
                borrowed.extend(find_borrowed_fields(body));
            }
            _ => {}
        }
    }
    borrowed
}
```

### 5b. Exclude borrowed fields from SSA

**File:** `src/backend/llvm/loop_engine/` — in the codegen path selection

```rust
// 2026-07-18: Before selecting A005c/A006, check for borrowed fields.
let borrowed = find_borrowed_fields(txn.body);
if !borrowed.is_empty() {
    // Fall back to memory mode for borrowed fields.
    // The borrowed fields use %State-based access (GEP + load/store),
    // while non-borrowed fields can still use SSA registers.
    for field_name in &borrowed {
        self.fun.memory_mode_fields.insert(field_name.clone());
    }
}
```

The `memory_mode_fields` set tells the per-field phi emission to use GEP/load/store for those fields instead of SSA phi registers.

### 5c. Emit GEP for AddrOf of state fields

**File:** `src/backend/llvm/emit_expr.rs` (already partially done at lines 349-372)

The existing `emit_addr_of` code already handles state fields with GEP. The SSA fallback ensures that when a field's address is taken, it has a memory location to point to (not just an SSA register).

**Test:** `test_ssa_fallback_on_borrow`
- Compile a txn with `&state_field` taken inside a bounded loop
- Assert the field is accessed via GEP/load/store, not SSA phi register
- Assert the program produces correct results

---

## Phase 6: Interpreter + Proof Engine Updates

### 6a. Interpreter: Value::Ptr

**File:** `src/interpreter/eval.rs:98-100`

Currently `Expr::AddrOf` and `Expr::Deref` just recurse into their inner expressions — they effectively become no-ops in the interpreter. Fix:

```rust
// In the expression evaluator:
Expr::AddrOf(inner) => {
    let val = self.eval(inner, state)?;
    // Return a Ptr wrapper containing the inner value's address
    Ok(Value::Ptr(Box::new(val)))
}
Expr::Deref(ptr) => {
    let val = self.eval(ptr, state)?;
    match val {
        Value::Ptr(inner) => Ok(*inner),  // Deref: return the pointed-to value
        other => Err(RuntimeError::TypeError(format!(
            "cannot deref non-pointer value: {:?}", other
        ))),
    }
}
```

**File:** `src/interpreter/value.rs` — add `Ptr` variant

```rust
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    // ... existing variants
    Ptr(Box<Value>),        // 2026-07-18
    Void,
}
```

**Test:** `test_interpreter_addrof_deref_roundtrip`
```
let x = 42;
let p = &x;
let y = *p;
// y == 42
```

### 6b. Proof engine: SymbolicValue::Pointer

**File:** `src/symbolic.rs:274-276`

```rust
// In the symbolic evaluator:
Expr::AddrOf(inner) => {
    let sv = self.eval_symbolic(inner, ctx)?;
    Ok(SymbolicValue::Pointer(Box::new(sv)))
}
Expr::Deref(ptr) => {
    let sv = self.eval_symbolic(ptr, ctx)?;
    match sv {
        SymbolicValue::Pointer(inner) => Ok(*inner),
        other => Err(ProofError::TypeError(format!(
            "cannot deref non-pointer symbolic value: {:?}", other
        ))),
    }
}
```

**File:** `src/symbolic.rs` — add `Pointer` variant

```rust
pub enum SymbolicValue {
    Constant(Bits),
    Variable(String),
    // ... existing variants
    Pointer(Box<SymbolicValue>),  // 2026-07-18
    Unknown,
}
```

**Test:** `test_symbolic_addrof_deref`

---

## Phase 7: Webstack + CIRCT Backend Updates

### 7a. Webstack: AddrOf/Deref

**File:** `src/backend/webstack.rs`

The webstack backend (WASM + JS glue) needs match arms for the two new AST variants. Since these are active backends (per AGENTS.md), they must be implemented.

```rust
// In the expression emitter:
Expr::AddrOf(inner) => {
    // Webstack: address-of is identity (WASM doesn't have pointers in the same sense)
    // Emit the inner expression and treat it as a reference
    self.emit_expr(inner, out)?;
    // The result is already a reference in WASM's model
    Ok(())
}
Expr::Deref(ptr) => {
    // Webstack: dereference is load through a reference
    self.emit_expr(ptr, out)?;
    // Emit WASM load instruction based on pointee type
    writeln!(out, "  local.get ${}", self.last_reg()).ok();
    let ty = self.infer_type(ptr)?;
    let pointee_ty = pointee_type(&ty).ok_or("Deref on non-pointer")?;
    let wasm_load = match pointee_ty.llvm_type() {
        "i64" => "i64.load",
        "i32" => "i32.load",
        "float" => "f32.load",
        _ => "i64.load", // default
    };
    writeln!(out, "  {}", wasm_load).ok();
    Ok(())
}
```

### 7b. CIRCT: AddrOf/Deref

**File:** `src/backend/circt.rs`

```rust
// In the expression emitter:
Expr::AddrOf(inner) => {
    // CIRCT (hardware): address-of is a wire reference
    self.emit_expr(inner, out)?;
    // In hardware, wires are always references — no explicit address-of needed
    Ok(())
}
Expr::Deref(ptr) => {
    // CIRCT: dereference is reading a wire
    self.emit_expr(ptr, out)?;
    // The result is already the wire value in CIRCT's dataflow model
    Ok(())
}
```

**Test:** `test_webstack_addr_of` — compile with `--target webstack`, verify output
**Test:** `test_circt_addr_of` — compile with `--target circt`, verify output

---

## Phase 8: Tests, Documentation, Benchmarks

### 8a. Behavioral tests (per Directive §5)

| Test | Phase | What it asserts |
|------|-------|-----------------|
| `test_is_local_provenance_with_locals` | 1 | `Known("temp")` is local when `"temp"` is in local_names set |
| `test_is_local_provenance_state_field` | 1 | `Known("state_x")` is not local when not in local_names |
| `test_dangling_warning_fires` | 1 | `&state_field = &local_var` produces warning |
| `test_dangling_warning_silent_on_value_copy` | 1 | `&state_field = local_var` (value, not pointer) no warning |
| `test_provenance_threaded_addrof` | 2 | `&field` → provenance `Known("field")` |
| `test_provenance_threaded_deref` | 2 | `*ptr` → provenance `Deref(ptr_prov)` |
| `test_provenance_unknown_for_literals` | 2 | `42` → provenance `Unknown` |
| `test_ptr_const_created_for_let` | 3 | `&let_x` → `Ptr<const Int>` |
| `test_ptr_mutable_created_for_state` | 3 | `&state_f` → `Ptr<Int>` |
| `test_write_through_const_pointer_errors` | 3 | `*p = 5` where p is `PtrConst` → type error |
| `test_write_through_mutable_pointer_ok` | 3 | `*p = 5` where p is `Ptr` → ok |
| `test_provenance_write_set_known` | 4 | Txn writes through `Known("counter")` → write set `{counter}` |
| `test_provenance_write_set_unknown` | 4 | Txn writes through `Unknown` → write set `{all}` (conservative) |
| `test_ssa_fallback_on_borrow` | 5 | Borrowed field uses memory mode, not SSA |
| `test_interpreter_addrof_deref_roundtrip` | 6 | `*(&42) == 42` |
| `test_webstack_addr_of` | 7 | Webstack output handles `&expr` |
| `test_circt_addr_of` | 7 | CIRCT output handles `&expr` |

**All existing tests must pass:** `cargo test --lib`

### 8b. Documentation

| Document | Change |
|----------|--------|
| `docs/architecture/features/ptr.md` | Add §"PtrConst — Read-Only Pointers": when PtrConst is inferred, what operations are forbidden. Add §"Provenance Tracking": description of provenance system and how it enables escape detection and parallel-txn safety. |
| `docs/architecture/llvm-memory-management.md` | Update §"SSA modes and pointer interaction": document that borrowed fields force memory mode. |
| `docs/architecture/backend-type-dispatch.md` | Add `PtrConst` to type dispatch table. |
| `src/analysis/provenance.rs` | Module-level doc comment explaining the provenance system, the `is_local_provenance` fix, and the `check_dangling_ptrs` pipeline. |
| `src/typechecker/mod.rs` | Doc comment on `infer_expression` noting that it returns `(Type, Provenance)`. Doc comment on `is_mutable_location`. |
| `src/analysis/transition_graph.rs` | Doc comment on provenance-aware write set computation. |

### 8c. Rationale comments

Every modified code site gets a `// 2026-07-18: <why>` comment:

| Site | Comment |
|------|---------|
| `provenance.rs:is_local_provenance` | `// 2026-07-18: Fix stub — now takes local_names set` |
| `typechecker/mod.rs:infer_expression` | `// 2026-07-18: Return (Type, Provenance) pair for pointer origin tracking` |
| `typechecker/mod.rs:is_mutable_location` | `// 2026-07-18: State fields → mutable Ptr<T>, let bindings → const` |
| `ast/types.rs:PtrConst` | `// 2026-07-18: Read-only pointer variant for let-bindings` |
| `transition_graph.rs:write_set_from_expr` | `// 2026-07-18: Provenance-based write set refinement` |
| `loop_engine/:memory_mode_fields` | `// 2026-07-18: Borrowed fields force memory mode, not SSA` |

### 8d. Per-Commit Checklist

1. `cargo test --lib` — all tests pass
2. `cargo build` — no warnings
3. Run Praetor on new/changed files
4. Update architecture docs if API contracts changed
5. Log bugs/gotchas in BUGS.md
6. Rationale comments on every change
7. Kani harnesses for all newly written or modified functions

---

## File Manifest

### Modified Files

| File | Phase | Change |
|------|-------|--------|
| `src/analysis/provenance.rs` | 1 | Fix `is_local_provenance` stub → accept `local_names` set |
| `src/analysis/provenance.rs` | 1 | Update `check_dangling_ptrs` to pass `local_names` |
| `src/analysis/provenance.rs` | 4 | Add `write_set_from_prov` helper |
| `src/compile.rs` | 1 | Wire `check_dangling_ptrs` into compilation pipeline |
| `src/typechecker/mod.rs` | 2 | Change `infer_expression` to return `(Type, Provenance)` |
| `src/typechecker/mod.rs` | 2 | Add `infer_type_only` convenience wrapper |
| `src/typechecker/mod.rs` | 3 | Add `is_mutable_location`, `is_mutable_subexpr` |
| `src/typechecker/mod.rs` | 3 | Write-through guard for `PtrConst` |
| `src/ast/types.rs` | 3 | Add `PtrConst` variant + `ptr_const()` constructor |
| `src/ast/display.rs` | 3 | Display `PtrConst(inner)` |
| `src/type_universe.rs` | 3 | Register `PtrConst` |
| `src/backend/llvm/types.rs` | 3 | Map `PtrConst` → `"ptr"` |
| `src/backend/llvm/helpers.rs` | 3 | `is_ptr_ty` includes `PtrConst` |
| `src/backend/llvm/emit_expr.rs` | 3 | Handle `PtrConst` in codegen |
| `src/backend/llvm/emit_stmt.rs` | 3 | Handle `PtrConst` in assignment |
| `src/backend/llvm/intrinsics.rs` | 3 | Handle `PtrConst` in Deref#/Index# |
| `src/backend/llvm/mod.rs` | 4 | Add `memory_mode_fields` set |
| `src/backend/llvm/mod.rs` | 4 | Add provenance field to `TypedRegister` |
| `src/analysis/transition_graph.rs` | 4 | Provenance-aware write set computation |
| `src/backend/llvm/loop_engine/` | 5 | SSA fallback for borrowed fields |
| `src/interpreter/value.rs` | 6 | Add `Value::Ptr` |
| `src/interpreter/eval.rs` | 6 | Handle `AddrOf`/`Deref` with `Value::Ptr` |
| `src/symbolic.rs` | 6 | Add `SymbolicValue::Pointer` |
| `src/backend/webstack.rs` | 7 | Handle `AddrOf`/`Deref` |
| `src/backend/circt.rs` | 7 | Handle `AddrOf`/`Deref` |
| All files matching `Type::Ptr` | 3 | Add `Type::PtrConst` match arms |

### New Files

None — all changes are modifications to existing files.

---

## Risk Register

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| **Provenance threading changes all `infer_expression` callers** | High | Medium — ~40 callers to update | Add `infer_type_only` wrapper. Update callers one at a time, testing each. |
| **`PtrConst` match arms missed** | Medium | Medium — build errors catch missing arms | Rust's `match` exhaustiveness check catches missing `PtrConst` arms at compile time. Run `cargo build` after adding the variant. |
| **SSA fallback too conservative** — all fields forced to memory mode when any field is borrowed | Medium | Medium — performance regression | Only force borrowed fields to memory mode, not all fields. The `memory_mode_fields` set is per-field. |
| **Webstack/CIRCT backends drift** — these are active but rarely tested | Low | Low — correctness unaffected for main LLVM path | Add minimal match arms that compile. The backends may not fully support PtrConst semantics, but they won't crash. |
| **Provenance tracking adds typechecker complexity** | Medium | Low — backward-compatible wrapper | `infer_type_only` preserves the old API. Only code that needs provenance uses the new return type. |

---

## Implementation Order

```
Phase 1 (fix is_local_provenance) → independent, high impact
    │
Phase 2 (thread provenance through typechecker) → depends on Phase 1
    │
Phase 3 (PtrConst + const inference) → depends on Phase 2
    │
Phase 4 (parallel txn safety) → depends on Phase 2
    │
Phase 5 (SSA fallback) → depends on Phases 1-3
    │
Phase 6 (interpreter + proof) → depends on Phases 2-3
    │
Phase 7 (Webstack + CIRCT) → depends on Phases 2-3
    │
Phase 8 (tests, docs) → runs alongside all phases
```

**Recommended start:** Phase 1 (fix `is_local_provenance`) — it's the smallest change with the highest impact (unlocks dangling warnings and Alloc# escape analysis). Phases 2-3 can be done in parallel with the allocation strategy plan's Phase 1 (InsertAt/ExtractFrom fixes) since they touch different files.
