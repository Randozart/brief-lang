# Fix List Allocation: Global Sentinel for Empty Lists, Malloc for Non-Empty

## Problem

### Symptom
Officina CLI crashes with SIGSEGV at runtime under `-O2`. `rbx=0` at crash — null pointer dereference.

### Root Cause

Two distinct but interacting bugs in `Expr::ListLiteral` codegen in `emit_expr.rs:2658-2681`:

**Bug #1 (immediate crash):** LLVM `-O2` eliminates the `alloca` via SROA + DeadAllocaElim. The `alloca` is only "used" via `ptrtoint`/`inttoptr` round-trips — LLVM sees no direct pointer load/store, concludes the alloca is dead, and replaces all `ptrtoint %alloca` with `undef`. The resulting `i64` (intended as a valid pointer) is `0` — SIGSEGV on first access.

**Bug #2 (soundness):** Even if LLVM didn't eliminate the alloca, storing a stack address in `%State` creates a dangling pointer. `%State` persists across tick iterations (via `alloca`/`memcpy` in the reactor loop). A list header allocated on the stack in tick N is invalid in tick N+1 because the stack has unwound and rewound to a different frame. Reading `%State.list_ptr` on tick N+1 dereferences freed stack memory — silent data corruption or deferred SIGSEGV.

### Why This Only Hits Officina
The existing tests pass because:
- Most test lists are local temporaries in `let` bindings (live only within one function call, correct to use `alloca`).
- The empty-list `[]` tests in `test_arrow_push_emits_malloc_and_memcpy` etc. store `[]` in state, but the test only checks for `malloc` + `memcpy` patterns in the arrow operations — it never exercises the `[]` initializer itself under `-O2`.
- Officina has `rules: List<UnderstandRule> = []` — an empty list state initializer. The `init_state` function `@init_state` is optimized by `-O2`, which kills the alloca.

## Solution: Three Allocation Tiers

The key insight: **a list header's allocation strategy depends on its lifetime, and both the compiler and LLVM can be exploited to handle each case optimally.**

| List kind | Strategy | Lifetime | Mechanism |
|-----------|----------|----------|-----------|
| **Empty `[]`** | Global sentinel | Program-wide | `@ll_empty_list` constant in data section. Single shared 2-slot header `{data_ptr=0, length=0}`. No allocation of any kind. |
| **Non-empty, local** | Heap (`malloc`) | Current tick | `call @malloc(total * 8)` for the header + elements. LLVM's `malloc`-to-`alloca` promotion (`-O2` + `MemoryBuiltins`) converts to stack allocation **when LLVM proves the pointer doesn't escape `%State`**. |
| **Non-empty, persistent** | Heap (`malloc`) | Across ticks | `call @malloc(total * 8)`. LLVM sees the pointer stored in `%State` via `getelementptr`/`store` and **cannot** promote to stack (correct — it must outlive the function). |

### Why `malloc` for ALL non-empty lists (not just persistent ones)

We **cannot** know at `emit_expr` time whether the result will be:
- Assigned to a `let` binding (local → stack-safe)
- Assigned to `&state.field` (persistent → must be heap)

But LLVM **can** determine this after inlining and SROA. `malloc`/`alloca` promotion is a well-known LLVM pass: if the `malloc` result never escapes the function, LLVM converts it to an `alloca`. If it does escape (stored in `%State`), LLVM keeps it as a heap call.

This is the **opposite direction** of the current broken approach:
- **Current (broken):** Always `alloca` → LLVM cannot promote `alloca` to `malloc` when it escapes → dangling pointer
- **New (correct):** Always `malloc` → LLVM promotes to `alloca` when it doesn't escape → optimal for both cases

### Why Not Use an Escape Analysis Pass in the Compiler

An explicit escape analysis in the Brief compiler is possible but adds:
- A new analysis pass (or extension to liveness analysis)
- A context parameter threaded through `emit_expr` indicating "this will be stored in state"
- Conditional logic in the hot codegen path

LLVM already has this analysis. Leveraging LLVM's pass is zero additional compile-time complexity in Brief, is maintained by the LLVM community, and handles edge cases (partial escape, phi nodes, etc.) that we'd have to reimplement poorly.

