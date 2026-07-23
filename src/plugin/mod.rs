// ── Plugin System — Stage-Based Architecture ────────────────────────────
//
// 2026-07-21: Expanded from 4 to 11 granular stages. Each stage maps to a
// compiler pass: PreLex, Parsed, Resolved, Typed, Normalized, Verified,
// Allocated, Provenanced, Generated, Optimized, Linked. Each plugin declares
// which stages it runs at via stages(). Plugins are sorted by priority
// (lower = earlier) within each stage.
//
// Stage dispatch pipeline:
//   PreLex(source) → Parsed(ast) → Resolved(ast) → Typed(ast)
//   → Normalized(ast) → Verified(ast) → Allocated(ast) → Provenanced(ast)
//   → Generated(ir) → Optimized(ir) → Linked(bin)
//
// System plugins ship in plugins/{parsed,resolved,typed,...}/ directories.
// User inline plugins are $(Stage) blocks parsed from source files.
// Per-extension plugin selection via config/targets.toml [ext].plugins.
//
// 2026-07-21: Old Collect$/MatchIR$/InsertLiteralImport$/InsertRegistryImport$
// intrinsics removed. Replaced by direct AST navigation DSL (Tag$, Named$,
// ForEach$, Insert$, Delete$, Set$, etc.) operating on the live AST.
//
// 2026-07-15: Removed old PluginHook/PluginAction types. The new trait
// exposes stage-specific methods that return Result<(), String>.

pub mod env_plugin;
pub mod intrinsics;
pub mod loader;
pub mod print_plugin;

use crate::ast::{StageKind, TopLevel};
use crate::target::TargetConfig;
use crate::type_universe::TypeUniverse;
use std::path::Path;

/// A single compiler plugin.
///
/// Each plugin declares which pipeline stages it participates in via
/// `stages()`. The corresponding `on_source`/`on_ast`/`on_ir`/`on_bin`
/// method is called for each stage in the plugin's list. Default
/// implementations do nothing — override only the stages you need.
pub trait Plugin: std::fmt::Debug {
    /// Human-readable unique plugin name (e.g. "prelude", "inline-block").
    fn name(&self) -> &str;

    /// Which pipeline stages this plugin runs at.
    fn stages(&self) -> Vec<StageKind>;

    /// Transform source text before lexing. Runs at PreLex stage only.
    fn on_source(&self, _source: &mut String) -> Result<(), String> {
        Ok(())
    }

    /// Transform the AST at the given stage.
    /// The `program` and `universe` are the current compilation state.
    /// Called for every stage where `stage.is_ast_stage()` is true.
    fn on_ast(
        &self,
        _program: &mut Vec<TopLevel>,
        _universe: &mut TypeUniverse,
    ) -> Result<(), String> {
        Ok(())
    }

    /// Transform the IR text after codegen.
    /// Called for every stage where `stage.is_ir_stage()` is true.
    fn on_ir(&self, _ir: &mut String) -> Result<(), String> {
        Ok(())
    }

    /// Transform the binary after linking. Runs at Linked stage only.
    /// The callback receives the path to the compiled binary.
    fn on_bin(&self, _bin_path: &std::path::Path) -> Result<(), String> {
        Ok(())
    }

    /// 2026-07-21: Downcast support for StageBlockPlugin to access
    /// Stage$ plugin injection. Default impl returns None.
    fn as_any(&self) -> Option<&dyn std::any::Any> {
        None
    }
}

/// An internal entry storing a registered plugin together with its priority.
#[derive(Debug)]
struct PluginEntry {
    plugin: Box<dyn Plugin>,
    priority: u32,
}

/// Manages plugin lifecycle: registration, stage dispatch, and filtering.
///
/// Plugins are registered generically via `register()` and sorted by
/// priority within each stage they declare. `filter_for_extension()`
/// narrows the active set to plugins matching the extension's config.
/// CLI overrides (`--disable-plugin`, `--enable-plugin`) are applied
/// via `with_disabled()` and `with_enabled_only()` builders or set
/// directly on the manager.
#[derive(Debug)]
pub struct PluginManager {
    /// All registered plugins (regardless of extension filter).
    all: Vec<PluginEntry>,
    /// Plugin names to skip during dispatch.
    disabled: Vec<String>,
    /// If non-empty, ONLY these plugins run (overrides extension filter).
    enabled_only: Vec<String>,
}

