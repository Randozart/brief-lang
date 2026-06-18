# LLVM Backend Hardening — Comprehensive Fix Plan

**Date:** 2026-06-18

**Governing Principle:** Brief shall not break under valid syntax. If the compiler accepts a program, the binary must be correct or the compiler must emit a compile-time error. A silently-crashing binary is a compiler bug — always.

**Current State:** 915 tests pass, 0 fail. Officina compiles, boots, renders top bar, crashes in `draw_prompt` (SIGSEGV — under investigation by separate agent).

---

## Phase 1: Routine Fixes (Small, Well-Understood)

### 1a. Fix `ArrowDiscard` Return Value

**File:** `src/backend/llvm/emit_expr.rs:2561`

**Problem:** The discard handler allocates a new buffer, copies before/after elements, writes back to state — but then returns `add i64 0, 0 ; discard void` with `Type::Void`. The state IS correctly updated, but the expression value is wrong.

**Fix:** Replace the stub return with the actual list handle (`base` register at line 2519: `ptrtoint i8* %new_buf to i64`), change `ty: Type::Void` to `ty: Type::Int`.

**Lines to change:**
```
- writeln!(out, "{}{} = add i64 0, 0 ; discard void", indent, v).ok();
- return TypedRegister { name: v, ty: Type::Void };
+ writeln!(out, "{}{} = add i64 0, {}", indent, v, base).ok();  // or just reuse base
+ return TypedRegister { name: v.clone(), ty: Type::Int };
```

### 1b. Fix `ArrowTransfer` Return Value

**File:** `src/backend/llvm/emit_expr.rs:2653`

**Problem:** Same pattern as Discard — allocates merged buffer, copies dest + source, writes both back — but returns `add i64 0, 0 ; transfer void`. The `dbase` register (line 2597: `ptrtoint i8* %new_buf to i64`) holds the correct merged list handle.

**Fix:** Return `dbase` with `Type::Int`.

**Lines to change:**
```
- writeln!(out, "{}{} = add i64 0, 0 ; transfer void", indent, v).ok();
- return TypedRegister { name: v, ty: Type::Void };
+ writeln!(out, "{}{} = add i64 0, {}", indent, v, dbase).ok();
+ return TypedRegister { name: v.clone(), ty: Type::Int };
```

### 1c. Fix `<- Push` Memory Leak

**File:** `src/backend/llvm/emit_expr.rs:2364`

**Problem:** Every push calls `malloc` for the new buffer but never `free`s the old buffer. For long-running programs (e.g., officina with input accumulation), every keystroke leaks.

**Fix:** Before `malloc`, emit `call void @free(i8* %old_buf)` where `%old_buf` comes from `inttoptr i64 %list_boxed to i8*`. This is simpler than `realloc` and verifiably correct — the old buffer is no longer referenced after the new one is stored.

**Also applies to:** Pop (line 2442), Discard (line 2514), Transfer (line 2592), and `emit_inline_concat` (line 2972). All allocate new buffers without freeing old ones — fix all of them.

### 1d. Implement `sleep` Intrinsic

**File:** `src/backend/llvm/emit_expr.rs:692`

**Problem:** `Intrinsic::Sleep` returns `add i64 0, 1` — hardcoded success. The `Intrinsic::Nanosleep` (line 1740) already has a full LLVM IR implementation with `timespec` struct, `nanosleep` call, and remainder handling.

**Fix:** Either:
- Option A: Route `Intrinsic::Sleep` → `Intrinsic::Nanosleep` by converting seconds to nanoseconds, or
- Option B: Implement `call i64 @sleep(i32 %secs)` using libc's `sleep(3)`.

Option B is simplest. Uses `trunc i64 %secs to i32`, calls `@sleep(i32)`, result is `i32` → `zext to i64`.

**Also declare free if not already declared:** Add `declare void @free(i8*)` and `declare noalias i8* @realloc(i8*, i64)` to `mod.rs` declarations section.

### 1e. Implement `write_file` Intrinsic

**File:** `src/backend/llvm/emit_expr.rs:689`

**Problem:** `Intrinsic::WriteFile` returns `add i64 0, 1` — hardcoded success.

**Fix:** Use Clang IR as reference for the correct fopen → fwrite → fclose sequence:
```
fopen(path, "w")   → call ptr @fopen(ptr %path, ptr @WRITE_MODE)
fwrite(data, 1, n, fp) → call i64 @fwrite(ptr %data, i64 1, i64 %n, ptr %fp)
fclose(fp)         → call i32 @fclose(ptr %fp)
```
Returns `1` on success, `0` on failure (wrapped as `Int` → `Bool` via `icmp ne` or direct).

