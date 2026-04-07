# Brief Compiler Architecture

**Document Version:** 1.0  
**Date:** 2026-04-04  
**Status:** Living Document  
**Rationale:** This document captures architectural decisions. When decisions change, update here first—this is the single source of truth for *why* the system works the way it does.

---

## Table of Contents

1. [Design Philosophy](#1-design-philosophy)
2. [Language Structure](#2-language-structure)
3. [Compilation Pipeline](#3-compilation-pipeline)
4. [Incremental Compilation](#4-incremental-compilation)
5. [Import System](#5-import-system)
6. [Manifest System](#6-manifest-system)
7. [Watch Mode](#7-watch-mode)
8. [Cache Architecture](#8-cache-architecture)
9. [Rendered Brief (Future)](#9-rendered-brief-future)
10. [Decision Log](#10-decision-log)

---

## 1. Design Philosophy

### 1.1 Core Principles

| Principle | Rationale |
|-----------|-----------|
| **Closed World** | All code paths known at compile time; enables full static analysis |
| **No Dynamic Dispatch** | No vtables, reflection, or runtime code loading |
| **Promise Exhaustiveness** | Every possible outcome must be handled explicitly |
| **Formal Verification without Boilerplate** | Contracts are first-class, not annotations |
| **LLM-Friendly** | Minimal nesting, flat logic, concise syntax |

### 1.2 Why These Principles

```
Closed World + No Dynamic Dispatch
         ↓
   Full call graph computable at compile time
         ↓
   Path analysis, dead code elimination, proofs all tractable
         ↓
   Incremental compilation viable (interface hashing)
         ↓
   <100ms feedback possible
```

### 1.3 Trade-offs Accepted

| Trade-off | Why It's Acceptable |
|-----------|---------------------|
| No dynamic imports | Closed world assumption; all deps declared |
| Strict contracts | Prevents entire class of runtime bugs |
| Flat logic | LLM token efficiency; human readability |

---

## 2. Language Structure

### 2.1 File Extension
**Decision:** `.bv` (Brief)

### 2.2 Top-Level Elements

```
signature     → External capability boundary
definition    → Named function with contracts
state_decl   → Reactive signal (mutable state)
constant     → Immutable binding
transaction  → Passive unit of work
rct          → Reactive transaction (blackboard-driven)
```

### 2.3 Contracts

Every `defn`, `txn`, and `rct` has two contracts:
- `[pre]` — When does this fire?
- `[post]` — Did it succeed?

### 2.4 State Model

| Concept | Description |
|---------|-------------|
| `let` | Reactive signal, mutable |
| `const` | Immutable binding |
| `&var` | Ownership write claim |
| `@var` | Prior state reference |

---

## 3. Compilation Pipeline

### 3.1 Stage Diagram

```
Source (.bv)
    │
    ▼
┌─────────────────────┐
│       Lexer          │  O(n) linear tokenization
│   Token stream      │
└─────────────────────┘
    │
    ▼
┌─────────────────────┐
│       Parser         │  O(n) AST construction
│    Program AST       │
└─────────────────────┘
    │
    ├────────────────────────────────────┐
    │                                    │
    ▼                                    ▼
┌─────────────────────┐    ┌─────────────────────┐
│    Annotator        │    │   TypeChecker       │
│  Call graph paths   │    │  Type inference     │
└─────────────────────┘    └─────────────────────┘
    │                                    │
    └─────────────────┬──────────────────┘
                      ▼
┌───────────────────────────────────────┐
│           ProofEngine                   │
│  - Exhaustiveness                      │
│  - Mutual exclusion                    │
│  - Dead code detection                 │
│  - Total-path checking                 │
└───────────────────────────────────────┘
                      │
                      ▼
┌───────────────────────────────────────┐
│          Interpreter/Reactor            │
│  - Execute transactions                │
│  - Blackboard state machine            │
└───────────────────────────────────────┘
```

### 3.2 Phase Costs

| Phase | Time Complexity | Parallelizable |
|-------|----------------|----------------|
| Lexer | O(n) | Yes (per file) |
| Parser | O(n) | Yes (per file) |
| TypeChecker | O(n × depth) | Yes (per file) |
| ProofEngine | O(n²) worst | Partially |
| Annotator | O(V + E) + path enumeration | Yes (per file) |
| Interpreter | O(statements × iterations) | No |

### 3.3 Independence Property

**Key Decision:** Annotator, TypeChecker, and ProofEngine are **independent**.
- They all consume Parser output
- They do not communicate with each other
- They can run in parallel

This independence enables:
- Parallel static analysis
- Independent caching per phase
- Clear error reporting

---

## 4. Incremental Compilation

### 4.1 Why Incremental?

| Context | Requirement |
|---------|-------------|
| Interactive editing | <100ms feedback |
| Rendered Brief web | Hot reload |
| Large codebases | Don't re-verify unchanged code |

### 4.2 Durability Levels

**Decision:** Three-tier durability system for input changes.

| Level | Contents | Invalidation Frequency | Compiler Priority |
|-------|----------|------------------------|-------------------|
| **VOLATILE** | Currently edited file | Every keystroke (debounced) | Immediate |
| **NORMAL** | Other project files | On save | Next compilation |
| **STABLE** | Dependencies, stdlib | Rarely | Background |

### 4.3 Interface Hash Invalidation

**Core Optimization:** Cache proofs at the **interface level**, not file level.

```rust
struct FileCache {
    source_hash: u64,
    interface_hash: u64,    // Hash of exports + signatures + public types
    parsed_ast: Arc<Program>,
    type_results: Vec<TypeError>,
    proof_results: Vec<ProofError>,
}

fn is_cache_valid(file: &FileCache, new_source: &str) -> bool {
    source_hash(new_source) == file.source_hash
}
```

### 4.4 Invalidation Rules

| What Changed | Invalidate | Why |
|--------------|------------|-----|
| Source code | Own cache | Types/proofs may change |
| Interface hash | Importers' cache | Their proofs may depend on exports |
| Manifest | Everything | Dependency graph may change |

### 4.5 Early Cutoff

**Decision:** Before recomputing, check if output would change.

```rust
fn should_reprove(item: &Item, cache: &ProofCache) -> bool {
    // 1. Has source changed?
    if cache.source_hash != current_hash { return true; }
    
    // 2. Have any dependencies (callees) changed?
    for dep in &item.dependencies {
        if cache.dep_interfaces[dep].stale { return true; }
    }
    
    // 3. Early cutoff: nothing changed
    return false;
}
```

---

## 5. Import System

### 5.1 Search Convention

**Decision:** No manifest lookup required to find local modules.

```
User writes:     import { login } from auth;
Compiler searches:
    ./lib/auth.bv
    ./imports/auth.bv
    ./auth.bv
```

**Search order:** `lib/` → `imports/` → `./` (root)

### 5.2 Import Resolution Flow

```
1. Parse import statement
2. Extract module name (e.g., "auth")
3. Check manifest:
   ├─ IN manifest → Use declared path
   └─ NOT in manifest:
      ├─ Found locally → Auto-add to manifest, proceed
      └─ Not found → Error with resolution hints
```

### 5.3 Import Statement Structure

```bnf
import_stmt ::= "import" "{" import_list "}" "from" source ";"
import_list ::= identifier ("," identifier)*
source ::= identifier
```

### 5.4 Why Convention Over Manifest-Only

- Reduces friction for new users
- Auto-discovery is fast (file system scan)
- Manifest provides reliability + registry support

---

## 6. Manifest System

### 6.1 File: `brief.toml`

**Decision:** TOML format. Standard, well-understood, tooling exists.

### 6.2 Structure

```toml
[project]
name = "my-app"
version = "0.1.0"
entry = "main.bv"

[dependencies]
auth = { path = "lib/auth.bv" }
utils = { path = "lib/utils.bv" }
std-io = { registry = "brief-std", version = "1.0.0" }
```

### 6.3 Dependency Declaration Options

| Property | Description | Example |
|----------|-------------|---------|
| `path` | Local file relative to project root | `path = "lib/auth.bv"` |
| `registry` | Package from registry | `registry = "brief-std"` |
| `version` | Semver constraint | `version = "1.0.0"` |
| `git` | Git URL (future) | `git = "https://..."` |
| `optional` | May not be installed | `optional = true` |

### 6.4 CLI Commands

```bash
brief import <name>              # Add from registry
brief import <name> --path <loc> # Add local path
brief install                    # Install all from manifest
brief list                      # List dependencies
brief remove <name>             # Remove from manifest
```

### 6.5 Error Resolution

When import not in manifest and not found locally:

```
error[E0001]: unresolved import 'auth'

  --> src/main.bv:3:20
   |
3  | import { login } from auth;
   |                       ^^^^

help: 'auth' is not in your brief.toml

  Available packages in scope:
    - ./lib/auth.bv       (found, not yet declared)
    
  To add 'auth', either:
    1. Run: brief import auth --path lib/auth.bv
    2. Or add manually to brief.toml:
       
       [dependencies]
       auth = { path = "lib/auth.bv" }
```

---

## 7. Watch Mode

### 7.1 Trigger Strategy

**Decision:** On-save + debounced hybrid.

| Trigger | Latency | Use Case |
|---------|---------|----------|
| On save | ~50ms | Immediate feedback |
| Debounced (300ms) | ~350ms | Continuous typing |
| Background idle | ~500ms | Full graph validation |

### 7.2 Interface-Level Granularity

**Core Optimization:** Watch at the interface level, not file level.

```
When main.bv is edited:
├─ Re-typecheck main.bv
├─ Re-verify main.bv proofs
├─ Compute interface hash
│   └─ (exports, signatures, public types)
├─ Compare to cached interface hash
│   │
│   ├─ Same → Importers unaffected! Done.
│   │
│   └─ Changed:
│       ├─ Invalidate importers' caches
│       └─ They'll re-verify on next access
```

### 7.3 Safety Guarantee

> "Any proof that was valid remains valid unless its inputs changed."

This is maintained by:
1. Re-typecheck on any edit (types always current)
2. Re-verify on any edit (proofs always current)
3. Interface hash determines downstream invalidation

### 7.4 Watch Events

```rust
enum WatchEvent {
    FileSaved { path: PathBuf },    // Re-check this file
    ManifestChanged,                 // Full restart
    DependencyInstalled,             // Re-resolve imports
}
```

### 7.5 Watch Mode Algorithm

```rust
fn on_file_saved(path: &Path) {
    // 1. Re-typecheck
    let type_errors = typecheck(path);
    
    // 2. Re-verify proofs
    let proof_errors = prove(path);
    
    // 3. Compute new interface hash
    let new_iface_hash = compute_interface_hash(path);
    
    // 4. Compare to cache
    if new_iface_hash != cached_interface_hash(path) {
        // Interface changed - invalidate importers
        for dependent in dependents_of(path) {
            invalidate_cache(dependent);
        }
        cache_interface_hash(path, new_iface_hash);
    }
    
    // 5. Report
    report_errors(type_errors, proof_errors);
}
```

---

## 8. Cache Architecture

### 8.1 Cache Location

**Decision:** Project-local cache, version-aware.

```
.brief-cache/
├── manifest.json          # Cache config, version
├── modules/
│   ├── auth.ast          # Parsed (hash of source)
│   ├── auth.types        # Type results
│   ├── auth.proofs       # Proof results
│   └── ...
└── graphs/
    ├── call-graph.json   # Full call graph (incremental update)
    └── dependency-graph.json
```

### 8.2 Cache Keys

| Cache Type | Key Components |
|------------|---------------|
| Parsed AST | Source hash + compiler version |
| Type results | AST hash + type env |
| Proof results | Type results hash + proof config |
| Interface | Exports hash + signature hashes |

### 8.3 Cache Invalidation Strategy

```
Manifest changes → Invalidate everything
Interface changes → Invalidate importers
Source changes → Invalidate own + interface
```

### 8.4 Content-Addressing

**Decision:** Content-addressed storage for artifacts.

```
Source: "import { foo } from bar;"
     ↓
Hash: sha256(source + version + flags)
     ↓
Cache key: "ab3f8c2d..." → ./modules/auth.proofs
```

Benefits:
- Deterministic (same input = same key)
- No timestamp dependency
- Easy to verify integrity

---

## 9. Rendered Brief (Future)

### 9.1 Derivation Model

```
Brief Compiler Core
        │
        ├── .bv interpretation (desktop)
        │
        └── .rbv compilation
                │
                ▼
        ┌───────────────────────┐
        │   .rbv Parser         │  Extract <script> + <view>
        │   (extends Brief)      │
        └───────────────────────┘
                │
                ▼
        ┌───────────────────────┐
        │   Semantic Analysis    │  Symbol table, directives
        │   (extends Brief)      │
        └───────────────────────┘
                │
                ▼
        ┌───────────────────────┐
        │   WASM Code Gen        │  Emit Rust → wasm32
        └───────────────────────┘
                │
                ▼
        ┌───────────────────────┐
        │   JS Glue Gen         │  DOM binding layer
        └───────────────────────┘
```

### 9.2 Shared Components

| Component | Brief | Rendered Brief |
|-----------|-------|----------------|
| Lexer | ✓ | ✓ (extends) |
| Parser | ✓ | ✓ (extends) |
| TypeChecker | ✓ | ✓ |
| ProofEngine | ✓ | ✓ (extends) |
| Annotator | ✓ | ✓ |
| Reactor | ✓ | ✓ (extends) |
| Import Resolver | ✓ | ✓ |
| Cache Manager | ✓ | ✓ |

### 9.3 Incremental Strategy for Web

| Scenario | Strategy |
|----------|----------|
| Single file edit | Interface hash invalidation |
| Directive change | Recompile signal bindings only |
| Transaction edit | Recompile transaction + affected signals |
| View edit | Incremental WASM rebuild |

### 9.4 WASM Compilation

**Decision:** Rust source emit → `wasm-pack` → `wasm32-unknown-unknown`

Rationale:
- Maximum portability
- Rust WASM ecosystem mature
- Debugging via `wasm-objdump`
- Size optimization via `wasm-opt`

### 9.5 Browser Caching

**Decision:** IndexedDB for client-side cache.

```javascript
// Cache structure
{
  "artifact-key": {
    "wasm": ArrayBuffer,
    "binding-table": JSON,
    "interface-hash": string
  }
}
```

---

## 10. Decision Log

| Date | Decision | Rationale | Status |
|------|----------|-----------|--------|
| 2026-04-04 | Three-tier durability (VOLATILE/NORMAL/STABLE) | Optimizes watch mode feedback | Active |
| 2026-04-04 | Interface-level cache invalidation | Enables safe incremental proofs | Active |
| 2026-04-04 | Early cutoff before recomputation | Avoids wasted work | Active |
| 2026-04-04 | Convention-based import discovery | Reduces friction, manifest optional | Active |
| 2026-04-04 | Manifest-driven package management | Explicit deps, registry-ready | Active |
| 2026-04-04 | TOML for manifest | Standard, tooling exists | Active |
| 2026-04-04 | On-save + debounced watch | Balance feedback speed vs. stability | Active |
| 2026-04-04 | Rendered Brief derived from Brief | Shared core, separate compilation | Planned |
| 2026-04-04 | Rust → wasm-pack for WASM | Portability, mature ecosystem | Planned |

---

## Appendix: Future Considerations

### Registry System
Not implemented in v1.0. Design allows for:
```toml
std-io = { registry = "brief-std", version = "1.2.0" }
```

### Workspace Support
Not implemented in v1.0. Design allows for:
```toml
[workspace]
members = ["packages/*"]
```

### Plugin System
Not implemented in v1.0. Design allows for:
```toml
[plugins]
linter = { path = "plugins/linter.bv" }
```

---

*Last updated: 2026-04-04*
