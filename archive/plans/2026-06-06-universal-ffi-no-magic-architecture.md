# Universal FFI & No-Magic Architecture

**Date**: 2026-06-07
**Status**: Phases 11–13 complete. BracketOp refactor complete. See AGENTS.md for current gaps.

## Core Philosophy

> Everything compiles to binary in the end. Briev tries to figure out the best way how,
> and treats every language equal to itself.

Briev is a **Cosmopolitan language**. Its role is not to reinvent forty years of systems
libraries. Its role is to be the **orchestrator** — taking any language that produces
LLVM bitcode, linking it into a unified module, optimizing across all language boundaries,
and emitting for the target. The bitcode doesn't remember what language it came from.
`opt -O3` inlines across all of them equally. The distinction between "compiled language"
and "interpreted language" disappears at the LLVM IR level.

This is the **Metropolitan FFI** — every language is a citizen of the same metropolitan
binary. The interpreter (FFI registry dispatch), the native LLVM backend, and the Webstack
backend are three faces of the same engine: take code, resolve imports, compile to binary.
The language the import came from is irrelevant.

### Sub-Principles

- **No Magic**: Every behavior is declared in `.bv` source, never in Rust string-match
  arms. No hardcoded names, no implicit destinations, no parser-discarded tokens.
- **Everything is `import "link/..."`**: Runtime, stdlib native implementations, external
  C/Rust/Zig/Python libraries — all go through the same generic pipeline.
- **Interpreter dispatch through FFI registry**: One hashmap lookup
  (`ffi_name_to_location` → `foreign_functions`). Faster than 20+ string comparisons
  in the old `dispatch_method_by_type`.
- **Webstack decomposes, doesn't transform**: Briev logic → WASM. HTML/CSS/SVG → as-is.
  JS/TS → emitted as native browser source. Nothing is forced into WASM that the browser
  can run natively.
- **Zero performance loss**: LTO inlines across all language boundaries. The dispatch path
  is strictly hashmap lookups — faster than the string matching it replaces.

## Web Native Citizens (Never Compile to WASM)

Anything the browser already has a native parser, renderer, or runtime for stays as-is. Compiling these to WASM would add latency for zero benefit.

| Technology | How it's emitted | Reasoning |
|---|---|---|
| **HTML** | `.rbv` `<view>` → emitted as `.html` unchanged | Browser parses HTML natively |
| **CSS** | `.rbv` `<style>` → `.css` file or inline `<style>` | Browser renders CSS natively |
| **SVG** | `TopLevel::SvgComponent` → embedded in HTML/SVG DOM | Native DOM-rendered vector graphics. Already supported in `.rbv` |
| **JS/TS (FFI targets)** | Emitted as native `.js`/`.ts` source alongside glue code | Browser's JS engine runs JS/TS natively. WASM would add an unnecessary compilation → decompilation round trip |
| **JSON** | `JSON.parse()` / `JSON.stringify()` calls in generated JS glue | Browser has native JSON parser. For native target, goes through `yyjson` bitcode-linked |
| **WebGL/WebGPU shaders (GLSL/WGSL/SPIR-V)** | Generated as string constants, passed directly to WebGL/WebGPU API by JS glue | GPU drivers consume these directly. No WASM bridge needed |
| **HTML Templates / `<template>`** | Emitted as-is | Native DOM cloning |
| **ES Modules / Import Maps** | `import './pkg/app.js'` — standard JS module system | Native browser behavior, already used by webstack glue |

**The rule**: Only compile to WASM what the browser cannot run natively. This keeps the WASM binary small — containing only the Briev state machine and its C/Rust FFI dependencies.

### FFI Target Classification per Backend

Each `frgn` in a `.rbv` source is classified by its target capability during webstack codegen.
The same `frgn` resolves differently depending on the target:

| `from` / TOML target | Native (.bv) path | Web (.rbv) path |
|---|---|---|
| `from "c"` or no `from` (C ABI) | `import "link/..."` → `clang -emit-llvm` → bitcode → LTO | `clang --target=wasm32 -emit-llvm` → bitcode → `llc -march=wasm32` → inlined WASM |
| `from "rs"` / `from "rust"` | `rustc --emit=llvm-bc` → bitcode → LTO | Same, wasm32 target |
| `from "zig"` | `zig build-obj --emit-llvm-ir` → bitcode → LTO | Same, wasm32 target |
| `from "py"` / `from "python"` (Codon) | `codon --emit-llvm -O3` → bitcode → LTO | Same, wasm32 target (Codon bitcode → wasm32) |
| `from "java"` (GraalVM) | `native-image --llvm` → bitcode → LTO | Same, wasm32 target (bloated — SubstrateVM included) |
| `from "ts"` / `from "js"` | — (no LLVM backend) | Emit as native JS/TS source alongside glue |
| `from "webgl"` / `from "webgpu"` | Generates GLSL/WGSL/SPIR-V strings | Same strings, passed to WebGL/WebGPU API by JS glue |
| `from "json"` | `yyjson` bitcode-linked | JS glue uses native `JSON.parse`/`JSON.stringify` |
| `from "<profile:...>"` | Resolved through DBVS profile | Resolved through DBVS profile, target checked |
| `target = "wasm"` in TOML | Uses TOML's `wasm_impl` JS snippet | Same (native for web) |

The bitcode doesn't remember what language it came from. `opt -O3` inlines across all of
them equally. On native target, the llc output is x86_64/AArch64/etc. On web target, the
llc output is wasm32. Everything that produces LLVM bitcode targets both.

## Phase 0: Infrastructure — Eliminate Runtime Special Treatment

**Goal**: `briev_rt.c` is a regular file, not an embedded constant. `LinkDependency` is generic. The LTO pipeline handles N modules.

### 0.1 — Move `briev_rt.c` from embedded constant to filesystem file

**Files**: `src/main.rs:389`, `src/main.rs:2212-2219`, `src/main.rs:1864-1978`

| Current | Problem | Fix |
|---------|---------|-----|
| `include_str!("../runtime/briev_rt.c")` | Embedded in compiler binary | Move to `lib/runtime/briev_rt.c`, delete the `include_str!` constant |
| `is_bundled_rt` flag in parser (line 800) | Hardcoded name match on `"briev_rt.o"` | Delete the field entirely |
| Always written to disk (line 2218-2219) | Even when program has no link deps | Only write when `import "link/briev_rt.c"` is present |
| Single-file `try_lto_pipeline(rt_c_path)` | Only handles exactly one C file | Replace with generic `link_and_optimize(&[LinkModule])` |

**After**: `import "link/briev_rt.c"` resolves through the exact same code path as `import "link/xxhash/xxhash.c"`. Zero special treatment.

### 0.2 — `LinkLanguage` enum in AST

**File**: `src/ast.rs:839`, `src/parser.rs:793-805`

```rust
pub enum LinkLanguage {
    C,              // .c  → clang -emit-llvm
    Cpp,            // .cpp / .cc / .cxx → clang++ -emit-llvm
    Rust,           // .rs → rustc --emit=llvm-bc
    Zig,            // .zig → zig build-obj --emit-llvm-ir
    Python,         // .py → codon build --emit-llvm (native target only)
    Bitcode,        // .bc → already bitcode, copy as-is
    Object,         // .o / .a → link at object level (no LTO possible)
}
```

Parser dispatch at line 793:

| Extension | `LinkLanguage` |
|-----------|---------------|
| `.c` | `C` |
| `.cpp` / `.cc` / `.cxx` | `Cpp` |
| `.rs` | `Rust` |
| `.zig` | `Zig` |
| `.py` | `Python` |
| `.bc` | `Bitcode` |
| `.o` / `.a` | `Object` |

The `LinkDependency` struct becomes:

```rust
pub struct LinkDependency {
    pub path: String,
    pub source_lang: LinkLanguage,
}
```

Delete the `is_bundled_rt` field.

### 0.3 — Generic multi-module LTO pipeline

**File**: `src/main.rs:1864-1978`

Replace `try_lto_pipeline(rt_c_path, ...)` with:

```rust
fn link_and_optimize(
    out_base: &Path,
    stem: &str,
    ll_file: &Path,
    link_modules: &[LinkModule],
    llvm_flags: &[String],
    wasm_target: bool,        // true → -march=wasm32 for llc
) -> Option<PathBuf>
```

Each `LinkModule` contains `(source_path: PathBuf, lang: LinkLanguage)`.

Compilation dispatch per language:

| Language | Command | Output |
|---|---|---|
| C | `clang -c -emit-llvm -O2 source.c -o source.bc` | `.bc` |
| C++ | `clang++ -c -emit-llvm -O2 source.cpp -o source.bc` | `.bc` |
| Rust | `rustc --emit=llvm-bc -C opt-level=3 source.rs -o source.bc` | `.bc` |
| Zig | `zig build-obj --emit-llvm-ir -O ReleaseFast source.zig -o source.bc` | `.bc` |
| Python | `codon build --emit-llvm -O3 source.py -o source.bc` | `.bc` |
| Bitcode | copy as-is | `.bc` |
| Object | skip (cannot LTO, emit warning) | — |

