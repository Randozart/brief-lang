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
| Python → Brief (ctypes) | 3057 ns (200k-call bench) |
| Python → Brief (native ext) | **1297 ns** (**2.4×** vs ctypes) |
| Python → Brief `add` (ctypes) | 1058 ns |
| Python → Brief `add` (native ext) | **179 ns** (**6×** — native CPython speed) |

- The `.a` path runs `opt -passes='default<O3>'` before llc so the emitted loop
  is fully SSA (a plain `llc -O3` in LLVM 18.1.3 did not SROA the transaction
  loop's allocas).
- The boundary is a single C-ABI call (~26 ns/call); the work dominates.
- The native extension's pure-call overhead is ~179 ns — the Python method
  dispatch + per-category parse/build in the generated shim. The compute-heavy
  case sits at the native compute floor (feature_hash's FNV-1a loop is ~1080 ns
  of real work; the Python method adds only ~217 ns).

## 10. Native Python extension (no ctypes)

`brief extension <bridge.bv> python` generates a CPython C-extension module
that calls the Brief exports directly — no ctypes marshalling layer:

```
$ brief extension rank.bv python --out build/
  Extension: build/rank.cpython-312-x86_64-linux-gnu.so
$ python3 -c "import rank; print(rank.feature_hash(1000, 42))"
```

- The shim is a single `.c`: a `PyInit_<bridge>` module with one method per
  export. Per-category parse/build snippets (in `config/glue.dbvl`, the python
  target's `native.*` templates) marshal natively — Python `int`/`float`/`str`
  in, native Python values out. String params use `PyUnicode_AsUTF8AndSize`
  (limited API ≥ 3.10); `#String` handles are the CStr/Brief pointer.
- The `CStr <-> String` meld (`lib/glue/c.bv`) makes boundary functions
  cast-free: `let s: String = name;` needs no `as`, and the marshalling inserts
  `cstr_to_brief`/`str_to_c` (zero-copy in the String → CStr direction — a
  Brief String's data region IS a nul-terminated C string).
- Adding another language is a config section (templates + protocol mappings)
  — the compiler renders, it never hardcodes a language.
- The composite ABI contract: a String/Data composite crosses as an i64 handle;
  every shim dereferences it to a state-owned `(ptr, len)` region (NUL
  invariant) valid for the state's life; hosts borrow read-only; mutability is
  declared by the meld, never per language. See
  `docs/architecture/casting-protocol.md` and the plan
  `docs/plans/2026-08-03-native-python-meld-composite.md`.

## 11. Per-language glue folders (referenced as config)

Each language's entire interop definition lives in `lib/glue/<lang>/` — a
`glue.dbvl` config file (protocols, ABI, templates, **toolchain recipe**),
`types.bv` (boundary declarations), and an optional `gen.bv` compile-time
plugin escape hatch. `brief export|bindings|extension <bridge.bv> <lang>`
resolves `lib/glue/<lang>/` BY NAME and loads its config; `load_glue_config`
scans the folders for extension routing. The compiler carries zero language
knowledge — the glue folder is data.

The toolchain recipe lives in `glue.dbvl`: `native_include_cmd`,
`native_suffix`/`native_suffix_cmd`, `native_link_cmd`, `native_cc`. The
compiler only does "compile C, link a shared library"; python's
`python3-config` and node's include discovery are config commands.

## 12. Native Node addon + the Python ↔ Node bridge

`brief extension <bridge.bv> node` generates a NAPI `.node` addon (no npm) —
same generic renderer as the Python shim, node's `native.*` templates in
`lib/glue/node/glue.dbvl`:

```
$ brief extension node_bridge.bv node --out build/
  Extension: build/node_bridge.node
$ node -e "const b = require('./node_bridge.node'); console.log(b.save('hi'))"
```

Python and Node have no native binding between them; Brief's composite is their
only common interface. The cross-language test (`tests/c_driver_node.rs`)
proves both directions: Node persists `"hello from node"` via the bridge's
`persist` (runtime file I/O), Python loads it with `load`; then Python
persists `"hello from python"` and Node loads it. Stateful exports (a String
state field read/written across calls) work — several latent backend bugs were
fixed along the way (see `BUGS.md`): the library `__brief_init_state` now
returns a module-global state instead of a dangling stack pointer, state-field
references from exports are no longer eliminated as dead, and stateful exports
keep their `%state` param.

Two shim-level correctness notes: every export is declared in the shim as
`__brief_export_<name>` with an `asm("<name>")` label (a bridge export named
like a libc function — `read`, `open` — would otherwise collide with the host's
prototype at compile time and be PLT-interposed at runtime); and the link adds
`-Wl,-Bsymbolic-functions` so the addon binds its own symbols.

## 13. The gen.bv plugin escape hatch (designed)

If `lib/glue/<lang>/gen.bv` exists, the command can invoke it instead of the
renderer: the pipeline writes a `bridge.dbvl` contract next to the output, the
plugin reads it via `FileRead$`/`ConfigGet$`, generates files via `FileWrite$`,
and runs the toolchain via `ShellCmd$`. Turing-complete generation for anything
the templates can't express. Staged after the config-driven path is proven.

## 14. Shipped language roster (2026-08-04)

Each language is a `lib/glue/<lang>/` folder — config, templates, toolchain
recipe. Zero compiler knowledge; `brief bindings` / `brief export` /
`brief extension` render through the generic pipeline.

| Language | Flavor | Command | Notes |
|----------|--------|---------|-------|
| C | bindings | `brief bindings <b> c` | header, C/C++-compatible (`extern "C"`) |
| C++ | bindings | same C header | g++ round-trip test |
| Rust | bindings | `brief export <b> rust` | cgo-free crate |
| Python | native ext | `brief extension <b> python` | CPython C-extension (no ctypes) |
| Node | native ext | `brief extension <b> node` | NAPI `.node` addon (no npm) |
| Go | cgo package | `brief export <b> go` | `import "C"` + wrappers; String → `C.GoString` |
| Java | native ext | `brief extension <b> java` + `brief export <b> java` | JNI shim (`lib<b>.so`) + class with `native` methods |
| Lua | native ext | `brief extension <b> lua` | C module `luaopen_<b>`; `lua_pushstring` |
| C# | bindings | `brief bindings <b> csharp` | P/Invoke DllImport class |

The composite String crosses every boundary as a pointer into the
NUL-invariant `[len][bytes][\0]` region — zero-copy read from every host
(`C.GoString`, `NewStringUTF`, `lua_pushstring`, `Marshal.PtrToStringUTF8`,
`PyUnicode_FromString`, `napi_create_string_utf8`).

**Timing** (feature_hash count=1000, ns/call, interleaved median): C 1223,
C++ 1229, Lua 1200, Java 1160 (JIT), Node 1260, Go 1302, Python 1430 — all
within ~17% of native C. See `docs/plans/2026-08-04-ship-common-language-environments.md`.

## 15. The zero-friction FFI gate

`benchmarks/bridge/gate/run_gate.sh` is the committed regression gate: Brief's
`feature_hash` vs each host's *own* native `feature_hash` (Gate A — real work)
and Brief's `add` vs the host's pure-internal `add` (Gate B — dispatch). Run it
with `BRIEF_RUN_GATE=1 cargo test --test gate`.

**Gate A — Brief vs native feature_hash (median):**
| host | ratio | |
|------|-------|---|
| C | 1.03 | parity |
| C++ | 1.01 | parity |
| Java | 0.95 | Brief faster (JIT) |
| Go | 1.12 | cgo + compute |
| Lua | 0.07 | Brief 14× |
| Python | 0.004 | Brief 238× |
| Node | 0.01 | Brief 192× |

Compiled hosts are at parity; interpreted hosts get Brief's native-machine-code
compute and win by 1–2 orders of magnitude.

**Gate B — dispatch (Brief add vs native internal add):** Python 0.63 (the
`METH_FASTCALL` shim dispatches *faster* than Python's own function call),
Lua 1.19, C 1.23, C++ 1.44, Node 2.15, Java 5.99, Go 143×. Node/Java/Go sit at
their structural FFI bounds (NAPI/JNI/cgo — the host's own foreign-call cost).

The gate keeps the native sink live (Go's dead-code eliminator stripped a
dead timed loop → bogus sub-1ns/iter numbers) and measures Go native in a
pure-Go binary (a cgo-linked binary distorted it). See
`docs/plans/2026-08-04-zero-friction-ffi-gate.md`.
