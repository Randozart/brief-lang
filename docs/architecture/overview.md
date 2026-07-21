# Brief Compiler Architecture Overview

> **2026-07-20:** The three-layer architecture (CTD + ALU + TOML config) described
> below is superseded by the **hashword protocol system**. See
> `docs/architecture/casting-protocol.md` and
> `docs/plans/2026-07-20-extensible-number-types-final.md`. TOML config files
> (`llvm-ops.toml`, `ctd-llvm-mappings.toml`) are removed; hashwords in op
> signatures are the replacement.
>
> **2026-07-21:** The four-stage plugin system (Front/Mid/Post/Back) was replaced
> with 11 granular stages (PreLex/Parsed/Resolved/Typed/Normalized/Verified/
> Allocated/Provenanced/Generated/Optimized/Linked) and a direct AST navigation DSL.
> See `docs/plans/2026-07-21-granular-pipeline-and-ast-navigation.md`.
> The old `Collect$`/`MatchIR$` serialize-deserialize intrinsics are removed.
> Plugins now navigate the live AST via `Tag$`, `Named$`, `ForEach$`, etc.
> Full Brief code (`defn`/`let`/`if`/`match`) is evaluated at compile time inside
> `$(Stage)` blocks (Level C). Plugins can inject other plugins via `Stage$.Insert$`.
> Diagnostics via `EmitInfo$`/`EmitWarning$`/`EmitError$`.

## Pipeline

```
                         FAST PATH (default, zero overhead)
Source ─► PreLex ─► Lex ─► Parse ─► Parsed ─► Resolve ─► Resolved ─► TypeCheck
                                           │            │               │
                                        AST plugins  AST snapshot    AST snapshot

──► Typed ─► Normalize ─► Normalized ─► Verify ─► Verified ─► AllocAnalyze
      │          │              │           │           │
   AST        AST            AST         AST         AST
   snapshot                                     snapshot

──► Allocated ─► Provenance ─► Provenanced ─► Codegen ─► Generated ─► Optimize
      │              │                │                      │
   AST            AST              AST                   Ir$ plugins
   snapshot                      snapshot                    snapshot

──► Optimized ─► Link ─► Linked
      │                     │
    Ir$                  Bin$ plugins
   snapshot

Every stage is a plugin hook point.  Plugins use the AST Navigation DSL
(Tag$, Named$, First$, Insert$, etc.) for tree stages and text operations
(Find$, ReplaceWith$, etc.) for text/IR stages.


                         PLUGIN ARCHITECTURE (2026-07-21)
Each stage maps to a $(StageName) block in .bv source or plugins/{stage}/.bv
files. Plugins operate on the LIVE data (no serialize/deserialize):

  $(Parsed) @ highest {
      Tag$("import").First$().Before$()
          .Insert$(Import$("std/types/bootstrap.bv"));
  };


                         BEAST DEBUG PATH (--emit-beast)
Source ─► ... ─► Parsed ─► Resolved ─► Typed ─► ... ─► Provenanced
                    │            │          │                │
               .beast.parse  .beast.resolve .beast.types  .beast.prov

--emit-beast writes .beast files at each AST stage for programmer
visualization.  Plugins never read .beast — they operate on the live AST.


                          BACKEND DISPATCH (--backend selects)
Source → Parse → Resolve → NORMALIZE → Codegen → Generated → Optimize → Binary
                               │                      │           │
                          Reads backend configs    Ir$ plugins  Ir$ plugins
                          Walks AST once           (text ops)   (text ops)
                          Attaches annotations

                     OPTIONAL PRE-CODEGEN ANALYSES
                         ┌─────────────────────┐
                         │ Allocation DAG      │
                         │ (analyze_alloc_     │
                         │  strategies)        │
                         │ → per-call-site     │
                         │   strategy map      │
                         └─────────────────────┘
                         ┌─────────────────────┐
                         │ Provenance Analysis │
                         │ (dangling pointers) │
                         └─────────────────────┘

## Normalizer Stage

The normalizer runs between the plugin chain and codegen. It reads the backend's config files and walks the entire AST once, attaching pre-resolved annotations so the codegen never needs to read config files or match on `primitive`/`bytes`.

### What each normalizer does

| Backend | Normalizer | Annotations attached | Metadata stripped |
|---------|-----------|---------------------|-------------------|
| **LLVM** | `LlvmNormalizer` | `llvm_type` ("double", "i64", "ptr") on every type ref | `hardware`, `jira_ticket`, `rest_route`, `encoding` if unused |
| **CIRCT** | `CirctNormalizer` | `bit_width` (64, 32, 16) on every type ref | `primitive`, `encoding`, `llvm_type`, `jira_ticket` |
| **GPU** | `GpuNormalizer` | `llvm_type` + `gpu_kernel` on kernel Txns | `hardware`, `jira_ticket` |
| **Webstack** | `WebstackNormalizer` | `js_type` ("number", "string", "boolean") on every type ref | `hardware`, `bit_width` |

### Normalizer trait

```rust
pub trait BackendNormalizer: std::fmt::Debug {
    fn name(&self) -> &str;
    fn normalize(&self, items: &mut Vec<TopLevel>, universe: &mut TypeUniverse) -> Result<(), String>;
}
```

### How the normalizer resolves operations

Every `Expr::Call("Add#", args)` in the AST is resolved by the normalizer:

1. Look at the argument types → derive `primitive` + `bytes`
2. Look up `(Add, primitive, bytes)` in backend's `ops.toml`
3. If found: attach the resolved lowering as an annotation on the Call node
4. If not found: error — operation not supported for these types on this backend

The backend reads the annotation and emits the pre-resolved IR template directly.

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
| **Intrinsic** | `Sqrt#(x)`, `Malloc#(64)` | Normalizer validates against backend's supported list; backend chooses how to emit | Backend declares support in config |
| **Override** | `op Add <~ custom_fn(#L, #R)` | Normalizer registers type-level override in dispatch table | Per-type in source |

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

