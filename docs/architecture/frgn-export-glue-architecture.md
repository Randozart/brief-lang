# frgn / export / GLUE — Cross-Language FFI Architecture

**Date:** 2026-07-22
**Status:** Foundational
**Applies to:** All backends, `src/glue/`, `src/compile.rs`, `src/ast/top.rs`, `src/parser/definitions.rs`, `lib/glue.toml`

---

## Overview

Brief's FFI architecture has three pillars:

| Pillar | Direction | Purpose |
|--------|-----------|---------|
| **`frgn`** | Host → Brief | Declare an external function so Brief can call it |
| **`export`** | Brief → Host | Expose a Brief function so foreign code can call it |
| **GLUE** | Broker | Negotiate protocol paths, meld data shapes, and generate bridge code when no direct path exists |

The backend decides *how* to implement each frgn or export. A `.c` source gets
compiled and LTO-inlined by LLVM. A `.py` source cannot be inlined by LLVM —
GLUE mediates through protocol negotiation and zero-copy melds. The frontend
resolves the dispatch path (inline vs bridge vs error) during the main
compilation pass, before the backend runs. The backend merely executes the
semantics of the chosen path.

---

## 1. `frgn` — Foreign Function Declaration

### 1.1 Syntax

```brief
frgn <brief_name>(<params>) -> <ret> as <foreign_symbol> from "<path>"
    fallback <expr>;
frgn <brief_name>(<params>) -> <ret> as <foreign_symbol> from <<registry_name>>
    fallback <fn_name>(<args>);

// Minimal (as = brief_name, no fallback = zero-value or compile error if required)
frgn log(msg: String) from "syslog.so";

// Full
frgn __get_user(id: Int) -> User as get_user from "db.py"
    fallback User { name: "unknown", id: 0 };
```

### 1.2 Grammar

```
frgn_decl ::= "frgn" identifier "(" [param_list] ")" ["->" type]
              ["as" identifier]
              "from" (string_literal | "<" identifier ">")
              ["fallback" (expr | identifier "(" [arg_list] ")")]
              ";"

param_list ::= param ("," param)*
param     ::= identifier ":" type
```

### 1.3 Fields

| Field | Required | Meaning |
|-------|----------|---------|
| `brief_name` | Yes | The name used in Brief code to call this function |
| `params` | Yes | Parameter names and types |
| `-> ret` | No | Return type. Omitted = void |
| `as foreign_symbol` | No | Override the foreign symbol name. Without it, symbol = brief_name |
| `from "path"` | **Yes** | Origin of the foreign function. Must always be present |
| `from <name>` | Yes | Compiler-resolved registry path (e.g., `<xxhash.c>`) |
| `fallback expr` | No | Static fallback value if the foreign call fails contract |
| `fallback fn(args)` | No | Brief function to call as fallback |

**`from` is mandatory.** There is no default. Every frgn must specify where
the foreign function lives. This avoids ambiguity about symbol resolution,
linking strategy, and dispatch path.

### 1.4 `as` — Symbol Rename

`as` is exclusively a symbol rename. It does NOT denote protocol, language,
or any semantic property. The foreign symbol name is `as <ident>`; the Brief
name is `brief_name`. This lets you write:

```brief
// C symbol is "func", Brief calls it "__func"
frgn __func(x: Int) -> Int as func from "lib.so";

defn func(x: Int) -> Int {
    // Brief-native wrapper around the frgn
    term __func(x) + 1;
};
```

Without `as`, the foreign symbol is assumed to equal the Brief name.

### 1.5 Fallback — Graceful Degradation

Every frgn call must produce a valid result even when the foreign world is
unreachable. The `fallback` clause declares what happens when the foreign
call returns a value that violates the expected contract, or when the call
cannot be completed.

**Three forms:**

| Form | Syntax | When it fires |
|------|--------|---------------|
| Static literal | `fallback <expr>` | Contract violation on return, or bridge failure |
| Function call | `fallback <fn_name>(<args>)` | Same, but calls a Brief function |
| Implicit | omitted | Zero-value for the return type (void frgn: skip the call) |

**How it works at codegen:**

```llvm
%result = call @bridge_call(@func, %transformed_args...)
%ok = call @verify_contract(%result, %expected_postcondition)
br i1 %ok, label %merge, label %try_fallback

try_fallback:
  %fallback_result = call @fallback_fn_or_literal(...)
  br label %merge

merge:
  %final = phi [%result, %continue], [%fallback_result, %try_fallback]
```