For WASM target (`wasm_target = true`):
- C/C++ compiled with `clang --target=wasm32 -emit-llvm`
- `llc -march=wasm32` instead of `--mcpu=native`
- Other languages checked for WASM bitcode support per-language

Then: `llvm-link` all `.bc` files + program `.bc` → `opt -O3` → `llc` → `.o`/`.wasm`.

---

## Phase 1: No-Magic FFI Dispatch

**Goal**: Eliminate `dispatch_method_by_type`, `is_builtin_constructor`, and `handle_result_method`. All collection operations, Result methods, Option constructors, and enum constructors resolve through the FFI registry — zero name-string matching.

### 1.1 — Register all built-in operations in FFI registry

**Files**: `src/ffi/registry.rs` (add match arms), `src/interpreter.rs:1821-2051` (delete)

Every operation currently in `dispatch_method_by_type` gets a `"__builtin.*"` location key and a Rust closure. The closures contain the **exact same Rust code** currently in the match arms — only the dispatch mechanism changes.

| Location key | Implements | Currently at (interpreter.rs) |
|---|---|---|
| `"__builtin.HashMap.insert"` | `map.insert(key, value); map` | line 1869 |
| `"__builtin.HashMap.get"` | `map.get(key) → Option::Some/None` | line 1876 |
| `"__builtin.HashMap.contains_key"` | `map.contains_key(key) → Bool` | line 1885 |
| `"__builtin.HashMap.remove"` | `map.remove(key); map` | line 1890 |
| `"__builtin.HashMap.len"` | `map.len() → Int` | line 1897 |
| `"__builtin.HashMap.is_empty"` | `map.is_empty() → Bool` | line 1898 |
| `"__builtin.HashMap.keys"` | `map.keys().collect() → List` | line 1899 |
| `"__builtin.HashMap.values"` | `map.values().collect() → List` | line 1903 |
| `"__builtin.HashMap.new"` | `HashMap::new()` | line 1834 |
| `"__builtin.HashSet.insert"` | `set.insert(item); set` | line 1914 |
| `"__builtin.HashSet.contains"` | `set.contains(item) → Bool` | line 1917 |
| `"__builtin.HashSet.remove"` | `set.remove(item); set` | line 1920 |
| `"__builtin.HashSet.len"` | `set.len() → Int` | line 1923 |
| `"__builtin.HashSet.is_empty"` | `set.is_empty() → Bool` | line 1924 |
| `"__builtin.HashSet.new"` | `HashSet::new()` | line 1835 |
| `"__builtin.StringBuilder.append_char"` | `buffer.push(c); buffer` | line 1942 |
| `"__builtin.StringBuilder.append_str"` | `buffer.push_str(s); buffer` | line 1949 |
| `"__builtin.StringBuilder.append_int"` | `buffer.push_str(&n.to_string()); buffer` | line 1956 |
| `"__builtin.StringBuilder.append_bool"` | `buffer.push_str(&b.to_string()); buffer` | line 1963 |
| `"__builtin.StringBuilder.append_float"` | `buffer.push_str(&f.to_string()); buffer` | line 1970 |
| `"__builtin.StringBuilder.to_string"` | `buffer.clone() → String` | line 1977 |
| `"__builtin.StringBuilder.clear"` | `buffer.clear(); buffer` | line 1978 |
| `"__builtin.StringBuilder.len"` | `buffer.len() → Int` | line 1983 |
| `"__builtin.StringBuilder.is_empty"` | `buffer.is_empty() → Bool` | line 1984 |
| `"__builtin.StringBuilder.capacity"` | `buffer.capacity() → Int` | line 1985 |
| `"__builtin.StringBuilder.new"` | `String::new()` | line 1836 |
| `"__builtin.Stack.push"` | `stack.push(item); stack` | line 1993 |
| `"__builtin.Stack.pop"` | `stack.pop() → Option::Some(item, stack)` | line 1998 |
| `"__builtin.Stack.peek"` | `stack.last().cloned() → Option::Some` | line 2007 |
| `"__builtin.Stack.len"` | `stack.len() → Int` | line 2012 |
| `"__builtin.Stack.is_empty"` | `stack.is_empty() → Bool` | line 2013 |
| `"__builtin.Stack.clear"` | `Vec::new() → Stack` | line 2014 |
| `"__builtin.Stack.new"` | `Vec::new()` | line 1837 |
| `"__builtin.Queue.enqueue"` | `queue.push_back(item); queue` | line 2022 |
| `"__builtin.Queue.dequeue"` | `queue.pop_front() → Option::Some(item, queue)` | line 2027 |
| `"__builtin.Queue.front"` | `queue.front().cloned() → Option::Some` | line 2036 |
| `"__builtin.Queue.len"` | `queue.len() → Int` | line 2041 |
| `"__builtin.Queue.is_empty"` | `queue.is_empty() → Bool` | line 2042 |
| `"__builtin.Queue.clear"` | `VecDeque::new() → Queue` | line 2043 |
| `"__builtin.Result.is_ok"` | `variant == "Ok" → Bool` | line 646 |
| `"__builtin.Result.is_err"` | `variant == "Err" → Bool` | line 647 |
| `"__builtin.Result.unwrap"` | `if Ok → fields["result"] else error` | line 648 |
| `"__builtin.Result.unwrap_err"` | `if Err → fields["error"] else error` | line 655 |
| `"__builtin.Option.Some"` | `Enum("Option", "Some", {"value": v})` | line 1348 (partial) |
| `"__builtin.Option.None"` | `Enum("Option", "None", {})` | line 1345 (partial) |
| `"__builtin.Result.Ok"` | `Enum("Result", "Ok", {"result": v})` | line 1348 (partial) |
| `"__builtin.Result.Err"` | `Enum("Result", "Err", {"error": v})` | line 1351 (partial) |
| `"__builtin.clone"` | `arg_values[0].clone()` | line 1841 |

### 1.2 — Create `lib/std/__builtin/` declaration files

New `.bv` files that declare `frgn` for each built-in. No `from` clause — they resolve through `fn_locations_by_name`.

```briev
// lib/std/__builtin/hashmap.bv
frgn __builtin_HashMap_insert<K,V>(map: HashMap<K,V>, key: K, value: V) -> HashMap<K,V>;
frgn __builtin_HashMap_get<K,V>(map: HashMap<K,V>, key: K) -> Option<V>;
frgn __builtin_HashMap_contains_key<K,V>(map: HashMap<K,V>, key: K) -> Bool;
frgn __builtin_HashMap_remove<K,V>(map: HashMap<K,V>, key: K) -> HashMap<K,V>;
frgn __builtin_HashMap_len<K,V>(map: HashMap<K,V>) -> Int;
frgn __builtin_HashMap_is_empty<K,V>(map: HashMap<K,V>) -> Bool;
frgn __builtin_HashMap_keys<K,V>(map: HashMap<K,V>) -> List<K>;
frgn __builtin_HashMap_values<K,V>(map: HashMap<K,V>) -> List<V>;
frgn __builtin_HashMap_new<K,V>() -> HashMap<K,V>;
```

```briev
// lib/std/__builtin/stack.bv
frgn __builtin_Stack_push<T>(stack: Stack<T>, item: T) -> Stack<T>;
frgn __builtin_Stack_pop<T>(stack: Stack<T>) -> Option<(T, Stack<T>)>;
frgn __builtin_Stack_peek<T>(stack: Stack<T>) -> Option<T>;
frgn __builtin_Stack_len<T>(stack: Stack<T>) -> Int;
frgn __builtin_Stack_is_empty<T>(stack: Stack<T>) -> Bool;
frgn __builtin_Stack_clear<T>(stack: Stack<T>) -> Stack<T>;
frgn __builtin_Stack_new<T>() -> Stack<T>;
```

```briev
// lib/std/__builtin/result.bv
frgn __builtin_Result_is_ok<T,E>(r: Result<T,E>) -> Bool;
frgn __builtin_Result_is_err<T,E>(r: Result<T,E>) -> Bool;
frgn __builtin_Result_unwrap<T,E>(r: Result<T,E>) -> T;
frgn __builtin_Result_unwrap_err<T,E>(r: Result<T,E>) -> E;
frgn __builtin_Result_Ok<T,E>(value: T) -> Result<T,E>;
frgn __builtin_Result_Err<T,E>(error: E) -> Result<T,E>;
```

```briev
// lib/std/__builtin/option.bv
frgn __builtin_Option_Some<T>(value: T) -> Option<T>;
frgn __builtin_Option_None<T>() -> Option<T>;
```

(Same pattern for `string_builder.bv`, `queue.bv`, `hashset.bv`)

### 1.3 — Update existing stdlib `.bv` files to use builtins

Each stdlib module imports its `__builtin` declarations and wraps them in proper `defn` definitions.

