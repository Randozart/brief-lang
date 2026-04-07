# Brief FFI Mapper System Specification

**Version:** 0.1 (Draft)
**Status:** Specification

---

## Overview

Brief's Foreign Function Interface (FFI) uses a modular mapper system to bridge Brief with any programming language. Mappers are independently developed executables that translate between Brief's type system and foreign packages.

### Core Principles

| Principle | Description |
|-----------|-------------|
| **TOML is contract** | Defines expected inputs, outputs, and error types |
| **Runtime errors via Result** | Foreign code violations (404, unavailable) return errors |
| **Brief never breaks** | All errors are handleable via Result types |
| **Treat foreign code as unpredictable** | Cannot validate at compile time |

---

## Architecture

### System Flow

```
brief run app.bv
    │
    ├── Parse app.bv
    │   └── Find: frgn format_date(...) from "date.toml"
    │
    ├── Read TOML contract
    │
    ├── Check cache (hash of TOML)
    │   ├── Hit → use cached bridge
    │   └── Miss → invoke mapper
    │
    ├── Mapper build
    │   ├── Read TOML
    │   ├── Analyze package
    │   ├── Generate bridge code
    │   ├── Compile bridge
    │   └── Output: bridge.wasm + metadata.json
    │
    ├── Link bridge to Brief
    │
    └── Run
```

---

## Mapper Protocol

### Invocation

```bash
brief-mapper <lang> <command> <toml> <package> [options]
```

### Commands

| Command | Description |
|---------|-------------|
| `analyze` | Analyze package, output TOML template |
| `build` | Build bridge from TOML contract |
| `clean` | Remove build artifacts |
| `info` | Output mapper metadata |

### Arguments

| Argument | Required | Description |
|----------|----------|-------------|
| `<lang>` | Yes | Target language (js, rust, c, python, etc.) |
| `<command>` | Yes | Command to execute |
| `<toml>` | Yes | Path to TOML contract file |
| `<package>` | Yes | Path to foreign package |
| `--output <path>` | Yes | Bridge output directory |
| `--verbose` | No | Debug output |

### Exit Codes

| Code | Meaning | Brief Action |
|------|---------|--------------|
| `0` | Success | Use bridge |
| `1` | Error | Compilation error |
| `2` | Missing dependency | Prompt user to install |
| `3` | Not applicable | Try next mapper |

### Example

```bash
# Build a JavaScript bridge
./mappers/js/build date.toml node_modules/date-fns --output ./target/date_fns

# Output:
{
  "success": true,
  "bridge": "./target/date_fns/bridge.wasm",
  "metadata": "./target/date_fns/metadata.json"
}
```

---

## Mapper Metadata

Each mapper directory contains `mapper.json`:

```json
{
  "name": "js-mapper",
  "version": "1.0.0",
  "language": "javascript",
  "runtime": "node",
  "description": "Maps JavaScript/TypeScript packages to Brief FFI via WASM",
  "targets": ["wasm"],
  "requires": ["node >= 18", "wasm-pack"],
  "files": {
    "entry": "./build.js",
    "bridge": "./bridge.js"
  },
  "package_formats": {
    "npm": "node_modules/{name}",
    "esm": "./{name}/dist/index.js",
    "cdn": "https://cdn.jsdelivr.net/npm/{name}"
  }
}
```

### Metadata Schema

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Mapper identifier |
| `version` | string | Yes | Semver version |
| `language` | string | Yes | Primary language |
| `runtime` | string | No | Required runtime (node, python, etc.) |
| `description` | string | No | Human description |
| `targets` | string[] | Yes | Supported targets (wasm, native) |
| `requires` | string[] | No | System dependencies |
| `files` | object | Yes | File locations |
| `package_formats` | object | No | Package path templates |

---

## Mapper Discovery

### Search Order

1. `./mappers/<lang>/` - Project-local
2. `~/.brief/mappers/<lang>/` - User-wide
3. `$BRIEF_MAPPERS_PATH/<lang>/` - Custom directory
4. `$BRIEF_REGISTRY/<lang>/` - Registry URL
5. Built-in - Bundled with compiler

### Discovery Process

```rust
fn find_mapper(lang: &str) -> Option<PathBuf> {
    let paths = [
        PathBuf::from("./mappers").join(lang),
        dirs::home_dir().unwrap().join(".brief/mappers").join(lang),
        std::env::var("BRIEF_MAPPERS_PATH")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join(lang),
    ];
    
    for path in paths {
        if path.exists() && path.join("mapper.json").exists() {
            return Some(path);
        }
    }
    None
}
```

### Environment Variables

| Variable | Description |
|----------|-------------|
| `BRIEF_MAPPERS_PATH` | Custom mappers directory |
| `BRIEF_REGISTRY` | Mapper registry URL |
| `BRIEF_CACHE_DIR` | Cache location |

---

## Cache System

### Cache Location

```
~/.brief/cache/
└── bridges/
    └── <toml_hash>/
        ├── bridge.wasm
        ├── metadata.json
        └── lock
```

### Cache Key

Hash of TOML file contents:

```rust
fn cache_key(toml_path: &Path) -> String {
    let content = std::fs::read_to_string(toml_path).unwrap();
    let hash = blake3::hash(content.as_bytes());
    hash.to_hex()[..16].to_string()
}
```

### Cache Validation

Bridge is valid if:
- `lock` file exists
- `lock.toml_hash` matches current TOML hash
- `bridge.*` files exist
- Bridge binary is executable

### Lock File

```json
{
  "version": "1.0.0",
  "toml_hash": "abc123def456",
  "toml_path": "./lib/date.toml",
  "mapper": "js-mapper",
  "mapper_version": "1.0.0",
  "built_at": "2026-04-07T12:00:00Z",
  "built_by": "brief-lang/1.0.0",
  "bridge": {
    "format_date": "bridge.wasm"
  }
}
```

---

## TOML Contract Format

### Basic Structure

```toml
# lib/date.toml

[package]
name = "date-fns"
version = "3.3.0"
source = "npm"

[[functions]]
name = "format_date"
location = "npm:date-fns#format"
target = "wasm"

[functions.input]
date = "String"
format = "String"

[functions.output.success]
result = "String"

[functions.output.error]
type = "DateError"
code = "Int"
message = "String"
```

### Global Configuration

```toml
# Default mapper for this file
default_mapper = "js"

# Target platform
target = "wasm"

[package]
name = "my-app"
version = "1.0.0"
```

### Per-Function Override

```toml
default_mapper = "js"

[[functions]]
name = "rust_crypto"
mapper = "rust"  # Override for this function
location = "crate:sha2"
target = "wasm"
```

### Location Formats

| Format | Example | Description |
|--------|---------|-------------|
| `npm:<package>` | `npm:date-fns` | NPM package |
| `npm:<package>#<export>` | `npm:date-fns#format` | NPM package export |
| `crate:<name>` | `crate:sha2` | Cargo crate |
| `pip:<package>` | `pip:numpy` | Python package |
| `gem:<name>` | `gem:json` | Ruby gem |
| `lib:<name>` | `lib:crypto` | System library |
| `path:<path>` | `path:./local/lib` | Local path |
| `<url>` | `https://...` | Remote URL |

### Type Mapping

```toml
[functions.types]
# How Brief types map to target language types

# Input: Brief -> Target
String = { js = "string", rust = "String", python = "str" }
Int = { js = "number", rust = "i64", python = "int" }
Bool = { js = "boolean", rust = "bool", python = "bool" }

# Output: Target -> Brief (same as input usually)
String = { js = "string", rust = "String" }
```

---

## Type System

### Brief Types

| Brief Type | Description |
|------------|-------------|
| `Int` | 64-bit signed integer |
| `Float` | 64-bit float |
| `String` | UTF-8 text |
| `Bool` | Boolean |
| `Void` | No return value |
| `Data` | Raw bytes |
| `List<T>` | Generic list |

### Result Types

All FFI functions return `Result<Success, Error>`:

```toml
[functions.output.success]
field1 = "Type"
field2 = "Type"

[functions.output.error]
type = "ErrorName"
code = "Int"
message = "String"
```

Maps to Brief:

```brief
frgn func(...) -> Result<(field1: Type, field2: Type), ErrorName>;

// Usage:
let result = func(...);
if result.is_ok() {
    let value = result.value;
    term;
}
if result.is_err() {
    let err = result.error;
    // Handle error
}
```

---

## Error Handling

### Error Categories

| Category | Example | Brief Error Type |
|----------|---------|-----------------|
| Network | 404, timeout | `NetworkError` |
| IO | File not found, permission denied | `IoError` |
| Type | JSON parse failure | `TypeError` |
| Validation | Invalid input | `ValidationError` |
| Unknown | Unexpected failure | `UnknownError` |

### Error Structure

```toml
[functions.output.error]
type = "NetworkError"
code = "Int"      # HTTP status or error code
message = "String"  # Human-readable message
details = "Data"   # Optional raw details
```

### Brief Error Handling

```brief
frgn fetch(url: String) -> Result<Json, NetworkError>;

txn get_data [true][true] {
    let response = fetch("https://api.example.com/data");
    
    # Handle error path
    if response.is_err() {
        let err = response.error;
        # err.code = HTTP status
        # err.message = error text
        escape;
    }
    
    # Success path
    let data = response.value;
    term data;
};
```

### Brief Never Breaks

- FFI errors are always handleable
- No panics from FFI
- Clear error messages
- Error codes for programmatic handling

---

## Target Platforms

### Web (WASM)

```toml
target = "wasm"
```

Flow:
1. Mapper generates wasm-bindgen Rust wrapper
2. Compile with wasm-pack
3. Output: WASM + JS glue

### Native

```toml
target = "native"
```

Flow:
1. Mapper generates Rust/C wrapper
2. Compile to static library or shared object
3. Link with Brief binary

### Multi-target

```toml
[[functions]]
name = "crypto"
target = ["wasm", "native"]  # Build both
```

Brief selects appropriate bridge at runtime based on compilation target.

---

## Mapper Implementation Guide

### Creating a New Mapper

