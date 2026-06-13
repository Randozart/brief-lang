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

use crate::ast::{Import, ImportItem, Program, StrictMode, TopLevel, Expr};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use crate::dbrief::v2 as dbrief_v2;

/// Recursively fill in the `path` field of any `Expr::DbvlTable` in an expression tree.
fn inject_dbvl_path(expr: &mut Expr, file_path: &str) {
    if let Expr::MapLiteral(pairs) = expr {
        for (_, val) in pairs.iter_mut() {
            inject_dbvl_path(val, file_path);
        }
    } else {
        expr.set_dbvl_path(file_path);
    }
}

pub struct ImportResolver {
    loaded_modules: HashMap<String, (Program, Vec<String>)>,
    search_paths: Vec<PathBuf>,
    strict_mode: StrictMode,
    root_path: PathBuf,
}

impl ImportResolver {
    pub fn new() -> Self {
        ImportResolver {
            loaded_modules: HashMap::new(),
            search_paths: vec![PathBuf::from("lib"), PathBuf::from("imports"), PathBuf::from(".")],
            strict_mode: StrictMode::Off,
            root_path: PathBuf::from("."),
        }
    }

    /// Set strict mode for all resolved imports (propagated to all Program objects)
    pub fn with_strict_mode(mut self, strict: bool) -> Self {
        self.strict_mode = if strict { StrictMode::Strict } else { StrictMode::Off };
        self
    }

    pub fn add_search_path(&mut self, path: PathBuf) {
        self.search_paths.push(path);
    }

    pub fn resolve_imports(
        &mut self,
        program: &Program,
        file_path: &PathBuf,
    ) -> Result<Program, String> {
        // Set root path from the main file's directory on first call
        if self.root_path == PathBuf::from(".") {
            self.root_path = file_path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."));
        }

        let mut items = program.items.clone();
        let mut index = 0;

        while index < items.len() {
            if let TopLevel::Import(import) = &items[index] {
                // DBrief imports (dbv/dbvl/dbvs) are handled in resolve_import,
                // not skipped like before.
                let resolved = self.resolve_import(import, file_path)?;
                items.remove(index);
                items.splice(index..index, resolved.items.clone());
            } else {
                index += 1;
            }
        }

        // 2026-06-13: Dedup items — same module imported through multiple paths
        // produces duplicate items. Keep first occurrence of each named item.
        // E.g., officina.bv imports "understand" directly AND via "layout" →
        // understand.bv's items appear twice without this dedup.
        items = dedup_items(items);

        Ok(Program {
            items,
            comments: program.comments.clone(),
            reactor_speed: program.reactor_speed,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: self.strict_mode,
            dispatch_mode: program.dispatch_mode,
            exit_condition: program.exit_condition.clone(),
            out_pragmas: program.out_pragmas.clone(),
            default_sig_modifier: program.default_sig_modifier,
        })
    }

