# Pointer Borrow Checking + Native Ring Functions

Merge of `docs/plans/2026-07-09-ptr-level3-borrow-checking.md` (Phases 1-6)
with the `<-` arrow restoration and native `ring_push`/`ring_pop` in Brief.

---

## Phase 0: Already Done (this session)

| Item | Status |
|------|--------|
| `Expr::AddrOf(Box<Expr>)` in AST | ✅ Committed |
| `&` unary operator in parser | ✅ Committed |
| `<-` push/pop/discard parser | ✅ Committed |
| `emit_ring_push` / `emit_ring_pop` (hardcoded inline GEP) | ✅ Committed |
| Type-based dispatch via `check_insert_strategy` | ✅ Committed |
| Remove orphaned `src/features/arrow.rs` | ✅ Committed |
| Arena docs in `docs/architecture/` | ✅ Committed |

---

## Phase 1: Type System Foundation (from Ptr L3 plan §9)

| Item | File | What |
|------|------|------|
| 1a. Replace `OwnedRef` match arms | All files | Already done — OwnedRef was already removed |
| 1b. `AddrOf` type inference → `Ptr<T>` | `src/typechecker/mod.rs` | `Expr::AddrOf(inner)` → infers `Ptr<inner_ty>`. Currently returns `Type::Ptr(inner_ty)`. Need const inference (see 1c) |
| 1c. Const inference from context | `src/typechecker/mod.rs` | `&state_field` → `Ptr<T>` (mutable), `&let_binding` → `Ptr<const T>` (read-only). Uses `is_mutable_location()` helper |
| 1d. `Deref` type inference | `src/typechecker/mod.rs` | `*ptr` → unwraps `Ptr(inner)` → `inner`. Error on non-pointer |
| 1e. `Ptr<const T>` in type universe | `src/type_universe.rs` | Add `PtrConst` variant for read-only pointers |
| 1f. Test: type inference for `&` and `*` | `src/typechecker/tests.rs` | Round-trip: `*(&field) == field` |

---

## Phase 2: Pointer Arithmetic + Cast Fixes

**2a. Fix `Expr::Cast` for `Ptr<T>`** (`src/backend/llvm/emit_expr.rs`)

Current: `(Ptr<Int>)i64_val` calls `__int_to_str__` (String path) because both
`String` and `Ptr<T>` map to LLVM `"ptr"`. Fix: check Brief type first:

```
if *target == Type::string() || *target == Type::data() => String runtime helper
else if matches!(target, Type::Ptr(_)) => inttoptr
else if target_ll == "double" => sitofp
else if target_ll == "i64" && src_ll == "double" => fptosi
else => bitcast
```

This enables `(Ptr<i64>)handle` → `inttoptr i64 %handle to ptr`.

**2b. Pointer-offset arithmetic** (`src/backend/llvm/emit_expr.rs`)

Currently `BinaryOp::Add` on two integers emits `add nsw i64 %a, %b`. When one
operand is `Type::Ptr(_)`, emit `getelementptr T, ptr %base, i64 %offset`:

```
if l.ty == Ptr(T) || r.ty == Ptr(T):
    // ptr + offset → GEP
    if l.ty == Ptr(T): base=l, offset=r
    else: base=r, offset=l
    %gep = getelementptr T, ptr %base, i64 %offset
    return Ptr(T)
else:
    // normal arithmetic
```

This enables `buf + 3` → `GEP i64, ptr %buf, i64 3`.

**2c. Test: cast and pointer arithmetic** | LLVM backend tests

---

## Phase 3: Implement `ring_push` / `ring_pop` in Brief

**3a. Write the Brief implementations** (`lib/std/os/ring.bv`)

```brief
defn ring_push(handle: i64, val: i64) {
    let buf: Ptr<i64> = (Ptr<i64>)handle;
    let tail: i64 = *buf;
    let mask: i64 = *(buf + 3);
    let data: Ptr<i64> = (Ptr<i64>)*buf;
    buf[0] = (tail + 1) & mask;
    data[tail & mask] = val;
};

defn ring_pop(handle: i64) -> i64 {
    let buf: Ptr<i64> = (Ptr<i64>)handle;
    let head: i64 = *buf;
    let tail: i64 = *(buf + 1);
    let mask: i64 = *(buf + 3);
    let empty: Bool = head == tail;
    let data: Ptr<i64> = (Ptr<i64>)*(buf);
    let val: i64 = data[head & mask];
    let new_head: i64 = (head + 1) & mask;
    *buf = empty ? head : new_head;
    ret empty ? 0 : val;
};
```

**3b. Remove hardcoded `Intrinsic::RingPush`/`RingPop`** (`src/backend/llvm/intrinsics.rs`)
Replace with codegen for `call @ring_push()`. LLVM -O3 inlines the 15-instruction
body automatically.

**3c. Remove hardcoded `emit_ring_push`/`emit_ring_pop`** (`src/backend/llvm/emit_stmt.rs`)
The `<-` codegen should emit `call @ring_push(handle, val)` instead of inline
GEP instructions. The strategy property `insert_at: "ring_push"` tells the
codegen which function to call.

