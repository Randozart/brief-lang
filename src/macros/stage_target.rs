// ── Stage$ Target — Plugin Injection ────────────────────────────────────
// 2026-07-21: Stage$.Insert$/Remove$/List$ for forward-only plugin
// injection. A $(Stage) block can register new plugins for later stages.
// Forward-only: N → >N only, enforced at registration time.

use crate::ast::StageKind;
use crate::plugin::PluginManager;

/// Names of all registered plugins (for Stage$.List$).
pub fn list_plugins(pm: &PluginManager) -> Vec<String> {
    pm.list_names()
}

/// Remove a plugin by name (Stage$.Remove$).
pub fn remove_plugin(pm: &mut PluginManager, name: &str) {
    pm.disable_plugin(name);
}

/// Register a new plugin from a $(Stage) block file path (Stage$.Insert$).
/// In Phase H, this parses the file and registers its blocks.
/// For now, returns a placeholder message.
pub fn insert_plugin_from_file(pm: &mut PluginManager, _path: &str, current_stage: StageKind) -> Result<(), String> {
    let _ = pm;
    let _ = current_stage;
    // Placeholder — full implementation in Phase H follow-up:
    // 1. Load file, parse it
    // 2. Extract $(Stage) blocks
    // 3. For each block, check stage > current_stage
    // 4. Register as StageBlockPlugin
    Err("Stage$.Insert$(path) not yet implemented — Phase H placeholder".into())
}