Needs globals: `@WRITE_MODE = private unnamed_addr constant [2 x i8] c"w\00"`

---

## Phase 2: Structural Fixes

### 2a. Fix SSA Phi Loop `done` Label Mismatch

**File:** `src/backend/llvm/loop_engine.rs:774`

**Problem:** Inside the phi-indvar body path (line 764-780), `loop_exit_label` is set to `Some("done".into())`. But the phi loop's termination label is `pdoneloop` (line 839). Any `term` statement inside a phi-loop body that branches to `%done` will reference an **undefined label**.

**Fix:** Change to `self.loop_exit_label = Some("pdoneloop".into())` at line 774, inside the `if self.phi_induction_reg.is_some()` block.

**Root cause:** The label was copied from the guard-based path (line 800) without accounting for the phi loop's different label naming (`pdoneloop` instead of `done`).

### 2b. Fix `Expr::Slice` Invalid `alloca`

**File:** `src/backend/llvm/emit_expr.rs:2194`

**Problem:** `alloca i64, i64 %count_reg` emits a dynamic-size typed alloca. This is valid LLVM IR only in the entry block. When used inside a txn body (non-entry block), the LLVM verifier rejects it.

**Fix:** Replace `alloca i64, i64 %count_reg` with `call i8* @malloc(i64 %byte_count)` followed by `bitcast i8* to i64*`. This produces valid IR in any block.

**Mathematical change:**
```
// Before:
ai = alloca i64, i64 %count   // invalid in non-entry block

// After:
byte_count = mul i64 %count, 8
raw = call i8* @malloc(i64 %byte_count)
ai = bitcast i8* %raw to i64*   // type-erased, valid anywhere
```

**All GEP and store instructions that reference `ai` continue to work unchanged** because they operate on `i64*`, which `ai` remains.

### 2c. Implement Remaining Projection Targets

**File:** `src/backend/llvm/emit_expr.rs:1933-1935`

**Problem:** The catch-all `_ =>` branch at line 1933 returns `add i64 0, 0 ; projection` for ALL non-Size, non-Bytes targets.

**Critical projection targets (needed by contracts and stdlib):**

| Target | Implementation | LLVM IR |
|--------|---------------|---------|
| `Ptr` | `ptrtoint` of list/int pointer to i64 | `%v = ptrtoint i64* %hp to i64` |
| `Alignment` | `add i64 0, 8` (default alignment) | `%v = add i64 0, 8` |
| `Range` | Return pair (lo, hi) as packed i64 | `%v = or i64 %lo, shl i64 %hi, 32` |
| `Popcount` | `@llvm.ctpop.i64` | `%v = call i64 @llvm.ctpop.i64(i64 %val)` |
| `LeadingZeros` | `@llvm.ctlz.i64` | `%v = call i64 @llvm.ctlz.i64(i64 %val, i1 false)` |
| `TrailingZeros` | `@llvm.cttz.i64` | `%v = call i64 @llvm.cttz.i64(i64 %val, i1 false)` |
| `Absolute` | `@llvm.abs.i64` | `%v = call i64 @llvm.abs.i64(i64 %val, i1 false)` |
| `BitReverse` | `@llvm.bitreverse.i64` | `%v = call i64 @llvm.bitreverse.i64(i64 %val)` |
| `Type` | `add i64 0, <type_id>` | `%v = add i64 0, 1` (compile-time type constant) |
| `Ptr!` | Dereference `Ptr<T>`: load from pointer stored in value | `%v = load i64, i64* %hp` |
| `Contains` | Linear search over list elements | Loop: GEP → load → icmp eq → br |
| `Keys`/`Values` | Return list handle as-is (for lists) | `%v = add i64 0, %src_val` |
| `Index(usize)` | GEP at fixed index | `%v = getelementptr i64, i64* %dp, i64 %idx` |

**Priority order:** Ptr, Popcount, LeadingZeros, TrailingZeros, Absolute, BitReverse first (direct LLVM intrinsic calls), then Index, Keys/Values, Contains, then Type, Ptr!, Range, Alignment.

---

## Phase 3: Infrastructure — C Shim Bitcode Pipeline

### 3a. Convert `brief_rt.c` to Bitcode Pipeline

**Files:** `src/backend/llvm/mod.rs`, `lib/runtime/brief_rt.c`

