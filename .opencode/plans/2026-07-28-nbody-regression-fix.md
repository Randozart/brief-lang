# Nbody Regression Fix: Extract @main_hot for SROA Enablement

**Date:** 2026-07-28
**Problem:** nbody_newton emits per-field phi nodes that fragment LLVM's SLP view,
reducing the horizontal reduction from -283 (Era 5) to -4 (current).
**Fix:** Split @main into a hot loop function (@main_hot) with `#8 =
willreturn memory(argmem: readwrite)`, enabling SROA on %State. The outer @main
calls @main_hot and handles only convergence checks and FFI-containing guards.

## Implementation

### Overview

The `hoist_terminating_guard` function at `mod.rs:2645` already separates the
pure computation body from FFI-containing guard bodies. The natural split:

```
@main_hot(ptr noundef dereferenceable(N) %state)
    #8                                     ← willreturn memory(argmem: readwrite)
{
  loop: (emitted by emit_countable_body for body_stmts)
    ... pure Briev computation ...
    %count_updated = add %count, 1
    store %count_updated, %state.count
    %guard_fired = ...                     ← computed from body (mod check)
    ret i1 %guard_fired                    ← return whether guard fired
  exit:
    ret i1 false
}

@main()
    #9                                     ← memory(readwrite)
{
  entry:
    %state = alloca %State
    init_state(%state)
  loop:
    %guard = call @main_hot(%state)        ← 1 call/iteration (~1ns overhead)
    br i1 %guard, guard_body, check
  guard_body:                               ← FFI here, NOT in @main_hot
    call __print_float(...)
    br label check
  check:
    if converged → exit, else → loop
  exit:
    ret i32 0
}
```

### Changes

**File: `src/backend/llvm/mod.rs`** — around line 2730 (the single-node dispatch)

Add the split logic after the dispatch decision:

```rust
// 2026-07-28: Function splitting for allocation-free SROA.
// If all FFI is inside guards (has_unguarded_ffi = false) and the body
// is large enough to benefit (≥ 10 fields), extract the pure computation
// into @main_hot with #8 (willreturn argmem:readwrite) and keep only
// convergence + FFI guards in @main with #9 (memory(readwrite)).
// This enables SROA on %State within @main_hot, which lets LLVM's SLP
// vectorizer find the -283 horizontal reduction for nbody_newton.
if !self.ctx.has_unguarded_ffi && total_fields >= 10 {
    // Miti @main_hot with #8
    writeln!(out, "define void @main_hot(ptr noundef noalias nocapture align 8 %state) local_unnamed_addr #8 {{").ok();
    emit_countable_body(out, &body_stmts, write_set, ...);
    writeln!(out, "}}").ok();

    // Emit @main with #9, calling @main_hot
    writeln!(out, "define i32 @main() local_unnamed_addr #9 {{").ok();
    writeln!(out, "  %state = alloca %State, align 8").ok();
    // ... init, loop calling @main_hot, compute guard condition + converge check ...
    writeln!(out, "}}").ok();
} else {
    // Existing dispatch path (no split)
    = emit_countable_main(out, ...);
}
```

### Verification

```bash
# 1. Build
cargo build --release

# 2. Check @main_hot exists
grep "^define.*@main_hot" /tmp/nbody_newton.ll
# Expected: define void @main_hot(...) ... #8

# 3. Check SLP reduction
opt -O3 -pass-remarks=slp-vectorizer /tmp/nbody_newton.ll -o /dev/null 2>&1 | grep "horizontal reduction"
# Expected: -283 cost (was -4)

# 4. Benchmark
BOUND=5000000 /usr/bin/time -f "%e" /tmp/nbody_newton
# Expected: ~0.75-0.80s (was 1.14s, Era 5 was 0.77s)

# 5. Full benchmark suite
bash benchmarks/build_and_bench.sh --runtime
# Expected: nbody_newton improves, no others regress
```
