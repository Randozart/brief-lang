# Brief v7.0 Implementation Summary

**Date:** 2026-04-07
**Status:** Implementation In Progress

---

## Quick Reference

| Feature | File | SPEC Section |
|---------|------|--------------|
| Implicit `term true;` | desugarer.rs | 5.3.1 |
| Multi-field FFI outputs | parser.rs | 7.4.1 |
| Multi-return validation | typechecker.rs | Part 11 |
| FFI error enforcement | typechecker.rs | 7.7 |
| Dynamic FFI registry | ffi/registry.rs | 7 |
| Term functionCall verification | typechecker.rs | 5.3.2 |
| R-Brief syntax fix | SPEC.md, refs | 9.2 |
| Reactor throttling | SPEC.md | 8.4 |
| Mutual exclusion fix | SPEC.md | 8.3 |
| **Modular FFI Mapper System** | FFI/mapper | 7.x |

---

## Completed Implementations

### 1. Implicit `term true;` Desugaring
**File:** `src/desugarer.rs`
**SPEC Section:** 5.3.1 Implicit `term true;`

Transforms `term;` to `term true;` when postcondition is literal `true`.

```brief
txn activate [ready][true] {
    term;  // becomes: term true;
};
```

**Commits:** `5616fa1`

---

### 2. Multi-Field FFI Success Output Parsing
**File:** `src/parser.rs`
**SPEC Section:** 7.4.1 Multi-Field Success Outputs

Added support for tuple syntax in FFI success types:
```brief
frgn divide(a: Int, b: Int) -> Result<(quotient: Int, remainder: Int), MathError> from "lib/math.toml";
```

**Commits:** `5616fa1`

---

### 3. Multi-Return Validation
**File:** `src/typechecker.rs`
**SPEC Section:** Part 11 (Multi-Return Functions)

Added `check_statement_with_outputs()` to validate that term outputs match definition output types.

**Commits:** `5616fa1`

---

### 4. FFI Error Enforcement
**File:** `src/typechecker.rs`
**SPEC Section:** 7.7 Error Handling Requirements

Complete implementation:
- Tracks FFI Result variables using `ResultCheckStatus` enum
- Records when variables are checked with `.is_ok()` or `.is_err()`
- Emits error when `.value` or `.error` accessed without prior check
- Warning (F101) when FFI result assigned without immediate handling

```brief
let result = read_file(path);  // Warning: should use is_ok()/is_err()
[result.is_ok()] { ... }
term result.value;  // OK - was checked
```

**Commits:** `a1277fc`, `25d4830`

---

### 5. R-Brief Syntax Corrections
**Files:** `spec/SPEC.md`, `spec/LANGUAGE-REFERENCE.md`, `spec/RENDERED-BRIEF-GUIDE.md`

- Fixed rstruct syntax: HTML is inline using `<tag>` inside rstruct
- Added `render` standalone view component documentation
- CSS imported at file top with standard `import` statement

**Commits:** `a6929ad`

---

### 6. Reactor Throttling Documentation
**Files:** `spec/SPEC.md`, `spec/RENDERED-BRIEF-GUIDE.md`
**SPEC Section:** 8.4 Reactor Throttling

Documented `@Hz` declarations:
```brief
reactor @10Hz;  // File-level default
rct txn fast [c][p] { ... } @60Hz;  // Per-transaction override
```

**Commits:** `a6929ad`

---

### 7. Mutual Exclusion Clarification
**File:** `spec/SPEC.md`
**SPEC Section:** 8.3 Async Transactions

Clarified that preconditions only need to be mutually exclusive when they write to overlapping state. Reading-only or writing to different variables is fine.

**Commits:** `a6929ad`

---

## Pending Implementations

### 1. Complete Term FunctionCall Verification
**SPEC Section:** 5.3.2

Full symbolic verification to prove that function call output satisfies postcondition.

**Status:** Partial (info diagnostic only, needs symbolic equality checking)

---

## Documentation Updates

### Version: 7.0

All documentation updated to v7.0:
- `spec/SPEC.md`
- `spec/LANGUAGE-REFERENCE.md`
- `spec/LANGUAGE-TUTORIAL.md`
- `spec/FFI-GUIDE.md`
- `spec/RENDERED-BRIEF-GUIDE.md`
- `spec/QUICK-REFERENCE.md`

**Commits:** `7d0d513`, `a6929ad`

---

---

# FFI Mapper System Implementation Plan

