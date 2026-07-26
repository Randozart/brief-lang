# Plan: Constant Materialization & Float Register Promotion

**Date:** 2026-06-02
**Status:** Plan — ready for implementation
**Version:** 2.0 (production-ready, fully researched)

## Problem

Two systemic inefficiencies in the LLVM backend waste cycles on every benchmark and prevent Brief from decisively beating C on float-heavy workloads.

### Bottleneck A: Constants Loaded from RAM Instead of Inlined as Immediates

At `llvm.rs:1951-1974`, `Expr::Identifier(name)` matching `self.constants` emits a `load`:

```llvm
; Current — load from RAM, even for compile-time-known literals:
%loaded = load i64, i64* @TOTAL, align 8     ; 4-5 cycles (L1 hit)
%result = add i64 0, %loaded                  ; 1 cycle

; Target — immediate encoding, zero memory traffic:
%result = add i64 0, 50000000                 ; 1 cycle, fused in instruction encoding
```

LLVM's `opt -O2` can hoist loads out of loops via LICM, but:
- Wake programs with multiple entry points emit the load in every path
- Programs without `opt` installed load every time
- Even hoisted, the load still appears at least once per function

### Bottleneck B: The i64 Boxing Tax on Float Values (Hidden Cost)

Brief uses `i64` as its universal wire type. Every float operation requires round-tripping through i32→float→i32→i64. Tracing a single Kalman filter iteration:

```llvm
; Load field x0 (3 instructions):
%ev = extractvalue %State %state, 0    ; extract as float from %State
%i32 = bitcast float %ev to i32        ; float → i32
%i64 = zext i32 %i32 to i64            ; i32 → i64  (3 insts, 2 are pure boxing)

; Use in fadd (2 instructions, each operand, plus output):
%a32 = trunc i64 %x0_i64 to i32        ; i64 → i32
%a_flt = bitcast i32 %a32 to float     ; i32 → float
; ... fadd float %a_flt, %b_flt ...
%r32 = bitcast float %result to i32    ; float → i32
%r64 = zext i32 %r32 to i64            ; i32 → i64  (4 insts per float op)

; Store result (3 instructions):
%s32 = trunc i64 %r64 to i32           ; i64 → i32
%s_flt = bitcast i32 %s32 to float     ; i32 → float
; insertvalue into %State               ; stores float directly
```

**Kalman filter cost:**
- 12 field loads × 3 instructions = 36 boxing instructions
- ~60 float operations × 5 instructions = 300 boxing instructions  
- 12 field stores × 2 instructions = 24 boxing instructions (insertvalue skips 1)
- **Total: ~360 boxing instructions per tick**

In C, all 12 float fields live in float registers for the entire loop body. Zero boxing. Zero unboxing. Just `fadd`, `fmul`, `fdiv` with native register operands.

### Bottleneck C: Float Literal Emission Produces 4-Instruction Chain

At `llvm.rs:1884-1891`:

```llvm
; Current — 4 instructions for a single float constant:
%ff0 = bitcast i32 <hex(f)> to float       ; i32 const → float
%fi0 = bitcast float %ff0 to i32           ; float → i32 (why?)
%v   = zext i32 %fi0 to i64               ; i32 → i64
```

When used in float context, the backend reverses this: `trunc i64 → bitcast i32 → float`. **6 instructions** for what should be **1**: `bitcast i32 <hex> to float`.

### Bottleneck D: `emit_exit_expr` is a Parallel Expression Emitter

The `emit_exit_expr` function at lines 2533-2639 is a **completely separate expression emitter** with its own handling of integers, bools, identifiers, comparisons, and logic ops. It does NOT call `emit_expr`. This means:
1. Every optimization to `emit_expr` must be duplicated
2. Constant inlining in `emit_expr` doesn't apply to exit conditions
3. The exit condition (used every tick in wake programs) loads constants from RAM

## Solution Part A: `EmitValue` Enum for Immediate Inlining

### Complete Call Site Audit

The investigation found **49 call sites** of `emit_expr`, categorized by how the result is consumed:

