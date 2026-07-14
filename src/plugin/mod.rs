// ── Phase 7: Plugin System ─────────────────────────────────────────
//
// 2026-07-11: Compiler plugin system. Plugins can observe and transform
// the program at defined hook points.
//
// Architecture:
//   Plugin trait — interface every plugin implements.
//   PluginManager — loads, stores, and invokes plugins.
//   PluginHook — enum identifying pipeline hook points.
//   PluginAction — Continue or Abort with error.
//
// External plugins are standalone executables that read/write BVIR
// format via stdin/stdout. See docs/plans/2026-07-14-bvir-plugin-midend.md

pub mod loader;
pub mod runner;

use crate::ast::TopLevel;
use crate::type_universe::TypeUniverse;

/// Hook points in the compilation pipeline where plugins can run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginHook {
    /// After parsing and import resolution, before desugaring.
    AfterParse,
    /// After type resolution — universe is fully populated. Primary plugin hook.
    AfterResolve,
    /// After analysis, before code generation.
    BeforeCodegen,
    /// After LLVM IR is generated.
    AfterCodegen,
}

/// Result of running a plugin at a hook point.
#[derive(Debug)]
pub enum PluginAction {
    /// Continue compilation normally.
    Continue,
    /// Abort compilation with an error message.
    Abort(String),
}

/// A single compiler plugin. Each plugin can observe and optionally
/// transform the program at defined hook points.
pub trait Plugin: std::fmt::Debug {
    /// Human-readable plugin name.
    fn name(&self) -> &str;

    /// Called at the given hook point. Gets mutable access to the AST
    /// and the type universe. Default implementation does nothing.
    fn on_hook(
        &self,
        _hook: PluginHook,
        _program: &mut Vec<TopLevel>,
        _universe: &mut TypeUniverse,
    ) -> PluginAction {
        PluginAction::Continue
    }

    /// Called when the backend has emitted the final IR string.
    /// The plugin can inspect or modify the IR before it is written to disk.
    fn on_ir_ready(
        &self,
        _ir: &mut String,
    ) -> PluginAction {
        PluginAction::Continue
    }
}

/// Manages plugin lifecycle: loading, hook dispatch, and cleanup.
#[derive(Debug)]
pub struct PluginManager {
    plugins: Vec<Box<dyn Plugin>>,
}

impl PluginManager {
    pub fn new() -> Self {
        PluginManager { plugins: Vec::new() }
    }

    pub fn register(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.push(plugin);
    }

    /// Run all plugins at the given hook point.
    /// Returns the first abort action, or Continue if all pass.
    pub fn run_hooks(
        &self,
        hook: PluginHook,
        program: &mut Vec<TopLevel>,
        universe: &mut TypeUniverse,
    ) -> PluginAction {
        for plugin in &self.plugins {
            let action = plugin.on_hook(hook, program, universe);
            if let PluginAction::Abort(msg) = action {
                return PluginAction::Abort(format!("[plugin:{}] {}", plugin.name(), msg));
            }
        }
        PluginAction::Continue
    }

    /// Run all plugins at the AfterCodegen hook with the final IR.
    pub fn run_ir_hooks(&self, ir: &mut String) -> PluginAction {
        for plugin in &self.plugins {
            let action = plugin.on_ir_ready(ir);
            if let PluginAction::Abort(msg) = action {
                return PluginAction::Abort(format!("[plugin:{}] {}", plugin.name(), msg));
            }
        }
        PluginAction::Continue
    }

    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}
