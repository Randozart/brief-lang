# String/FFI Cleanup — Remaining C Conversion Bugs + Projection Stubs

**Date:** 2026-06-18
**State:** 915 tests pass. All 4 previous sprints (A-D) committed.

---

## Sprint E1: 4 C Functions Casting `int64_t` → `const char*` Directly

These functions receive `int64_t` (ptrtoint of Brief string header) but cast to `const char*` without calling `brief_str_to_c`. The header bytes are interpreted as C string chars.

| Function | File:Line | Bug |
|----------|-----------|-----|
| `brief_getaddrinfo` | `brief_rt.c:1255` | `getaddrinfo((const char*)(uintptr_t)node, ...)` — hostname bytes from header |
| `brief_getenv` | `brief_rt.c:1270` | `getenv((const char*)(uintptr_t)name)` — reads env name from header |
| `brief_setenv` | `brief_rt.c:1276` | `setenv((const char*)(uintptr_t)name, ...)` — sets env with wrong name |
| `brief_unsetenv` | `brief_rt.c:1280` | `unsetenv((const char*)(uintptr_t)name)` — unsets wrong name |

**Fix pattern** (already proven by `brief_read_file`, `brief_write_file`):
```c
int64_t brief_getenv(int64_t name_bstr) {
    char* c_name = brief_str_to_c(name_bstr);
    if (!c_name) return 0;
    const char* val = getenv(c_name);
    free(c_name);
    return val ? (int64_t)val : 0;
}
```

### Additional: `__get_env_int` (line 61)
This function takes `const char* name` but is called from LLVM init code which passes a C string literal (e.g., "BOUND"). If it's ever called from Brief code, it would break. **Low risk** — called from LLVM only. Leave as-is.

---

## Sprint E2: 8+ frgn Functions Taking `const char*` Receiving Brief Headers

These functions are declared in `.bv` files as `frgn fn(msg: String) -> Bool` and are called from Brief code. The LLVM backend passes the Brief string header pointer (`i8*`), but the C function expects a C string (`const char*`). The header bytes (data pointer in little-endian) are garbage as a C string.

### Functions affected

```
brief_rt.c:128  int64_t __print(const char* msg)
brief_rt.c:695  int64_t __trim_left(const char* s)
brief_rt.c:702  int64_t __trim_right(const char* s)
brief_rt.c:716  int64_t __to_lower(const char* s)
brief_rt.c:728  int64_t __contains_at(const char* haystack, const char* needle, int64_t start)
brief_rt.c:734  int64_t __find_from(const char* s, const char* needle, int64_t start)
brief_rt.c:748  int64_t __splitn(const char* s, const char* delim, int64_t n_val)
brief_rt.c:778  int64_t __spawn_with_output(const char* cmd, int64_t args_val)
brief_rt.c:1309 int64_t substring(const char* s)
```

### Fix pattern

Change each function signature from `const char* s` to `int64_t s_bstr`, use `brief_str_to_c` at the top, and `free` after use. When a function takes multiple strings (`__contains_at`, `__find_from`), convert both.

**Caveat:** Some functions output to stdout or return results that are consumed by LLVM. The return types stay `int64_t` (boxed Brief strings).

**Note:** `__print` is superseded by the `Println` intrinsic (which correctly loads `hdr[0]` and calls `fprintf`). But `__print` is still used directly via `frgn` calls in user code — fix it anyway for symmetry.

---

## Sprint E3: Keys/Values/AsStack/AsQueue Projection Stubs

Current code at `emit_expr.rs:1994`:
```rust
ProjectionTarget::Keys | ProjectionTarget::Values
| ProjectionTarget::AsStack | ProjectionTarget::AsQueue => {
    writeln!(out, "{}{} = add i64 0, {} ; keys/values/as/as", indent, v, src_val.name).ok();
}
```

Returns the source value unchanged — wrong for all four.

| Target | Should do | Implementation |
|--------|-----------|----------------|
| `Keys` | Extract keys from HashMap → List | C helper `brief_map_keys(i64)` — iterate key-value pairs, collect keys into new list |
| `Values` | Extract values from HashMap → List | C helper `brief_map_values(i64)` — iterate key-value pairs, collect values into new list |
| `AsStack` | Reinterpret List as Stack | Identity — a list IS a stack. Return source unchanged (this is correct!) |
| `AsQueue` | Reinterpret List as Queue | Identity — a list IS a queue. Return source unchanged (correct!) |

So only `Keys` and `Values` actually need new C helpers. `AsStack` and `AsQueue` are already correct (list header is compatible with stack/queue).

---

## Sprint E4: ProjectionTarget::Match Stub

`ProjectionTarget::Match(expr)` falls through to the catch-all (returns 0). The interpreter evaluates the match expression against the source value and returns a boolean (whether they match).

**Fix:** Compare source value with match expression, return `Bool`. Same pattern as `Contains` but with a simple equality check instead of a linear search loop.

---

## Execution Order

1. **E1** — Fix getaddrinfo, getenv, setenv, unsetenv (C only, 4 functions, ~20 min)
2. **E2** — Fix 8+ frgn functions (C only, signature + brief_str_to_c conversion, ~30 min)
3. **E3** — Add `brief_map_keys`/`brief_map_values` C helpers + LLVM IR matches (~20 min)
4. **E4** — Add `Match` projection match arm in emit_expr.rs (~10 min)
5. **Build + Test + Commit**

Each step: `cargo build && cargo test --lib` must pass before moving to next.
