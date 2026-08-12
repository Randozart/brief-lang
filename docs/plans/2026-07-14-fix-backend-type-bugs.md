# Plan: Fix Backend Type-Bug Audit — `fadd i64` and Related Codegen Regressions

## The Problem

The Phase 7 refactoring introduced a **family of instruction-selection bugs** in
the `emit_expr.rs` and `emit_stmt.rs` modules. These bugs produce LLVM IR with
type mismatches (e.g. `fadd i64`) that LLVM's verifier rejects at `llc` time,
blocking all benchmark compilation.

The root cause is a **missed `if is_float` branch** in three out of four
arithmetic operations (`Add`, `Sub`, `Mul`), combined with hardcoded `"i64"`
type annotations in store/return emission. The `Div` arm is correct — it
branches on `is_float` — but the other three arms unconditionally emit
`f`-prefixed floating-point instructions, producing IR like `fadd i64 %a, %b`
that LLVM cannot lower.

Additionally, `emit_unary_op::Neg` always emits `sub i64 0, %x` — correct for
integers but wrong for float operands — and `emit_user_call` always returns
`i64` regardless of the function's declared return type.

## Scope

Five fixes, all in `src/backend/llvm/`:

| # | Location | Severity | Description |
|---|----------|----------|-------------|
| 1 | `emit_expr.rs:269-279` | **Critical** | `Add`, `Sub`, `Mul` unconditionally emit `fadd`/`fsub`/`fmul`; must select integer vs float instruction at runtime |
| 2 | `emit_expr.rs:364` | Minor | `Neg` always uses `sub i64 0, x`; must emit `fsub double -0.0, x` for float operands |
| 3 | `emit_stmt.rs:34,39,43` | **Important** | Store instructions hardcode `"i64"` as the value type; must use `lower_type(&val.ty)` |
| 4 | `emit_stmt.rs:52,60` | **Important** | Return instructions hardcode `"i64"`; must use `lower_type(&reg.ty)` |
| 5 | `emit_expr.rs:256-257` | Minor | `emit_user_call` returns `TypedRegister { ty: Type::int() }` always; should look up real return type when available |

## Detailed Fixes

---

### Fix #1 — `emit_binary_op` Add/Sub/Mul

**File:** `src/backend/llvm/emit_expr.rs:269-279`

Current code (each arm is a single unconditional `writeln!`):

```rust
// Line 268-271 (Add)
crate::ast::BinaryOpKind::Add => {
    writeln!(out, "{}{} = fadd{} {} {}, {}", indent, v, fast, ty_str, l.name, r.name).ok();
    TypedRegister { name: v.to_string(), ty: if is_float { Type::float() } else { Type::int() } }
}
// Line 272-275 (Sub), 276-279 (Mul) — identical pattern with fsub/fmul
```

**Replace** each with an `if is_float` guard exactly like `Div` already has
(lines 281-285). The pattern for all three:

```rust
crate::ast::BinaryOpKind::Add => {
    if is_float {
        writeln!(out, "{}{} = fadd{} {} {}, {}", indent, v, fast, ty_str, l.name, r.name).ok();
    } else {
        writeln!(out, "{}{} = add i64 {}, {}", indent, v, l.name, r.name).ok();
    }
    TypedRegister { name: v.to_string(), ty: if is_float { Type::float() } else { Type::int() } }
}
```

**Key invariant:** The `is_float`, `ty_str`, and `fast` variables at lines 263-266
remain unchanged. The `ty_str`/`fast` combo is only used in the `is_float` branch.
The integer branch hardcodes `i64` because Briev's default integer type is always
`Int` → `i64`.

---

### Fix #2 — `emit_unary_op::Neg` float support

**File:** `src/backend/llvm/emit_expr.rs:364`

Current code (one unconditional instruction):

```rust
crate::ast::UnaryOpKind::Neg => {
    writeln!(out, "{}{} = sub i64 0, {}", indent, v, operand.name).ok();
    TypedRegister { name: v.to_string(), ty: operand.ty.clone() }
}
```

**Replace** with a float-vs-int guard. The `TypedRegister` output is already
correct (it copies `operand.ty`), so only the `writeln!` needs branching:

```rust
crate::ast::UnaryOpKind::Neg => {
    let is_float = operand.ty == Type::float() || operand.ty == Type::float64();
    if is_float {
        let fty = if operand.ty == Type::float64() { "double" } else { "float" };
        writeln!(out, "{}{} = fsub {} -0.0, {}", indent, v, fty, operand.name).ok();
    } else {
        writeln!(out, "{}{} = sub i64 0, {}", indent, v, operand.name).ok();
    }
    TypedRegister { name: v.to_string(), ty: operand.ty.clone() }
}
```

