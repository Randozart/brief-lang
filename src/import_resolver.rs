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

use crate::ast::{Constant, Expr, Import, ImportItem, Program, StrictMode, TopLevel, Type};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
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
    stdlib_path: Option<PathBuf>,
    use_stdlib: bool,
    core_imported: bool,
    /// Board name for `import "target"` resolution (e.g., "stm32f407").
    board_name: Option<String>,
    // 2026-07-01: Cycle detection for import resolution.
    // Tracks path strings currently being resolved to detect A→B→A cycles.
    in_progress: HashSet<String>,
}

impl ImportResolver {
    pub fn new() -> Self {
        ImportResolver {
            loaded_modules: HashMap::new(),
            search_paths: vec![PathBuf::from("lib"), PathBuf::from("imports"), PathBuf::from(".")],
            strict_mode: StrictMode::Off,
            root_path: PathBuf::from("."),
            stdlib_path: None,
            use_stdlib: true,
            core_imported: false,
            board_name: None,
            in_progress: HashSet::new(),
        }
    }

    /// Set the board name for `import "target"` resolution.
    pub fn with_board(mut self, board: &str) -> Self {
        self.board_name = Some(board.to_string());
        self
    }

    /// Set strict mode for all resolved imports (propagated to all Program objects)
    pub fn with_strict_mode(mut self, strict: bool) -> Self {
        self.strict_mode = if strict { StrictMode::Strict } else { StrictMode::Off };
        self
    }

    /// Set the stdlib root path for import# resolution
    pub fn with_stdlib_path(mut self, path: Option<PathBuf>) -> Self {
        self.stdlib_path = path;
        self
    }

    /// Enable or disable auto-import of std/core/* (default: enabled)
    pub fn with_use_stdlib(mut self, use_it: bool) -> Self {
        self.use_stdlib = use_it;
        self
    }

    pub fn add_search_path(&mut self, path: PathBuf) {
        self.search_paths.push(path);
    }

