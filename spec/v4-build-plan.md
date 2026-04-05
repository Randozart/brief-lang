# Brief Compiler Build Plan

**Based on:** `ARCHITECTURE.md` v1.0  
**Target:** Incremental compiler for `.bv` files with manifest-driven imports  
**Language:** Rust

---

## Phase 0: Architecture Foundation (1 day)

**Goal:** Set up incremental compilation infrastructure

### Deliverables
- `src/cache/` module - Content-addressed cache
- `src/watch/` module - File watching with interface-level invalidation
- `manifest.rs` - TOML manifest parsing

### Tasks
1. Implement `CacheManager` with content-addressed storage
2. Implement `InterfaceHasher` for exports/signatures
3. Set up file watching (notify crate)
4. Implement debounced watch events

---

## Phase 1: Manifest System (1 day)

**Goal:** Package management with `brief.toml`

### Deliverables
- `src/manifest.rs` - Parse and validate brief.toml
- CLI commands: `import`, `install`, `list`, `remove`

### Tasks
1. Define `Manifest` struct with TOML serde
2. Implement dependency resolution (path, registry, version)
3. Auto-add discovered imports to manifest
4. Clear error messages for unresolved imports

### CLI Commands
```bash
brief import <name> --path <loc>   # Add dependency
brief install                      # Install all deps
brief list                         # List deps
```

---

## Phase 2: Import Resolver (1 day)

**Goal:** Convention-based + manifest-driven import resolution

### Deliverables
- `src/resolver.rs` - Import path resolution
- Search path: `lib/` → `imports/` → `./`

### Tasks
1. Implement search path discovery
2. Check manifest before searching
3. Auto-add found imports to manifest
4. Cache resolved paths per session

---

## Phase 3: Watch Mode (2 days)

**Goal:** Sub-100ms feedback on file changes

### Deliverables
- `src/watch.rs` - Watch mode with debouncing
- Interface hash invalidation

### Tasks
1. Set up file system watcher
2. Implement on-save trigger
3. Implement debounced trigger (300ms)
4. Compute interface hashes
5. Invalidate importers on interface change

### Performance Target
- File save → feedback: ~50ms
- Debounced → full check: ~350ms

---

## Phase 4: Cache Manager (2 days)

**Goal:** Persistent, content-addressed caching

### Deliverables
- `.brief-cache/` directory
- Phase-level caching (AST, types, proofs)

### Tasks
1. Implement cache key generation (source hash + version)
2. Implement cache read/write
3. Implement cache invalidation on changes
4. Implement cache integrity verification

### Cache Structure
```
.brief-cache/
├── manifest.json
├── modules/
│   ├── {name}.ast
│   ├── {name}.types
│   └── {name}.proofs
└── graphs/
    ├── call-graph.json
    └── dependency-graph.json
```

---

## Phase 5: Parallel Execution (2 days)

**Goal:** Utilize all CPU cores for compilation

### Deliverables
- Parallel type checking
- Parallel proof verification
- Parallel import resolution

### Tasks
1. Add `rayon` for data parallelism
2. Parallelize static analysis stages
3. Implement work-stealing for import resolution
4. Profile and tune thread count

---

## Phase 6: CLI Overhaul (1 day)

**Goal:** Complete command-line interface

### Deliverables
- `brief check` - Type check without execution
- `brief build` - Full compilation
- `brief watch` - Watch mode
- `brief init` - Scaffold new project

### Commands
```bash
brief check <file.bv>      # Skip interpreter, fast feedback
brief build <file.bv>      # Full compilation
brief watch <file.bv>      # Watch mode
brief init --name <name>   # Create project scaffold
```

---

## Phase 7: Integration Tests (2 days)

**Goal:** Verify incremental compilation correctness

### Test Cases
1. Edit file → only it re-checked
2. Change interface → importers invalidated
3. Manifest change → full re-resolution
4. Cache invalidation correctness
5. Watch mode debouncing

---

## Phase 8: Rendered Brief Preparation (Ongoing)

**Goal:** Design for future .rbv compilation

### Shared Components
- Lexer extensions for HTML/directives
- Parser extensions for `<script>`/`<view>` blocks
- WASM code generation module

### Not in Scope for v1.0
- Actual WASM compilation
- JS glue generation
- Browser integration

---

## Dependencies

```
Phase 0: Architecture Foundation
    │
    ├──► Phase 1: Manifest System
    │         │
    │         └──► Phase 2: Import Resolver
    │                   │
    │                   └──► Phase 3: Watch Mode
    │                             │
    │                             └──► Phase 4: Cache Manager
    │                                       │
    │                                       └──► Phase 5: Parallel Execution
    │                                                 │
    │                                                 └──► Phase 6: CLI Overhaul
    │                                                           │
    │                                                           └──► Phase 7: Integration Tests
    │
    └──► Phase 8: Rendered Brief Preparation (Parallel)
```

---

## Current State (as of 2026-04-04)

| Component | Status | Notes |
|-----------|--------|-------|
| Lexer | ✓ Complete | `src/lexer.rs` |
| Parser | ✓ Complete | `src/parser.rs` |
| AST | ✓ Complete | `src/ast.rs` |
| Interpreter | ⚠ Basic | Works, needs reactor integration |
| TypeChecker | ✓ Complete | `src/typechecker.rs` |
| ProofEngine | ✓ Complete | `src/proof_engine.rs` |
| Annotator | ✓ Complete | `src/annotator.rs` |
| Reactor | ⚠ Basic | Needs dependency tracking |
| Manifest | ✗ Not started | Design in ARCHITECTURE.md |
| Import Resolver | ✗ Not started | Parses, doesn't resolve |
| Watch Mode | ✗ Not started | Design in ARCHITECTURE.md |
| Cache Manager | ✗ Not started | Design in ARCHITECTURE.md |
| Parallel Execution | ✗ Not started | Will use rayon |
| CLI | ⚠ Basic | Needs `--check`, `--watch` |

---

## Key References

- **Architecture:** `ARCHITECTURE.md`
- **Language Spec:** `brief-lang-spec.md`
- **Rendered Brief:** `rendered-brief-rbv-spec-v3.md`