    fn resolve_import(
        &mut self,
        import: &Import,
        source_file: &PathBuf,
    ) -> Result<Program, String> {
        // Skip .dbvs schema imports - they're handled by schema validation, not as Brief modules
        let path_str = if import.path.is_empty() {
            return Ok(Program {
                items: vec![],
                comments: vec![],
                reactor_speed: None,
                attrs: Vec::new(),
                ffi: None,
                strict_mode: self.strict_mode,
                dispatch_mode: Default::default(),
                exit_condition: None,
                out_pragmas: vec![],
            default_sig_modifier: None,
            });
        } else {
            // Check if this is a file-based import (ends with .css, .svg, etc.)
            let last_component = import.path.last().unwrap();
            if last_component.ends_with(".css") || last_component.ends_with(".svg") || last_component.ends_with(".dbv") || last_component.ends_with(".dbvs") || last_component.ends_with(".dbvl") {
                import.path.join("/")
            } else {
                // Use "/" directly — eliminates the ".join(".")" / ".replace('.', '/')" round trip.
                import.path.join("/")
            }
        };

        if let Some((cached, sed_names)) = self.loaded_modules.get(&path_str) {
            return self.filter_items(cached, sed_names, &import.items);
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
                    (Program {
                        items: vec![TopLevel::Stylesheet(css_for_cache)],
                        comments: vec![],
                        reactor_speed: None,
                        attrs: Vec::new(),
                        ffi: None,
                        strict_mode: self.strict_mode,
                        dispatch_mode: Default::default(),
                        exit_condition: None,
                        out_pragmas: vec![],
            default_sig_modifier: None,
                    }, vec![]),
                );
                return Ok(Program {
                    items: vec![TopLevel::Stylesheet(css_for_return)],
                    comments: vec![],
                    reactor_speed: None,
                    attrs: Vec::new(),
                    ffi: None,
                    strict_mode: self.strict_mode,
                    dispatch_mode: Default::default(),
                    exit_condition: None,
                    out_pragmas: vec![],
            default_sig_modifier: None,
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
                    (Program {
                        items: vec![TopLevel::SvgComponent {
                            name: component_name.clone(),
                            content: svg_for_cache,
                        }],
                        comments: vec![],
                        reactor_speed: None,
                        attrs: Vec::new(),
                        ffi: None,
                        strict_mode: self.strict_mode,
                        dispatch_mode: Default::default(),
                        exit_condition: None,
                        out_pragmas: vec![],
            default_sig_modifier: None,
                    }, vec![]),
                );
                return Ok(Program {
                    items: vec![TopLevel::SvgComponent {
                        name: component_name,
                        content: svg_for_return,
                    }],
                    comments: vec![],
                    reactor_speed: None,
                    attrs: Vec::new(),
                    ffi: None,
                    strict_mode: self.strict_mode,
                    dispatch_mode: Default::default(),
                    exit_condition: None,
                    out_pragmas: vec![],
            default_sig_modifier: None,
                });
            }
        }

        // Check for DBrief import (.dbv, .dbvl, .dbvs)
        if path_str.ends_with(".dbv") || path_str.ends_with(".dbvl") || path_str.ends_with(".dbvs") {
            let dbrief_src_dir = source_file
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."));

            // Search in source dir and all search paths
            let dbrief_path = self
                .search_paths
                .iter()
                .map(|p| dbrief_src_dir.join(p).join(&path_str))
                .chain(std::iter::once(dbrief_src_dir.join(&path_str)))
                .find(|p| p.exists())
                .ok_or_else(|| {
                    format!(
                        "DBrief file not found: {} (searched in lib/, imports/, ./ and source dir)",
                        path_str
                    )
                })?;

            let content = std::fs::read_to_string(&dbrief_path)
                .map_err(|e| format!("Failed to read DBrief file '{}': {}", dbrief_path.display(), e))?;

            let is_dbvl = path_str.ends_with(".dbvl");

            // For .dbvl files, use offset-tracking parser for lazy loading
            let doc = if is_dbvl {
                dbrief_v2::parse_document_track_offsets(&content)
            } else {
                dbrief_v2::parse_document(&content)
            }.map_err(|e| format!("Failed to parse DBrief file '{}': {}", dbrief_path.display(), e))?;

            // Determine the constant name from import items
            let constant_name = import
                .items
                .first()
                .map(|item| item.alias.as_ref().unwrap_or(&item.name).clone())
                .unwrap_or_else(|| {
                    // Fallback: use filename without extension
                    let fname = dbrief_path
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "data".to_string());
                    fname
                });

            let file_path_str = dbrief_path.to_string_lossy().to_string();

            // Generate program items, then fill in the path for lazy DbvlTable entries
            let mut dbrief_items = crate::dbrief::bridge::document_to_program_flags(
                &doc, &constant_name, is_dbvl,
            );

            // Fill in the file path for any lazy DbvlTable expressions
            let file_path_clone = file_path_str.clone();
            for item in &mut dbrief_items {
                if let crate::ast::TopLevel::Constant(c) = item {
                    if let crate::ast::Expr::MapLiteral(pairs) = &mut c.expr {
                        for (_, val) in pairs.iter_mut() {
                            val.set_dbvl_path(&file_path_clone);
                        }
                    } else {
                        c.expr.set_dbvl_path(&file_path_clone);
                    }
                }
            }

