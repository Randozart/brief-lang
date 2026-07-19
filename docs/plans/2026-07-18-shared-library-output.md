# Shared Library Output (`--shared`)

**Status:** Design
**Test baseline:** `cargo test --lib` = 931 tests passing
**Estimated effort:** ~10 hours
**Depends on:** Block 15 (arena allocator), A7 (thread-safe arena CAS)

---

## Goal

Add a `--shared` flag to the `build` command that produces a position-independent
shared library (`.so` on Linux, `.dylib` on macOS) instead of an executable binary.
The library exposes functions annotated with `export` as C-callable entry points
with a stable ABI, no GC dependency, and contract-proven memory safety.

## Architecture

```
┌─ Brief source ───────────────────────────────────────────────┐
│                                                               │
│  export defn process(input: Ptr<Byte>, len: Int) -> Int {     │
│      [input != null][result >= 0]                              │
│      // contract-proven, arena-allocated, no heap              │
│  };                                                            │
│                                                               │
└───────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─ briefc build --shared ──────────────────────────────────────┐
│                                                               │
│  1. Parse export annotations                                   │
│  2. Generate wrapper per exported function:                    │
│     • Stack-allocate a transient arena                         │
│     • Derive %State struct on stack or TLS                     │
│     • Call internal Brief body                                 │
│     • Return result via C ABI                                  │
│     • Teardown arena                                           │
│  3. Skip main loop, reactor tick, runtime init                 │
│  4. Emit __brief_init / __brief_fini (constructor/destructor)  │
│  5. clang -shared -fPIC -o component.so                       │
│                                                               │
└───────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─ C host ─────────────────────────────────────────────────────┐
│                                                               │
│  void *lib = dlopen("./component.so", RTLD_NOW);              │
│  int64_t (*fn)(int64_t, int64_t) = dlsym(lib, "process");     │
│  int64_t result = fn(input, len); // thread-safe per call      │
│  dlclose(lib);                                                 │
│                                                               │
└───────────────────────────────────────────────────────────────┘
```

## Phase breakdown

---

### L1: Build flag + output path

**Files:** `src/main.rs`, `src/compile.rs`

**CLI:**
```
briefc build --shared example.bv --out component.so
```

**Changes:**

`src/main.rs` — add `--shared` flag:
```rust
if matches.subcommand_matches("build").map_or(false, |m| m.get_flag("shared")) {
    opts.shared = true;
}
```

`src/compile.rs` — `BuildOptions.shared: bool` (default `false`).

`compile_ll_to_binary()` — when `opts.shared`:
```rust
let args = vec![
    "-shared".to_string(),
    "-fPIC".to_string(),
    "-o".to_string(), out_path.to_string_lossy().to_string(),
    ll_path.to_string_lossy().to_string(),
    "-lm".to_string(),
];
// No brief_rt.o linked — no runtime threads, no barrier sync.
// No main() expected.
```

**Rollback:** Remove `--shared` flag, revert `compile_ll_to_binary`.

---

### L2: Export annotation

**Files:** `src/ast/top.rs`, `src/parser/definitions.rs`, `src/backend/llvm/mod.rs`

**Parser:** Accept optional `export` keyword before `defn`:
```rust
// src/parser/definitions.rs
fn parse_definition(&mut self) -> Result<TopLevel, SyntaxError> {
    let is_export = self.eat(&Token::Export);
    // ... existing defn parsing ...
    Ok(TopLevel::Definition(Definition { is_export, .. }))
}
```

**AST:** Add `pub is_export: bool` to `Definition` struct.

**Backend registration:** In `LlvmBackend::generate()`, collect exported function names:
```rust
pub(crate) exported_fn_names: Vec<String>,
```
Populated during the topology pass:
```rust
if let TopLevel::Definition(d) = item {
    if d.is_export { self.exported_fn_names.push(d.name.clone()); }
}
```

**Rollback:** Remove `export` keyword, revert `Definition.is_export`.

---

### L3: Wrapper generation (C-callable ABI)

**Files:** `src/backend/llvm/emit_toplevel.rs`

For each exported function `defn f(a: Int, b: Int) -> Int`:

1. Emit the normal Brief function body (internal name: `brief_impl_f`)
2. Emit a C-callable wrapper `f`:

```llvm
define i64 @f(i64 %arg0, i64 %arg1) {
entry:
  ; Per-call stack arena (256KB — can be tuned)
  %arena = alloca i8, i64 262144
  %arena_end = getelementptr i8, ptr %arena, i64 262144
  store ptr %arena, ptr @__brief_arena_ptr
  store ptr %arena_end, ptr @__brief_arena_end
  store ptr %arena, ptr @__brief_arena_base

  ; Set up a minimal %State — just the arena pointers
  %state = alloca %State, align 8
  %ap = getelementptr %State, ptr %state, i32 0, i32 0  ; arena ptr slot
  store ptr %arena, ptr %ap

  ; Call the real implementation
  %result = call i64 @brief_impl_f(ptr %state, i64 %arg0, i64 %arg1)

  ; Arena reset (not free — memory is reused on next call from same thread)
  store ptr %arena, ptr @__brief_arena_ptr
  ret i64 %result
}
```

**ABI mapping:**

| Brief type | C type | LLVM type |
|-----------|--------|-----------|
| `Int` | `int64_t` | `i64` |
| `Float` | `double` | `double` |
| `Bool` | `int8_t` | `i8` |
| `Ptr<Byte>` | `void*` | `ptr` |
| `Void` | `void` | `void` |
| `String` | `struct { int64_t data; int64_t len; }` | `{ i64, i64 }` (SSO handle) |

**Multiple return values** — `defn f(a: Int) -> (Int, Bool)` — use out-parameters:
```c
void f(int64_t a, int64_t *out0, int8_t *out1);
```

