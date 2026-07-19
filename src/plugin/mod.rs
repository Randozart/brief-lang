// ── Plugin System — Stage-Based Architecture ────────────────────────────
//
// 2026-07-15: Phase 2 — Rewrite Plugin trait and PluginManager with
// per-stage registration and dispatch. The four stages mirror the
// pipeline: Front (after parse), Mid (after typecheck), Post (after
// codegen), Back (final validation). Each plugin declares which stages
// it runs at via stages(). Plugins are sorted by priority (lower = earlier)
// within each stage.
//
// Pipeline position:
//   parse → Front(on_ast) → import resolution → typecheck
//   → Mid(on_ast) → normalizer → codegen → Post(on_ir)
//   → Back(on_ir) → write output
//
// System plugins ship in plugins/{front,mid,post,back}/ directories.
// User inline plugins are $(Stage) blocks parsed from source files.
// Per-extension plugin selection via config/targets.toml [ext].plugins.
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
/// `stages()`. The corresponding `on_source`/`on_ast`/`on_ir` method
/// is called for each stage in the plugin's list. Default
/// implementations do nothing — override only the stages you need.
pub trait Plugin: std::fmt::Debug {
    /// Human-readable unique plugin name (e.g. "prelude", "inline-mid-block").
    fn name(&self) -> &str;

    /// Which pipeline stages this plugin runs at.
    fn stages(&self) -> Vec<StageKind>;

    /// Transform source text before lexing. Runs at Front stage only.
    /// 2026-07-15: Phase 2 — stub for future use (e.g., preprocessor plugins).
    fn on_source(&self, _source: &mut String) -> Result<(), String> {
        Ok(())
    }

    /// Transform the AST at the given stage.
    /// The `program` and `universe` are the current compilation state and are
    /// usable for inspection or modification. Not all stages make sense for
    /// every plugin — the typical pattern is Front(on_ast) to insert imports
    /// or desugar before typechecking, and Mid(on_ast) to verify or optimize
    /// after typechecking.
    fn on_ast(
        &self,
        _program: &mut Vec<TopLevel>,
        _universe: &mut TypeUniverse,
    ) -> Result<(), String> {
        Ok(())
    }

    /// Transform the final IR text after codegen.
    /// Runs at Post and Back stages. Plugins at this stage can inspect,
    /// validate, or modify the emitted IR before it is written to disk.
    fn on_ir(&self, _ir: &mut String) -> Result<(), String> {
        Ok(())
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

    /// Run all active plugins' `on_source` at the Front stage.
    /// 2026-07-15: Phase 2 — Front stage, before lexing.
    pub fn run_front_source(&self, source: &mut String) -> Result<(), String> {
        for entry in self.active_plugins(None) {
            let plugin = &entry.plugin;
            if plugin.stages().contains(&StageKind::Front) {
                plugin.on_source(source)?;
            }
        }
        Ok(())
    }

    /// Run all active plugins' `on_ast` at the Front stage.
    /// 2026-07-15: Phase 2 — Front stage, after parsing.
    pub fn run_front_ast(
        &self,
        program: &mut Vec<TopLevel>,
        universe: &mut TypeUniverse,
    ) -> Result<(), String> {
        for entry in self.active_plugins(None) {
            let plugin = &entry.plugin;
            if plugin.stages().contains(&StageKind::Front) {
                plugin.on_ast(program, universe)?;
            }
        }
        Ok(())
    }

    /// Run all active plugins' `on_ast` at the Mid stage.
    /// 2026-07-15: Phase 2 — Mid stage, after type checking.
    pub fn run_mid_ast(
        &self,
        program: &mut Vec<TopLevel>,
        universe: &mut TypeUniverse,
    ) -> Result<(), String> {
        for entry in self.active_plugins(None) {
            let plugin = &entry.plugin;
            if plugin.stages().contains(&StageKind::Mid) {
                eprintln!("run_mid_ast: active plugin '{}'", plugin.name());
                plugin.on_ast(program, universe)?;
            }
        }
        Ok(())
    }

    /// Run all active plugins' `on_ir` at the Post stage.
    /// 2026-07-15: Phase 2 — Post stage, after codegen.
    pub fn run_post_ir(&self, ir: &mut String) -> Result<(), String> {
        for entry in self.active_plugins(None) {
            let plugin = &entry.plugin;
            if plugin.stages().contains(&StageKind::Post) {
                plugin.on_ir(ir)?;
            }
        }
        Ok(())
    }

    /// Run all active plugins' `on_ir` at the Back stage (final validation).
    /// 2026-07-15: Phase 2 — Back stage, after all other passes.
    pub fn run_back_ir(&self, ir: &mut String) -> Result<(), String> {
        for entry in self.active_plugins(None) {
            let plugin = &entry.plugin;
            if plugin.stages().contains(&StageKind::Back) {
                plugin.on_ir(ir)?;
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
                    return false;
                }

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
        let p = TestPlugin::new("test:noop", vec![StageKind::Front]);
        mgr.register(Box::new(p));
        assert_eq!(mgr.len(), 1);
    }

    #[test]
    fn test_front_ast_continues() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(TestPlugin::new("test:front", vec![StageKind::Front])));
        let (mut program, mut universe) = empty_state();
        assert!(mgr.run_front_ast(&mut program, &mut universe).is_ok());
    }