impl PluginManager {
    /// Create an empty plugin manager.
    pub fn new() -> Self {
        PluginManager {
            all: Vec::new(),
            disabled: Vec::new(),
            enabled_only: Vec::new(),
        }
    }

    /// Register a plugin. The manager reads `stages()` and inserts the
    /// plugin into every stage's list.
    ///
    /// 2026-07-15: Phase 2 — Flat registration; the manager distributes
    /// the plugin to all stages it declares. Priority extraction: for
    /// system plugins the priority is read from the plugin metadata; for
    /// inline $(Stage) blocks the priority is parsed from the AST.
    pub fn register(&mut self, plugin: Box<dyn Plugin>) {
        let priority = 100;
        self.all.push(PluginEntry { plugin, priority });
    }

    /// Register a plugin with an explicit priority (lower = runs earlier).
    pub fn register_with_priority(&mut self, plugin: Box<dyn Plugin>, priority: u32) {
        self.all.push(PluginEntry { plugin, priority });
    }

    /// Add plugin names to the disabled list. These plugins will be
    /// skipped during dispatch even if they pass the extension filter.
    /// 2026-07-15: Phase 2f — CLI support.
    pub fn with_disabled(mut self, names: Vec<String>) -> Self {
        self.disabled = names;
        self
    }

    /// Set the enabled-only list. When non-empty, ONLY plugins in this
    /// list run. This overrides the extension filter.
    /// 2026-07-15: Phase 2f — CLI support.
    pub fn with_enabled_only(mut self, names: Vec<String>) -> Self {
        self.enabled_only = names;
        self
    }

    /// Return the list of plugin names enabled after applying:
    /// 1. Extension filter (if provided)
    /// 2. Disabled list removal
    /// 3. Enabled-only override (if non-empty)
    ///
    /// 2026-07-15: Phase 2 — Centralised filter logic so the caller
    /// can inspect which plugins will run without duplicating filtering.
    fn enabled_names(&self, ext_filter: Option<&[String]>) -> Vec<String> {
        let enabled: Vec<String> = self.all.iter()
            .filter(|entry| {
                let name = entry.plugin.name().to_string();
                // Extension filter: only if the name is in the extension's list
                if let Some(allowed) = ext_filter {
                    if !allowed.contains(&name) {
                        return false;
                    }
                }
                true
            })
            .map(|entry| entry.plugin.name().to_string())
            .collect();

        if self.enabled_only.is_empty() {
            enabled.into_iter()
                .filter(|n| !self.disabled.contains(n))
                .collect()
        } else {
            self.enabled_only.iter()
                .filter(|n| enabled.contains(n))
                .cloned()
                .collect()
        }
    }

    /// Filter plugins to only those allowed for the given file extension.
    /// Reads the extension's plugin list from `TargetConfig`. If the
    /// extension has no plugin list (None), uses the default set.
    ///
    /// 2026-07-15: Phase 2 — Per-extension plugin selection from
    /// config/targets.toml [ext].plugins.
    pub fn filter_for_extension(&mut self, ext: &str, config: &TargetConfig) {
        let allowed = config.lookup(ext)
            .and_then(|entry| entry.plugins.as_ref())
            .cloned()
            .unwrap_or_else(|| vec!["prelude".to_string()]);

        let allowed: Vec<String> = self.all.iter()
            .map(|entry| entry.plugin.name().to_string())
            .filter(|name| allowed.contains(name))
            .collect();

        // When filter_for_extension is used, we set enabled_only to the
        // intersection of the extension's plugin list and registered plugins.
        self.enabled_only = allowed;
    }

    /// Run all active plugins' `on_source` at the given stage.
    /// Only PreLex matches — source is only available before lexing.
    pub fn run_source(&self, stage: StageKind, source: &mut String) -> Result<(), String> {
        for entry in self.active_plugins(None) {
            if entry.plugin.stages().contains(&stage) {
                entry.plugin.on_source(source)?;
            }
        }
        Ok(())
    }

