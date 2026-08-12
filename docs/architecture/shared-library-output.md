# Shared Library Output (`--shared`)

**Date:** 2026-07-19
**Status:** Active

## Overview

The `--shared` flag produces a position-independent shared library (`.so`)
instead of an executable. The library exposes `export`-annotated functions
as C-callable entry points with a stable ABI.

## Build Flow

```
brievc build --shared component.bv --out build/
  → component.ll (LLVM IR)
  → clang -O3 -shared -fPIC component.ll lib/runtime/briev_rt.c -o component.so -lm
  → component.so
```

The compiler:
1. Parses `export` keyword → `TopLevel::Export` wrapper
2. Emits each definition with `dso_local` visibility (public symbol)
3. Skips `main()` / reactor loop / barrier sync (no runtime entry needed)
4. Emits `__briev_init` / `__briev_fini` via `llvm.global_ctors`
5. Links `briev_rt.c` for runtime functions (malloc, syscall wrappers, etc.)

## Export Annotation

```briev
export defn add(a: Int, b: Int) -> Int {
    term a + b;
};

export node process [x < TOTAL][x == TOTAL] {
    x = x + 1;
    term;
};
```

- `export defn` — C-callable function wrapper. Takes `ptr %state` as first arg.
- `export node` — Reactive convergence entry point. Runs all exported
  reactive txns to convergence, then returns to the host.

## C ABI

| Briev signature | C signature |
|----------------|-------------|
| `defn f(a: Int, b: Int) -> Int` | `int64_t f(void* state, int64_t a, int64_t b)` |
| `defn f(a: Float) -> Float` | `double f(void* state, double a)` |
| `defn f(a: Bool) -> Bool` | `int8_t f(void* state, int8_t a)` |
| `defn f(a: Ptr<Byte>) -> Int` | `int64_t f(void* state, void* a)` |

The `state` pointer is an opaque `%State` struct. The host must allocate
it (minimum 8 bytes, zero-initialized) or use `__briev_init_state` to
initialize it properly.

## Example

```c
#include <dlfcn.h>
#include <stdio.h>
#include <stdint.h>

int main() {
    void *lib = dlopen("./component.so", RTLD_NOW);
    if (!lib) { /* error */ }

    // Functions are resolved by their original name
    int64_t (*add)(void*, int64_t, int64_t) = dlsym(lib, "add");
    if (!add) { /* error */ }

    // Allocate a minimal state struct (8 bytes, zeroed)
    void *state = __builtin_alloca(8);
    *(int64_t*)state = 0;

    int64_t result = add(state, 40, 2);
    printf("result = %ld\n", result);  // → 42

    dlclose(lib);
    return 0;
}
```

## Reactive Entry Points

`export node` compiles to a `run_reactive()` function that runs all
exported reactive transactions to convergence. The halting proof is
established at compile time via the transition graph analysis
(`bounded_pre` + `increments` checks). If any reactive txn lacks a
provably bounded precondition, the compiler emits a diagnostic.

## Limitations

- Reactive transactions with unbounded preconditions are rejected
- No `main()` / reactor loop — only exported functions are callable
- The `state` pointer ABI is a placeholder — future work will provide
  a state-less wrapper that allocates %State internally
- Multiple `.so` files loaded into the same process may have symbol
  conflicts (symbol prefixing TBD)

## Files

| File | Role |
|------|------|
| `src/main.rs` | `--shared` CLI flag |
| `src/compile.rs` | `BuildOptions.shared`, `-shared -fPIC` in `compile_ll_to_binary` |
| `src/backend/llvm/mod.rs` | `self.ctx.is_shared_lib` check, skip main loop |
| `src/backend/llvm/emit_toplevel.rs` | `emit_shared_lib_exports`, `dso_local` visibility |
| `src/parser/definitions.rs` | `Token::Export` parsing |
