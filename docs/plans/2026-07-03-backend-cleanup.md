# Backend Architecture Cleanup — Comprehensive Plan

## Motivation

Two immediate problems, one architectural opportunity:

**Problem 1: SROA blocked.** `emit_hoisted_post_loop_prints` calls
`pre_load_all_fields` which emits GEP+load from `%State` in the `done:`
block. Those 30+ GEP references keep `%State` alive across the function,
preventing SROA from decomposing the struct. Result: LLVM vectorizer can't
analyze individual float fields → 0 vector ops in Briev vs 233 in C.

**Problem 2: ~2000 inline `writeln!` calls.** The backend emits LLVM IR as
flat strings via `writeln!` throughout. This means:
- Manual `%t{N}` register counter arithmetic (collision-prone)
- Type strings duplicated as `"float"`, `"i64"` etc (error-prone)
- No IR validation until `opt -O3` runs
- `#![allow(unused_imports)]` on `use std::fmt::Write` in every file

**Opportunity: An LLVM IR builder already exists** at `builder.rs` (738 lines)
with typed methods for arith, memory, control flow, phis, and labels. It
handles register allocation via `gen_reg()` — the sole source of SSA names.
Currently used only in `expr/rest.rs` and the `Foreach` statement. Migrating
the loop emission code to use it would eliminate the largest concentration
of `writeln!` calls (loop_engine.rs alone has ~500).

## Guiding Principles

1. **Flat code, always.** Every function we touch, we flatten. Max depth 2.
   Extract helpers, use guard clauses, use `?` and early returns.
2. **Additive only.** New builder-based code paths coexist with old `writeln!`
   paths. Old paths removed only after the new path is proven correct
   (tests pass, benchmarks match).
3. **One concern per commit.** The phi register fix is separate from the builder
   migration. The builder migration is separate from the nesting cleanup.

---

## Phase 0: Fix SROA Blockage (the phi register fix)

**Duration:** 10 minutes
**Files:** `src/backend/llvm/loop_engine.rs`
**Risk:** Low (same pattern already used in body: block)

### Change

Replace `self.pre_load_all_fields(out, "%state")` in
`emit_hoisted_post_loop_prints` with populating `ssa_old_float_regs` and
`ssa_old_int_regs` from `self.fun.phi_field_regs`.

**Why this is correct:** The per-field phi registers at `loop_hdr` are
available in `done:` (loop_hdr dominates done in SSA). The latch reloads
modified fields from `%State` before the backedge, so phi registers hold
the final stored values. The existing comment saying "phi registers are
stale" is wrong — the latch runs before the phi takes its value.

**Effect:** No `%State` GEP references in the `done:` block. SROA sees
zero uses of `%State` after the latch → can decompose the struct into
scalar `float` phis → vectorizer can analyze individual field values.

### Nesting

`emit_hoisted_post_loop_prints` is already flat (depth 2: `if` guard +
`for` loop). No nesting cleanup needed.

### Verification

```bash
cargo test --lib
bash benchmarks/build_and_bench.sh --correctness
opt -O3 -pass-remarks-missed=sroa,loop-vectorize nbody_sqrt.ll
```

Expected: SROA remarks disappear, vectorizer may still be blocked by
"value used outside loop" but the SROA blockage is removed.

---

## Phase 1: Build LLVMBuilder Coverage (the programmatic IR migration)

**Duration:** Per file, incremental
**Files:** `loop_engine.rs` (primary), `emit_stmt.rs`, `emit_toplevel.rs`
**Risk:** Medium (new code paths, but additive)

### Strategy

Each function that emits IR gains a `&mut LLVMBuilder` parameter alongside
the existing `&mut String out`. Instructions are added to the builder via
typed methods (`emit_add`, `emit_phi`, `emit_gep`, etc.). At the end,
`builder.finish_into(out, indent)` flushes the IR text.

During migration, both approaches coexist. New builder code is surrounded
by conditions or feature flags. Old `writeln!` paths are the default until
the builder path passes all tests.

### Migration Order

| File | writeln! count | Builder methods needed | Complexity |
|------|----------------|----------------------|------------|
| `loop_engine.rs` | ~500 | add, sub, mul, icmp, phi, br, store, load, alloca | High — loop structure |
| `emit_stmt.rs` | ~300 | store, load, call, alloca, gep | Medium — many stmt types |
| `emit_toplevel.rs` | ~200 | store, load, call, alloca, gep | Medium — field init |
| `mod.rs` | ~200 | mostly function headers | Low — dispatch logic |

### File-by-file: loop_engine.rs

#### Functions to convert

| Function | writeln! count | Builder approach | Nesting depth now | Target depth |
|----------|---------------|-----------------|-------------------|-------------|
| `emit_hoisted_post_loop_prints` | ~5 | Builder phis → ssa_old | 2 | 2 |
| `emit_countable_main` | ~80 | Full builder for latch/phi/header | 4 | 2 |
| `emit_folded_memory_main` | ~60 | Builder for GEP+load+store | 5 | 2 |
| `emit_ssa_main` | ~200 | Builder for phi pipeline | 6 | 2 |
| `emit_folded_main` | ~120 | Builder for folded body | 4 | 2 |
| `pre_load_all_fields` | ~30 | Builder for GEP+load | 2 | 2 |