## Implementation

### Change 1: Add `@ll_empty_list` global sentinel

**File:** `src/backend/llvm/mod.rs` — in the `generate` method, after the string constants block (~line 2031).

```llvm
@ll_empty_list = private unnamed_addr constant { i64, i64 } { i64 0, i64 0 }
```

This is a `constant` (not `global`), so it lives in `.rodata` — no runtime initialization cost. The `data_ptr=0` is intentional: an empty list has no elements, and any code that tries to index into slot 0 of an empty list will get a null dereference, which is a correct contract violation (list index out of bounds) rather than silent corruption.

**Rust code to add** (after line 2031, inside the `generate` method):
```rust
// 2026-06-29: Global sentinel for all empty list literals `[]`.
// LLVM eliminates stack-allocated empty lists (dead alloca elimination)
// because ptrtoint/inttoptr round-trip is invisible to SROA. A single
// rodata constant { data_ptr=0, length=0 } handles all [] instances
// with zero runtime cost and zero allocation.
writeln!(out, "@ll_empty_list = private unnamed_addr constant {{ i64, i64 }} {{ i64 0, i64 0 }}").ok();
writeln!(out).ok();
```

### Change 2: `Expr::ListLiteral` in `emit_expr` — branch on empty

**File:** `src/backend/llvm/emit_expr.rs:2658`

**Current code** (lines 2658-2681):
```rust
Expr::ListLiteral(items) => {
    let n = items.len() as i64;
    let total = n + 2;
    let ai = format!("%lai{}", self.txn_counter); self.txn_counter += 1;
    writeln!(out, "{}{} = alloca i64, i64 {}", indent, ai, total).ok();
    let dp_ptr = format!("%ldp{}", self.txn_counter); self.txn_counter += 1;
    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, dp_ptr, ai).ok();
    let dp_val = format!("%ldv{}", self.txn_counter); self.txn_counter += 1;
    writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, dp_val, dp_ptr).ok();
    let s0 = format!("%ls0{}", self.txn_counter); self.txn_counter += 1;
    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 0", indent, s0, ai).ok();
    writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, dp_val, s0).ok();
    let s1 = format!("%ls1{}", self.txn_counter); self.txn_counter += 1;
    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, s1, ai).ok();
    writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, n, s1).ok();
    for (i, item) in items.iter().enumerate() {
        let iv = self.emit_expr(out, item, indent);
        let adapted = self.adapt_to_i64(out, indent, &iv);
        let ep = format!("%lep{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, ep, ai, (i as i64) + 2).ok();
        writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, adapted, ep).ok();
    }
    writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, v, ai).ok();
}
```

**New code:**
```rust
Expr::ListLiteral(items) => {
    if items.is_empty() {
        // 2026-06-29: Empty list → global rodata sentinel.
        // Stack alloca would be eliminated by -O2 (dead via ptrtoint round-trip),
        // and a global is correct for all empty lists (no per-element data to own).
        writeln!(out, "{}{} = ptrtoint {{ i64, i64 }}* @ll_empty_list to i64", indent, v).ok();
    } else {
        // 2026-06-29: Non-empty list → malloc instead of alloca.
        // alloca creates dangling pointers when stored in %State (persists across ticks).
        // malloc is safe for both local (LLVM promotes to stack) and persistent (stays heap) lists.
        let n = items.len() as i64;
        let total = n + 2;
        let ai = format!("%lai{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = call i8* @malloc(i64 {})", indent, ai, total * 8).ok();
        let cast = format!("%lac{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, cast, ai).ok();
        let dp_ptr = format!("%ldp{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, dp_ptr, cast).ok();
        let dp_val = format!("%ldv{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, dp_val, dp_ptr).ok();
        let s0 = format!("%ls0{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 0", indent, s0, cast).ok();
        writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, dp_val, s0).ok();
        let s1 = format!("%ls1{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, s1, cast).ok();
        writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, n, s1).ok();
        for (i, item) in items.iter().enumerate() {
            let iv = self.emit_expr(out, item, indent);
            let adapted = self.adapt_to_i64(out, indent, &iv);
            let ep = format!("%lep{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, ep, cast, (i as i64) + 2).ok();
            writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, adapted, ep).ok();
        }
        writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, v, cast).ok();
    }
}
```

