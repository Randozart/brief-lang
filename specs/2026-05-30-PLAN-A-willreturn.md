# Plan A: Fix `willreturn` on `main()` and `reactor_tick()`

> Created: 2026-05-30T14:15Z
> Status: Draft — ready for implementation
> Depends on: Nothing

## Problem

`main()` and `reactor_tick()` contain infinite loops (`br label %tick`), but are annotated with `attributes #0` which includes `willreturn`. The LLVM IR verifier accepts this, but LLVM's optimizer is permitted to assume the function will eventually return, which can lead to:

1. The infinite loop being optimized away entirely
2. Code after the loop being hoisted or speculated incorrectly
3. General miscompilation under `-O2`/`-O3`

## Root Cause

In `src/backend/llvm.rs`, every user-defined function uses `#0`:
```
attributes #0 = {
    mustprogress nofree norecurse nosync nounwind willreturn
    memory(argmem: readwrite)
}
```

`willreturn` is valid for `init_state`, transaction functions, definitions, `pre_*` checkers, and fused transactions — because those all `ret void`. But `main()` and `reactor_tick()` never return.

## Fix

### Step 1: Add `attributes #2` (no `willreturn`)

After the `attributes #1` definition (line 345 area), emit:

```rust
writeln!(out, "attributes #2 = {{ mustprogress nofree norecurse nosync nounwind memory(argmem: readwrite) }}").ok();
```

Same as `#0` but with `willreturn` removed.

### Step 2: Change `emit_main()` to use `#2`

File: `src/backend/llvm.rs`
Function: `emit_main()` (line 1326)

Change:
```rust
writeln!(out, "define i32 @main() local_unnamed_addr #0 {{").ok();
```
To:
```rust
writeln!(out, "define i32 @main() local_unnamed_addr #2 {{").ok();
```

### Step 3: Change `emit_reactor()` to use `#2`

File: `src/backend/llvm.rs`
Function: `emit_reactor()` (line 1124)

Change:
```rust
writeln!(out, "define void @reactor_tick() local_unnamed_addr #0 {{").ok();
```
To:
```rust
writeln!(out, "define void @reactor_tick() local_unnamed_addr #2 {{").ok();
```

### Step 4: Change `emit_parallel_reactor()` to use `#2`

File: `src/backend/llvm.rs`
Function: `emit_parallel_reactor()` — search for `define void @reactor_tick` within it.

Change `#0` to `#2` for `reactor_tick` in the parallel dispatch version.

### Verification

- Existing test `test_llvm_has_noalias` checks for `#0` — must still pass (only checks that `#0` exists, not which functions use it)
- New LLVM IR output must contain `attributes #2 = { ... }` (no `willreturn`)
- `main()` must use `#2`, not `#0`
- `reactor_tick()` must use `#2`, not `#0`
- All other functions (`init_state`, `@txn`, `pre_*`, etc.) must continue using `#0`

## Test Updates

**New test** in `src/backend/llvm.rs` tests module:

```rust
#[test]
fn test_main_and_reactor_use_non_willreturn_attr() {
    // Verify that main() and reactor_tick() use #2 (no willreturn)
    // while other functions still use #0
}
```

Assertions:
- `attributes #2 =` appears in output
- `define i32 @main() local_unnamed_addr #2` in output
- `define void @reactor_tick() local_unnamed_addr #2` in output
- `attributes #0` still present
- `define void @init_state() local_unnamed_addr #0` still present