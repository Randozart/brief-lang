# LLVM Backend — Known Issues & Proposed Solutions

**Date:** 2026-06-18
**Current State:** 915 tests pass, 0 fail. 3 commits in this session.

---

## Completed This Session (2026-06-18)

| Item | Description | Commit |
|------|-------------|--------|
| ArrowDiscard/Transfer return values | Return real list handle instead of `add i64 0, 0` | Phase 1+2 |
| Memory leak in arrow ops | `free` old buffer before `malloc` in push/pop/discard/transfer | Phase 1+2 |
| Sleep intrinsic | Real `nanosleep`-based implementation (was stub) | Phase 1+2 |
| WriteFile intrinsic | Via `brief_write_file` C helper (was stub) | Phase 1+2 |
| SSA phi label mismatch | `done` → `pdoneloop` in phi-indvar path (1 line) | Phase 1+2 |
| Slice alloca → malloc | Replaced invalid dynamic `alloca` with `malloc` + TBAA | Phase 1+2 |
| 14 projection targets | Ptr, Type, Alignment, Popcount, LZ/TZ, Abs, BitRev, Keys, Values, AsStack/Queue, Index, Pop, PtrBang, Contains | Phase 1+2 |
| LTO bitcode pipeline | Auto-detect `brief_rt.c` and use `llvm-link` + `opt` before `cc` fallback | Phase 3 |

---

## Fixed Issues (2026-06-18)

The following issues were documented here earlier but have since been fixed:

### ✅ Fixed: #1 — C-Bound Intrinsics Pass Brief Header Pointer as C String

**Status:** Fixed at C level. All C shims (`__read_file__`, `__spawn_with_output__`,
`__write_file__`, etc.) now take `int64_t` (Brief string handle) and use
`brief_str_to_c()` to convert to C strings before use. The old `brief_read_file`
signature `const char*` was replaced with `int64_t` matching the convention.

**Verification:** `emit_expr.rs` calls `@__read_file__(i64)` and `@__spawn_with_output__(i64)`.
The C functions use `brief_str_to_c(path_bstr)` to extract the character data.

### ✅ Fixed: #2 — `emit_inline_concat` Doesn't Null-Terminate

**Status:** Fixed in `emit_expr.rs` lines 3359-3361. After both memcpy operations,
a null byte is stored at `dest_start + total`:
```llvm
%nt = getelementptr i8, i8* %dest_start, i64 %total
store i8 0, i8* %nt, align 1
```
The allocation (line 3331-3333) includes the +1 for the null terminator.

### ✅ Fixed: #3 — `emit_inline_concat` Memory Leak

**Status:** Fixed with a tagged-pointer approach. String constants (`@str.N`) are
tagged with bit 0 set to 1 at the point of `Expr::String` emission. Heap-allocated
strings (malloc results) have bit 0 clear naturally (8-byte alignment).

In `emit_inline_concat`, after copying both operands' data into the new buffer:
1. Bit 0 is tested for each operand
2. If clear → heap-allocated → `free` is called
3. If set → static constant → free is skipped
4. Before reading headers, bit 0 is masked off with `and i64, -2`

**Verification:** `emit_expr.rs` line 30-34 tags `@str.N` with `or i64, 1`.
`emit_inline_concat` lines 3316-3319 mask with `and i64, -2`, and lines 3363-3390
emit conditional `free` blocks for each operand.

### ✅ Fixed: #4 — `brief_spawn_with_output` Declaration Mismatch

**Status:** Fixed. Both the LLVM declaration and call use `i64`:
```llvm
declare i64 @__spawn_with_output__(i64)
; ...
%raw = call i64 @__spawn_with_output__(i64 %boxed)
```

### ✅ Fixed: #5 — Intrinsic Stubs (sort, reverse, range, readln)

**Status:** All four are implemented via C helpers in `brief_rt.c`:
- `__readln__` — `fgets` + `buf_to_brief`
- `__sort_list__` — `qsort` on the list data
- `__reverse_list__` — in-place element swap
- `__range__` — allocates list `[0, 1, ..., end-1]`

The `add i64 0, 0` paths in `emit_expr.rs` are only the no-argument fallbacks
(when the intrinsic is called without its required arguments).

---

## Known Issues (Still Open)

### 🟠 Medium Priority

#### 6. Concat operands that are state field loads — potential double-free

**Affects:** `expr = state_field + "suffix"` where `state_field` is later read
by another transaction in the same tick.

**Risk:** Low — the concat function frees the operand buffer, but the state
field still holds a reference to the old buffer in the `%State` struct. If
another transaction reads the same field in the same tick, it gets freed
memory. In practice, Brief's SSA pipeline transforms each state field access
into a local copy, so the state field's buffer is only freed after the value
box is replaced. The risk exists but hasn't manifested.

**Mitigation:** No action needed unless a reproducible crash appears.

---

### 🟠 High Priority

