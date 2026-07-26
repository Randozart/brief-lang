# FFI Simplification: Implementation Plan

**Date:** 2026-04-29
**Status:** ✅ IMPLEMENTED (2026-04-30)
**Related:** `BRIEF_COMPILER_CHECKLIST.md`, `VITRIOL_ARCHITECTURE.md`

---

## Overview

The current FFI system uses complex TOML binding files with per-function definitions. The new model simplifies by:
1. **Importing language profiles** (type mappings, conventions)
2. **Calling functions by name** (no LUT)
3. **Using `@link` syntax** for memory addresses
4. **Global and per-function type overrides** via attributes

---

## FFI Memory Model (Confirmed)

**Core Principle: Brief owns all memory.**

### Memory Ownership
- **Brief allocates** memory for FFI parameters
- **Brief tracks** the memory address
- **Brief deallocates** memory after call (success OR error)
- **Compiler handles** all memory operations automatically

### TOML's Role: Declarative Ruleset
The TOML profile is **declarative only** - it describes rules, not operations:
- Type mappings (`Int → int32_t`)
- Error conventions (bounds checking, null pointer)
- Calling convention (`cdecl`, `stdcall`, `wasm`)
- Memory layout hints (alignment, size)

### Compiler's Role: Operational
The compiler handles all memory operations:
- Memory allocation for parameters
- Writing parameters to memory (via Mapper)
- Generating the function call
- Reading result from memory
- Freeing memory on success or error

### Memory Flow
```
1. Allocate memory for parameters
2. Write params to memory (via Mapper)
3. Track memory address
     ↓
4. Call FFI function
     ↓
5. Read result from memory
6. FREE memory (always - success or error)
```

### Error Handling
- If function returns error → result in `Err`, memory freed
- If function throws exception → caught, memory freed
- If non-void returned when `Void` expected → stored in `Err`

### Implications for Implementation
1. No manual memory management in FFI calls
2. All buffers are scoped to the transaction
3. Memory freed automatically regardless of outcome
4. TOML only defines HOW to map types, not WHEN to free

---

## Syntax Reference

### File-Level Attribute
```brief
#![ffi.<lang>, bind("./<profile>.toml"), import("./<script>")]
```

### Global Type Mapping
```brief
#![ffi.c, map("uint","uint32_t"), map("string","char*")]
#![ffi.js, map("uint","Number"), map("string","String")]
```

### Per-Function Type Override
```brief
#[ffi.c, type("Int32Array")]
frgn get_buffer() -> Result<Data, Err>;
```

### Disambiguation
```brief
#[ffi.c]    frgn printf(fmt: String) -> Result<Int, Err>;
#[ffi.rust] frgn printf(...);
```

### Simple frgn! Fire-and-Forget
```brief
frgn! sync()              // Compiler picks address
frgn! write_reg @ 0x40001000 (val: UInt);
```

---

## Implementation Phases

### Phase 1: TOML Language Profile Schema

**File:** `lib/ffi/profiles/<lang>_profile.toml`

```toml
# JavaScript Profile Example
[language]
name = "JavaScript"
endianness = "native"
pointer_size = 8

[types]
String = { representation = "UTF8", size = "variable" }
Number = { representation = "IEEE754", size = 8 }
Boolean = { size = 1 }
Int = { representation = "IEEE754", size = 8 }

[mapping]
# Default mappings (can be overridden in Brief)
String = "String"
Int = "Number"
UInt = "Number"
Bool = "Boolean"
Void = "void"

[conventions]
# Error handling
null_pointer = 0
error_return = -1

[[functions]]
name = "console_log"
location = "console.log"
description = "Print to console"
```

```toml
# C/Kernel Profile Example (VITRIOL PCIe)
[language]
name = "C/Kernel"
endianness = "little"
pointer_size = 8

[types]
String = { representation = "ASCII", size = "variable" }
Int = { representation = "two's complement", size = 4 }
UInt = { representation = "unsigned", size = 4 }
Void = { size = 0 }

[mapping]
String = "char*"
Int = "int32_t"
UInt = "uint32_t"
Bool = "int"

[conventions]
error.min = 0
error.max = 0xFFFFFFFF
```

**Deliverable:** Create `lib/ffi/profiles/` directory with at least:
- `c_profile.toml` - C/kernel profile with PCIe support
- `js_profile.toml` - JavaScript profile
- `wasm_profile.toml` - WebAssembly profile

---

### Phase 2: Parser Changes

#### 2.1 Parse `#![ffi.<lang>, ...]` Attribute

**Location:** `src/parser.rs`

```brief
#![ffi.c, bind("./c_profile.toml"), import("./libc.a")]
```

**AST Node:**
```rust
struct FfiDirective {
    lang: String,           // "c", "js", "rust", etc.
    bind_path: String,       // "./c_profile.toml"
    import_path: String,     // "./libc.a" or "./functions.js"
    global_maps: Vec<(String, String)>,  // [("uint","uint32_t")]
    type_override: Option<String>,       // Global type override
}
```

#### 2.2 Parse Global Mapping

**Syntax:**
```brief
#![ffi.c, map("uint","uint32_t"), map("string","char*")]
```

**Parser:** Collect `map("from","to")` pairs into `global_maps` vector.

#### 2.3 Parse Function-Level Override

**Syntax:**
```brief
#[ffi.c, type("Int32Array")]
```

**Parser:** Same attribute parser, but for function-level context.

#### 2.4 Update `frgn` Parsing

**Remove:** `from "bindings.toml"` requirement
**Keep:** `-> Result<T, E>` return type