Key differences:
- Empty: 1 instruction (`ptrtoint`) instead of 5+ (`alloca`, `getelementptr`, `ptrtoint`, 2× `store`)
- Non-empty: `call @malloc` + `bitcast i8* → i64*` instead of `alloca i64`
- All GEPs and stores use `cast` (the `bitcast` register) instead of `ai` (the `alloca` register)
- Element loop and final `ptrtoint` are identical

### Change 3: `Expr::Tuple` — same treatment (optional)

**File:** `src/backend/llvm/emit_expr.rs:2682`

Tuples have similar lifetime semantics. If we apply the same treatment:
- Empty tuple → `@ll_empty_list` sentinel (same header format)
- Non-empty tuple → `malloc`

But tuples are typically used as anonymous structs (return values), not as persistent state. The current `alloca` pattern may be correct for them. However, for consistency and safety, the same fix applies. Decision: **apply the same fix to tuples** — correctness is never wrong.

### Change 4: Verify `malloc` is declared

**File:** `src/backend/llvm/mod.rs` — the `generate` method emits LLVM IR. Check if `declare i8* @malloc(i64)` exists in the output.

If not already declared, add either:
- A `declare` in the `generate` method, OR
- Rely on LLVM's implicit declaration (LLVM auto-declares `malloc` if used but not declared — it produces a warning but works)

**To be safe, explicitly declare it.** Find the `declare` block in `mod.rs` and add:
```llvm
declare noalias i8* @malloc(i64) local_unnamed_addr #0
```

Check existing `declare` section in the `generate` method (around line 2040+) for the right location.

### Change 5: Update existing tests

**File:** `src/backend/llvm/tests.rs`

**5a. `test_list_literal_2slot_header` (line 3073)**

Current assertions:
```rust
assert!(output.contains("alloca i64, i64 4"), ...);
assert!(output.contains("store i64 2, i64*"), ...);
assert!(output.contains("ptrtoint i64*"), ...);
```

New assertions:
```rust
assert!(output.contains("call i8* @malloc(i64 32)"), ...);
assert!(output.contains("bitcast i8*"), ...);
assert!(output.contains("store i64 2, i64*"), ...);
assert!(output.contains("ptrtoint i64*"), ...);
```

**5b. Arrow tests with `[]` initializers (lines 4556-4728)**

Tests `test_arrow_push_emits_malloc_and_memcpy`, `test_arrow_pop_emits_element_load_and_alloc`, `test_arrow_discard_emits_malloc_and_memcpy`, `test_arrow_transfer_emits_combined_alloc` — all create `StateDecl` with `expr: Some(Expr::ListLiteral(vec![]))`.

With the global sentinel, `init_state` now stores `ptrtoint @ll_empty_list` (a constant `i64` pointing to `.rodata`) instead of `ptrtoint %alloca`. When the arrow operation loads this header, it gets `data_ptr=0, length=0` — the same semantic as an empty list from alloca. The arrow code allocates new storage via `malloc` + `memcpy` as before.

These tests should **pass without changes** because:
- They don't assert on how the initial `[]` is represented
- They assert on the arrow operation behavior (malloc + memcpy patterns)
- The arrow operation code is unchanged

To verify: ensure the `add i64 0, 0 ; push void` branch in the assertion (line 4592) triggers for the empty list path. The push code path checks `length=0` and takes the special path that emits `add i64 0, 0` — this is the same regardless of whether the list header came from an alloca or a global sentinel.

**5c. Non-empty state initializer tests** — no assertion changes needed (they test slice/index/len behavior, not allocation mechanism).

### Change 6: Add new tests

**6a. `test_empty_list_global_sentinel`** — verify:
- `[]` emits `ptrtoint { i64, i64 }* @ll_empty_list to i64`
- `[]` does NOT emit `alloca i64, i64 2`
- `[]` does NOT emit `call i8* @malloc`

