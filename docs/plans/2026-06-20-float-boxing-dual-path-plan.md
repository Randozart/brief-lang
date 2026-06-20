# Plan: LLVM Float Boxing — Dual-Path Reconciliation

**Date**: 2026-06-20
**Author**: OpenCode analysis session
**Status**: Plan (not yet implemented)
**Motivation**: Two diverged code paths for float value emission cause `opt -O2` verifier errors in
nbody_newton.ll and nbody_sqrt.ll (`bitcast float %t17 to i32` where `%t17` is i64). This plan
reconciles the paths, then systematically annotates every changed code site with a datetime stamp
explaining WHY that path was chosen.

---

## 1. Evidence: The Two Diverged Float Paths

### Path A: `Expr::Float` (direct handler) — emit_expr.rs:22-28

```rust
Expr::Float(f) => {
    let bits = float_to_llvm_hex(*f);
    let fl = format!("%ff{}", self.txn_counter); self.txn_counter += 1;
    writeln!(out, "{}{} = bitcast i32 {} to float", indent, fl, bits).ok();
    self.reg_float_cache.insert(fl.clone(), fl.clone());  // self-map: %ffN → %ffN
    return TypedRegister { name: fl, ty: Type::Float };
}
```

**What it emits**:
```llvm
%ffN = bitcast i32 <hex> to float
```

Returns `{ name: "%ffN" (float-typed register), ty: Type::Float }`. The register is genuinely `float`.

### Path B: `LiteralExpr::Float` (via `emit_llvm` trait) — literal.rs:96-105

```rust
LiteralExpr::Float(f) => {
    let bits = float_to_llvm_hex(*f);
    let fl = format!("%ff{}", ctx.txn_counter); ctx.txn_counter += 1;
    writeln!(out, "{}{} = bitcast i32 {bits} to float", ..., fl = fl, bits = bits).ok();
    let i32r = format!("%fi{}", ctx.txn_counter); ctx.txn_counter += 1;
    writeln!(out, "{}{} = bitcast float {fl} to i32", ..., i32r = i32r, fl = fl).ok();
    writeln!(out, "{}{} = zext i32 {i32r} to i64", ..., v = v, i32r = i32r).ok();
    ctx.reg_float_cache.insert(v.clone(), fl.clone());  // cross-map: %tM → %ffN
    return TypedRegister { name: v, ty: Type::Float };  // name = %tM (i64), but ty = Float!
}
```

**What it emits**:
```llvm
%ffN = bitcast i32 <hex> to float
%fiN = bitcast float %ffN to i32
%tM  = zext i32 %fiN to i64
```

Returns `{ name: "%tM" (i64 zext result), ty: Type::Float }`. The register is i64 but the type annotation says Float.

### Historical Origin

- **Jun 9** (commit `4782b78`, Phase 1.1 LiteralExpr migration): Both Path A and Path B created.
  Path A kept the original simple native-float emission. Path B followed the general `LiteralExpr`
  pattern where ALL variants boxed to i64 first (Bool used `add i64 0, 1`, Char used
  `zext i32 %cc to i64`). The Float variant retained `Type::Float` because "float is special."
- **Jun 16** (commit `f1df149`, Phase 0 boxing type fix): String/Char/Bool were fixed to return
  correct types (`Type::Int` for boxed values, `Type::Bool` for native i1). The fix note says
  *"Float stays Type::Float (handled specially)"* — the `reg_float_cache` ↔ `native_float_or_box`
  infrastructure was considered sufficient.
- **Jun 20 (this plan)**: Discovery that `adapt_to_i64` (emit_stmt.rs:28-34) naively trusts
  `Type::Float` and `bitcast float %r.name to i32` — does NOT check `reg_float_cache`. When
  `LiteralExpr::Float` is used in a state initializer, `adapt_to_i64` dereferences the i64 register
  name as float → LLVM verifier error.

---

## 2. Bug Mechanism: Chain of Failure

