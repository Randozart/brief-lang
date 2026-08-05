# DBriv FFI Schema and Dependency Management Plan

## Overview

This document outlines the plan to extend DBriv with:
1. Keyword abbreviations for all DBriv keywords
2. A `DEPENDS` keyword for dependency declarations
3. Documentation of DBriv's unique syntax and capabilities

---

## Implementation Status

### Phase 1: Keyword Abbreviations ✅ IMPLEMENTED

**Implemented in:** `src/dbriv/parser.rs`

| Keyword | Abbreviations | Status |
|---------|---------------|--------|
| `register` | `reg`, `regs` | ✅ Implemented |
| `alias` | `ali` | ✅ Implemented |
| `service` | `serv`, `svc` | ✅ Implemented |
| `struct` | `stru`, `str` | ✅ Implemented |
| `enum` | `en` | ✅ Implemented |
| `rule` | `rl`, `rul` | ✅ Implemented |
| `check` | `chk` | ✅ Implemented |
| `depends` | `dep`, `deps` | ✅ Implemented |

### Phase 2: DEPENDS Keyword ✅ IMPLEMENTED

**Implemented in:** `src/dbriv/parser.rs` and `src/dbriv/ast.rs`

```dbriv
// Syntax supported:
DEPENDS "sqlite3" VERSION ">=3.36.0" PLATFORM native;
DEPENDS "openssl" VERSION "^1.1" PLATFORM [native, wasm];
DEPENDS "math" VERSION ">=2.0" FEATURES [simd, unsafe];
DEPENDS "mylib" VERSION "1.0" SOURCE "https://github.com/...";
```

### Phase 3: Documentation 📋 PENDING

Need to document:
- All keywords and their abbreviations  
- `register` with arbitrary names (`@banana`, `@my_func`)
- `DEPENDS` syntax
- File types: `.dbvs` (schema), `.dbv` (view), `.dbvl` (live)

### Phase 4: CLI for Dependency Management (Optional) 📋 PENDING

---

## Known Issues

- The DBVS parser has pre-existing parse errors with some register block formats
- Additional testing and bug-fixing needed for complex FFI binding schemas

---

## Related Files

- `src/dbriv/parser.rs` - DBriv parser with keyword abbreviations
- `src/dbriv/ast.rs` - DBriv AST with DbrivDependency struct
- `std/bindings/*.dbvs` - Existing FFI binding schemas