The contract verification checks the postcondition declared by the frgn's
return type. If the foreign return violates `[result > 0]`, the fallback
fires. This is the existing contract system — not a new mechanism.

---

## 2. Backend Extension Dispatch

### 2.1 The Decision Tree

Every backend has a baked-in (but TOML-exposed) table of which file extensions
it can inline directly, which require a GLUE bridge, and which are unsupported.

```
frgn x(args) -> Ret as sym from "path.ext"
                      │
                      ▼
  ┌─ extensions_inlineable.contains("ext")? ────────┐
  │ YES                                              │
  │   → Emit direct call to @sym                    │
  │   → Collect path for linking (compile if .c)     │
  │   → (LLVM: .c compiled via clang, .so via dlsym) │
  └──────────────────────────────────────────────────┘
  │
  ┌─ extensions_bridge.contains("ext")? ─────────────┐
  │ YES                                              │
  │   → Find protocol mapping for params/ret types   │
  │   → Generate CastTo/CastFrom transform chain     │
  │   → Emit GLUE bridge call                        │
  └──────────────────────────────────────────────────┘
  │
  └─ NO ─────────────────────────────────────────────┐
    → Compile error:                                  │
      "Backend '{name}' cannot handle extension       │
       '{ext}' for frgn '{brief_name}'."              │
    └─────────────────────────────────────────────────┘
```

### 2.2 Per-Backend Extension Maps

| Backend | Inline | Bridge | Unsupported |
|---------|--------|--------|-------------|
| **LLVM** | `.c`, `.cpp`, `.cc`, `.cxx`, `.m`, `.so`, `.dylib`, `.a`, `.o` | `.py`, `.pyc`, `.js`, `.ts`, `.mjs`, `.java` | Everything else |
| **Webstack** | `.js`, `.ts`, `.mjs`, `.wasm` | `.c`, `.py`, `.rs`, `.java` | Everything else |
| **CIRCT** | *(none)* | *(none)* | **All frgn** (hardware validator B5002) |
| **SPIR-V** | `.spv`, `.ptx`, `.cubin` | `.c`, `.py` | Everything else |

These maps are baked into each backend's `dispatch_frgn()` method, but also
exposed via `config/targets.toml` for documentation and debugging:

```toml
[backend.llvm.frgn]
inline = ["c", "cpp", "cc", "cxx", "m", "so", "dylib", "a", "o"]
bridge = ["py", "pyc", "js", "ts", "mjs"]
```

### 2.3 The `ResolvedFrgn` Enum

The dispatch decision is made during the main compilation pass (in
`src/compile.rs`), **before the backend runs**. The backend receives a fully
resolved dispatch instruction:

```rust
pub enum ResolvedFrgn {
    /// Backend compiles and links the source, calls the symbol directly
    Inline {
        /// The LLVM symbol name (from `as` or brief_name)
        symbol: String,
        /// If true, compile source to .o before linking
        compile_source: bool,
    },
    /// Route through GLUE bridge with protocol negotiation
    Bridge {
        /// Language identifier from lib/glue.toml
        language: String,
        /// Protocol transform chain for each argument and return
        /// (Brief type, foreign type, transform cost)
        protocol_paths: Vec<ProtocolStep>,
        /// Fallback strategy
        fallback: Fallback,
    },
    /// Compile error — extension not supported by this backend
    Unsupported(String),
}
```

### 2.4 Protocol Path Resolution

For each parameter and return type in a bridged frgn, the compiler finds the
shortest path through the protocol graph using the existing `find_cast_path()`
BFS in `src/type_universe/operators.rs`:

```
Source = Brief String
Target = PythonString

BFS finds: [String, #String<utf8>, PythonString]
  ├─ String.CastTo(#String<utf8>)     → identity (already UTF-8)
  └─ PythonString.CastFrom(#String<utf8>) → utf8_to_ucs4 (actual work)

Alternative: meld exists between String and PythonString?
  → Yes: [String, PythonString] via meld → cost 0 if identity
```

