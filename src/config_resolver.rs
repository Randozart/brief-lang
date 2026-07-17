// ── ConfigResolver — Runtime Config File Resolution ────────────────────
// 2026-07-16: P1 — Centralizes config loading so profiles can be swapped
// at runtime via `brief-compiler config set <profile>`.
// Resolution chain: --config-dir CLI flag → BRIEF_CONFIG_DIR env var →
// ./.brief/config/ (project) → ~/.config/brief-compiler/active_profile →
// compile-time baked fallback.

use crate::config::{OpConfig, TypeConfig};
use crate::target::TargetConfig;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// 2026-07-16: Resolved configuration for a single compilation session.
/// Loaded once at pipeline start and threaded through CompilerContext.
pub struct ConfigResolver {
    pub config_dir: PathBuf,
    pub target_config: TargetConfig,
    pub type_config: TypeConfig,
    pub op_config: OpConfig,
    pub spirv_op_config: OpConfig,
    pub module_registry: HashMap<String, String>,
}

impl ConfigResolver {
    /// Follow the resolution chain to find config files and load them.
    /// Priority:
    ///   1. --config-dir CLI flag override
    ///   2. BRIEF_CONFIG_DIR env var
    ///   3. ./.brief/config/ (project-local)
    ///   4. ~/.config/brief-compiler/active_profile symlink/text
    ///   5. Compile-time baked fallback (path = "__baked__")
    pub fn resolve(config_dir_override: Option<&Path>) -> Self {
        let config_dir = Self::resolve_config_dir(config_dir_override);

        let target_config = if config_dir.to_string_lossy() == "__baked__" {
            TargetConfig::load()
        } else {
            let path = config_dir.join("targets.toml");
            TargetConfig::load_from(&path).unwrap_or_else(|e| {
                eprintln!("warning: cannot load '{}': {} — using baked fallback", path.display(), e);
                TargetConfig::load()
            })
        };

        let type_config = if config_dir.to_string_lossy() == "__baked__" {
            TypeConfig::load()
        } else {
            // 2026-07-17: Renamed from llvm-primitives.toml to ctd-llvm-mappings.toml
            let path = config_dir.join("ctd-llvm-mappings.toml");
            TypeConfig::load_from(&path).unwrap_or_else(|e| {
                eprintln!("warning: cannot load '{}': {} — using baked fallback", path.display(), e);
                TypeConfig::load()
            })
        };

        let op_config = if config_dir.to_string_lossy() == "__baked__" {
            OpConfig::load()
        } else {
            let path = config_dir.join("llvm-ops.toml");
            OpConfig::load_from_path(&path).unwrap_or_else(|e| {
                eprintln!("warning: cannot load '{}': {} — using baked fallback", path.display(), e);
                OpConfig::load()
            })
        };

        let spirv_op_config = if config_dir.to_string_lossy() == "__baked__" {
            OpConfig::load_from("spirv-ops.toml")
        } else {
            let path = config_dir.join("spirv-ops.toml");
            OpConfig::load_from_path(&path).unwrap_or_else(|e| {
                eprintln!("warning: cannot load '{}': {} — using baked fallback", path.display(), e);
                OpConfig::load_from("spirv-ops.toml")
            })
        };

        let module_registry = Self::load_module_registry(&config_dir);

        ConfigResolver {
            config_dir,
            target_config,
            type_config,
            op_config,
            spirv_op_config,
            module_registry,
        }
    }

    /// Resolve the config directory path through the priority chain.
    fn resolve_config_dir(override_dir: Option<&Path>) -> PathBuf {
        // 1. CLI override
        if let Some(dir) = override_dir {
            return dir.to_path_buf();
        }
        // 2. Environment variable
        if let Ok(env) = std::env::var("BRIEF_CONFIG_DIR") {
            return PathBuf::from(env);
        }
        // 3. Project-local
        if Path::new(".brief/config").exists() {
            return PathBuf::from(".brief/config");
        }
        // 4. User-global active profile
        if let Some(user_config) = dirs::config_dir() {
            let brief_config = user_config.join("brief-compiler");
            let active = brief_config.join("active_profile");
            // Try symlink first
            if let Ok(target) = std::fs::read_link(&active) {
                let profile_dir = if target.is_absolute() {
                    target
                } else {
                    brief_config.join(target)
                };
                if profile_dir.exists() {
                    return profile_dir;
                }
            }
            // Fallback: read as text file
            if let Ok(name) = std::fs::read_to_string(&active) {
                let profile_dir = brief_config.join("profiles").join(name.trim());
                if profile_dir.exists() {
                    return profile_dir;
                }
            }
        }
        // 5. Compile-time baked — marker path triggers fallback loading
        PathBuf::from("__baked__")
    }

    /// Load the module registry (config/module-registry.toml).
    fn load_module_registry(config_dir: &Path) -> HashMap<String, String> {
        let path = if config_dir.to_string_lossy() == "__baked__" {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("config/module-registry.toml")
        } else {
            config_dir.join("module-registry.toml")
        };
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(parsed) = toml::from_str::<ModuleRegistry>(&content) {
                return parsed.modules;
            }
        }
        HashMap::new()
    }
}