### Mid-End — BEAST (`src/beast/`)

| Module | Purpose |
|--------|---------|
| `beast/sexpr.rs` | S-expression tokenizer, parser, pretty-printer |
| `beast/serialize.rs` | Walk `Vec<TopLevel>` + `TypeUniverse` → `.beast` text |
| `beast/deserialize.rs` | `.beast` text → `Vec<TopLevel>` + `TypeUniverse` |
| `beast/pattern.rs` | Pattern compiler for `.beast` query syntax (retained for `Pattern$`) |
| `beast/layout.rs` | Layout DSL parser for metadata annotations |

### AST Navigation Macros (`src/macros/`)

| Module | Purpose |
|--------|---------|
| `macros/selection.rs` | `Selection`, `Selector` trait, traversal (children/descendants/parent/etc.) |
| `macros/pattern_live.rs` | Live AST pattern compiler — matches `.beast` patterns on `Vec<TopLevel>` directly |
| `macros/actions.rs` | `Position`, mutation ops (insert/delete/replace/wrap/rename/set) |
| `macros/text_ops.rs` | `TextSelection`, text operations for Source$/Ir$ (find/replace/insert/delete) |
| `macros/flow.rs` | `foreach`/`if`/`match` evaluation as interpreter special forms |
| `macros/stage_target.rs` | `Stage$.Insert$`, `Stage$.Remove$`, `Stage$.List$` |
| `macros/compile_time.rs` | Compile-time `defn`/`let` evaluation via interpreter |
| `macros/diagnostics.rs` | `EmitInfo$`, `EmitWarning$`, `EmitError$` |

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
| `plugin/mod.rs` | `Plugin` trait, `PluginManager`, per-stage dispatch |
| `plugin/loader.rs` | System plugin discovery from `plugins/{stage}/`, inline `$(Stage)` block extraction |
| `plugin/intrinsics.rs` | `$` intrinsic dispatch — navigation chains, AST constructors, diagnostics |