```
Benchmark: nbody_newton.rbv
  → Field: bx0: Float = 0.0
  → Parser produces: Expr::Literal(LiteralExpr::Float(0.0))
    (NOT Expr::Float(0.0), because parser goes through parse_primary → Expr::Literal)
  → emit_inline_init_stores (emit_toplevel.rs:561-577): match { Some(expr) => ... }
    → self.emit_expr(out, &expr, indent)
      → Expr::Literal dispatch (emit_expr.rs:50)
        → LiteralExpr::emit_llvm (literal.rs:96-105)
          → Creates: %ff18 = bitcast i32 0 to float
                     %fi19 = bitcast float %ff18 to i32
                     %t17  = zext i32 %fi19 to i64
          → Returns { name: "%t17", ty: Type::Float }
          → reg_float_cache: "%t17" → "%ff18"
    → self.adapt_to_i64(out, indent, &val_reg)    // val_reg = { name: "%t17", ty: Float }
      → adapt_to_i64 line 28-34:
        if r.ty == Type::Float {
            let bi = "%rbi20";
            writeln!("{bi} = bitcast float %t17 to i32")    // ✗ BUG: %t17 is i64!
            let ze = "%rze21";
            writeln!("{ze} = zext i32 %rbi20 to i64")
        }
    → self.native_float_or_box(out, indent, &val_reg.to_string())
      → Checks cache: "%t17" → "%ff18" ✓ returns "%ff18"
    → store float %ff18, float* %ip2
```

**Result**: Lines 653-657 of nbody_newton.ll produce:
```llvm
%ff18 = bitcast i32 0 to float           ; correct
%fi19 = bitcast float %ff18 to i32       ; correct
%t17  = zext i32 %fi19 to i64            ; correct
%rbi20 = bitcast float %t17 to i32       ; ✗ LLVM ERROR: %t17 is i64, not float
%rze21 = zext i32 %rbi20 to i64          ; would not be reached
store float %ff18, float* %ip2, align 4  ; correct
```

Lines 653-654 are dead (the native float `%ff18` is stored directly at line 657), but the
`adapt_to_i64` call at line 563 generates them and crashes.

---

## 3. Design: Code Path Selection Matrix

The optimal path depends on the USE CASE, not on which AST variant produced the value. We need
FOUR code paths, each annotated with its reasoning:

### Path 1: State Initialization — Direct Native Float Store (BEST)

**When**: `Expr::Float(f)` or `Expr::Literal(LiteralExpr::Float(f))` used as state field initializer.
**Code**: `emit_init_state` lines 356-361, `emit_inline_init_stores` lines 486-490.
**Emitted**: `%ipNb = bitcast i32 <hex> to float` + `store float ...`
**Rationale**: State fields are `float` type. Storing a native float avoids all boxing/unboxing.
**No `adapt_to_i64` needed. No `reg_float_cache` needed.**
**Status**: ALREADY EXISTS for `Expr::Float`, but `Expr::Literal(LiteralExpr::Float(...))` falls
through to the catch-all. NEEDS FIX: add explicit arm for `Expr::Literal(LiteralExpr::Float(_))`.

### Path 2: Expression Value — Return Native Float (BEST for computation)

**When**: Float literal used in arithmetic (`+`, `-`, `*`, `/`), comparisons (`<`, `>`, `==`),
function arguments.
**Code**: `emit_binop` lines 3793-3808, already handles via `ensure_float_reg` → cache.
**Emitted**: `%ffN = bitcast i32 <hex> to float`
**Rationale**: Arithmetic and comparisons need native float. `ensure_float_reg` resolves the
cache. `adapt_to_i64` will be called if the value needs boxing.
**Status**: `Expr::Float` is correct (returns native float). `LiteralExpr::Float` is BROKEN
(returns i64-boxed value). NEEDS FIX: return native float register from `LiteralExpr::Float`.

### Path 3: FFI / Param Marshaling — Box to i64 (NECESSARY for ABI)

**When**: Float value crosses function boundary (C ABI parameter, callable txn param).
**Code**: `emit_toplevel.rs` lines 621-625 (callable txn params).
**Emitted**: `%aiN = bitcast float %raw to i32` + `%acN = zext i32 %aiN to i64`
**Rationale**: The LLVM C ABI passes float as float in registers (xmm), but Brief's internal
representation stores everything in a uniform i64 state slot. When marshaling into a callable
txn, the incoming native float must be boxed to i64. The `reg_float_cache` maps the i64 result
back to the native float for downstream computation.
**Status**: CORRECT. Uses `reg_float_cache.insert(conv, raw)` to preserve native float.

