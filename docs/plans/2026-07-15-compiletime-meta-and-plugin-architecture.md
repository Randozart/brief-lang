# Compile-Time Metaprogramming & Plugin Architecture

**Date:** 2026-07-15
**Status:** Plan — ready for implementation
**Directives:** See "Plan Directives" in AGENTS.md — this document and all
implementation commits must adhere to flat control flow, rationale comments,
example updates, doc updates, and behavioral tests.

---

## Table of Contents

1. [Summary](#summary)
2. [Pipeline Architecture](#pipeline-architecture)
3. [The Four Stages](#the-four-stages)
4. [Two Plugin Tiers](#two-plugin-tiers)
5. [Priority System](#priority-system)
6. [`$(Stage)` Syntax](#stage-syntax)
7. [Full `$` Intrinsic Catalog](#full--intrinsic-catalog)
8. [Import Syntax: `""` vs `<>`](#import-syntax---vs-)
9. [`AddressOf#<T>()` — Unified Trigger Architecture](#addressoft--unified-trigger-architecture)
10. [What Gets Scrapped](#what-gets-scrapped)
11. [Lexer Changes](#lexer-changes)
12. [Parser Changes](#parser-changes)
13. [AST Changes](#ast-changes)
14. [Plugin Loading & Wiring](#plugin-loading--wiring)
15. [Implementation Order](#implementation-order)
16. [File Change Inventory](#file-change-inventory)
17. [Testing Strategy](#testing-strategy)

---

## Summary

This plan replaces four categories of "magical" (implicit, hardcoded) compiler
behavior with an explicit `$(Stage)` metaprogramming system:

1. **Prelude injection** — currently hardcoded in `import_resolver.rs:146-215`,
   becomes `plugins/front/prelude.bv`.
2. **Macro/template expansion** — currently `src/features/macros/` (6 files),
   becomes `$(Stage)` blocks with `$` intrinsics.
3. **Trigger address hardcoding** — `LinkRef::Stdin`/`Timer`/`Signal` variants,
   becomes `AddressOf#<T>()` with type-carried listening strategies.
4. **Import path resolution** — TypeScript-style filesystem search, becomes
   `""` for file/project paths and `<>` for compiler config registry.

The result is a compiler where every behavior is either a system plugin (ships
with the compiler, `--disable-plugin` to turn off) or a user `$(Stage)` block
(written inline in Briev source).

---

## Pipeline Architecture

```
Source → $(Front) → Lex → Parse → Typecheck → $(Mid) → [DAG stitch / contract→metadata / sugar strip] → $(Post) → Normalizer → Codegen → $(Back) → Output
```

The pipeline has four fixed stages. Every stage is optional — if no plugins
are registered for a stage, it produces its input unchanged.

---

## The Four Stages

### `$(Front)` — Raw Source Manipulation

| Property | Value |
|----------|-------|
| **Runs** | Before lexing |
| **Input** | `&mut String` (raw source text per file) |
| **Output** | `&mut String` (modified source text) |
| **Pipeline position** | Immediately after reading the source file, before `Token::lexer()` |

**Use cases:**
- Inserting imports at the top of every file (prelude)
- Wrapping source in scripting mode boilerplate
- Text-level find-and-replace
- Adding/removing file headers and footers

### `$(Mid)` — Pre-Validation AST

| Property | Value |
|----------|-------|
| **Runs** | After typecheck, **before** DAG stitch / contract lowering / sugar stripping |
| **Input** | `&mut Vec<TopLevel>`, `&mut TypeUniverse` — full rich AST |
| **Output** | `&mut Vec<TopLevel>`, `&mut TypeUniverse` — modified AST |
| **Pipeline position** | Immediately after `typechecker::check_program()`, before the structural analysis pass |

**Use cases:**
- Inspecting and modifying contracts
- Rewriting transaction boundaries
- Structural analysis (cycle detection, completeness checks)
- Renaming symbols, injecting declarations
- Annotating types with properties (replaces normalizer work)

### `$(Post)` — Post-Optimization AST

| Property | Value |
|----------|-------|
| **Runs** | After DAG stitch / contract→metadata / sugar strip, **before** normalizer |
| **Input** | `&mut Vec<TopLevel>`, `&mut TypeUniverse` — stripped AST |
| **Output** | `&mut Vec<TopLevel>`, `&mut TypeUniverse` — modified AST |
| **Pipeline position** | After the lowering pass, before `normalizer::normalize()` |

**Use cases:**
- Final AST-level transformations before the backend sees it
- `.beast` pattern matching and rewriting (`MatchIR$`)
- Lowering intrinsics to backend-specific patterns
- Structural assertions and validation

### `$(Back)` — Backend IR Text

| Property | Value |
|----------|-------|
| **Runs** | After codegen, before write-to-disk |
| **Input** | `&mut String` (backend IR text: `.ll`, `.mlir`, `.ts`) |
| **Output** | `&mut String` (modified IR text) |
| **Pipeline position** | In each backend's `generate()` method, after IR emission, before `std::fs::write()` |

**Use cases:**
- Patching the emitted IR
- Overriding target triple or data layout
- Inserting instrumentation or debug probes
- Target-specific fixups that can't be expressed in the AST

---

## Two Plugin Tiers

| Tier | Location | Who writes | Enabled by | Priority |
|------|----------|------------|------------|----------|
| **System** | `plugins/{front,mid,post,back}/` directory | Compiler engineers | Default on, ships with compiler binary | `@ highest` (1000) |
| **User** | `$(Stage @ N) { ... }` inline in `.bv` files | Any Briev programmer | Declared in source file | `@ normal` (500) default |

### System Plugin Discovery

System plugins are loaded from the compiler's configured plugin directory.
The compiler searches:

1. `--plugin-dir` CLI flag
2. `BRIEV_PLUGIN_DIR` environment variable
3. Compiler installation path: `<executable_dir>/../share/briev/plugins/`
4. Project-local: `<project_root>/plugins/`

Each subdirectory (`front/`, `mid/`, `post/`, `back/`) contains `.bv` files.
Every `.bv` file in the directory is loaded and executed at that stage.

### Plugin Control CLI

```bash
# Disable a specific system plugin by name
briev-compiler build --disable-plugin prelude main.bv

# Disable all system plugins
briev-compiler build --disable-plugin '*' main.bv

# Load an additional user plugin from a path
briev-compiler build --enable-plugin ./my-plugin.bv main.bv
```

The `--disable-plugin` flag accepts a plugin name (filename stem) or `'*'`.
The `--enable-plugin` flag accepts a file path and loads it as a user plugin.

---

## Priority System

```briev
$(Front) { ... }                     // priority 500 (default = normal)
$(Front @ 100) { ... }               // explicit integer priority
$(Front @ highest) { ... }           // named alias
```

### Named Priority Levels

| Name | Value | Used by |
|------|-------|---------|
| `highest` | 1000 | System plugins (default) |
| `high` | 750 | — |
| `normal` | 500 | User inline blocks (default) |
| `low` | 250 | — |
| `lowest` | 0 | — |

### Execution Order

1. All system plugins for the stage, sorted by priority descending
2. All user inline blocks for the stage, sorted by priority descending

Within equal priorities, the order is declaration order (the order files are
encountered during import resolution). This is deterministic.

---

## `$(Stage)` Syntax

### Grammar

```
stage-block ::= "$" "(" stage-name ("@" priority)? ")" "{" body "}"
stage-name  ::= "Front" | "Mid" | "Post" | "Back"
priority    ::= INTEGER | "highest" | "high" | "normal" | "low" | "lowest"
body        ::= statement*
```

### Parsing Detail

The parser sees:
- `$` token (`Token::Dollar`)
- `(` token (`Token::LParen`)
- `Front` / `Mid` / `Post` / `Back` identifier (`Token::Identifier`)
- Optional `@` then priority (integer or identifier)
- `)` token (`Token::RParen`)
- `{` token, body statements, `}` token

Produces `TopLevel::StageBlock { stage: StageKind, priority: u32, body: Vec<Statement> }`.

### Examples

```briev
$(Front) {
    InsertRegistryImport$("std/types/bootstrap");
}

$(Mid @ 750) {
    for_each_type(SetTypeProperty$(name, "llvm_type", "i64"));
}

$(Post @ lowest) {
    EmitWarning$("this build uses post-stage plugins");
}

$(Back) {
    TargetTriple$("x86_64-unknown-linux-gnu");
}
```

### Body Execution

The body of a `$(Stage)` block is **regular Briev code** run at compile time.
The compiler:

1. Parses the body into `Vec<Statement>`
2. Registers the body as a plugin for that stage
3. At build time, evaluates the body in order, statement by statement
4. `$`-suffixed identifiers (`InsertRegistryImport$`, `Collect$`, etc.) are
   resolved as compiler-known intrinsics, not runtime function calls

All other Briev constructs (`let`, `if`, `defn`, type constructors, etc.) work
normally within the body. The only restriction: `$` intrinsics cannot be called
from outside a `$(Stage)` block.

---

## Full `$` Intrinsic Catalog

Every intrinsic is PascalCase + `$` suffix. The `$` is part of the identifier
(just like `Sqrt#`), so `InsertRegistryImport$` is a single token. All are
callable only within `$(Stage)` blocks.

### `$(Front)` — Source Text Manipulation

These intrinsics have access to the source text of the current file in the
compilation unit. When `ForEachFile$` is used, they apply to every file.

```
InsertFileImport$(path: String)
```
Inserts `import "path";` at the top of the current file.
Replaces: hardcoded prelude imports in `import_resolver.rs`.

```
InsertRegistryImport$(name: String)
```
Inserts `import <name>;` at the top of the current file.
Uses the compiler registry to resolve the name to a path.

```
WrapRange$(start_pattern: String, end_pattern: String, wrapper: String)
```
Wraps source text between `start_pattern` and `end_pattern` in `wrapper`.
The wrapper should contain a placeholder (e.g., `{{body}}`) that is replaced
with the matched text range.
Replaces: scripting mode wrapper generation.

```
PrependText$(text: String)
```
Prepends raw text to the beginning of the file.
Use case: license headers, `#!` shebangs.

```
AppendText$(text: String)
```
Appends raw text to the end of the file.
Use case: epilogue code, module registrations.

```
ReplaceText$(pattern: String, replacement: String)
```
Text-level find-and-replace using the pattern as a search string (not regex
unless the backend supports it). Operates on the entire file.
Replaces: preprocessing steps.

```
ForEachFile$(body: Block)
```
Iterates over every file in the compilation unit. The body block runs once
per file, with `InsertFileImport$`, `PrependText$`, etc. scoped to each file.
If `ForEachFile$` is not used, `$(Front)` intrinsics apply only to the entry
file.

### `$(Mid)` — Pre-Validation AST

These intrinsics operate on `Vec<TopLevel>` and `TypeUniverse` after
type-checking but before structural analysis.

```
InsertDecl$(position: Position, declaration: TopLevel)
```
Inserts a `TopLevel` declaration at the given position.
Position is one of: `Top`, `Before(name)`, `After(name)`, `End`.
Replaces: ad-hoc declaration injection in normalizers.

```
RemoveDecls$(pattern: String)
```
Removes all `TopLevel` declarations whose name matches `pattern` (glob-style).
Replaces: ad-hoc filtering of AST nodes.

```
Collect$(tag: String) -> Vec<&TopLevel>
```
Returns all AST nodes annotated with the given metadata tag.
The tag refers to `metadata <~` annotations on types or declarations.
Replaces: manual AST traversal for annotation collection.

```
Weave$(tag: String, body: Fn)
```
Scans all AST nodes annotated with `tag`. At each matching node, the body
function is called with `(node, universe)` and can generate/modify code at
that site. The body can call `InsertDecl$`, `SetTypeProperty$`, etc.
Replaces: prelude site injection, metadata-driven code generation.

```
Annotate$(pattern: String, key: String, value: PropertyValue)
```
Adds a metadata annotation `key = value` to every type or declaration whose
name matches `pattern`.
Replaces: the normalizer's `rt.properties.insert(...)` pattern.

```
Rename$(old_name: String, new_name: String)
```
Renames a symbol (defn, txn, type, field) across the entire AST.
Replaces: manual symbol table patching.

```
WrapBody$(fn_name: String, pre_statements: Vec<Statement>, post_statements: Vec<Statement>)
```
Injects `pre_statements` before and `post_statements` after every statement
in the named function's body. If the function is a `txn`, wraps the reactive
loop body.
Replaces: transaction boilerplate injection.

```
PromoteToEntry$()
```
Marks the most recently defined `defn` as the entry point (main function).
Replaces: implicit `main` detection.

```
SetTypeProperty$(type_name: String, key: String, value: PropertyValue)
```
Sets a property on a type in the `TypeUniverse`. Properties include
`llvm_type`, `primitive`, `bytes`, `layout`, etc.
Replaces: the normalizer's property annotation pass.

```
AssertAcyclic$()
```
Verifies the declaration graph (imports, dependencies, transaction calls) is
acyclic. Aborts with a diagnostic if a cycle is detected.
Replaces: cycle detection in `import_resolver.rs:in_progress`.

```
InjectImport$(path: String)
```
Injects an `import` node into the AST (not text-level — AST-level).
Useful when a `$(Mid)` plugin needs to ensure a module is loaded without
touching source text.

### `$(Post)` — Post-Optimization AST

These intrinsics operate on the stripped AST after contract lowering and
sugar removal.

```
MatchIR$(pattern: String, replacement: String)
```
Pattern-match and rewrite AST nodes using the `.beast` S-expression pattern
language. The pattern is parsed into an AST matcher; the replacement is
parsed as the new sub-AST. Both use the `.beast` format documented in
`src/beast/sexpr.rs`.
Replaces: the entire external BEAST plugin chain.

```
CollectAnnotated$(tag: String) -> Vec<&TopLevel>
```
Like `Collect$()` but only returns nodes whose annotations survived the
optimization passes (contract→metadata, sugar stripping). Annotations in
`properties` on `ResolvedType` are preserved.

```
AssertStructure$(condition: String)
```
Asserts a structural property of the AST. The `condition` is a Briev
expression evaluated at compile time. If it evaluates to `false`,
compilation aborts with a diagnostic.
Replaces: validation passes currently in the backend.

```
EmitDiagnostic$(kind: DiagnosticKind, message: String)
```
Emits a compiler diagnostic (Warning, Error, Note) to the user. The plugin
can use this to communicate with the programmer.
Replaces: `eprintln!` / `return Err(...)` in compiler internals.

```
LowerIntrinsic$(name: String, pattern: String)
```
Replaces all calls to intrinsic `name` with the given `.beast` lowering
pattern. For example, lowering `PrintInt#` to a backend-specific call.
Replaces: intrinsic lowering in individual backends.

```
FoldConstants$()
```
Triggers constant folding on the AST. All expressions with known constant
values are replaced with their evaluated results.
Replaces: explicit optimization pass invocation.

```
ReadConfig$(key: String) -> String
```
Reads a value from the compiler's configuration files at compile time.
Keys use dot notation: `"target.triple"`, `"stdlib.path"`, etc.
Replaces: `TypeConfig::load()` and `OpConfig::load()` in normalizers.

### `$(Back)` — Backend IR Text

These intrinsics operate on the emitted IR string (`.ll`, `.mlir`, `.ts`).
The IR string is the final output of the backend's `generate()` method.

```
PatchIR$(pattern: String, replacement: String)
```
Find-and-replace in the IR text. Pattern is a text search string (or regex
if the plugin opts in). Replacement is the new text.
Replaces: `PluginManager::run_ir_hooks()`.

```
InsertIRPrologue$(text: String)
```
Prepends text to the IR file, after any existing header/comments.
Replaces: backend `emit_header()` methods.

```
InsertIREpilogue$(text: String)
```
Appends text to the end of the IR file.
Replaces: backend `emit_footer()` methods.

```
InsertIRBefore$(sentinel: String, text: String)
```
Inserts `text` before the first line in the IR that contains `sentinel`.
Useful for injecting declarations before a specific function.

```
InsertIRAfter$(sentinel: String, text: String)
```
Inserts `text` after the first line in the IR that contains `sentinel`.

```
TargetTriple$(triple: String)
```
Overrides the target triple declaration in the emitted LLVM IR.
Replaces: `self.ctx.target_triple` in LLVM backend `emit_header()`.

```
DataLayout$(layout: String)
```
Overrides the data layout declaration in the emitted LLVM IR.
Replaces: `self.ctx.data_layout` in LLVM backend `emit_header()`.

### Cross-Stage Utility Intrinsics

Available in any `$(Stage)` block:

```
ReadConfig$(key: String) -> String
```
Reads compiler configuration. See description under `$(Post)`.

```
GetEnv$(var_name: String) -> String
```
Reads an environment variable at compile time. Returns `""` if unset.
Replaces: `std::env::var("BRIEV_STDLIB_PATH")` magic.

```
EmitWarning$(message: String)
```
Emits a compiler warning. The warning includes the plugin name and source
location for traceability.

```
EmitError$(message: String)
```
Aborts compilation with an error message. The error includes the plugin
name and source location.

```
Log$(message: String)
```
Writes to the compile-time log file (if `--verbose` or `--log <path>` is
passed to the compiler). No effect in normal builds.

```
Assert$(condition: bool, message: String)
```
Asserts a compile-time condition. Equivalent to:
```
if !condition { EmitError$(message); }
```

```
IncludeBEAST$(file_path: String)
```
Loads `.beast` pattern/replacement rules from an external file and registers
them as `MatchIR$` rules at the current stage.
Replaces: external plugin files without needing a subprocess.

---

## Import Syntax: `""` vs `<>`

### Current Behavior (Unchanged for `""`)

```
import "std/os/fs"          // file/project-anchored path
import "./relative"         // relative to current file
import "../other"           // relative to parent
```

Resolution order: relative to file, then relative to project root, then
search paths (`lib/`, `imports/`, `.`). Unchanged.

### New `<>` Syntax

```
import <std/os/fs>          // compiler registry lookup
import <my-project/utils>   // user-registered module
```

Resolution: looks up `<name>` in the module registry config file.

### Module Registry Config

File: `config/module-registry.toml`

```toml
[aliases]
"std/types/bootstrap" = { path = "lib/std/types/bootstrap.bv" }
"std/os/fs" = { path = "lib/std/os/fs.bv" }
"my-project/utils" = { path = "/home/user/projects/utils/utils.bv" }
```

Each entry maps a registry name to an absolute or project-relative path.

### CLI Registration

```bash
briev-compiler register my-project/utils /home/user/projects/utils/utils.bv
briev-compiler unregister my-project/utils
briev-compiler list-registry   # lists all registered modules
```

The `register` command appends to `config/module-registry.toml`. The
command operates on the project-local config by default, or a user-global
config at `~/.config/briev/registry.toml` with `--global`.

### What This Replaces

- The `import#` magic syntax (designed in `docs/architecture/prelude-and-import-magic.md`
  but never wired in the parser) is scrapped entirely.
- The automatic stdlib search (`resolve_stdlib_root()` in `import_resolver.rs:85-129`)
  is replaced by explicit registry entries.

### Parser Changes

After `import` keyword or `from` keyword:

```rust
// Current (unchanged):
if peek == String { parse_string_path() }

// New:
if peek == Lt { parse_angle_path() }  // <name> → import { module: "<name>", is_lookup: true }
```

The `<` token after `import`/`from` is unambiguous — it cannot start an
expression in that context.

### AST Changes

```rust
// Before:
pub struct Import {
    pub module: String,
    pub symbols: Vec<String>,
    pub span: Option<Span>,
}

// After:
pub enum ImportKind {
    /// import "path" or import {x} from "path"
    Literal(String),
    /// import <name> or import {x} from <name>
    Registry(String),
}

pub struct Import {
    pub kind: ImportKind,
    pub symbols: Vec<String>,
    pub span: Option<Span>,
}
```

### ImportResolver Changes

In `resolve_import()`, branch on `ImportKind`:
- `ImportKind::Literal(path)` → current filesystem search (unchanged)
- `ImportKind::Registry(name)` → look up in `config/module-registry.toml`
  and load the resolved path. If not found, compile error.

---

## `AddressOf#<T>()` — Unified Trigger Architecture

### Intrinsic Signature

```
AddressOf#<T?, strategy?>(id: String) -> T
```

- `T` (optional): the type to return. When omitted, inferred from the
  intrinsic's internal mapping of known resource IDs.
- `strategy` (optional): override the listening strategy. When omitted,
  uses the type's `listen <~` metadata.
- `id`: a string identifying the resource (`"stdin"`, `"SIGINT"`,
  `"uart0"`, `0xFFE01000` treated as string, etc.).

### Examples

```briev
// Simple — type inferred, strategy from type metadata
trg input @ AddressOf#("stdin").#line_ready;

// Explicit type parameter
trg rx @ AddressOf#<MMIO>(0xFFE01000).#data_ready;

// Explicit strategy override — backend validates
trg sig @ AddressOf#<Stdin, "poll">("stdin").#line_ready;

// Raw address — shortcut for AddressOf#<MMIO>
trg rx @ 0xFFE01000.#data_ready;

// Config-driven address
trg rx @ AddressOf#<MMIO>(from_config("CHIP_UART_PORT")).#data_ready;
```

### Type-Carried Listening Strategy

Types carry a `listen <~` metadata annotation that declares what listening
strategy the backend should use:

```briev
// std/types/io.bv — ships with the compiler
type Stdin {
    metadata listen <~ "select";
}

type MMIO {
    metadata listen <~ "poll";
}

type Signal {
    metadata listen <~ "sigaction";
}

type DomRef {
    metadata listen <~ "event";
}
```

### Backend Strategy Validation Table

| Backend | `"poll"` | `"select"` | `"sigaction"` | `"interrupt"` | `"event"` |
|---------|----------|------------|---------------|---------------|-----------|
| LLVM (Linux) | ✅ polling load | ✅ `select()` | ✅ `sigaction()` | ❌ compile err | ❌ |
| LLVM (macOS) | ✅ | ✅ `kqueue` | ✅ | ❌ | ❌ |
| LLVM (Windows) | ✅ | ✅ `WaitForMultipleObjects` | ✅ `SetConsoleCtrlHandler` | ❌ | ❌ |
| CIRCT (no_std) | ✅ | ❌ compile err | ❌ | ✅ ISR | ❌ |
| CIRCT (with RTOS) | ✅ | ✅ RTOS API | ❌ | ✅ | ❌ |
| Webstack | ✅ `setInterval` | ❌ | ❌ | ❌ | ✅ `addEventListener` |

The backend maps `listen <~` values to its native API. An unsupported
strategy is a compile error with a clear message listing supported values.

### `from_config()` — Compile-Time Config Read

```briev
AddressOf#<MMIO>(from_config("CHIP_UART_PORT"))
```

`from_config` is a compile-time function (not a `$` intrinsic — it's a
special form recognized by the parser in intrinsic argument position). It
reads from the target's address map config file:

```toml
# config/address-map.toml
[addresses]
"CHIP_UART_PORT" = "0xFFE01000"
"CHIP_GPIO_BASE" = "0xFFE02000"
```

The config file is loaded at compile time and the string value is inlined.

### `.#` — Layout Port Access

Per the Layout DSL (`docs/architecture/layout-dsl.md`), `#` is the prefix
for all layout-level access:

```
packet.#magic       → layout field read at known bit position
packet.#crc = 42    → layout field write, masked to field width
list.#length        → structural field read
list.#get(i)        → layout operation call
```

In trigger context:

```
trg sig: Bool @ AddressOf#("SIGINT").#raised;
                     │                │
                     │                └── .#raised: access "raised" bit in Signal's layout
                     └── AddressOf# returns the typed handle
```

**Types are `Bits(N)` with layout patterns, not structs.** The `.#` prefix
distinguishes layout navigation from struct field access. The parser must
reject plain `.port` (without `#`) in trigger bindings as a type error —
the layout port must always be accessed with `.#`.

### Raw Hex Address Shorthand

```
trg rx @ 0xFFE01000.#data_ready;
```

This desugars to `AddressOf#<MMIO>(0xFFE01000).#data_ready`. The parser
recognizes a raw integer literal after `@` and wraps it implicitly. This
is a convenience for the common case of memory-mapped I/O.

### What Gets Scrapped

The following variants in `ast/top.rs` `LinkRef` are removed:

- `LinkRef::Stdin` — replaced by `AddressOf#("stdin")`
- `LinkRef::Timer(u64)` — replaced by `AddressOf#<Timer>("timer_100ms")`
- `LinkRef::Signal(String)` — replaced by `AddressOf#("SIGINT")`

The `TriggerDeclaration` struct (backend-compat layer) loses the `address`
field's reliance on these variants. The new `Trigger { instance: Expr }`
already uses expressions — it Just Works once `AddressOf#` is parsed as
a regular `Expr::Call`.

### Interpreter Addition

Add to `execute_intrinsic()` in `src/interpreter/intrinsics.rs`:

- `AddressOf#("stdin")` → returns `Value::Int(0)` (or a magic handle)
- `AddressOf#<MMIO>(0xFFE01000)` → returns `Value::Int(0xFFE01000)`

### Backend Emission (LLVM)

In `src/backend/llvm/emit_expr.rs`, when encountering
`Expr::Call("AddressOf#", [arg])`:

```rust
// Match on the resolved type's "listen" property
match type_property(return_type, "listen") {
    "select"  => emit_select_listener(builder, arg, port),
    "poll"    => emit_polling_load(builder, arg),
    "sigaction" => emit_sigaction(builder, arg, port),
    _ => return Err(format!("unsupported listen strategy for {}", return_type_name)),
}
```

### Dynamic Trigger Targets: `@ *ptr`

#### Motivation

Static triggers (`trg x @ fixed_instance.#port`) are resolved at compile
time — the target entity is known and checked before the binary is linked.
But many systems need to bind handlers to entities that aren't known until
runtime: a USB device on a hot-swappable bus, a virtual device registered
by another component, or a memory-mapped peripheral whose address is read
from a device tree.

Briev's contract system must extend to these cases without sacrificing
safety. The solution is a **two-phase safety model**: the compile-time
type parameter on `AddressOf#` guarantees shape, a runtime init guard
guarantees the entity exists and matches.

#### The Pattern

`AddressOf#<T>(id)` returns `Ptr<T>` — a typed pointer carrying the
entity's declared shape `T`. Applying `.#field` scopes the pointer type
to a specific port:

```briev
let uart_rx: Ptr<UartRxPort> = AddressOf#<UartRxPort>("sys:uart/rx").#rx;
trg x @ *uart_rx;
```

`*uart_rx` dereferences the pointer. The backend resolves the target at
**init time** instead of compile time. Since `UartRxPort` was type-checked
when `AddressOf#` resolved, the trigger binding is statically type-safe.

When the pointer type already describes the full port (no field projection
needed):

```briev
trg x @ *AddressOf#<UartRxPort>("sys:uart/rx");
```

Here `UartRxPort` carries the complete port shape. No `.#field` is needed
because the type already describes exactly what trigger to set up.

#### Safety Model

| Check | When | What happens on failure |
|-------|------|------------------------|
| `T` matches expected port shape | Compile (type resolution) | Type error — rejected |
| Target entity exists at runtime address | Init time | Warning with `--warn-unresolved-trg` |
| Entity shape matches `T` | Init time | Error with `--error-unresolved-trg` (warning by default) |

The compile-time contract (`T`) guarantees shape correctness. The runtime
check guarantees the entity exists and matches. Two-phase safety mirrors
Briev's overall contract philosophy: contracts are verified, never assumed.

#### Example: Hot-swappable Input Device

```briev
type GamepadInput : InputPort {
    // fields: button_a, button_b, dpad_x, dpad_y
};

txn handle_input [has_device][has_device] {
    let device: Ptr<GamepadInput> = AddressOf#<GamepadInput>("usb:gamepad");
    [*device != null] {
        trg x @ *device;
        term;
    };
    [*device == null] {
        term; // no device — skip
    };
};
```

#### Usage in Inline `$` calls (non-trg, just address)

`AddressOf#` is usable anywhere, not only in trigger bindings:

```briev
let counter_addr: Ptr<Int> = AddressOf#<Int>("sys:sysclock_ticks");
let ticks: Int = *counter_addr;  // read via deref
```

The difference from trigger bindings is the `listen <~` metadata: when
used in `trg @ *addr.#port`, the backend sets up a listener. When used
inline (no `trg`), it's just an address dereference.

#### Implementation Steps (Phase 5 additions)

| Step | Task | Files | Tests needed |
|------|------|-------|-------------|
| 5j | Add `Expr::Deref(Box<Expr>)` variant to AST | `src/ast/expr.rs` | Parser parses `*expr` |
| 5k | Parse `*expr` in trigger instance position | `src/parser/definitions.rs` | `trg x @ *ptr` parses |
| 5l | Type-checker: verify deref target is `Ptr<T>`, extract `T` | `src/typechecker/` | Type error on non-Ptr deref |
| 5m | LLVM codegen: emit init-time table lookup + listener registration | `src/backend/llvm/emit_expr.rs` | `@ *ptr` emits different IR than `@ fixed.#port` |
| 5n | CIRCT codegen: init-time entity resolution | `src/backend/circt.rs` | Correct MLIR for dynamic trigger |
| 5o | Webstack codegen: JS init-time binding | `src/backend/webstack.rs` | Dynamic `addEventListener` etc |
| 5p | Post-stage plugin: inject runtime validation guard | `plugins/post/validate-trg.bv` | Warning emitted for missing entities |
| 5q | CLI flags: `--warn-unresolved-trg`, `--error-unresolved-trg` | `src/main.rs` | Flags control runtime behavior |

---

## What Gets Scrapped

### Files to Delete

| File/Directory | Lines | Replacement |
|----------------|-------|-------------|
| `src/features/macros/` | ~1200 | `$(Stage)` blocks + `$` intrinsics |
| — `context.rs` | | Stage block evaluation |
| — `expand.rs` | | `MatchIR$`, `Weave$` |
| — `hygiene.rs` | | N/A — hygiene is automatic in stage blocks |
| — `macro_.rs` | | Covered by `Collect$`, `Annotate$` |
| — `template.rs` | | Covered by `Weave$`, `MatchIR$` |
| — `mod.rs` | | — |

### AST Variants to Remove

- `TopLevel::MacroDef { name, params, return_type, body }`
- `TopLevel::TemplateDef { name, params, return_type, body }`

### Tokens to Remove

- `Token::Macro`
- `Token::Template`

### LinkRef Variants to Remove

- `LinkRef::Stdin`
- `LinkRef::Timer(u64)`
- `LinkRef::Signal(String)`

If `LinkRef` is only used in the backward-compat `TriggerDeclaration`, it
can be removed entirely and `TriggerDeclaration` can use `Expr` directly.

### Implicit Behavior to Remove

| Location | Lines | Behavior | Replacement |
|----------|-------|----------|-------------|
| `import_resolver.rs` | 146-215 | Prelude auto-injection | `plugins/front/prelude.bv` |
| `import_resolver.rs` | 85-129 | Stdlib root search | `ReadConfig$` + registry |
| `compile.rs` | 41-64 | BEAST external plugin chain | `$(Post)` + `IncludeBEAST$` |
| `compile.rs` | 91-97 | Empty `AfterCodegen` hook | `$(Back)` blocks |
| `llvm/normalizer.rs` | 15-55 | Auto-annotation pass | `SetTypeProperty$` in `$(Mid)` |

### `--no-stdlib` Flag to Deprecate

Replaced by `--disable-plugin prelude`. The old flag can remain for
backward compatibility but should delegate to `--disable-plugin`.

---

## Lexer Changes

### 1. `$` as Valid Identifier Character

Currently `$` is a standalone `Token::Dollar`. It must also be valid inside
identifiers, just like `#`:

```rust
// Current (lexer.rs):
#[token("$")]
Dollar,

// After:
// $ is valid inside identifiers, AND also a standalone token when
// followed by something that can't be part of an identifier.
// logos handles this via priority — the longer match wins.
// Add $ to the identifier character set.
```

Implementation in `logos`: Add `$` to the regex character class for
identifiers. The `logos` derive macro uses `#[regex(r"[a-zA-Z_$][a-zA-Z0-9_$#]*")]`
for identifiers. When `$` is followed by `(`, the `Dollar` token still
matches first because it's an exact match — `$(Front)` lexes as
`Dollar LParen Identifier("Front") RParen`.

If logos cannot handle this coexistence, fall back to a manual lookahead:
when lexer sees `$` followed by `(`, emit `Dollar`; when `$` is followed
by an identifier character, append `$` to the current identifier token.

### 2. Remove Macro/Template Tokens

```rust
// Remove:
#[token("macro")]
Macro,

#[token("template")]
Template,
```

---

## Parser Changes

### 1. `$(Stage)` Block Parsing

Add a new method `parse_stage_block()` in `src/parser/definitions.rs`:

```rust
/// Parse: $(Stage @ priority) { body }
fn parse_stage_block(&mut self) -> Result<StageBlock, SyntaxError> {
    // Expect: $ ( identifier @ priority? ) { statements }
}
```

Called from `parse_top_level()` when `peek == Dollar && peek_next == LParen`.

### 2. Import `<>` Syntax

Modify `parse_import()` to check for `Lt` token after `import`/`from`:

```rust
fn parse_import(&mut self) -> Result<Import, SyntaxError> {
    self.pos += 1;
    if self.eat(&Token::LParen) {
        // import { a, b } from ...
        // ...
        if self.eat(&Token::Lt) {
            // from <name>
            let name = self.expect_identifier()?;
            self.expect(Token::Gt)?;
            return Ok(Import { kind: ImportKind::Registry(name), ... });
        } else {
            // from "path"
            let module = self.expect_string()?;
            return Ok(Import { kind: ImportKind::Literal(module), ... });
        }
    }
    if self.eat(&Token::Lt) {
        // import <name>
        let name = self.expect_identifier()?;
        self.expect(Token::Gt)?;
        self.expect(Token::Semicolon)?;
        return Ok(Import { kind: ImportKind::Registry(name), ... });
    }
    // existing string path handling...
}
```

### 3. Trigger `.#` Enforcement

In `parse_top_level_trg()`, after parsing the instance expression and `Dot`,
if the next token is not `Hash` (`#`), emit a type error:

```rust
fn parse_top_level_trg(&mut self) -> Result<Trigger, SyntaxError> {
    // trg name @ instance.#port;
    self.pos += 1;
    let name = self.expect_identifier()?;
    self.expect(Token::At)?;
    let instance = self.parse_expression()?;
    self.expect(Token::Dot)?;
    // Require # prefix for layout port access
    self.expect(Token::Hash)?;  // NEW
    let port = self.expect_identifier()?;
    self.expect(Token::Semicolon)?;
    // ...
}
```

### 4. Remove Macro/Template Parsing

Remove `parse_macro()` and `parse_template()` methods (if they exist), and
remove the corresponding match arms in `parse_top_level()`.

---

## AST Changes

### 1. New `TopLevel::StageBlock`

```rust
/// A compile-time $(Stage) block.
/// The body is executed at compile time during the specified stage.
pub struct StageBlock {
    pub stage: StageKind,
    pub priority: u32,
    pub body: Vec<Statement>,
    pub span: Option<Span>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageKind {
    Front,
    Mid,
    Post,
    Back,
}
```

Add `TopLevel::StageBlock(StageBlock)` variant.

### 2. Modified `Import`

```rust
pub enum ImportKind {
    /// import "path" — file/project-anchored
    Literal(String),
    /// import <name> — compiler registry lookup
    Registry(String),
}

pub struct Import {
    pub kind: ImportKind,
    pub symbols: Vec<String>,
    pub span: Option<Span>,
}
```

### 3. Removed Variants

- Remove `TopLevel::MacroDef(...)`
- Remove `TopLevel::TemplateDef(...)`
- Remove `LinkRef::Stdin`, `LinkRef::Timer(u64)`, `LinkRef::Signal(String)`

### 4. New `AddressOf#` in Type Universe

Add `"AddressOf#"` to the intrinsic registry in
`get_intrinsic_signature()`:

```rust
"AddressOf#" => Some(IntrinsicSignature {
    params: vec![Type::string()],
    returns: Box::new(Type::ptr(Type::unit())),  // raw address
    observable: true,
})
```

And for the typed variant, when `<T>` is provided, the return type is `T`
instead of `*()`.

---

## Plugin Loading & Wiring

### 1. Plugin Trait Updates

The `Plugin` trait in `src/plugin/mod.rs` gains stage-specific methods:

```rust
pub enum PluginAction {
    Continue,
    Abort(String),
}

pub trait Plugin: std::fmt::Debug {
    fn name(&self) -> &str;

    /// Stage: $(Front) — operates on source text
    fn on_source(&self, source: &mut String) -> PluginAction {
        PluginAction::Continue
    }

    /// Stage: $(Mid) / $(Post) — operates on AST
    fn on_ast(&self, program: &mut Vec<TopLevel>, universe: &mut TypeUniverse) -> PluginAction {
        PluginAction::Continue
    }

    /// Stage: $(Back) — operates on backend IR
    fn on_ir(&self, ir: &mut String) -> PluginAction {
        PluginAction::Continue
    }
}
```

The old `on_hook()` and `on_ir_ready()` methods are removed.

### 2. `PluginManager` Wiring

```rust
pub struct PluginManager {
    front_plugins: Vec<Box<dyn Plugin>>,
    mid_plugins: Vec<Box<dyn Plugin>>,
    post_plugins: Vec<Box<dyn Plugin>>,
    back_plugins: Vec<Box<dyn Plugin>>,
}

impl PluginManager {
    pub fn new() -> Self;

    pub fn register_front(&mut self, plugin: Box<dyn Plugin>);
    pub fn register_mid(&mut self, plugin: Box<dyn Plugin>);
    pub fn register_post(&mut self, plugin: Box<dyn Plugin>);
    pub fn register_back(&mut self, plugin: Box<dyn Plugin>);

    pub fn run_front(&self, source: &mut String) -> PluginAction;
    pub fn run_mid(&self, program: &mut Vec<TopLevel>, universe: &mut TypeUniverse) -> PluginAction;
    pub fn run_post(&self, program: &mut Vec<TopLevel>, universe: &mut TypeUniverse) -> PluginAction;
    pub fn run_back(&self, ir: &mut String) -> PluginAction;
}
```

### 3. System Plugin Loading

In `compile.rs`, before the pipeline starts:

```rust
fn load_system_plugins(opts: &BuildOptions) -> PluginManager {
    let mut pm = PluginManager::new();
    let plugin_dirs = resolve_plugin_dirs(opts);

    // Load plugins/front/*.bv
    if let Some(dir) = &plugin_dirs.front {
        for entry in std::fs::read_dir(dir).ok()? {
            let plugin = compile_plugin_from_file(&entry.path());
            pm.register_front(plugin);
        }
    }
    // Same for mid, post, back...
    pm
}
```

### 4. User Inline Block Compilation

When `$(Stage) { ... }` is parsed, the body is compiled into a `Plugin`:

```rust
fn compile_stage_block(block: &StageBlock) -> Box<dyn Plugin> {
    // The body is a Vec<Statement>. The plugin evaluates it by
    // walking the statements and dispatching known $ intrinsics.
    Box::new(StageBlockPlugin {
        stage: block.stage,
        priority: block.priority,
        body: block.body.clone(),
    })
}
```

The `StageBlockPlugin` implements `Plugin` and, when invoked, evaluates
each statement, resolving `$`-suffixed calls to compiler intrinsics.

### 5. Pipeline Integration in `compile.rs`

```rust
pub fn compile_source(file_path, source, opts) -> Result<(), String> {
    let mut source = source.to_string();
    let mut pm = load_system_plugins(opts);

    // ── $(Front) Stage ──────────────────────────────────
    pm.run_front(&mut source)?;

    // ── Lex, Parse, Resolve ─────────────────────────────
    let tokens = lex(&source)?;
    let items = parse(file_path, &tokens, &source)?;

    // During parsing, collect $(Stage) blocks from source
    let user_plugins = collect_stage_blocks(&items);
    for plugin in user_plugins {
        pm.register_for_stage(plugin);
    }

    let mut resolver = ImportResolver::new()
        .with_use_stdlib(false);  // No magic — prelude is a plugin
    let mut items = resolver.resolve_imports(items, &PathBuf::from(file_path))?;

    let mut universe = TypeUniverse::new();
    check_types(&items, &universe)?;

    // ── $(Mid) Stage ────────────────────────────────────
    pm.run_mid(&mut items, &mut universe)?;

    // ── DAG stitch / contract→metadata / sugar strip ────
    perform_structural_analysis(&mut items, &mut universe)?;

    // ── $(Post) Stage ───────────────────────────────────
    pm.run_post(&mut items, &mut universe)?;

    // ── Normalizer ──────────────────────────────────────
    normalizer::normalize(&mut items, &mut universe)?;

    // ── Codegen ─────────────────────────────────────────
    let mut output = backend.generate(&items);

    // ── $(Back) Stage ───────────────────────────────────
    pm.run_back(&mut output)?;

    // ── Write output ────────────────────────────────────
    std::fs::write(&out_path, &output)?;
    Ok(())
}
```

---

## Implementation Order

### Phase 1: Foundation (Lexer + Parser + AST)

| Step | Task | Files | Tests needed |
|------|------|-------|-------------|
| 1a | Add `$` to identifier character set in lexer | `src/lexer.rs` | Identifiers with `$` parse correctly |
| 1b | Add `StageBlock` AST type and `StageKind` enum | `src/ast/top.rs` | AST construction, serialization |
| 1c | Add `$(Stage)` parsing to parser | `src/parser/definitions.rs` | `$(Front) {}` parses, invalid stage name rejected |
| 1d | Add `.#` enforcement in trigger parsing | `src/parser/definitions.rs` | `trg x @ y.#port` OK, `trg x @ y.port` rejected |
| 1e | Modify `Import` to `ImportKind` enum | `src/ast/top.rs`, `src/parser/definitions.rs` | Both `""` and `<>` import forms |
| 1f | Add `<>` import parsing | `src/parser/definitions.rs` | `import <std/io>` parses correctly |
| 1g | Remove `Macro`/`Template` tokens and AST variants | `src/lexer.rs`, `src/ast/top.rs` | Files that used macro/template now error with clear message |
| 1h | Remove `features/macros/` directory | Delete 6 files | Everything still compiles |

### Phase 2: Plugin System

| Step | Task | Files | Tests needed |
|------|------|-------|-------------|
| 2a | Rewrite `Plugin` trait with stage-specific methods | `src/plugin/mod.rs` | Each method dispatches correctly |
| 2b | Rewrite `PluginManager` with per-stage registration | `src/plugin/mod.rs` | Plugins run in correct stage |
| 2c | Implement system plugin discovery | `src/plugin/loader.rs` | Plugins loaded from correct directories |
| 2d | Implement `StageBlockPlugin` (inline `$(Stage)` compilation) | `src/plugin/mod.rs` or new file | Body statements execute as intrinsics |
| 2e | Wire plugin stages into `compile.rs` pipeline | `src/compile.rs` | Full pipeline runs each stage |
| 2f | Implement `--disable-plugin` / `--enable-plugin` CLI | `src/main.rs` | Flags correctly enable/disable plugins |

### Phase 3: `$` Intrinsics

| Step | Task | Files | Tests needed |
|------|------|-------|-------------|
| 3a | Implement `$(Front)` intrinsics (`InsertFileImport$`, `InsertRegistryImport$`, etc.) | New `src/intrinsics/front.rs` | Each intrinsic produces correct output |
| 3b | Implement `$(Mid)` intrinsics (`InsertDecl$`, `Collect$`, `Weave$`, etc.) | New `src/intrinsics/mid.rs` | Each intrinsic transforms AST correctly |
| 3c | Implement `$(Post)` intrinsics (`MatchIR$`, `AssertStructure$`, etc.) | New `src/intrinsics/post.rs` | Each intrinsic operates on stripped AST correctly |
| 3d | Implement `$(Back)` intrinsics (`PatchIR$`, `TargetTriple$`, etc.) | New `src/intrinsics/back.rs` | Each intrinsic modifies IR text correctly |
| 3e | Implement cross-stage intrinsics (`ReadConfig$`, `GetEnv$`, etc.) | New `src/intrinsics/common.rs` | Config/env reads work at compile time |
| 3f | Implement `IncludeBEAST$` | `src/intrinsics/post.rs` | External `.beast` files load and apply |

### Phase 4: Prelude Migration

| Step | Task | Files | Tests needed |
|------|------|-------|-------------|
| 4a | Create `plugins/front/prelude.bv` | New file | Prelude imports all std modules |
| 4b | Remove prelude auto-injection from `import_resolver.rs:146-215` | `src/import_resolver.rs` | No regression on imports |
| 4c | Add `prelude.bv` to system plugin directory | `plugins/front/` | Prelude loads by default |
| 4d | Test `--disable-plugin prelude` | CLI | No prelude imports when disabled |

### Phase 5: `AddressOf#` Implementation

| Step | Task | Files | Tests needed |
|------|------|-------|-------------|
| 5a | Add `AddressOf#` to intrinsic signature registry | `src/interpreter/intrinsics.rs` | Signature returned correctly |
| 5b | Add `AddressOf#` to interpreter `execute_intrinsic()` | `src/interpreter/intrinsics.rs` | Returns correct value for known IDs |
| 5c | Create `config/address-map.toml` | New file | Config loads correctly |
| 5d | Implement `from_config()` compile-time function | `src/parser/special_forms.rs` or similar | `from_config("key")` reads config |
| 5e | Add LLVM backend emission for `AddressOf#` | `src/backend/llvm/emit_expr.rs` | Correct LLVM IR emitted for each strategy |
| 5f | Add CIRCT backend emission for `AddressOf#` | `src/backend/circt.rs` | Correct MLIR emitted |
| 5g | Add Webstack backend emission for `AddressOf#` | `src/backend/webstack.rs` | Correct JS emitted |
| 5h | Add listening strategy validation per backend | Each backend's normalizer | Invalid strategy = compile error |
| 5i | Remove `LinkRef::Stdin`/`Timer`/`Signal` | `src/ast/top.rs` | `TriggerDeclaration` uses `Expr` entirely |
| 5j | Add `Expr::Deref(Box<Expr>)` variant to AST | `src/ast/expr.rs` | Parser parses `*expr` |
| 5k | Parse `*expr` in trigger instance position | `src/parser/definitions.rs` | `trg x @ *ptr` parses |
| 5l | Type-checker: verify deref target is `Ptr<T>`, extract `T` | `src/typechecker/` | Type error on non-Ptr deref |
| 5m | LLVM codegen: emit init-time table lookup + listener registration | `src/backend/llvm/emit_expr.rs` | Dynamic trigger IR differs from static |
| 5n | CIRCT codegen: init-time entity resolution | `src/backend/circt.rs` | Correct MLIR for dynamic trigger |
| 5o | Webstack codegen: JS init-time binding | `src/backend/webstack.rs` | Dynamic `addEventListener` etc |
| 5p | Post-stage plugin: inject runtime validation guard | `plugins/post/validate-trg.bv` | Warning emitted for missing entities |
| 5q | CLI flags: `--warn-unresolved-trg`, `--error-unresolved-trg` | `src/main.rs` | Flags control runtime behavior |

### Phase 6: `.beast` Pattern Compiler

| Step | Task | Files | Tests needed |
|------|------|-------|-------------|
| 6a | Extend S-expression parser for pattern variables (`?x`) | `src/beast/sexpr.rs` | `?x` matches any sub-AST |
| 6b | Build pattern-match compiler (Sexpr → match tree) | New `src/beast/pattern.rs` | Pattern matches correctly |
| 6c | Build replacement compiler (Sexpr → AST builder) | New `src/beast/replace.rs` | Replacement constructs correct AST |
| 6d | Wire `MatchIR$` to use pattern compiler | `src/intrinsics/post.rs` | `MatchIR$("pattern", "replacement")` works end-to-end |

### Phase 7: Migration & Cleanup

| Step | Task | Files | Tests needed |
|------|------|-------|-------------|
| 7a | Update all example files that used old syntax | `examples/*.bv` | Examples compile and run |
| 7b | Create new example files for `$(Stage)` syntax | `examples/stage/*.bv` | Examples compile and run |
| 7c | Update architecture docs | `docs/architecture/*.md` | Docs reflect new architecture |
| 7d | Remove `--no-stdlib`, replace with `--disable-plugin prelude` | `src/main.rs` | CLI works correctly |
| 7e | Remove `--emit-beast` / `--plugin` flags if superseded | `src/main.rs` | Flags removed or deprecated |

---

## File Change Inventory

### New Files

| File | Purpose |
|------|---------|
| `plugins/front/prelude.bv` | System prelude plugin |
| `plugins/mid/README.md` | Documentation for mid-stage plugins |
| `plugins/post/README.md` | Documentation for post-stage plugins |
| `plugins/back/README.md` | Documentation for back-stage plugins |
| `config/module-registry.toml` | Import `<>` lookup table |
| `config/address-map.toml` | Address mapping for hardware targets |
| `src/intrinsics/mod.rs` | Module root for compile-time intrinsics |
| `src/intrinsics/common.rs` | Cross-stage intrinsics (`ReadConfig$`, `GetEnv$`, etc.) |
| `src/intrinsics/front.rs` | `$(Front)` intrinsics |
| `src/intrinsics/mid.rs` | `$(Mid)` intrinsics |
| `src/intrinsics/post.rs` | `$(Post)` intrinsics |
| `src/intrinsics/back.rs` | `$(Back)` intrinsics |
| `src/beast/pattern.rs` | Pattern-match compiler for `MatchIR$` |
| `src/beast/replace.rs` | Replacement AST builder for `MatchIR$` |
| `docs/examples/stage/front-example.bv` | `$(Front)` usage example |
| `docs/examples/stage/mid-example.bv` | `$(Mid)` usage example |
| `docs/examples/stage/post-example.bv` | `$(Post)` usage example |
| `docs/examples/stage/back-example.bv` | `$(Back)` usage example |
| `plugins/post/validate-trg.bv` | Post-stage guard: warns on unresolved dynamic triggers |
| `src/plugin/intrinsics.rs` | Stage-agnostic `$` intrinsic dispatch (`InsertRegistryImport$`, `EmitWarning$`, `EmitError$`, etc.) |

### Modified Files

| File | What changes |
|------|-------------|
| `src/lexer.rs` | `$` in identifiers, remove `Macro`/`Template` tokens |
| `src/ast/top.rs` | Add `StageBlock`, modify `Import`, remove `MacroDef`/`TemplateDef`/`LinkRef` variants |
| `src/parser/definitions.rs` | Add `parse_stage_block()`, `<>` import, `.#` enforcement, remove macro/template |
| `src/import_resolver.rs` | Remove prelude injection (lines 146-215), add `ImportKind` dispatch |
| `src/compile.rs` | Full pipeline: plugin stages Front/Mid/Post/Back |
| `src/plugin/mod.rs` | New `Plugin` trait with stage methods, new `PluginManager` |
| `src/plugin/loader.rs` | System plugin discovery, `StageBlockPlugin` |
| `src/plugin/runner.rs` | Remove or refactor BEAST subprocess chain |
| `src/interpreter/intrinsics.rs` | Add `AddressOf#` |
| `src/backend/llvm/mod.rs` | Remove `emit_header` target triple/data layout (now in `$(Back)`) |
| `src/backend/llvm/normalizer.rs` | Remove auto-annotation (now in `$(Mid)` via `SetTypeProperty$`) |
| `src/backend/llvm/emit_expr.rs` | Add `AddressOf#` emission, strategy dispatch |
| `src/backend/llvm/emit_toplevel.rs` | Remove header emission, add `$(Back)` hook point |
| `src/backend/circt.rs` | Add `AddressOf#` emission, `$(Back)` hook |
| `src/backend/webstack.rs` | Add `AddressOf#` emission, `$(Back)` hook |
| `src/ast/expr.rs` | Add `Expr::Deref(Box<Expr>)` for `*ptr` dereference expressions |
| `src/main.rs` | Replace `--no-stdlib` with `--disable-plugin`, add `register` subcommand, add `--warn-unresolved-trg` / `--error-unresolved-trg` |
| `src/backend/llvm/context.rs` | Remove `target_triple`/`data_layout` fields (now config-driven) |
| `AGENTS.md` | Add "Plan Directives" section (already done in this commit) |

### Deleted Files

| File | Reason |
|------|--------|
| `src/features/macros/mod.rs` | Replaced by `$(Stage)` blocks |
| `src/features/macros/context.rs` | Replaced by `$(Stage)` block execution context |
| `src/features/macros/expand.rs` | Replaced by `Collect$`, `Weave$`, `MatchIR$` |
| `src/features/macros/hygiene.rs` | Not needed — stage blocks execute in their own scope |
| `src/features/macros/macro_.rs` | Replaced by `$` intrinsics |
| `src/features/macros/template.rs` | Replaced by `Weave$`, `MatchIR$` |

---

## Testing Strategy

Per Plan Directive #5: **Behavioral tests, not literal tests.**

### Categories of Tests

#### 1. Lexer Tests (`src/lexer.rs` — `#[cfg(test)]`)

```
test_identifier_with_dollar:
  input:  "InsertRegistryImport$"
  tokens: [Identifier("InsertRegistryImport$")]

test_stage_block_token_sequence:
  input:  "$(Front @ 100) {}"
  tokens: [Dollar, LParen, Identifier("Front"), At, Int(100), RParen, LBrace, RBrace]

test_import_angle_bracket:
  input:  "import <std/io>;"
  tokens: [Import, Lt, Identifier("std/io"), Gt, Semicolon]
```

#### 2. Parser Tests (`src/parser/definitions.rs` — `#[cfg(test)]`)

```
test_parse_stage_block_default_priority:
  input:  "$(Front) { InsertFileImport$(\"foo\"); }"
  output: StageBlock { stage: Front, priority: 500, body: [Call("InsertFileImport$", ["foo"])] }

test_parse_stage_block_explicit_priority:
  input:  "$(Mid @ 1000) { }"
  output: StageBlock { stage: Mid, priority: 1000, body: [] }

test_parse_import_angle:
  input:  "import <std/io>;"
  output: Import { kind: Registry("std/io"), symbols: [] }

test_parse_trg_requires_hash:
  input:  "trg sig @ AddressOf#(\"SIGINT\").raised;"
  result: Error — # required before port name

test_parse_trg_with_hash:
  input:  "trg sig @ AddressOf#(\"SIGINT\").#raised;"
  output: Trigger { name: "sig", instance: Call("AddressOf#", ["SIGINT"]), port: "#raised" }
```

#### 3. Plugin System Tests (`src/plugin/mod.rs` — `#[cfg(test)]`)

```
test_plugin_manager_front_stage:
  // Register a $(Front) plugin that prepends text
  // Verify the source string is modified

test_plugin_manager_mid_stage:
  // Register a $(Mid) plugin that renames a symbol
  // Verify the AST has the new name

test_plugin_manager_post_stage:
  // Register a $(Post) plugin that asserts structure
  // Verify it catches invalid ASTs

test_plugin_manager_back_stage:
  // Register a $(Back) plugin that patches IR
  // Verify the IR string is modified

test_plugin_priority_order:
  // Register two plugins with different priorities
  // Verify they execute in descending priority order

test_system_plugin_discovery:
  // Create a temporary plugins/front/ directory
  // Verify plugins are loaded from it

test_disable_plugin:
  // Register a plugin, then disable it by name
  // Verify it does not execute

test_user_inline_stage_block:
  // Parse a $(Front) block from source text
  // Verify the body executes as a plugin at the Front stage
```

#### 4. Import System Tests (`src/import_resolver.rs` — `#[cfg(test)]`)

```
test_import_literal_path:
  // import "relative/path" → resolves relative to file

test_import_registry_lookup:
  // import <std/io> → looks up in config/module-registry.toml
  // Returns error if not found in registry

test_register_cli_command:
  // briev-compiler register name path → adds entry to registry
  // Subsequent import <name> resolves to that path

test_unregister_cli_command:
  // briev-compiler unregister name → removes entry
  // Subsequent import <name> returns error

test_no_magic_prelude:
  // Without plugins/front/prelude.bv loaded, no implicit imports
```

#### 5. `AddressOf#` Tests

```
test_address_of_no_type:
  input:  "AddressOf#(\"stdin\")"
  output: Intrinsic signature: returns *()

test_address_of_with_type:
  input:  "AddressOf#<Stdin>(\"stdin\")"
  output: Intrinsic signature: returns Stdin

test_address_of_with_strategy:
  input:  "AddressOf#<Stdin, \"poll\">(\"stdin\")"
  output: Stdin returned, backend validates "poll" strategy

test_raw_hex_address:
  input:  "trg rx @ 0xFFE01000.#data_ready;"
  output: Desugars to trg rx @ AddressOf#<MMIO>(0xFFE01000).#data_ready;

test_listening_strategy_validation:
  // LLVM backend: "select" → OK, "interrupt" → compile error
  // CIRCT backend: "interrupt" → OK, "select" → compile error
```

#### 6. Integration Tests (End-to-End)

```
test_compile_with_prelude_plugin:
  // Compile a minimal Briev file with system plugins enabled
  // Verify prelude imports are present in the resolved AST

test_compile_with_disable_plugin:
  // briev-compiler build --disable-plugin prelude minimal.bv
  // Verify no prelude imports in the resolved AST

test_compile_with_user_stage_block:
  // File contains $(Front) { InsertFileImport$("other"); }
  // Verify "other" is imported

test_compile_with_scripting_mode:
  // $(Front) block wraps body in scripting transaction
  // Verify generated code has the wrapper

test_compile_trigger_with_address_of:
  // trg input @ AddressOf#("stdin").#line_ready;
  // Verify LLVM IR contains select() call

test_compile_trigger_with_raw_address:
  // trg rx @ 0xFFE01000.#data_ready;
  // Verify LLVM IR contains polling load

test_backend_ir_patching:
  // $(Back) { PatchIR$("target triple", "target triple = \"custom\""); }
  // Verify emitted IR has custom target triple
```

### Testing Principles

1. **No literal snapshot tests.** Do not test that `$(Front) { ... }` produces
   exact byte-for-byte output. Test that the prelude imports are present.

2. **Behavior over implementation.** A test for `Collect$` should verify that
   annotated nodes are found, not that the internal traversal uses a recursive
   function.

3. **Regression guards.** Every intrinsic must have a test that verifies its
   behavior. If the intrinsic's implementation is refactored, the test must
   still pass as long as the behavior is preserved.

4. **Error messages matter.** Test that invalid inputs produce clear,
   actionable error messages. For example, an unsupported listening strategy
   should produce a message listing supported strategies.

---

## Document History

| Date | Change | Author |
|------|--------|--------|
| 2026-07-15 | Initial plan | Planning session |
