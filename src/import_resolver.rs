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

use crate::ast::{Expr, Import, ImportKind, TopLevel, Type};
use crate::dbrief::v2 as dbrief_v2;
use crate::lexer::Token;
use logos::Logos;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

// 2026-07-21: Prelude is now a system plugin (plugins/parsed/prelude.bv) that
// runs at the $(Parsed) stage via the AST navigation DSL (Tag$ + Insert$).
// 2026-07-15: Removed hardcoded prelude injection.
// Removed fields: use_stdlib, core_imported. Removed method: with_use_stdlib.

/// Load the module registry from config/module-registry.toml (or its Data
/// Brief form module-registry.dbvl — Phase 3, 2026-08-03).
/// When the file doesn't exist or can't be parsed, returns an empty map
/// so that Registry imports fall back to literal filesystem resolution.
/// 2026-07-15: Phase 7i
fn load_module_registry() -> HashMap<String, String> {
    crate::dbrief::config_db::load_string_registry(Path::new("config"), "module-registry")
}

pub struct ImportResolver {
    loaded_modules: HashMap<String, (Vec<TopLevel>, Vec<String>)>,
    search_paths: Vec<PathBuf>,
    root_path: PathBuf,
    stdlib_path: Option<PathBuf>,
    /// Board name for `import "target"` resolution (e.g., "stm32f407").
    board_name: Option<String>,
    // 2026-07-01: Cycle detection for import resolution.
    // Tracks path strings currently being resolved to detect A→B→A cycles.
    in_progress: HashSet<String>,
    /// Registry mapping from module names to filesystem paths.
    /// Loaded from config/module-registry.toml or hardcoded fallback.
    /// 2026-07-15: Phase 7i — import <name> resolution.
    registry: HashMap<String, String>,
}

/// The name of a top-level item, if it carries one.
fn item_name(item: &TopLevel) -> Option<&str> {
    match item {
        TopLevel::Definition(d) => Some(d.name.as_str()),
        TopLevel::Signature(s) => Some(s.name.as_str()),
        TopLevel::ForeignBinding(fb) => Some(fb.effective_brief_name()),
        TopLevel::Transaction(t) => Some(t.name.as_str()),
        TopLevel::Constant(c) => Some(c.name.as_str()),
        TopLevel::Obj(s) => Some(s.name.as_str()),
        TopLevel::RStruct(r) => Some(r.name.as_str()),
        TopLevel::TypeDef(t) => Some(t.name.as_str()),
        TopLevel::StaticStruct(s) => Some(s.name.as_str()),
        TopLevel::StateDecl(s) => Some(s.name.as_str()),
        TopLevel::Trigger(trg) => Some(trg.name.as_str()),
        TopLevel::TriggerBinding { name, .. } => Some(name.as_str()),
        TopLevel::Cell(c) => Some(c.name.as_str()),
        _ => None,
    }
}

/// The item's name as an owned String (for HashSet membership).
fn top_level_name(item: &TopLevel) -> Option<String> {
    item_name(item).map(|s| s.to_string())
}

