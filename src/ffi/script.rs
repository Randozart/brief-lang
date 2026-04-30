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

//! Script Import Resolver
//!
//! Handles importing scripts/libraries for FFI:
//! - JavaScript files (.js)
//! - C libraries (.a, .so)
//! - WebAssembly modules (.wasm)
//!
//! No LUT - functions are resolved by exact name match from imported scripts

use std::collections::HashMap;
use std::path::PathBuf;

/// Represents a resolved foreign function from an imported script
#[derive(Debug, Clone)]
pub struct ScriptFunction {
    pub name: String,
    pub return_type: String,
    pub parameters: Vec<(String, String)>,
    pub source_lang: ScriptLanguage,
}

/// Supported script languages
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptLanguage {
    JavaScript,
    C,
    WASM,
    Unknown,
}

/// Script import resolver - no LUT, direct name matching
pub struct ScriptResolver {
    loaded_scripts: HashMap<String, Vec<ScriptFunction>>,
}

impl ScriptResolver {
    pub fn new() -> Self {
        Self {
            loaded_scripts: HashMap::new(),
        }
    }

    /// Resolve a script import path and load functions
    /// Returns all functions found in the script
    pub fn resolve_import(&mut self, path: &str, source_file: &PathBuf) -> Result<Vec<ScriptFunction>, String> {
        // Resolve relative to source file
        let resolved = if path.starts_with('/') || path.contains(':') {
            PathBuf::from(path)
        } else {
            source_file
                .parent()
                .map(|p| p.join(path))
                .unwrap_or_else(|| PathBuf::from(path))
        };

        let extension = resolved
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        let lang = match extension.to_lowercase().as_str() {
            "js" => ScriptLanguage::JavaScript,
            "c" | "h" => ScriptLanguage::C,
            "a" | "so" | "dylib" => ScriptLanguage::C, // Library file
            "wasm" => ScriptLanguage::WASM,
            _ => ScriptLanguage::Unknown,
        };

        // Load based on language
        let functions = match lang {
            ScriptLanguage::JavaScript => self.load_js(&resolved)?,
            ScriptLanguage::C => self.load_c_header(&resolved)?,
            ScriptLanguage::WASM => self.load_wasm(&resolved)?,
            ScriptLanguage::Unknown => {
                return Err(format!("Unknown script type: {}", path));
            }
        };

        // Cache by path
        let path_str = resolved.to_string_lossy().to_string();
        self.loaded_scripts.insert(path_str, functions.clone());

        Ok(functions)
    }

    /// Load JavaScript file and extract function signatures
    /// Currently parses common patterns; full parsing would require JS AST
    fn load_js(&self, path: &PathBuf) -> Result<Vec<ScriptFunction>, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read JS file: {}", e))?;

        let mut functions = Vec::new();

        // Simple pattern matching for common JS function signatures
        // This is a simplified approach - full JS parsing would require proper AST
        for line in content.lines() {
            let trimmed = line.trim();
            
            // Match: function name(arg, arg) { or const name = (arg) => {
            if trimmed.starts_with("function ") {
                if let Some(name_end) = trimmed.find('(') {
                    let name = trimmed[9..name_end].trim().to_string();
                    if !name.is_empty() {
                        // Extract parameters
                        let params_start = name_end + 1;
                        let params_end = trimmed.find(')').unwrap_or(params_start);
                        let params_str = &trimmed[params_start..params_end];
                        let params: Vec<(String, String)> = params_str
                            .split(',')
                            .filter_map(|p| {
                                let p = p.trim();
                                if p.is_empty() {
                                    None
                                } else {
                                    Some((p.to_string(), "unknown".to_string()))
                                }
                            })
                            .collect();
                        
                        functions.push(ScriptFunction {
                            name,
                            return_type: "unknown".to_string(),
                            parameters: params,
                            source_lang: ScriptLanguage::JavaScript,
                        });
                    }
                }
            }
            // Match arrow functions: const name = (arg) => {
            else if trimmed.starts_with("const ") || trimmed.starts_with("let ") || trimmed.starts_with("var ") {
                if let Some(name_end) = trimmed.find(" = ") {
                    let name_start = trimmed.find("const ").map(|i| i + 6)
                        .or_else(|| trimmed.find("let ").map(|i| i + 4))
                        .or_else(|| trimmed.find("var ").map(|i| i + 4))
                        .unwrap_or(0);
                    let name = trimmed[name_start..name_end].trim().to_string();
                    if !name.is_empty() && !name.starts_with('{') {
                        functions.push(ScriptFunction {
                            name,
                            return_type: "unknown".to_string(),
                            parameters: vec![],
                            source_lang: ScriptLanguage::JavaScript,
                        });
                    }
                }
            }
        }

