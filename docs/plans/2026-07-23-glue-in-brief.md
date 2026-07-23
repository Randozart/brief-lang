# GLUE in Brief — Fully Native Bridge Generation

**Date:** 2026-07-23
**Status:** Plan

---

## Goal

Rewrite the entire GLUE bridge generator pipeline in Brief — as `$defn`
compile-time functions and a `$(Normalized)` stage block — so the bridge
is generated, compiled, and optimized entirely by Brief's compiler.

The hypothesis: **a Brief-native bridge can beat C's linker** for
cross-language call overhead because the compiler understands both sides
and can eliminate the boundary entirely via LTO.

## Current Architecture

```
Rust pipeline (brief export):
  .bv → parse → type-check → LLVM IR → llc → .o
                                   ↓
                          TOML templates → Rust wrappers → rustc → .o
                                   ↓
                                   ld → bridge.so

Brief .bv plugin (current state):
  .bv → Tag$ exports → StrReplace$ templates → FileWrite$ → Rust .rs files
         ↑                                    ↑
    $defn render_fn                    FileWrite$ with persist
```

The `.bv` plugin already does template rendering and file writing in Brief.
The missing pieces: generating Brief bridge code instead of Rust wrappers,
and orchestrating the full compile-link pipeline from within macros.

## Proposed Architecture

```
Brief-native pipeline (brief build with inline stage block):
  .bv → [Normalized stage block runs:]
         1. Tag$ exports → extract bridge info (already done in $defn)
         2. $defn generate_bridge_bv(): emit Brief bridge code
         3. FileWrite$("bridge.bv") → write bridge source
         4. The compiler compiles bridge.bv as part of the project
         5. LTO eliminates the boundary → single optimized binary
         6. FileWrite$("bridge-exports.dbvl") → metadata
```

No Rust wrappers. No `rustc` invocation. No `llc` for the bridge layer.
Just Brief source files that the compiler optimizes as one unit.

## Key Insight: Emit Brief, Not Rust

**Current approach** — emit Rust wrapper source that calls into Brief `.so` via FFI:
```
Brief .so (compiled) ← FFI → Rust wrapper ← FFI → Python/Caller
```
Two FFI boundaries. Caller → ctypes → Rust → ctypes → Brief. Each boundary
has overhead: argument marshaling, ABI conversion, stack frame setup.

**Proposed approach** — emit Brief bridge source, compile as one unit:
```
Caller ← FFI → Brief binary (bridge code inlined by LTO)
```
One FFI boundary. The bridge is just a Brief module that the compiler
compiles alongside the rest of the program. LTO sees everything and can
inline the boundary into a single `call` instruction — identical to what
C's linker produces after LTO.

## Implementation Steps

### Step 1: Bridge Code Generator as `$defn`

Build a `$defn generate_python_bridge(exports: Selection) -> String` that
produces a Python-callable C extension module written in Brief syntax:

```brief
$defn generate_python_bridge(exports: Selection) -> String {
    let mut code = "";
    code = code + "frgn Python_Init(Int) from \"c\" fallback 0;\n";
    code = code + "frgn Python_Arg(Int) -> Int from \"c\" fallback 0;\n";
    code = code + "frgn Python_Return(Int) from \"c\" fallback 0;\n\n";

    foreach(exp in exports) {
        let name = TypeInfo$(exp, "name");
        let pcount_str = TypeInfo$(exp, "params.count");
        let pcount = pcount_str;
        let ret_type = TypeInfo$(exp, "output_type");

        code = code + "defn bridge_" + name + "(";
        // ... build param list ...
        code = code + ") -> " + ret_type + " {\n";
        when pcount > 0 {
            // read each param from Python
            code = code + "    let " + p0n + " = Python_Arg$(0);\n";
        };
        code = code + "    term " + name + "(" + pnames_str + ");\n";
        code = code + "};\n\n";
    };
    term code;
};
```

