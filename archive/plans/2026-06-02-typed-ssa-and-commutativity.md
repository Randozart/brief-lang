# Next Phase: Typed SSA + Commutativity Fix — 2026-06-02

Timestamp: 2026-06-02 16:30 UTC  
Based on: AI agent evaluation + manual investigation of float_math_nonzero 2.25× gap.

---

## Context

The float_math_nonzero 2.25× gap (0.371s vs 0.165s) is a **microarchitectural scheduling artifact**,
not an LLVM IR or codegen quality issue. Both Briv and C produce identical 15-instruction
AVX hot loops (vmulss/vaddss, no spills, same alignment, same dependency structure).
Option 4 (alloca-based loop) would converge to the same IR through SROA+mem2reg.

**Conclusion**: The gap is at the µop level (pipeline port contention, register renaming,
µop fusion boundaries). It cannot be closed through IR restructuring. Correctness must
take priority.

---

## Implementation Order

### Phase 3A — Step 5: Commutativity Fix (A6)
**File**: `src/backend/llvm.rs` → `extract_trigger_keys`
**Effort**: 1 line, 30 seconds.

Remove unreachable second match arm:
```rust
// BEFORE:
Expr::Eq(l, r) | Expr::Eq(r, l) => {

// AFTER:
Expr::Eq(l, r) => {
```

The `| Expr::Eq(r, l)` pattern matches positionally — it's identical to the first
arm and can never fire.

---

### Phase 3B — Step 2: Typed SSA (A4)
**Effort**: ~200 lines, medium difficulty.

#### 1. Define TypedRegister (`src/backend/llvm.rs`)
```rust
#[derive(Debug, Clone)]
pub struct TypedRegister {
    pub name: String,
    pub ty: Type,
}
```

#### 2. Change `emit_expr` signature
```rust
// BEFORE:
fn emit_expr(&mut self, out: &mut String, expr: &Expr, indent: &str) -> String

// AFTER:
fn emit_expr(&mut self, out: &mut String, expr: &Expr, indent: &str) -> TypedRegister
```

#### 3. Return type for each expression kind

| Expression | IR | Returns |
|-----------|-----|---------|
| `Expr::Integer(n)` | `add i64 0, n` | `Type::Int` |
| `Expr::Bool(b)` | `add i64 0, 1/0` | `Type::Bool` |
| `Expr::Float(f)` | bitcast → trunc boxing (same as now) | `Type::Float` |
| `Expr::String(s)` | ptrtoint boxing (same as now) | `Type::String` |
| `Expr::Char(c)` | `zext i32 c to i64` | `Type::Char` |
| `Expr::Identifier(n)` | extractvalue from state | Type from `field_types[idx]` |
| `Expr::OwnedRef(n)` | Same as Identifier | Type from `field_types[idx]` |
| `Expr::PriorState(n)` | Same as Identifier | Type from `field_types[idx]` |
| `Expr::Call(name, args)` | Call instruction **+ resolve type** | Type from `frgn_map` or `defn_params` |
| `Expr::Add/Sub/Mul/Div(l, r)` | Dispatch to `emit_binop` | Float or Int based on operands |
| `Expr::Eq/Ne/Lt/Le/Gt/Ge(l, r)` | Dispatch to `emit_fcmp` | `Type::Bool` |
| `Expr::And/Or(l, r)` | `and i64` / `or i64` | `Type::Bool` |
| `Expr::Not(a)` | `xor i64 a, 1` | `Type::Bool` |
| `Expr::Neg(a)` | Sub or fsub based on type | Float or Int based on operand |
| `Expr::BitAnd/Or/Xor/Shl/Shr` | int ops | `Type::Int` |
| `Expr::BitNot(a)` | `xor i64 a, -1` | `Type::Int` |
| `Expr::ListLiteral(elems)` | ptrtoint | `Type::Data` |
| `Expr::ListIndex(list, idx)` | pointer math | Type from `field_types` |
| `Expr::Slice { .. }` | pointer math | `Type::Data` |
| `Expr::ListLen(a)` | structure field access | `Type::Int` |
| `Expr::Concat(a, b)` | pointer math | `Type::Data` |
| `Expr::Cast(a, ty)` | bitcast/trunc/zext | The target `Type` |
| `Expr::Term` | N/A (handled earlier) | N/A |

