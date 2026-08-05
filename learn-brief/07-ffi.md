# FFI — Make Briv the central language

Briv compiles to a native library that any other language can call at
**native speed**. There are two directions:

| Direction | Keyword / command | What it does |
|-----------|-------------------|-------------|
| **Import** | `frgn ... from "..."` | Call a foreign (C/runtime) function from Briv |
| **Export** | `export defn ...` + `brivc bindings\|export\|extension` | Expose a Briv function to any host language |

Every language is a folder in `lib/glue/<lang>/` — config + templates. The
compiler has zero language knowledge; adding a language is adding a folder.

---

## 1. `frgn` — importing foreign functions

```briv
// frgn <C_symbol>(<params>) [-> <ret>] [as <briv_name>] from <source> [fallback <expr>];
frgn __read_file__(path: String) -> Int as briv_read_file_raw
  from "lib/runtime/briv_rt.c" fallback 0;
frgn __write_file__(path: String, data: String) -> Int as briv_write_file
  from "lib/runtime/briv_rt.c" fallback 0;
```

| Part | Meaning |
|------|---------|
| `frgn` | This is an import declaration |
| `__read_file__` | The **C/runtime symbol** — what the linker sees |
| `(path: String)` | Parameters with Briv types |
| `-> Int` | Return type (optional, defaults to void) |
| `as briv_read_file_raw` | The **Briv-side name** — what Briv code calls |
| `from "..."` | Provenance: an inlined C source, a GLUE bridge target, or a linked library |
| `fallback 0` | Value to use if the foreign function cannot be called |

Call it like any Briv function:

```briv
let w: Int = briv_write_file(path, saved);   // writes via the runtime
```

The `from` sources:

| Source form | What happens |
|-------------|-------------|
| `from "lib/runtime/briv_rt.c"` | Compiles the C source and links it |
| `from "something.py"` / `.mjs` | Routes through a GLUE bridge target |
| `from "link/library.so"` | Links a system library |

`import "glue/c.bv"` brings the C boundary types (`CStr`, `CDouble`, …) and the
`meld CStr -> String` declaration, so boundary modules' vocabulary applies to
your bridge.

---

## 2. `export defn` — exporting Briv functions

The export signature **is** the boundary contract:

```briv
import "glue/c.bv";

export defn echo(name: CStr) -> CStr { term name; };
export defn greet(name: CStr) -> CStr {
    let s: String = name;      // the CStr ↔ String meld: no `as` needed
    term s;
};
export defn join(a: CStr, b: CStr) -> CStr { term a + b; };
export defn identity(x: CDouble) -> CDouble { term x; };
```

Boundary types live in `lib/glue/c.bv`: `CStr` (`#String<C_String>`),
`CFloat`, `CDouble`, `CI64`, `CI32`, `CBool`, `CChar`, `CPtr`.

A **stateful** export (one that reads or writes a state field) automatically
carries the `%state` pointer:

```briv
let saved: String = "";
export defn read() -> CStr { term saved; };   // takes the state handle
```

The composite String crosses every boundary as a pointer into a state-owned
`[len][bytes][\0]` region — the host reads it zero-copy in place.

### The three commands

| Command | What you get |
|---------|-------------|
| `brivc bindings <bridge> <lang>` | Declarative bindings (C header, C# class) |
| `brivc export <bridge> <lang>` | A language package (Go package, Java class, Rust crate) |
| `brivc extension <bridge> <lang>` | A **native extension** (Python `.so`, Node `.node`, Java JNI `lib*.so`, Lua C module) |

```bash
# A C-callable static + shared library:
brivc build my_bridge.bv --library --out build/
#   → libmy_bridge.a  +  my_bridge.so

# A native Python extension (no ctypes):
brivc extension my_bridge.bv python --out build/
$ python3 -c "import my_bridge; print(my_bridge.feature_hash(1000, 42))"

# A NAPI Node addon (no npm):
brivc extension my_bridge.bv node --out build/
$ node -e "const b = require('./my_bridge.node'); console.log(b.join('foo','bar'))"

# A Go cgo package:
brivc export my_bridge.bv go --out build/

# A Java JNI shim + class:
brivc extension my_bridge.bv java --out build/
brivc export my_bridge.bv java --out build/
```

---

## 3. Adding a new language

Add a folder `lib/glue/<lang>/` — no compiler changes:

1. **`types.bv`** — boundary declarations (usually `import "glue/c.bv";`).
2. **`glue.dbvl`** — the target: `protocols` (category → native / C-ABI),
   `conversions` (`to_abi`/`from_abi`), `state`, `param_decl`, and the
   **toolchain recipe** (`native_include_cmd`, `native_suffix`,
   `native_link_cmd`, `native_cc`, `native_prefix`).
3. **Templates** — `bindings.*`, `templates.*` (with `{{exports}}`), and/or
   `native.*` (the extension shim: module, method, per-category parse/build).
4. **A test** — render assertion + toolchain-guarded round-trip.

`brivc bindings|export|extension <bridge> <lang>` finds `lib/glue/<lang>/` by
name and renders through the generic pipeline.

> **New to this? The full step-by-step is in
> `docs/guides/add-an-ffi-target.md`** — a field-by-field walkthrough with the
> shipped Lua target as the worked example, the template system explained, the
> renderer variables, a copy-the-right-folder table, and a checklist. The
> anatomy reference is `docs/architecture/glue-ffi.md` §5.

---

## 4. The speed table (zero friction)

`feature_hash(count=1000)` — **Briv vs the host writing it natively**
(median ns/call; run the gate with `BRIEF_RUN_GATE=1 cargo test --test gate`):

| host | Briv | native | ratio |
|------|-------|--------|-------|
| C | 1098 | 1100 | 1.00 |
| C++ | 1107 | 1094 | 1.01 |
| Java | 1116 | 1122 | 1.00 |
| Go | 1189 | 1107 | 1.07 |
| Lua | 1162 | 12309 | **0.09** |
| Python | 1179 | 229794 | **0.01** |
| Node | 1282 | 190498 | **0.01** |

Compiled hosts are at parity. **Interpreted hosts get Briv's native-machine-code
compute and win by 1–2 orders of magnitude** — Python calling Briv is like
calling a super-efficient version of Python. Even a zero-work call dispatches
faster than Python's own function call (the `METH_FASTCALL` shim).

---

## 5. Summary

| Task | Tool |
|------|------|
| Call a C/runtime function | `frgn ... as ... from "file.c" ...` |
| Expose a Briv function | `export defn ...` |
| Build a linkable library | `brivc build <bridge>.bv --library` |
| C/C++ bindings | `brivc bindings <bridge> c` |
| Native extension (Python/Node/Java/Lua) | `brivc extension <bridge> <lang>` |
| Language package (Go/Java/Rust) | `brivc export <bridge> <lang>` |
| Add a language | a `lib/glue/<lang>/` folder |
| Verify the zero-friction gate | `BRIEF_RUN_GATE=1 cargo test --test gate` |

Deep reference: `docs/architecture/glue-ffi.md` and `docs/guides/ffi-and-export.md`.
