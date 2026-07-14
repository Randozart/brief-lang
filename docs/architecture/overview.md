# Brief Compiler Architecture Overview

## Pipeline

```
                        FAST PATH (default, zero overhead)
Source ─► Lex ─► Parse ─► Resolve ─► Analyze ─► Codegen ─► (.ll) ─► clang ─► binary
                              │                       │
                              │  TypeUniverse          │  Backend chosen by
                              │  populated from         │  --backend flag or
                              │  bootstrap.bv +         │  config/targets.toml
                              │  TypeDefs               │
                              ▼                       ▼
                          Read-only                 Read-only


                         PLUGIN PATH (--plugin path/to/exe)
Source ─► Lex ─► Parse ─► Resolve ─► serialize ─► [PLUGIN CHAIN] ─► deserialize ─► Analyze ─► Codegen ─► .ll
                              │            │                                  │
                              │       .bvir text                         .bvir text
                              │       (stdin/pipe)                       (stdout/pipe)
                              ▼            ▼                                  ▼
                          TypeUniverse   plugins see                   plugins see
                          written as     &mut Vec<TopLevel>            &mut String
                          (universe ...) + &mut TypeUniverse          (final IR)


                         BVIR DEBUG PATH (--emit-bvir)
Source ─► ... ─► Resolve ─► serialize ─► [PLUGIN CHAIN] ─► deserialize ─► ... ─► .ll
                              │            │                    │
                              │       ┌────┴────┐          ┌───┴────┐
                              │    program.bvir      program.bvir
                              │    .before           .after
                              ▼
                          Plugin authors diff these
                          to see what their plugin mutated


                         BACKEND DISPATCH (--backend selects)
Source → Parse → Resolve → match opts.backend {
    "llvm"     → LlvmBackend::new().generate()   → .ll  → clang → binary
    "circt"    → CirctBackend::new().generate()  → .mlir → circt-opt → verilog
    "webstack" → WebstackGenerator::generate()   → .ts   → tsc → wasm
}
```

## Frontend/Backend Detachment

```
SOURCE ──► PARSER ──► AST ──► UNIVERSE ──► BACKEND
                           │                  │
                           │  NEVER mutates   │
                           │  the AST         │
                           └──────────────────┘

The frontend OWNS:   Expr, Statement, Type, TopLevel, TypeDef, PropertyValue
The backend READS:    All of the above, via &reference
The backend OWNS:    CompilerContext, TypedRegister, output string

Backend selection is driven by config/targets.toml:
  [.bv]   backend = "llvm"
  [.ebv]  backend = "llvm"
  [.cbv]  backend = "circt"
  [.wbv]  backend = "webstack"

CLI: --backend overrides the config file.
```

## Operations vs Intrinsics

Brief distinguishes between two kinds of compiler-known operations:

| Kind | Syntax | Dispatch | Configurable |
|------|--------|----------|--------------|
| **Operation** | `+`, `==`, `++`, `list[i]` | Config file: `(op, primitive, bytes)` → IR template | Yes — any backend provides its own `ops.toml` |
| **Intrinsic** | `Sqrt#(x)`, `Malloc#(64)` | Backend chooses: LLVM intrinsic, external call, or error | Backend decides per call |
| **Override** | `op Add <~ custom_fn(#L, #R)` | Type registry check before config lookup | Per-type in source |

Example:
```brief
a + b           → Expr::BinaryOp(Add, a, b)    →  Operation — config file dispatch
Sqrt#(x)        → Expr::Call("Sqrt#", [x])     →  Intrinsic — backend chooses
my_int + other  → Expr::BinaryOp(Add, my, other) →  Operation, but type override checked first
```

### Operation dispatch

```
Expr::BinaryOp(Add, lhs, rhs)
  │
  ├─ Cross-type override: (Add, lhs.ty, rhs.ty) in universe?
  │    └─ Yes: emit call to override function
  │
  ├─ Same-type override: lhs.ty.properties["op.Add"] ?
  │    └─ Yes: emit call to override function
  │
  ├─ Config lookup: (Add, primitive, bytes) in llvm-ops.toml?
  │    └─ Yes: emit LLVM IR template with %v, %a, %b
  │
  └─ Fallback: error — operation not supported
```

### Override syntax

```brief
// Same-type: inside type definition
type Int <: Bits {
    bytes <~ 8;
    primitive <~ Int;
    op Add <~ add_int_int(#L, #R);   // Int + Int → custom function
}

// Cross-type: top-level declaration
op Add(Int, String) <~ add_int_string(#L, #R);
op Add(String, Int) <~ add_string_int(#L, #R);
```

## Type System — Metadata Driven