**Why `fsub X -0.0, %x` and not `fneg X %x`?** `fneg` requires LLVM 11+.
`fsub X -0.0, %x` is valid in all LLVM versions and produces identical machine
code (`XORPS %xmm0, %xmm1` on x86_64, which is a bitwise NOT of the sign bit).

---

### Fix #3 — Store instructions: use actual value type

**File:** `src/backend/llvm/emit_stmt.rs:34,39,43`

Three locations where `val.ty` is available but the LLVM type string is
hardcoded to `"i64"`. The fix is identical at each site.

**Line 33-34** — local variable assign:
```rust
if let Some(reg) = backend.fun.let_bindings.get(name) {
    writeln!(out, "{}store i64 {}, ptr {}", indent, val.name, reg).ok();
```

Change to:
```rust
if let Some(reg) = backend.fun.let_bindings.get(name) {
    let store_ty = crate::backend::llvm::types::lower_type(&val.ty);
    writeln!(out, "{}store {} {}, ptr {}", indent, store_ty, val.name, reg).ok();
```

**Line 38-39** — MMIO volatile store:
```rust
let ptr = backend.fun.gen_reg();
writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, addr).ok();
writeln!(out, "{}store volatile i64 {}, ptr {}", indent, val.name, ptr).ok();
```

Change the second line to:
```rust
let store_ty = crate::backend::llvm::types::lower_type(&val.ty);
writeln!(out, "{}store volatile {} {}, ptr {}", indent, store_ty, val.name, ptr).ok();
```

(Line 38 `inttoptr i64` stays — addresses are always 64-bit.)

**Line 41-43** — state field store via GEP:
```rust
let ptr = backend.fun.gen_reg();
writeln!(out, "{}{} = getelementptr %State, ptr %state, i32 0, i32 {}", indent, ptr, idx).ok();
writeln!(out, "{}store i64 {}, ptr {}", indent, val.name, ptr).ok();
```

Change the third line:
```rust
let store_ty = crate::backend::llvm::types::lower_type(&val.ty);
writeln!(out, "{}store {} {}, ptr {}", indent, store_ty, val.name, ptr).ok();
```

**Note:** `lower_type` is already imported at `emit_stmt.rs:8` via
`use crate::backend::llvm::types::lower_type;`. We just need to call it.

---

### Fix #4 — Return instructions: use actual value type

**File:** `src/backend/llvm/emit_stmt.rs:52,60`

Same fix as #3 but for `ret` instructions:

**Line 51-52** — `term`/`term!` return:
```rust
let reg = backend.emit_expr(out, val, indent);
writeln!(out, "{}ret i64 {}", indent, reg.name).ok();
```

Change to:
```rust
let reg = backend.emit_expr(out, val, indent);
let ret_ty = crate::backend::llvm::types::lower_type(&reg.ty);
writeln!(out, "{}ret {} {}", indent, ret_ty, reg.name).ok();
```

**Line 59-60** — `return` statement:
```rust
let reg = backend.emit_expr(out, val, indent);
writeln!(out, "{}ret i64 {}", indent, reg.name).ok();
```

Change to:
```rust
let reg = backend.emit_expr(out, val, indent);
let ret_ty = crate::backend::llvm::types::lower_type(&reg.ty);
writeln!(out, "{}ret {} {}", indent, ret_ty, reg.name).ok();
```

**Line 105** (`Escape → ret i64 0`): Leave as-is. `Escape` is a default/error
path that always produces `Int` 0. The hardcoded `"i64"` is correct here.

---

### Fix #5 — `emit_user_call` return type

**File:** `src/backend/llvm/emit_expr.rs:252-257`

Current code always returns `TypedRegister { ty: Type::int() }`:

```rust
fn emit_user_call(&mut self, out: &mut String, v: &str, name: &str, ...) -> TypedRegister {
    let arg_regs: Vec<String> = args.iter()
        .map(|a| self.emit_expr(out, a, indent).name)
        .collect();
    writeln!(out, "{}{} = call i64 @{}({})", indent, v, name, arg_regs.join(", ")).ok();
    TypedRegister { name: v.to_string(), ty: Type::int() }
}
```

**Replace** with a lookup into `self.ctx.defn_return_types` (populated in
`src/backend/llvm/mod.rs:1455,1481`):