```brief
# Old syntax (still supported):
frgn read_spi(addr: Int) -> Result<Int, Err> from "bindings.toml";

# New syntax:
frgn read_reg @ 0x40001000 (addr: UInt) -> Result<UInt, Err>;
```

**Update `parse_frgn_binding()` to:**
1. Check for `@ address` after function name
2. If no `@`, use function name lookup in imported script
3. Parse `-> Result<T, E>` for return type

---

### Phase 3: Code Generation

#### 3.1 C Backend (`src/backend/c.rs`)

**Generate function calls:**
```c
// Brief:
frgn pci_iomap(dev: UInt, bar: UInt, len: UInt) -> Result<UInt, Err>;

// Generated C:
uint32_t dev = state->dev;
uintptr_t addr = pci_iomap(dev, bar, len);
if (addr == NULL) {
    return (Err){ .code = -1, .message = "pci_iomap failed" };
}
```

#### 3.2 Mapper System Updates

**Keep existing `Mapper::drop()/fetch()`** for data serialization, but:
1. Use type mappings from language profile
2. Apply global `map()` overrides
3. Apply per-function `#[ffi.c, type("...")]` overrides

#### 3.3 Address Assignment for `frgn!`

**Compiler picks address if not specified:**
```brief
frgn! sync()  // Compiler assigns address
```

**Implementation:** Maintain address counter, increment per `frgn!` call.

---

### Phase 4: Script Import System

#### 4.1 Import Resolver Updates

**Location:** `src/import_resolver.rs`

```rust
fn resolve_script_import(path: &str) -> Result<String, ImportError> {
    // For JS: Return script content
    // For C: Return library path for linker
    // For WASM: Return module path
}
```

#### 4.2 Function Resolution

**No LUT.** Instead:
1. Parse imported script for function signatures
2. Match `frgn funcname` to function by exact name
3. If ambiguous → compiler error with disambiguation hint

#### 4.3 Cache System

**Goal:** Cache compiled foreign functions.

```rust
struct FfiCache {
    functions: HashMap<String, CompiledFunction>,
}

impl FfiCache {
    fn get(&mut self, name: &str) -> Option<&CompiledFunction> {
        self.functions.get(name)
    }
    
    fn insert(&mut self, name: String, func: CompiledFunction) {
        self.functions.insert(name, func);
    }
}
```

---

### Phase 5: Error Handling

#### 5.1 Built-in `Err` Type

```rust
enum Err {
    IoError { code: Int, message: String },
    MappingError { expected: String, got: String },
    BoundsError { min: Int, max: Int, value: Int },
    VoidError { return_value: String },  // For Result<Void, Err>
}
```

#### 5.2 Error Generation

**TOML conventions:**
```toml
[conventions]
error.null = 0
error.valid_range = { min = 0, max = 32 }
```

**Code generation:**
```c
// Check bounds
if (result < 0 || result > 32) {
    return (Err){ .tag = BoundsError, .min = 0, .max = 32, .value = result };
}
```

#### 5.3 `Result<Void, Err>` Verification

For `frgn!` calls expecting `Void`:
```brief
frgn! sync() -> Result<Void, Err>;
```

**Compiler generates:**
```c
// Verify void return
void* result = sync();
if (result != NULL) {
    return (Err){ .tag = VoidError, .return_value = result };
}
```

---

### Phase 6: Backward Compatibility

#### 6.1 Keep Existing TOML

Existing `bindings.toml` files continue to work:
- `#![ffi.c, import("./io.toml")]` still loads full function catalog
- `frgn func(...) -> Result<T, E> from "io.toml"` still works

#### 6.2 Migration Path

1. **Phase 1-2:** Implement new syntax alongside old
2. **Phase 3+:** Gradually migrate stdlib to new model
3. **Deprecation:** Mark old `from "bindings.toml"` as deprecated
4. **Removal:** After verification, remove old system

---

## File Changes Summary

| File | Change |
|------|--------|
| `src/parser.rs` | Parse `#![ffi.<lang>, ...]` attribute |
| `src/parser.rs` | Parse `map("from","to")` syntax |
| `src/parser.rs` | Update `parse_frgn_binding()` for `@ address` |
| `src/ast.rs` | Add `FfiDirective`, `FfiProfile` structs |
| `src/backend/c.rs` | Generate calls with type mappings |
| `src/ffi/mod.rs` | Add profile loading |
| `src/ffi/profiles/*.toml` | New language profiles (created) |
| `src/import_resolver.rs` | Resolve script imports |
| `lib/ffi/profiles/` | New directory for profile TOMLs (created) |

---

## Profile Files Created

| Profile | Path | Purpose |
|---------|------|---------|
| C/Kernel | `lib/ffi/profiles/c_profile.toml` | PCIe, kernel modules, low-level C |
| JavaScript | `lib/ffi/profiles/js_profile.toml` | Browser JS, Node.js, WASM |
| WebAssembly | `lib/ffi/profiles/wasm_profile.toml` | WASM modules, WASI |

---

## Next Steps

1. **Parser changes** - Implement `#![ffi.<lang>, ...]` parsing
2. **Type mapping system** - Global `map()` and per-function `#[ffi.c, type("...")]` overrides
3. **Script import** - Resolve and execute imported scripts
4. **Address assignment** - Compiler picks addresses for `frgn!`
5. **Error bounds** - Implement TOML conventions in code generation
6. **Backward compatibility** - Keep existing TOML working during migration

**Status:** Schema complete. Ready for parser implementation.

---

## Open Questions

1. **Script Execution:** How does Brief execute imported JS scripts at runtime?
2. **Cache Invalidation:** When to recompile foreign functions?
3. **Err Enum Definition:** Final schema for built-in `Err` type variants?
