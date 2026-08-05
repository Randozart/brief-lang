# Execution Plan: String Architecture + Native Types + Officina Fixes

**Date:** 2026-06-16 23:00  
**Status:** Build  
**Mode:** Full cleanup — no "future sessions", no deferrals

---

## Architecture Decision

**Native LLVM types throughout, no i64 boxing for non-integer types.**
Strings are `i8*` (pointer to Briv 2-slot header). Bool is `i1` (SSA) / `i8` (memory). Char is `i32`. Only Int/UInt remain `i64` (they ARE integers).

This eliminates all `inttoptr`/`ptrtoint` round-trips for strings, `trunc/zext` round-trips for bools and chars. Every conversion to/from C is explicit and at the FFI boundary only.

---

## Execution Order (must be followed exactly)

### Step 0 — Add `TypedRegister::llvm()` method
**File:** `src/backend/llvm/emit_expr.rs`  
**What:** New method that maps `Type → native LLVM type string`. Single source of truth.

### Step 1 — String constants → global Briv headers
**File:** `src/backend/llvm/mod.rs`  
**Lines:** 947-950 (string constant emission)  
**Before:** `[N x i8] c"..."` C-string globals  
**After:** `<{i64, i64, [N x i8]}>` Briv-header globals (data_ptr, length, chars)  
**Test:** `cargo test --lib` still passes (no semantic change yet — downstream still uses ptrtoint)

### Step 2 — Update declares
**File:** `src/backend/llvm/mod.rs`  
**Changes:**
- Line 793: Remove `declare i8* @__str_concat(i8*, i8*) #1`  
- Add `declare noalias i8* @malloc(i64) #1`  
- Add `declare i64 @strlen(i8*) #1`  
- Line 781: Change `"i8"` to `"i8*"` for `Type::String | Type::Data`

### Step 3 — Cleanup `briv_rt.c`
**File:** `lib/runtime/briv_rt.c`  
**Remove:** `__str_concat` function, `safe_cstr` function, `__resolve_briv` helper, debug `fprintf(stderr, "DEBUG...")`  
**Keep:** `cstr_to_briv` (used internally by `__int_to_str` etc.)

### Step 4 — Native string operations in `emit_expr.rs`

**4a: `Expr::String`** (lines 29-35)  
- Before: `getelementptr [N x i8]* @str.X, i64 0, i64 0` → `ptrtoint i8* %p to i64`  
- After: `bitcast <{i64, i64, [N x i8]}>* @str.X to i8*` (returns `i8*`, not `i64`)

**4b: `Expr::Concat`** (line 272)  
- Before: `inttoptr i64 %a to i8*` → `call i8* @__str_concat` → `ptrtoint i8* %r to i64`  
- After: Inline `malloc` + header setup + `memcpy` on `i8*` directly (no inttoptr/ptrtoint)

**4c: `emit_binop` string path** (lines 1590-1600)  
- Same transformation as 4b — inline concat on `i8*`

**4d: FFI param marshaling** (lines 290, 327)  
- Before: `inttoptr i64 %raw to i8*` (passes header ptr — WRONG)  
- After: `bitcast i8* %raw to i64*` → `load i64, i64* %hp, align 8` (data_ptr) → `inttoptr i64 %dp to i8*`  
- BUT: since strings are now `i8*` natively, it's: `bitcast i8* %raw to i64*` → `load i64, i64* %hp` → `inttoptr i64 %dp to i8*` — wait, `%raw` IS already `i8*` (the Briv header pointer). So we just bitcast to `i64*` and load slot 0.

**4e: FFI return marshaling** (line 302 area)  
- When C returns `i8*`: strlen → malloc → header setup → memcpy → return `i8*`

**4f: Update TypedRegister creation** — all string-returning expressions set `ty: Type::String` and use `i8*` LLVM type.

### Step 5 — Native Bool type (i1/i8) in `emit_stmt.rs`

**5a: State field loads for Bool** — load as `i8`, use in SSA as `i1` via `trunc`
**5b: State field stores for Bool** — `zext i1 to i8`, store as `i8`
**5c: Conditional guards** — currently merge i64 to i1; with native i1, no trunc needed