**3d. Property → function resolution** (`src/backend/llvm/emit_stmt.rs`)
When `check_insert_strategy` returns `"ring_push"`, look up the function
definition and emit a call to it:

```rust
fn emit_push_by_strategy(backend, out, target, val, strategy: &str) {
    match strategy {
        "ring_push" => {
            // Emit: call void @ring_push(handle, val)
            let handle = emit_addr_of(target);
            writeln!("call void @ring_push(i64 {}, i64 {})", handle, val);
        }
        _ => error!("unknown insert strategy: {}", strategy),
    }
}
```

---

## Phase 4: Borrow Warnings + Provenance (from Ptr L3 plan §11)

**4a. Provenance data structure** (`src/analysis/provenance.rs`)
New file with `Provenance` enum and `ProvenanceMap`:
- `Known(String)` — points to a known variable
- `FieldAccess { base, field }` — points to a struct field
- `Index { base, index }` — points to an array element
- `Deref(Box<Provenance>)` — points through a dereference
- `Unknown` — provenance lost

**4b. Provenance inference** (in typechecker, alongside type inference)
Thread `Provenance` through the expression tree:
- `Identifier(name)` → `Known(name)`
- `AddrOf(inner)` → same provenance as inner
- `Deref(ptr)` → unwrap one level of provenance
- `FieldAccess(base, f)` → `FieldAccess { base, f }`
- Other → `Unknown`

**4c. Dangling pointer warnings** (post-typecheck scan)
When `&state_field = &local_var`, warn if local is not a state field.
Always a warning with restructure suggestion — never a hard error.

**4d. Provenance for parallel txn safety** (`src/analysis/transition_graph.rs`)
Write-set analysis uses provenance to refine aliasing:
- `*p` with `Known("counter")` → writes to `{counter}`
- `*p` with `Unknown` → conservatively conflicts with everything

---

## Phase 5: Arrow Operations Update (from Ptr L3 plan §12)

**5a. Update `<-` codegen** (`src/backend/llvm/emit_stmt.rs`)
Pop/discard currently checks `rhs` for `AddrOf` pattern. With provenance,
verify that the source is a valid collection type (not a random pointer).

**5b. Remove inline GEP path**  
Now that `ring_push`/`ring_pop` are Brief functions, the inline GEP path in
`emit_stmt.rs` is dead code. Remove it. The `<-` codegen calls the strategy
function instead.

---

## Phase 6: Deletions + Cleanup

**6a. Remove `Intrinsic::RingPush`/`RingPop`** | `intrinsics.rs`
**6b. Remove `emit_ring_push`/`emit_ring_pop`** | `emit_stmt.rs`
**6c. Remove `emit_inttoptr` helper** (no longer needed — Cast handles it) | `emit_toplevel.rs` or helpers
**6d. Update `docs/architecture/arrow-syntax-and-arena.md`** | Reflect native ring functions

---

## Phase 7: Tests, Benchmarks, Documentation (from Ptr L3 plan §14)

| Test area | Count |
|-----------|-------|
| Typechecker: `&field` → `Ptr<Int>` | 3 |
| Typechecker: `*ptr` → inner type | 2 |
| Typechecker: `*(&field) == field` | 2 |
| Typechecker: const coercion | 2 |
| LLVM: `(Ptr<T>)i64` → inttoptr | 2 |
| LLVM: `buf + N` → GEP | 2 |
| LLVM: `<-` via function call | 3 |
| Dangling warning | 3 |
| Provenance for parallel safety | 2 |

---

## Files Changed

| File | Phase | Change |
|------|-------|--------|
| `src/typechecker/mod.rs` | 1 | `AddrOf`→`Ptr<T>`, `Deref`→unwraps, const inference |
| `src/type_universe.rs` | 1 | Add `PtrConst` variant |
| `src/backend/llvm/emit_expr.rs` | 2a | Fix Cast for Ptr<T> vs String |
| `src/backend/llvm/emit_expr.rs` | 2b | Pointer-offset BinaryOp → GEP |
| `lib/std/os/ring.bv` | 3a | Native ring_push/ring_pop in Brief |
| `src/backend/llvm/intrinsics.rs` | 3b, 6a | Remove RingPush/RingPop |
| `src/backend/llvm/emit_stmt.rs` | 3c, 5b, 6b | strategy-based dispatch, remove inline GEP |
| `src/backend/llvm/mod.rs` | 3d | Property→function resolution helper |
| `src/analysis/provenance.rs` | 4a | NEW — Provenance enum + map |
| `src/analysis/transition_graph.rs` | 4d | Provenance-based write set refinement |
| `src/annotator.rs`, `src/ast/display.rs`, etc. | 1 | `PtrConst` match arms |
| `docs/architecture/arrow-syntax-and-arena.md` | 6d | Update for native ring fns |

---

## Ordering

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6 → Phase 7
```

Phases 1-2 are prerequisites for 3 (ring_push needs Ptr cast + arithmetic to compile).
Phase 3 removes the hardcoded intrinsics; don't do it before Phase 2 is tested.
Phase 4-5 build on top without dependency on Phase 3.
