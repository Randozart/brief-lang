# Metropolitan FFI — Foreign Function Interface

Brief's **Metropolitan FFI** is the umbrella architecture for all cross-language
interoperability. It has two mechanisms:

| Mechanism | When | What it does |
|-----------|------|-------------|
| **GLUE** | Compile time | Exports Brief functions as native wrappers (Rust crate, Python module, Node module) |
| **Metropipe** | Runtime | Shared memory IPC between running processes |

Both use the same `frgn` declaration syntax for importing foreign functions,
and the same `export defn` syntax for exporting Brief functions.

---

## 1. `frgn` — Importing Foreign Functions

Declare functions from C, Rust, Python, or any other language:

```brief
// C function: int64_t getenv_brief(const char* key);
// Brief calls it "frgn__getenv_brief"
frgn __getenv_brief(key: String) -> String as frgn__getenv_brief
  from "lib/runtime/brief_rt.c" fallback "";
```

**The Syntax:**

```
frgn <C_symbol>(<params>) [-> <ret>] [as <brief_name>] from <source> [fallback <expr>];
```

| Part | Meaning |
|------|---------|
| `frgn` | Keyword — this is an import declaration |
| `__getenv_brief` | The **C/foreign symbol name** — what the linker sees |
| `(key: String)` | Parameters with Brief types |
| `-> String` | Return type (optional, defaults to void) |
| `as frgn__getenv_brief` | Brief-side name — what Brief code uses at call sites |
| `from "..."` | **Required** — provenance of the foreign module (path or registry name) |
| `fallback ""` | Value to use if the foreign function cannot be called |

### Calling Imported Functions

Imported functions resolve through the GLUE pipeline and produce either inline
calls (C/Rust sources) or bridge calls (Python/JavaScript):

```brief
import "std/env.bv";

txn print_env() [true][true] {
    let home = frgn__getenv_brief("HOME");
    // Result type handling...
    [home.is_ok()] {
        frgn__print_str(home.value);
    };
};
```

### Common Patterns

```brief
// C symbol has no underscore prefix — use `as` for clear naming
frgn XXH64(data: Int, len: Int, seed: Int) -> Int as frgn__xxh64
  from "lib/xxhash.c" fallback 0;

// Void return — no `->` needed
frgn log_message(msg: String) from "lib/runtime/brief_rt.c" fallback;

// Result type with custom error
frgn read_file(path: String) -> Result<String, IOError>
  from "lib/runtime/brief_rt.c" fallback Err(IOError { message: "" });
```

### Naming Convention

FFI declarations use the C/foreign symbol name as the first identifier,
optionally renamed with `as`:

```brief
frgn __getenv_brief(key: String) -> String as get_env from "lib/runtime/brief_rt.c" fallback "";
```

The `as` clause provides the Brief-side name. When omitted, the C symbol
name is used directly. There is no required `frgn__` prefix convention —
use descriptive Brief-style names in the `as` clause:

```brief
frgn XXH64(data: Int, len: Int, seed: Int) -> Int as hash64 from "link/xxhash/xxhash.c" fallback 0;
```

### `from` Sources

| Source form | Example | What happens |
|-------------|---------|-------------|
| `from "path/to/file.c"` | `from "lib/runtime/brief_rt.c"` | Compiles C source and links it |
| `from "<registry_name>"` | `from "<xxhash.c>"` | Resolves via TypeUniverse registry |
| `from "link/library.so"` | `from "link/libm.so.6"` | Links to system library |

---

## 2. `export defn` — Exporting Brief Functions

Expose a Brief function so foreign code can call it:

```brief
export defn brief_pp_type(n: String) -> String {
    term pp_type(n);
};
```

The `export` keyword creates a `dso_local` symbol that's visible to the linker.
At LTO time, LLVM can inline across the boundary — zero-cost calls.

### Exporting for Specific Languages

Use the `brief export` CLI to generate language-specific wrappers:

```bash
# Generate a compilable Rust crate with safe wrappers
brief export my_bridge.bv rust --out ./rust-crate

# Generate a ctypes Python module
brief export my_bridge.bv python --out ./py-module

# Generate an ffi-napi Node.js module
brief export my_bridge.bv node --out ./node-module
```

Adding a new language = adding a `[lang]` section to `lib/glue.toml` — zero
Rust changes.

### How Export Works

The `brief export` command:

1. Compiles the bridge `.bv` file through the **full LLVM backend** (real
   function bodies, no `ret i64 0` stubs)
2. Reads the language target's configuration from `lib/glue.toml`
3. For each exported function, generates:
   - A **safe wrapper** using the language's native types (from `protocols`)
   - An **FFI declaration** using the C ABI types (from `c_abi` fields)
   - **Conversion expressions** when native and C ABI types differ