This generates Brief source files that:
- Declare Python FFI functions as `frgn`
- Define `bridge_<fn>` wrappers that marshal between Python ABI and Brief
- The compiler then compiles these natively with the rest of the project

### Step 2: `$txn`-Driven Protocol Path Resolution

For complex bridge scenarios (types that don't match between languages),
compute the protocol path at compile time:

```brief
$txn resolve_paths(
    exports: Selection,
    lang: String
) [changed > 0][changed == 0] -> List {
    foreach(exp in exports) {
        let ret_type = TypeInfo$(exp, "output_type");
        let native = ConfigGet$(lang, "protocols." + ret_type + ".native");
        let path = CastPath$(ret_type, native);
        // ... accumulate resolved paths ...
    };
    term paths;
};
```

This already works — `CastPath$` is available. The `$txn` form is needed
for iterative path resolution when types reference each other (circular
protocol dependencies).

### Step 3: Compile-and-Link Orchestration

The stage block orchestrates the full pipeline:

```brief
$(Normalized @ highest) {
    // 1. Generate bridge source code in Brief
    let exports = Tag$("export");
    let bridge_code = generate_python_bridge(exports);
    FileWrite$("glue-out/bridge.bv", bridge_code, true);

    // 2. Generate metadata
    let dbvl = generate_dbvl(exports, "python");
    FileWrite$("glue-out/bridge-exports.dbvl", dbvl, true);
    
    EmitInfo$("GLUE bridge generated: bridge.bv");
};
```

The file `bridge.bv` is not compiled by a separate compiler invocation —
it's output for the user to include in their project. The bridge code gets
compiled together with the rest of the project next time `brief build` runs.

**Alternatively**, the bridge code could be injected directly into the
current program via `Insert$` — no file system needed at all:

```brief
let bridge_bv = generate_python_bridge(exports);
let bridge_node = Import$("glue-out/bridge.bv");
Insert$(AppendTo$(All$()), bridge_node);
```

This would make the bridge part of the same compilation unit automatically.

### Step 4: Language-Specific ABI Helpers

Each target language needs a small set of `frgn` declarations for its ABI
(how to read arguments from the calling convention, return values, etc.).
These live in `lib/glue/<lang>.bv`:

```brief
// lib/glue/python.bv
frgn PyArg_ParseTuple(Int, String, ...) -> Int from "c" fallback 0;
frgn Py_BuildValue(String, ...) -> Int from "c" fallback 0;
```

The bridge generator imports these and uses them in the wrapper code.

## Files Touched

| File | Change |
|------|--------|
| `lib/glue/generator.bv` | Add `generate_python_bridge` `$defn`, orchestration stage block |
| `lib/glue/python.bv` | New — Python ABI frgn declarations |
| `lib/glue/rust.bv` | New — Rust ABI frgn declarations (if needed) |
| `lib/glue/node.bv` | New — Node.js ABI frgn declarations |
| (none in `src/`) | All changes are in `.bv` files |

## Measuring Success

Compare against the current pipeline:

| Metric | Current (Rust wrappers) | Proposed (Brief bridge) | Target |
|--------|------------------------|------------------------|--------|
| Bridge call overhead (Python → fn) | ~1.2µs (ctypes → .so) | ~0.8µs (LTO inlined) | Match C's ~0.8µs |
| Compile time | ~2s (llc + rustc) | ~0.5s (single briefc pass) | Beat C |
| Lines of Rust | ~400 (export.rs, config.rs) | 0 | Zero |
| Lines of Brief | ~60 (generator.bv) | ~120 (all helpers) | Manageable |

## Risk: What if LTO Can't Inline?

Even without cross-boundary inlining, the call overhead for a Brief-to-Brief
call is a single `call` instruction (same as C-to-C). The current ~1.2µs
overhead comes from: ctypes marshaling + `dlopen` symbol resolution +
the `state` pointer argument. With the state-optional fix already applied,
the remaining overhead is just ctypes. Moving to a direct Brief binary
eliminates ctypes entirely — the FFI boundary is just the Python C API,
which Brief already handles via `frgn`.