**Problem:** Current pipeline compiles Brief → LLVM IR (`program.ll`), then assembles to `.s` with `llc`, then links `brief_rt.o` as a native `.o` file. This prevents cross-module inlining and constant propagation through the C shim boundary.

**Fix:** Add an `llvm-link` step in the backend pipeline:

1. Pre-compile `brief_rt.c` to bitcode once (build.rs or manual):
   ```bash
   clang -O3 -emit-llvm -c lib/runtime/brief_rt.c -o lib/runtime/brief_rt.bc
   ```

2. After generating `program.ll`, merge with runtime bitcode:
   ```bash
   llvm-link program.ll lib/runtime/brief_rt.bc -S -o linked.ll
   ```

3. Run `opt -O3` on the merged module:
   ```bash
   opt -O3 linked.ll -S -o opt.ll
   ```

4. Assemble:
   ```bash
   llc -O3 opt.ll -o program.s
   clang program.s -o program -lm -lpthread -lrt
   ```

**Implementation:** Add `link_bitcode` method to the LLVM backend that runs `llvm-link` and `opt` via `std::process::Command`, gated on existence of `brief_rt.bc` (fall back to current .o linking if not found).

### 3b. Clang IR Reference Workflow

**Goal:** Use `clang -S -emit-llvm -O3 -fno-discard-value-names` to verify LLVM IR patterns.

For each construct where correctness is uncertain:
1. Write minimal C test case
2. Compile to LLVM IR with Clang
3. Compare with emitted Brief IR
4. Adjust Brief IR to match if Clang's version is more correct/optimizable

**Key targets for Clang reference:**
- `write_file` (fopen/fwrite/fclose)
- `read_file` (fopen/fseek/ftell/malloc/fread/fclose)
- String manipulation (`strlen`, `memcpy` patterns)
- Variadic function calls (`fprintf`)
- Atomic operations (`cmpxchg`, `atomicrmw`)
- Signal handling (`sigaction`, `sigprocmask`)

---

## Execution Order

| Step | Description | File | Lines Changed | Risk |
|------|-------------|------|---------------|------|
| 1a | Fix ArrowDiscard return | `emit_expr.rs` | ~3 | Low |
| 1b | Fix ArrowTransfer return | `emit_expr.rs` | ~3 | Low |
| 1c | Fix push/pop/discard/transfer leak | `emit_expr.rs` + `mod.rs` | ~25 | Low |
| 1d | Implement sleep intrinsic | `emit_expr.rs` | ~10 | Low |
| 1e | Implement write_file intrinsic | `emit_expr.rs` | ~30 | Medium |
| 2a | Fix SSA phi label | `loop_engine.rs` | 1 | Low |
| 2b | Fix Slice alloca | `emit_expr.rs` | ~15 | Medium |
| 2c | Projection targets | `emit_expr.rs` | ~80 | Medium |
| 3a | Bitcode pipeline | `mod.rs` | ~50 | Medium |

**Commit after each phase:** `git add` + `git commit -m "Phase X: description"`

---

## Files Modified

| File | Phase(s) | Nature of Change |
|------|----------|------------------|
| `src/backend/llvm/emit_expr.rs` | 1a, 1b, 1c, 1d, 1e, 2b, 2c | Replace stub `add i64 0, 0` with real IR |
| `src/backend/llvm/loop_engine.rs` | 2a | Single `"done"` → `"pdoneloop"` string change |
| `src/backend/llvm/mod.rs` | 1c, 3a | Add `free`/`realloc` declares, bitcode pipeline method |

---

## Verification

After each phase:
1. `cargo build` — no warnings
2. `cargo test --lib` — 915+ tests pass (no regressions)
3. If officina is fixed by other agent: `./target/release/brief-compiler llvm officina.bv -o officina` + run

Final:
4. Commit plan document alongside code
5. Document root causes in docs/architecture/praetor-log.md if applicable

## Clang IR Reference Workflow

Use Clang as a reference for correct LLVM IR patterns:

```bash
# See how Clang emits variadic function calls, structs, loops, etc.
clang -S -emit-llvm -O3 -fno-discard-value-names test.c -o -
# Pre-compile C shims to bitcode for cross-module inlining:
clang -O3 -emit-llvm -c brief_rt.c -o brief_rt.bc
# Merge with generated Brief IR:
llvm-link program.ll brief_rt.bc -S -o final.ll
# The compiler now auto-detects brief_rt.c and runs this pipeline
# before falling back to cc compilation.
```

This would have caught the `fprintf` variadic syntax bug instantly. Use for any construct where the emitted IR is uncertain.
