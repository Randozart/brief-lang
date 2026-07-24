# Per-Binding Int Width Narrowing — Protocol + Size Type Derivation
## 2026-07-24

## Problem

The narrowing pass computes value ranges per binding, but its results are
ignored. The LLVM backend hardcodes `"Int" → "i64"` in `types.rs` regardless
of whether the actual values fit in 32 bits.

On WASM, `i64` forces JavaScript BigInt (~50ns overhead). If the compiler
could prove a value fits in 32 bits and emit `i32`, WASM calls would use
plain Numbers — matching native JS performance (~70ns).

## Architecture

### Data Flow

```
Narrowing pass                      CompilerContext                    LLVM codegen
━━━━━━━━━━━━━━━                     ━━━━━━━━━━━━━━                    ━━━━━━━━━━━━━
                                    narrow_bindings:
compute ranges per binding ───→       "add" → {            ───→     FunctionContext["add"]
  "add.ret" → [0, 255]                  "ret" → 8,                       narrowed:
  "add.x"   → unknown                   "param_0" → 64,                    "ret" → 8
  "add.b"   → unknown                   "let_z" → 16                      "let_z" → 16
  "add.let_z" → [0, 100]              }                                 }

                                    emit returns:
                                      derive_llvm_type("#Int", 8) → "i8"
```

### Type Derivation

Replace hardcoded `"Int" → "i64"` with:

```rust
fn derive_llvm_type(protocol: &str, max_bits: u64) -> String {
    match protocol {
        "#Int" | "#UInt" => format!("i{}", max_bits),
        "#Float" => {
            if max_bits <= 32 { "float".into() }
            else { "double".into() }
        }
        "#Bool" => "i8".into(),
        _ => format!("i{}", max_bits.max(8)),
    }
}
```

`type_size` follows the same pattern — `max_bits.div_ceil(8)`.

### Scope

**Phase 1 (this plan):** Narrow function returns and let-bindings only.
Parameters stay at their declared width unless proven by range checks.

**Phase 2 (future):** Inter-procedural narrowing — caller-proven ranges
propagated to callee parameters.

## Changes

### 1. `CompilerContext` — new field

```rust
pub struct CompilerContext {
    // ... existing fields ...
    /// 2026-07-24: Per-function binding width narrowing.
    /// Key: function_name, Value: map of binding → max_bits.
    /// "ret" = return value, "param_0" = first param, "let_x" = let binding.
    pub narrow_bindings: HashMap<String, HashMap<String, u64>>,
}
```

### 2. `FunctionContext` — new field

```rust
pub struct FunctionContext {
    // ... existing fields ...
    /// 2026-07-24: Narrowed widths for this function's bindings.
    /// Populated from CompilerContext.narrow_bindings[fn_name].
    pub narrowed: HashMap<String, u64>,
}
```

### 3. Narrowing pass — populate map instead of universe

```rust
pub fn narrow_types(items: &mut [TopLevel]) -> HashMap<String, HashMap<String, u64>> {
    let mut bindings = HashMap::new();
    for item in items {
        match item {
            TopLevel::Definition(d) => { bindings.insert(d.name.clone(), narrow_defn(d)); }
            TopLevel::Transaction(t) => { bindings.insert(t.name.clone(), narrow_txn(t)); }
            _ => {}
        }
    }
    bindings
}
```

`narrow_defn` returns a `HashMap<String, u64>` with `"ret"` → bit_width
and `"let_<name>"` → bit_width for each narrowed binding.

### 4. LLVM backend — read from map

```rust
// In rt_llvm_type fallback or derive_llvm_type:
fn get_effective_max_bits(ty: &Type, scope: &FunctionContext, ctx: &CompilerContext) -> u64 {
    match ty {
        Type::Custom(name) if name == "Int" || name == "UInt" => {
            let fn_narrowed = ctx.narrow_bindings.get(&scope.fn_name);
            let binding_narrowed = fn_narrowed.and_then(|m| m.get("ret"));
            binding_narrowed.copied().unwrap_or(64)
        }
        _ => type_bits(ty), // existing logic
    }
}
```

### 5. Pipeline integration

```rust
// compile.rs — after Normalized stage:
let narrow_bindings = brief_compiler::optimizer::narrow_int::narrow_types(&mut items, &mut universe);
ctx.narrow_bindings = narrow_bindings;
```

## Implementation Order

| Step | File | What |
|------|------|------|
| 1 | `src/backend/llvm/context.rs` | Add `narrow_bindings` to CompilerContext, `narrowed` to FunctionContext |
| 2 | `src/optimizer/narrow_int.rs` | Return `HashMap<String, HashMap<String, u64>>` instead of mutating universe |
| 3 | `src/compile.rs` | Capture return value, assign to `ctx.narrow_bindings` |
| 4 | `src/backend/llvm/helpers.rs` | `rt_llvm_type` → `derive_llvm_type`, read from narrowed map |
| 5 | `src/backend/llvm/types.rs` | `type_size` → `derive_size`, read from narrowed map |
| 6 | Test | Build, run tests, benchmark WASM |

## Effect on Benchmarks

| Function | Before | After | Mechanism |
|----------|--------|-------|-----------|
| `add(a, b)` — params unknown | i64 | i64 | No change (params are unknown) |
| `let x = 42; term x;` | i64 | i8 | Literal ≤ 8 bits |
| `let x = 300; term x;` | i64 | i16 | Range [0,300] fits in 16 bits |
| `when x < 100 { ... }` — range-checked | i64 | i8 | Guard narrows param to 8 bits |
| WASM benchmark (120ns) | i64 | i32 only if range-proven | ~70ns if narrowed |
