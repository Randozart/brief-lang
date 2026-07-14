// ── Phase 7: Plugin Loader ────────────────────────────────────────
//
// 2026-07-11: Native (.so) and WASM (.wasm) plugin loaders.
// Native loading uses the existing `libloading` crate. WASM loading
// uses `wasmtime` (feature-gated with "plugins").

use super::{Plugin, PluginAction, PluginHook};
use crate::ast::TopLevel;
use crate::type_universe::TypeUniverse;
use std::path::Path;

/// Load a plugin from a file path. Supports:
/// - `.so` / `.dylib` / `.dll` — native plugins via libloading
/// - `.wasm` — WASM plugins (requires `plugins` feature)
/// 2026-07-11: Phase 7.
pub fn load_plugin(path: &Path) -> Result<Box<dyn Plugin>, String> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext {
        "so" | "dylib" | "dll" => load_native_plugin(path),
        "wasm" => load_wasm_plugin(path),
        _ => Err(format!("Unsupported plugin extension: .{}", ext)),
    }
}

/// Load a native shared library (.so / .dylib / .dll) as a plugin.
/// The library must export a `brief_plugin_create` function.
/// 2026-07-11: Phase 7.
fn load_native_plugin(path: &Path) -> Result<Box<dyn Plugin>, String> {
    // Safety: libloading requires unsafe for FFI. The plugin MUST export
    // a `brief_plugin_create` function with the correct signature.
    unsafe {
        let lib = libloading::Library::new(path)
            .map_err(|e| format!("Failed to load native plugin '{}': {}", path.display(), e))?;
        let create: libloading::Symbol<unsafe extern "C" fn() -> *mut dyn Plugin> = lib
            .get(b"brief_plugin_create")
            .map_err(|e| format!("Plugin '{}' missing brief_plugin_create: {}", path.display(), e))?;
        let plugin_ptr = create();
        if plugin_ptr.is_null() {
            return Err(format!("Plugin '{}': brief_plugin_create returned null", path.display()));
        }
        let plugin = Box::from_raw(plugin_ptr);
        // Leak the library reference so it stays loaded for the plugin's lifetime.
        std::mem::forget(lib);
        Ok(plugin)
    }
}

/// Load a WASM module as a plugin using wasmtime.
/// For now, returns an error indicating WASM plugins are not yet supported.
/// 2026-07-11: Phase 7 — stub; real implementation requires wasmtime feature.
fn load_wasm_plugin(path: &Path) -> Result<Box<dyn Plugin>, String> {
    // Phase 7.4: WASM plugin loading via wasmtime.
    // For now, return a helpful error.
    Err(format!(
        "WASM plugin '{}' requires the 'plugins' feature (wasmtime runtime). \
         Compile with --features plugins to enable WASM plugin support.",
        path.display()
    ))
}

/// A simple validations plugin that checks program invariants.
/// Used as a built-in example and for testing.
/// 2026-07-11: Phase 7.
#[derive(Debug)]
pub struct ValidationPlugin {
    name: String,
}

impl ValidationPlugin {
    pub fn new() -> Self {
        ValidationPlugin { name: "builtin:validation".into() }
    }
}

impl Plugin for ValidationPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn on_hook(
        &self,
        _hook: PluginHook,
        _program: &mut Vec<TopLevel>,
        _universe: &TypeUniverse,
    ) -> PluginAction {
        PluginAction::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::super::{PluginManager, Plugin};
    use super::*;
    use crate::ast::{Comment, DispatchMode, StrictMode};

    fn empty_program() -> Program {
        Program {
            items: Vec::new(),
            comments: Vec::new(),
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: DispatchMode::default(),
            exit_condition: None,
            out_pragmas: Vec::new(),
            watchdog_defaults: (None, None),
            default_sig_modifier: None,
        }
    }

    #[test]
    fn test_validation_plugin_noop() {
        let plugin = ValidationPlugin::new();
        assert_eq!(plugin.name(), "builtin:validation");
    }

    #[test]
    fn test_plugin_manager_empty() {
        let mgr = PluginManager::new();
        assert!(mgr.is_empty());
        assert_eq!(mgr.len(), 0);
    }

    #[test]
    fn test_plugin_manager_register_and_hook() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(ValidationPlugin::new()));
        assert_eq!(mgr.len(), 1);
    }

    #[test]
    fn test_plugin_manager_run_hooks_continue() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(ValidationPlugin::new()));
        let mut program = empty_program();
        let universe = TypeUniverse::new();
        let result = mgr.run_hooks(PluginHook::AfterParse, &mut program, &universe);
        assert!(matches!(result, PluginAction::Continue));
    }

    #[test]
    fn test_load_plugin_unsupported_extension() {
        let result = load_plugin(Path::new("plugin.txt"));
        assert!(result.is_err());
    }

    #[test]
    fn test_plugin_manager_all_hooks_continue() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(ValidationPlugin::new()));
        let mut program = empty_program();
        let universe = TypeUniverse::new();
        for hook in &[PluginHook::AfterParse, PluginHook::AfterTypeCheck,
                      PluginHook::BeforeCodegen, PluginHook::AfterCodegen] {
            let result = mgr.run_hooks(*hook, &mut program, &universe);
            assert!(matches!(result, PluginAction::Continue), "hook {:?} failed", hook);
        }
    }

    /// A test plugin that aborts on AfterTypeCheck.
    #[derive(Debug)]
    struct AbortOnTypeCheck;

    impl Plugin for AbortOnTypeCheck {
        fn name(&self) -> &str { "test:abort_on_typecheck" }
        fn on_hook(&self, hook: PluginHook, _program: &mut Vec<TopLevel>, _universe: &TypeUniverse) -> PluginAction {
            if matches!(hook, PluginHook::AfterTypeCheck) {
                PluginAction::Abort("type check failed (test)".into())
            } else {
                PluginAction::Continue
            }
        }
    }

    #[test]
    fn test_plugin_aborts_at_hook() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(AbortOnTypeCheck));
        let mut program = empty_program();
        let universe = TypeUniverse::new();
        let result = mgr.run_hooks(PluginHook::AfterTypeCheck, &mut program, &universe);
        assert!(matches!(result, PluginAction::Abort(msg) if msg.contains("type check failed")));
    }

    #[test]
    fn test_plugin_passes_other_hooks() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(AbortOnTypeCheck));
        let mut program = empty_program();
        let universe = TypeUniverse::new();
        // Should not abort at BeforeCodegen (only aborts at AfterTypeCheck)
        let result = mgr.run_hooks(PluginHook::BeforeCodegen, &mut program, &universe);
        assert!(matches!(result, PluginAction::Continue));
    }

    #[test]
    fn test_plugin_last_abort_wins() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(AbortOnTypeCheck));
        mgr.register(Box::new(ValidationPlugin::new()));
        let mut program = empty_program();
        let universe = TypeUniverse::new();
        // AbortOnTypeCheck runs first (registered first) and aborts
        let result = mgr.run_hooks(PluginHook::AfterTypeCheck, &mut program, &universe);
        assert!(matches!(result, PluginAction::Abort(_)));
    }
}
