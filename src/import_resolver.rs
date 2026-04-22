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

use crate::ast::{Import, ImportItem, Program, TopLevel};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct ImportResolver {
    search_paths: Vec<PathBuf>,
    loaded_modules: HashMap<String, Program>,
}

impl ImportResolver {
    pub fn new() -> Self {
        ImportResolver {
            search_paths: vec![
                PathBuf::from("lib"),
                PathBuf::from("imports"),
                PathBuf::from("."),
            ],
            loaded_modules: HashMap::new(),
        }
    }

    pub fn add_search_path(&mut self, path: PathBuf) {
        self.search_paths.push(path);
    }

    pub fn resolve_imports(
        &mut self,
        program: &Program,
        file_path: &PathBuf,
    ) -> Result<Program, String> {
        let mut items = program.items.clone();
        let mut index = 0;

        while index < items.len() {
            if let TopLevel::Import(import) = &items[index] {
                let resolved = self.resolve_import(import, file_path)?;
                items.remove(index);
                items.splice(index..index, resolved.items.clone());
            } else {
                index += 1;
            }
        }

        Ok(Program {
            items,
            comments: program.comments.clone(),
            reactor_speed: program.reactor_speed,
        })
    }

    fn resolve_import(
        &mut self,
        import: &Import,
        source_file: &PathBuf,
    ) -> Result<Program, String> {
        if import.items.is_empty() && import.path.is_empty() {
            return Ok(Program {
                items: vec![],
                comments: vec![],
                reactor_speed: None,
            });
        }

        let path_str = if import.path.is_empty() {
            return Ok(Program {
                items: vec![],
                comments: vec![],
                reactor_speed: None,
            });
        } else {
            // Check if this is a file-based import (ends with .css, .svg, etc.)
            // If so, use slashes instead of dots to preserve the file path
            let last_component = import.path.last().unwrap();
            if last_component.ends_with(".css") || last_component.ends_with(".svg") {
                import.path.join("/")
            } else {
                import.path.join(".")
            }
        };

        if let Some(cached) = self.loaded_modules.get(&path_str) {
            return self.filter_items(cached, &import.items);
        }

        // Check for CSS import
        if path_str.ends_with(".css") {
            let css_path = source_file
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."))
                .join(&path_str);

            if css_path.exists() {
                let css_content = std::fs::read_to_string(&css_path)
                    .map_err(|e| format!("Failed to read CSS '{}': {}", css_path.display(), e))?;
                let css_for_cache = css_content.clone();
                let css_for_return = css_content.clone();
                self.loaded_modules.insert(
                    path_str.clone(),
                    Program {
                        items: vec![TopLevel::Stylesheet(css_for_cache)],
                        comments: vec![],
                        reactor_speed: None,
                    },
                );
                return Ok(Program {
                    items: vec![TopLevel::Stylesheet(css_for_return)],
                    comments: vec![],
                    reactor_speed: None,
                });
            }
        }

        // Check for SVG import
        if path_str.ends_with(".svg") {
            let svg_path = source_file
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."))
                .join(&path_str);

            if svg_path.exists() {
                let svg_content = std::fs::read_to_string(&svg_path)
                    .map_err(|e| format!("Failed to read SVG '{}': {}", svg_path.display(), e))?;
                // Extract alias name from import items
                let component_name = import
                    .items
                    .first()
                    .map(|item| item.alias.as_ref().unwrap_or(&item.name).clone())
                    .unwrap_or_else(|| {
                        // Fallback: sanitize filename
                        // Extract just the filename from the path (e.g., "assets/logo.svg" -> "logo")
                        let file_name = if let Some(last_slash) = path_str.rfind('/') {
                            &path_str[last_slash + 1..]
                        } else {
                            &path_str
                        };
                        let file_name = file_name.trim_end_matches(".svg");
                        file_name
                            .split('-')
                            .map(|s| {
                                let mut chars = s.chars();
                                match chars.next() {
                                    Some(c) => {
                                        c.to_uppercase().collect::<String>() + chars.as_str()
                                    }
                                    None => String::new(),
                                }
                            })
                            .collect::<String>()
                    });
                let svg_for_cache = svg_content.clone();
                let svg_for_return = svg_content.clone();
                self.loaded_modules.insert(
                    path_str.clone(),
                    Program {
                        items: vec![TopLevel::SvgComponent {
                            name: component_name.clone(),
                            content: svg_for_cache,
                        }],
                        comments: vec![],
                        reactor_speed: None,
                    },
                );
                return Ok(Program {
                    items: vec![TopLevel::SvgComponent {
                        name: component_name,
                        content: svg_for_return,
                    }],
                    comments: vec![],
                    reactor_speed: None,
                });
            }
        }

        // Default: Brief module (.bv or .ebv)
        let module_path = path_str.replace('.', "/");
        let source_dir = source_file
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        // Try both .bv and .ebv extensions
        let mut found_path = None;
        let mut found_both = false;
        for search_dir in &self.search_paths {
            let bv_candidate = source_dir
                .join(search_dir)
                .join(format!("{}.bv", module_path));
            let ebv_candidate = source_dir
                .join(search_dir)
                .join(format!("{}.ebv", module_path));

            if bv_candidate.exists() && ebv_candidate.exists() {
                found_both = true;
                break;
            } else if bv_candidate.exists() {
                found_path = Some(bv_candidate);
            } else if ebv_candidate.exists() {
                found_path = Some(ebv_candidate);
            }
        }

        if !found_both && found_path.is_none() {
            let direct_bv = source_dir.join(format!("{}.bv", module_path));
            let direct_ebv = source_dir.join(format!("{}.ebv", module_path));

            if direct_bv.exists() && direct_ebv.exists() {
                found_both = true;
            } else if direct_bv.exists() {
                found_path = Some(direct_bv);
            } else if direct_ebv.exists() {
                found_path = Some(direct_ebv);
            }
        }

        if found_both {
            return Err(format!(
                "Ambiguous import '{}'. Both .bv and .ebv files exist. Please specify the extension.",
                path_str
            ));
        }

        let resolved_path = found_path.ok_or_else(|| {
            format!(
                "Cannot find module '{}'. Searched in: lib/{}.{{bv,ebv}}, imports/{}.{{bv,ebv}}, ./{}.{{bv,ebv}}",
                path_str,
                module_path,
                module_path,
                module_path
            )
        })?;

        let source = std::fs::read_to_string(&resolved_path)
            .map_err(|e| format!("Failed to read '{}': {}", resolved_path.display(), e))?;

        let mut parser = crate::parser::Parser::new(&source);
        let imported_program = parser
            .parse()
            .map_err(|e| format!("Failed to parse '{}': {}", resolved_path.display(), e))?;

        self.loaded_modules
            .insert(path_str.clone(), imported_program.clone());

        let resolved = self.resolve_imports(&imported_program, &resolved_path)?;
        self.filter_items(&resolved, &import.items)
    }

    fn filter_items(&self, program: &Program, items: &[ImportItem]) -> Result<Program, String> {
        if items.is_empty() {
            return Ok(program.clone());
        }

        let item_names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();

        let filtered: Vec<TopLevel> = program
            .items
            .iter()
            .filter(|item| {
                let name = match item {
                    TopLevel::Definition(d) => Some(d.name.as_str()),
                    TopLevel::Signature(s) => Some(s.name.as_str()),
                    TopLevel::ForeignBinding { name, .. } => Some(name.as_str()),
                    TopLevel::Constant(c) => Some(c.name.as_str()),
                    TopLevel::Struct(s) => Some(s.name.as_str()),
                    TopLevel::RStruct(r) => Some(r.name.as_str()),
                    TopLevel::RenderBlock(rb) => Some(rb.struct_name.as_str()),
                    _ => None,
                };
                name.map(|n| item_names.contains(&n)).unwrap_or(false)
            })
            .cloned()
            .collect();

        Ok(Program {
            items: filtered,
            comments: vec![],
            reactor_speed: None,
        })
    }
}

impl Default for ImportResolver {
    fn default() -> Self {
        Self::new()
    }
}
