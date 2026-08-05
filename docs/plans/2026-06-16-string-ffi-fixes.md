# Fix: String Format Mismatch + FFI Marshaling

**Date:** 2026-06-16  
**Status:** Planned  
**Context:** Two bugs exposed while getting officina-cli to compile and run.

---

## Problem 1 — `__str_concat` uses C strings instead of Briv strings

`__str_concat` in `briv_rt.c` expects `const char*` (C strings). But the generated IR passes Briv string headers (2-slot format: data_ptr + length + characters) as `i64` values via `inttoptr`. `strlen` on a Briv header reads the header bytes as string data → SIGSEGV.

The `safe_cstr` hack masks the crash but produces empty strings and leaks memory.

**Fix:** Rewrite `__str_concat` to operate entirely in Briv string space:
- Signature: `int64_t __str_concat(int64_t a, int64_t b)`
- Validate inputs as Briv headers (data_ptr == header + 16, 0 < len < 1M)
- Allocate result header, copy characters from both inputs
- Return `ptrtoint` of result header

---

## Problem 2 — FFI marshaling doesn't convert between Briv and C strings

When a `frgn` takes `String` parameters or returns `String`, the generated IR passes/receives raw `i64` values through `inttoptr`/`ptrtoint`. This only works for string constants — computed strings crash.

**Fix:**
- **Inbound** (Briv → C): Extract `data_ptr` from slot 0 of the Briv header before passing to the C function.
- **Outbound** (C → Briv): Wrap the C `char*` return in a Briv header via `cstr_to_briv()` (already exists in `briv_rt.c`).

---

## Work Items

### Item 1 — Rewrite `__str_concat`

**File:** `lib/runtime/briv_rt.c`

Change signature from `char*(char*, char*)` to `int64_t(int64_t, int64_t)`. Operate on Briv headers:

```c
int64_t __str_concat(int64_t a, int64_t b) {
    int64_t* ha = (int64_t*)a;
    int64_t* hb = (int64_t*)b;
    int64_t la = 0, lb = 0;
    if (ha && (uintptr_t)ha > 65536 && ha[0] == a + 16 && ha[1] > 0 && ha[1] < 1000000) la = ha[1];
    if (hb && (uintptr_t)hb > 65536 && hb[0] == b + 16 && hb[1] > 0 && hb[1] < 1000000) lb = hb[1];
    int64_t* h = malloc((la + lb + 2) * sizeof(int64_t));
    if (!h) return 0;
    h[0] = (int64_t)(h + 2);
    h[1] = la + lb;
    for (int64_t i = 0; i < la; i++) h[i + 2] = ha[i + 2];
    for (int64_t i = 0; i < lb; i++) h[la + i + 2] = hb[i + 2];
    return (int64_t)h;
}
```

### Item 2 — Fix FFI arg marshaling for `String` params

**File:** `src/backend/llvm/emit_expr.rs` — `Expr::Call` handler (around line 285-310)

When `arg_ty == Type::String`, emit data pointer extraction before the C call:

```llvm
; raw = i64 Briv string header pointer
%hp = inttoptr i64 %raw to i64*       ; header pointer
%dp = load i64, i64* %hp, align 8     ; data_ptr from slot 0
%cstr = inttoptr i64 %dp to i8*        ; C string pointer
; pass %cstr to C function
```

### Item 3 — Fix FFI return marshaling for `String` results

**File:** `src/backend/llvm/emit_expr.rs` — `Expr::Call` handler, return processing

When return type is `String`, wrap the C `char*` result:
```llvm
; %call_result is i8*
%briv = call i64 @cstr_to_briv(i8* %call_result)
; %briv is the Briv string header ptr
```

### Item 4 — Add `cstr_to_briv` declare

**File:** `src/backend/llvm/mod.rs` — declare section

```rust
writeln!(out, "declare i64 @cstr_to_briv(i8*) #1").ok();
```

Also make `cstr_to_briv` in `briv_rt.c` non-static (remove `static`).

### Item 5 — Update `__str_concat` declare

**File:** `src/backend/llvm/mod.rs` — declare section

Change:
```rust
// OLD: declare i8* @__str_concat(i8*, i8*) #1
writeln!(out, "declare i64 @__str_concat(i64, i64) #1").ok();
```

### Item 6 — Update `Expr::Concat` and `emit_binop` emission

**File:** `src/backend/llvm/emit_expr.rs`

Two call sites. Remove `inttoptr`/`ptrtoint` round-trips:

```rust
// OLD:
// let ip = inttoptr i64 %a to i8*
// let jp = inttoptr i64 %b to i8*
// call i8* @__str_concat(i8* %ip, i8* %jp)
// %v = ptrtoint i8* %result to i64

// NEW:
// call i64 @__str_concat(i64 %a, i64 %b)
// %v = add i64 0, %result
```

### Item 7 — Cleanup `briv_rt.c`

- Remove `fprintf(stderr, "DEBUG __int_to_str...` from `__int_to_str`
- Remove `safe_cstr` function and `#include <unistd.h>`
- Remove `__resolve_briv` helper
- Make `cstr_to_briv` non-static (line 1272)

### Item 8 — End-to-end test

```bash
cd ~/Desktop/Projects/officina-cli
cargo build --release -p briv-compiler
./target/release/briv-compiler llvm officina.bv
clang -O3 officina.ll lib/runtime/briv_rt.c -lc -o officina_bin
timeout 10 ./officina_bin
```

---

## Files Changed

| File | What | Lines changed |
|------|------|---------------|
| `lib/runtime/briv_rt.c` | Rewrite `__str_concat`; remove debug/safe_cstr; make `cstr_to_briv` non-static | ~40 |
| `src/backend/llvm/emit_expr.rs` | FFI arg/return marshaling for String; `Expr::Concat`/`emit_binop` emission | ~30 |
| `src/backend/llvm/mod.rs` | Update `__str_concat` declare; add `cstr_to_briv` declare | ~3 |

**Total: 3 files, ~75 lines changed.**
