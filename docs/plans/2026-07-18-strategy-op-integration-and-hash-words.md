# Strategy Op Integration + Hash-Prefixed Compiler Words

## Overview

Integrate `InsertAt` / `ExtractFrom` strategy dispatch into the OP_CONFIG
template system using `#L`, `#R`, `#T` as positional marker tokens.

---

## Phase 1: Parser — Accept `#L`, `#R`, `#T` Tokens

**Files:** `src/lexer.rs`, `src/parser/definitions.rs`

### 1a. Lexer tokens

Add three new tokens to the lexer:

```rust
// src/lexer.rs
#[token("#L")] HashL,
#[token("#R")] HashR,
#[token("#T")] HashT,
```

### 1b. Property value parsing

Currently `parse_bracket_decl_body` at `definitions.rs:425` expects an identifier
after `<~`. Extend to also accept `HashL`, `HashR`, `HashT` as property values:

```rust
Some(Token::Identifier(id)) => { /* existing */ }
Some(Token::HashL) => { lhs.push(("insert".to_string(), PropertyValue::HashL)); }
Some(Token::HashR) => { lhs.push(("insert".to_string(), PropertyValue::HashR)); }
```

### 1c. PropertyValue enum

Add `HashL`, `HashR`, `HashT` variants to `PropertyValue` in `src/ast/top.rs`.

---

## Phase 2: Property Resolution + Substitution

**Files:** `src/backend/llvm/emit_stmt.rs`, `src/backend/llvm/emit_toplevel.rs`

### 2a. Strategy dispatch

Replace the hardcoded `match strat.as_deref() { Some("ring_push") => ... }` with:

```rust
// Look up the strategy from the type property
let strat = match backend.check_insert_strategy(target) {
    Some(PropertyValue::HashL) => /* #L = collection handle */,
    Some(PropertyValue::HashR) => /* #R = value */,
    None => { /* regular store */ },
};
```

### 2b. Substitution logic

For `InsertAt <~ ring_push(#L, #R)`:
- `#L` → `emit_addr_of(target)` → handle register
- `#R` → the value register (already computed)

For `ExtractFrom <~ ring_pop(#R)`:
- `#R` → `emit_addr_of(source)` → handle register

For `as #T`:
- Resolves to the concrete element type from the collection's type parameter
- Used in generic strategies that need element size/width

---

## Phase 3: Deprecate `emit_ring_push`/`emit_ring_pop` + Remove Intrinsics

**Files:** `src/backend/llvm/emit_stmt.rs`, `src/backend/llvm/intrinsics.rs`

### 3a. Remove inline GEP functions

Delete `emit_ring_push` and `emit_ring_pop` from `emit_stmt.rs`. The strategy
dispatch now calls the Briv function definition directly.

### 3b. Remove RingPush/RingPop intrinsics

Delete `Intrinsic::RingPush` and `Intrinsic::RingPop` from `intrinsics.rs`.
These are no longer needed — the strategy dispatch emits `call @ring_push(...)`
which LLVM -O3 inlines.

---

## Phase 4: Documentation

### 4a. Architecture note — Hash-prefixed compiler words (`#words`)

New file: `docs/architecture/hash-words.md`

Document that `#` prefix denotes compiler-internal words:
- `#L` — left operand
- `#R` — right operand  
- `#T` — type parameter
- Future: `#Self` reserved

### 4b. Update strategy docs

Update `docs/architecture/arrow-syntax-and-arena.md` with the new op-based
strategy dispatch.

---

## Files Changed

| File | Change |
|------|--------|
| `src/lexer.rs` | Add `HashL`, `HashR`, `HashT` tokens |
| `src/ast/top.rs` | Add `PropertyValue::HashL/R/T` variants |
| `src/parser/definitions.rs` | Parse `#L`/`#R`/`#T` in property values |
| `src/backend/llvm/emit_stmt.rs` | Strategy dispatch via property values |
| `src/backend/llvm/emit_toplevel.rs` | `check_insert_strategy` returns PropertyValue |
| `src/backend/llvm/intrinsics.rs` | Remove RingPush/RingPop |
| `docs/architecture/hash-words.md` | NEW — hash-prefixed compiler words |
| `docs/architecture/arrow-syntax-and-arena.md` | Update for op-based dispatch |

