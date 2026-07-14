# Brief Compiler Architecture Overview

## Pipeline

```
                        FAST PATH (default, zero overhead)
Source ─► Lex ─► Parse ─► Resolve ─► Analyze ─► Codegen ─► (.ll) ─► clang ─► binary
                              │                       │
                              │  TypeUniverse          │  CompilerContext
                              │  populated from         │  built during
                              │  bootstrap.bv +         │  generate()
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
The backend OWNS:    CompilerContext, TypedRegister, LLVM IR output string
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
| `region.rs` | `RegionAnalyzer` — 9 phases: declarations, deps, frontier, regions, value sets, chains, bounds, scores |
| `transition_graph.rs` | `ReactorTransitionGraph` — reactive transaction scheduling |
| `dependency_graph.rs` | `DependencyGraph` — topological ordering of state fields |

### Plugins (`src/plugin/`)

| Module | Purpose |
|--------|---------|
| `plugin/mod.rs` | `Plugin` trait, `PluginManager`, `PluginHook`, `PluginAction` |
| `plugin/loader.rs` | Native `.so`/`.dylib` loading via `libloading` |
| `plugin/runner.rs` | External plugin subprocess chain (stdin/stdout BVIR) |

### Backend (`src/backend/llvm/`)

| Module | Purpose |
|--------|---------|
| `context.rs` | `CompilerContext` — all backend state |
| `mod.rs` | `LlvmBackend`, `generate()`, `build_field_index()`, report, main emission |
| `emit_expr.rs` | Expression → LLVM IR |
| `emit_stmt.rs` | Statement → LLVM IR |
| `emit_toplevel.rs` | Top-level item → LLVM IR, `llvm_type()`, `init_state` |
| `helpers.rs` | `emit_async_phase()`, `type_is()`, casts |
| `loop_engine/` | `emit_main()`, `emit_ssa_main()`, folded counters |
| `types.rs` | `lower_type()`, `type_size()`, config loader |

## Key Files

| File | Purpose |
|------|---------|
| `config/llvm-primitives.toml` | (primitive, bytes) → LLVM type string mapping |
| `src/config.rs` | `TypeConfig` reader, `derive_llvm_type()` |
| `src/compile.rs` | Compilation pipeline driver |
| `src/main.rs` | CLI entry point with `--plugin` and `--emit-bvir` |
| `src/lib.rs` | Crate root, module declarations |
| `docs/architecture/backend-type-dispatch.md` | Type dispatch design (mandatory reading) |