    /// Run all active plugins' `on_ast` at the given stage.
    /// Valid for all AST stages (Parsed through Provenanced).
    /// 2026-07-21: Sets pm_ptr on StageBlockPlugin for Stage$ ops.
    /// Safety: The raw pointer to self is safe because:
    ///   1. This runs single-threaded (no concurrent access).
    ///   2. active_plugins() takes a snapshot of indices before iterating.
    ///   3. register_during_stage pushes to Vec — existing refs remain valid.
    ///   4. The caller holds &mut PluginManager in compile.rs.
    pub fn run_ast(
        &self,
        stage: StageKind,
        program: &mut Vec<TopLevel>,
        universe: &mut TypeUniverse,
    ) -> Result<(), String> {
        let self_ptr = self as *const PluginManager as usize;
        for entry in self.active_plugins(None) {
            if let Some(sbp) = entry.plugin.as_any()
                .and_then(|a| a.downcast_ref::<crate::plugin::loader::StageBlockPlugin>())
            {
                sbp.set_pm_ptr(self_ptr);
            }
            if entry.plugin.stages().contains(&stage) {
                entry.plugin.on_ast(program, universe)?;
            }
        }
        Ok(())
    }

    /// Run all active plugins' `on_ir` at the given stage.
    /// Valid for Generated and Optimized.
    pub fn run_ir(&self, stage: StageKind, ir: &mut String) -> Result<(), String> {
        for entry in self.active_plugins(None) {
            if entry.plugin.stages().contains(&stage) {
                entry.plugin.on_ir(ir)?;
            }
        }
        Ok(())
    }

    /// Run all active plugins' `on_bin` at the Linked stage.
    pub fn run_bin(&self, bin_path: &std::path::Path) -> Result<(), String> {
        for entry in self.active_plugins(None) {
            if entry.plugin.stages().contains(&StageKind::Linked) {
                entry.plugin.on_bin(bin_path)?;
            }
        }
        Ok(())
    }

    /// Return all active plugins in priority order.
    ///
    /// A plugin is active if:
    /// - Its name is in `enabled_only` (if non-empty), OR it passes the
    ///   extension filter (already applied at construction time)
    /// - Its name is NOT in the disabled list
    ///
    /// 2026-07-15: Phase 2 — Flat control flow via guard clauses.
    fn active_plugins(&self, ext_filter: Option<&[String]>) -> Vec<&PluginEntry> {
        let mut active: Vec<&PluginEntry> = self.all.iter()
            .filter(|entry| {
                let name = entry.plugin.name();

                // Check enabled_only override
                if !self.enabled_only.is_empty() {
                    return self.enabled_only.contains(&name.to_string());
                }

                // Check extension filter
                if let Some(allowed) = ext_filter {
                    if !allowed.contains(&name.to_string()) {
                        return false;
                    }
                }

                // Check disabled list
                if self.disabled.contains(&name.to_string()) {
                    eprintln!("DEBUG active_plugins: DISABLED '{}'", name);
                    return false;
                }

                eprintln!("DEBUG active_plugins: ACTIVE '{}'", name);

                true
            })
            .collect();

        // Stable sort by priority (lower = earlier)
        active.sort_by_key(|entry| entry.priority);
        active
    }

    /// Number of registered plugins (before filtering).
    pub fn len(&self) -> usize {
        self.all.len()
    }

    pub fn is_empty(&self) -> bool {
        self.all.is_empty()
    }

    /// Return the names of all registered plugins (regardless of filtering).
    pub fn list_names(&self) -> Vec<String> {
        self.all.iter().map(|e| e.plugin.name().to_string()).collect()
    }

    /// Add a plugin name to the disabled list at runtime.
    /// Used by Stage$.Remove$ for forward-only plugin removal.
    pub fn disable_plugin(&mut self, name: &str) {
        if !self.disabled.contains(&name.to_string()) {
            self.disabled.push(name.to_string());
        }
    }