| Category | Count | Can accept immediate? | Sites |
|----------|-------|----------------------|-------|
| Arithmetic binop (add/sub/mul/div) | 4 | **Yes** | llvm.rs:1998-2001 |
| Comparison (eq/ne/lt/le/gt/ge) | 6 | **Yes** | llvm.rs:2004-2009 |
| Bitwise (and/or/xor/shl/lshr/srem) | 7 | **Yes** | llvm.rs:2002, 2011-2038 |
| Boolean ops (and/or/not/neg) | 4 | **Yes** | llvm.rs:2013-2038 |
| Store/call operands | 6 | **No** (force_reg) | llvm.rs:1724, 2047, 2084 |
| Return value (ret i64) | 2 | **No** (force_reg) | llvm.rs:1694, 1707 |
| Precondition check (icmp + br) | 5 | **No** (force_reg) | llvm.rs:1609, 1629, 1648, 1790, 2839 |
| Cast/trg_load operands | 1 | **No** | llvm.rs:2170 |
| Discarded (side-effect only) | 6 | N/A | llvm.rs:1716, 1869, 1992, 2143, 2163-2166 |
| Block/Tuple/FieldAccess wrappers | 8 | N/A | llvm.rs:2164-2183 |

**~17 sites can accept immediates directly.** ~6 sites require `force_reg` for store/call. The rest are either discarded or wrapped.

### Implementation

```rust
#[derive(Clone)]
enum EmitValue {
    Register(String),
    Immediate(i64),
}

// Render for LLVM IR operand position:
fn render(&self, val: &EmitValue) -> String {
    match val {
        EmitValue::Register(r) => r.clone(),
        EmitValue::Immediate(n) => n.to_string(),
    }
}

// Force register for sites that need one:
fn force_reg(&mut self, out: &mut String, val: &EmitValue, indent: &str) -> String {
    match val {
        EmitValue::Register(r) => r.clone(),
        EmitValue::Immediate(n) => {
            let r = format!("%imm{}", self.txn_counter);
            self.txn_counter += 1;
            writeln!(out, "{}{} = add i64 0, {}", indent, r, n).ok();
            r
        }
    }
}
```

### `emit_expr` changes

```rust
Expr::Integer(n) => return EmitValue::Immediate(*n),
Expr::Bool(b) => return EmitValue::Immediate(if *b { 1 } else { 0 }),

// Constants with literal values:
} else if let Some((ty, expr)) = self.constants.get(name) {
    if let Some(val) = extract_literal_int(expr) {
        return EmitValue::Immediate(val);
    }
    // fall through to existing load
}

// Float constants get the dual-emission treatment:
Expr::Float(f) => {
    let (i64_reg, _float_reg) = self.emit_float_literal(out, *f, indent);
    return EmitValue::Register(i64_reg);
}
```

## Solution Part B: Float Register Promotion (The Big Win)

The Kalman filter spends ~360 instructions per tick on float boxing. **Bypass the boxing entirely** inside transaction bodies.

### How: Per-Transaction Float Extraction as Native `float` Type

The `%State` struct already stores float fields as `float` type. When `extractvalue` extracts a `float` field, LLVM's SROA can keep it in a float register. The problem is the intermediate `i64 → i32 → float` conversion.

**Fix:** In SSA mode (`ssa_state_reg` is `Some`), when the field type is `"float"`, emit:

```rust
// In Expr::Identifier handler, SSA path (llvm.rs:1909-1934):
if let Some(ref ssa_reg) = self.ssa_state_reg {
    if let Some(&idx) = self.field_index_map.get(name) {
        let ty = &self.field_types[idx];
        if ty == "float" {
            // Extract directly as float — no i64 boxing!
            let flt = format!("%flt_{}", name);
            writeln!(out, "{}{} = extractvalue %State {}, {}", indent, flt, ssa_reg, idx).ok();
            self.register_types.insert(flt.clone(), Type::Float);

            // ALSO emit the i64 form for stores/comparisons that need it:
            let i32_f = format!("%fi_{}", self.txn_counter); self.txn_counter += 1;
            let i64_r = format!("%fz_{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "{}{} = bitcast float {} to i32", indent, i32_f, flt).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, i64_r, i32_f).ok();

            // Cache both: float form for math, i64 form for store
            self.reg_float_cache.insert(ssa_reg.to_string() + name, flt.clone());
            return EmitValue::Register(i64_r); // i64 form for compatibility
        }
    }
}
```

The key: the `float` register is emitted once, then `to_float_reg()` checks `self.register_types` and returns the cached float register directly — zero additional instructions.

### `to_float_reg` with caching:

```rust
fn to_float_reg(&mut self, out: &mut String, reg: &str, indent: &str) -> String {
    // If already a float register, return it directly
    if self.register_types.get(reg) == Some(&Type::Float) {
        return reg.to_string();
    }
    // If we have a cached float form, use it
    if let Some(cached) = self.reg_float_cache.get(reg) {
        return cached.clone();
    }
    // Otherwise, convert: i64 → trunc → bitcast → float
    let tr = format!("%ftr{}", self.txn_counter); self.txn_counter += 1;
    let fl = format!("%ffl{}", self.txn_counter); self.txn_counter += 1;
    writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, reg).ok();
    writeln!(out, "{}{} = bitcast i32 {} to float", indent, fl, tr).ok();
    self.register_types.insert(fl.clone(), Type::Float);
    fl
}
```

