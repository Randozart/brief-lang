# Examples Overhaul — Fix, Migrate, and Complete

**Date:** 2026-06-24
**Scope:** All `examples/` files + `lib/std/` frgn-to-intrinsic migration

## Goals

1. **Fix `[true]` contracts** — Every weak `[true]` / `[true][true]` contract replaced with meaningful pre/post
2. **Stale syntax** — Multi-output, TOML imports, dead code, method calls → current syntax
3. **`lib/std/` frgn → intrinsic** — Convert all `frgn` declarations that have matching `Intrinsic` variants
4. **15+ new example files** — Cover every feature currently missing an example

---

## Phase 1: Fix Existing Examples

### 1.1 Strengthen Contracts (17 files, ~33 occurrences)

| File | Occurrences | Replacement |
|---|---|---|
| `wasm-import.rbv:32` | `[true][true]` | `[result != @result \|\| tick_count < 1][tick_count == @tick_count + 1]` |
| `shopping_cart.rbv` | 8× `[true]` | Guard against invalid state transitions (already in shop vs checkout, etc.) |
| `rbv-no-script-tags.rbv` | 2× `[true]` | `[count < MAX]` / `[count > 0]` |
| `todo.rbv` | 2× `[true]` | `[new_task != ""]` / `[items .#Size > 0]` |
| `timeout_test.ebv` | `[true]` post | Meaningful postcondition |
| `stdlib_usage.bv` | Tautology | Remove placeholder, rewrite with real stdlib calls |
| `main.bv` | `[true][true]` | `[source != ""][term >= 0]` |
| `counter.rbv` | 2× `[true]` | Guard bounds on increment/reset |
| `hello/main.bv` | 2× `[true]]` | Actual output guarantee |
| `fizzbuzz.bv` | `[true]` post | `[current == 100]` |
| `contract_verification.bv` | 2× weak | Guard deposit against overflow, fix sufficient_funds logic |
| `hello-world/src/main.bv` | `[true]` pre | `[true]` → meaningful |
| `constraint_range.bv` | `[true]` post | `[score == @score + 1]` |
| `constraint_expression.bv` | `[true]` post | `[value == @value + 1]` |
| `test_ffi_minimal.bv` | `[true]]` | `[output_written == false][output_written == true]` |
| `test_ffi.bv` | 5× `[true]` | Guard each test against expected results |
| `cobol/bank_system.bv` | `[true]` pre | `[total_withdrawals > 0]` |

### 1.2 Fix Stale/Broken Syntax

- **`multi_output.bv`**: Fix tuple destructuring — `let a: Int; let b: String; let c: Bool;` followed by `let results = process_step()` but `a,b,c` never bound to results. Change to `let (a, b, c) = process_step();`
- **`main.bv`**: Align `compile_main` body with postcondition `[term == "compilation complete"]` or change postcondition
- **`inop-sadd.bv`**: Remove dead code after `term!` (lines 23-24)
- **`wasm-import.rbv`**: Remove deprecated `import "link/default.toml"`, declare `data`/`count` state variables, fix `[true][true]`
- **`test_ffi.bv`**: Replace TOML-binding `frgn from "std/bindings/string.toml"` with GLUE-style `frgn` declarations
- **`todo.rbv`**: Replace `items.len()` with `items .#Size` projection operator
- **`test_ffi_minimal.bv`**: Fix typo `test_ff` → `test_ffi`

### 1.3 Rewrite Placeholder/Broken Files

- **`stdlib_usage.bv`** — Currently all comments with no actual stdlib imports. Rewrite to import and use `char`, `bits`, `option`, `result` modules
- **`multi_output.bv`** — Fix logic bug and make it a working example
- **`contract_verification.bv`** — Fix `sufficient_funds` to actually check the contract

---

## Phase 2: `lib/std/` frgn → Intrinsic Conversion

Convert `frgn` declarations in `lib/std/` where a matching `Intrinsic` variant exists:

| File | `frgn` Name | Intrinsic | Status |
|---|---|---|---|
| `shm.bv:29` | `__munmap` | `munmap#` | Comment says "Replaced" already |
| `ffi/shm.bv:29` | `__munmap` | `munmap#` | Comment says "Replaced" already |
| `process.bv:7` | `__spawn` / `__spawn_with_output` | `spawn#` / `spawn_with_output#` | Comment says "Replaced" |
| `process.bv` | `__env_var` / `__set_env_var` | `getenv#` / `setenv#` | Convert |
| `env.bv` | `__get_env_int` | `getenv_int#` | Convert |
| `shm.bv` | `__atomic_cas`, `__atomic_store`, `__atomic_fence`, `__atomic_add`, `__atomic_load`, `__atomic_xchg` | `atomic_cas#`, `atomic_store#`, `fence#`, `atomic_add#`, `atomic_load#`, `atomic_xchg#` | Convert |
| `string.bv` (ffi) | String operations with `__name__` convention | Various string intrinsics | Evaluate per function |

---

## Phase 3: New Example Files

### 3.1 Core Language Features