**Part of:** Brief v7.0
**Status:** Planned
**Priority:** High

---

## Overview

Brief's FFI system enables integration with any language via a modular mapper architecture. Mappers bridge Brief and foreign packages, handling type conversions and compilation.

### Core Principles

| Principle | Description |
|-----------|-------------|
| **TOML is contract** | Defines what Brief expects; source of truth |
| **Runtime violations = errors** | Foreign code can break promises (404, unavailable resources) → Result types |
| **Treat foreign code as unpredictable** | Cannot validate at compile time; always assume error margin |
| **Errors must be handleable** | `frgn sig` requires Result type; Brief code must handle errors |
| **Brief never breaks** | All errors are handled, not crashes |

### FFI Error Examples

```brief
// Console unavailable
frgn print(msg: String) -> Result<Void, IoError>;

// API returns 404
frgn fetch(url: String) -> Result<Json, NetworkError>;

// File not found
frgn read_file(path: String) -> Result<String, IoError>;
```

---

## Architecture

### System Components

```
┌─────────────────────────────────────────────────────────────┐
│                      Brief Compiler                         │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐   │
│  │   Parser    │  │ TypeChecker │  │ FFI Resolver    │   │
│  └─────────────┘  └─────────────┘  └─────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                   FFI Mapper System                         │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐   │
│  │  Discovery  │  │  Invoker    │  │    Cache        │   │
│  └─────────────┘  └─────────────┘  └─────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                              │
          ┌───────────────────┼───────────────────┐
          ▼                   ▼                   ▼
    ┌──────────┐      ┌──────────┐      ┌──────────┐
    │js-mapper │      │rust-mapper│      │c-mapper  │
    └──────────┘      └──────────┘      └──────────┘
```

### Mapper Invocation Flow

```
brief run app.bv
    │
    ├── Parse app.bv + frgn declarations
    │
    ├── For each FFI function:
    │   ├── Read TOML contract
    │   ├── Check cache (hash of TOML)
    │   │   └── If cached & valid → use bridge
    │   │   └── If not cached → invoke mapper
    │   │
    │   └── Mapper:
    │       ├── Reads TOML
    │       ├── Generates type-bridge code
    │       ├── Compiles bridge
    │       └── Writes to cache
    │
    └── Link bridges + run
```

---

## Mapper Specification

### Mapper Protocol

Each mapper is an executable (any language) invoked by Brief:

```bash
brief-mapper <lang> build <toml> <package> --output <bridge_path>
```

**Arguments:**
| Argument | Description |
|----------|-------------|
| `<lang>` | Target language (js, rust, c, python, etc.) |
| `build` | Command: analyze, build, clean |
| `<toml>` | Path to TOML contract file |
| `<package>` | Path to foreign package |
| `--output` | Bridge output directory |
| `--verbose` | Debug output (optional) |

**Exit Codes:**
| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | Error (compilation error) |
| `2` | Missing dependency (Brief prompts user) |
| `3` | Mapper not found (Brief tries next) |

### Mapper Metadata (JSON)

Each mapper includes `mapper.json` for discovery:

```json
{
  "name": "js-mapper",
  "version": "1.0.0",
  "language": "javascript",
  "runtime": "node",
  "description": "Maps JavaScript/TypeScript packages to Brief FFI",
  "targets": ["wasm", "native"],
  "requires": ["node", "wasm-pack"],
  "files": {
    "mapper": "./index.js",
    "bridge": "./bridge.js"
  },
  "mappings": {
    "npm": "node_modules/{package}",
    "esm": "./{package}/dist/index.js"
  }
}
```

### Mapper Discovery

Search order (first found wins):
1. `./mappers/<lang>/` (project-local)
2. `~/.brief/mappers/<lang>/` (user-wide)
3. `$BRIEF_MAPPERS_PATH/` (custom directory)
4. `$BRIEF_REGISTRY/<lang>/` (registry URL)
5. Built-in (bundled with compiler)

### Mapper Installation

```bash
brief install js-mapper           # From registry
brief install js-mapper --from ./my_mapper.js  # Local path
brief install js-mapper --from github:user/js-mapper  # GitHub
```

---

## TOML Contract Format

### Global Configuration

```toml
# Optional: Default mapper for this file
default_mapper = "js"

[package]
name = "my-app"
version = "1.0.0"
```

### Function Declaration