```rust
fn emit_user_call(&mut self, out: &mut String, v: &str, name: &str, ...) -> TypedRegister {
    let arg_regs: Vec<String> = args.iter()
        .map(|a| self.emit_expr(out, a, indent).name)
        .collect();
    let ret_type = self.ctx.defn_return_types.get(name)
        .and_then(|types| types.first().cloned())
        .unwrap_or(Type::int());
    let ret_llvm = crate::backend::llvm::types::lower_type(&ret_type);
    writeln!(out, "{}{} = call {} @{}({})", indent, v, ret_llvm, name, arg_regs.join(", ")).ok();
    TypedRegister { name: v.to_string(), ty: ret_type }
}
```

**Why this is safe:** If `defn_return_types` doesn't have an entry for `name`
(e.g. the function is imported but import resolution didn't run), the
`.unwrap_or(Type::int())` falls back to the previous behaviour. No existing
program breaks.

**Why `types.first()`:** Briev supports multiple return values. When we get
around to implementing that, `types[0]` is the first return type. For now,
single-return is the only path exercised.

---

## Interaction Map

| Fix | After | Before | Must not affect |
|-----|-------|--------|-----------------|
| 1 | `add i64` for ints, `fadd double`/`fadd float` for floats | `fadd i64` (invalid IR) | integer overflow semantics (LLVM `add` is wrapping — same as `fadd` was trying to be) |
| 2 | `fsub double -0.0` for floats, `sub i64 0` for ints | `sub i64 0` always | integer negation path unchanged |
| 3 | `store {actual} %val, ptr %ptr` | `store i64 %val, ptr %ptr` | any code that assigns to a Bool variable (now correctly uses `i8`) |
| 4 | `ret {actual} %val` | `ret i64 %val` | function return types correctly match the value type |
| 5 | `call {actual_ret} @fn(...)` | `call i64 @fn(...)` | only user-defined function calls (intrinsic calls go through a separate path) |

**No fix weakens an existing optimization** — all changes are additive match
arms or guard clauses. The `_ =>` fallthrough at `emit_expr.rs:352` is
untouched.

## Coding Standards

Every edit **must** follow AGENTS.md:

### 1. Max 2 nesting depth — NO arrowhead code

All conditionals use early-return / guard-clause style:

```rust
// GOOD — max 2 levels
if is_float {
    writeln!(...).ok();
} else {
    writeln!(...).ok();
}

// BAD — 3 levels
if is_float {
    if something_else {
        ...
    }
}
```

If a branch would exceed 2 levels, extract the body into a named helper.

### 2. Rationale comments at every change site

Every modified line gets a comment in the format:

```rust
// 2026-07-14: [op] must branch on is_float — fadd i64 is invalid LLVM IR
```

Location-specific comments:

| Fix | Comment |
|-----|---------|
| `emit_binary_op::Add` | `// 2026-07-14: Add must branch on is_float — fadd i64 is invalid LLVM IR` |
| `emit_binary_op::Sub` | `// 2026-07-14: Sub must branch on is_float — fsub i64 is invalid LLVM IR` |
| `emit_binary_op::Mul` | `// 2026-07-14: Mul must branch on is_float — fmul i64 is invalid LLVM IR` |
| `emit_unary_op::Neg` | `// 2026-07-14: Neg must use fsub for float operands — sub i64 is invalid for doubles` |
| `emit_stmt.rs:34` | `// 2026-07-14: store type must match val.ty — hardcoded i64 breaks bool/float assigns` |
| `emit_stmt.rs:39` | `// 2026-07-14: volatile store type must match val.ty — hardcoded i64 breaks MMIO bools` |
| `emit_stmt.rs:43` | `// 2026-07-14: state field store type must match val.ty — hardcoded i64 breaks bool/float fields` |
| `emit_stmt.rs:52` | `// 2026-07-14: return type must match reg.ty — hardcoded i64 breaks bool/float returns` |
| `emit_stmt.rs:60` | `// 2026-07-14: return type must match reg.ty — hardcoded i64 breaks bool/float returns` |
| `emit_expr.rs:256` | `// 2026-07-14: user call return type from defn_return_types — fall back to i64` |

### 3. Doc comments on every `pub fn`

No new `pub fn` are introduced in this plan. Existing functions keep their doc
comments. No change needed.

### 4. Flat control flow

The `if/else` branches for each fix are exactly 2 levels deep (one level for
`match arm`, one level for `if/else`). This is within the limit.

### 5. No `todo!()` / `unreachable!()`

Every path is handled:
- Integer: `add i64 / sub i64 / mul i64 / sub i64 0` — exhaustive
- Float: `fadd/fsub/fmul/fsub -0.0` — covers `float()` and `float64()`
- Bool: `i8` — handled by `lower_type("Bool") → "i8"`
- Store/return: `lower_type()` covers all known types; `_ => "i64"` in
  `lower_type` catches anything unexpected

## Files Changed

| File | Lines Changed | What |
|------|---------------|------|
| `src/backend/llvm/emit_expr.rs` | 269, 273, 277, 364, 256-257 | Fix #1, #2, #5 |
| `src/backend/llvm/emit_stmt.rs` | 34, 39, 43, 52, 60 | Fix #3, #4 |
| `docs/plans/2026-07-14-fix-backend-type-bugs.md` | all | This file |
| `BUGS.md` | update | Move the `fadd i64` bug entry to fixed, cross-ref this plan |

## Verification

### Step 1: Unit tests pass

```bash
cargo test --lib
```

All 796 existing tests must pass. No test is removed or modified; only
instruction-selection logic changes.

### Step 2: Build clean

```bash
cargo build --release 2>&1 | grep -E 'warning|error'
```

Must produce zero new warnings. The only pre-existing warning is the
`unused variable: file_path` in `src/library.rs:57`.

### Step 3: IR correctness — manual smoke test

Create a test file that exercises integer arithmetic, boolean assign, and
return:

```bash
cat > /tmp/type_test.bv << 'EOF'
defn main() -> Int {
    let x: Int = 1 + 2;
    let y: Bool = true;
    term x;
};
EOF
./target/release/briev-compiler build /tmp/type_test.bv 2>&1
```

Then inspect the `.ll` file:
```bash
rg 'fadd|fsub|fmul|store|ret' /tmp/type_test.ll
```

Expected:
- `x = add i64 1, 2` (not `fadd i64`)
- `store i8 1, ptr %...` (not `store i64`)
- `ret i64 %...` (matches the return type)

### Step 4: Benchmark regression check

Run correctness-check-only on a few representative benchmarks:

```bash
bash benchmarks/build_and_bench.sh --correctness
```

All benchmarks that previously failed with `fadd i64` must now pass
correctness. Document any new failures as separate bugs.

### Step 5: Full benchmark suite

Run the full runtime benchmark suite to confirm no regression in
benchmarks that already worked (e.g. `nbody_newton` which uses float
arithmetic and was not affected by the `fadd i64` bug):

```bash
bash benchmarks/build_and_bench.sh --runtime
```

## Risk Analysis

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| `fsub double -0.0, %x` changes sign of NaN payloads | Very low | IEEE 754 requires `-0.0` for `fsub`. Any NaN-payload-dependent code is already non-portable. |
| `lower_type(&val.ty)` produces unexpected type for some | Low | `lower_type` has `_ => "i64"` fallback; unknown types stay 64-bit. No regression possible. |
| `defn_return_types.get(name)` returns wrong type for overloaded defns | Very low | Briev does not currently support function overloading. When it does, the key will include type parameters. |
| `call {ret} @fn(...)` ABI mismatch if function was declared with different return type in LLVM `declare` | Medium | The `defn_return_types` is populated from the same AST that generates LLVM declarations. They are always in sync. |
| Bool variable `store i8` vs `store i64` changes LLVM's DSE behaviour | Low to medium | `i8` stores are narrower; LLVM's DSE handles them correctly (they may even enable better alias analysis). If a performance regression appears, we isolate it. |

## Fallback

If any fix causes a regression in a benchmark that previously passed, the fix
for that specific site can be scoped to a `#[cfg(feature = "fix_type_bugs")]`
flag. However, all five fixes are strict type-correctness improvements and
should not regress correct code — only code that was already emitting invalid
IR benefits.

## Commit Strategy

Do all five fixes in **one commit** (they are interdependent — fixing stores
without fixing returns would leave half the codegen wrong). Use commit message:

```
Fix five backend type-bug regressions from Phase 7 refactoring

- emit_binary_op Add/Sub/Mul: branch on is_float (was always fadd/fsub/fmul)
- emit_unary_op Neg: use fsub for float operands (was always sub i64)
- emit_stmt stores: use lower_type(&val.ty) (was hardcoded i64)
- emit_stmt returns: use lower_type(&reg.ty) (was hardcoded i64)
- emit_user_call: look up real return type from defn_return_types

All fixes are additive guard clauses — existing match arms unchanged.
Div was already correct (lines 281-285); this applies the same pattern
to the other arithmetic arms.

Fixes the fadd i64 bug documented in BUGS.md (2026-07-14 entry).
```