            let program_for_cache = Program {
                items: dbrief_items.clone(),
                comments: vec![],
                reactor_speed: None,
                attrs: Vec::new(),
                ffi: None,
                strict_mode: self.strict_mode,
                dispatch_mode: Default::default(),
                exit_condition: None,
                out_pragmas: vec![],
                default_sig_modifier: None,
            };

            self.loaded_modules.insert(
                path_str.clone(),
                (program_for_cache, vec![]),
            );

            return Ok(Program {
                items: dbrief_items,
                comments: vec![],
                reactor_speed: None,
                attrs: Vec::new(),
                ffi: None,
                strict_mode: self.strict_mode,
                dispatch_mode: Default::default(),
                exit_condition: None,
                out_pragmas: vec![],
                default_sig_modifier: None,
            });
        }

        // Default: Brief module (.bv or .ebv)
        let module_path = {
            if path_str.ends_with(".bv") {
                path_str[..path_str.len() - 3].replace('.', "/")
            } else if path_str.ends_with(".ebv") {
                path_str[..path_str.len() - 4].replace('.', "/")
            } else {
                path_str.replace('.', "/")
            }
        };
        // TypeScript-style import resolution:
        //   "./foo" or "../foo" → relative to importing file
        //   "foo/bar"           → relative to project root
        let is_relative = path_str.starts_with("./") || path_str.starts_with("../");
        let source_dir = if is_relative {
            source_file
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."))
        } else {
            self.root_path.clone()
        };

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

        // For std.* or std/ imports, also search from project root's lib/ directory
        if !found_both && found_path.is_none() && (path_str.starts_with("std.") || path_str.starts_with("std/")) {
            // Walk up from source_dir to find project root (where Cargo.toml exists)
            let mut current = source_dir.clone();
            while let Some(parent) = current.parent() {
                if parent.join("Cargo.toml").exists() {
                    let std_path = parent.join("lib").join(format!("{}.bv", module_path));
                    let std_ebv = parent.join("lib").join(format!("{}.ebv", module_path));
                    if std_path.exists() && std_ebv.exists() {
                        found_both = true;
                    } else if std_path.exists() {
                        found_path = Some(std_path);
                    } else if std_ebv.exists() {
                        found_path = Some(std_ebv);
                    }
                    break;
                }
                current = parent.to_path_buf();
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
            let dir = source_dir.display();
            format!(
                "Cannot find module '{}'. Searched in: \
                 {dir}/lib/{mp}.{{bv,ebv}}, \
                 {dir}/imports/{mp}.{{bv,ebv}}, \
                 {dir}/{mp}.{{bv,ebv}}",
                path_str,
                mp = module_path,
            )
        })?;

        let source = std::fs::read_to_string(&resolved_path)
            .map_err(|e| format!("Failed to read '{}': {}", resolved_path.display(), e))?;

        let mut parser = crate::parser::Parser::new(&source);
        let imported_program = parser
            .parse()
            .map_err(|e| format!("Failed to parse '{}': {}", resolved_path.display(), e))?;
        let sed_names = parser.take_sed_item_names();

        let resolved = self.resolve_imports(&imported_program, &resolved_path)?;

        // Cache the fully resolved program (with sub-imports processed),
        // not the parsed-only version. Otherwise cache hits return
        // programs whose internal imports were never resolved.
        self.loaded_modules
            .insert(path_str.clone(), (resolved.clone(), sed_names.clone()));

        self.filter_items(&resolved, &sed_names, &import.items)
    }

    fn filter_items(&self, program: &Program, sed_names: &[String], items: &[ImportItem]) -> Result<Program, String> {
        let item_names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();

        let filtered: Vec<TopLevel> = program
            .items
            .iter()
            .filter(|item| {
                if matches!(item, TopLevel::ForeignBinding { .. }) {
                    // Always include foreign declarations — they are zero-cost
                    // type declarations that a kept defn/txn body may reference.
                    return true;
                }
                if matches!(item, TopLevel::LinkDependency(_)) {
                    return true;
                }
                let name = match item {
                    TopLevel::Definition(d) => Some(d.name.as_str()),
                    TopLevel::Signature(s) => Some(s.name.as_str()),
                    TopLevel::ForeignBinding { name, .. } => Some(name.as_str()),
                    TopLevel::Transaction(t) => Some(t.name.as_str()),
                    TopLevel::Constant(c) => Some(c.name.as_str()),
                    TopLevel::Struct(s) => Some(s.name.as_str()),
                    TopLevel::RStruct(r) => Some(r.name.as_str()),
                    TopLevel::RenderBlock(rb) => Some(rb.struct_name.as_str()),
                    TopLevel::Trigger(t) => Some(t.name.as_str()),
                    TopLevel::StateDecl(s) => Some(s.name.as_str()),
                    _ => None,
                };
                let name = match name {
                    Some(n) => n,
                    None => return false,
                };
                // Filter out sed (file-private) items — they are not importable
                if sed_names.iter().any(|s| s == name) {
                    return false;
                }
                // If specific items are requested, only include matches
                if items.is_empty() {
                    true
                } else {
                    item_names.contains(&name)
                }
            })
            .cloned()
            .collect();

        Ok(Program {
            items: filtered,
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: self.strict_mode,
            dispatch_mode: Default::default(),
            exit_condition: None,
            out_pragmas: vec![],
            default_sig_modifier: None,
        })
    }
}

