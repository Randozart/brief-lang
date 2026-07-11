# Compiler Plugins

Brief supports loadable compiler plugins — WASM or native dynamic libraries
that run at defined hooks in the compilation pipeline.

## Loading a Plugin

```bash
brief build file.bv --plugin ./my_plugin.so
```

Multiple plugins run in registration order:

```bash
brief build file.bv --plugin ./lint.wasm --plugin ./verify.wasm
```

## What a Plugin Can Do

A plugin implements the `Plugin` trait:

```rust
pub trait Plugin: Debug {
    fn name(&self) -> &str;
    fn on_hook(&self, hook: PluginHook, program: &mut Program,
               universe: &TypeUniverse) -> PluginAction;
}
```

At each hook point, the plugin receives the current `Program` and
`TypeUniverse`. It returns `PluginAction::Continue` to proceed or
`PluginAction::Abort(msg)` to stop compilation with a diagnostic.

## Hook Points

| Hook | When It Fires |
|------|---------------|
| `AfterParse` | After import resolution (not yet wired) |
| `AfterTypeCheck` | After type checking |
| `BeforeCodegen` | Before LLVM IR generation |
| `AfterCodegen` | After LLVM IR generation (not yet wired) |

## Writing a Plugin (Native `.so`)

A native plugin is a shared library exporting `brief_plugin_create`:

```rust
#[no_mangle]
pub extern "C" fn brief_plugin_create() -> *mut dyn Plugin {
    Box::into_raw(Box::new(MyPlugin))
}
```

See `src/plugin/loader.rs` for the `ValidationPlugin` example.

## Writing a Plugin (WASM)

WASM plugin loading requires the `plugins` Cargo feature. Plugins are
compiled to `wasm32-wasi` (use `--triple wasm32-unknown-wasi`). A WIT
interface defines the contract between the compiler and the plugin.

## Why WASM?

See `docs/architecture/features/plugins.md` for the full rationale:
sandboxing, language independence, stable ABI via WIT, and microsecond
instantiation.