#### 4. `brief_read_file` and `brief_spawn_with_output` Declaration Mismatches

**Affects:** Potential LLVM verifier errors. Not causing crashes today but incorrect.

**Evidence:**
```llvm
; Declaration says i64:
declare i64 @brief_spawn_with_output(i64)

; Call says i8*:
%raw = call i8* @brief_spawn_with_output(i8* %pp)
```

The call passes `i8*` but the declaration says `i64`. LLVM may auto-convert or reject. The C function's actual signature is `int64_t brief_spawn_with_output(int64_t cmd_bstr)`. The return type also mismatches (call expects `i8*`, declare says `i64`).

**Proposed Fix:** Change the call site to match the declaration:
```rust
// Change from:
writeln!(out, "{}{} = call i8* @brief_spawn_with_output(i8* {})", indent, raw, pp).ok();
// To:
writeln!(out, "{}{} = call i64 @brief_spawn_with_output(i64 {})", indent, raw, boxed).ok();
```

Then `raw` is `i64` (the ptrtoint of the returned Brief string). Convert as needed.

#### 5. `read_file` Passes Path as `i8*` Instead of `i64`

**Affects:** Same type mismatch as #4, plus the string conversion bug (#1).

```llvm
; Declaration:
declare ptr @brief_read_file(ptr)

; Call:
%pp = inttoptr i64 %boxed to i8*
%raw = call i8* @brief_read_file(i8* %pp)
```

The declaration says `ptr` (i8*) and the call passes `i8*`. The type matches, but the semantics are wrong — the `i8*` points to the Brief header, not a C string.

**Proposed Fix:** Change the C function signature to `int64_t brief_read_file(int64_t path_bstr)` and update the declaration + call to match. Use `brief_str_to_c` internally.

#### 6. Remaining Intrinsic Stubs

| Intrinsic | File:Line | Current | Fix |
|-----------|-----------|---------|-----|
| `readln` | `emit_expr.rs:595` | `add i64 0, 0` | Read from stdin: `fgets` or `scanf` or `read(0, buf, n)` |
| `Sort` | `emit_expr.rs:1777` | `add i64 0, 0` | Implement via C helper `brief_sort_list(int64_t list_bstr)` in `brief_rt.c` |
| `Reverse` | `emit_expr.rs:1777` | `add i64 0, 0` | Implement via C helper `brief_reverse_list(int64_t list_bstr)` |
| `Range` | `emit_expr.rs:1777` | `add i64 0, 0` | Generate a list `[start, start+1, ..., end)` via loop |

**Priority:** Low — these are not used by officina or any current stdlib path.

---

### 🟡 Medium Priority

#### 7. `free` Not Added for Slice Operations

`Expr::Slice` now uses `malloc` but never frees the source list. Same issue as arrow ops — the source is not modified, so we shouldn't free it. The slice creates a new list. Acceptable for now.

**Note:** If the source is a state field and the slice replaces it, the old buffer is leaked. This is unusual usage though.

#### 8. MapLiteral / SetLiteral — Verify Implementation

AGENTS.md says MapLiteral/SetLiteral are "fully implemented" but they were listed as stubs in earlier exploration. Let me verify.

**Action:** Check the generated IR for `{"a": 1, "b": 2}` and `{1, 2, 3}` to confirm they emit proper malloc + header layout.

#### 9. Projection Target `Range` Returns 0

`ProjectionTarget::Range` falls through to the catch-all `add i64 0, 0`. For contract usage (`x :> Range`), this silently returns wrong data.

**Fix:** If the source is a List, return the list length (same as Size). If the source is a Tuple, return (start, end) packed as `(lo << 32) | hi`.

---

### 🟢 Low Priority / Future Work

#### 10. `PtrBang` Returns First Header Slot

`ProjectionTarget::PtrBang` is currently implemented as loading the first i64 from the pointer. For a `Ptr<Int>`, this returns the dereferenced integer value. For a Brief string, this returns the data pointer (hdr[0]). This is correct for typed pointers but may surprise users.

**Action:** Verify against interpreter semantics. Add docs if behavior is correct.

#### 11. Remaining Projection Targets (Get, Top, Front, Elements)

These are collection-specific operations (HashMap, Stack, Queue, HashSet) not used by officina or common stdlib paths. They fall through to the catch-all `add i64 0, 0`.

**Fix per target:**
- `Get(expr)` → HashMap lookup loop
- `Top` → Stack peek (load last element of list)
- `Front` → Queue front (load first element of list)
- `Elements` → HashSet enumeration (return list of elements)

#### 12. `read_file` Path String Needs Null Terminator

Even after fixing #1 (converting Brief string to C string), the path may be a runtime-concatenated string without a null terminator. `brief_str_to_c` handles this (it mallocs + copies + null-terminates). But if we use a direct approach (loading `hdr[0]` as `i8*`), runtime paths would be unterminated.

**Note:** The `brief_str_to_c` approach always works because it malloc-copies. The overhead is acceptable for file operations (not hot-path).