/// Keep only the first occurrence of each named top-level item.
/// When the same module is imported through multiple paths (direct + transitive),
/// its items appear multiple times in `items`. This dedup eliminates duplicates.
fn dedup_items(items: Vec<TopLevel>) -> Vec<TopLevel> {
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let mut result = Vec::with_capacity(items.len());
    for item in items {
        let key = match &item {
            TopLevel::Definition(d) => Some(("def", &d.name)),
            TopLevel::Transaction(t) => Some(("txn", &t.name)),
            TopLevel::StateDecl(s) => Some(("state", &s.name)),
            TopLevel::Trigger(t) => Some(("trigger", &t.name)),
            TopLevel::Constant(c) => Some(("const", &c.name)),
            TopLevel::Signature(s) => Some(("sig", &s.name)),
            TopLevel::ForeignBinding { name, .. } => Some(("frgn", name)),
            TopLevel::Struct(s) => Some(("struct", &s.name)),
            TopLevel::RStruct(r) => Some(("rstruct", &r.name)),
            TopLevel::Enum(e) => Some(("enum", &e.name)),
            TopLevel::TypeDef(t) => Some(("typedef", &t.name)),
            TopLevel::RenderBlock(r) => Some(("render", &r.struct_name)),
            TopLevel::LinkDependency(l) => Some(("link", &l.path)),
            TopLevel::ResourceDecl(r) => Some(("rsrc", &r.name)),
            _ => None,
        };
        match key {
            Some((cat, name)) if seen.insert((cat.to_string(), name.clone())) => result.push(item),
            Some(_) => {} // skip duplicate
            None => result.push(item), // keep unnamed items
        }
    }
    result
}