---

## Phase 5: Expose All Operators as `#` Intrinsics

Every non-short-circuit operator sugar desugars to a callable `#` intrinsic.
The codegen for `Expr::Call("Add#", ...)` goes through the same `OP_CONFIG`
template lookup as the sugar `+` goes through `emit_binary_op`.

### 5a. Intrinsic signatures — add to `get_intrinsic_signature()`

| Intrinsic | Parameters | Return | Notes |
|-----------|-----------|--------|-------|
| `BitAnd#` | `(a, b)` | Inferred | |
| `BitOr#` | `(a, b)` | Inferred | |
| `BitXor#` | `(a, b)` | Inferred | |
| `Shl#` | `(a, b)` | Inferred | |
| `Shr#` | `(a, b)` | Inferred | Arithmetic shift right (signed) |
| `BitNot#` | `(a)` | Inferred | |
| `Not#` | `(a)` | Inferred | Logical not — unary, no short-circuit |
| `Deref#` | `(ptr)` | Inferred | Load through pointer |
| `Index#` | `(obj, idx)` | Inferred | Get element at index |
| `Cast#` | `(val)` | Inferred | Type reinterpretation — target type inferred from context |
| `Ptr#` | `(val)` | Inferred | inttoptr — pointee type inferred from context |

Each signature entry follows the existing pattern:
```rust
"BitAnd#" => Some(Signature {
    name: "BitAnd#",
    parameters: vec![],
    return_kind: ReturnKind::Inferred,
    observable: false,
}),
```

### 5b. Config templates — add to `config/llvm-ops.toml`

| Op | Primitive | Width | Template |
|----|-----------|-------|----------|
| `[op.BitAnd]` | `.Int` | 8 | `and i64 %a, %b` |
| `[op.BitOr]` | `.Int` | 8 | `or i64 %a, %b` |
| `[op.BitXor]` | `.Int` | 8 | `xor i64 %a, %b` |
| `[op.Shl]` | `.Int` | 8 | `shl i64 %a, %b` |
| `[op.Shr]` | `.Int` | 8 | `ashr i64 %a, %b` |
| `[op.BitNot]` | `.Int` | 8 | `xor i64 -1, %a` |
| `[op.Not]` | `.Bool` | 1 | `xor i8 1, %a` |

### 5c. Interpreter — add `execute_intrinsic` arms

```rust
"BitAnd#" => exec_binop(args, |a,b| a & b, |a,b| a & b),
"BitOr#" => exec_binop(args, |a,b| a | b, |a,b| a | b),
"BitXor#" => exec_binop(args, |a,b| a ^ b, |a,b| a ^ b),
"Shl#" => exec_binop(args, |a,b| a.wrapping_shl(b as u32), ...),
"Shr#" => exec_binop(args, |a,b| a.wrapping_shr(b as u32), ...),
"BitNot#" => |a| !a,
"Not#" => |a| if a != 0 { 0 } else { 1 },
```

### 5d. LLVM codegen — type-dependent intrinsics as special cases

`Deref#`, `Index#`, `Cast#`, and `Ptr#` cannot use OP_CONFIG templates
because their LLVM IR depends on the **pointee element type** or the
**target type** (the template system only knows `%a`/`%b` registers, not
types). Handle them as special cases in `emit_intrinsic_call`:

