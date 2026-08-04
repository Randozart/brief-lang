# Brief FFI & Export — A Practical Guide

**Date:** 2026-08-03
**Applies to:** the `glue-host-callable` work, merged into `main` at `ff108698`.

Brief compiles to a native library that any language can call at near-native
speed — C, Rust (within ~2.4% of native), Python (at ctypes parity, and see
the native-extension note at the end). The FFI is **protocol-driven**: Brief
has no type layouts, only adaptive protocols. A boundary representation is a
sub-protocol (`#String<C_String>`), a `proto` declaration supplies the
transforms, and the casting graph finds the minimal path between a Brief type
and its boundary representation — emitting the **delta**, not a chain.

---

## 1. The mental model

- **The export signature IS the boundary contract.** Declare a function with
  `export defn` and boundary types, and the generated ABI (widths, pointers)
  derives from the protocol graph — no per-language conversion tables.
- **Boundary types are declared in Brief** (`lib/glue/c.bv`): `CStr`,
  `CFloat`, `CDouble`, `CI64`, `CI32`, `CBool`, `CChar`, `CPtr`.
- **Marshalling is ordinary Brief casting.** `name as String` on a `CStr`
  emits the graph's binding call (`cstr_to_brief`); `s as CStr` emits
  `str_to_c`. `+` concatenates strings; `CStr + CStr` uses the variant's own
  `cstring_concat`.
- **The compiler knows no type names and no language.** Only protocol
  categories and metadata are hardcoded; every language's vocabulary lives in
  the Data Brief config (`config/glue.dbvl`).

## 2. Quick start

`examples/glue-host/boundary.bv` is the running example:

```brief
import "glue/c.bv";

export defn echo(name: CStr) -> CStr {
    term name;
};

export defn greet(name: CStr) -> CStr {
    let s: String = name as String;   // CStr → String via cstr_to_brief
    term s as CStr;                    // String → CStr via str_to_c
};

export defn join(a: CStr, b: CStr) -> CStr {
    term a + b;                        // the C_String variant's cstring_concat
};

export defn identity(x: CDouble) -> CDouble {
    term x;                            // CDouble → double ABI
};
```

Build a C-callable static library **and** a PIC shared library:

```bash
briefc build examples/glue-host/boundary.bv --library --out build/
#   → libboundary.a  (gcc/rustc-linkable, real ELF, -O3)
#   → boundary.so    (clang -O3 -flto, for ctypes/ffi-napi)
```

Generate C bindings (a header that declares the exports + the `BriefState`
lifecycle):

```bash
briefc bindings examples/glue-host/boundary.bv c --out build/
#   → build/boundary-bindings/brief_types.h
```

The header resolves the boundary types to C ABI names (`CStr → int64_t`,
`CDouble → double`) and declares:

```c
typedef struct BriefState BriefState;
extern BriefState* __brief_init_state(void);
extern void __glue_release(BriefState* state);
extern void __brief_set_cancel(BriefState* state, int32_t flag);
extern void __brief_clear_cancel(BriefState* state);

int64_t echo(int64_t name);
int64_t greet(int64_t name);
int64_t join(int64_t a, int64_t b);
double identity(double x);
```

## 3. Calling from C

```c
#include "boundary-bindings/brief_types.h"
#include <stdio.h>

int main(void) {
    BriefState* st = __brief_init_state();
    printf("%s\n", (char*)(uintptr_t)echo((int64_t)"hello"));   // hello
    printf("%s\n", (char*)(uintptr_t)greet((int64_t)"hello"));  // hello
    printf("%s\n", (char*)(uintptr_t)join((int64_t)"foo", (int64_t)"bar")); // foobar
    printf("%f\n", identity(3.14));                              // 3.140000
    __glue_release(st);
    return 0;
}
```

```bash
cc -o driver driver.c -I build/boundary-bindings -L build/ -lboundary
```

## 4. Calling from Rust

`examples/glue-host/rust-host/` is a self-contained crate whose `build.rs`
compiles the Brief library with `briefc` and links it:

```bash
cd examples/glue-host/rust-host
BRIEFC=$PWD/../../../target/release/briefc cargo run --release
```

The generated `brief_bindings.rs` exposes plain `extern "C"` functions. The
boundary is a single C-ABI call; measured `feature_hash` runs **within 2.4% of
native Rust** — this is the path for writing compiler-internal components in
Brief without loss of efficiency.

## 5. Calling from Python (ctypes)

```python
import ctypes
lib = ctypes.CDLL("build/boundary.so")
lib.__brief_init_state.restype = ctypes.c_void_p
state = lib.__brief_init_state()
lib.greet.argtypes = [ctypes.c_void_p, ctypes.c_int64]
lib.greet.restype = ctypes.c_int64
print(ctypes.cast(lib.greet(state, ctypes.c_void_p(b"hello").value), ctypes.c_char_p).value.decode())
```

The ~2µs/call is ctypes marshalling (identical for C through ctypes) — Brief is
within 5% of C through Python. A **native Python C-extension** target (no
ctypes) is in development for ~10× lower per-call overhead (see §9).

## 6. Callbacks (host → Brief → host)

A host can pass a function pointer into Brief; Brief calls it back for
first-level-primitive updates (progress bars, per-item status):

```brief
export defn apply(cb: fn(Int) -> Int, x: Int) -> Int {
    term CallPtr#(cb, x);        // call through the pointer
};
```

```c
int64_t doubler(int64_t x) { return x * 2; }
apply(doubler, 21);              // → 42
```

The generated header declares the parameter as a C function pointer
(`int64_t (*cb)(int64_t)`). `fn(P) -> R` annotations are the boundary contract
for callbacks.

## 7. Cancellation

A host thread can cancel a long-running Brief call:

```brief
txn sum_loop(acc: Int, i: Int, count: Int)
    [i < count && !CancelRequested#()][i == count] -> Int
{
    let na: Int = acc + (i * 3);
    acc = na;
    i = i + 1;
    term acc;
};

export defn cancellable_sum(count: Int) -> Int {
    term sum_loop(0, 0, count);
};
```

```c
pthread_t t;
pthread_create(&t, NULL, canceller, NULL);   // canceller calls __brief_set_cancel(st, 1)
int64_t partial = cancellable_sum(st, 2000000000);   // stops early
```

Polling is **explicit** (`CancelRequested#()` in the loop precondition) — the
compiler never injects checks.

## 8. Extending: adding a language

The FFI is infinitely extensible through `config/glue.dbvl` (Data Brief). A
language target is a section: protocols (category → native/c-ABI names), state
representation, parameter-declaration formats, and wrapper/binding templates.
Boundary representations are `proto` declarations in `.bv` files — the compiler
teaches, the config/stdlib learns.

## 9. Performance notes

| path | per-call (feature_hash, count=1000) |
|------|-------------------------------------|
| Rust → Brief | 1127 ns (native Rust 1101 — **2.4%**) |
| C → Brief (.a) | 1092 ns (native C 1082 — **1%**) |
| Python → Brief (ctypes) | 2033 ns (Python → C 1927 — **5%**) |

- The `.a` path runs `opt -passes='default<O3>'` before llc so the emitted loop
  is fully SSA (a plain `llc -O3` in LLVM 18.1.3 did not SROA the transaction
  loop's allocas).
- The boundary is a single C-ABI call (~26 ns/call); the work dominates.
- Native Python C-extension target: **in development** — see the plan
  `docs/plans/2026-08-03-native-python-extension.md`.