**Struct returns** — `defn f() -> MyStruct` — return by value if ≤16 bytes, else by out-pointer.

**Stateful exports** — `export defn init()` and `export defn run()` share the same persistent arena
(via the module's `__brief_arena_*` globals, not per-call stack arena).

**Rollback:** Remove wrapper emission from `emit_toplevel.rs`.

---

### L4: Skip main loop for shared libs

**Files:** `src/backend/llvm/mod.rs`, `src/backend/llvm/context.rs`

Add `is_shared_lib: bool` to `CompilerContext`. When true:

- `emit_main()` / `emit_ssa_main()` / `emit_reactor()` are NOT called
- No `@main()` function emitted
- No `ss_main_loop`, no reactive transaction dispatch
- No `brief_rt.c` functions linked (no thread pool, no barrier)

Instead, emit module constructors/destructors:

```llvm
; Module constructor — called automatically when .so is loaded
define void @__brief_init() __attribute__((constructor)) {
  ; Zero-initialize arena globals, or allocate TLS keys
  ret void
}

; Module destructor — called when .so is unloaded
define void @__brief_fini() __attribute__((destructor)) {
  ; Free persistent arena, destroy TLS keys
  ret void
}
```

**Reactive transactions are NOT supported in `--shared` mode** — they require a
main loop and barrier synchronization. The compiler emits a diagnostic:
```
error: --shared mode does not support reactive transactions
```

**Rollback:** Remove `is_shared_lib` checks, restore main loop emission.

---

### L5: Per-call arena safety

**Files:** `src/backend/llvm/mod.rs` (A7 infrastructure)

The per-call stack arena (L3) avoids threading issues because each call gets its
own stack frame. However:

1. **Stack overflow** — 256KB default per call. If the function needs more,
   the compiler emits a runtime fallback to `malloc` (same as A3). The threshold
   can be tuned via `--stack-threshold`.

2. **Cross-call persistence** — If an exported function returns a `Ptr<Byte>`,
   the allocation must outlive the call. This uses a **persistent arena** stored
   in a module-level global, not the per-call stack arena. The persistent arena
   is grown as needed (with CAS-based thread safety from A7 if called from
   multiple threads simultaneously).

3. **Thread safety** — The per-call stack arena is inherently thread-safe
   (each thread has its own stack). The persistent arena uses the existing
   CAS + mutex infrastructure from A7. The arena reset between calls is:
   ```llvm
   store ptr %arena_base, ptr @__brief_arena_ptr
   ```
   This is a plain store on the per-call arena (stack-local, no race).
   For the persistent arena, use `atomicrmw` if shared.

**Rollback:** No infrastructure changes needed — reuses existing A7 code.

---

### L6: Linking and deployment

**Files:** None (build script changes only)

Generated `.so` is linked with:
```
clang -shared -fPIC -o component.so component.ll -lm
```

No `brief_rt.c` linked. The `.so` has zero external runtime dependencies
beyond libc and libm.

To make this work on macOS (`.dylib`), detect the platform in `compile.rs`:
```rust
let ext = if cfg!(target_os = "macos") { ".dylib" } else { ".so" };
let shared_flag = if cfg!(target_os = "macos") { "-dynamiclib" } else { "-shared" };
```

**Verification:** A C test program:
```c
#include <dlfcn.h>
#include <stdio.h>
#include <stdint.h>

int main() {
    void *lib = dlopen("./component.so", RTLD_NOW);
    int64_t (*fn)(int64_t) = dlsym(lib, "process");
    int64_t result = fn(42);
    printf("result = %ld\n", result);
    dlclose(lib);
    return 0;
}
```

Compiled with:
```
cc -o test_host test_host.c -ldl
./test_host
```

**Rollback:** None — this is purely additive.

---

## Files modified

| File | Phase | Change |
|------|-------|--------|
| `src/main.rs` | L1 | `--shared` CLI flag |
| `src/compile.rs` | L1 | `shared: bool` on `BuildOptions`, `-shared -fPIC` in `compile_ll_to_binary` |
| `src/ast/top.rs` | L2 | `is_export: bool` on `Definition` |
| `src/parser/definitions.rs` | L2 | `export` keyword parsing |
| `src/backend/llvm/mod.rs` | L2, L4 | `exported_fn_names`, `is_shared_lib`, skip main loop |
| `src/backend/llvm/emit_toplevel.rs` | L3 | `emit_export_wrapper` for C ABI wrappers |
| `src/backend/llvm/context.rs` | L4 | `is_shared_lib: bool` on `CompilerContext` |
| `src/backend/llvm/emit_toplevel.rs` | L4 | `__brief_init` / `__brief_fini` constructors |

---

## Open questions

1. **Stateful vs stateless exports** — Should each call be fully isolated (own arena,
   own transient state), or should repeated calls to the same function share state?
   Recommendation: stateless by default (`export defn`), stateful with explicit
   annotation (`export persistent defn`).

2. **Error reporting across FFI boundary** — If a contract is violated inside
   the `.so`, what happens? Panic/abort? Or return an error code? Recommendation:
   abort by default (fail-stop, no undefined behavior), with optional error
   return via `export defn ... -> Result<Int, ErrorCode>`.

3. **TLS arena optimization** — The per-call stack arena (256KB) wastes memory
   for tiny functions. A TLS arena (one per thread, reused across calls) is
   more efficient. Should this be the default, or opt-in? Recommendation: stack
   arena for safety (no TLS complexity), TLS as follow-up optimization.

4. **Multiple `.so` loading** — If two Brief `.so` files are loaded into the
   same process, their arena globals might conflict (symbol interposition).
   Recommendation: prefix all exported symbols and globals with a module hash.