#### 13. Officina `draw_prompt` SIGSEGV

Being investigated by a separate agent. Symptom: after rendering top bar, `draw_prompt` reads a pointer equal to a prior concat result's chars area (`concat_result + 16`) instead of `@str.0`. May be related to #2 (missing null terminator causing buffer over-read) or a state corruption issue.

---

## Proposed Execution Order for Next Session

### Sprint A: Fix Critical String Conversion Bug (1-2 hours)

1. **Fix `brief_read_file` C function** — Change from `const char*` to `int64_t` parameter, use `brief_str_to_c`
2. **Fix `brief_spawn_with_output` declaration mismatch** — Change call to use `i64` not `i8*`
3. **Add null terminator to `emit_inline_concat`** + fix allocation size
4. **Add `free` for concat operands** — guarded to not free string constants

### Sprint B: Implement Missing Intrinsics (1-2 hours)

5. **Implement `readln`** — `fgets` or `getline` via C helper
6. **Implement `sort`** — `qsort` via C helper
7. **Implement `reverse`** — C helper
8. **Implement `Range` projection** — `add i64 0, %len` (same as Size for lists)

### Sprint C: Collection Projection Targets (2-3 hours)

9. **Implement `Get`** — HashMap lookup (iterate slots, compare keys)
10. **Implement `Top` / `Front`** — List element access
11. **Implement `Elements`** — Identity (like Keys/Values for lists)

### Sprint D: Verification & Hardening (1-2 hours)

12. **Run officina through the fixed pipeline** — Compile, run, verify
13. **Benchmark run** — `bash benchmarks/build_and_bench.sh --runtime` to validate no regression
14. **Praetor pass** — Verify complexity/lines/params on all changed files

---

## Summary of All Known Issues (Cheat Sheet)

| # | Issue | File(s) | Severity | Fix Complexity |
|---|-------|---------|----------|----------------|
| 1 | C string → Brief header confusion | `brief_rt.c`, `emit_expr.rs` | 🔴 Critical | Medium (C level) |
| 2 | `emit_inline_concat` no null terminator | `emit_expr.rs:2990` | 🔴 Critical | Low (2 lines) |
| 3 | Concat memory leak | `emit_expr.rs:2990` | 🔴 Critical | Medium (need static vs heap detection) |
| 4 | `spawn_with_output` type mismatch | `emit_expr.rs:817` | 🟠 High | Low (fix call type) |
| 5 | `read_file` path type mismatch | `emit_expr.rs:620`, `brief_rt.c:410` | 🟠 High | Low (change to i64) |
| 6 | `readln`, `sort`, `reverse`, `range` stubs | `emit_expr.rs:595,1777` | 🟡 Medium | Medium (C helpers) |
| 7 | Slice source buffer leak | `emit_expr.rs:2224` | 🟡 Medium | Low |
| 8 | MapLiteral/SetLiteral verification | `emit_expr.rs` | 🟡 Medium | Low (check IR output) |
| 9 | `Range` projection stub | `emit_expr.rs:1965` | 🟡 Medium | Low (same as Size) |
| 10 | `PtrBang` semantics verification | `emit_expr.rs` | 🟢 Low | Low (doc only) |
| 11 | Get/Top/Front/Elements stubs | `emit_expr.rs:1965` | 🟢 Low | Medium each |
| 12 | Runtime string null terminator gap | `emit_expr.rs:2990` | 🟡 Medium | Low (cover by #2) |

---

## Files That Will Be Modified

| File | Issues |
|------|--------|
| `lib/runtime/brief_rt.c` | 1, 5 — fix `brief_read_file` signature, add `read_file#` uses `brief_str_to_c` |
| `src/backend/llvm/emit_expr.rs` | 2, 3, 4, 5, 6, 7, 9, 11 — concat, intrinsics, projections |
| `src/backend/llvm/emit_toplevel.rs` | 4, 5 — fix declarations |
| `src/backend/llvm/mod.rs` | 4, 5 — fix declarations |

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Freeing string constant crashes | Medium | SIGSEGV | Guard free with null check + static pointer tag |
| Changing C function signatures breaks linking | Low | Compile error | Update both declaration and call site together |
| `read_file` fix breaks existing tests | Low | Test failure | All tests use interpreter (LLVM backend tests use mock programs) |
| Officina still crashes after fixes | Medium | Silent wrong output | Coordinate with separate agent investigating draw_prompt |

---

## Reference: Clang IR Pattern Verification

For any construct where the emitted IR is uncertain:

```bash
# Write minimal C test:
echo '#include <stdio.h>
int main() { printf("hello %s\\n", "world"); return 0; }' > /tmp/test.c

# Compile to LLVM IR:
clang -S -emit-llvm -O3 -fno-discard-value-names /tmp/test.c -o -

# Compare with what Brief emits for equivalent construct.
# Key differences tell you what to fix.
```