### Step 6 — Native Char type (i32) in `emit_expr.rs`

**6a: `Expr::Char`** — emit `i32` directly instead of `add i32 0, N` + `zext i32 N to i64`
**6b: Char comparisons/ops** — use `i32` throughout

### Step 7 — Verify compiler builds and tests pass
```bash
cd ~/Desktop/Projects/briv-compiler
cargo test --lib
cargo build --release
```

### Step 8 — Officina `.bv` fixes

**8a: Remove malformed frgn declarations**  
File: `officina.bv` — delete lines 20-21 (frgn tty_raw_mode, frgn tty_size)

**8b: Convert to intrinsics**  
File: `officina.bv`
- `tty_raw_mode(true)` → `tty_raw_mode#(true)` (line 44, 99)
- `tty_size()` → `tty_size#()` (lines 46, 91)
- `exit#(0)` → `term! -> exit#(0);` (line 101)
- `&running = false` before exit action `term;` (line 111 area)

**8c: Fix typos**  
File: `officina.bv`
- `encoded . 10000` → `encoded / 10000` (lines 47, 92)
File: `officina/lib/std/io.bv`
- `[[term == true]` → `[[term == true]]` (line 14)

**8d: Remove duplicate stdlib**  
Delete: `lib/std/io.bv`, `lib/std/string.bv`, `lib/std/process.bv`, `lib/std/result.bv`, `lib/std/core/ptr.bv`

**8e: Logic bugfixes**
- `rules.bv:8` — `rm -rf .` → `rm -rf /`
- `translate/file.bv:38,48` — `.f` → `/f`, `.q` → `/q`, `.s` → `/s`
- `persistence.bv:12` — `"~/.config/officina"` → `getenv#("HOME") + "/.config/officina"`
- `persistence.bv:71-73` — add Result unwrapping before json_is_array
- `officina.bv` — add `before_exec` call to query/ensure/watch spawn paths

**8f: Remove generated artifacts**  
`officina.c`, `*.o`, `*.ll`, `*.bc`, `officina_bin`, `officina_noopt`, `officina_dbg`, `null.ll`

### Step 9 — String interpolation

**9a: Parser desugaring** (`src/parser.rs`)  
When parsing `"..."`, scan for `{...}` segments. Parse expressions inside `{}`. Build `Expr::Concat` chain.

**9b: `@` prefix** → skip interpolation, emit as literal `Expr::String`

**9c: Audit** — find existing `{` in string literals across `lib/std/` and benchmarks, prefix with `@`.

### Step 10 — Git commit + documentation

### Step 11 — Compile officina and verify it runs

---

## Files Changed Summary

| File | Changes | Est. lines |
|------|---------|------------|
| `src/backend/llvm/mod.rs` | String constants (B1), declares (B6) | 15 |
| `src/backend/llvm/emit_expr.rs` | TypedRegister::llvm(), string ops, native char, FFI marshaling | 200 |
| `src/backend/llvm/emit_stmt.rs` | Bool load/store native types, guard simplification | 80 |
| `lib/runtime/briv_rt.c` | Remove dead functions | 30 |
| `src/parser.rs` | String interpolation | 120 |
| `officina.bv` | Fix frgn→intrinsic, typos, running flag | 30 |
| officina `lib/std/*.bv` | Delete duplicates, fix syntax | 5 |
| officina `rules.bv`, `translate/file.bv`, `persistence.bv` | Logic bugfixes | 20 |
| `docs/architecture/features/string-concat.md` (NEW) | String operation documentation | 50 |

**Total: ~550 lines across ~10 files**

---

## Key LLVM IR Patterns

### Briv header layout (constant)
```llvm
@str.0 = private unnamed_addr constant <{ i64, i64, [5 x i8] }> <{
  i64 ptrtoint (i8* getelementptr inbounds (<{ i64, i64, [5 x i8] }>, <{ i64, i64, [5 x i8] }>* @str.0, i64 0, i32 2) to i64),
  i64 5,
  [5 x i8] c"hello\00"
}>, align 8
```