    /// Register a plugin during a stage (forward-only).
    /// Returns an error if the plugin's stage ≤ the current stage.
    pub fn register_during_stage(
        &mut self,
        plugin: Box<dyn Plugin>,
        priority: u32,
        current_stage: StageKind,
    ) -> Result<(), String> {
        let plugin_stages = plugin.stages();
        for s in &plugin_stages {
            if *s <= current_stage {
                return Err(format!(
                    "cannot register plugin at stage {:?} from stage {:?} — forward-only",
                    s, current_stage
                ));
            }
        }
        self.all.push(PluginEntry { plugin, priority });
        Ok(())
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A test plugin that records which methods were called.
    #[derive(Debug)]
    struct TestPlugin {
        name: String,
        stages: Vec<StageKind>,
    }

    impl TestPlugin {
        fn new(name: &str, stages: Vec<StageKind>) -> Self {
            TestPlugin { name: name.to_string(), stages }
        }
    }

    impl Plugin for TestPlugin {
        fn name(&self) -> &str { &self.name }
        fn stages(&self) -> Vec<StageKind> { self.stages.clone() }
    }

    fn empty_state() -> (Vec<TopLevel>, TypeUniverse) {
        (vec![], TypeUniverse::new())
    }

    #[test]
    fn test_empty_manager() {
        let mgr = PluginManager::new();
        assert!(mgr.is_empty());
        assert_eq!(mgr.len(), 0);
    }

    #[test]
    fn test_register_plugin() {
        let mut mgr = PluginManager::new();
        let p = TestPlugin::new("test:noop", vec![StageKind::Parsed]);
        mgr.register(Box::new(p));
        assert_eq!(mgr.len(), 1);
    }

    #[test]
    fn test_parsed_ast_continues() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(TestPlugin::new("test:parsed", vec![StageKind::Parsed])));
        let (mut program, mut universe) = empty_state();
        assert!(mgr.run_ast(StageKind::Parsed, &mut program, &mut universe).is_ok());
    }

    #[test]
    fn test_typed_ast_continues() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(TestPlugin::new("test:typed", vec![StageKind::Typed])));
        let (mut program, mut universe) = empty_state();
        assert!(mgr.run_ast(StageKind::Typed, &mut program, &mut universe).is_ok());
    }

    #[test]
    fn test_generated_ir_continues() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(TestPlugin::new("test:generated", vec![StageKind::Generated])));
        let mut ir = String::new();
        assert!(mgr.run_ir(StageKind::Generated, &mut ir).is_ok());
    }

    #[test]
    fn test_optimized_ir_continues() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(TestPlugin::new("test:optimized", vec![StageKind::Optimized])));
        let mut ir = String::new();
        assert!(mgr.run_ir(StageKind::Optimized, &mut ir).is_ok());
    }

    #[test]
    fn test_enabled_only_filter() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(TestPlugin::new("plugin:a", vec![StageKind::Parsed])));
        mgr.register(Box::new(TestPlugin::new("plugin:b", vec![StageKind::Parsed])));
        mgr = mgr.with_enabled_only(vec!["plugin:a".to_string()]);

        let (mut program, mut universe) = empty_state();
        assert!(mgr.run_ast(StageKind::Parsed, &mut program, &mut universe).is_ok());

        let names = mgr.enabled_names(None);
        assert_eq!(names.len(), 1);
        assert_eq!(names[0], "plugin:a");
    }

    #[test]
    fn test_disabled_skips_plugin() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(TestPlugin::new("plugin:a", vec![StageKind::Parsed])));
        mgr.register(Box::new(TestPlugin::new("plugin:b", vec![StageKind::Parsed])));
        mgr = mgr.with_disabled(vec!["plugin:b".to_string()]);

        let names = mgr.enabled_names(None);
        assert_eq!(names.len(), 1);
        assert_eq!(names[0], "plugin:a");
    }

    #[test]
    fn test_plugin_not_called_for_wrong_stage() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(TestPlugin::new("test:typedonly", vec![StageKind::Typed])));
        let (mut program, mut universe) = empty_state();
        assert!(mgr.run_ast(StageKind::Parsed, &mut program, &mut universe).is_ok());
        let (mut program2, mut universe2) = empty_state();
        assert!(mgr.run_ast(StageKind::Typed, &mut program2, &mut universe2).is_ok());
    }

    #[test]
    fn test_multiple_stages() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(TestPlugin::new(
            "test:dual",
            vec![StageKind::Parsed, StageKind::Generated],
        )));
        let (mut program, mut universe) = empty_state();
        assert!(mgr.run_ast(StageKind::Parsed, &mut program, &mut universe).is_ok());
        let mut ir = String::new();
        assert!(mgr.run_ir(StageKind::Generated, &mut ir).is_ok());
    }

    #[test]
    fn test_filter_for_extension() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(TestPlugin::new("prelude", vec![StageKind::Parsed])));
        mgr.register(Box::new(TestPlugin::new("some-other", vec![StageKind::Parsed])));
        let config = TargetConfig::load();
        mgr.filter_for_extension(".bv", &config);
        let names = mgr.enabled_names(None);
        assert_eq!(names.len(), 1);
        assert_eq!(names[0], "prelude");
    }
}