#### 4. Update `emit_binop` to use type dispatch
```rust
fn emit_binop(&mut self, out: &mut String, indent: &str, v: &str, 
              l: &Expr, r: &Expr, int_op: &str, float_op: &str) -> Type {
    let left = self.emit_expr(out, l, indent);
    let right = self.emit_expr(out, r, indent);
    if left.ty == Type::Float || right.ty == Type::Float {
        let fa = self.i64_to_float_reg(out, &left.name, indent);
        let fb = self.i64_to_float_reg(out, &right.name, indent);
        let fr = format!("%bfr{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = {} float {}, {}", indent, fr, float_op, fa, fb).ok();
        let fi = format!("%bfi{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = bitcast float {} to i32", indent, fi, fr).ok();
        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, fi).ok();
        Type::Float
    } else {
        writeln!(out, "{}{} = {} i64 {}, {}", indent, v, int_op, left.name, right.name).ok();
        Type::Int
    }
}
```

#### 5. Update `emit_fcmp` similarly
```rust
fn emit_fcmp(&mut self, out: &mut String, indent: &str, v: &str,
             l: &Expr, r: &Expr, cond: &str) -> Type {
    let left = self.emit_expr(out, l, indent);
    let right = self.emit_expr(out, r, indent);
    if left.ty == Type::Float || right.ty == Type::Float {
        let fa = self.i64_to_float_reg(out, &left.name, indent);
        let fb = self.i64_to_float_reg(out, &right.name, indent);
        writeln!(out, "{}{} = fcmp fast {} float {}, {}", indent, v, cond, fa, fb).ok();
        writeln!(out, "{}{} = zext i1 {} to i64", indent, 
                 format!("%bz{}", self.txn_counter), v).ok();
        // ... returns Type::Bool
    } else {
        writeln!(out, "{}{} = icmp {} i64 {}, {}", indent, v, cond, left.name, right.name).ok();
        // ... returns Type::Bool
    }
}
```

#### 6. Remove dead code
- `fn is_float_expr(&self, expr, local_floats) -> bool` — DELETE
- `fn is_float_expr_pre_cg(&self, expr, local_floats) -> bool` — DELETE
- `register_types: HashMap<String, Type>` — DELETE field + all writes/reads
- `fn i64_to_float_reg()` — KEEP (still needed for struct field boxing)

#### 7. Update all 49 call sites

Each call site follows the same pattern:
```rust
// BEFORE:
let reg = self.emit_expr(out, expr, indent);
// ... use `reg` as register name ...

// AFTER:
let tr = self.emit_expr(out, expr, indent);
// ... use `tr.name` as register name, `tr.ty` for type info ...
```

#### Risk Mitigation
- Build after every 10 expression arms to isolate type errors
- The IR emission does NOT change — only the return type changes
- `368 tests must pass` is the gate

---

## Files Changed

| File | Changes | Lines |
|------|---------|-------|
| `src/backend/llvm.rs` | TypedRegister, emit_expr rewrite, emit_binop/fcmp, remove is_float_expr, update 49 call sites, commutativity fix | ~200 |
| `src/backend/llvm.rs` (tests) | Update any tests referencing removed APIs | ~10 |

---

## Benchmark Targets (Post-Phase 3B)

| Benchmark | Before | After (est.) | Notes |
|-----------|--------|-------------|-------|
| float_math | 0.011s | 0.011s | No change (O(1)) |
| float_math_nonzero | 0.371s | 0.371s | µarch gap, not IR-related |
| iir_filter | 0.000s | 0.000s | O(1) |
| All others | unchanged | unchanged | No behavioral change |
| **Correctness** | Bug: `Expr::Call` returning float → int codegen | **Fixed**: float calls compile as float ops | |