4. Outputs a complete crate/module with the bridge `.so`

The generated wrappers handle:
- State allocation and initialization (`init_state()`)
- Type conversion between native and C ABI representations
- Proper `state` pointer passing to every exported function

---

## 3. Protocol-Driven Type Mapping

The type mapping between Brief types and foreign types is driven by
**protocol categories** (`#String`, `#Int`, `#Float`, `#Bits`), not by
Brief-type-specific rules.

**In `lib/glue.toml`:**
```toml
[rust.protocols]
"#String" = { native = "str", c_abi = "i64" }
"#Int" = { native = "i64", c_abi = "i64" }
"#Float" = { native = "f64", c_abi = "double" }
```

**Resolution flow:**
1. A Brief `String` parameter has `CastTo(#String)` in the type universe
2. The protocol is `#String` → look up in TOML → native = `"str"`, c_abi = `"i64"`
3. The wrapper uses `str` as the parameter type, the FFI uses `i64`
4. The conversion is: `n as i64` (pointer → integer)

For **Rust** (calling convention `"lto"`), the generated wrapper:

```rust
pub fn brief_pp_type(n: *mut u8) -> *mut u8 {
    unsafe { ffi::brief_pp_type(STATE, n as i64) as *mut u8 }
}
```

The `protocols` section replaces the old `type_map` + `c_type_map` + `conversions`
system — only protocol categories appear in the TOML.

---

## 4. Protocol Path Optimization

When a frgn call crosses a language boundary, the BFS in `find_cast_path()`
computes the cheapest transform chain. If both sides speak the same protocol
with compatible layouts, the boundary compiles to **zero instructions**:

```
Brief String (SSO {i64,i64}) → #String → Rust &str ({ptr, len})
  Step 1: CastTo(#String) — identity (both layouts represent UTF-8)
  Step 2: CastFrom(#String) — identity (both are {ptr, len} UTF-8)
  Total cost: 0 → LLVM eliminates the boundary at LTO time
```

If the types differ in layout, the bridge emits the necessary transforms:

| Transform | When | IR Emitted |
|-----------|------|-----------|
| **Identity** | Same layout | Nothing |
| **Bitcast** | Same byte width | `%r = bitcast T1 %v to T2` |
| **MeldShuffle** | Field reordering | `extractvalue`/`insertvalue` |
| **ProtocolTransform** | Real conversion needed | `call @_CastTo_#Cat(T %v)` |

---

## 5. Metropipe (Runtime Shared Memory)

For inter-process communication at runtime, use **Metropipe**:

```brief
import "std/metro_bridge.bv";

txn exchange_data() [true][true] {
    let ch = frgn__metro_create_channel("my_channel", 1024, 1024);
    frgn__mmap_write(addr, 0, data, length);
    frgn__atomic_store_u32(addr, 0, 1);  // signal readiness
};
```

Metropipe provides:
- Shared memory segments with atomic operations
- Signal triggers for data availability notification
- Consensus protocol for multi-process coordination

---

## 6. Target Integer Width (`--int-bits`)

The `--int-bits <N>` CLI flag controls the target integer width for compiled
output. Supported values:

| Value | Effect |
|-------|--------|
| `64` (default) | All `Int` values are i64. Best performance on 64-bit hosts. |
| `32` | All `Int` values are i32. Required for WASM targets (avoids BigInt). |
| `16` | All `Int` values are i16. For embedded targets. |
| `8` | All `Int` values are i8. For tiny embedded targets. |

The narrowing pass (contracts) can prove narrower widths — the declared
`--int-bits` is the maximum integer width the backend will use.

**Example for WASM:**
```bash
briefc build myfile.bv --llvm --int-bits 32
```

This emits i32 for all `Int` values, avoiding JavaScript BigInt
interop issues.

---

## 7. Summary

| Task | Syntax | Tool |
|------|--------|------|
| Call a C function | `frgn ... as ... from "file.c" ...` | Compiler |
| Call a Python function | `frgn ... as ... from "file.py" ...` | GLUE bridge |
| Export to Rust | `export defn ...` + `brief export ... rust` | GLUE |
| Export to Python | `export defn ...` + `brief export ... python` | GLUE |
| Process IPC | `import "std/metro_bridge.bv"` | Metropipe |

The `frgn` and `export` syntax gives you import/export for any language.
The protocol system ensures type safety at the boundary. The GLUE bridge
handles code generation for the wrapper. Metropipe handles runtime IPC.
All under the **Metropolitan FFI** umbrella.