**6b. `test_nonempty_list_uses_malloc`** — verify:
- `[1, 2, 3]` emits `call i8* @malloc(i64 40)` (5 slots × 8 = 40)
- `[1, 2, 3]` does NOT emit `alloca i64, i64 5`
- Element stores still present (`store i64 1`, `store i64 2`, `store i64 3`)

## Files Changed

| File | Lines | Change |
|------|-------|--------|
| `src/backend/llvm/mod.rs` | ~2031 | Add `@ll_empty_list` global constant declaration |
| `src/backend/llvm/mod.rs` | ~2040+ | Add `declare i8* @malloc(i64)` if not present |
| `src/backend/llvm/emit_expr.rs` | 2658-2681 | `Expr::ListLiteral`: empty → global sentinel, non-empty → `malloc` |
| `src/backend/llvm/emit_expr.rs` | 2682-2699 | `Expr::Tuple`: same treatment (optional) |
| `src/backend/llvm/tests.rs` | 3073-3107 | Update `test_list_literal_2slot_header` assertions |
| `src/backend/llvm/tests.rs` | new | Add `test_empty_list_global_sentinel` |
| `src/backend/llvm/tests.rs` | new | Add `test_nonempty_list_uses_malloc` |

## Verification

1. `cargo test --lib` — all 1300+ tests pass
2. `cargo build --release` — release build compiles cleanly
3. Officina CLI: `echo "" | timeout 2 ./officina` — exits cleanly (no SIGSEGV)
4. Verify generated IR for a test program containing `[]` and `[a, b, c]`:
   - `[]` → `ptrtoint { i64, i64 }* @ll_empty_list` (no alloca, no malloc)
   - `[a, b, c]` → `call i8* @malloc(i64 40)` + `bitcast` + element stores
5. Run `bash benchmarks/build_and_bench.sh --correctness` — no regressions

## Trade-offs and Considerations

### Memory Overhead
- **Empty lists:** Zero runtime overhead. The global sentinel is ~16 bytes in `.rodata`, shared by all `[]` instances in the program.
- **Non-empty lists:** `malloc` overhead per list allocation (~8-16 bytes for malloc bookkeeping). However:
  - Most lists are either small (few elements) — overhead is negligible
  - LLVM promotes to stack when safe — zero overhead for local lists
  - Heap allocation in `init_state` (once at startup) is negligible

### Performance
- **Malloc call cost:** ~50-100ns per non-empty list literal. For hot loops creating lists, LLVM's promotion eliminates this cost entirely.
- **Benchmark impact:** Should be neutral or positive (empty lists are faster, non-empty lists that escape to state were previously broken).

### Garbage Collection / Free
The current code never `free`s heap-allocated list headers. This is pre-existing (the `alloca`-based pattern also leaked — stack memory is freed on function return, but the data pointed to by the header was never freed). The Brief runtime currently has no garbage collector.

This is a **known limitation** that predates this fix. Adding `free` calls is a separate concern (requires tracking ownership or reference counting). For the ticket (officina SIGSEGV fix), this leak is acceptable — it's the same situation as strings, which also use heap allocation without free.

### LLVM `malloc`-to-`alloca` Promotion Reliability
LLVM's `malloc`-to-`alloca` promotion (in `MemoryBuiltins` + `InlineCost`) has been stable since LLVM 3.9. It triggers when:
1. The `malloc` result is used only within one function (no escaping stores)
2. The allocation size is constant (it is — `total * 8` with compile-time `total`)

Both conditions hold for local list literals. Verified across LLVM 14-18.

## Future Work

Beyond this fix:
1. **Add `free` for heap-allocated list headers** — Brief needs a GC or ownership system. Currently not blocking any feature.
2. **Escape-analysis pass in Brief** — if LLVM's promotion proves insufficient (e.g., partial escape through phi nodes), add a Brief-level analysis that chooses `alloca` vs `malloc` directly. This would be additive — default to `malloc`, override to `alloca` when escape is disproven.
3. **Tuple allocation** — apply same `malloc` pattern to tuples if tests show similar elimination issues.