### Path 4: Intrinsic Float Returns — Box Result to i64 + Cache Bridge (NECESSARY)

**When**: `sin#(x)`, `cos#(x)`, `pow#(x,y)` etc. return float from C ABI.
**Code**: `emit_expr.rs` lines 460-488.
**Emitted**: `%call_result = call float @sinf(float %fa)` + `%fbi = bitcast float %call to i32`
+ `%fze = zext i32 %fbi to i64`
**Rationale**: C ABI returns float in xmm0. The result is boxed to i64 for uniform storage in
Brief's register model. The `reg_float_cache` maps `%fze → %call_result` so downstream code
can recover the native float.
**Status**: CORRECT. Returns `{ name: "%fze", ty: Type::Float }` which has the same issue as
Path B (i64 register with Float type), BUT: all callers of intrinsic results either go through
`emit_binop` (which uses `ensure_float_reg` → cache) or call `native_float_or_box`. No caller
calls `adapt_to_i64` on an intrinsic float result unconditionally.

---

## 4. Fix Plan

### Fix 1: `LiteralExpr::Float::emit_llvm` — Return Native Float Register

**File**: `src/features/literal.rs`
**Lines**: 96-105
**Change**: Return `{ name: fl, ty: Type::Float }` instead of `{ name: v, ty: Type::Float }`.

The instruction `%tM = zext i32 %fiN to i64` at line 102 becomes dead code (LLVM DCE removes it).
This unifies Path B with Path A: both return `%ffN` (native float) with `Type::Float`.

**Cache behavior**: Change to `ctx.reg_float_cache.insert(fl.clone(), fl.clone())` matching
`Expr::Float`'s self-map pattern.

**Annotated result**:
```rust
// 2026-06-20: Return native float register, matching Expr::Float (emit_expr.rs:22-28).
// Previously returned the i64-boxed value with Type::Float, which caused adapt_to_i64
// to bitcast an i64 register as float. See docs/plans/2026-06-20-float-boxing-dual-path-plan.md
LiteralExpr::Float(f) => {
    let bits = crate::backend::llvm::float_to_llvm_hex(*f);
    let fl = format!("%ff{}", ctx.txn_counter); ctx.txn_counter += 1;
    writeln!(out, "{indent}{fl} = bitcast i32 {bits} to float", indent = "", fl = fl, bits = bits).ok();
    ctx.reg_float_cache.insert(fl.clone(), fl.clone());
    crate::backend::llvm::TypedRegister { name: fl, ty: Type::Float }
}
```

### Fix 2: `adapt_to_i64` — Check `reg_float_cache` for Belt-and-Suspenders

**File**: `src/backend/llvm/emit_stmt.rs`
**Line**: 28-34
**Change**: Before `bitcast float %r.name to i32`, check the cache first.

```rust
// 2026-06-20: Check reg_float_cache before bitcasting — guarantees correctness even if
// a future code path returns Type::Float with an i64 register name (e.g. intrinsic returns).
// Dual-path defense: the cache is the source of truth for native float ↔ i64-boxed mappings.
} else if r.ty == Type::Float {
    if let Some(cached) = self.reg_float_cache.get(&r.name) {
        // Native float counterpart is cached — box it to i64 properly.
        let bi = format!("%rbi{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = bitcast float {} to i32", indent, bi, cached).ok();
        let ze = format!("%rze{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = zext i32 {} to i64", indent, ze, bi).ok();
        ze
    } else {
        // r.name is genuinely a native float register.
        let bi = format!("%rbi{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = bitcast float {} to i32", indent, bi, r.name).ok();
        let ze = format!("%rze{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = zext i32 {} to i64", indent, ze, bi).ok();
        ze
    }
```

### Fix 3: `emit_init_state` / `emit_inline_init_stores` — Add Explicit `LiteralExpr::Float` Arm

**File**: `src/backend/llvm/emit_toplevel.rs`
**Lines**: emit_init_state ~351, emit_inline_init_stores ~481

