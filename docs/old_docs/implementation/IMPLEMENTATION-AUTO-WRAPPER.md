# Auto-wrapper Generation System - Implementation Plan

## Overview

Add `brief map` and `brief wrap` commands to automatically generate FFI bindings for foreign libraries. The system analyzes libraries and generates Brief definitions with suggested contracts.

## Command Interface

```
brief map <library_path> [--mapper <mapper_name>] [--out <output_dir>] [--force]
brief wrap <library_path> [--mapper <mapper_name>] [--out <output_dir>] [--force]
```

- `map` - Analyze and show what would be generated (dry-run)
- `wrap` - Actually generate the files
- `--mapper` - Optional, defaults to auto-detected (rust→rust, .h→c, .wasm→wasm)
- `--out` - Output directory (default: `lib/ffi/generated/<libname>/`)
- `--force` - Overwrite existing files

## Generated Output Structure

```
lib/ffi/generated/<libname>/
├── lib.bv              # Generated Brief definitions (frgn interfaces)
├── bindings.toml       # FFI binding metadata
├── README.md           # Documentation + user instructions
└── wrapper/            # Language-specific wrapper code (optional)
    ├── lib.rs          # Rust wrapper (if mapper=rust)
    ├── lib.h           # C header (if mapper=c)
    └── ...
```

## lib.bv Structure

```brief
// Auto-generated wrapper for <libname>
// Mapper: <mapper_name>

// Foreign function declarations (frgn sig)
frgn sig read_file(path: String) -> (String, IoError);

// User MUST define these manually:
defn read_file(path: String) -> String [
  true  // precondition - TODO: refine
][
  result.len() > 0  // postcondition - TODO: refine
] {
  __raw_read_file(path)
};
```

## bindings.toml Structure

```toml
# Auto-generated bindings for <libname>
# Mapper: rust

[[functions]]
name = "read_file"
location = "mylib::read_file"
target = "native"
mapper = "rust"

[functions.input]
path = "String"

[functions.output.success]
content = "String"

[functions.output.error]
type = "IoError"
code = "Int"
message = "String"
```

## Implementation Phases

### Phase 1: Fix Build Errors (COMPLETED)

| Error | Location | Fix |
|-------|----------|-----|
| E0282 | interpreter.rs:68 | Add type annotation via named functions |
| E0614 | interpreter.rs:114 | Change `*n` to `n` (for Int sqrt) |
| E0614 | interpreter.rs:125 | Change `*exp` to `exp` |
| E0277 | interpreter.rs:258 | Change `haystack.contains(&needle)` |

### Phase 2: Complete Mapper Integration (COMPLETED)

1. ✅ Add `mapper: Option<String>` and `path: Option<String>` to `ForeignBinding` in `src/ast.rs`
2. ✅ Update `src/ffi/loader.rs` to parse `mapper`/`path` from TOML
3. ✅ Add `MapperNotFound` to `FfiError` enum
4. ✅ Added new ForeignTarget variants (C, Python, Js, Swift, Go)
5. ⏳ Wire mapper resolution in interpreter (needs TOML bindings to actually load)

### Phase 3: Add CLI Commands (COMPLETED)

1. ✅ Added `map` and `wrap` subcommands in `src/main.rs`
2. ✅ Created `run_map_or_wrap()` function
3. ✅ Added mapper discovery via `MapperRegistry`
4. ✅ Created `src/wrapper/mod.rs` - main orchestration
5. ✅ Created `src/wrapper/c_analyzer.rs` - C header parsing
6. ✅ Created `src/wrapper/rust_analyzer.rs` - Rust crate analysis
7. ✅ Created `src/wrapper/wasm_analyzer.rs` - WASM module analysis
8. ✅ Created `src/wrapper/generator.rs` - generates lib.bv + bindings.toml
9. ✅ Created `src/wrapper/contracts.rs` - contract inference

### Phase 4: Per-Mapper Analyzers (COMPLETED)

- ✅ **C**: Parse `.h` header files
- ✅ **Rust**: Analyze Rust crates (Cargo.toml + src/lib.rs)
- ⏳ **WASM**: Basic .wat parsing (binary WASM needs wasmparser crate)

### Phase 5: Contract Inference (COMPLETED)

- ✅ Pattern-based inference (read, write, parse, alloc, free, etc.)
- ✅ Parameter-based checks (null pointers, size/len > 0)
- ✅ Return type checks (result >= 0, result != null)

### Phase 6: Interactive Disambiguation (NOT STARTED)

---

## Current Status

**Working:** 
- `brief map <lib>` - analyzes library, shows full preview of generated files
- `brief wrap <lib>` - generates lib.bv + bindings.toml with full contracts
- `--mapper` flag (rust, c, wasm)
- `--out` flag for output directory
- `--force` flag to overwrite

**Test Results:**
```
test result: ok. 58 passed; 0 failed
```

**Example:**
```bash
# Analyze and preview C header
brief map test_lib.h

# Generate bindings
brief wrap test_lib.h --force
```

**Remaining:**
- Real binary WASM parsing (needs wasmparser crate)
- Interactive disambiguation mode

## Key Design Decisions

1. **Contract inference**: Use pattern-based heuristics, not full static analysis
2. **Overloads**: Handled via native sig casting (defns support this already)
3. **Library detection**: Auto-detect from file extension
4. **Interactive mode**: Numbered prompts for user choice

## Files to Create/Modify

| File | Action |
|------|--------|
| `src/interpreter.rs` | Fix 4 build errors |
| `src/ast.rs` | Add mapper/path to ForeignBinding |
| `src/ffi/loader.rs` | Parse mapper/path |
| `src/ffi/mod.rs` | Add MapperNotFound to FfiError |
| `src/main.rs` | Add map/wrap commands |
| `src/wrapper/mod.rs` | Create - orchestration |
| `src/wrapper/analyzer.rs` | Create - library analysis |
| `src/wrapper/generator.rs` | Create - file generation |
| `src/wrapper/contracts.rs` | Create - contract inference |
| `IMPLEMENTATION-AUTO-WAPPER.md` | This file |

## Dependencies

| Component | Depends On |
|-----------|------------|
| CLI commands | Phase 1, 2 |
| Analyzer | Phase 2 (mappers) |
| Contract inference | Analyzer |
| Sig casting | Already exists |