The compiler picks the shortest path by cost:
1. Meld identity = cost 0
2. CastTo/CastFrom with identity ops = cost 0
3. CastTo/CastFrom with real transforms = cost N (inlined, LLVM-optimized)
4. Implicit Cast(#Bits) = last resort

---

## 3. GLUE — The Zero-Copy FFI Broker

### 3.1 What GLUE Does

GLUE mediates between a Brief program and a foreign language when the backend
cannot inline the foreign code directly. It:

1. **Negotiates protocol paths** — finds CastTo/CastFrom chains through shared
   protocols (e.g., `#String<utf8>`) for each parameter and return type
2. **Resolves melds** — finds structural identity between Brief types and
   foreign types, enabling zero-copy passthrough
3. **Generates bridge calls** — emits the transform chain, calls the foreign
   function via the appropriate mechanism (dlopen, Python embedding, etc.),
   and unwraps the result
4. **Applies fallbacks** — wraps the call in contract verification with
   fallback dispatch

### 3.2 Bridge Call Generation

For `FrgnDispatch::Bridge { language: "python" }`, the backend emits:

```
For each argument:
  1. Find protocol path from Brief type → foreign type
  2. Emit the CastTo/CastFrom transform chain (inlined)
  3. If a meld exists and is identity → zero-cost passthrough

Call the foreign function:
  1. Via the mechanism declared in lib/glue.toml for this language
     (native_module, esm_module, jni_c_extension, etc.)

For the return value:
  1. Find protocol path from foreign type → expected Brief type
  2. Emit the reverse Cast/CastFrom transform chain

Wrap everything:
  1. Contract verification on the return value
  2. Fallback dispatch if contract is violated
```

### 3.3 Zero-Copy Conditions

Zero-copy happens when meld identity eliminates the need for protocol transforms:

```
Case 1: Same layout, same protocol
  PythonBytes { ptr, len } = CBuffer { ptr, len }
  Both CastTo/CastFrom #String are identity
  → Bridge is literally: call @process(%arg_data)
  → No transform, no copy

Case 2: Different layout, same protocol
  PythonBytes { ptr, len, refcount } vs CBuffer { ptr, len }
  Meld extracts ptr+len from the foreign struct
  → Bridge performs struct reshape (meld shuffle)
  → No encoding transform, only pointer arithmetic

Case 3: Different protocol encoding
  PythonString is UCS-4 internally
  Brief String is UTF-8 internally
  → Encode transform required via CastTo/CastFrom
  → LLVM inlines it; cannot be eliminated (real work)
```

### 3.4 The GLUE Bridge Is Not a Process

The bridge is **inline codegen**, not a separate runtime process. The compiler
emits the transform chain (which LLVM optimizes), wraps the foreign call, and
includes the fallback. No interpreter, no serialization, no IPC.

---

## 4. `export` — Brief Dressed Up as the Foreign Language

### 4.1 Syntax

```brief
export defn <name>(<params>) -> <ret> { <body> };
export node <name> [<pre>][<post>] { <body> };
```

### 4.2 The Export Mechanism is Configured Per Language

The `lib/glue.toml` registry declares how each language wants to receive
exported functions:

```toml
[python]
types_module = "glue/python/types.bv"
extension = "py"
bridge_kind = "native_module"    # Generate a Python .py module
calling_convention = "c_abi"     # Use ctypes.CDLL on a .so

[node]
types_module = "glue/node/types.bv"
extension = "mjs"
bridge_kind = "esm_module"       # Generate an ES module
calling_convention = "c_abi"     # Use Node FFI on a .so

[rust]
types_module = "glue/rust/types.bv"
extension = "rs"
bridge_kind = "extern_c_crate"   # Generate Cargo crate with extern "C"
calling_convention = "lto"       # LLVM LTO — no C ABI boundary
```

Generated output (what `brief export <bridge.bv> <lang> --out <dir>` produces):

| Language | Files produced |
|----------|---------------|
| Python | `<name>/__init__.py` (ctypes), `<name>/bridge.so` |
| Node | `<name>/index.mjs` (FFI), `<name>/index.d.ts`, `<name>/bridge.so` |
| Rust | `<name>/Cargo.toml`, `<name>/build.rs`, `<name>/src/lib.rs`, `<name>/src/ffi.rs`, `<name>/libbridge.a` |

### 4.3 Calling Convention Decision

```
export defn add(a: Int, b: Int) -> Int { ... }
                          │
                          ▼
  ┌─ Does config say "lto"? ───────────────────────┐
  │ YES (Rust, C, Zig, Swift via LTO)              │
  │   → Emit dso_local wrapper with C ABI          │
  │   → Compile to .ll → .o → .a                  │
  │   → Foreign linker resolves via LTO            │
  └────────────────────────────────────────────────┘
  │
  ┌─ Does config say "c_abi"? ─────────────────────┐
  │ YES (Python ctypes, Node FFI, Java JNI)        │
  │   → Same dso_local wrapper with C ABI          │
  │   → Compile to .so via clang -shared           │
  │   → Generate wrapper module for the language   │
  └────────────────────────────────────────────────┘
```

The LLVM backend always emits `dso_local` C-ABI wrappers for `export` functions.
The difference is:
- **LTO path**: Foreign build system links the `.a` directly, LLVM can inline
  across the boundary
- **C ABI path**: Foreign runtime loads a `.so` via FFI (dlopen), calls go
  through the C ABI wrapper

---

## 5. The TOML Registry

### 5.1 `lib/glue.toml`

Replaces the old `glue.dbvl` as the compiler's built-in registry. Shipped with
the compiler, with project-level override capability matching `targets.toml`'s
pattern.

```toml
# lib/glue.toml
[python]
types_module = "glue/python/types.bv"
extension = "py"
bridge_kind = "native_module"
calling_convention = "c_abi"

[python.c_type_map]
Int = "int64_t"
Float = "double"
Bool = "bool"
String = "cstring"

[node]
types_module = "glue/node/types.bv"
extension = "mjs"
bridge_kind = "esm_module"
calling_convention = "c_abi"

[node.c_type_map]
Int = "int64_t"
Float = "double"
Bool = "bool"
String = "cstring"

[rust]
types_module = "glue/rust/types.bv"
extension = "rs"
bridge_kind = "extern_c_crate"
calling_convention = "lto"
```

### 5.2 Per-Language Type Files (`lib/glue/<lang>/types.bv`)

These declare foreign type representations in Brief's type universe. They are
regular `.bv` files that define types, ops (CastTo/CastFrom), and melds for
the foreign language's data model.

```brief
// lib/glue/python/types.bv
// Python types declared in Brief's type universe

type PyBytes <: Bits {
    bytes <~ 8;
    alignment <~ 8;
};

type PyString <: Bits {
    bytes <~ 16;   // PyObject header + { ptr, len, refcount, hash }
    alignment <~ 8;
    // Python str is UCS-4 internally
    op CastTo(#String<utf8>) = ucs4_to_utf8(#L);
    op CastFrom(#String<utf8>) = utf8_to_ucs4(#L);
    op Cast(#String<ascii>) = ucs4_to_ascii(#L);
    op CastFrom(#String<ascii>) = ascii_to_ucs4(#L);
};

type PyInt <: Bits {
    bytes <~ 8;
    alignment <~ 8;
    op CastTo(#Int) = pylong_to_i64(#L);
    op CastFrom(#Int) = i64_to_pylong(#L);
};

// Melds to known types where structurally identical
meld PyBytes -> CBuffer {
    ptr -> ptr;
    len -> len;
};
```

These files are versioned alongside the compiler in `lib/glue/`. They are NOT
loaded automatically — only when a frgn/export references them, or when GLUE
resolves them from the registry.

---

## 6. Layout Optimization — "Become the Foreign"

### 6.1 Principle

When data crosses a language boundary, the most efficient path is the one
that requires the least transformation. If Brief can adopt the foreign
language's data layout for data that crosses the boundary, the protocol
transform at the boundary may become identity — zero cost.

### 6.2 How the Optimizer Works

The optimizer (proposed pass, run after normalization, before codegen):

```
For every data value that crosses a frgn/export boundary:

1. Compute the protocol path (CastTo → CastFrom)
   → This tells us the transform cost

2. Check if a meld exists between source and target
   → If meld is identity (all fields match structurally):
     cost = 0, zero-copy

3. If no identity meld, consider:
   "What if I stored this data in the target's layout instead?"
   → Compute the reverse meld from Brief → Foreign layout
   → Evaluate whether adopting the foreign layout eliminates
     the protocol transform at the boundary
   → If the net cost is lower (or zero), specialize the type
```

### 6.3 Example

```brief
// Brief String = { ptr: Int, len: Int }, UTF-8 encoding
// PythonString = { ptr: Int, len: Int, rc: Int, hash: Int }, UCS-4 encoding

// Protocol path: BriefString → #String<utf8> → PythonString
// Cost: CastTo = identity, CastFrom = utf8_to_ucs4 (real work)
//   OR (reverse): PythonString → #String<utf8> → BriefString
//   Cost: CastTo = ucs4_to_utf8, CastFrom = identity (also real work)

// Optimizer asks: "What if Brief used PythonString layout for boundary data?"
// → Define BoundString with { ptr, len, rc, hash } layout
// → Melds to PythonString with identity
// → Brief internal operations on BoundString use CastTo(#String<utf8>) = ucs4_to_utf8
//   or preserve UCS-4 internally
// → The boundary transforms: identity (meld) instead of encode transform
// → If LLVM can prove the UCS-4 storage is used locally, it may eliminate
//   the ucs4_to_utf8 / utf8_to_ucs4 round trip entirely
```

### 6.4 What Makes This Possible Now

The protocol + meld system already provides:

1. **`CastTo(#Category)` / `CastFrom(#Category)`** — transforms through a shared protocol
2. **`meld Source -> Target { fields }`** — structural mapping, identity or shuffle
3. **`find_cast_path()` BFS** — shortest path through the protocol graph
4. **`alwaysinline`** on op bindings — LLVM eliminates redundant transforms
5. **Contract verification** — validates round-trip correctness

The optimizer is a **new analysis pass** (not in codegen) that proposes layout
specialization based on call-graph analysis. It reuses all existing
infrastructure. See the plan document for implementation details.

---

## 7. Edge Cases

### 7.1 `frgn` with no `from`

Rejected by the parser. `from` is mandatory. This avoids ambiguity about
linking strategy, symbol resolution, and dispatch path.

### 7.2 `frgn` calling a `#` intrinsic

If a `frgn` name matches a compiler `#` intrinsic, the compiler should prefer
the intrinsic. The `link.rs` module already cross-references against known
intrinsics — this same logic applies at codegen time. When the Brief name
matches an intrinsic (e.g., `Sqrt#`), emit `intrinsic_call#()` instead of a
frgn call.

### 7.3 Circular melds

If `meld A -> B { ... }` and `meld B -> A { ... }` both exist, the compiler
uses the shortest path and does not loop. The BFS in `find_cast_path()` tracks
visited nodes.

### 7.4 Type does not exist in foreign language

If a Brief type has no corresponding foreign type and no protocol path exists,
the bridge path fails with a clear error:
```
error: no protocol path from 'CustomType' to 'python' target.
  Required by frgn 'process' in bridge.bv.
  Consider adding a meld or CastTo/CastFrom declaration.
```

### 7.5 `export` with no language target

If `brief export <bridge.bv> <language>` specifies a language not in
`lib/glue.toml`, the compiler returns:
```
error: unknown export target 'kotlin'. Add an entry to lib/glue.toml
  or provide a project-level glue configuration.
```

### 7.6 Interpreter can't call frgn

The interpreter's `dispatch_ffi()` remains a stub — it cannot load native
libraries. This is correct: the interpreter is for compile-time evaluation,
not runtime FFI. A frgn call during interpretation either:
- Falls back to the declared `fallback` value
- Returns an error if no fallback and the interpreter cannot resolve it

---

## 8. Relationship to Existing Systems

| System | Role in frgn/export/GLUE |
|--------|------------------------|
| **Protocol system** (`#String<utf8>`) | The shared vocabulary for type negotiation. Links foreign types to Brief types through common protocols |
| **CastTo/CastFrom** | Transforms between a concrete type and a protocol. `alwaysinline` lets LLVM eliminate redundant round-trips |
| **Melds** | Structural type compatibility. Identity melds = zero-copy at boundaries |
| **`find_cast_path()` BFS** | Finds the shortest protocol path from source to target |
| **Contract system** (`[pre][post]`) | Fallback detection — if the foreign return violates the postcondition, the fallback fires |
| **Operator defs** (`op Add(#Int)`) | Declares how types interact with protocols. Foreign types declare `op CastTo(#String)` to participate |
| **`verify_roundtrips()`** | Validates that Cast/CastFrom chains are round-trip correct |
| **`config/targets.toml`** | Backend capability hints — which protocols are supported |

---

## 9. CLI Subcommands

### 9.1 `brief export <bridge.bv> <language> --out <dir>`

```
1. Parse + typecheck bridge.bv
2. Extract ExportDecl items from TopLevel::Export
3. Find language adapter in lib/glue.toml
4. Generate LLVM IR wrappers via library mode codegen
5. Compile to .ll → .o → .so/.a
6. Generate native wrapper module (Python __init__.py, Rust crate, etc.)
7. Write bridge-exports.dbvl metadata alongside the compiled module
```

### 9.2 `brief link <library.so/a/o> [--out <dir>]`

```
1. Run nm --defined-only -g on the library
2. Extract T (text) symbols
3. Cross-reference against known # intrinsics
4. Generate .bv file with frgn declarations (or intrinsic_call wrappers)
5. Output .bv for the user to import
```

### 9.3 `.dbvl` as output format

The `.dbvl` format is retained as an **output** format for `bridge-exports.dbvl`
metadata. It is consumed by foreign build systems (build.rs, setup.py, etc.).
The compiler's registry is TOML (`lib/glue.toml`), but the output metadata
remains DBVL for machine consumption. The `dbvl_reader`/`dbvs_validator`
modules remain for this purpose.
