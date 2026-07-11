# Plugin System Architecture

**Status:** Phase 7 (2026-07-11)  
**File:** `src/plugin/`  
**CLI:** `brief build file.bv --plugin <path>`

## Overview

The Brief compiler supports loadable plugins — WASM or native dynamic libraries
that run at defined hook points in the compilation pipeline. Plugins can inspect
the program AST, validate invariants, and abort compilation with a diagnostic
error message.

---

## Why WebAssembly for Compiler Plugins

Using WASM as the plugin runtime is the industry standard adopted by modern
infrastructure tools (Envoy, Shopify, database engines) for four architectural
reasons:

### 1. Hard Sandboxing and Fault Isolation

A WASM plugin runs in a strictly isolated virtual machine with its own linear
memory space. It cannot read or write the compiler's internal memory — no
segfault from a buggy plugin can corrupt the AST, the symbol tables, or the
type universe. If a plugin traps (division by zero, out-of-bounds access),
the WASM host catches the error and the compiler produces a clean diagnostic:

```
Error: Plugin 'jira_validator' crashed during 'post_type_resolution'
       (reason: index out of bounds)
```

By default, WASM has zero OS access. It cannot open files, write to disk, or
open network sockets. If a plugin needs external access, the compiler must
grant it explicitly via WASI capabilities — capabilities-based security rather
than the "all or nothing" model of native `.so` plugins.

### 2. Language Agnosticism

A native plugin must be written in the same language as the compiler (Rust).
This forces every plugin author to learn Rust and match the compiler's exact
ABI — a high barrier to entry.

WASM is a universal compiler target. Plugins can be written in Rust, C, C++,
Zig, Go, or — most importantly — **Brief itself**. A developer writes a Brief
function, compiles it to `wasm32-wasi` (Phase 6), and loads it as a compiler
plugin. The plugin ecosystem becomes an extension of the language, not a
separate Rust project.

### 3. Stable ABI via the WIT Contract

Native plugins link against the compiler's internal data structures, which
change between every minor release. Updating the compiler means recompiling
every plugin.

WASM solves this through the Wasm Component Model and WIT (Wasm Interface
Types). The compiler and plugin communicate across a strict, versioned
interface defined in a `.wit` file:

```wit
record derivation-example {
    inputs: list<string>,
    output: string,
}
```

As long as the `.wit` contract is stable, a plugin compiled today will run
on the compiler five years from now — completely independent of internal
refactoring or Rust version upgrades.

### 4. Performance: Microsecond Instantiation

The alternative to in-process WASM sandboxing is OS subprocess isolation —
spawning a separate process and communicating via IPC. But process creation
takes tens of milliseconds (OS context switches, virtual memory allocation,
serialization).

WASM module instantiation takes microseconds. Modern runtimes (wasmtime with
Cranelift) compile WASM bytecode to native machine instructions on the fly;
the plugin executes at near-native speed. Compile-time checks do not slow
the developer's build loop.

### Summary: Four Properties for Brief

| Property | Native `.so` | WASM plugin |
|----------|-------------|-------------|
| Fault isolation | None (same address space) | Hard sandbox |
| Plugin language | Rust only | Any WASM target (including Brief) |
| ABI stability | None (linked to compiler internals) | Versioned WIT contract |
| Instantiation cost | ~1ns (dlopen) | ~1μs (wasmtime) |

---

## Plugin Lifecycle

```
Compiler starts
  ↓
CLI parses --plugin paths
  ↓
PluginManager created
  ├─ Native (.so): dlopen + brief_plugin_create
  └─ WASM (.wasm): wasmtime instantiation (future)
  ↓
Hooks fire at pipeline positions:
  ├─ AfterParse      (after import resolution)
  ├─ AfterTypeCheck  (after type checking)
  ├─ BeforeCodegen   (before LLVM IR generation)
  └─ AfterCodegen    (after LLVM IR generation)
  ↓
Any hook can return PluginAction::Abort(msg)
  → compilation stops with diagnostic
All hooks return PluginAction::Continue
  → compilation proceeds
```

---

## Plugin Trait

```rust
pub trait Plugin: std::fmt::Debug {
    fn name(&self) -> &str;
    fn on_hook(&self, hook: PluginHook, program: &mut Program,
               universe: &TypeUniverse) -> PluginAction;
}
```

**Hook points** (defined in `PluginHook`):

| Variant | Fires | Purpose |
|---------|-------|---------|
| `AfterParse` | Not yet wired | Validate raw AST |
| `AfterTypeCheck` | `run_llvm_compile()` line ~2600 | Type-level validation |
| `BeforeCodegen` | `run_llvm_compile()` line ~2615 | Last transformation |
| `AfterCodegen` | Not yet wired | Post-process LLVM IR |

---

## Native Plugin ABI

A shared library must export:

```c
void* brief_plugin_create(void);
```

Returns a `*mut dyn Plugin`. The compiler calls this function after
`libloading::Library::new()` and takes ownership of the returned pointer.

**File:** `src/plugin/loader.rs`

---

## WASM Plugin Loading (Future)

Requires the `plugins` Cargo feature. The WIT interface lives in `wit/`.
The plugin function `brief_plugin_process` receives a serialized `Program`
and returns a serialized `PluginAction`.

**Current status:** Stub. WASM plugin loading returns an error with a
helpful message directing users to enable the `plugins` feature.

---

## Files

| File | Purpose |
|------|---------|
| `src/plugin/mod.rs` | `Plugin` trait, `PluginManager`, `PluginHook`, `PluginAction` |
| `src/plugin/loader.rs` | Native `.so` loader, WASM stub, `ValidationPlugin` example |
| `src/main.rs` | `--plugin` CLI parsing, `PluginManager` lifecycle in `run_llvm_compile()` |

---

## Testing

```
cargo test --lib plugin   # runs 5 unit tests
```

Tests cover: empty manager, register plugin, run hooks (Continue),
load unsupported extension.

---

## Future Work (Phase 14+)

- WASM plugin loading via `wasmtime`
- WIT interface definition
- Plugin-driven synthesis (SMT solver as plugin)
- Plugin marketplace / registry
