# Fix: briv_read_file FFI marshaling mismatch (C string vs Briv header)

**Date**: 2026-06-14
**Author**: OpenCode (diagnosis from officina-cli agent)
**Bug**: briv_read_file crashes with corrupt malloc call (size = 0x6E6174737265646F)

## Root Cause

The LLVM backend marshals `String` as `i8*` (C string pointer) when crossing the
FFI boundary. This is documented at `briv_rt.c:376`:

> The LLVM backend marshals String as i8*, Int as i64, Bool as i64.

Functions like `__print(const char* msg)` correctly follow this convention.
However, `briv_read_file` was written to accept a **Briv internal header pointer**
(the old format: `[data_ptr, len, chars...]` stored as `int64_t[]`):

```c
// Old (broken) convention:
int64_t briv_read_file(int64_t path_ptr) {
    int64_t* path_str = (int64_t*)path_ptr;
    int64_t path_len = path_str[1];  // reads bytes 8-15 of C string chars!
```

Since the LLVM backend passes the C string pointer directly (without the `inttoptr`
marshaling that frgn calls use), the function reads raw character bytes as if they
were header fields. For the path `"system/understands.dbv"`, bytes 8-15 decode to
`"oderstan"` (size 0x6E6174737265646F), causing a multi-GB malloc → ENOMEM → hang.

## Fix Summary

Two files changed:

### 1. `lib/runtime/briv_rt.c` — Rewrite `briv_read_file` to C string convention

```c
char* briv_read_file(const char* path) {
    if (!path) return NULL;
    FILE* fp = fopen(path, "rb");
    if (!fp) return NULL;
    fseek(fp, 0, SEEK_END);
    long file_size = ftell(fp);
    fseek(fp, 0, SEEK_SET);
    if (file_size <= 0) { fclose(fp); return NULL; }
    char* data = malloc((size_t)file_size + 1);
    if (!data) { fclose(fp); return NULL; }
    size_t n = fread(data, 1, (size_t)file_size, fp);
    fclose(fp);
    if (n == 0) { free(data); return NULL; }
    data[n] = '\0';
    return data;
}
```

### 2. `src/backend/llvm/emit_expr.rs` — Add inttoptr/ptrtoint marshaling

The `Intrinsic::ReadFile` intrinsic was passing the raw `i64` SSA register without
the `inttoptr` cast that frgn calls use (line 270). Fixed by adding explicit
pointer marshaling:

```llvm
%fp = inttoptr i64 %path_val to i8*
%raw = call i64 @briv_read_file(i8* %fp)
%result = ptrtoint i8* %raw to i64   ; back to i64 for SSA
```

### 3. `src/backend/llvm/emit_toplevel.rs` — Update declaration

Changed `declare i64 @briv_read_file(i64)` to `declare ptr @briv_read_file(ptr)`
so LLVM's optimizer sees the correct type for alias analysis. The call site now
uses explicit `inttoptr`/`ptrtoint` casts, so the declaration is purely for
LLVM's benefit.

## Safety

- **Interpreter**: Unaffected — handles `ReadFile` entirely in Rust
  (`std::fs::read_to_string`, `src/interpreter.rs:1586`)
- **Other intrinsics**: Unaffected — only `ReadFile` had custom C marshaling
- **Return value**: `ptrtoint` back to `i64` preserves the downstream usage pattern
  (all SSA values are `i64`)
- **Declaration change**: Net-positive — `ptr` enables LLVM alias analysis

## Test

After fix, `read_file#("system/understands.dbv")` in a briv program compiled with
the LLVM backend should correctly read the file instead of crashing with ENOMEM.