```toml
[[functions]]
# Required
name = "format_date"
location = "npm:date-fns"

# Optional overrides
mapper = "js"  # Override default mapper for this function
target = "wasm"  # Target platform

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

### Location Formats

| Format | Example | Mapper |
|--------|---------|--------|
| `npm:<package>` | `npm:date-fns` | js-mapper |
| `crate:<name>` | `crate:sha2` | rust-mapper |
| `pip:<package>` | `pip:numpy` | python-mapper |
| `lib:<name>` | `lib:crypto` | c-mapper |
| `path:<path>` | `path:./local/lib` | inferred |
| `<url>` | `https://...` | inferred |

### Type Mapping

TOML defines type conversions:

```toml
# Brief type → Target type
# Target type → Brief type (return)

[functions.types]
# Input mapping
String = { js = "string", rust = "String" }
Int = { js = "number", rust = "i64" }

# Output mapping
String = { js = "string", rust = "String" }
```

---

## Cache System

### Cache Location

```
~/.brief/cache/
└── bridges/
    └── <hash_of_toml>/
        ├── bridge.wasm (or .so, .a, etc.)
        ├── metadata.json
        └── lock
```

### Cache Key

Hash of TOML file contents:
```
SHA256("name=format_date\nmapper=js\n...")
```

### Cache Invalidation

- If TOML changes → rebuild
- If `--force` flag → rebuild
- If `--clean` flag → clear cache

### Lock File

```json
{
  "toml_hash": "abc123...",
  "mapper_version": "1.0.0",
  "built_at": "2026-04-07T12:00:00Z",
  "bridge": "bridge.wasm"
}
```

---

## Implementation Phases

### Phase 1: Core Infrastructure
**Files:** `src/ffi/mapper/`

1. **Mapper Discovery** (`src/ffi/mapper/discovery.rs`)
   - Search configured paths
   - Parse `mapper.json` metadata
   - Validate mapper requirements

2. **Mapper Invoker** (`src/ffi/mapper/invoker.rs`)
   - Build command line
   - Execute subprocess
   - Capture output/errors
   - Parse exit codes

3. **Cache Manager** (`src/ffi/mapper/cache.rs`)
   - Compute TOML hash
   - Store/retrieve bridges
   - Invalidate on changes
   - Lock file management

### Phase 2: TOML Schema Updates
**Files:** `src/ffi/loader.rs`, `src/ffi/types.rs`

1. Add `mapper` field support (global + per-function)
2. Add `location` format parsing (npm:, crate:, etc.)
3. Add `target` field support

### Phase 3: Built-in Mappers Refactor
**Files:** `src/wrapper/`, `src/ffi/mappers/`

1. **wasm-mapper**
   - Already exists in `wrapper/wasm_analyzer.rs`
   - Refactor to executable protocol

2. **rust-mapper**
   - Already exists in `wrapper/rust_analyzer.rs`
   - Refactor to executable protocol

3. **c-mapper**
   - Already exists in `wrapper/c_analyzer.rs`
   - Refactor to executable protocol

### Phase 4: JS Mapper (New)
**Files:** `mappers/js/`

```javascript
// mappers/js/index.js
const { build } = require('./builder');
const { parseToml } = require('./toml-parser');
const { generateBridge } = require('./bridge-generator');
const { runWasmPack } = require('./wasm-pack');

async function main() {
  const [,, command, tomlPath, packagePath, ...args] = process.argv;
  
  if (command === 'build') {
    const output = args.find(a => a === '--output') ? args[args.indexOf('--output') + 1] : './target';
    const toml = parseToml(tomlPath);
    const bridge = generateBridge(toml, packagePath);
    await runWasmPack(bridge);
    console.log(JSON.stringify({ success: true, bridge: `${output}/bridge.wasm` }));
  }
}

main().catch(err => {
  console.error(JSON.stringify({ success: false, error: err.message }));
  process.exit(1);
});
```

**JS Mapper Responsibilities:**
1. Read TOML contract
2. Analyze package.json exports
3. Parse TypeScript definitions (.d.ts) if available
4. Generate wasm-bindgen Rust wrapper
5. Run wasm-pack build
6. Output bridge + metadata

### Phase 5: Compilation Integration
**Files:** `src/compiler.rs`, `src/interpreter.rs`

1. **Pre-flight FFI resolution**
   - Collect all frgn declarations
   - Resolve TOML paths
   - Check/invoke mappers
   - Collect bridges

2. **Link-time integration**
   - Load compiled bridges
   - Register with interpreter
   - Verify bindings