```briev
// lib/std/hashmap.bv
import { __builtin_HashMap_new, __builtin_HashMap_insert, __builtin_HashMap_get,
         __builtin_HashMap_contains_key, __builtin_HashMap_remove,
         __builtin_HashMap_len, __builtin_HashMap_is_empty,
         __builtin_HashMap_keys, __builtin_HashMap_values }
    from "std/__builtin/hashmap.bv";

defn new<K,V>() -> HashMap<K,V> [true][true] {
    term __builtin_HashMap_new();
};

defn insert<K,V>(map: HashMap<K,V>, key: K, value: V) -> HashMap<K,V> [true][true] {
    term __builtin_HashMap_insert(map, key, value);
};

defn get<K,V>(map: HashMap<K,V>, key: K) -> Option<V> [true][true] {
    term __builtin_HashMap_get(map, key);
};

// ... etc.
```

```briev
// lib/std/result.bv
import { __builtin_Result_Ok, __builtin_Result_Err,
         __builtin_Result_is_ok, __builtin_Result_is_err,
         __builtin_Result_unwrap, __builtin_Result_unwrap_err }
    from "std/__builtin/result.bv";

defn Ok<T,E>(value: T) -> Result<T,E> [true][true] {
    term __builtin_Result_Ok(value);
};

defn Err<T,E>(error: E) -> Result<T,E> [true][true] {
    term __builtin_Result_Err(error);
};

defn is_ok<T,E>(r: Result<T,E>) -> Bool [true][true] {
    term __builtin_Result_is_ok(r);
};

// ... etc.
```

User code: `import { new, insert, get } from "std/hashmap.bv"` — calls the `defn`, which calls the `frgn`, which resolves through `ffi_name_to_location` → `foreign_functions`. **Zero string matching anywhere.**

### 1.4 — Register `fn_locations_by_name` mappings

In the DBVS or TOML bindings (or a new `__builtin` binding file), register the reverse name→location mappings:

```
"__builtin_HashMap_insert" → "__builtin.HashMap.insert"
"__builtin_HashMap_get"    → "__builtin.HashMap.get"
"__builtin_Result_Ok"       → "__builtin.Result.Ok"
// ... etc.
```

These are loaded by `load_from_bindings_dir()` into `fn_locations_by_name`, exactly as the existing DBVS bindings work today.

### 1.5 — Clean up interpreter dispatch chain

**File**: `src/interpreter.rs:1370-1378`, `src/interpreter.rs:639-667`, `src/interpreter.rs:1821-2051`

Delete:
- `is_builtin_constructor()` (line 1821) — constructors are now in the registry
- `dispatch_method_by_type()` (line 1830) — all operations in registry
- The call site at line 1373-1378 — remove the block entirely
- `handle_result_method()` (line 639) — Result methods moved to registry

New dispatch chain at `Expr::Call`:

```
1. User defn → self.definitions hashmap lookup
2. Dynamic .so FFI → self.frgn_registry
3. Enum constructors from state → self.state lookup (for unit constructors stored as Value::Defn)
4. ffi_name_to_location → foreign_functions → handles EVERYTHING built-in
5. Error: UndefinedForeignFunction
```

**Performance analysis**:
- Old path: 1 hashmap lookup (`definitions`) + up to 20+ string comparisons spread across 7 match arms (`is_ok`/`is_err`/`unwrap`/`unwrap_err` early check, then `dispatch_method_by_type` with nested type-variant arms and inner name matches)
- New path: 1 hashmap lookup (`definitions`) + 1 hashmap lookup (`ffi_name_to_location`) + 1 hashmap lookup (`foreign_functions`) = **3 hashmap lookups, zero string comparisons**

Strictly faster.

---

## Phase 2: Generic `import "link/..."` Multi-Language Pipeline

**Goal**: `import "link/foo.c"`, `import "link/bar.rs"`, `import "link/baz.zig"`, `import "link/qux.py"` all compile to bitcode and LTO with the program through the same code path.

### 2.1 — Link path resolution

**File**: `src/import_resolver.rs`

Paths starting with `link/` resolve in this order:
1. `lib/runtime/<rest>` (for `link/briev_rt.c` → `lib/runtime/briev_rt.c`)
2. `lib/std/c/<rest>` (for `link/xxhash/xxhash.c` → `lib/std/c/xxhash/xxhash.c`)
3. Project root `<rest>` (for `link/mylib.c` → `./mylib.c`)
4. Absolute paths as-is

The `link/` prefix is a convention, not a filesystem requirement. The resolver strips it and searches.

### 2.2 — Compiler driver generic compilation

**File**: `src/main.rs:2129-2331`

The driver already collects `LinkDependency` items (line 2129). Replace the hardcoded single-file briev_rt pipeline:

```rust
let link_modules: Vec<LinkModule> = link_deps.iter()
    .filter_map(|dep| resolve_link_path(&dep.path).ok()
        .map(|p| LinkModule { source: p, lang: dep.source_lang }))
    .collect();

if !link_modules.is_empty() {
    let lto_obj = link_and_optimize(
        &out_base, stem, &output_file,
        &link_modules, &llvm_flags, wasm_target
    );
    // ... link with cc or wasm-ld
}
```

### 2.3 — WASM target support in the pipeline

When compiling for WASM (detected from `.rbv` target or `--target wasm` flag):

- C/C++ sources compiled with `clang --target=wasm32 -emit-llvm -O2`
- `llc -march=wasm32` instead of `--mcpu=native` 
- Output `.wasm` object instead of native `.o`
- Final linking via `wasm-ld` or direct `.wasm` output from `llc`

The same `link_and_optimize()` function handles both — just different flags.

---

## Phase 3: Standard Library FFI Couplings

Each library follows the same pattern:

1. Vendor C/Rust source to `lib/std/c/<name>/`
2. Create `lib/std/<name>.bv` with `import "link/std/c/<name>/..."` + `frgn` declarations
3. Wrap with Briev `sig` and `defn` as needed
4. Run `frgn` declarations through the standard FFI dispatch (no special treatment)

### Priority order

| # | Library | Why | Source pattern | License |
|---|---|---|---|---|
| 1 | **xxHash** | Trivial, first validation of pipeline | Single `xxhash.c` + `xxhash.h` | BSD 2-Clause |
| 2 | **yyjson** | Zero-copy JSON for stdlib | Single `yyjson.c` + `yyjson.h` | MIT |
| 3 | **stb_image** | Image loading, validates `#define STB_IMAGE_IMPLEMENTATION` pattern | Header-only, needs wrapper `.c` | Public Domain / MIT |
| 4 | **lz4** | Compression for `.vpo` datasets | `lz4.c` + `lz4.h` | BSD 2-Clause |
| 5 | **libgrapheme** | Unicode grapheme segmentation for safe string slicing | Tiny, single `.c` + `.h` | ISC |
| 6 | **openlibm** | Bit-accurate cross-platform math | Full source tree (multiple `.c`) | BSD 2-Clause / Public Domain |
| 7 | **nanopb** | Zero-allocation Protocol Buffers | `pb.c` + `pb.h` | zlib |
| 8 | **sqlite** | Database — major milestone | Amalgamation (single `sqlite3.c`) | Public Domain |
| 9 | **miniaudio** | Audio playback/capture | Header + `#define MINIAUDIO_IMPLEMENTATION` | Unlicense / MIT |
| 10 | **sokol** | GPU graphics, windowing | Multiple headers + wrapper `.c` files | zlib |
| 11-12 | **libuv / mbedtls** | Async I/O + Cryptography | Full source trees (many files) | MIT / Apache 2.0 |
| 13 | **cimgui** | Immediate-mode GUI — first C++ FFI test | `cimgui.h` + `cimgui.cpp` | MIT |
| 14 | **lwIP** | Bare-metal TCP/IP | Full source tree | BSD 3-Clause |

The `#define IMPLEMENTATION` pattern (stb, miniaudio, sokol) gets a wrapper `.c` file:

```c
// lib/std/c/stb_image/stb_image.c
#define STB_IMAGE_IMPLEMENTATION
#include "stb_image.h"
```

The Briev source imports the wrapper:
```briev
import "link/std/c/stb_image/stb_image.c";
frgn stbi_load(filename: Ptr<Byte>, x: Ptr<Int>, y: Ptr<Int>, comp: Ptr<Int>, req_comp: Int) -> Ptr<Byte>;
```

---

## Phase 4: Webstack — `.rbv` Compilation

**Key architecture**: The webstack **decomposes** the `.rbv` file by component type. Each component goes to its native runtime. Nothing is forced into WASM that the browser can run natively.

### 4.1 — Component routing