### Expr::String → i8* (string constant reference)
```llvm
%v = bitcast <{ i64, i64, [5 x i8] }>* @str.0 to i8*
```

### Inline concat on i8* (replace __str_concat)
```llvm
; %a and %b are i8* (Briv header pointers)
; Extract lengths from slot 1
%ha = bitcast i8* %a to i64*
%la_ptr = getelementptr i64, i64* %ha, i64 1
%la = load i64, i64* %la_ptr
%hb = bitcast i8* %b to i64*
%lb_ptr = getelementptr i64, i64* %hb, i64 1
%lb = load i64, i64* %lb_ptr

; Allocate result header: (total + 2) * 8 bytes
%total = add i64 %la, %lb
%slot_count = add i64 %total, 2
%alloc_size = mul i64 %slot_count, 8
%result = call i8* @malloc(i64 %alloc_size)

; Set slot 0 = data_ptr (result + 16)
%hp = bitcast i8* %result to i64*
%base = ptrtoint i8* %result to i64
%dp = add i64 %base, 16
store i64 %dp, i64* %hp

; Set slot 1 = total length
%len_slot = getelementptr i64, i64* %hp, i64 1
store i64 %total, i64* %len_slot

; Copy chars from a (slot 0 of input header = data_ptr)
%a_dp = load i64, i64* %ha
%a_chars = inttoptr i64 %a_dp to i8*
%dest_slot2 = getelementptr i64, i64* %hp, i64 2
%dest = bitcast i64* %dest_slot2 to i8*
call void @llvm.memcpy.p0i8.p0i8.i64(i8* %dest, i8* %a_chars, i64 %la, i1 false)

; Copy chars from b (offset by la)
%dest_off = getelementptr i8, i8* %dest, i64 %la
%b_dp = load i64, i64* %hb
%b_chars = inttoptr i64 %b_dp to i8*
call void @llvm.memcpy.p0i8.p0i8.i64(i8* %dest_off, i8* %b_chars, i64 %lb, i1 false)

; Return result header as i8*
%v = bitcast i8* %result to i8*  ; or just use %result directly
```

### FFI param: Briv string → C string
```llvm
; %str is i8* (Briv header pointer)
%hp = bitcast i8* %str to i64*
%data_ptr = load i64, i64* %hp, align 8
%cstr = inttoptr i64 %data_ptr to i8*
; pass %cstr to C function
```

### FFI return: C string → Briv string (inline)
```llvm
; %cstr is i8* (C string from C function)
%len = call i64 @strlen(i8* %cstr)
%slot_count = add i64 %len, 2
%alloc_size = mul i64 %slot_count, 8
%result = call i8* @malloc(i64 %alloc_size)

%hp = bitcast i8* %result to i64*
%base = ptrtoint i8* %result to i64
%dp = add i64 %base, 16
store i64 %dp, i64* %hp

%len_slot = getelementptr i64, i64* %hp, i64 1
store i64 %len, i64* %len_slot

%dest_slot2 = getelementptr i64, i64* %hp, i64 2
%dest = bitcast i64* %dest_slot2 to i8*
call void @llvm.memcpy.p0i8.p0i8.i64(i8* %dest, i8* %cstr, i64 %len, i1 false)

; %result is the Briv header pointer (i8*)
```

---

## Commits

1. **`feat: native LLVM types for strings (i8*), chars (i32), bools (i1/i8)`**  
   Compiler core: all emit_expr.rs, emit_stmt.rs, mod.rs changes + briv_rt.c cleanup  
   Test: `cargo test --lib` commit-verify

2. **`feat: string interpolation "hello {name}" desugaring`**  
   Parser changes for `{expr}` interpolation + `@"..."` anchor syntax  
   Test: `cargo test --lib`

3. **`fix: officina-cli migration to intrinsics + native types`**  
   All office BV file changes: frgn→intrinsic, stdlib deletion, logic bugfixes  
   Test: compile officina with `briv-compiler llvm`

4. **`docs: string concat + native type architecture`**  
   `docs/architecture/features/string-concat.md` with LLVM IR lowering docs  
   Update `docs/architecture/optimization-pipeline.md` if structure changed
