// Copyright 2026 Randy Smits-Schreuder Goedheijt
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// Runtime Exception for Use as a Language:
// When the Work or any Derivative Work thereof is used to generate code
// ("generated code"), such generated code shall not be subject to the
// terms of this License, provided that the generated code itself is not
// a Derivative Work of the Work. This exception does not apply to code
// that is itself a compiler, interpreter, or similar tool that incorporates
// or embeds the Work.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ManifestError {
    #[error("Failed to parse manifest: {0}")]
    ParseError(String),
    #[error("Manifest file not found at {0}")]
    NotFound(PathBuf),
    #[error("Invalid dependency specification for '{0}': {1}")]
    InvalidDependency(String, String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    #[serde(default)]
    pub project: Project,
    #[serde(default)]
    pub dependencies: HashMap<String, Dependency>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Project {
    #[serde(default = "default_name")]
    pub name: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default = "default_entry")]
    pub entry: String,
}

fn default_name() -> String {
    "unnamed-project".to_string()
}

fn default_version() -> String {
    "0.1.0".to_string()
}

fn default_entry() -> String {
    "main.bv".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Dependency {
    Path(PathDependency),
    Registry(RegistryDependency),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathDependency {
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryDependency {
    pub registry: String,
    #[serde(default)]
    pub version: Option<String>,
}

impl Manifest {
    pub fn load(path: &Path) -> Result<Self, ManifestError> {
        if !path.exists() {
            return Err(ManifestError::NotFound(path.to_path_buf()));
        }
        let content = fs::read_to_string(path)?;
        Self::parse(&content)
    }

    pub fn parse(content: &str) -> Result<Self, ManifestError> {
        toml::from_str(content).map_err(|e| ManifestError::ParseError(e.to_string()))
    }

    pub fn save(&self, path: &Path) -> Result<(), ManifestError> {
        let content =
            toml::to_string_pretty(self).map_err(|e| ManifestError::ParseError(e.to_string()))?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn find_dependency(&self, name: &str) -> Option<&Dependency> {
        self.dependencies.get(name)
    }

    pub fn add_dependency(&mut self, name: String, dep: Dependency) {
        self.dependencies.insert(name, dep);
    }

    pub fn remove_dependency(&mut self, name: &str) -> Option<Dependency> {
        self.dependencies.remove(name)
    }

    pub fn resolve_path(&self, name: &str, project_root: &Path) -> Option<PathBuf> {
        match self.find_dependency(name)? {
            Dependency::Path(p) => {
                let resolved = project_root.join(&p.path);
                if resolved.exists() {
                    Some(resolved)
                } else {
                    None
                }
            }
            Dependency::Registry(_) => None,
        }
    }

    pub fn project_dir(&self, manifest_path: &Path) -> PathBuf {
        manifest_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

impl fmt::Display for Manifest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} v{} ({})",
            self.project.name, self.project.version, self.project.entry
        )?;
        if !self.dependencies.is_empty() {
            write!(f, "\n\nDependencies:")?;
            for (name, dep) in &self.dependencies {
                write!(f, "\n  {}: ", name)?;
                match dep {
                    Dependency::Path(p) => write!(f, "{}", p.path.display())?,
                    Dependency::Registry(r) => {
                        write!(f, "{} v{}", r.registry, r.version.as_deref().unwrap_or("*"))?
                    }
                }
            }
        }
        Ok(())
    }
}

pub fn find_manifest(start_dir: &Path) -> Option<PathBuf> {
    let mut current = start_dir.to_path_buf();

    loop {
        let manifest_path = current.join("brief.toml");
        if manifest_path.exists() {
            return Some(manifest_path);
        }

        let parent = current.parent()?;
        if parent == current {
            break;
        }
        current = parent.to_path_buf();
    }

    None
}

pub fn create_default_manifest(path: &Path) -> Result<Manifest, ManifestError> {
    let manifest = Manifest {
        project: Project {
            name: path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(default_name),
            version: default_version(),
            entry: default_entry(),
        },
        dependencies: HashMap::new(),
    };
    manifest.save(path)?;
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_manifest() {
        let content = r#"
[project]
name = "test-project"
version = "0.2.0"
entry = "src/main.bv"

[dependencies]
auth = { path = "lib/auth.bv" }
utils = { path = "lib/utils.bv" }
"#;
        let manifest = Manifest::parse(content).unwrap();
        assert_eq!(manifest.project.name, "test-project");
        assert_eq!(manifest.project.version, "0.2.0");
        assert_eq!(manifest.dependencies.len(), 2);
    }

    #[test]
    fn test_find_manifest() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path();

        fs::create_dir(project_dir.join("src")).unwrap();
        let manifest_path = project_dir.join("brief.toml");
        fs::write(&manifest_path, "").unwrap();

        let found = find_manifest(&project_dir.join("src").join("main.bv"));
        assert!(found.is_some());
        assert_eq!(found.unwrap(), manifest_path);
    }
}