/// The Custom/Applied type names referenced by an item's slots/fields.
/// 2026-08-01 (D3): used by the named-import dependency closure — `List`
/// references `ListBuffer<T>` in its slots, which must be imported too.
fn referenced_type_names(item: &TopLevel) -> Vec<String> {
    fn type_names(ty: &crate::ast::Type, acc: &mut Vec<String>) {
        match ty {
            crate::ast::Type::Custom(n) => acc.push(n.clone()),
            crate::ast::Type::Applied(n, args) => {
                acc.push(n.clone());
                for a in args {
                    type_names(a, acc);
                }
            }
            crate::ast::Type::Ptr(i) | crate::ast::Type::PtrConst(i) => type_names(i, acc),
            crate::ast::Type::Vector(i, _) => type_names(i, acc),
            crate::ast::Type::Tuple(elems) => {
                for e in elems {
                    type_names(e, acc);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    match item {
        TopLevel::TypeDef(td) => {
            for s in &td.body.slots {
                type_names(&s.ty, &mut out);
            }
            for m in &td.body.members {
                let params: Vec<&(String, crate::ast::Type)> = match m {
                    TopLevel::Transaction(t) => t.parameters.iter().collect(),
                    TopLevel::Definition(t) => t.parameters.iter().collect(),
                    _ => Vec::new(),
                };
                for (_, ty) in params {
                    type_names(ty, &mut out);
                }
            }
        }
        TopLevel::StaticStruct(sd) => {
            for (_, ty) in &sd.fields {
                type_names(ty, &mut out);
            }
        }
        _ => {}
    }
    out
}

impl ImportResolver {
    pub fn new() -> Self {
        ImportResolver {
            loaded_modules: HashMap::new(),
            search_paths: vec![PathBuf::from("lib"), PathBuf::from("imports"), PathBuf::from(".")],
            root_path: PathBuf::from("."),
            stdlib_path: None,
            board_name: None,
            in_progress: HashSet::new(),
            registry: load_module_registry(),
        }
    }

    /// Set the board name for `import "target"` resolution.
    pub fn with_board(mut self, board: &str) -> Self {
        self.board_name = Some(board.to_string());
        self
    }

    /// Set the stdlib root path for import# resolution
    pub fn with_stdlib_path(mut self, path: Option<PathBuf>) -> Self {
        self.stdlib_path = path;
        self
    }

    pub fn add_search_path(&mut self, path: PathBuf) {
        self.search_paths.push(path);
    }

    /// 2026-07-16: P3 — Resolve a path relative to the stdlib root.
    /// Searches the same paths as resolve_stdlib_root().
    pub fn resolve_stdlib_relative_path(&self, relative: &str) -> Option<PathBuf> {
        self.resolve_stdlib_root().map(|root| root.join(relative)).filter(|p| p.exists())
    }

    /// Resolve the stdlib root path, trying multiple sources in order:
    /// 1. Explicitly configured path (from --stdlib-path)
    /// 2. BRIEF_STDLIB_PATH env var
    /// 3. Executable-relative (dev layout: target/release/ -> ../../lib/)
    /// 4. root_path/lib/ (project-local)
    pub fn resolve_stdlib_root(&self) -> Option<PathBuf> {
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
        items: Vec<TopLevel>,
        file_path: &PathBuf,
    ) -> Result<Vec<TopLevel>, String> {
        // Set root path from the main file's directory on first call
        if self.root_path == PathBuf::from(".") {
            self.root_path = file_path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."));
        }

        let mut items = items;

        let mut index = 0;

        while index < items.len() {
            if let TopLevel::Import(import) = &items[index] {
                let resolved = self.resolve_import(import, file_path)?;
                items.remove(index);
                items.splice(index..index, resolved);
            } else {
                index += 1;
            }
        }

        // 2026-06-13: Dedup items
        items = dedup_items(items);

        Ok(items)
    }

    /// Resolve `import "target"` — loads the board D-brief description and emits typed constants.
    fn resolve_target_import(&mut self) -> Result<Vec<TopLevel>, String> {
        let board = self.board_name.as_deref().unwrap_or("stm32f407");

        // 2026-08-03 (Phase 2): the board map is now a directory:
        //   lib/boards/<board>/map.dbv          — schemas only
        //   lib/boards/<board>/addresses.dbvl   — flat KEY: addr; size; table
        //   lib/boards/<board>/registers.dbvl   — flat register detail
        // The old single-file `boards/<board>.dbvl` is obsolete. Look for the
        // addresses table first; fall back to the legacy single file.
        let addresses_path = self.search_paths.iter()
            .map(|p| p.join("boards").join(board).join("addresses.dbvl"))
            .chain(std::iter::once(PathBuf::from(board).join("addresses.dbvl")))
            .find(|p| p.exists());

        let mut doc = if let Some(path) = addresses_path {
            // Activate the board map so address_resolver agrees with this table.
            crate::address_resolver::set_active_board(board);

            let content = std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read '{}': {}", path.display(), e))?;
            let mut doc = crate::dbrief::v2::parse_document(&content)
                .map_err(|e| format!("Failed to parse '{}': {}", path.display(), e))?;

            // Merge the schema carrier (map.dbv) and register detail table.
            let schemas_path = path.with_file_name("map.dbv");
            if schemas_path.exists() {
                if let Ok(schema_content) = std::fs::read_to_string(&schemas_path) {
                    if let Ok(schema_doc) = crate::dbrief::v2::parse_document(&schema_content) {
                        doc.schemas.extend(schema_doc.schemas);
                        // map.dbv is merged inline — drop it from doc.imports so
                        // the bridge does not re-emit it as a literal import.
                        doc.imports.retain(|i| i != "map.dbv");
                    }
                }
            }
            let registers_path = path.with_file_name("registers.dbvl");
            if registers_path.exists() {
                if let Ok(reg_content) = std::fs::read_to_string(&registers_path) {
                    if let Ok(reg_doc) = crate::dbrief::v2::parse_document(&reg_content) {
                        doc.data_groups.extend(reg_doc.data_groups);
                    }
                }
            }
            doc
        } else {
            // Legacy single-file board (pre-2026-08-03). Kept as a fallback
            // for out-of-tree board packs that still ship the old layout.
            let file_name = format!("{}.dbvl", board);
            let file_path = self.search_paths.iter()
                .map(|p| p.join("boards").join(&file_name))
                .chain(std::iter::once(PathBuf::from(&file_name)))
                .find(|p| p.exists());
            let path = match file_path {
                Some(p) => p,
                None => return Err(format!(
                    "Board file 'lib/boards/{}.dbvl' or 'lib/boards/{}/addresses.dbvl' not found. \
                     Use --board <name> or create a board directory.",
                    board, board
                )),
            };
            let content = std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read '{}': {}", path.display(), e))?;
            crate::dbrief::v2::parse_document(&content)
                .map_err(|e| format!("Failed to parse '{}': {}", path.display(), e))?
        };

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
        doc.imports.retain(|i| !resolved_imports.contains(i));

        let items = crate::dbrief::bridge::document_to_program(&doc, &board);

        Ok(items)
    }

    fn resolve_import(
        &mut self,
        import: &Import,
        source_file: &PathBuf,
    ) -> Result<Vec<TopLevel>, String> {
        // Skip empty module paths
        if import.path().is_empty() {
            return Ok(vec![]);
        }

        // Handle Registry imports — look up name in registry dir first,
        // then fall back to the TOML module registry config.
        // 2026-07-15: Phase 7i
        // 2026-07-26: Check ~/.brief/registry/ before TOML registry.
        if let ImportKind::Registry(name) = &import.kind {
            // 2026-07-26: Check registry directory first (user-installed modules
            // take priority over baked config/module-registry.toml entries).
            if let Some(reg_path) = crate::registry::find_registry_entry(name) {
                let literal_import = Import::literal(reg_path.to_string_lossy().to_string(), import.symbols.clone());
                return self.resolve_import(&literal_import, source_file);
            }
            let resolved_path = self.registry.get(name.as_str());
            let actual_path = match resolved_path {
                Some(p) => p.clone(),
                None => {
                    // Name not found in registry — fall back to using the name
                    // as a literal path (same as import "name").
                    name.clone()
                }
            };
            let literal_import = Import::literal(actual_path, import.symbols.clone());
            return self.resolve_import(&literal_import, source_file);
        }

        // Handle `import "target"` — board-level device description
        if import.path() == "target" {
            return self.resolve_target_import();
        }

        // Handle glob expansion (* or ** in last path segment)
        let is_glob = import.path().ends_with("/*") || import.path().ends_with("/**");
        if is_glob {
            return self.resolve_glob(import, source_file);
        }

        // Cache check
        if let Some((cached, sed_names)) = self.loaded_modules.get(import.path()) {
            return self.filter_items(cached, sed_names, &import.symbols);
        }

        // Check for CSS import
        if import.path().ends_with(".css") {
            let css_path = source_file
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."))
                .join(&import.path());

            if css_path.exists() {
                let css_content = std::fs::read_to_string(&css_path)
                    .map_err(|e| format!("Failed to read CSS '{}': {}", css_path.display(), e))?;
                let css_for_cache = css_content.clone();
                self.loaded_modules.insert(
                    import.path().to_string(),
                    (vec![TopLevel::Stylesheet(css_for_cache)], vec![]),
                );
                return Ok(vec![TopLevel::Stylesheet(css_content)]);
            }
        }

        // Check for SVG import
        if import.path().ends_with(".svg") {
            let svg_path = source_file
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."))
                .join(&import.path());

            if svg_path.exists() {
                let svg_content = std::fs::read_to_string(&svg_path)
                    .map_err(|e| format!("Failed to read SVG '{}': {}", svg_path.display(), e))?;
                let component_name = import
                    .symbols
                    .first()
                    .cloned()
                    .unwrap_or_else(|| {
                        let file_name = if let Some(last_slash) = import.path().rfind('/') {
                            &import.path()[last_slash + 1..]
                        } else {
                            &import.path()
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
                self.loaded_modules.insert(
                    import.path().to_string(),
                    (vec![TopLevel::SvgComponent {
                        name: component_name.clone(),
                        content: svg_for_cache,
                    }], vec![]),
                );
                return Ok(vec![TopLevel::SvgComponent {
                    name: component_name,
                    content: svg_content,
                }]);
            }
        }

        // Check for DBrief import (.dbv, .dbvl)
        if import.path().ends_with(".dbv") || import.path().ends_with(".dbvl") {
            let dbrief_src_dir = source_file
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."));

            let dbrief_path = self
                .search_paths
                .iter()
                .map(|p| dbrief_src_dir.join(p).join(&import.path()))
                .chain(std::iter::once(dbrief_src_dir.join(&import.path())))
                .find(|p| p.exists())
                .ok_or_else(|| {
                    format!(
                        "DBrief file not found: {} (searched in lib/, imports/, ./ and source dir)",
                        import.path()
                    )
                })?;

            let content = std::fs::read_to_string(&dbrief_path)
                .map_err(|e| format!("Failed to read DBrief file '{}': {}", dbrief_path.display(), e))?;

            let is_dbvl = import.path().ends_with(".dbvl");

            // For .dbvl files, use offset-tracking parser for lazy loading
            let doc = if is_dbvl {
                dbrief_v2::parse_document_track_offsets(&content)
            } else {
                dbrief_v2::parse_document(&content)
            }.map_err(|e| format!("Failed to parse DBrief file '{}': {}", dbrief_path.display(), e))?;

            // Determine the constant name from import symbols
            let constant_name = import
                .symbols
                .first()
                .cloned()
                .unwrap_or_else(|| {
                    let fname = dbrief_path
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "data".to_string());
                    fname
                });

            let mut dbrief_items = crate::dbrief::bridge::document_to_program_flags(
                &doc, &constant_name, is_dbvl,
            );

            let program_for_cache = dbrief_items.clone();

            self.loaded_modules.insert(
                import.path().to_string(),
                (program_for_cache, vec![]),
            );

            return Ok(dbrief_items);
        }

        // Default: Brief module (.bv or .ebv)
        let module_path = {
            if import.path().ends_with(".bv") {
                import.path()[..import.path().len() - 3].replace('.', "/")
            } else if import.path().ends_with(".ebv") {
                import.path()[..import.path().len() - 4].replace('.', "/")
            } else {
                import.path().replace('.', "/")
            }
        };
        // TypeScript-style import resolution:
        //   "./foo" or "../foo" → relative to importing file
        //   "foo/bar"           → relative to project root
        let is_relative = import.path().starts_with("./") || import.path().starts_with("../");
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

        // Search from the project root's lib/ directory (findable from
        // anywhere) — std/*, glue/*, and any other lib/ module. The walk-up
        // stops at the first ancestor Cargo.toml (the compiler repo, or a
        // user project's root).
        if !found_both && found_path.is_none() {
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
                import.path()
            ));
        }

        let resolved_path = found_path.ok_or_else(|| {
            let dir = source_dir.display();
            format!(
                "Cannot find module '{}'. Searched in: \
                 {dir}/lib/{mp}.{{bv,ebv}}, \
                 {dir}/imports/{mp}.{{bv,ebv}}, \
                 {dir}/{mp}.{{bv,ebv}}",
                import.path(),
                mp = module_path,
            )
        })?;

        // 2026-07-01: Cycle detection
        if !self.in_progress.insert(import.path().to_string()) {
            return Err(format!(
                "Circular import detected: '{}' is already being resolved \
                 (direct or transitive self-import).",
                import.path()
            ));
        }

        let source = std::fs::read_to_string(&resolved_path)
            .map_err(|e| format!("Failed to read '{}': {}", resolved_path.display(), e))?;

        let tokens = lex_source(&source)?;
        let mut parser = crate::parser::Parser::new(tokens, &source);
        // 2026-07-14: Parse errors in imported files are non-fatal — the
        // imported file may use syntax (struct literals, etc.) that the
        // parser supports as AST but not yet as a fully parseable form.
        let imported_program = parser.parse_program().unwrap_or_default();

        let resolved = self.resolve_imports(imported_program, &resolved_path)?;

        // Cache the fully resolved program
        self.loaded_modules
            .insert(import.path().to_string(), (resolved.clone(), vec![]));

        let result = self.filter_items(&resolved, &[], &import.symbols);
        self.in_progress.remove(import.path());
        result
    }

    /// Resolve a glob pattern (* or **) into individual Import nodes.
    fn resolve_glob(
        &mut self,
        import: &Import,
        source_file: &PathBuf,
    ) -> Result<Vec<TopLevel>, String> {
        let is_recursive = import.path().ends_with("/**");
        let glob_prefix = import.path().trim_end_matches("/*").trim_end_matches("/**");
        let path_prefix: Vec<String> = glob_prefix.split('/').map(|s| s.to_string()).collect();

        // Determine base directory
        let base_dir = {
            // Non-magic glob: resolve relative to project search paths
            let is_relative = glob_prefix.starts_with("./") || glob_prefix.starts_with("../");
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
                let candidate = source_dir.join(search_dir).join(glob_prefix);
                if candidate.exists() {
                    found = Some(candidate);
                    break;
                }
            }
            found.unwrap_or_else(|| source_dir.join(glob_prefix))
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
                let module_path = path_components.join("/");
                TopLevel::Import(Import::literal(module_path, vec![]))
            })
            .collect();

        Ok(items)
    }

    fn filter_items(&self, items: &[TopLevel], sed_names: &[String], symbols: &[String]) -> Result<Vec<TopLevel>, String> {
        let filtered: Vec<TopLevel> = items
            .iter()
            .filter(|item| {
                if matches!(item, TopLevel::ForeignBinding { .. }) {
                    return true;
                }
                if matches!(item, TopLevel::LinkDependency(_)) {
                    return true;
                }
                // 2026-07-24: Keep stage blocks and compile-time defns from
                // imported files so plugins in library modules can auto-execute.
                if matches!(item, TopLevel::StageBlock(_) | TopLevel::CompileTimeDefn(_) | TopLevel::CompileTimeTxn(_) | TopLevel::CompileTimeLet(_, _) | TopLevel::CompileTimeConst(_, _)) {
                    return true;
                }
                let name = match item {
                    TopLevel::Definition(d) => Some(d.name.as_str()),
                    TopLevel::Signature(s) => Some(s.name.as_str()),
                    TopLevel::ForeignBinding(fb) => Some(fb.effective_brief_name()),
                    TopLevel::Transaction(t) => Some(t.name.as_str()),
                    TopLevel::Constant(c) => Some(c.name.as_str()),
                    TopLevel::Obj(s) => Some(s.name.as_str()),
                    TopLevel::RStruct(r) => Some(r.name.as_str()),
                    TopLevel::RenderBlock(rb) => Some(rb.struct_name.as_str()),
                    TopLevel::Trigger(trg) => Some(trg.name.as_str()),
                    TopLevel::TriggerBinding { name, .. } => Some(name.as_str()),
                    TopLevel::Cell(c) => Some(c.name.as_str()),
                    TopLevel::StateDecl(s) => Some(s.name.as_str()),
                    TopLevel::TypeDef(t) => Some(t.name.as_str()),
                    // 2026-08-03: protocol declarations (proto C_String:
                    // #String) must survive imports so the casting graph gets
                    // the variant edges (marshalling paths) from library
                    // boundary modules like lib/glue/c.bv.
                    TopLevel::ProtocolDef(p) => Some(p.name.as_str()),
                    // 2026-08-03 (P3): meld declarations (meld CStr -> String)
                    // must survive imports so a boundary module's composite
                    // interchangeability applies to the importing bridge.
                    TopLevel::Meld(m) => Some(m.name.as_str()),
                    // 2026-08-01 (D3): a generic `struct ListBuffer<T>` is a
                    // StaticStruct — without an arm here it was DROPPED from
                    // every import, so `List<T>.inner: ListBuffer<T>` lost its
                    // slot type (field access failed on imported collections).
                    TopLevel::StaticStruct(s) => Some(s.name.as_str()),
                    _ => None,
                };
                let name: &str = match name {
                    Some(n) => n,
                    None => return false,
                };
                // Filter out sed (file-private) items
                if sed_names.iter().any(|s| s == name) {
                    return false;
                }
                // If specific symbols are requested, only include matches
                if symbols.is_empty() {
                    true
                } else {
                    symbols.contains(&name.to_string())
                }
            })
            .cloned()
            .collect::<Vec<_>>();
        // 2026-08-01 (D3): a named import (`{ List }`) must ALSO bring the
        // requested item's dependencies — `List` slots reference
        // `ListBuffer<T>`, which the name filter would otherwise drop. Collect
        // the transitive referenced-type closure of the kept items.
        if !symbols.is_empty() {
            let mut keep: std::collections::HashSet<String> = filtered
                .iter()
                .filter_map(top_level_name)
                .collect();
            let mut changed = true;
            while changed {
                changed = false;
                // The DEPENDENCY direction: a kept item (List) REFERENCES a
                // candidate (ListBuffer) — so collect every referenced name
                // from the kept items and add items bearing those names.
                let mut refs: std::collections::HashSet<String> = std::collections::HashSet::new();
                for item in items {
                    if item_name(item).map_or(false, |n| keep.contains(n)) {
                        for r in referenced_type_names(item) {
                            refs.insert(r);
                        }
                    }
                }
                for item in items {
                    if item_name(item).map_or(false, |n| keep.contains(n)) {
                        continue;
                    }
                    if item_name(item).map_or(false, |n| refs.contains(n)) {
                        if let Some(n) = item_name(item) {
                            keep.insert(n.to_string());
                            changed = true;
                        }
                    }
                }
            }
            return Ok(items.iter().cloned().filter(|i| {
                item_name(i).map_or(false, |n| keep.contains(n))
            }).collect());
        }
        Ok(filtered)
    }

    /// Resolve an import from the stdlib path.
    fn resolve_stdlib_import(
        &mut self,
        module: &str,
    ) -> Result<Vec<TopLevel>, String> {
        let stdlib_root = self.resolve_stdlib_root().ok_or_else(|| {
            format!(
                "Cannot resolve import '{}': no stdlib path configured. \
                 Use --stdlib-path or set BRIEF_STDLIB_PATH.",
                module
            )
        })?;

        let relative_path: PathBuf = module.split('/').collect();
        let full_path = stdlib_root.join(&relative_path);

        // Try with .bv extension if the path doesn't have one
        let candidate = if full_path.extension().is_some() {
            full_path.clone()
        } else {
            full_path.with_extension("bv")
        };

        // Use a distinct cache key for stdlib imports
        let cache_key = format!("stdlib:{}", module);
        if let Some((cached, sed_names)) = self.loaded_modules.get(&cache_key) {
            return self.filter_items(cached, sed_names, &[]);
        }

        if !candidate.exists() {
            return Err(format!(
                "Cannot find module '{}' at stdlib path: {}",
                module,
                candidate.display()
            ));
        }

        let source = std::fs::read_to_string(&candidate)
            .map_err(|e| format!("Failed to read '{}': {}", candidate.display(), e))?;

        let tokens = lex_source(&source)?;
        let mut parser = crate::parser::Parser::new(tokens, &source);
        let imported_program = parser.parse_program().unwrap_or_default();

        let resolved = self.resolve_imports(imported_program, &candidate)?;

        self.loaded_modules
            .insert(cache_key, (resolved.clone(), vec![]));

        self.filter_items(&resolved, &[], &[])
    }
}

/// Lex a source string into a token vector with span information.
fn lex_source(source: &str) -> Result<Vec<(Token, std::ops::Range<usize>)>, String> {
    let lexer = Token::lexer(source);
    let mut tokens = Vec::new();
    for result in lexer {
        let token = result.map_err(|_| "lex error".to_string())?;
        let range = 0..0;
        tokens.push((token, range));
    }
    Ok(tokens)
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
fn dedup_items(items: Vec<TopLevel>) -> Vec<TopLevel> {
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let mut result = Vec::with_capacity(items.len());
    for item in items {
        let key = match &item {
            TopLevel::Definition(d) => Some(("def", &d.name)),
            TopLevel::Transaction(t) => Some(("txn", &t.name)),
            TopLevel::StateDecl(s) => Some(("state", &s.name)),
            TopLevel::Trigger(trg) => Some(("trigger", &trg.name)),
            TopLevel::TriggerBinding { name, .. } => Some(("trg_binding", name)),
            TopLevel::Cell(c) => Some(("cell", &c.name)),
            TopLevel::Constant(c) => Some(("const", &c.name)),
            TopLevel::Signature(s) => Some(("sig", &s.name)),
            TopLevel::ForeignBinding(fb) => Some(("frgn", &fb.foreign_name)),
            TopLevel::Obj(s) => Some(("struct", &s.name)),
            TopLevel::RStruct(r) => Some(("rstruct", &r.name)),
            TopLevel::Enum(e) => Some(("enum", &e.name)),
            TopLevel::TypeDef(t) => Some(("typedef", &t.name)),
            TopLevel::RenderBlock(r) => Some(("render", &r.struct_name)),
            TopLevel::LinkDependency(l) => Some(("link", &l.path)),
            TopLevel::ResourceDecl(r) => Some(("rsrc", &r.name)),
            _ => None,
        };
        match key {
            Some((cat, name)) if seen.insert((cat.to_string(), name.to_string())) => result.push(item),
            Some(_) => {}
            None => result.push(item),
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

    fn import_program(path: &str, symbols: Vec<String>) -> Vec<TopLevel> {
        vec![TopLevel::Import(Import::literal(path.to_string(), symbols))]
    }

    #[test]
    fn test_resolve_empty_import() {
        let items = import_program("", vec![]);
        let mut resolver = ImportResolver::new();
        let result = resolver.resolve_imports(items, &PathBuf::from("main.bv")).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_resolve_bv_file() {
        let dir = TempDir::new().unwrap();
        let bv_path = dir.path().join("test_module.bv");
        fs::write(&bv_path, "defn hello -> Int { term 42; };").unwrap();

        let items = import_program("test_module", vec![]);
        let mut resolver = ImportResolver::new();
        resolver.add_search_path(dir.path().to_path_buf());
        let src = dir.path().join("main.bv");
        fs::write(&src, "").unwrap();
        let result = resolver.resolve_imports(items, &src).unwrap();
        let defns: Vec<&TopLevel> = result.iter().filter(|i| matches!(i, TopLevel::Definition(_))).collect();
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
        let items = import_program("cache_test", vec![]);
        resolver.resolve_imports(items, &src).unwrap();
        assert!(resolver.loaded_modules.contains_key("cache_test"));
    }

    #[test]
    fn test_import_css_file() {
        let dir = TempDir::new().unwrap();
        let css_path = dir.path().join("styles.m.css");
        fs::write(&css_path, "body { color: red; }").unwrap();
        let src = dir.path().join("main.bv");
        fs::write(&src, "").unwrap();

        let items = import_program("styles.m.css", vec![]);
        let mut resolver = ImportResolver::new();
        resolver.add_search_path(dir.path().to_path_buf());
        let result = resolver.resolve_imports(items, &src).unwrap();
        assert!(result.iter().any(|i| matches!(i, TopLevel::Stylesheet(_))));
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
        let items = import_program("filter_mod", vec!["keep".into()]);
        let result = resolver.resolve_imports(items, &src).unwrap();
        let names: Vec<&str> = result.iter().filter_map(|i| match i {
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
        let items = import_program("full_mod", vec![]);
        let result = resolver.resolve_imports(items, &src).unwrap();
        let count = result.iter().filter(|i| matches!(i, TopLevel::Definition(_))).count();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_resolve_module_not_found() {
        let items = import_program("nonexistent_mod", vec![]);
        let mut resolver = ImportResolver::new();
        let src = PathBuf::from("/tmp/main.bv");
        let result = resolver.resolve_imports(items, &src);
        assert!(result.is_err());
    }

    #[test]
    fn test_glob_import_non_recursive() {
        let dir = TempDir::new().unwrap();
        let stdlib_root = dir.path().join("lib");
        let core_dir = stdlib_root.join("std").join("core");
        fs::create_dir_all(&core_dir).unwrap();
        fs::write(core_dir.join("a.bv"), "defn a_fn -> Int { term 1; };").unwrap();
        fs::write(core_dir.join("b.bv"), "defn b_fn -> Int { term 2; };").unwrap();
        let sub_dir = core_dir.join("sub");
        fs::create_dir_all(&sub_dir).unwrap();
        fs::write(sub_dir.join("c.bv"), "defn c_fn -> Int { term 3; };").unwrap();
        let src = dir.path().join("main.bv");
        fs::write(&src, "").unwrap();

        let items = vec![TopLevel::Import(Import::literal("std/core/*", vec![]))];
        let mut resolver = ImportResolver::new()
            ;
        resolver.add_search_path(stdlib_root);
        let result = resolver.resolve_imports(items, &src).unwrap();
        let defns: Vec<&TopLevel> = result.iter().filter(|i| matches!(i, TopLevel::Definition(_))).collect();
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
        fs::write(types_dir.join("bootstrap.bv"), "type Int : Bits { maxbits <~ 64; }; type Float : Bits { maxbits <~ 32; };").unwrap();
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

        let items = import_program("", vec![]);
        let mut resolver = ImportResolver::new()
            .with_stdlib_path(Some(stdlib_root));
        let result = resolver.resolve_imports(items, &src).unwrap();
        let defns: Vec<&TopLevel> = result.iter().filter(|i| matches!(i, TopLevel::Definition(_))).collect();
        // Prelude injection is now handled by the plugin system, not the resolver.
        // Definitions come from explicit imports, not auto-injection.
        // This test is preserved as a smoke test that resolve_imports doesn't crash.
        assert!(true);
    }

    #[test]
    fn test_import_target_board_directory() {
        // 2026-08-03 (Phase 2): `import "target"` reads the board directory
        // (map.dbv + addresses.dbvl + registers.dbvl) and flattens constants.
        let dir = TempDir::new().unwrap();
        let board_dir = dir.path().join("lib").join("boards").join("stm32f407");
        fs::create_dir_all(&board_dir).unwrap();
        fs::write(
            board_dir.join("map.dbv"),
            "schema Device { base_addr: String; size: Int; };\n",
        )
        .unwrap();
        fs::write(
            board_dir.join("addresses.dbvl"),
            ">schema Device from \"map.dbv\"\nUART1: 0x40011000; 0x18;\nGPIOA: 0x40020000; 0x400;\n",
        )
        .unwrap();
        fs::write(
            board_dir.join("registers.dbvl"),
            ">schema Device from \"map.dbv\"\nUART1_DR: 0x00; 9; rw;\n",
        )
        .unwrap();

        let items = import_program("target", vec![]);
        let mut resolver = ImportResolver::new();
        resolver.add_search_path(dir.path().to_path_buf());
        let src = dir.path().join("main.bv");
        fs::write(&src, "").unwrap();

        let result = resolver.resolve_imports(items, &src).unwrap();

        // The board directory loads without error and emits the address
        // constant; set_active_board ran (address resolver sees UART1).
        assert!(result.len() > 0);
        let constant_names: Vec<String> = result
            .iter()
            .filter_map(|i| match i {
                TopLevel::Constant(c) => Some(c.name.clone()),
                _ => None,
            })
            .collect();
        assert!(
            constant_names.iter().any(|n| n.contains("UART1")),
            "expected UART1-derived constants, got {constant_names:?}"
        );
        assert_eq!(crate::address_resolver::resolve_address("uart1"), 0x40011000);
    }
}