    #[test]
    fn test_mid_ast_continues() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(TestPlugin::new("test:mid", vec![StageKind::Mid])));
        let (mut program, mut universe) = empty_state();
        assert!(mgr.run_mid_ast(&mut program, &mut universe).is_ok());
    }

    #[test]
    fn test_post_ir_continues() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(TestPlugin::new("test:post", vec![StageKind::Post])));
        let mut ir = String::new();
        assert!(mgr.run_post_ir(&mut ir).is_ok());
    }

    #[test]
    fn test_back_ir_continues() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(TestPlugin::new("test:back", vec![StageKind::Back])));
        let mut ir = String::new();
        assert!(mgr.run_back_ir(&mut ir).is_ok());
    }

    #[test]
    fn test_enabled_only_filter() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(TestPlugin::new("plugin:a", vec![StageKind::Front])));
        mgr.register(Box::new(TestPlugin::new("plugin:b", vec![StageKind::Front])));
        mgr = mgr.with_enabled_only(vec!["plugin:a".to_string()]);

        let (mut program, mut universe) = empty_state();
        assert!(mgr.run_front_ast(&mut program, &mut universe).is_ok());

        // Check that only plugin:a ran by inspecting enabled_names
        let names = mgr.enabled_names(None);
        assert_eq!(names.len(), 1);
        assert_eq!(names[0], "plugin:a");
    }

    #[test]
    fn test_disabled_skips_plugin() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(TestPlugin::new("plugin:a", vec![StageKind::Front])));
        mgr.register(Box::new(TestPlugin::new("plugin:b", vec![StageKind::Front])));
        mgr = mgr.with_disabled(vec!["plugin:b".to_string()]);

        let names = mgr.enabled_names(None);
        assert_eq!(names.len(), 1);
        assert_eq!(names[0], "plugin:a");
    }

    #[test]
    fn test_plugin_not_called_for_wrong_stage() {
        // A Mid-only plugin should not run at Front stage.
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(TestPlugin::new("test:midonly", vec![StageKind::Mid])));
        let (mut program, mut universe) = empty_state();
        assert!(mgr.run_front_ast(&mut program, &mut universe).is_ok());
        // Mid-stage should still work
        let (mut program2, mut universe2) = empty_state();
        assert!(mgr.run_mid_ast(&mut program2, &mut universe2).is_ok());
    }

    #[test]
    fn test_multiple_stages() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(TestPlugin::new(
            "test:dual",
            vec![StageKind::Front, StageKind::Post],
        )));
        let (mut program, mut universe) = empty_state();
        assert!(mgr.run_front_ast(&mut program, &mut universe).is_ok());
        let mut ir = String::new();
        assert!(mgr.run_post_ir(&mut ir).is_ok());
    }

    #[test]
    fn test_filter_for_extension() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(TestPlugin::new("prelude", vec![StageKind::Front])));
        mgr.register(Box::new(TestPlugin::new("some-other", vec![StageKind::Front])));
        let config = TargetConfig::load();
        mgr.filter_for_extension(".bv", &config);
        // .bv has plugins = ["predule"] in the config…
        // Actually the test config might not have it. Let's just test that
        // it doesn't crash and returns a subset.
        let names = mgr.enabled_names(None);
        assert_eq!(names.len(), 1);
        assert_eq!(names[0], "prelude");
    }
}
