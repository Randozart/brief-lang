# Intrinsic C Function Convention

**Established 2026-06-18**

## Convention

C functions that serve as compiler-internal implementations of Briv
intrinsics follow the `__name__` naming convention (double underscore
both sides). This distinguishes them from:

| Convention | Meaning | Example |
|---|---|---|
| `name` | Briv intrinsic call syntax | `print#(msg)` |
| `__name__` | C implementation of an intrinsic | `int64_t __print__(const char* msg)` |
| `__name` | **DEPRECATED** — old frgn convention | `frgn __print(...)` |
| `briv_name` | **DEPRECATED** — old intrinsic C convention | `briv_read_file(...)` |

## Rules

1. Every `__name__` C function corresponds to a Briv intrinsic callable
   via `name#(args)` syntax.
2. String parameters use `const char*` — the LLVM intrinsic handler loads
   `hdr[0]` (the data pointer) from the Briv string struct before calling.
   This is an `i8*` pointing to the tight-packed char data.
3. Non-string parameters (`Int`, `Bool`, `Float`) keep their native C
   types (`int64_t`, `int64_t` boxed, `float`).
4. Return values are `int64_t` (a Briv-packed value: pointer for strings,
   raw integer for ints, packed enum for Result/Option).
5. `__name__` functions are declared in `emit_toplevel.rs` declares
   section and called from `emit_expr.rs` intrinsic handlers.

## Intrinsic handler pattern

In `emit_expr.rs`, the intrinsic handler for a string-accepting intrinsic
loads `hdr[0]` to pass a `const char*`:

```llvm
%str = inttoptr i64 %str_boxed to ptr
%hdr0 = load i64, ptr %str, align 8
%chars = inttoptr i64 %hdr0 to ptr
%result = call i64 @__print__(ptr %chars)
```

For non-string intrinsics (e.g. `int_to_str#`), the value is passed
directly:

```llvm
%result = call i64 @__int_to_str__(i64 %n)
```

## Currently converted intrinsics

| Intrinsic | C function | Params |
|---|---|---|
| `print#` | `__print__` | `const char*` |
| `println#` | (LLVM direct: `fprintf` + newline) | — |
| `print_int#` | (LLVM direct: `fprintf`) | — |
| `print_float#` | (LLVM direct: `fprintf`) | — |
| `trim_left#` | `__trim_left__` | `const char*` |
| `trim_right#` | `__trim_right__` | `const char*` |
| `to_lower#` | `__to_lower__` | `const char*` |
| `contains_at#` | `__contains_at__` | `const char*`, `const char*`, `int64_t` |
| `find_from#` | `__find_from__` | `const char*`, `const char*`, `int64_t` |
| `splitn#` | `__splitn__` | `const char*`, `const char*`, `int64_t` |
| `spawn_with_output#` | `__spawn_with_output__` | `const char*` |
| `read_file#` | `__read_file__` | `int64_t` |
| `write_file#` | `__write_file__` | `int64_t`, `int64_t` |
| `readln#` | `__readln__` | — |
| `sort#` | `__sort_list__` | `int64_t` |
| `reverse#` | `__reverse_list__` | `int64_t` |
| `range#` | `__range__` | `int64_t` |
| `int_to_str#` | `__int_to_str__` | `int64_t` |