The compiler does NOT hardcode primitive type mappings in Rust match arms.
Type metadata declared in **source** drives all backend emission decisions.

```
Source type definition:
  type Int <: Bits { bytes <~ 8; primitive <~ Int; }

Flow:
  parser ──► TypeDefBody.metadata["primitive"] = "Int"
  resolve ──► ResolvedType.properties["primitive"] = PropertyValue::Identifier("Int")
  codegen ──► derive_llvm_type(Some("Int"), 8, &config) → "i64"
```

The Rust binary is a thin reader of the type system defined in Brief source.

## How the Backend Derives Types (Backend Chooses)

The compiler provides ALL metadata. The backend selects what it needs:

| Approach | What the backend reads | When it's used |
|----------|----------------------|----------------|
| **`primitive` + `bytes`** | `ResolvedType.properties["primitive"]` + `bytes` | LLVM default |
| **Raw `bytes` only** | Just `bytes`, ignores properties | CIRCT, GPU |
| **Other metadata** | `encoding`, `alignment`, custom properties | Webstack, framework plugins |

A backend can start with just `bytes` and be fully correct. It then opts into `primitive` and other properties as needed.

## Modules

### Frontend (`src/ast/`, `src/parser/`, `src/lexer.rs`)

| Module | Purpose |
|--------|---------|
| `ast/expr.rs` | `Expr` enum — all expression variants |
| `ast/top.rs` | `TopLevel`, `Statement`, `Transaction`, `TypeDef`, `Trigger`, etc. |
| `ast/types.rs` | `Type` enum, `PropertyValue` — type system |
| `lexer.rs` | Logos-based tokenizer |
| `parser/` | Recursive descent parser → `Vec<TopLevel>` |

### Mid-End — BVIR (`src/bvir/`)

| Module | Purpose |
|--------|---------|
| `bvir/sexpr.rs` | S-expression tokenizer, parser, pretty-printer |
| `bvir/serialize.rs` | Walk `Vec<TopLevel>` + `TypeUniverse` → `.bvir` text |
| `bvir/deserialize.rs` | `.bvir` text → `Vec<TopLevel>` + `TypeUniverse` |

### Analysis (`src/analysis/`)

| Module | Purpose |
|--------|---------|
| `region.rs` | `RegionAnalyzer` — 9 phases |
| `transition_graph.rs` | `ReactorTransitionGraph` — reactive transaction scheduling |
| `dependency_graph.rs` | `DependencyGraph` — topological ordering of state fields |

### Target Config (`src/target.rs`)

| Module | Purpose |
|--------|---------|
| `target.rs` | `TargetConfig` — reads `config/targets.toml`, resolves `BackendKind` |

### Plugins (`src/plugin/`)

| Module | Purpose |
|--------|---------|
| `plugin/mod.rs` | `Plugin` trait, `PluginManager`, `PluginHook` |
| `plugin/loader.rs` | Native `.so`/`.dylib` loading |
| `plugin/runner.rs` | External plugin chain (stdin/stdout BVIR) |

### Backend (`src/backend/`)

| Module | Purpose |
|--------|---------|
| `llvm/mod.rs` | `LlvmBackend` — LLVM IR emission |
| `llvm/emit_expr.rs` | Expression → LLVM IR |
| `llvm/emit_stmt.rs` | Statement → LLVM IR |
| `llvm/emit_toplevel.rs` | Top-level → LLVM IR, `llvm_type()` |
| `llvm/helpers.rs` | `emit_async_phase()`, `type_is()`, casts |
| `llvm/types.rs` | `lower_type()`, `type_size()` |
| `circt.rs` | `CirctBackend` — MLIR emission |
| `webstack.rs` | `WebstackGenerator` — TypeScript + WASM |

## Config Files

| File | Purpose |
|------|---------|
| `config/targets.toml` | File extension → backend routing |
| `config/llvm-primitives.toml` | (primitive, bytes) → LLVM type string |
| `config/llvm-ops.toml` | (operation, primitive, bytes) → LLVM IR template |
| `config/circt-ops.toml` | (operation, primitive, bytes) → MLIR template |
| `config/webstack-ops.toml` | (operation, primitive, bytes) → JS/TS template |

## Key Source Files

| File | Purpose |
|------|---------|
| `src/config.rs` | `TypeConfig`, `OpConfig`, `derive_llvm_type()` |
| `src/target.rs` | `TargetConfig`, `BackendKind` resolution |
| `src/compile.rs` | Compilation pipeline with backend dispatch |
| `src/main.rs` | CLI entry point with `--backend`, `--plugin`, `--emit-bvir` |
| `src/lib.rs` | Crate root, module declarations |
| `docs/architecture/backend-type-dispatch.md` | Type dispatch design (mandatory reading) |
