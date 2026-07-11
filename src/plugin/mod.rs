// ── Phase 7: Plugin System ─────────────────────────────────────────
//
// 2026-07-11: Compiler plugin system. Plugins are WASM (or native .so)
// modules loaded at compile time that can observe/transform the program
// at defined hook points in the compilation pipeline.
//
// Architecture:
//   Plugin trait — the interface every plugin implements.
//   PluginManager — loads, stores, and invokes plugins.
//   PluginHook — enum identifying pipeline hook points.
//
// Future: WASM plugin loading via wasmtime (feature-gated).

pub mod loader;

use crate::ast::Program;
use crate::type_universe::TypeUniverse;

/// Hook points in the compilation pipeline where plugins can run.
/// 2026-07-11: Phase 7.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginHook {
    /// After parsing and import resolution, before desugaring.
    AfterParse,
    /// After type checking completes.
    AfterTypeCheck,
    /// After analysis, before code generation.
    BeforeCodegen,
    /// After LLVM IR is generated.
    AfterCodegen,
}

/// Result of running a plugin at a hook point.
/// 2026-07-11: Phase 7.
#[derive(Debug)]
pub enum PluginAction {
    /// Continue compilation normally.
    Continue,
    /// Abort compilation with an error message.
    Abort(String),
}

/// A single compiler plugin. Each plugin can observe and optionally
/// transform the program at defined hook points.
/// 2026-07-11: Phase 7.
pub trait Plugin: std::fmt::Debug {
    /// Human-readable plugin name.
    fn name(&self) -> &str;

    /// Called at the given hook point. Returns an action.
    /// The default implementation does nothing.
    fn on_hook(
        &self,
        _hook: PluginHook,
        _program: &mut Program,
        _universe: &TypeUniverse,
    ) -> PluginAction {
        PluginAction::Continue
    }
}

/// Manages plugin lifecycle: loading, hook dispatch, and cleanup.
/// 2026-07-11: Phase 7.
#[derive(Debug)]
pub struct PluginManager {
    /// Loaded plugins in registration order.
    plugins: Vec<Box<dyn Plugin>>,
}

impl PluginManager {
    /// Create an empty plugin manager (no plugins loaded).
    pub fn new() -> Self {
        PluginManager { plugins: Vec::new() }
    }

    /// Register a plugin. The plugin is appended to the hook chain.
    pub fn register(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.push(plugin);
    }

    /// Run all plugins at the given hook point.
    /// Returns the first abort action, or Continue if all pass.
    pub fn run_hooks(
        &self,
        hook: PluginHook,
        program: &mut Program,
        universe: &TypeUniverse,
    ) -> PluginAction {
        for plugin in &self.plugins {
            let action = plugin.on_hook(hook, program, universe);
            match action {
                PluginAction::Continue => {}
                PluginAction::Abort(msg) => {
                    return PluginAction::Abort(format!("[plugin:{}] {}", plugin.name(), msg));
                }
            }
        }
        PluginAction::Continue
    }

    /// Number of loaded plugins.
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    /// True if no plugins are loaded.
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}