| Component | Fate | Implementation |
|---|---|---|
| **Briev logic** (reactive state machine, transactions, contracts) | → WASM via LLVM `-march=wasm32` | Existing `WebstackGenerator::generate_rust_code()` → wasm-bindgen → wasm-pack, OR direct LLVM→wasm32 path |
| **HTML** (from `<view>` tag) | → Emitted as `.html` unchanged | Existing behavior, already works |
| **CSS** (from `<style>` tag or imports) | → Emitted as `.css` file or inline `<style>` | Existing behavior, already works |
| **SVG** (from `TopLevel::SvgComponent`) | → Embedded in HTML/SVG DOM | Already supported in `.rbv` pipeline |
| **JS/TS FFI targets** (e.g., `frgn foo() from "ts"`) | → Emitted as native `.js`/`.ts` source | Browsers run JS/TS natively. JS glue calls the function directly — no WASM boundary |
| **JSON parsing** (for web target) | → `JSON.parse()` / `JSON.stringify()` in generated JS glue | Browser's native JSON parser is faster than any WASM JSON library |
| **WebGL/WebGPU shaders** | → Generated as GLSL/WGSL/SPIR-V string constants, passed to WebGL/WebGPU API by JS glue | GPU drivers consume these natively. No WASM bridge |
| **HTML Templates / `<template>`** | → Emitted as-is | Native DOM cloning |
| **C/Rust/Zig/Python (Codon)/Java (GraalVM) FFI targets** | → WASM via LLVM LTO pipeline | `import "link/lib.c"` → `clang --target=wasm32 -emit-llvm` → `llvm-link` → `opt -O3` → `llc -march=wasm32`. The bitcode is target-agnostic — the language it came from is irrelevant |

### 4.2 — LLVM-to-WASM pipeline for C/Rust FFI in `.rbv`

```
import "link/lib.c" in .rbv source
  → clang --target=wasm32 -emit-llvm -O2 lib.c → lib.bc
  → llvm-as program.ll → program.bc
  → llvm-link program.bc lib.bc → merged.bc
  → opt -O3 merged.bc → merged.opt.bc
  → llc -march=wasm32 merged.opt.bc → module.wasm
  → Base64-encode → inline in generated HTML/JS
```

Single file output. No network fetch for WASM. Zero CORS issues.

### 4.3 — JS/TS emission for web FFI targets

When `.rbv` declares `frgn formatDate(d: String) -> String from "ts"`:

1. The webstack generator emits the TS/JS source as a separate file (or inlined in the JS glue)
2. The JS glue directly calls the TS/JS function — no WASM boundary, no serialization
3. TS/JS runs natively in the browser's JS engine

**Why not WASM for JS/TS?** Browsers execute JS natively in highly optimized JIT compilers (V8, SpiderMonkey). Compiling JS→WASM would add a compilation step for zero benefit — the browser would need to decompile it back or run it in a WASM interpreter that's slower than the native JIT. TS/JS are **native web citizens** — let them run as-is.

### 4.4 — Source maps and debugging

Generated JS glue preserves line-number mappings back to the original `.rbv` source where possible, so developers can debug their Briev→WASM logic and their JS/TS FFI targets in browser DevTools.

---

## Phase 5: Language-Capability Matrix (All Languages, All Targets)

**Philosophy**: "Everything compiles to binary in the end." The question is which
toolchain produces that binary from a given source language. Briev delegates to the
appropriate compiler and treats the resulting bitcode as its own. The bitcode doesn't
remember what language it came from — it targets whatever `llc` backend is selected.

### 5.1 — Universal toolchain dispatch

| Language | Native (.bv) → LLVM bitcode | Web (.rbv) → WASM | Web (.rbv) → native browser |
|---|---|---|---|
| **C** | `clang -emit-llvm` → `.bc` | `clang --target=wasm32 -emit-llvm` → `.bc` → `llc -march=wasm32` | — |
| **C++** | `clang++ -emit-llvm` → `.bc` | Same, wasm32 target | — |
| **Rust** | `rustc --emit=llvm-bc` → `.bc` | Same, wasm32 target | — |
| **Zig** | `zig build-obj --emit-llvm-ir` → `.bc` | Same, wasm32 target | — |
| **Python (Codon)** | `codon build --emit-llvm -O3` → `.bc` | Same, wasm32 target (Codon targets LLVM, not CPython) | — |
| **Java (GraalVM)** | `native-image --llvm --emit-llvm-bc` → `.bc` | Same, wasm32 target (includes SubstrateVM runtime — acceptable for cosmopolitan tier) | — |
| **TypeScript (AssemblyScript)** | `asc` → `.wasm` → `wasm2llvm` → `.bc` | Same pipeline | Emit as native JS (preferred for web — browser runs JS natively) |
| **JavaScript** | — (no LLVM producer) | — | Emit as native JS source |
| **Full CPython** | emscripten CPython → `.wasm` (linked as separate module, not inlined) | Same | — |

### 5.2 — The CODON path (Python → bitcode)

```
import "link/mylib.py"
  → codon build --emit-llvm -O3 mylib.py → mylib.bc
  → llvm-link program.bc mylib.bc → merged.bc
  → opt -O3 → llc (-march=native | -march=wasm32) → binary
```