        Ok(functions)
    }

    /// Load C header file and extract function signatures
    fn load_c_header(&self, path: &PathBuf) -> Result<Vec<ScriptFunction>, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read C header: {}", e))?;

        let mut functions = Vec::new();

        // Parse C function declarations
        // Matches: return_type function_name(param_type param_name, ...);
        for line in content.lines() {
            let trimmed = line.trim();
            
            // Skip comments and preprocessor
            if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('#') {
                continue;
            }

            // Simple regex-like parsing for: type name(params);
            if let Some(semi_pos) = trimmed.find(';') {
                let declaration = &trimmed[..semi_pos];
                
                // Find the opening paren for parameters
                if let Some(paren_start) = declaration.find('(') {
                    // Find the last word before paren - that's the function name
                    let before_paren = declaration[..paren_start].trim();
                    if let Some(space_pos) = before_paren.rfind(' ') {
                        let return_type = before_paren[..space_pos].to_string();
                        let name = before_paren[space_pos + 1..].to_string();
                        
                        // Skip keywords like if, while, etc.
                        if name == "if" || name == "while" || name == "for" || name == "return" {
                            continue;
                        }

                        // Extract parameters
                        let params_str = &declaration[paren_start + 1..];
                        let params_end = params_str.find(')').unwrap_or(params_str.len());
                        let params_part = &params_str[..params_end];
                        
                        let params: Vec<(String, String)> = params_part
                            .split(',')
                            .filter_map(|p| {
                                let p = p.trim();
                                if p.is_empty() || p == "void" {
                                    return None;
                                }
                                // Split by last space to get name and type
                                let parts: Vec<&str> = p.rsplitn(2, ' ').collect();
                                if parts.len() == 2 {
                                    Some((parts[0].to_string(), parts[1].to_string()))
                                } else {
                                    Some(("param".to_string(), p.to_string()))
                                }
                            })
                            .collect();

                        if !name.is_empty() {
                            functions.push(ScriptFunction {
                                name,
                                return_type,
                                parameters: params,
                                source_lang: ScriptLanguage::C,
                            });
                        }
                    }
                }
            }
        }

        Ok(functions)
    }

    /// Load WASM module - requires wasm-bindgen or similar for full parsing
    /// For now, this is a placeholder that would need external tooling
    fn load_wasm(&self, path: &PathBuf) -> Result<Vec<ScriptFunction>, String> {
        // WASM binary format parsing would go here
        // For now, return empty - full WASM support would require 
        // wasmparser or similar crate
        Ok(vec![])
    }

    /// Find a function by exact name in loaded scripts
    /// Returns None if not found (no LUT - direct match only)
    pub fn find_function(&self, name: &str) -> Option<ScriptFunction> {
        for functions in self.loaded_scripts.values() {
            for func in functions {
                if func.name == name {
                    return Some(func.clone());
                }
            }
        }
        None
    }

    /// Get all functions from all loaded scripts
    pub fn all_functions(&self) -> Vec<ScriptFunction> {
        self.loaded_scripts.values().flatten().cloned().collect()
    }
}

impl Default for ScriptResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_script_resolver() {
        let resolver = ScriptResolver::new();
        assert!(resolver.find_function("test").is_none());
    }
}