### Impact on Kalman filter (projected):

| Metric | Before | After |
|--------|--------|-------|
| Float boxing instructions per tick | ~360 | ~48 (only for stores to state) |
| Float register allocation | i64 registers (XMM underused) | Native float registers (XMM used fully) |
| Runtime (50M iters) | 0.71s | **~0.45-0.50s** |
| vs C (0.75s) | +5% | **~33-40% faster than C** |

## Solution Part C: `llvm.assume` for Branchless Preconditions

For `node work [count < bound] { &count = count + 1; }`, the precondition emits:

```llvm
%cond = icmp slt i64 %count, %bound
br i1 %cond, label %fire, label %post       ; taken N-1 times, not-taken once
```

The proof engine **proves** convergence — `count < bound` is always true until the final iteration. Tell LLVM:

```llvm
%cond = icmp slt i64 %count, %bound
call void @llvm.assume(i1 %cond)             ; NEW: tells LLVM %cond is always true
br i1 %cond, label %fire, label %post        ; LLVM now removes this branch
```

**Effect:** The conditional branch is converted to an unconditional jump. The `cmp` instruction may still exist (needed for the loop exit), but the branch is eliminated from the critical path.

**Implementation** (in `emit_folded_loop`, after precondition check):
```rust
if self.proof_engine_proves_convergence(txn_name) {
    writeln!(out, "  call void @llvm.assume(i1 %cond)").ok();
}
```

## Solution Part D: Compile-Time Peephole Constant Folding

When `emit_binop` sees both operands as compile-time constants, fold them at the Brief compiler level:

```rust
fn emit_binop(&mut self, out: &mut String, indent: &str, v: &str,
    l: &Expr, r: &Expr, int_op: &str, _float_op: &str) -> EmitValue
{
    // Peephole: constant fold when both operands are literals
    if let (Expr::Integer(li), Expr::Integer(ri)) = (l, r) {
        let result = match int_op {
            "add" => li.wrapping_add(*ri),
            "sub" => li.wrapping_sub(*ri),
            "mul" => li.wrapping_mul(*ri),
            "sdiv" if *ri != 0 => li / ri,
            "and" => li & ri,
            "or"  => li | ri,
            "xor" => li ^ ri,
            _ => return self.emit_binop_normal(out, indent, v, l, r, int_op, _float_op),
        };
        writeln!(out, "{}{} = {} i64 {}, {}", indent, v, int_op, result, 0).ok();
        return EmitValue::Register(v.to_string());
    }
    self.emit_binop_normal(out, indent, v, l, r, int_op, _float_op)
}
```

**When this matters:** Programs with heavily constant-foldable expressions, like address calculations or precomputed matrix coefficients.

## Solution Part E: Float Literal Caching

When `Expr::Float(f)` is encountered:

```rust
fn emit_float_literal(&mut self, out: &mut String, f: f64, indent: &str) -> (String, String) {
    let hex = float_to_llvm_hex(f);
    if let Some(cached) = self.float_literal_cache.get(&hex) {
        return cached.clone();
    }
    let i64_reg = format!("%flt_i64_{}", self.txn_counter); self.txn_counter += 1;
    let float_reg = format!("%flt_f_{}", self.txn_counter); self.txn_counter += 1;

    writeln!(out, "{}{} = bitcast i32 {} to i64",  indent, i64_reg,  hex).ok();
    writeln!(out, "{}{} = bitcast i32 {} to float", indent, float_reg, hex).ok();
    // Only register the float form for float context lookups
    self.register_types.insert(float_reg.clone(), Type::Float);
    self.float_literal_cache.insert(hex, (i64_reg.clone(), float_reg.clone()));
    (i64_reg, float_reg)
}
```

Subsequent uses of the same literal reuse cached registers — zero instructions.

## Solution Part F: `emit_exit_expr` Refactoring

The `emit_exit_expr` (lines 2533-2639) handles `#!exit` conditions and is a parallel emitter to `emit_expr`. It has its own handling of identifiers, comparisons, and logic ops.