1. **Create directory structure:**
   ```
   mappers/
   └── mylang/
       ├── mapper.json
       └── build.sh (or .js, .py, etc.)
   ```

2. **Write mapper.json:**
   ```json
   {
     "name": "mylang-mapper",
     "version": "1.0.0",
     "language": "mylang",
     "targets": ["wasm", "native"],
     "requires": ["mylang >= 1.0"],
     "files": {
       "entry": "./build.sh"
     }
   }
   ```

3. **Implement build command:**
   ```bash
   #!/bin/bash
   # mappers/mylang/build.sh
   
   COMMAND=$1
   TOML=$2
   PACKAGE=$3
   OUTPUT=$4
   
   case $COMMAND in
     build)
       # Parse TOML
       # Analyze package
       # Generate bridge code
       # Compile bridge
       # Output metadata.json
       echo '{"success": true}'
       ;;
     clean)
       rm -rf "$OUTPUT"
       ;;
   esac
   ```

4. **Test:**
   ```bash
   ./mappers/mylang/build.sh build lib/example.toml ./packages/example --output ./target
   ```

### Mapper Best Practices

1. **Cache aggressively** - Mappers should cache their own artifacts
2. **Clear errors** - Provide helpful error messages
3. **Validate inputs** - Check TOML structure before building
4. **Document requirements** - List system dependencies in mapper.json
5. **Version control** - Lock dependency versions

---

## Registry

### Default Registry

Brief ships with built-in mappers for:
- `rust` - Rust crates
- `c` - C libraries  
- `wasm` - WASM modules

### Installing Mappers

```bash
# From registry
brief install js-mapper
brief install python-mapper

# From local path
brief install js-mapper --from ./my-mapper

# From GitHub
brief install js-mapper --from github:user/mapper
```

### Registry Format

```json
{
  "version": "1",
  "mappers": {
    "js-mapper": {
      "version": "1.0.0",
      "description": "JavaScript/TypeScript bridge via WASM",
      "url": "https://github.com/brief-lang/mapper-js",
      "release": "https://github.com/brief-lang/mapper-js/releases/download/v1.0.0/mapper-js.zip",
      "sha256": "abc123..."
    },
    "python-mapper": {
      "version": "0.5.0",
      "description": "Python package bridge",
      "url": "https://github.com/brief-lang/mapper-python",
      "release": "...",
      "sha256": "def456..."
    }
  }
}
```

---

## Examples

### Example 1: NPM Package

**TOML (lib/date.toml):**
```toml
[package]
name = "date-fns"
version = "3.3.0"
source = "npm"

[[functions]]
name = "format_date"
location = "npm:date-fns#format"
target = "wasm"

[functions.input]
date = "String"
format = "String"

[functions.output.success]
result = "String"

[functions.output.error]
type = "DateError"
code = "Int"
message = "String"
```

**Brief code (app.bv):**
```brief
import "./lib/date.toml";

frgn format_date(date: String, fmt: String) -> Result<String, DateError> from "lib/date.toml";

txn display_date [true][true] {
    let result = format_date("2026-04-07", "yyyy-MM-dd");
    if result.is_err() {
        escape;
    }
    term result.value;
};
```

### Example 2: Rust Crate

**TOML (lib/crypto.toml):**
```toml
[package]
name = "sha2"
version = "0.10"
source = "crate"

[[functions]]
name = "sha256"
mapper = "rust"
location = "crate:sha2#sha256"
target = "wasm"

[functions.input]
data = "String"

[functions.output.success]
hash = "String"

[functions.output.error]
type = "CryptoError"
message = "String"
```

### Example 3: Mixed Sources

**TOML (lib/mixed.toml):**
```toml
default_mapper = "js"

[[functions]]
name = "fetch_data"
location = "npm:axios#get"
target = "wasm"

[[functions]]
name = "compute_hash"
mapper = "rust"  # Override
location = "crate:sha2#sha256"
target = "wasm"
```

---

## Appendix

### A. Exit Code Reference

| Code | Name | Description |
|------|------|-------------|
| 0 | Success | Bridge built successfully |
| 1 | BuildError | Compilation failed |
| 2 | MissingDependency | Required tool not found |
| 3 | NotApplicable | Package format not supported |
| 4 | InvalidTOML | TOML file malformed |
| 5 | PackageNotFound | Foreign package not found |
| 6 | NetworkError | Download/connection failed |
| 7 | PermissionDenied | Cannot write output |

### B. Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `BRIEF_MAPPERS_PATH` | - | Custom mappers directory |
| `BRIEF_REGISTRY` | built-in | Mapper registry URL |
| `BRIEF_CACHE_DIR` | `~/.brief/cache` | Cache location |
| `BRIEF_OFFLINE` | `false` | Skip network operations |

### C. File Extensions

| Extension | Type | Description |
|-----------|------|-------------|
| `.wasm` | WASM binary | WebAssembly bridge |
| `.wat` | WASM text | WebAssembly source |
| `.so` | Shared object | Linux native bridge |
| `.dylib` | Dynamic library | macOS native bridge |
| `.dll` | DLL | Windows native bridge |
| `.a` | Static archive | Static library |

---

**End of Specification**