Add an explicit arm for `Some(Expr::Literal(lit))` where `lit` is `LiteralExpr::Float` or
`LiteralExpr::Neg(LiteralExpr::Float(...))`, matching the existing `Expr::Float` / `Expr::Neg(Expr::Float)`.

This prevents the catch-all from unnecessarily boxing a literal float to i64 and back.

**Annotated addition** (pattern follows existing `Expr::Float` arm at lines 486-490):
```rust
// 2026-06-20: Handle LiteralExpr::Float directly, matching Expr::Float arm above.
// Without this, the catch-all boxes the float to i64 and immediately unboxes it back,
// producing dead IR instructions. LLVM DCE would clean them, but they may cause verifier
// errors if they cross adapt_to_i64 before DCE runs.
Some(Expr::Literal(lit)) if matches!(lit.as_ref(), crate::features::literal::LiteralExpr::Float(_)) => {
    if let crate::features::literal::LiteralExpr::Float(f) = lit.as_ref() {
        let h = float_to_llvm_hex(*f);
        let bits_reg = format!("%ip_{}b", idx);
        writeln!(out, "{}{} = bitcast i32 {} to float", indent, bits_reg, h).ok();
        writeln!(out, "{}store float {}, float* {}, align {}", indent, bits_reg, p, self.align_of("float")).ok();
    }
}
```

---

## 5. Why Each Path Is Optimal

| Use Case | Optimal Path | Alternative Considered | Why Optimal |
|----------|-------------|----------------------|-------------|
| State field init | Direct `bitcast i32 <hex> to float` | `emit_expr` → `adapt_to_i64` → `native_float_or_box` | Zero dead IR. The compiler knows the value at codegen time. No cache needed. |
| Expression computation | Native float `%ffN` + `Type::Float` | i64-boxed + `reg_float_cache` | Arithmetic (fadd, fsub, fmul, fdiv) operates on native float. `ensure_float_reg` resolves via cache. Simpler IR for LLVM to optimize. |
| C ABI parameter marshaling | Box to i64 + cache bridge | Store direct float in state field | ABI expects float in xmm registers, but Brief's cross-function internal ABI is i64-uniform. The cache preserves the native float for consumption within the same function. |
| Intrinsic float return | Box result to i64 + cache bridge | Return native float directly | Consistent with Brief's internal ABI (all values are i64). Cache recovers native float for downstream computation. |
| `adapt_to_i64` on unknown `Type::Float` reg | Check cache first, then `bitcast float` | Assume register name is float | Defense-in-depth. If a new code path returns i64-boxed with Type::Float (like intrinsic returns do), this path handles it correctly instead of crashing. |

---

## 6. DateTime Stamp Convention

Every changed code site gets a comment with:
```
// 2026-06-20: <reason> Why this path is optimal vs alternatives.
```

Purpose: Two years from now, a reader sees `bitcast float %x to i32` and knows whether it's
safe (because %x is genuinely float) or if they need to check the cache first. The stamp
documents the design intent at the time of writing.

---

## 7. Implementation Order

1. **Fix `LiteralExpr::Float::emit_llvm`** (literal.rs) — return native float. This fixes the root
   cause: the type annotation now matches the register type.
2. **Fix `adapt_to_i64`** (emit_stmt.rs) — add cache check. This is the defense-in-depth layer.
3. **Add `LiteralExpr::Float` explicit arm** in `emit_init_state` / `emit_inline_init_stores`
   (emit_toplevel.rs) — avoid unnecessary boxing in state init. This is a cleanliness
   optimization (prevents dead IR).
4. **Run tests**: `cargo test --lib`
5. **Run benchmarks**: `bash benchmarks/build_and_bench.sh`
6. **Update docs**: Add this plan to `docs/architecture/fixes/float-boxing-dual-path.md`

---

## 8. Acceptance Criteria

- [ ] `opt -O2 -S benchmarks/nbody_newton.ll -disable-output` passes (exit 0)
- [ ] `opt -O2 -S benchmarks/nbody_sqrt.ll -disable-output` passes (exit 0)
- [ ] All existing `.ll` files pass `opt -O2 -disable-output`
- [ ] `cargo test --lib` — all tests pass
- [ ] No regression in benchmark timing (nbody_newton, nbody_sqrt produce correct output)
- [ ] Every changed code site has a datetime stamp with rationale