    /// Resolve the stdlib root path, trying multiple sources in order:
    /// 1. Explicitly configured path (from --stdlib-path)
    /// 2. BRIEF_STDLIB_PATH env var
    /// 3. Executable-relative (dev layout: target/release/ -> ../../lib/)
    /// 4. root_path/lib/ (project-local)
    fn resolve_stdlib_root(&self) -> Option<PathBuf> {
        // 1. Explicitly configured
        if let Some(ref path) = self.stdlib_path {
            if path.exists() {
                return Some(path.clone());
            }
        }

        // 2. Environment variable
        if let Ok(env_path) = std::env::var("BRIEF_STDLIB_PATH") {
            let p = PathBuf::from(env_path);
            if p.exists() {
                return Some(p);
            }
        }

        // 3. Executable-relative (dev layout)
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                // Development: brief-compiler/target/release/ -> ../../lib/
                let dev_p = exe_dir.join("../../lib/");
                if dev_p.exists() {
                    return Some(dev_p);
                }
                // Alternate: brief-compiler/target/debug/ -> ../../lib/
                let debug_p = exe_dir.join("../../lib/");
                if debug_p.exists() {
                    return Some(debug_p);
                }
                // Installed: ~/.local/bin/ -> ~/.local/share/brief/
                let installed_p = exe_dir.join("../share/brief/");
                if installed_p.join("std/core").exists() {
                    return Some(installed_p);
                }
            }
        }

        // 4. Project-local lib/
        let local = self.root_path.join("lib");
        if local.exists() {
            return Some(local);
        }

        None
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

        // Auto-import known-safe std/core modules once
        // NOTE: Some stdlib files use `uni`/`<-` syntax that the current
        // Rust parser doesn't support (pre-existing limitation). Only
        // files that parse correctly are included here.
        if self.use_stdlib && !self.core_imported {
            self.core_imported = true;

            // Auto-import the bootstrap type universe (14 primitive types)
            let has_bootstrap_import = items.iter().any(|item| {
                if let TopLevel::Import(imp) = item {
                    imp.is_magic
                        && imp.path.len() >= 3
                        && imp.path[0] == "std"
                        && imp.path[1] == "types"
                        && imp.path[2] == "bootstrap.bv"
                } else {
                    false
                }
            });
            if !has_bootstrap_import {
                items.insert(0, TopLevel::Import(Import {
                    is_magic: true,
                    path: vec!["std".to_string(), "types".to_string(), "bootstrap.bv".to_string()],
                    items: vec![],
                    target: crate::ast::ImportTarget::Native,
                }));
            }

            // 2026-07-08: Phase 3 — auto-import OS module prelude
            // 2026-07-08: Phase 3 — auto-import OS module prelude
            // inop declarations call brief_rt.c's brief_* functions via the preamble.
            let prelude_modules = [
                "std/os/fs.bv", "std/os/net.bv", "std/os/signal.bv",
                "std/os/ipc.bv", "std/os/thread.bv", "std/os/dir.bv",
                "std/os/process.bv", "std/os/tty.bv", "std/os/user.bv",
                "std/os/time.bv", "std/os/mem.bv", "std/os/rand.bv",
                "std/os/sched.bv", "std/os/resource.bv", "std/os/sysinfo.bv",
                "std/os/temp.bv", "std/os/dynlib.bv", "std/os/debug.bv",
                "std/os/ring.bv", "std/os/io.bv",
                // atomic.bv excluded — atomics need LLVM IR, not C calls
            ];
            for module_path in &prelude_modules {
                let has_import = items.iter().any(|item| {
                    if let TopLevel::Import(imp) = item {
                        imp.is_magic && imp.path.iter()
                            .cloned().collect::<Vec<_>>().join("/") == *module_path
                    } else {
                        false
                    }
                });
                if !has_import {
                    let path_parts: Vec<String> = module_path.split('/').map(|s| s.to_string()).collect();
                    items.push(TopLevel::Import(Import {
                        is_magic: true,
                        path: path_parts,
                        items: vec![],
                        target: crate::ast::ImportTarget::Native,
                    }));
                }
            }

            let has_core_imports = items.iter().any(|item| {
                if let TopLevel::Import(imp) = item {
                    imp.is_magic
                        && imp.path.len() >= 2
                        && imp.path[0] == "std"
                        && imp.path[1] == "core"
                } else {
                    false
                }
            });
            if !has_core_imports {
                // Files that parse correctly with the current Rust parser
                // Only files that BOTH parse AND pass the TypeChecker without errors.
                // Other core files (bits, char, collections, hashmap, hashset, etc.)
                // use features the Rust TypeChecker doesn't fully support yet.
                let safe_core_modules = [
                    "ptr.bv",
                    "string_builder.bv",
                ];
                for module in safe_core_modules {
                    items.insert(0, TopLevel::Import(Import {
                        is_magic: true,
                        path: vec!["std".to_string(), "core".to_string(), module.to_string()],
                        items: vec![],
                        target: crate::ast::ImportTarget::Native,
                    }));
                }
            }
        }

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
            default_sig_modifier: program.default_sig_modifier.clone(),
                watchdog_defaults: (None, None),
        })
    }

    /// Resolve `import "target"` — loads the board D-brief description and emits typed constants.
    fn resolve_target_import(&mut self) -> Result<Program, String> {
        let board = self.board_name.as_deref().unwrap_or("stm32f407");
        let file_name = format!("{}.dbvl", board);

        let file_path = self.search_paths.iter()
            .map(|p| p.join("boards").join(&file_name))
            .chain(std::iter::once(PathBuf::from(&file_name)))
            .find(|p| p.exists());

        let path = match file_path {
            Some(p) => p,
            None => return Err(format!(
                "Board file 'lib/boards/{}.dbvl' not found. Use --board <name> or create a board file.",
                board
            )),
        };

        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read '{}': {}", path.display(), e))?;

        // Parse the .dbvl board file using D-brief v2 parser
        let mut doc = crate::dbrief::v2::parse_document(&content)
            .map_err(|e| format!("Failed to parse '{}': {}", path.display(), e))?;

        // Resolve schema imports (schema <path>; directives)
        let mut resolved_imports = Vec::new();
        for import_path in &doc.imports {
            let schema_path = self.search_paths.iter()
                .map(|p| p.join(&import_path))
                .chain(std::iter::once(PathBuf::from(&import_path)))
                .find(|p| p.exists());

            if let Some(sp) = schema_path {
                if let Ok(schema_content) = std::fs::read_to_string(&sp) {
                    if let Ok(schema_doc) = crate::dbrief::v2::parse_document(&schema_content) {
                        doc.schemas.extend(schema_doc.schemas);
                        resolved_imports.push(import_path.clone());
                    }
                }
            }
        }
        // Keep only unresolved imports in doc.imports (resolved ones are merged into schemas)
        doc.imports.retain(|i| !resolved_imports.contains(i));

        // Bridge to TopLevel constants via the standard D-brief bridge
        let items = crate::dbrief::bridge::document_to_program(&doc, &board);

        Ok(Program {
            items,
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: self.strict_mode,
            dispatch_mode: Default::default(),
            exit_condition: None,
            out_pragmas: vec![],
            default_sig_modifier: None,
            watchdog_defaults: (None, None),
        })
    }

    fn resolve_import(
        &mut self,
        import: &Import,
        source_file: &PathBuf,
    ) -> Result<Program, String> {
        // Skip .dbvs schema imports - they're handled by schema validation, not as Brief modules
        if import.path.is_empty() {
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
                watchdog_defaults: (None, None),
            });
        }

        // Handle `import "target"` — board-level device description
        if import.path.len() == 1 && import.path[0] == "target" {
            return self.resolve_target_import();
        }

        // Handle glob expansion (* or ** in last path segment)
        if let Some(last) = import.path.last() {
            if last == "*" || last == "**" {
                return self.resolve_glob(import, source_file);
            }
        }

        // Magic imports (import#) resolve against stdlib path, not project paths
        if import.is_magic {
            return self.resolve_magic_import(import, source_file);
        }

        // Non-magic imports: build path string for resolution
        let path_str = {
            let last_component = import.path.last().unwrap();
            if last_component.ends_with(".css") || last_component.ends_with(".svg") || last_component.ends_with(".dbv") || last_component.ends_with(".dbvs") || last_component.ends_with(".dbvl") {
                import.path.join("/")
            } else {
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
                watchdog_defaults: (None, None),
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
                watchdog_defaults: (None, None),
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
                watchdog_defaults: (None, None),
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
                watchdog_defaults: (None, None),
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
                    watchdog_defaults: (None, None),
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
                    watchdog_defaults: (None, None),
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

        // 2026-07-01: Cycle detection — if path_str is already in in_progress,
        // we have a circular import (A imports B, B imports A).
        if !self.in_progress.insert(path_str.clone()) {
            return Err(format!(
                "Circular import detected: '{}' is already being resolved \
                 (direct or transitive self-import).",
                path_str
            ));
        }

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

        // Remove from in_progress now that resolution is complete.
        // Must do this before returning, since filter_items is the return expr.
        let result = self.filter_items(&resolved, &sed_names, &import.items);
        self.in_progress.remove(&path_str);
        result
    }

    /// Resolve a magic import (import#) — resolves the path relative to the stdlib root.
    fn resolve_magic_import(
        &mut self,
        import: &Import,
        _source_file: &PathBuf,
    ) -> Result<Program, String> {
        let stdlib_root = self.resolve_stdlib_root().ok_or_else(|| {
            format!(
                "Cannot resolve import# '{}': no stdlib path configured. \
                 Use --stdlib-path or set BRIEF_STDLIB_PATH.",
                import.path.join("/")
            )
        })?;

        let relative_path: PathBuf = import.path.iter().collect();
        let full_path = stdlib_root.join(&relative_path);

        // Try with .bv extension if the path doesn't have one
        let candidate = if full_path.extension().is_some() {
            full_path.clone()
        } else {
            full_path.with_extension("bv")
        };

        // Use a distinct cache key for magic imports to avoid collisions
        let cache_key = format!("magic:{}", import.path.join("/"));
        if let Some((cached, sed_names)) = self.loaded_modules.get(&cache_key) {
            return self.filter_items(cached, sed_names, &import.items);
        }

        if !candidate.exists() {
            return Err(format!(
                "Cannot find module '{}' at stdlib path: {}",
                import.path.join("/"),
                candidate.display()
            ));
        }

        let source = std::fs::read_to_string(&candidate)
            .map_err(|e| format!("Failed to read '{}': {}", candidate.display(), e))?;

        let mut parser = crate::parser::Parser::new(&source);
        let imported_program = parser
            .parse()
            .map_err(|e| format!("Failed to parse '{}': {}", candidate.display(), e))?;
        let sed_names = parser.take_sed_item_names();

        let resolved = self.resolve_imports(&imported_program, &candidate)?;

        self.loaded_modules
            .insert(cache_key, (resolved.clone(), sed_names.clone()));

        self.filter_items(&resolved, &sed_names, &import.items)
    }

    /// Expand a glob pattern (* or **) into individual Import nodes.
    /// *  — all .bv files in the matched directory
    /// ** — all .bv files recursively from the matched directory
    fn resolve_glob(
        &mut self,
        import: &Import,
        source_file: &PathBuf,
    ) -> Result<Program, String> {
        let is_recursive = import.path.last().map(|s| s == "**").unwrap_or(false);

        // Determine base directory
        let base_dir = if import.is_magic {
            let stdlib_root = self.resolve_stdlib_root().ok_or_else(|| {
                format!(
                    "Cannot resolve import# glob '{}': no stdlib path configured.",
                    import.path.join("/")
                )
            })?;
            let path_prefix: PathBuf = import.path[..import.path.len() - 1].iter().collect();
            stdlib_root.join(path_prefix)
        } else {
            // Non-magic glob: resolve relative to project search paths
            let module_path = import.path[..import.path.len() - 1].join("/");
            let is_relative = module_path.starts_with("./") || module_path.starts_with("../");
            let source_dir = if is_relative {
                source_file
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| PathBuf::from("."))
            } else {
                self.root_path.clone()
            };
            let mut found = None;
            for search_dir in &self.search_paths {
                let candidate = source_dir.join(search_dir).join(&module_path);
                if candidate.exists() {
                    found = Some(candidate);
                    break;
                }
            }
            found.unwrap_or_else(|| source_dir.join(&module_path))
        };

        // Collect .bv files
        let mut entries: Vec<PathBuf> = Vec::new();
        if is_recursive {
            collect_bv_files_recursive(&base_dir, &mut entries)
                .map_err(|e| format!("Error reading directory '{}': {}", base_dir.display(), e))?;
        } else {
            if let Ok(rd) = std::fs::read_dir(&base_dir) {
                for entry in rd.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.is_file()
                        && path.extension().map(|ext| ext == "bv").unwrap_or(false)
                    {
                        entries.push(path);
                    }
                }
            }
        }
        entries.sort();

        // Generate wildcard Import nodes for each file
        let path_prefix: Vec<String> = import.path[..import.path.len() - 1].to_vec();
        let items: Vec<TopLevel> = entries
            .into_iter()
            .map(|path| {
                let rel_path = path
                    .strip_prefix(&base_dir)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();
                let mut path_components = path_prefix.clone();
                path_components.extend(rel_path.split('/').map(|s| s.to_string()));
                TopLevel::Import(Import {
                    is_magic: import.is_magic,
                    path: path_components,
                    items: vec![],
                    target: import.target.clone(),
                })
            })
            .collect();

        Ok(Program {
            items,
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: self.strict_mode,
            dispatch_mode: Default::default(),
            exit_condition: None,
            out_pragmas: vec![],
            default_sig_modifier: None,
                watchdog_defaults: (None, None),
        })
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
                if matches!(item, TopLevel::Inop(_)) {
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
                    TopLevel::TriggerBinding { name, .. } => Some(name.as_str()),
                    TopLevel::Cell(c) => Some(c.name.as_str()),
                    TopLevel::StateDecl(s) => Some(s.name.as_str()),
                    TopLevel::Inop(i) => Some(i.name.as_str()),
                    TopLevel::TypeDef(t) => Some(t.name.as_str()),
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
                watchdog_defaults: (None, None),
        })
    }
}