### Normalizer (`src/backend/normalizer.rs`)

| Module | Purpose |
|--------|---------|
| `normalizer.rs` | Shared helpers: `attach_llvm_types()`, `validate_intrinsics()`, `collect_intrinsic_calls()` |
| `llvm/normalizer.rs` | `LlvmNormalizer` — attaches `llvm_type`, strips irrelevant metadata |
| `circt/normalizer.rs` | `CirctNormalizer` — attaches `bit_width` |
| `webstack/normalizer.rs` | `WebstackNormalizer` — attaches `js_type` |
| `gpu/normalizer.rs` | `GpuNormalizer` — attaches `llvm_type`, marks kernel entry points |

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

## Feature Flags

| Flag | Default | Effect |
|------|---------|--------|
| `feature_sso_strings` | `false` | SSO: String becomes `{i64,i64}` struct, ≤6 bytes inline |
| `feature_svo` | `false` | SVO: `List<T>` becomes (N+1)-slot struct, ≤3 elements inline |

## Build Modes

| Mode | Command | Output |
|------|---------|--------|
| Default | `briefc build file.bv` | Executable binary |
| LLVM IR | `briefc build --llvm` | `file.ll` |
| Static library | `briefc library file.bv` | `libfile.a` |
| Shared library | `briefc build --shared` | `file.so` |

## Pure-Brief Standard Library Functions

Several byte-string operations are implemented in pure Brief (no frgn) using
`Load#` + convergent `txn` loops with convergence contracts:

| Function | File | Algorithm |
|----------|------|-----------|
| `memcmp(a, b, len)` | `lib/std/types/utf8view.bv` | Byte-by-byte loop via `txn [i < len][i == len]` |
| `utf8_find(hay, hay_len, needle, needle_len)` | `lib/std/types/utf8view.bv` | Nested txns: `find_loop` calls `memcmp_at` per position |
| `utf8_validate(data, len)` | `lib/std/types/utf8view.bv` | Single txn decodes lead byte, checks continuations, validates code points |
| `smallstring_get`, `push_byte`, etc. | `lib/std/types/small_string.bv` | `when`-chained slot selection for inline 64-byte buffer |

These replace the earlier `frgn` declarations. The compiler can optimize
the convergent txn loops through SROA + loop unrolling.

## Config Files

| File | Purpose |
|------|---------|
| `config/targets.toml` | File extension → backend routing |
| `config/ctd-llvm-mappings.toml` | (ctd, bytes) → LLVM type string |
| `config/llvm-ops.toml` | (operation, primitive, bytes) → LLVM IR template |
| `config/circt-ops.toml` | (operation, primitive, bytes) → MLIR template |
| `config/alloc-strategies.toml` | Custom allocation strategy templates + Free# dispatch |
| `config/encodings.toml` | String encoding metadata (char_width, ops for index_at/char_len) |
| `config/webstack-ops.toml` | (operation, primitive, bytes) → JS/TS template |

## Key Source Files

| File | Purpose |
|------|---------|
| `src/config.rs` | `TypeConfig`, `OpConfig`, `derive_llvm_type()` |
| `src/target.rs` | `TargetConfig`, `BackendKind` resolution |
| `src/compile.rs` | Compilation pipeline with 11-stage dispatch, `BeastStage` snapshots |
| `src/macros/` | AST navigation DSL: `selection.rs`, `pattern_live.rs`, `actions.rs`, `text_ops.rs`, `flow.rs` |
| `src/main.rs` | CLI entry point with `--backend`, `--emit-beast [stage]` |
| `src/lib.rs` | Crate root, module declarations |
| `docs/architecture/backend-type-dispatch.md` | Type dispatch design (mandatory reading) |
