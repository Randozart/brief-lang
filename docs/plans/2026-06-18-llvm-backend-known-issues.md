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

## Known Issues (Discovered But Not Fixed)

### 🔴 Critical

#### 1. C-Bound Intrinsics Pass Brief Header Pointer as C String

**Affects:** `read_file#`, `spawn_with_output#`, `spawn#`, and any intrinsic calling a C function with string parameters.

**Root Cause:** The LLVM IR emits:
```llvm
%boxed = ptrtoint i8* %string_reg to i64    ; Brief string header → integer
%c_ptr = inttoptr i64 %boxed to i8*          ; integer → C string pointer
call i8* @brief_read_file(i8* %c_ptr)        ; C function receives header pointer
```

The C function receives a pointer to the Brief string struct (`<{ i64, i64, [N x i8] }>`), whose first 8 bytes are the data pointer (a large address in little-endian). `fopen` interprets these bytes as a file path — garbage for any real path.

**Why it "works":** `brief_read_file` returns NULL when `fopen` fails, and the caller treats NULL as Err. So `read_file` always returns `Err("file not found")`. It silently fails for every input.

**Same issue affects:** `brief_spawn_with_output` (line 817 in `emit_expr.rs` — but its C declaration says `declare i64 @brief_spawn_with_output(i64)` while the call passes `i8*`... the types are mismatched but LLVM silently converts).

**Evidence:** `brief_rt.c:410`:
```c
char* brief_read_file(const char* path) {
    if (!path) return NULL;
    FILE* fp = fopen(path, "rb");  // path = header struct, not C string!
```

**Proposed Fix:**

Option A — Fix at C level (recommended, minimal LLVM IR changes):
- Change `brief_read_file` to take `int64_t path_bstr` (like `brief_spawn_with_output`)
- Use `brief_str_to_c(path_bstr)` to convert to C string before `fopen`
- Same for any other C shim receiving string parameters
- This is the pattern already used correctly by `brief_spawn_with_output`

Option B — Fix at LLVM IR level (more correct, more work):
- In LLVM IR, load the data pointer from the Brief string header
- The data pointer is at `hdr[0]` (first i64 slot of the struct, which is `ptrtoint` of offset 16)
- Convert `hdr[0]` to `i8*` and pass that to C functions
- But this only works for string constants (which have a null terminator)
- Runtime strings (via `emit_inline_concat`) don't have null terminators

**Recommendation:** Option A — fix at C level. It's a 5-line change per function and matches the pattern used by `brief_spawn_with_output`, `brief_str_to_c`, etc.

#### 2. `emit_inline_concat` Doesn't Null-Terminate

**Affects:** All runtime string concatenations.

**Root Cause:** `emit_inline_concat` (line 2990 in `emit_expr.rs`) allocates `(total + 2) * 8` bytes, copies header + characters, but does NOT append a `\0` null terminator after the character data.

String constants DO have a null terminator (see `mod.rs:1083` — `[{} x i8] c\"{}\\00\"`). But runtime strings don't. This means:
- Passing a runtime-concatenated string to any C function (even after Fix #1) would read past the buffer
- `strlen()` on a runtime string reads garbage

**Proposed Fix:** In `emit_inline_concat`, after the final `memcpy`, emit:
```llvm
%null_term = getelementptr i8, i8* %dest, i64 %total
store i8 0, i8* %null_term
```
This adds 1 byte of null terminator after the last character. The allocation should also be increased by 1 byte.

**Same issue:** `read_file`'s Ok path (line 670-674) DOES null-terminate (`store i8 0`). The concat path doesn't. Add it.

#### 3. `emit_inline_concat` Memory Leak

**Affects:** Every `s1 + s2` expression leaks both operands' buffers.

**Root Cause:** `emit_inline_concat` allocates a new buffer but never frees the old buffers (`a` and `b` operands). Same pattern as the arrow ops before we fixed them.

**Challenge:** Unlike arrow ops (which always operate on state fields = heap buffers), concat operands could be:
- String constants (`@str.N`) — MUST NOT free (static data)
- State field reads — SHOULD free (heap buffers)
- Intermediate concat results — SHOULD free (heap buffers)
- Literal empty strings — MAY be null (check first)

**Proposed Fix:** Add `free` calls for both operands, but guard against null:
```llvm
; Free operand A if non-null
%is_null_a = icmp eq i64 %a_boxed, 0
br i1 %is_null_a, label %skip_free_a, label %do_free_a
do_free_a:
  %a_ptr = inttoptr i64 %a_boxed to i8*
  call void @free(i8* %a_ptr)
  br label %skip_free_a
skip_free_a:
; (same for B)
```

This is safe: `free(NULL)` is a no-op in C, but since we're in LLVM IR, we call `@free(i8* null)` which is also well-defined (it's the same libc `free`). Actually, `free(null)` is guaranteed to be safe by the C standard. So we can skip the null check and just call `free` unconditionally — `free(null)` is well-defined.

The remaining issue: string constants like `@str.0` are in the data section, NOT heap-allocated. Freeing them would crash. We need a way to distinguish heap strings from static strings.

**Actually,** looking at how strings are created:
- String constants (`Expr::String`) → bitcast of a global constant → should NOT be freed
- Runtime strings (`emit_inline_concat`, `read_file#`, list operations) → result of `malloc` → SHOULD be freed

The expression `s1 + s2` where `s1` is a literal `"hello"` and `s2` is `current_input` (state field): freeing `@str.0` would crash.

**Need:** A runtime tag or separate allocator. Without it, we can't safely free concat operands.

**Short-term:** Don't free concat operands. Accept the leak for now. The arrow ops already handle state-field paths.

**Medium-term:** Add a "heap-allocated" bit to the lowest bit of the boxed pointer (since mallocs are at least 8-byte aligned, bit 0 is always 0 for heap pointers and 1 for static pointers, or vice versa).

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
