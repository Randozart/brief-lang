# Narrowing by Proof — Per-Chain Int Width
## 2026-07-25

### Goal

When the compiler proves a specific code path (an `Int` chain) never exceeds
N bits, emit i8/i16/i32/i64 accordingly. On WASM, ≤32 bits → no BigInt.

### Design

**`bytes` is removed.** Replaced by `minbits`/`maxbits`. `alignment` stays.

**The narrowing pass** runs after typechecking. It already computes
per-binding ranges and populates `self.fun.narrowed`. No changes to the
pass itself.

**`llvm_type()`** in `emit_toplevel.rs:297` — add check for narrowed width
BEFORE the universe lookup:

```rust
if let Type::Custom(name) = ty {
    if name == "Int" || name == "UInt" {
        if let Some(&bits) = self.fun.narrowed.get("ret") {
            if bits <= 8  { return "i8".into(); }
            if bits <= 16 { return "i16".into(); }
            if bits <= 32 { return "i32".into(); }
        }
    }
}
// fall through: existing universe → "i64"
```

**Pipeline:** narrowing runs after typechecking (already at compile.rs:309).
No reordering needed.

**Bootstrap.bv:** Remove `bytes <~ N`. Replace with `minbits`/`maxbits`.

**No normalizer changes.** `llvm_type()` checks narrowed BEFORE the universe,
so the narrower width wins at emit time regardless of what the normalizer stamped.

**No backend changes** to binary ops, struct layout, or parameter marshalling.
Everything flows through `llvm_type()`.

### Effect

| Input | Before | After | WASM BigInt? |
|-------|--------|-------|--------------|
| `add(a,b) [a<1000][b<1000]` | i64 | i16 | No |
| `answer() { term 42; }` | i64 | i8 | No |
| `add(a,b)` (no contract) | i64 | i64 | Yes |
| `let x = 255;` | i64 | i8 | No |
| `let x = 65535;` | i64 | i16 | No |

### Files Changed

| File | Change |
|------|--------|
| `src/backend/llvm/emit_toplevel.rs` | `llvm_type()`: add narrowed check before universe (~7 lines) |
| `lib/std/types/bootstrap.bv` | Remove `bytes <~ N`, replace with `minbits`/`maxbits` |
| `lib/std/*.bv` | Remove `bytes` references |