/// TOML structure for config/module-registry.toml
#[derive(serde::Deserialize)]
struct ModuleRegistry {
    modules: HashMap<String, String>,
}

/// Manage config profiles in ~/.config/brief-compiler/
pub fn list_profiles() -> Result<Vec<String>, String> {
    let base = config_base_dir()?;
    let profiles_dir = base.join("profiles");
    if !profiles_dir.exists() {
        return Ok(vec![]);
    }
    let mut profiles = Vec::new();
    for entry in std::fs::read_dir(&profiles_dir)
        .map_err(|e| format!("cannot read '{}': {}", profiles_dir.display(), e))?
    {
        let entry = entry.map_err(|e| format!("read error: {}", e))?;
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            if let Some(name) = entry.file_name().to_str() {
                profiles.push(name.to_string());
            }
        }
    }
    profiles.sort();
    Ok(profiles)
}

pub fn set_active_profile(name: &str) -> Result<(), String> {
    let base = config_base_dir()?;
    let profile_path = base.join("profiles").join(name);
    if !profile_path.exists() {
        return Err(format!("profile '{}' not found", name));
    }
    let active = base.join("active_profile");
    // Try symlink first
    if std::fs::remove_file(&active).is_err() {
        // File may not exist — ignore
    }
    // On platforms without symlink support, write a text file
    #[cfg(unix)]
    {
        let target = format!("profiles/{}", name);
        std::os::unix::fs::symlink(&target, &active)
            .map_err(|e| format!("cannot create symlink '{}': {}", active.display(), e))?;
    }
    #[cfg(not(unix))]
    {
        std::fs::write(&active, name)
            .map_err(|e| format!("cannot write '{}': {}", active.display(), e))?;
    }
    Ok(())
}

pub fn show_active_profile() -> Result<(), String> {
    let base = config_base_dir()?;
    let active = base.join("active_profile");
    let profile_dir = if let Ok(target) = std::fs::read_link(&active) {
        if target.is_absolute() { target } else { base.join(target) }
    } else if let Ok(name) = std::fs::read_to_string(&active) {
        base.join("profiles").join(name.trim())
    } else {
        return Err("no active profile set".to_string());
    };

    println!("Active profile: {}", profile_dir.display());
    println!();
    if profile_dir.exists() {
        for entry in std::fs::read_dir(&profile_dir)
            .map_err(|e| format!("cannot read '{}': {}", profile_dir.display(), e))?
        {
            let entry = entry.map_err(|e| format!("read error: {}", e))?;
            if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                if let Some(name) = entry.file_name().to_str() {
                    println!("  {}:", name);
                    if let Ok(content) = std::fs::read_to_string(entry.path()) {
                        for line in content.lines().take(10) {
                            println!("    {}", line);
                        }
                        if content.lines().count() > 10 {
                            println!("    ... ({} total lines)", content.lines().count());
                        }
                    }
                }
            }
        }
    } else {
        println!("  (profile directory does not exist)");
    }
    Ok(())
}

pub fn init_profile(name: &str) -> Result<(), String> {
    let base = config_base_dir()?;
    let profile_dir = base.join("profiles").join(name);
    std::fs::create_dir_all(&profile_dir)
        .map_err(|e| format!("cannot create '{}': {}", profile_dir.display(), e))?;

    let baked_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("config");
    // 2026-07-17: Renamed llvm-primitives.toml to ctd-llvm-mappings.toml
    for file in &["targets.toml", "ctd-llvm-mappings.toml", "llvm-ops.toml", "spirv-ops.toml"] {
        let src = baked_dir.join(file);
        if src.exists() {
            let content = std::fs::read_to_string(&src)
                .map_err(|e| format!("cannot read '{}': {}", src.display(), e))?;
            let dst = profile_dir.join(file);
            std::fs::write(&dst, &content)
                .map_err(|e| format!("cannot write '{}': {}", dst.display(), e))?;
            println!("wrote {}", dst.display());
        }
    }

    // Also copy module-registry.toml
    let registry_src = baked_dir.join("module-registry.toml");
    if registry_src.exists() {
        let content = std::fs::read_to_string(&registry_src)
            .map_err(|e| format!("cannot read '{}': {}", registry_src.display(), e))?;
        let dst = profile_dir.join("module-registry.toml");
        std::fs::write(&dst, &content)
            .map_err(|e| format!("cannot write '{}': {}", dst.display(), e))?;
        println!("wrote {}", dst.display());
    }

    // Set as active
    set_active_profile(name)?;
    println!("Profile '{}' initialized and set as active.", name);
    Ok(())
}

/// Get the base config directory (~/.config/brief-compiler/).
fn config_base_dir() -> Result<PathBuf, String> {
    dirs::config_dir()
        .map(|d| d.join("brief-compiler"))
        .ok_or_else(|| "cannot determine config directory (no $HOME?)".to_string())
}