#### Flattening strategy for loop_engine.rs

**emit_ssa_main** (currently depth 6):
- Extract txn dispatch into `emit_ssa_txn_block` helper
- Extract phi setup into `emit_ssa_phdr_block` helper
- Extract latch block into `emit_ssa_latch_block` helper

**emit_countable_main** (currently depth 4):
- Extract latch block into `emit_countable_latch` helper
- Extract phi setup into `emit_countable_phdr` helper

**emit_folded_memory_main** (currently depth 5):
- Extract body emission into `emit_folded_body` helper
- Extract latch into `emit_folded_latch` helper

### Multi-file changes

The builder adds a `&mut LLVMBuilder` parameter to many functions.
A shared builder on `LlvmBackend` might simplify this, but that would
create ordering dependencies (all code in one function must use the same
builder). Instead, pass the builder explicitly where needed, create new
ones for distinct regions.

### Verification

```bash
cargo test --lib  # after each function conversion
```

During migration, both old and new paths exist. A feature flag or
environment variable selects the path. Once the builder path matches
exactly (same register names, same IR structure), the old path is removed.

---

## Phase 2: Nesting Cleanup (flatten existing code)

**Duration:** Touches each file as we convert it
**Risk:** Low (same logic, flatter structure)

### Pattern

Every function we convert to the builder also gets flattened. The rule:

```rust
// Before (depth 4):
fn process(x: Option<Value>) -> Option<i64> {
    if let Some(val) = x {
        if let Some(result) = val.as_i64() {
            if result > 0 {
                return Some(result);
            }
        }
    }
    None
}

// After (depth 2):
fn process(x: Option<Value>) -> Option<i64> {
    let val = x?;
    let result = val.as_i64()?;
    if result <= 0 {
        return None;
    }
    Some(result)
}
```

### Functions to refactor

| Function | Current depth | Strategy |
|----------|-------------|----------|
| `emit_ssa_main` | 6 | Extract 3 helpers, use guard clauses |
| `emit_folded_memory_main` | 5 | Extract body/latch helpers |
| `emit_countable_main` | 4 | Extract latch/phi helpers |
| `emit_stmt` in `emit_stmt.rs` | 8 | Extract field-store helpers (already started: ensure_typed_value) |

### Deferred (structural chains, not repeated patterns)

The following files have depth > 2 but the depth comes from unique
conditional logic per function, not from repeated code blocks. They
are deferred until those functions need modification:

| File | Lines | Max depth | Reason |
|------|-------|-----------|--------|
| `mod.rs` | 3223 | 13 | Dispatch decision tree — will flatten when dispatch changes |
| `transition_graph.rs` | 1987 | 9 | Analysis module, not backend |

---

## Phase 3: Remove Old Paths

**Duration:** One commit at the end of Phase 1
**Risk:** Low (old paths are dead code after migration)

Remove all `writeln!`-based emission code that has a builder equivalent.
Remove `emit_folded_main` and `emit_folded_memory_main` if they're fully
superceded by `emit_countable_main`. Remove `#![allow(unused_imports)]`
workarounds.

---

## Timeline

| Phase | What | Est. time | Depends on |
|-------|------|-----------|------------|
| 0 | SROA fix (phi in done:) | 10 min | None |
| 1a | loop_engine.rs builder + flatten | 4-6 hours | Phase 0 |
| 1b | emit_stmt.rs builder + flatten | 3-5 hours | Phase 1a |
| 1c | emit_toplevel.rs builder + flatten | 2-3 hours | Phase 1a |
| 2 | mod.rs dispatch cleanup | 1-2 hours | Phase 1 |
| 3 | Remove old paths | 30 min | Phase 1-2 |

## Expected Outcome

1. **Vectorization:** SROA decomposes `%State` → vectorizer sees individual
   float phis → nbody_sqrt approaches 1.0x of C
2. **Code quality:** LLVM IR generated through typed builder methods,
   automatic register allocation, no string collisions
3. **Flat structure:** Max depth 2 across all backend files
4. **Correctness:** All benchmarks MATCH, all 1364 tests pass

## Files Created/Modified

| File | Status | Notes |
|------|--------|-------|
| `docs/plans/2026-07-03-backend-cleanup.md` | New | This document |
| `src/backend/llvm/builder.rs` | Extend | May need new emit_ methods (e.g. fadd, fmul, select) |
| `src/backend/llvm/loop_engine.rs` | Heavy refactor | Main target — builder migration + flattening |
| `src/backend/llvm/emit_stmt.rs` | Moderate refactor | Builder migration where needed |
| `src/backend/llvm/emit_toplevel.rs` | Light refactor | Builder migration where needed |
| `src/backend/llvm/mod.rs` | Light refactor | Dispatch cleanup |