/// Recursively collect all .bv files under a directory.
fn collect_bv_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for entry in rd {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                collect_bv_files_recursive(&path, files)?;
            } else if path.extension().map(|ext| ext == "bv").unwrap_or(false) {
                files.push(path);
            }
        }
    }
    Ok(())
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
            TopLevel::TriggerBinding { name, .. } => Some(("trg_binding", name)),
            TopLevel::Cell(c) => Some(("cell", &c.name)),
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
            TopLevel::Inop(i) => Some(("inop", &i.name)),
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
            items: vec![TopLevel::Import(Import { is_magic: false, target: crate::ast::ImportTarget::Native, path: path.iter().map(|s| s.to_string()).collect(), items })],
            comments: vec![], reactor_speed: None, attrs: vec![], ffi: None,
            strict_mode: StrictMode::Off, dispatch_mode: Default::default(),
            exit_condition: None, out_pragmas: vec![], default_sig_modifier: None, watchdog_defaults: (None, None),
        }
    }

    #[test]
    fn test_resolve_empty_import() {
        let prog = import_program(vec![], vec![]);
        let mut resolver = ImportResolver::new().with_use_stdlib(false);
        let result = resolver.resolve_imports(&prog, &PathBuf::from("main.bv")).unwrap();
        assert_eq!(result.items.len(), 0);
    }

    #[test]
    fn test_resolve_bv_file() {
        let dir = TempDir::new().unwrap();
        let bv_path = dir.path().join("test_module.bv");
        fs::write(&bv_path, "defn hello -> Int { term 42; };").unwrap();

        let prog = import_program(vec!["test_module"], vec![]);
        let mut resolver = ImportResolver::new().with_use_stdlib(false);
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

        let mut resolver = ImportResolver::new().with_use_stdlib(false);
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
        let mut resolver = ImportResolver::new().with_use_stdlib(false);
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

        let mut resolver = ImportResolver::new().with_strict_mode(true).with_use_stdlib(false);
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

        let mut resolver = ImportResolver::new().with_use_stdlib(false);
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

        let mut resolver = ImportResolver::new().with_use_stdlib(false);
        resolver.add_search_path(dir.path().to_path_buf());
        let prog = import_program(vec!["full_mod"], vec![]);
        let result = resolver.resolve_imports(&prog, &src).unwrap();
        let count = result.items.iter().filter(|i| matches!(i, TopLevel::Definition(_))).count();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_resolve_module_not_found() {
        let prog = import_program(vec!["nonexistent_mod"], vec![]);
        let mut resolver = ImportResolver::new().with_use_stdlib(false);
        let src = PathBuf::from("/tmp/main.bv");
        let result = resolver.resolve_imports(&prog, &src);
        assert!(result.is_err());
    }

    #[test]
    fn test_magic_import_resolution() {
        let dir = TempDir::new().unwrap();
        let stdlib_root = dir.path().join("lib");
        let core_dir = stdlib_root.join("std").join("core");
        fs::create_dir_all(&core_dir).unwrap();
        fs::write(core_dir.join("test_magic.bv"), "defn greet -> String { term \"hello\"; };").unwrap();
        let src = dir.path().join("main.bv");
        fs::write(&src, "").unwrap();

        let prog = import_program(vec!["std", "core", "test_magic.bv"], vec![]);
        // Mark as magic by converting to Import with is_magic: true
        let prog = Program {
            items: vec![TopLevel::Import(Import {
                is_magic: true,
                path: vec!["std".to_string(), "core".to_string(), "test_magic.bv".to_string()],
                items: vec![],
                target: crate::ast::ImportTarget::Native,
            })],
            ..prog
        };
        let mut resolver = ImportResolver::new()
            .with_use_stdlib(false)
            .with_stdlib_path(Some(stdlib_root));
        let result = resolver.resolve_imports(&prog, &src).unwrap();
        let defns: Vec<&TopLevel> = result.items.iter().filter(|i| matches!(i, TopLevel::Definition(_))).collect();
        assert_eq!(defns.len(), 1);
        if let Some(TopLevel::Definition(d)) = defns.first() {
            assert_eq!(d.name, "greet");
        }
    }

    #[test]
    fn test_glob_import_non_recursive() {
        let dir = TempDir::new().unwrap();
        let stdlib_root = dir.path().join("lib");
        let core_dir = stdlib_root.join("std").join("core");
        fs::create_dir_all(&core_dir).unwrap();
        fs::write(core_dir.join("a.bv"), "defn a_fn -> Int { term 1; };").unwrap();
        fs::write(core_dir.join("b.bv"), "defn b_fn -> Int { term 2; };").unwrap();
        // Should not be picked up by non-recursive glob
        let sub_dir = core_dir.join("sub");
        fs::create_dir_all(&sub_dir).unwrap();
        fs::write(sub_dir.join("c.bv"), "defn c_fn -> Int { term 3; };").unwrap();
        let src = dir.path().join("main.bv");
        fs::write(&src, "").unwrap();

        let prog = import_program(vec!["std", "core", "*"], vec![]);
        let prog = Program {
            items: vec![TopLevel::Import(Import {
                is_magic: true,
                path: vec!["std".to_string(), "core".to_string(), "*".to_string()],
                items: vec![],
                target: crate::ast::ImportTarget::Native,
            })],
            ..prog
        };
        let mut resolver = ImportResolver::new()
            .with_use_stdlib(false)
            .with_stdlib_path(Some(stdlib_root));
        let result = resolver.resolve_imports(&prog, &src).unwrap();
        let defns: Vec<&TopLevel> = result.items.iter().filter(|i| matches!(i, TopLevel::Definition(_))).collect();
        assert_eq!(defns.len(), 2, "non-recursive glob should pick up a.bv and b.bv, but not sub/c.bv");
    }

    #[test]
    fn test_auto_core_injection() {
        let dir = TempDir::new().unwrap();
        let stdlib_root = dir.path().join("lib");
        let core_dir = stdlib_root.join("std").join("core");
        let types_dir = stdlib_root.join("std").join("types");
        fs::create_dir_all(&core_dir).unwrap();
        fs::create_dir_all(&types_dir).unwrap();
        fs::write(core_dir.join("ptr.bv"), "defn p_fn -> Int { term 0; };").unwrap();
        fs::write(core_dir.join("string_builder.bv"), "defn s_fn -> Int { term 0; };").unwrap();
        // Bootstrap types needed for auto-import of bootstrap.bv prelude
        fs::write(types_dir.join("bootstrap.bv"), "type Int <: Bits { bytes <~ 8; }; type Float <: Bits { bytes <~ 4; };").unwrap();
        // Create std/os/ directory for prelude auto-import
        let os_dir = stdlib_root.join("std").join("os");
        fs::create_dir_all(&os_dir).unwrap();
        for module in &["fs.bv", "net.bv", "signal.bv", "ipc.bv", "thread.bv", "dir.bv",
                        "process.bv", "tty.bv", "user.bv", "time.bv", "mem.bv", "rand.bv",
                        "sched.bv", "resource.bv", "sysinfo.bv", "temp.bv", "dynlib.bv",
                        "debug.bv", "ring.bv", "atomic.bv", "io.bv"] {
            fs::write(os_dir.join(module), "").unwrap();
        }
        let src = dir.path().join("main.bv");
        fs::write(&src, "").unwrap();

        let prog = import_program(vec![], vec![]);
        let mut resolver = ImportResolver::new()
            .with_use_stdlib(true)
            .with_stdlib_path(Some(stdlib_root));
        let result = resolver.resolve_imports(&prog, &src).unwrap();
        // Auto-core injection resolves imports, so the result should contain
        // the defn from ptr.bv but no remaining Import items
        let defns: Vec<&TopLevel> = result.items.iter().filter(|i| matches!(i, TopLevel::Definition(_))).collect();
        assert!(!defns.is_empty(), "should have auto-injected definitions");
        if let Some(TopLevel::Definition(d)) = defns.first() {
            assert_eq!(d.name, "s_fn", "string_builder.bv injected first (last in safe_core_modules)");
        }
    }

    #[test]
    fn test_auto_core_disabled() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("main.bv");
        fs::write(&src, "").unwrap();

        let prog = import_program(vec![], vec![]);
        let mut resolver = ImportResolver::new()
            .with_use_stdlib(false);
        let result = resolver.resolve_imports(&prog, &src).unwrap();
        let imports: Vec<&TopLevel> = result.items.iter().filter(|i| matches!(i, TopLevel::Import(_))).collect();
        assert!(imports.is_empty(), "no imports should be injected when use_stdlib is false");
    }
}