```rust
"Deref#" => {
    let ptr_reg = emit_expr(args[0]);
    let inner_ty = pointee_type(&ptr_reg.ty).unwrap_or(Type::int());
    let llvm_ty = lower_type(&inner_ty);
    writeln!("{} = load {}, ptr {}", v, llvm_ty, ptr_reg.name);
    TypedRegister { name: v, ty: inner_ty }
}
"Index#" => {
    let obj_reg = emit_expr(args[0]);
    let idx_reg = emit_expr(args[1]);
    // Same GEP+load logic as Expr::Index handler
}
"Cast#" => {
    let src = emit_expr(args[0]);
    // Same logic as Expr::Cast handler — reads target type from
    // expected return type (Inferred), dispatches on (source, target).
    // Ptr<T> → inttoptr, String/Data → runtime helpers, etc.
}
"Ptr#" => {
    let src = emit_expr(args[0]);
    // inttoptr — pointee type inferred from return type context.
    // Equivalent to (Ptr<T>)i64_val via Cast# but always inttoptr.
    writeln!("{} = inttoptr i64 {} to ptr", v, src.name);
    TypedRegister { name: v, ty: inferred_return_type }
}
```

### 5e. Config template enrichment — `%t` type placeholder

The current template system supports `%v` (result), `%a`/`%b`/`%c` (args).
Add `%t` to resolve the **pointee type** (for `Deref#`, `Index#`) or the
**target type** (for `Cast#`, `Ptr#`) to an LLVM type string:

| Template | Placeholder | Resolves to |
|----------|-------------|-------------|
| `Deref#(ptr: Ptr<Float>)` | `%t` | `lower_type(pointee(ptr.ty))` = `"float"` |
| `Index#(arr: Ptr<Int>, idx)` | `%t` | `lower_type(pointee(arr.ty))` = `"i64"` |
| `Cast#(val) → Float64` | `%t` | `lower_type(inferred_return_ty)` = `"double"` |
| `Ptr#(val) → Ptr<Int>` | `%t` | (ignored — ptr type is always `ptr`) |

**Substitution logic** (in `emit_intrinsic_call`, after `%v`/`%a`/`%b`/`%c`):

```rust
let t_ty = if matches!(arg_regs[0].ty, Type::Ptr(_)) {
    // Deref# or Index# — pointee type
    pointee_type(&arg_regs[0].ty).map(|t| lower_type(&t))
        .unwrap_or_else(|| "i64".to_string())
} else {
    // Cast# — target type from return type context
    lower_type(&inferred_return_ty)
};
let ir = ir.replace("%t", &t_ty);
```

This lets the config define templates like:

```toml
[op.Deref]
.Int.8 = "load i64, ptr %a"
.Float.4 = "load float, ptr %a"
.Float64.8 = "load double, ptr %a"

[op.Index]
.Int.8 = "getelementptr i64, ptr %a, i64 %b\n\t%v = load i64, ptr %gep"
.Float.4 = "getelementptr float, ptr %a, i64 %b\n\t%v = load float, ptr %gep"
```

The `%t` substitution is applied AFTER the standard `%v`/`%a`/`%b`/`%c`
substitutions, so templates can use any combination.

### 5f. Excluded — short-circuit operators

| Operator | Intrinsic | Reason for exclusion |
|----------|-----------|---------------------|
| `&&` | `And#` | Short-circuit: RHS not evaluated if LHS is false |
| `\|\|` | `Or#` | Short-circuit: RHS not evaluated if LHS is true |

As pure functions, `And#(a, b)` would eagerly evaluate both arguments,
breaking the expected control flow. Keep these as syntax-only.

### Files Changed — Phase 5

| File | Change |
|------|--------|
| `src/intrinsic_signatures.rs` | Add `BitAnd#`, `BitOr#`, `BitXor#`, `Shl#`, `Shr#`, `BitNot#`, `Not#`, `Deref#`, `Index#` |
| `config/llvm-ops.toml` | Add `[op.BitAnd]` through `[op.Not]` template entries |
| `src/interpreter/intrinsics.rs` | Add `execute_intrinsic` arms for each new intrinsic |
| `src/backend/llvm/intrinsics.rs` | Add `Deref#` and `Index#` special case handlers in `emit_intrinsic_call` |<｜end▁of▁thinking｜>