**Phase 1: Decouple.** Refactor `emit_exit_expr` to call `emit_expr` for leaf expressions (integers, identifiers, bools, floats) while retaining its own handling of comparisons and logic (because exit conditions use `icmp` + `zext` patterns, not `emit_expr`'s generic `i64` returns).

**Phase 2: Unify.** Eventually fold `emit_exit_expr`'s comparison/logic handling into `emit_expr` with a mode flag.

**Phase 1 only requires ~30 lines** — capture `emit_expr`'s `EmitValue` and call `force_reg()` for comparison operands.

## Implementation Summary

| Change | Lines | File | Priority |
|--------|-------|------|----------|
| `EmitValue` enum + `render()` + `force_reg()` | ~30 | llvm.rs | **Immediate** |
| `emit_expr` return type change, all arms | ~40 | llvm.rs | **Immediate** |
| ~17 arithmetic callers use `render()` | ~15 | llvm.rs | **Immediate** |
| ~6 store/call callers use `force_reg()` | ~5 | llvm.rs | **Immediate** |
| **Float register promotion in SSA mode** | ~30 | llvm.rs | **HIGH IMPACT** |
| `to_float_reg()` with cache | ~15 | llvm.rs | **HIGH IMPACT** |
| `emit_float_literal()` + `float_literal_cache` | ~20 | llvm.rs | Medium |
| `llvm.assume` for convergent preconditions | ~5 | llvm.rs | Medium |
| Peephole constant folding in `emit_binop` | ~20 | llvm.rs | Low |
| Constant deduplication in `generate()` | ~15 | llvm.rs | Low |
| `emit_exit_expr` Phase 1 refactor | ~30 | llvm.rs | Low |
| `reg_float_cache` field + init | ~5 | llvm.rs | **HIGH IMPACT** |

## How Brief Beats C on This

| Optimization | Brief | C (world-class) |
|-------------|-------|-----------------|
| Constant immediates | `add i64 0, 50000000` (auto) | `cmp rax, 50000000` (manual) |
| Float register promotion | Bypasses i64 boxing inside SSA bodies | Local float variables stay in registers naturally |
| `llvm.assume` branch elimination | Uses proof engine results to remove branches | C cannot prove branches are always taken without `__builtin_unreachable()` |
| Float literal caching | Single `bitcast` per unique literal | Compiler handles this automatically |
| Constant peephole folding | Brief-level folding before LLVM | Clang does this too (no advantage) |
| `emit_exit_expr` unification | Currently duplicated → fixable | N/A |

**C's advantage:** Local variables naturally stay in float registers. Brief's i64 boxing adds overhead that float register promotion eliminates.

**Brief's advantage:** `llvm.assume` on convergent preconditions removes branches that C's optimizer cannot prove are always-taken. The proof engine's convergence proof is information C's compiler fundamentally lacks.

## Benchmark Strategy

### Benchmark 1: Constant-Bound Loop

| Variant | Before | After | C |
|---------|--------|-------|---|
| 1 const (bound) | `load @BOUND` per iter | `add i64 0, 50000000` | `cmp rax, 50000000` immediate |
| 10 consts | 10 loads | 10 immediates | 10 immediates |

### Benchmark 2: Kalman Filter (float-intensive)

| Metric | Before | After | C |
|--------|--------|-------|---|
| Float boxing instructions | ~360/tick | ~48/tick | 0 (native float regs) |
| Runtime (50M) | 0.71s | **~0.45-0.50s** | 0.75s |
| Ratio vs C | +5% (Brief wins) | **+33-40% (Brief wins decisively)** | baseline |

### Benchmark 3: Kalman with `llvm.assume`

| Metric | Before | After |
|--------|--------|-------|
| Branch instructions in loop | 1 (precondition check) | 0 (removed via assume) |
| Runtime impact | baseline | ~2-5% improvement |

### Benchmark 4: Redundant Globals

`const A = 100; const B = 100; const C = 200;`
| Metric | Before | After |
|--------|--------|-------|
| Global declarations | `@A`, `@B`, `@C` (3) | `@A`, `@C` (2, B→A alias) |

## Acceptance Criteria

1. **`const X = 50000000`**: no `load i64, i64* @X` in emitted IR; `50000000` appears as immediate
2. **Float register promotion**: Kalman filter IR shows `fadd` directly on `extractvalue` results without `trunc`/`bitcast` chains
3. **`llvm.assume`**: Convergent folded loops contain `call void @llvm.assume(i1 %cond)` before the branch
4. **Duplicate constants**: Single global declaration for identical values
5. **Float literal cache**: Same float constant used twice → single `bitcast` in IR
6. **`emit_exit_expr`**: `#!exit` conditions correctly use `emit_expr` constants
7. **`force_reg()`**: Used for all `store`/`call`/`switch` operands → valid LLVM IR
8. **`cargo test --lib`**: 368+ tests pass
9. **Kalman filter benchmark**: Brief beats C by ≥10%