impl Default for ImportResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;
    use std::path::PathBuf;
    use std::fs;
    use tempfile::TempDir;

    fn import_program(path: Vec<&str>, items: Vec<ImportItem>) -> Program {
        Program {
            items: vec![TopLevel::Import(Import { path: path.iter().map(|s| s.to_string()).collect(), items })],
            comments: vec![], reactor_speed: None, attrs: vec![], ffi: None,
            strict_mode: StrictMode::Off, dispatch_mode: Default::default(),
            exit_condition: None, out_pragmas: vec![], default_sig_modifier: None,
        }
    }

    #[test]
    fn test_resolve_empty_import() {
        let prog = import_program(vec![], vec![]);
        let mut resolver = ImportResolver::new();
        let result = resolver.resolve_imports(&prog, &PathBuf::from("main.bv")).unwrap();
        assert_eq!(result.items.len(), 0);
    }

    #[test]
    fn test_resolve_bv_file() {
        let dir = TempDir::new().unwrap();
        let bv_path = dir.path().join("test_module.bv");
        fs::write(&bv_path, "defn hello -> Int { term 42; };").unwrap();

        let prog = import_program(vec!["test_module"], vec![]);
        let mut resolver = ImportResolver::new();
        resolver.add_search_path(dir.path().to_path_buf());
        let src = dir.path().join("main.bv");
        fs::write(&src, "").unwrap();
        let result = resolver.resolve_imports(&prog, &src).unwrap();
        let defns: Vec<&TopLevel> = result.items.iter().filter(|i| matches!(i, TopLevel::Definition(_))).collect();
        assert_eq!(defns.len(), 1);
    }

    #[test]
    fn test_resolve_checked_cached_modules() {
        let dir = TempDir::new().unwrap();
        let bv_path = dir.path().join("cache_test.bv");
        fs::write(&bv_path, "defn cached -> Int { term 1; };").unwrap();
        let src = dir.path().join("main.bv");
        fs::write(&src, "").unwrap();

        let mut resolver = ImportResolver::new();
        resolver.add_search_path(dir.path().to_path_buf());
        let prog = import_program(vec!["cache_test"], vec![]);
        resolver.resolve_imports(&prog, &src).unwrap();
        // The cache key is the path_str used in resolve_import
        assert!(resolver.loaded_modules.contains_key("cache_test"));
    }

    #[test]
    fn test_import_css_file() {
        let dir = TempDir::new().unwrap();
        let css_path = dir.path().join("styles.m.css");
        fs::write(&css_path, "body { color: red; }").unwrap();
        let src = dir.path().join("main.bv");
        fs::write(&src, "").unwrap();

        let prog = import_program(vec!["styles.m.css"], vec![]);
        let mut resolver = ImportResolver::new();
        resolver.add_search_path(dir.path().to_path_buf());
        let result = resolver.resolve_imports(&prog, &src).unwrap();
        assert!(result.items.iter().any(|i| matches!(i, TopLevel::Stylesheet(_))));
    }

    #[test]
    fn test_strict_mode_propagation() {
        let dir = TempDir::new().unwrap();
        let bv_path = dir.path().join("strict_test.bv");
        fs::write(&bv_path, "defn s -> Int { term 0; };").unwrap();
        let src = dir.path().join("main.bv");
        fs::write(&src, "").unwrap();

        let mut resolver = ImportResolver::new().with_strict_mode(true);
        resolver.add_search_path(dir.path().to_path_buf());
        let prog = import_program(vec!["strict_test"], vec![]);
        let result = resolver.resolve_imports(&prog, &src).unwrap();
        assert_eq!(result.strict_mode, StrictMode::Strict);
    }

    #[test]
    fn test_filter_items_by_name() {
        let dir = TempDir::new().unwrap();
        let bv_path = dir.path().join("filter_mod.bv");
        fs::write(&bv_path, "defn keep -> Int { term 1; };\ndefn discard -> Int { term 2; };").unwrap();
        let src = dir.path().join("main.bv");
        fs::write(&src, "").unwrap();

        let mut resolver = ImportResolver::new();
        resolver.add_search_path(dir.path().to_path_buf());
        let prog = import_program(vec!["filter_mod"], vec![ImportItem { name: "keep".into(), alias: None }]);
        let result = resolver.resolve_imports(&prog, &src).unwrap();
        let names: Vec<&str> = result.items.iter().filter_map(|i| match i {
            TopLevel::Definition(d) => Some(d.name.as_str()),
            _ => None,
        }).collect();
        assert_eq!(names, vec!["keep"]);
    }

    #[test]
    fn test_filter_items_empty() {
        let dir = TempDir::new().unwrap();
        let bv_path = dir.path().join("full_mod.bv");
        fs::write(&bv_path, "defn a -> Int { term 0; }; defn b -> Int { term 0; };").unwrap();
        let src = dir.path().join("main.bv");
        fs::write(&src, "").unwrap();

        let mut resolver = ImportResolver::new();
        resolver.add_search_path(dir.path().to_path_buf());
        let prog = import_program(vec!["full_mod"], vec![]);
        let result = resolver.resolve_imports(&prog, &src).unwrap();
        let count = result.items.iter().filter(|i| matches!(i, TopLevel::Definition(_))).count();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_resolve_module_not_found() {
        let prog = import_program(vec!["nonexistent_mod"], vec![]);
        let mut resolver = ImportResolver::new();
        let src = PathBuf::from("/tmp/main.bv");
        let result = resolver.resolve_imports(&prog, &src);
        assert!(result.is_err());
    }
}