Same pipeline as C. Python (via Codon's AOT compiler, which compiles a typed subset)
becomes LLVM bitcode and gets inlined by `opt -O3` across the language boundary.
Target-agnostic — the same `.py` file compiles to both native and WASM.

### 5.3 — The GraalVM path (Java → bitcode)

```
import "link/MyClass.java"
  → javac MyClass.java → MyClass.class
  → native-image --llvm --emit-llvm-bc MyClass → MyClass.bc   (GraalVM)
  → llvm-link → LTO pipeline
```

**Caveat**: Java AOT via GraalVM Native Image carries the SubstrateVM runtime (GC, thread
model, metadata). This is acceptable for the cosmopolitan tier (`.bv`, general purpose).
Embedded Briev (`.ebv`) rejects GC-dependent bitcode at compile time via the hardware
validator.

### 5.4 — The AssemblyScript path (TypeScript → bitcode)

```
import "link/mylib.ts"
  → asc mylib.ts --exportRuntime --optimize --outFile mylib.wasm
  → wasm2llvm mylib.wasm → mylib.bc   (or use AssemblyScript's experimental LLVM backend)
  → llvm-link → LTO pipeline
```

**For `.rbv` (web target)**: TS is emitted as **native JS** (see 4.3). No WASM compilation
needed — the browser runs TS/JS natively. The AssemblyScript path is only for native targets
where TS code must be inlined into the compiled binary.

### 5.5 — Boundary enforcement

The hardware validator (`src/hardware_validator.rs`) enforces per-target constraints across
all languages:

| Target | Codon Python | GraalVM Java | GC-dependent bitcode | Dynamic heap | Unrestricted syscalls |
|---|---|---|---|---|---|
| `.bv` (general) | ✓ | ✓ | ✓ (warns) | ✓ | ✓ |
| `.rbv` (web) | ✓ (via LLVM→WASM) | ✓ (via LLVM→WASM, bloated) | N/A (WASM sandbox) | ✓ (WASM linear memory) | ✗ (WASM sandbox) |
| `.ebv` (embedded) | ✗ (no Codon for bare metal) | ✗ | ✗ (rejects at compile time) | ✗ (rejects) | ✗ (no OS) |

---

## Phase 6: Eliminate Remaining Magic

### 6.1 — Remove `Ok`/`Err`/`Some`/`None` hardcoded enum constructors

**File**: `src/interpreter.rs:1345-1356`

These are currently hardcoded Rust match arms that create `Value::Enum` on name match. After Phase 1, they become FFI registry entries:
- `"__builtin.Result.Ok"` → wraps value in `Enum("Result", "Ok", {"result": v})`
- `"__builtin.Result.Err"` → wraps value in `Enum("Result", "Err", {"error": e})`
- `"__builtin.Option.Some"` → wraps value in `Enum("Option", "Some", {"value": v})`
- `"__builtin.Option.None"` → returns `Enum("Option", "None", {})`

Declared in `lib/std/__builtin/result.bv` and `lib/std/__builtin/option.bv`, re-exported through `lib/std/result.bv` and `lib/std/option.bv`.

**After**: `Ok(value)` in Briev source resolves to `defn Ok` in `std/result.bv` → calls `__builtin_Result_Ok` → FFI registry → `Enum("Result", "Ok", ...)`. Zero name magic.

### 6.2 — Remove hardcoded LLVM `emit_declares()`

**File**: `src/backend/llvm.rs:1840-1864`

The backend has hardcoded LLVM `declare` statements for `__rt_init`, `__rt_poll`, `__rt_wait`. These should come from `std/rt.bv` through the generic `frgn` declaration emission (lines 727-751, which already works for any `frgn` in the program).

Delete the hardcoded block at lines 1840-1864. The functions are already declared in `lib/std/briev_rt.bv` or should be moved there.

### 6.3 — Remove `from "libruntime"` parser discard

**File**: `src/parser.rs`

Search for `"libruntime"` string handling. The parser currently discards `from "libruntime"` values. Delete this special case — all `from` values must be meaningful or absent.

---

## File Change Summary

| File | Phase | Change |
|---|---|---|
| `src/ast.rs:839` | 0.2 | Replace `is_bundled_rt: bool` with `source_lang: LinkLanguage` |
| `src/parser.rs:793-805` | 0.2 | Dispatch on extension → set `LinkLanguage` variant |
| `src/parser.rs` | 6.3 | Remove `"libruntime"` discard |
| `src/import_resolver.rs` | 0.2, 2.1 | Preserve `LinkLanguage` through resolution; resolve `link/` paths to stdlib or project |
| `src/main.rs:389` | 0.1 | Delete `include_str!("../runtime/briev_rt.c")` |
| `src/main.rs:1864-1978` | 0.3 | Replace `try_lto_pipeline()` with generic `link_and_optimize()` |
| `src/main.rs:2129-2331` | 0.3, 2.2 | Generic link-dep collection + compilation dispatch |
| `src/ffi/registry.rs` | 1.1 | Add `"__builtin.*"` match arms for all collection/Result/Option operations |
| `src/interpreter.rs:639-667` | 1.5 | Delete `handle_result_method()` |
| `src/interpreter.rs:1345-1356` | 6.1 | Delete hardcoded `Ok`/`Err`/`Some`/`None` constructors |
| `src/interpreter.rs:1370-1378` | 1.5 | Delete `dispatch_method_by_type` call site |
| `src/interpreter.rs:1821-2051` | 1.5 | Delete `is_builtin_constructor()` + `dispatch_method_by_type()` |
| `src/backend/llvm.rs:1840-1864` | 6.2 | Delete hardcoded `emit_declares()` block |
| `lib/std/__builtin/*.bv` | 1.2 | **New** — frgn declarations for all built-in operations |
| `lib/std/hashmap.bv` | 1.3 | Rewrite `defn` to call `__builtin_*` frgn functions |
| `lib/std/result.bv` | 1.3, 6.1 | Rewrite `Ok`/`Err`/`is_ok`/etc as `defn` calling `__builtin_*` |
| `lib/std/option.bv` | 6.1 | Rewrite `Some`/`None` as `defn` calling `__builtin_*` |
| `lib/std/stack.bv` | 1.3 | Rewrite to use `__builtin_Stack_*` |
| `lib/std/queue.bv` | 1.3 | Rewrite to use `__builtin_Queue_*` |
| `lib/std/string_builder.bv` | 1.3 | Rewrite to use `__builtin_StringBuilder_*` |
| `lib/runtime/briev_rt.c` | 0.1 | **Moved** from `runtime/briev_rt.c` |
| `lib/std/c/<name>/` | 3 | **New directories** — vendored C library sources |
| `lib/std/<name>.bv` | 3 | **New files** — Briev wrappers for each library |

---

## Verification Strategy

| Phase | Gate |
|---|---|
| 0.1 | `cargo test --lib` passes; `briev_rt.c` no longer in binary; file exists at `lib/runtime/briev_rt.c` |
| 0.2 | Parser test: `import "link/foo.c"` produces `LinkDependency{ lang: C }` |
| 0.3 | Link trivial C file (`int add(a,b){return a+b;}`) from Briev → call `add(2,3)` → verify result is 5 |
| 1 | All collection/stdlib tests pass with `dispatch_method_by_type` deleted |
| 2 | `import "link/foo.c"` + `import "link/bar.rs"` both compile to bitcode and LTO together |
| 3 | xxHash value from Briev matches C reference output |
| 4.1 | `.rbv` with inlined C library → single HTML file with embedded WASM |
| 4.2 | `.rbv` with TS FFI target → `.ts` file emitted alongside JS glue |
| 4.3 | WebGL shader in `.rbv` → GLSL string in generated JS, passed to WebGL API |
| 5.1 | `import "link/mylib.py"` → Codon bitcode → LTO → native binary |
| 5.2 | `import "link/mylib.py"` → Codon bitcode → `llc -march=wasm32` → WASM binary |
| 5.3 | `import "link/MyClass.java"` → GraalVM bitcode → LTO → native binary |
| 6 | Zero hardcoded string matches remain in interpreter or backends |

All phases maintain `cargo test --lib` passing with 0 regressions.

---

## Phase 7: Embedded Tier System — `.ebv` / `.sebv` / `.hebv`

**Goal**: Define a clear three-tier strictness model for embedded/hardware targets,
with cross-tier module boundaries and progressively stricter validation.

### 7.0 — Contract sugar rules

These apply to ALL `.bv` files, regardless of tier:

| Syntax | Precondition | Postcondition | Meaning |
|---|---|---|---|
| `[pre][post]` | `pre` | `post` | Full contract (both sides) |
| `[[post]` | `true` (omitted) | `post` | Postcondition only, no guard. The opening `[[` means the precondition was omitted |
| `[pre]]` | `pre` | `true` (omitted) | Guard only, no guarantee. The closing `]]` means the postcondition was omitted |

Memory aid: the left bracket `[` is always the precondition. `[[` = two left brackets =
the first one opens an empty precondition (defaults to `true`), the second opens the
postcondition. `]]` = two right brackets = the first closes the precondition, the second
closes an empty postcondition (defaults to `true`).

These sugar forms are **banned** in `.sbv`, `.srbv`, `.sebv`, and `.hebv`.

### 7.1 — Tier definitions

| Extension | Tier | Contracts | FFI / link | Dynamic types | Backend output |
|---|---|---|---|---|---|
| `.bv` | Cosmopolitan | Sugar allowed | Any language via `import "link/..."` | All allowed | Native binary, WASM, C/Rust source |
| `.sbv` | Strict cosmopolitan | Full contracts required | Any language | All allowed | Same |
| `.ebv` | Embedded (bare-metal) | Sugar allowed | C/Rust only (warn on Python/Java/JS) | Allowed but may warn | Native binary (no OS deps) |
| `.sebv` | Strict embedded | Full contracts required | C/Rust only | Allowed | Native binary (no OS deps) |
| `.hebv` | Hardware (logic graph) | Full contracts required, must be **total** | **No `import "link/..."`. No `frgn`.** | **Rejected** — only `Bit`, `UInt[N]`, `SInt[N]`, fixed arrays, structs | Verilog, VHDL, or SystemVerilog |

### 7.2 — Cross-tier module boundaries (firmware interface)

A stricter module can be imported by a looser module. The stricter module's contracts are
**trusted** by the caller — the caller gets the guarantee without re-verification:

```briev
// heater_firmware.sebv — strict, verified, no OS deps
export txn read_thermocouple(ch: Int) -> Int [ch >= 0 && ch < 8][term >= 0 && term < 4096];
```

```briev
// controller.bv — cosmopolitan, calls the firmware boundary
import { read_thermocouple } from "heater_firmware.sebv";
// read_thermocouple guaranteed [term >= 0 && term < 4096] — caller trusts this
```

Import rules:

| Importer ╲ Exporter | `.bv` | `.sbv` | `.ebv` | `.sebv` | `.hebv` |
|---|---|---|---|---|---|
| **`.bv`** | ✓ | ✓ | ✓ | ✓ | ✓ |
| **`.sbv`** | ❌ | ✓ | ❌ | ✓ | ✓ |
| **`.ebv`** | ❌ | ❌ | ✓ | ✓ | ✓ |
| **`.sebv`** | ❌ | ❌ | ❌ | ✓ | ✓ |
| **`.hebv`** | ❌ | ❌ | ❌ | ❌ | ✓ |

A stricter module cannot depend on a looser one — a verified module must not rely on
an unverified contract.

### 7.3 — `.hebv` total contract requirement

"Total" means the contract must describe a closed logic graph:

- **Precondition** must fully specify all valid input states — `[true]` left open is an error
- **Postcondition** must fully specify all output states — `[true]` right open is an error
- **Convergence** must be compile-time provable — the transaction body must terminate
  with a statically known bound
- **No infinite loops** — all convergence bounds must be known at compile time
- **No partial updates** — every state field write must be covered by the postcondition
- **All types must be synthesizable** — only `Bit`, `UInt[N]`, `SInt[N]`, fixed-size
  arrays, and structs with known bit widths

This is what makes `.hebv` safe for hardware synthesis. The Verilog/VHDL backend can
translate the transaction directly to a state machine with known states and known
transition boundaries — because the contract is a complete behavioral specification.

### 7.4 — Hardware validator additions

**File**: `src/hardware_validator.rs`

| Check | `.ebv` | `.sebv` | `.hebv` |
|---|---|---|---|
| No `import "link/..."` | ⚠️ Warn on Python/Java/JS | ⚠️ Warn on Python/Java/JS | ❌ Error any |
| No `frgn` | ✓ OK | ✓ OK | ❌ Error |
| No `syscall!` | ❌ Error | ❌ Error | ❌ Error |
| No dynamic heap | ❌ Error | ❌ Error | ❌ Error |
| Contracts total (no `true` defaults) | ⚠️ Warn | ❌ Error | ❌ Error |
| All loops provably bounded | ⚠️ Warn | ❌ Error | ❌ Error |
| Only synthesizable types | ⚠️ Warn | ⚠️ Warn | ❌ Error |
| Cross-tier import rules | ✓ OK | ✓ OK | ❌ Error if importing non-`.hebv` |

---

## Phase 8: MMIO / DBVS Address Resolution and Bus Interface Generation

**Goal**: Both `.ebv`/`.sebv` and `.hebv` target the same MMIO addresses, but the
backend generates different hardware interface code around them.

### 8.1 — Single source of truth: `.dbvs` address bindings

The `.dbvs` schema defines address bindings symbolically. Both tiers read the same file:

```dbvs
// heater_config.dbvs
register MMIO_THERMOCOUPLE @ 0x40000000 {
    width: 32;
    access: read-only;
    interface: "axi4-lite";    // used by .heb vs .sebv backend selects appropriate
};
```

```briev
// heater_control.hebv — pin-level
trg thermocouple_raw: UInt[32] @ MMIO_THERMOCOUPLE;
// compiler resolves MMIO_THERMOCOUPLE → 0x40000000 from .dbvs
```

```briev
// heater_monitor.ebv — MMIO level
trg thermocouple_raw: Int @ MMIO_THERMOCOUPLE;
// same .dbvs, same address, different backend
```

### 8.2 — Backend-specific interface generation

| Tier | Address resolution | Bus generation | Backend |
|---|---|---|---|
| `.hebv` | `@ MMIO_THERMOCOUPLE` → 0x40000000 (pin address) | Verilog/VHDL backend auto-generates AXI-lite (or Wishbone, GPIO, etc.) slave wrapper from the `interface` field in `.dbvs` | Verilog/VHDL/SV codegen |
| `.ebv`/`.sebv` | `@ MMIO_THERMOCOUPLE` → 0x40000000 (MMIO window) | No bus generation — hardware-side bus bridge already exists. LLVM/C backend emits volatile load/store at the address | LLVM, C, Rust codegen |

For `.hebv`, the `.dbvs` `interface` field controls which bus protocol wrapper the
hardware backend generates:

```
register MMIO_BUFFER @ 0x40001000 {
    width: 64;
    access: read-write;
    interface: "axi4-lite";    // → AXI4-Lite slave controller generated
};

register GPIO_LEDS @ 0x40002000 {
    width: 8;
    access: write-only;
    interface: "gpio";         // → simple GPIO output port
};
```

If no `interface` field is specified, the backend infers a reasonable default:
- `read-only` → GPIO input
- `write-only` → GPIO output  
- `read-write` → AXI4-Lite slave

### 8.3 — Verification

| Gate |
|---|
| Same `.dbvs` file drives both `.hebv` and `.ebv` compilation with correct address resolution |
| `.hebv` output includes auto-generated AXI/GPIO wrapper in VHDL/Verilog |
| `.ebv` output emits volatile load/store at the same address (no bus wrapper) |

---

## Phase 9: DBVS Redesign — Decouple FFI Bindings from MMIO Registers

**Goal**: The DBVS format currently uses `register <hex_address> as "<name>"` for both
hardware register maps and FFI function bindings. For FFI bindings, the hex address is
meaningless — a legacy from the MMIO use case. The DBVS parser and schema should be
redesigned to use appropriate syntax for each domain.

### 9.1 — Current problem

The DBVS parser at `src/dbriev/parser.rs:294` calls `parse_register()` which requires
a hex address as the first token. For FFI bindings (`std/bindings/*.dbvs`), this address:

```
register 0x1000 as "__builtin_HashMap_new" {
    location: "__builtin.HashMap.new";
};
```

The address `0x1000` is never used — it's parsed into `DbrievRegister.address` but
never read during FFI binding resolution (which uses `as "<name>"` → `location`
mapping). It's cargo-culted from hardware register definitions.

### 9.2 — Proposed direction

Add a new DBVS keyword for bindings that don't require an address, e.g.:

```
bind "__builtin_HashMap_new" {
    location: "__builtin.HashMap.new";
    description: "Create empty HashMap";
}
```

This would parse into the same internal representation but without requiring a fake
hex address. The `register` keyword remains for MMIO/hardware use cases.

Alternatively, keep the `register` keyword but make the address optional for
bindings that have `type: Data` (non-MMIO):

```
register as "__builtin_HashMap_new" {
    location: "__builtin.HashMap.new";
    description: "Create empty HashMap";
}
```

### 9.3 — Status

**DEFERRED** — Not a priority. The current syntax works correctly for FFI bindings;
the hex address is simply ignored for non-MMIO operations. This redesign is a
cosmetic/syntax cleanup for when DBVS is revisited.

### 9.4 — Files affected

| File | Change |
|---|---|
| `src/dbriev/parser.rs:294-330` | Add `bind` keyword or optional address on `register` |
| `std/bindings/*.dbvs` | Migrate to new syntax |
| `src/ffi/loader.rs` | Verify no breaking changes to binding loading |

---

## Phase 10: Watchdog Trigger Preemptibility Analysis

**Date**: 2026-06-07
**Status**: Planned — design approved, not yet implemented
**Files**: `src/analysis/watchdog.rs` (NEW), `src/analysis/mod.rs`, minor wiring in `src/interpreter.rs`

### Motivation

Briev's computational model is one big state machine — the program is a `main()` that
transitions between states until it hits `term!` or exhausts all paths. For embedded,
safety-critical, and interactive programs, **provable termination is less useful than
provable preemptibility**. A program that cannot prove it ends on its own should still
prove it CAN be stopped — by a user, a supervisor, or an external system.

The watchdog concept already exists in the AST (`Contract.watchdog: Option<WatchdogSpec>`,
`WatchdogSpec { condition: Expr, is_required: bool }`) and is parsed as a third bracket
`[pre][post][watchdog]` or external `?[cond]` / `?![cond]` after the brackets. Currently
it serves as a runtime condition check (evaluated at `term`). Phase 10 extends it to also
support **compile-time liveness proofs** when the expression references a `frgn trg`.

### 10.1 — Semantics

| Syntax | Meaning |
|--------|---------|
| `?[@button]` | Prove external trigger `button` can preempt this transaction. **Optional**: skipped if the transaction already proves natural termination (convergent loop with `[count < N][count == N]` + `term`). |
| `?![@button]` | Prove external trigger `button` can preempt this transaction. **Required**: always enforced, even if the transaction terminates naturally. |
| `?[timeout]` | Existing runtime watchdog — boolean condition checked at `term`. No change in behavior. |

The `@` prefix parses as `Expr::PriorState(Ident("button"))` — the existing prior-state
expression. The analysis detects the `PriorState` wrapper to distinguish trigger references
from variable references.

### 10.2 — Detection

When `WatchdogSpec.condition` is `Expr::PriorState(Box::new(Expr::Identifier(name)))`,
treat the watchdog as a trigger preemptibility proof. Otherwise, treat it as an existing
runtime condition check (no change in behavior).

### 10.3 — Conflict Chain Analysis

For each transaction T with `?[@trg]` or `?![@trg]`:

| Step | What the analyzer does | Error if fails |
|------|----------------------|----------------|
| **10.3.1** | Resolve `trg` against all `frgn trg` declarations in the program | `"@trg is not a declared frgn trg"` |
| **10.3.2** | Find all transactions with `trg` in their guard (`[trg]` or `[trg && ...]` condition) | `"No handler for trigger @trg"` |
| **10.3.3** | Collect all variables written by the handler chain — walk the transition graph from each handler through all reachable `node` and `defn` calls, collecting every `&var = expr` assignment | — |
| **10.3.4** | Compute intersection with T's precondition variables (all identifiers appearing in `pre_condition`) | `"Handler chain for @trg doesn't write to any variable in T's precondition"` |
| **10.3.5** | For each intersecting variable, evaluate: does the handler's write pattern **falsify** T's precondition? (e.g., if T says `[ready == true]`, does the handler set `ready = false`?) | `"@trg handler writes to ready but doesn't falsify [ready == true]"` |
| **10.3.6** | Verify the handler chain does NOT restore the precondition before its own `term` (i.e., the kill is permanent — the loop won't restart) | `"@trg handler chain restores T's precondition — loop would restart"` |

For `?` (optional): if step 10.3.4 fails (no conflict), check whether T already proves
natural termination — if so, the proof passes (watchdog is optional, not needed).

For `?!` (required): always enforce all 6 steps. Even if T terminates, the user explicitly
wants proof that `@trg` CAN preempt it.

### 10.4 — Triple Brackets vs External Syntax

The existing parser already handles two syntaxes:

1. **External syntax** (all modes): `?[@trg]` or `?![@trg]` written after the bracket
   pairs. This is the primary syntax for trigger watchdogs.

2. **Third bracket** (strict mode only): `[pre][post][@trg]` — the third bracket holds
   a trigger reference. The same analysis applies.

Both parse into the same `WatchdogSpec { condition: Expr::PriorState(..), is_required: bool }`.
The analysis pass treats them identically.

### 10.5 — Pipeline Placement

The analysis runs after the transition graph is built and before backend codegen.
It is a new file `src/analysis/watchdog.rs` that consumes:

- The full `Program` (for `frgn trg` declarations)
- The transition graph (for handler reachability)
- The transaction's `Contract` (for the watchdog spec and precondition)

It produces a `Vec<WatchdogError>` — one error per failed link in the proof chain.
If any errors exist and the watchdog is required (`is_required = true`), compilation
fails. If optional (`is_required = false`) and the error is in steps 10.3.4–10.3.6,
a warning is emitted but compilation continues (the transaction might still terminate
naturally).

### 10.6 — Interaction with Existing Backends

The analysis is purely compile-time — it produces no runtime code. Backends that
currently emit watchdog code (COBOL recursion depth limiter, Verilog timeout counter)
continue to do so when the condition is a non-trigger expression. When the condition
is a trigger reference, the backend emits a comment noting the preemptibility proof
succeeded at compile time:

```rust
// Verified: @button can preempt transaction loop
```

### 10.7 — Test Plan

| Test | Expectation |
|------|-------------|
| `?[@valid_button]` with handler that sets `can_run = false` where T's precondition is `[can_run]` | ✅ Pass |
| `?[@valid_button]` but no txn guards on `[button]` | ❌ "No handler for trigger" |
| `?[@valid_button]` but handler writes to unrelated variable | ❌ "No conflict with precondition" |
| `?[@valid_button]` but handler toggles `ready` back to `true` before `term` | ❌ "Handler restores precondition" |
| `?[@valid_button]` on convergent loop `[i < N][i == N]` | ✅ Pass (optional — skip) |
| `?![@valid_button]` on same convergent loop | ❌ "Required watchdog lacks preemptibility proof" |
| `?[@undeclared]` | ❌ "Not a declared frgn trg" |
| `?[timeout]` (normal variable, not a trigger) | ✅ Existing runtime watchdog — no analysis change |

### 10.8 — Prerequisite: Extend `uni` pattern matching with structured `Pattern` AST

**Date**: 2026-06-07
**Status**: In progress — required before Phase 1.3 can land

The current `Statement::Unification` stores `pattern: String` — a flat encoding
like `"Some(v)"` or `"Some((item,rest))"`. The interpreter splits this string on
`,` to extract field names, which breaks on nested tuples (`"(item,rest)"` becomes
`["(item", "rest)"]`).

Phase 1.3 (native collection builtins) needs `__builtin_Stack_pop` to return
`Option<(T, Stack<T>)>` and have `uni pair(Some((item, new_stack)))` correctly
destructure the inner tuple. The string-based pattern can't handle this.

**Fix**: Replace `pattern: String` with `pattern: Pattern` in both compilers.

#### AST

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Var(String),              // v, _, "literal"
    Tuple(Vec<Pattern>),      // (a, b), (_, rest)
    Wildcard,                 // _
    LitInt(i64),
    LitFloat(f64),
    LitString(String),
    LitChar(char),
    LitBool(bool),
}
```

The `Statement::Unification` struct changes from:
```rust
pub struct Unification {   // current
    pub name: String,
    pub pattern: String,       // flat string
    pub expr: Box<Expr>,
}
```
to:
```rust
pub struct PatternMatch {
    pub name: String,          // variable being matched
    pub variant: String,       // variant name (Ok, Some, None, etc.)
    pub fields: Vec<Pattern>,  // structured patterns
}
```

Keep backwards compat: the old `Unification` pattern string format can be
parsed into `Pattern::Var` during a migration shim.

#### Files affected

| File | Change |
|------|--------|
| `src/ast.rs` | Add `Pattern` enum; update `Statement::Unification` to use `Pattern` |
| `src/parser.rs` | `parse_pattern_fields()` returns `Vec<Pattern>` instead of `Vec<String>` |
| `src/interpreter.rs` | `Statement::Unification` handler matches recursively on `Pattern::Tuple` |
| `src/desugarer.rs` | Update `Unification` construction |
| `src/analysis/*.rs` | Update pattern field access (transition_graph, call_graph, range, region) |
| `src/backend/*.rs` | Update all backends' `Unification` handling |
| `lib/compiler/parser.bv` | Mirror pattern parsing in self-hosted compiler |
| `lib/compiler/ast.bv` | Mirror `Pattern` type in self-hosted compiler |

#### After

```briev
let pair: Option<(T, Stack<T>)> = __builtin_Stack_pop(stack);
uni pair(Some((item, rest))) = {
    // item: T, rest: Stack<T>
    term rest;
};
```

Pattern matching becomes a general-purpose tool available to all `uni`
statements — not just enums with flat fields.

---

```
                    ┌──────────────────────────────────────┐
                    │          Briev .bv source             │
                    │  frgn ...  import "link/..."          │
                    └──────┬──────┬──────┬──────────────────┘
                           │      │      │
                 ┌─────────┘      │      └──────────────┐
                 ▼                ▼                      ▼
      ┌──────────────────┐ ┌──────────────┐   ┌────────────────────┐
      │ Interpreter      │ │ LLVM native  │   │ Webstack (.rbv)    │
      │ (FFI registry    │ │ (--target    │   │                    │
      │  dispatch, zero  │ │  native)     │   │ Briev logic → WASM │
      │  string matching)│ │              │   │ HTML/CSS → as-is   │
      │                  │ │ C → bitcode  │   │ SVG → DOM          │
      │                  │ │ C++ → bitc.  │   │ JS/TS → native JS  │
      │                  │ │ Rust → bitc. │   │ WebGL/GPU → native │
      │                  │ │ Zig → bitc.  │   │ JSON → native JS   │
      │                  │ │ Python→bitc. │   │ C libs → WASM      │
      │                  │ │ (Codon AOT)  │   │ (via LTO pipeline) │
      │                  │ │ Java→bitcode │   │                    │
      │                  │ │ (GraalVM)    │   │ JS glue → gen'd    │
      │                  │ │ TS→bitcode   │   │ deterministically  │
      │                  │ │ (AssemblyScr)│   │                    │
      │                  │ │              │   │ Single-file output │
      │                  │ │ LTO inlines  │   │ (all native        │
      │                  │ │ ALL together │   │  citizens inlined) │
       └──────────────────┘ └──────────────┘   └────────────────────┘
```

---

## Phase 11: Sync Domains — Deterministic Lockstep Execution (2026-06-07)

### Motivation

Every concurrent system — multi-core CPU, RTOS task set, FPGA clock domain — needs a way to declare **deterministic execution boundaries across concurrent operations**. Without this, parallel tasks can read half-committed state from other tasks, leading to transient corruption. The `sync(domain)` keyword provides a native, compiler-proven synchronization primitive.

### Design

Three forms, one concept: **"these operations must start and finish at the same time."**

| Form | Syntax | Use case |
|------|--------|----------|
| **Prefix modifier** | `sync(domainA) node Name [pre][post] { body }` | Reactive lockstep — all txns in `domainA` fire and commit simultaneously |
| **Prefix on callable** | `sync(domainB) txn Name [pre][post] -> Ret { body }` | Same lockstep guarantee when called as a group |
| **Prefix on defn** | `sync(domainC) defn Name(args) -> Ret { body }` | Declares a defn as participating in a domain (affects its behavior when called from a sync block) |
| **Block statement** | `sync { stmt1; stmt2; ... };` | Fork-join barrier — all statements run in parallel, barrier at end |

### Semantics

- **Entry barrier**: No transaction in the domain starts executing until ALL members have met their preconditions. The domain's effective precondition is the AND of all member preconditions.
- **Exit barrier**: No transaction in the domain commits its state until ALL members have completed their bodies. Swan songs (`term ->`) are deferred until the barrier resolves.
- **Block statement**: Inside a `defn` or `txn` body, `sync { ... }` dispatches all statements to the async thread pool, waits on a futex barrier, and resumes after all complete. If the thread pool is disabled, executes sequentially (same effect, compatible fallback).
- **No `let` inside sync blocks**: `let` creates a binding that other statements may depend on, violating the "all start simultaneously" invariant. The parser emits an error.
- **No nested sync blocks**: Contradicts the barrier model. Parser error.

### Implementation Steps

| Step | File(s) | Change |
|------|---------|--------|
| 11.1 | `lexer.rs` | Add `Token::Sync` keyword |
| 11.2 | `ast.rs` | Add `Statement::SyncBlock { body: Vec<Statement> }` |
| 11.3 | `parser.rs` | Parse `sync(id...)` prefix on txn/node/defn → store as `modifiers: Vec<Hashtag>` entry (`Hashtag { name: "sync", args: [...] }`). Parse `sync { body }` as `Statement::SyncBlock`. Error on `let` inside sync block. Error on nested sync blocks. |
| 11.4 | `interpreter.rs` | Execute `SyncBlock` (fallback serial). In reactor loop, extract sync groups from `modifiers`; enforce entry/exit barriers. |
| 11.5 | `analysis/transition_graph.rs` | Group transitions by sync domain extracted from `modifiers`; precondition = AND of all members |
| 11.6 | All backends | Add stub match arm for `Statement::SyncBlock` |
| 11.7 | Tests | Parser + interpreter + transition graph |
| 11.8 | AGENTS.md | Note Phase 11 complete, update test count |

### Language Design — Arrow Conventions (`->` vs `<-`)

The two arrow tokens have distinct, non-overlapping roles:

**`->` (Arrow)** — reserved for "eventful" transformations:
- `defn fn(params) -> RetType` — function return type
- `term expr -> cleanup` — swan song / commit action triggered after convergence

No mutation. No data movement. Only declaration of what happens *after* a computation completes.

**`<-` (ArrowLeft)** — exclusively for collection mutation and data movement.
The `&` sigil marks which operand(s) are being mutated:

| Syntax | `&` on left | `&` on right | Semantics |
|--------|-------------|-------------|-----------|
| `&list <- value` | ✅ Push | — | Append `value` to tail of `list` |
| `value <- &list` | — | ✅ Pop | Remove one element from tail of `list` into `value` |
| `&list[0] <- value` | ✅ Push at index | — | Insert `value` at head of `list` |
| `value <- &list[0]` | — | ✅ Pop at index | Remove head element of `list` into `value` |
| `<- &list` | — | ✅ Discard | Pop tail from `list`, throw away |
| `<- &list[0]` | — | ✅ Discard at index | Pop head from `list`, throw away |
| `&dest <- &src[; cond]` | ✅ Transfer | ✅ Transfer | Pop all matching elements from `src` into `dest` |
| `dest <- src[; cond]` | — | — | Query/copy — no mutation |

The `;` inside brackets introduces a mask/filter expression (parsed as `MultiSocket`'s `mask` field).

### Open Work Items

| Item | Scope | Priority |
|------|-------|----------|
| **A** — `extract_arrow_target` MultiSocket arm | `parser.rs` only, ~5 min | Immediate (fixes `adults <- &list[; cond]`) |
| **B** — `Expr::ArrowTransfer` for two-sided `&` | `ast.rs`, `parser.rs`, `interpreter.rs`, all backends, ~1 hr | Immediate (fixes `&dest <- &src[; cond]`) |
| **C** — Phase 12 | HashMap/HashSet primitives, ~3-4 hr | Next |
| **D** — Phase 13 | Stack/Queue/Tuple primitives, ~2-3 hr | After C |

---

## Phase 12: HashMap/HashSet Primitives (2026-06-07)

### Motivation

`HashMap` and `HashSet` are already first-class `Value` variants backed by Rust's native `HashMap` and `HashSet`. But they lack literal syntax, direct dispatch, and `:>` projection targets — forcing users through `__builtin_*` FFI calls and the interpreter through string-match dispatch (an explicit anti-pattern per AGENTS.md).

### Design

```briev
// Literal syntax (disambiguated from blocks at expression level)
let map: HashMap<String, Int> = {"a": 1, "b": 2};
let set: HashSet<Int> = {1, 2, 3};

// Projection targets
let n: Int = map :> Size;           // 2
let ks: List<String> = map :> Keys; // ["a", "b"]
let has: Bool = map :> Contains("a"); // true
let first: Int = set :> Pop;        // 1 (set now {2, 3})

// Direct dispatch (no string-match)
let m2 = map.insert("c", 3);     // dispatches on Value::HashMap, not fn_name == "insert"
let exists = set.contains(1);    // dispatches on Value::HashSet
```

#### Arrow Syntax for HashMap/HashSet

| Syntax | `&` on left | `&` on right | Semantics |
|--------|-------------|-------------|-----------|
| `&map <- (key, value)` | ✅ Push | — | Insert entry (tuple as key-value pair) |
| `&map[key] <- value` | ✅ Push at key | — | Insert/replace at key (indexed push) |
| `value <- &map[key]` | — | ✅ Pop at key | Remove by key, return value |
| `<- &map[key]` | — | ✅ Discard at key | Remove by key, discard |
| `&set <- value` | ✅ Push | — | Insert into set |
| `value <- &set` | — | ✅ Pop | Remove arbitrary element |
| `<- &set` | — | ✅ Discard | Remove arbitrary element, discard |
| `&dest <- &map[; cond]` | ✅ Transfer | ✅ Transfer | Transfer matching entries between maps |

The right-hand `(key, value)` is a tuple literal — no new parsing needed. `[key]` on map uses the existing list-index bracket syntax. This eliminates the old `fn_name == "insert"` string-match dispatch entirely: the interpreter dispatches on `ArrowMut` variant + `Value` type instead.

### Implementation Steps

| Step | File(s) | Change |
|------|---------|--------|
| 12.1 | `lexer.rs` | Handle `{` at expression level as dict/set literal opener (vs block at statement level) |
| 12.2 | `ast.rs` | Add `Expr::MapLiteral { entries: Vec<(Expr, Expr)> }`, `Expr::SetLiteral { entries: Vec<Expr> }`, `ProjectionTarget::Keys`, `ProjectionTarget::Values`, `ProjectionTarget::Contains(Expr)`, `ProjectionTarget::Pop` |
| 12.3 | `parser.rs` | `{` after `=`, `,`, `(`, `[`, infix op → expression literal; `{` after statement keyword → block |
| 12.4 | `interpreter.rs` | Evaluate `MapLiteral`/`SetLiteral` directly. Eliminate all `dispatch_method_by_type` string matches for `insert`, `keys`, `values`, `contains`, `contains_key`, `get`, `remove`, `clear`, `is_empty`, `len` — dispatch on `Value::HashMap` / `Value::HashSet` variant directly |
| 12.5 | `stdlib` | `hashmap.bv` / `hashset.bv` become thin wrappers (or eliminated) |
| 12.6 | All backends | Add stubs for `MapLiteral`, `SetLiteral`, new projection targets |
| 12.7 | Tests | Literal parsing, interpreter dispatch, projection targets |

---

## Phase 13: Stack/Queue/Tuple Primitives (2026-06-07)

### Motivation

Same rationale as Phase 12, extended to Stack (`Vec`), Queue (`VecDeque`), and Tuple (currently flattened to `Value::List`). These complete the collection-primitive story.

### Design

```briev
// True tuple type (not flattened to List)
let pair: (Int, String) = (1, "hello");
let first: Int = pair :> 0;             // projection by index

// Stack/Queue already have runtime backing via Value::Stack / Value::Queue
```

#### Arrow Syntax for Stack/Queue

| Syntax | Semantics |
|--------|-----------|
| `&stack <- value` | Push to top |
| `value <- &stack` | Pop from top |
| `<- &stack` | Pop from top, discard |
| `&queue <- value` | Enqueue (push back) |
| `value <- &queue` | Dequeue (pop front) |
| `<- &queue` | Dequeue, discard |

Both use the existing `<-` arrow patterns. No new syntax — the interpreter dispatches on `Value::Stack` vs `Value::Queue`.

### Implementation Steps

| Step | File(s) | Change |
|------|---------|--------|
| 13.1 | `ast.rs` | Add `Value::Tuple(Vec<Value>)` as a true distinct variant. Add `ProjectionTarget::Index(usize)` for tuple indexing. |
| 13.2 | `interpreter.rs` | Stack operations dispatch on `Value::Stack`, Queue on `Value::Queue`. String-match elimination for `push`, `pop`, `enqueue`, `dequeue`, `peek`. |
| 13.3 | `stdlib` | `stack.bv` / `queue.bv` become thin wrappers |
| 13.4 | All backends | Add stubs for new projection targets |
| 13.5 | Tests | Stack/Queue dispatch, tuple projection |

---

### BracketOp Design — Unified Bracket Operations for `MultiSlice`

`list[::3 ; age >= 18 ::2]` reveals that `coordinates` + `mask` as separate fields is
too rigid. A flat `Vec<BracketOp>` replaces them for `Expr::MultiSlice`:

```rust
pub enum BracketOp {
    Coord(SliceCoordinate),  // `5`, `0..10`, `time:5`, `@dim`, `...`
    Mask(Box<Expr>),          // `; age >= 18`
    Stride(Box<Expr>),        // `::3`
}

MultiSlice {
    value: Box<Expr>,
    ops: Vec<BracketOp>,
}
```

Semantic order in interpreter: walk `ops` sequentially, maintaining an element stream:
- `Coord` — dimension navigation / index selection
- `Mask` — filter the stream
- `Stride` — decimate the stream (take every Nth)

Example: `list[::3 ; age >= 18 ::2]`
`ops` = `[Stride(3), Mask(age >= 18), Stride(2)]`
1. Take every 3rd element from source
2. Filter to those with `age >= 18`
3. Take every 2nd of the filtered set

`Expr::Slice` (single-dim) keeps its existing `start`, `end`, `stride`, `mask` fields
— only `MultiSlice` gets the flat ops list, since it's the only form that needs
arbitrary ordering of bracket symbols.

`SliceCoordinate` enum stays as-is (used inside `BracketOp::Coord`).

#### Impact

| File | Change |
|------|--------|
| `ast.rs` | Add `BracketOp` enum, update `MultiSlice`, remove standalone `mask` field |
| `parser.rs` | `parse_multi_slice` emits `Vec<BracketOp>`; `::` after `;` becomes `Stride` |
| `interpreter.rs` | Walk `ops` sequentially instead of `coordinates` + `mask` |
| All other files | Match arms for `MultiSlice` — field name change from `coordinates`/`mask` to `ops` |
| `extract_arrow_target` | MultiSlice arm checks for `Coord` ops only (extract index/range) |