3. **Runtime FFI calls**
   - Call through bridge
   - Handle Result types
   - Map errors

### Phase 6: Registry System
**Files:** `src/cli/install.rs`

```bash
brief install js-mapper
brief install rust-mapper
brief install python-mapper
```

**Registry format (JSON):**
```json
{
  "mappers": [
    {
      "name": "js-mapper",
      "version": "1.0.0",
      "description": "JavaScript/TypeScript package bridge",
      "url": "https://github.com/brief-lang/mapper-js/releases/v1.0.0.zip",
      "sha256": "abc123..."
    }
  ]
}
```

---

## Error Handling

### Compilation Errors

| Error | Cause | Action |
|-------|-------|--------|
| Mapper not found | `--install` prompt | Offer to install |
| Mapper failed | Exit code 1 | Show mapper error |
| Missing dependency | Exit code 2 | Show install instructions |
| Cache invalid | TOML changed | Rebuild automatically |

### Runtime Errors

| Error | Source | Brief Handling |
|-------|--------|----------------|
| NetworkError | HTTP requests | Result types |
| IoError | File system | Result types |
| TypeError | Bridge mismatch | Panic + diagnostic |
| Timeout | Long operations | Result types |

### Brief Never Breaks

```brief
// All FFI errors are handleable
txn safe_read [true][true] {
    let result = read_file("config.json");
    if result.is_err() {
        let err = result.error;
        // Handle gracefully
        escape;
    }
    term result.value;
};
```

---

## File Structure

```
brief-lang/
├── src/
│   ├── ffi/
│   │   ├── mod.rs
│   │   ├── loader.rs
│   │   ├── types.rs
│   │   ├── registry.rs
│   │   └── mapper/
│   │       ├── mod.rs
│   │       ├── discovery.rs
│   │       ├── invoker.rs
│   │       ├── cache.rs
│   │       └── metadata.rs
│   ├── compiler.rs
│   └── interpreter.rs
├── mappers/
│   ├── js/
│   │   ├── mapper.json
│   │   ├── index.js
│   │   ├── builder.js
│   │   ├── toml-parser.js
│   │   └── bridge-generator.js
│   ├── rust/
│   │   ├── mapper.json
│   │   └── src/
│   └── c/
│       ├── mapper.json
│       └── src/
├── spec/
│   ├── FFI-MAPPER-SPEC.md (new)
│   └── FFI-GUIDE.md (update)
└── docs/
    └── mapper-guide.md (new)
```

---

## Testing Plan

### Unit Tests
- Mapper discovery
- Cache hash computation
- TOML parsing
- Type mapping

### Integration Tests
- Full build with wasm-mapper
- Cache invalidation
- Error propagation

### E2E Tests
```bash
# Test JS package bridge
brief run tests/fetch_time.bv

# Test native Rust crate bridge
brief run tests/crypto_hash.bv

# Test C library bridge
brief run tests/zlib_compress.bv
```

---

## Documentation Updates Required

1. **FFI-MAPPER-SPEC.md** (new)
   - Complete mapper protocol specification
   - Metadata format (JSON)
   - Discovery mechanism
   - Cache system

2. **FFI-GUIDE.md** (update)
   - Add mapper section
   - Add installation instructions
   - Add examples for each language

3. **Tutorial** (update)
   - "Using NPM packages in Brief"
   - "Using Rust crates in Brief"
   - "Using Python libraries in Brief"

---

## Milestones

| Milestone | Description | Status |
|-----------|-------------|--------|
| M1 | Mapper discovery + invocation | Planned |
| M2 | Cache system | Planned |
| M3 | TOML schema update | Planned |
| M4 | Built-in mappers refactor | Planned |
| M5 | JS mapper | Planned |
| M6 | Registry + install command | Planned |
| M7 | Full integration tests | Planned |
| M8 | Documentation | Planned |

---

## Git History

```
a3e0f72 docs: add WASM target to FFI guide
25d4830 impl: add term functionCall verification
3b5e83e impl: complete FFI error enforcement and add dynamic FFI registry
c0b52a0 docs: add v7.0 implementation summary
a1277fc impl: add FFI error enforcement warning
5616fa1 impl: Phase 1 core language features
a6929ad docs: fix rstruct syntax and add reactor throttling
740aa41 docs: update implementation summary with quick reference table
7d0d513 docs: update documentation to v7.0 with new features
```
