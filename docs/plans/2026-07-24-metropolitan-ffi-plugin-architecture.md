# Metropolitan FFI — Multi-Language Bridge Plugin Architecture
## 2026-07-24

## Goal

Provide a **Briv-native plugin system** for cross-language FFI. Each language
gets a standalone `.bv` plugin that generates the **most efficient bridge** for
that language's runtime. The plugin system is infinitely extensible: adding a
new language = creating one `.bv` file.

## Two Performance Tiers

### Tier 1 — Zero-Cost (LTO-Compatible)
Languages that compile to the C ABI (C, Rust, C++, Zig, etc.) get **zero bridge
overhead** after LTO. The Briv function IS the native function.

| Language | Method | Overhead | Mechanism |
|----------|--------|----------|-----------|
| C | Direct `.so` / `.a` | ~0ns | Same ABI, LTO merges |
| Rust | extern "C" + LTO | ~0ns | Single binary after LTO |
| C++ | extern "C" + LTO | ~0ns | Same as C |
| Zig | extern C ABI | ~0ns | LLVM LTO across languages |

Generator: `gen_c.bv`, `gen_rust.bv`

### Tier 2 — Runtime-Bound
Languages with managed runtimes (Python, JS, Java, Lua, etc.) require crossing a
language boundary. The plugin generates the **most efficient boundary** the
runtime supports.

| Language | Best Method | Overhead | Benchmark Source |
|----------|-------------|----------|------------------|
| Python | C extension (CPython API) | ~150ns | 2026-07-24 multi-lang benchmarks |
| Node.js | koffi FFI ESM | ~280ns | 2026-07-24 multi-lang benchmarks |
| Lua | Lua C API | ~300ns (est.) | |
| Java | JNI | ~500ns (est.) | |
| Shell | Subprocess protocol | ~1-5ms | 2026-07-24 multi-lang benchmarks |
| Any | Protocol bridge (universal) | ~1-5ms | Fallback for all languages |

Generator: `gen_python.bv`, `gen_node.bv`, `gen_protocol.bv`

## Architecture

### File Layout

```
lib/ffi/
  gen_c.bv          # Tier 1: C header + .so export declarations
  gen_rust.bv       # Tier 1: Rust extern "C" crate + LTO
  gen_python.bv     # Tier 2: Python C extension (CPython API)
  gen_node.bv       # Tier 2: Node.js koffi ESM module
  gen_protocol.bv   # Tier 2: Universal subprocess text protocol shim
  metropolitan.bv   # Orchestrator: imports all gen_* plugins
```

### Each Plugin Has Three Parts

```briv
// 1. Type-mapping helpers ($defn functions)
$defn map_type_to_target(briv_type: String) -> String {
    when briv_type == "Int"    { term "native_int_t"; };
    when briv_type == "Float"  { term "native_float_t"; };
    // ...
};

// 2. Code generator ($defn function)
$defn generate_bridge(exports: Selection, bridge_name: String) -> String {
    // Returns generated source code as a string
};

// 3. Stage block (auto-executes when imported)
$(Normalized @ highest) {
    let exports = Tag$("export");
    // ... generate and write files
};
```

### Usage Patterns

```briv
// Single language:
import "lib/ffi/gen_python";

// All languages:
import "lib/ffi/metropolitan";

// Custom: import helpers + write your own stage block
import "lib/ffi/gen_c";
$(Normalized @ highest) {
    let exports = Tag$("export");
    let code = gen_c_header(exports, "my_bridge");
    FileWrite$("out/bridge.h", code, true);
};
```

### Zero-Cost Usage (Tier 1)

For embedding Briv in C/Rust/etc., the most efficient pattern is:

```bash
# Build as a shared library
briv build my_module.bv --shared --out target/
# Link directly with your C/Rust project
gcc -o my_program main.c target/my_module.so
```

The `gen_c.bv` and `gen_rust.bv` generators produce headers/declarations that
match the `.so`'s ABI exactly — no runtime marshalling, no binding overhead.

### Adding a New Language

1. Create `lib/ffi/gen_<lang>.bv`
2. Add `$defn` type-mapping helpers
3. Add `$defn generate_bridge(exports, bridge_name) -> String`
4. Add `$(Normalized @ highest)` stage block
5. Add `import "lib/ffi/gen_<lang>";` to `metropolitan.bv`

No Rust code changes needed. The plugin system is pure-Briv.

## Implementation Status

| Plugin | Status | Overhead | Lines |
|--------|--------|----------|-------|
| `gen_c.bv` | To build | ~0ns | — |
| `gen_rust.bv` | To build | ~0ns | — |
| `gen_python.bv` | Scafolded | ~150ns | ~200 |
| `gen_node.bv` | Scafolded | ~280ns | ~80 |
| `gen_protocol.bv` | To build | ~1-5ms | — |
| `metropolitan.bv` | Scafolded | — | ~150 |

## Infrastructure Changes

The following compiler fixes were required for cross-file plugin support:

- **`import_resolver.rs` `filter_items()`**: Keep `TopLevel::StageBlock`,
  `CompileTimeDefn`, and `CompileTimeTxn` items from imported modules
  (previously filtered out for having no "name").
- **`compile.rs`**: Second `extract_inline_stage_blocks` call after import
  resolution so stage blocks from imported plugins are registered.
- **`src/macros/eval.rs` BinaryOp Eq/Neq**: String comparison for `Str` operands.
  Previously used `nav_to_i64` which parsed `"Int"` as `0`, breaking string
  equality in `when` guards.
- **`src/lexer.rs` `tokenize()`**: Return real span ranges instead of `0..0`.
  The plugin loader, macro eval, and import resolver all use `tokenize()`; zero
  spans caused parser panics on `@counter` and other span-sensitive constructs.

## Key Benchmarks (2026-07-24)

| Language | Transport | Median | vs Native |
|----------|-----------|--------|-----------|
| Python   | ctypes    | 2,106ns | 8.7× |
| Python   | C ext     | 149ns   | — (prev session) |
| Node.js  | koffi     | 281ns   | 2.4× |
| Python   | protocol  | 1.23ms  | 5,067× |
| Node.js  | protocol  | 3.74ms  | 32,241× |
| Shell    | protocol  | 4.76ms  | — |