| # | File | Type | Features |
|---|------|------|----------|
| 1 | `examples/macro-demo.bv` | `.bv` | `$!macro`, `quote { }`, `@`-interpolation, `compile#()`, `error#()`, `warn#()`, `gensym#()` |
| 2 | `examples/proof-oracle.bv` | `.bv` | `?#` proof oracle, fuel injection, structural recursion checker |
| 3 | `examples/foreach.bv` | `.bv` | `foreach` over `List<T>`, `!llvm.loop.vectorize.enable` SIMD metadata |
| 4 | `examples/sync-domain.bv` | `.bv` | `sync(domain)` prefix on `txn`/`defn`, `SyncBlock`, `TopLevel::SyncGroup` |
| 5 | `examples/arrow-mutation.bv` | `.bv` | `<-` push/pop/discard/transfer for List, Stack, Queue, HashMap, HashSet |
| 6 | `examples/projections.bv` | `.bv` | All projection targets: `Keys`, `Values`, `Contains`, `Pop`, `Index`, `Ptr`, `Bytes`, `Alignment`, `Range`, `Popcount`, `LeadingZeros`, `TrailingZeros`, `Absolute`, `BitReverse`, `Type`, `Match` |
| 7 | `examples/map-set.bv` | `.bv` | `{"a": 1}` MapLiteral, `{1, 2, 3}` SetLiteral, `:>` operations |
| 8 | `examples/swan-song.bv` | `.bv` | `term -> print_int#(x)` (commit action), `term! -> print_int#(x)` (exit), difference |

### 3.2 Stdlib & Error Handling

| # | File | Type | Features |
|---|------|------|----------|
| 9 | `examples/error-handling.bv` | `.bv` | `import Option/Result from stdlib`, `uni` pattern matching on `Ok`/`Err`/`Some`/`None` |
| 10 | `examples/stdlib-demo.bv` | `.bv` | Import and use HashMap, HashSet, Stack, Queue, StringBuilder, Iterator, string/char from stdlib |

### 3.3 Domain-Specific

| # | File | Type | Features |
|---|------|------|----------|
| 11 | `examples/gpu-compute.abv` | `.abv` | `get_global_id#`, `get_local_id#`, `get_group_id#`, `get_num_groups#`, `barrier#` |
| 12 | `examples/networking.bv` | `.bv` | `socket#`, `bind#`, `connect#`, `send#`, `recv#`, TCP client pattern |
| 13 | `examples/process-spawn.bv` | `.bv` | `spawn#`, `spawn_with_output#`, `argv#`, `getpid#`, `getenv#`/`setenv#` |
| 14 | `examples/mmap-demo.bv` | `.bv` | `mmap#`, `munmap#`, `mprotect#`, memory-mapped file I/O |

### 3.4 Hardware / Brief Variants

| # | File | Type | Features |
|---|------|------|----------|
| 15 | `examples/inop-side-effect.bv` | `.bv` | `inop!` (side-effectful intrinsic), BILD body with side-effects |
| 16 | `examples/data-brief` | `.dbv/.dbvs/.dbvl` | Data Brief schema, data files, validation |
| 17 | `examples/glue-macro.bv` | `.bv` | `#export` with `frgn` + `meld`, full GLUE bridge pattern |
| 18 | `examples/assume-pragma.bv` | `.bv` | `#assume_event(name)`, `#assume_shape(guard, action)` pragmas |

### 3.5 RBV View Enhancements

| # | File | Type | Features |
|---|------|------|----------|
| 19 | `examples/view-directives.rbv` | `.rbv` | `b-style`, `b-class`, `b-if`, `b-show`, `b-each` with more complex template |

---

## Phase 4: Compile Verification

After all edits:
1. `cargo build` — ensure compiler still builds
2. For each example: capture compilation errors and fix
3. `cargo test --lib` — all tests pass

---

## Execution Order

1. Write and commit this plan file
2. Phase 1: Fix all existing examples (contracts + syntax)
3. Phase 2: `lib/std/` frgn → intrinsic migration
4. Phase 3: Create new example files (highest-priority first)
5. Phase 4: Build verification + test pass

## Files to Touch

- **Fix in place (18 files):** `wasm-import.rbv`, `shopping_cart.rbv`, `rbv-no-script-tags.rbv`, `todo.rbv`, `timeout_test.ebv`, `stdlib_usage.bv`, `main.bv`, `counter.rbv`, `hello/main.bv`, `fizzbuzz.bv`, `contract_verification.bv`, `hello-world/src/main.bv`, `constraint_range.bv`, `constraint_expression.bv`, `test_ffi_minimal.bv`, `test_ffi.bv`, `multi_output.bv`, `inop-sadd.bv`, `cobol/bank_system.bv`
- **Rewrite (3 files):** `stdlib_usage.bv`, `multi_output.bv`, `contract_verification.bv`
- **Stdlib migrate (4+ files):** `lib/std/env.bv`, `lib/std/process.bv`, `lib/std/shm.bv`, `lib/std/ffi/shm.bv`
- **New files (19):** `macro-demo.bv`, `proof-oracle.bv`, `foreach.bv`, `sync-domain.bv`, `arrow-mutation.bv`, `projections.bv`, `map-set.bv`, `swan-song.bv`, `error-handling.bv`, `stdlib-demo.bv`, `gpu-compute.abv`, `networking.bv`, `process-spawn.bv`, `mmap-demo.bv`, `inop-side-effect.bv`, `data-brief/*`, `glue-macro.bv`, `assume-pragma.bv`, `view-directives.rbv`
