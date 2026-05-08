# DBrief FFI Schema and Dependency Management Plan

## Overview

This document outlines the plan to extend DBrief with:
1. Keyword abbreviations for all DBrief keywords
2. A `DEPENDS` keyword for dependency declarations
3. Documentation of DBrief's unique syntax and capabilities

---

## Background

DBrief (`.dbv`, `.dbvs`, `.dbvl` files) is Brief's hardware register and data description language. It already supports:
- `register` - FFI/hardware register declarations
- `service` - I/O interface definitions  
- `rule` - Datalog logic rules
- `alias` - named address aliases
- `check` - standalone contracts
- `record` - data instances at addresses (`@addr { ... }`)
- `import` - file imports

However:
- Keyword abbreviations are not implemented
- No dependency/installation metadata exists
- This has never been properly documented

---

## Current State Analysis

### 1. `register` with Arbitrary Names

**Already supported!** The parser (`src/dbrief/parser.rs:555-599`) supports:
- `auto` - automatic address allocation
- `0x...` - hexadecimal addresses  
- Numeric - decimal numbers
- **Named identifiers** - any valid identifier (like `@banana`)

Example (already works):
```dbrief
register @banana as "print" { ... }
register my_function as "sqrt" { ... }
```

### 2. Keyword Abbreviations

**Not implemented.** Current parser only matches exact keywords.

### 3. `DEPENDS` Keyword

**Not implemented.** No dependency declaration syntax exists.

---

## Implementation Plan

### Phase 1: Add Keyword Abbreviations

**File:** `src/dbrief/parser.rs`

**Changes:** Modify the keyword dispatch (lines 45-105) to accept multiple prefixes per keyword.

**Abbreviation Table:**

| Keyword | Abbreviations | Notes |
|---------|---------------|-------|
| `register` | `reg`, `regs` | FFI binding |
| `alias` | `ali` | Address alias |
| `service` | `serv`, `svc` | I/O interface |
| `struct` | `stru`, `str` | Type definition |
| `enum` | `en`, `e` | Enum type |
| `rule` | `rl`, `rul` | Logic rule |
| `check` | `chk` | Contract |
| `record` | `rec` | Data record |
| `import` | `imp` | File import |
| `depends` | `dep`, `deps` | (New) Dependency |

**Implementation approach:**
- Use prefix matching with fallthrough (e.g., "reg" → "register", "regs" → "register")
- Case-insensitive matching (already implemented)

### Phase 2: Add `DEPENDS` Keyword

**New construct in DBrief:**

```dbrief
// Single dependency
DEPENDS "sqlite3" VERSION ">=3.36.0" PLATFORM native;

// Multiple dependencies  
DEPENDS "openssl" VERSION "^1.1" PLATFORM [native, wasm];
DEPENDS "curl" VERSION ">=7.0" PLATFORM native;

// With features
DEPENDS "math" VERSION ">=2.0" FEATURES [simd, unsafe];

// From specific source
DEPENDS "mylib" VERSION "1.0" SOURCE "https://github.com/...";
```

**AST addition:** New `DbriefDependency` struct:
```rust
struct DbriefDependency {
    name: String,
    version_constraint: Option<String>,
    platform: Vec<String>,          // native, wasm, c, etc.
    features: Vec<String>,          // simd, unsafe, etc.
    source: Option<String>,         // URL or registry path
}
```

**Parser:** Add `parse_dependency()` similar to other parse functions.

**Integration:** Add to keyword dispatch alongside existing keywords.

### Phase 3: Documentation

**Where to document:**

1. **`lib/std/README.md`** - Already exists, add DBrief section
2. **`lib/dbrief/README.md`** - New file for DBrief-specific docs
3. **`std/bindings/README.md`** - Explain binding schema

**Document:**
- All keywords and their abbreviations
- `register` with arbitrary names (`@banana`, `@my_func`)
- `DEPENDS` syntax
- File types: `.dbvs` (schema), `.dbv` (view), `.dbvl` (live)
- How to create FFI bindings

### Phase 4: CLI for Dependency Management (Optional)

**Commands:**
```
brief deps check    # Verify all dependencies available
brief deps install  # Install missing dependencies
```

**Implementation:** Read `.dbv`/`.dbvs` files, parse `DEPENDS` declarations, verify/install packages.

---

## DBVL Format Note

DBVL (`.dbvl`) is **line-oriented** (like JSONL), perfect for bulk data:

```dbvl
0x00|sqrt|Fn(Float)->Result<Float>|std::f64::sqrt|native
0x01|pow|Fn(Float,Float)->Result<Float>|std::f64::powf|native
```

Each line = one `register` entry. This is ideal for:
- Declaring many FFI bindings efficiently
- Auto-generating from C headers
- Bulk importing from external sources

---

## Summary

| Phase | Item | Effort |
|-------|------|--------|
| 1 | Keyword abbreviations | Medium |
| 2 | `DEPENDS` keyword | Medium |
| 3 | Documentation | Low |
| 4 | CLI (optional) | High |

---

## Related Files

- `src/dbrief/parser.rs` - DBrief parser (main implementation file)
- `src/dbrief/ast.rs` - DBrief AST types
- `std/bindings/*.dbvs` - Existing FFI binding schemas
- `lib/dbrief/README.md` - (To be created) DBrief documentation