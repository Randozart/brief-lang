// ── Compilation Pipeline ──────────────────────────────────────────────
// 2026-07-12: Phase 7 — Compile a Brief source file end-to-end.
// Pipeline: lex -> parse -> typecheck -> codegen -> output.
// 2026-07-14: Wire real LlvmBackend instead of stub codegen.
//             Add binary compilation via clang. Add --out / --optimize-budget flags.
// 2026-07-14: Plugin path — serialize to BEAST, run external plugins, deserialize.
// 2026-07-15: Phase 2 — Wire per-stage plugin dispatch into pipeline.
//             Front: on_ast after parse, Mid: on_ast after typecheck,
//             Post/Back: on_ir after codegen. Per-extension plugin selection
//             from config/targets.toml. System plugin discovery from
//             plugins/{front,mid,post,back}/.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use brief_compiler::backend::llvm::LlvmBackend;
use brief_compiler::ast::{Expr, StageKind, TopLevel};
use brief_compiler::lexer::Token;
use brief_compiler::plugin::loader::{discover_system_plugins, extract_inline_stage_blocks};
use brief_compiler::plugin::PluginManager;
use brief_compiler::target::{BackendKind, TargetConfig, get_extension};
use brief_compiler::type_universe::TypeUniverse;

/// Re-export the LLVM backend's TrgUnresolvedAction for CLI flag parsing.
/// 2026-07-15: Phase 7i — Defined in the backend to avoid circular deps.
pub use brief_compiler::backend::llvm::TrgUnresolvedAction;

/// Pipeline stage at which to emit a BEAST snapshot or IR snapshot.
/// 2026-07-21: Expanded to granular stages matching the pipeline.
/// AST stages emit .beast files; IR stages emit .ir files.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BeastStage {
    Parse,
    Resolve,
    TypeCheck,
    Normalize,
    Verify,
    Alloc,
    Provenance,
    Codegen,
    Optimize,
}

impl BeastStage {
    /// Stages that have AST data (get .beast files).
    pub fn has_ast(&self) -> bool {
        matches!(self, BeastStage::Parse | BeastStage::Resolve
            | BeastStage::TypeCheck | BeastStage::Normalize | BeastStage::Verify
            | BeastStage::Alloc | BeastStage::Provenance)
    }

    /// Stages that have IR text data (get .ir files).
    pub fn has_ir(&self) -> bool {
        matches!(self, BeastStage::Codegen | BeastStage::Optimize)
    }
}

impl std::str::FromStr for BeastStage {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "parse" => Ok(BeastStage::Parse),
            "resolve" => Ok(BeastStage::Resolve),
            "type-check" => Ok(BeastStage::TypeCheck),
            "normalize" => Ok(BeastStage::Normalize),
            "verify" => Ok(BeastStage::Verify),
            "alloc" => Ok(BeastStage::Alloc),
            "provenance" => Ok(BeastStage::Provenance),
            "codegen" => Ok(BeastStage::Codegen),
            "optimize" => Ok(BeastStage::Optimize),
            "all" => Err("use multiple --emit-beast flags for individual stages".to_string()),
            _ => Err(format!("unknown BEAST stage '{}'. Use one of: parse, resolve, type-check, \
                             normalize, verify, alloc, provenance, codegen, optimize", s)),
        }
    }
}

/// Snapshot position relative to plugin execution.
/// 2026-07-23: Used by --emit-beast for pre/post plugin snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeastPosition {
    Before,
    After,
}

impl BeastPosition {
    pub fn priority(self) -> u32 {
        match self {
            BeastPosition::Before => 999,
            BeastPosition::After => 0,
        }
    }
}

/// A BEAST snapshot filter: emit at a specific (stage, position) pair.
/// 2026-07-23: --emit-beast accepts stage.position (before/after) or plain stage (both).
#[derive(Debug, Clone, PartialEq)]
pub struct BeastFilter {
    pub stage: BeastStage,
    pub position: Option<BeastPosition>,
}

impl std::str::FromStr for BeastFilter {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(dot) = s.find('.') {
            let stage_str = &s[..dot];
            let pos_str = &s[dot + 1..];
            let stage: BeastStage = stage_str.parse()?;
            let position = match pos_str {
                "before" => BeastPosition::Before,
                "after" => BeastPosition::After,
                _ => return Err(format!("unknown beast position '{}'. Use before, after, or omit", pos_str)),
            };
            Ok(BeastFilter { stage, position: Some(position) })
        } else {
            let stage: BeastStage = s.parse()?;
            Ok(BeastFilter { stage, position: None })
        }
    }
}

/// Options parsed from the `brief-compiler build` CLI flags.
#[derive(Clone)]
pub struct BuildOptions {
    pub config_dir: Option<String>,
    pub file_path: String,
    pub emit_ir_only: bool,
    pub out_dir: Option<String>,
    pub optimize_budget: u64,
    pub gpu_offload: bool,
    /// BEAST snapshot stages to emit (--emit-beast). Empty = no emission.
    pub emit_beast_stages: Vec<BeastFilter>,
    /// Selected backend (resolved from extension + --backend flag).
    pub backend: BackendKind,
    /// Disable automatic stdlib import (for bare-metal/no-OS targets).
    pub no_stdlib: bool,
    /// Override stdlib search path.
    pub stdlib_path: Option<String>,
    /// Plugin names to disable (CLI --disable-plugin).
    pub disable_plugins: Vec<String>,
    /// Plugin names to enable exclusively (CLI --enable-plugin).
    pub enable_plugins: Vec<String>,
    /// Action on unresolved dynamic trigger target (--error-unresolved-trg).
    pub trg_unresolved_action: TrgUnresolvedAction,
    /// 2026-07-16: P4 — Pre-compiled .o / .so / .a objects linked into the binary.
    pub extra_objects: Vec<PathBuf>,
    /// 2026-07-18: Build a shared library (.so) instead of an executable.
    pub shared: bool,
    /// 2026-07-18: Phase B — Enable SSO (Short String Optimization) for String types.
    /// When ON, String is a 2-field \`{ i64, i64 }\` struct with inline storage for ≤6
    /// bytes, heap for longer. When OFF (default), String is passed as \`i8*\` (legacy).
    pub feature_sso_strings: bool,
    /// 2026-07-18: SVO (Small Vector Optimization) — inline storage for
    /// small List<T> elements (≤ N where N is from svo <~ N metadata).
    pub feature_svo: bool,
    /// 2026-07-22: Override path for lib/glue.toml. None = use compiler-shipped default.
    pub glue_config: Option<String>,
    /// 2026-07-18: Maximum size in bytes for stack allocation (alloca).
    /// Allocations exceeding this threshold fall back to heap (malloc).
    /// Used by the runtime fallback check in emit_dynamic_alloc.
    /// Default 4096 (4KB) — safe for most stack frames.
    pub stack_threshold: u64,
    /// 2026-07-25: Native integer width for #Int protocol (default 64).
    /// WASM targets should set to 32 to avoid BigInt in JavaScript.
    pub int_bits: u64,
    /// 2026-07-26: Phase 6b — CSS content from <style> block in .rbv files.
    /// Written as app.css alongside the compiled output for webstack backend.
    pub style_css: Option<String>,
    /// 2026-07-26: Phase 6b — Raw HTML from <view> block in .rbv files.
    /// Wrapped in index.html boilerplate for webstack backend.
    pub view_html: Option<String>,
    /// 2026-07-26: Phase 6b — View bindings from processed <html> template.
    /// Passed to GlueWebGenerator for DOM binding table generation.
    pub view_bindings: Vec<brief_compiler::view_compiler::Binding>,
    /// 2026-07-26: Item 3 — Enable SSR (Server-Side Rendering).
    /// Pre-renders initial state into the HTML at compile time.
    /// Only meaningful for webstack backend.
    pub ssr: bool,
    /// 2026-07-26: Item 4 — Enable dev mode (HMR support).
    /// Uses dev-shim.mjs instead of dom-shim.mjs.
    /// Only meaningful for webstack backend.
    pub dev: bool,
    /// 2026-07-23: Allow macros to read files (FileRead$).
    pub allow_read: bool,
    /// 2026-07-23: Allow macros to write files (FileWrite$).
    pub allow_write: bool,
    /// 2026-07-23: Allow macros to execute shell commands (ShellCmd$).
    pub allow_run: bool,
    /// 2026-07-23: Allow macros to query host hardware (SysQuery$).
    pub allow_sys_query: bool,
    /// 2026-07-23: Allow macros network access (HttpFetch$).
    pub allow_net: bool,
    /// 2026-07-23: Instruction budget for macro execution (0 = unlimited).
    pub macro_budget: u64,
    /// 2026-07-23: Print virtual filesystem contents after compilation.
    pub dump_vfs: bool,
    /// 2026-07-23: Regenerate macro-lock.toml from current plugin files.
    pub update_lockfile: bool,
    /// 2026-07-23: Print macro expansion traces after compilation.
    pub dump_traces: bool,
    /// 2026-07-23: Diff mode — show changes macros make to the AST without
    /// writing output. Acts as a dry-run: shows added/removed/modified items.
    pub diff_mode: bool,
    /// 2026-07-23: Overrides for SysQuery$ results.
    /// Populated by --sysquery <key=value> and --sysquery-file <path> flags.
    /// Also populated by --target <name> from brief.toml profiles.
    /// Empty = query real host. Later values override earlier ones.
    pub sysquery_overrides: HashMap<String, String>,
    /// 2026-07-23: Target profile name (--target). None = default/single build.
    /// Overrides are resolved from brief.toml and merged into sysquery_overrides.
    pub target: Option<String>,
    /// 2026-07-23: Raw --sysquery flag pairs (unresolved, for run_build).
    pub sysquery_pairs: Vec<(String, String)>,
    /// 2026-07-23: Raw --sysquery-file paths (unresolved, for run_build).
    pub sysquery_files: Vec<String>,
}

/// Compile a Brief source file: produce an executable binary (or `.ll` with `--llvm`).
/// 2026-07-25: Evaluate pending $let/$const compile-time variable initializers.
/// Called after both extract_inline_stage_blocks calls, before any stage blocks
/// execute. This ensures $let/$const values are available to all stage blocks.
fn evaluate_pending_comptime(
    pm: &mut PluginManager,
    program: &mut Vec<TopLevel>,
    universe: &mut TypeUniverse,
) -> Result<(), String> {
    let pending: Vec<(String, brief_compiler::ast::Expr, bool)> = pm.pending_comptime.drain()
        .map(|(k, (e, c))| (k, e, c))
        .collect();
    // 2026-07-25: Use a fresh sandbox cloned from pm for evaluation, then
    // merge back to preserve capability tracking.
    let mut sandbox = pm.sandbox.clone();
    for (name, expr, is_const) in pending {
        let val = {
            let mut pm_opt: Option<&mut PluginManager> = Some(pm);
            brief_compiler::macros::eval::eval_nav_chain(
                &expr, program, universe, StageKind::Parsed,
                &std::collections::HashMap::new(),
                &mut sandbox, &mut pm_opt,
            )?
        };
        pm.comptime_vars.insert(name, (val, is_const));
    }
    pm.sandbox = sandbox;
    Ok(())
}

/// 2026-07-25: Resolve comptime variable references in const initializers and
/// trg instance expressions. Replaces Expr::Identifier references to $let/$const
/// names with their evaluated NavValue literals before type checking / codegen.
fn resolve_comptime_refs(
    pm: &PluginManager,
    program: &mut Vec<TopLevel>,
) -> Result<(), String> {
    for item in program.iter_mut() {
        match item {
            TopLevel::Constant(c) => {
                if let Expr::Identifier(name) = &c.expr {
                    if let Some((val, _)) = pm.comptime_vars.get(name) {
                        c.expr = nav_value_to_expr(val)?;
                    }
                }
            }
            TopLevel::Trigger(trg) => {
                if let Expr::Identifier(name) = &trg.instance {
                    if let Some((val, _)) = pm.comptime_vars.get(name) {
                        trg.instance = nav_value_to_expr(val)?;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// 2026-07-25: Convert a NavValue to its corresponding Expr literal.
fn nav_value_to_expr(val: &brief_compiler::macros::eval::NavValue) -> Result<Expr, String> {
    match val {
        brief_compiler::macros::eval::NavValue::Int(n) => Ok(Expr::Decimal(*n)),
        brief_compiler::macros::eval::NavValue::Bool(b) => Ok(Expr::Bool(*b)),
        brief_compiler::macros::eval::NavValue::Str(s) => Ok(Expr::Quoted(s.as_bytes().to_vec())),
        _ => Err(format!("cannot convert {:?} to Expr literal", val)),
    }
}

pub fn compile_source(file_path: &str, source: &str, opts: &BuildOptions) -> Result<(), String> {
    // ── Macro lockfile handling ────────────────────────────────────
    // 2026-07-23: If --update-lockfile, regenerate macro-lock.toml from
    // current plugin files and --allow-* flags. Otherwise validate the
    // lockfile against loaded plugins and apply approved capabilities.
    let mut pm = build_plugin_manager(file_path, opts);
    let project_root = std::env::current_dir()
        .map_err(|e| format!("cannot determine project root: {}", e))?;
    let project_root_str = project_root.to_string_lossy().to_string();
    if opts.update_lockfile {
        let granted = brief_compiler::macros::lockfile::cli_granted_set(
            opts.allow_read,
            opts.allow_write,
            opts.allow_run,
            opts.allow_sys_query,
            opts.allow_net,
        );
        let lock = brief_compiler::macros::lockfile::generate_lockfile(&granted, None)?;
        brief_compiler::macros::lockfile::save_lockfile(&project_root_str, &lock)?;
    } else {
        if let Some(lock) = brief_compiler::macros::lockfile::load_lockfile(&project_root_str)? {
            brief_compiler::macros::lockfile::validate_and_apply(&lock, &mut pm, None)?;
        }
    }

    // ── PreLex stage: source transformation ──────────────────────────
    let mut source = source.to_string();
    pm.run_source(StageKind::PreLex, &mut source)?;

    // ── Parse ─────────────────────────────────────────────────────────
    let tokens = lex(&source)?;
    let mut items = parse(file_path, &tokens, &source)?;

    // Extract inline $(Stage) blocks from the AST — they are plugins,
    // not runtime code.
    extract_inline_stage_blocks(&mut items, &mut pm);

    // 2026-07-25: Evaluate $let/$const initializers before Parsed stage.
    {
        let mut eval_universe = TypeUniverse::new();
        evaluate_pending_comptime(&mut pm, &mut items, &mut eval_universe)?;
    }

    // 2026-07-23: Snapshot the program before any macro evaluation for --diff.
    let pre_macro_items = if opts.diff_mode {
        Some(items.clone())
    } else {
        None
    };

    // ── Parsed stage: AST transformation (before import resolution) ───
    {
        let mut parsed_universe = TypeUniverse::new();
        emit_beast_snapshot(file_path, BeastStage::Parse, BeastPosition::Before, &items, &TypeUniverse::new(), opts)?;
                pm.run_ast(StageKind::Parsed, &mut items, &mut parsed_universe)?;
    }

    // BEAST snapshot at Parse stage
    {
        let snapshot_universe = TypeUniverse::new();
        emit_beast_snapshot(file_path, BeastStage::Parse, BeastPosition::After, &items, &snapshot_universe, opts)?;
    }

    // ── Resolved stage (after import resolution) ──────────────────────
    let mut resolver = brief_compiler::import_resolver::ImportResolver::new();
    if let Some(ref stdlib_path) = opts.stdlib_path {
        resolver = resolver.with_stdlib_path(Some(std::path::PathBuf::from(stdlib_path)));
    }
    items = resolver.resolve_imports(items, &std::path::PathBuf::from(file_path))?;

    // 2026-07-24: Extract stage blocks from imported files. The first
    // extract_inline_stage_blocks ran before import resolution, so stage
    // blocks in imported modules were not captured.
    extract_inline_stage_blocks(&mut items, &mut pm);

    // 2026-07-25: Evaluate any new $let/$const from imported modules.
    {
        let mut eval_universe = TypeUniverse::new();
        evaluate_pending_comptime(&mut pm, &mut items, &mut eval_universe)?;
    }

    {
        emit_beast_snapshot(file_path, BeastStage::Resolve, BeastPosition::Before, &items, &TypeUniverse::new(), opts)?;
                pm.run_ast(StageKind::Resolved, &mut items, &mut TypeUniverse::new())?;
    }
    emit_beast_snapshot(file_path, BeastStage::Resolve, BeastPosition::After, &items, &TypeUniverse::new(), opts)?;

    // ── Type check ────────────────────────────────────────────────────
    // 2026-07-25: Resolve comptime var references in const initializers
    // and trg instance expressions before type checking.
    resolve_comptime_refs(&pm, &mut items)?;
    let mut universe = TypeUniverse::new();
    check_types(&items, &universe)?;

    // ── Typed stage: AST transformation (after type check) ────────────
    emit_beast_snapshot(file_path, BeastStage::TypeCheck, BeastPosition::Before, &items, &universe, opts)?;
    pm.run_ast(StageKind::Typed, &mut items, &mut universe)?;
    emit_beast_snapshot(file_path, BeastStage::TypeCheck, BeastPosition::After, &items, &universe, opts)?;

    // ── Normalizer pass ───────────────────────────────────────────────
    match opts.backend {
        BackendKind::Llvm | BackendKind::Gpu => {
            brief_compiler::backend::llvm::normalizer::normalize(&mut items, &mut universe)?;
        }
        BackendKind::Circt => {
            brief_compiler::backend::circt_normalizer::normalize(&mut items, &mut universe)?;
        }
        BackendKind::Webstack => {
            brief_compiler::backend::webstack_normalizer::normalize(&mut items, &mut universe)?;
        }
        BackendKind::Spirv => {
            brief_compiler::backend::spirv::normalizer::normalize(&mut items, &mut universe)?;
        }
        BackendKind::Vm => {
            // VM backend is target-agnostic — no normalization needed.
        }
    }

    emit_beast_snapshot(file_path, BeastStage::Normalize, BeastPosition::Before, &items, &universe, opts)?;
    pm.run_ast(StageKind::Normalized, &mut items, &mut universe)?;
    emit_beast_snapshot(file_path, BeastStage::Normalize, BeastPosition::After, &items, &universe, opts)?;

    // ── Build protocol graph from protocol declarations ────────────────
    // 2026-07-23: Builds variant-aware CastTo/CastFrom edges from
    // TopLevel::ProtocolDef items (proto ASCII: #String { ... }) and
    // TypeDef.protocol fields. Injects edges into the universe so the
    // existing BFS can find them.
    let protocol_graph = brief_compiler::analysis::protocol_graph::ProtocolGraph::build_from(&items);
    protocol_graph.inject_edges(&mut universe);

    // ── Protocol contract enforcement via SMT ──────────────────────────
    // 2026-07-23: For each protocol declaration with a contract, prove
    // the invariant holds using the SMT solver. If unprovable, deny.
    // Also validate that all CastTo/CastFrom have bindings.
    for item in &items {
        if let brief_compiler::ast::TopLevel::ProtocolDef(pd) = item {
            // Validate bindings exist on all CastTo/CastFrom edges
            for edge in &pd.cast_edges {
                if edge.binding.is_none() {
                    return Err(format!(
                        "protocol '{}': {} must have a binding (e.g., CastTo(#target) = fn(#L))",
                        pd.name,
                        match edge.direction {
                            brief_compiler::ast::top::CastDirection::CastTo => "CastTo",
                            brief_compiler::ast::top::CastDirection::CastFrom => "CastFrom",
                        }
                    ));
                }
            }
            // Validate contract if present
            if let Some(ref contract) = pd.contract {
                let pre = &contract.pre_condition;
                let post = &contract.post_condition;
                let params = vec![("Self".to_string(), brief_compiler::ast::Type::int())];
                if let Err(errs) = brief_compiler::proof_engine::prove_contract(pre, post, &params) {
                    return Err(format!("protocol contract violation in '{}': {:?}", pd.name, errs));
                }
            }
            // 2026-07-23: Round-trip proof — CastFrom(CastTo(x)) == x
            if let Err(msg) = brief_compiler::analysis::protocol_graph::verify_protocol_roundtrip(pd, &items) {
                return Err(msg);
            }
            // 2026-07-23: Cross-op equivalence proof
            if let Err(msg) = brief_compiler::analysis::protocol_graph::verify_crossop_equivalence(pd, &items) {
                return Err(msg);
            }
        }
    }

    // ── frgn? guard safety check ────────────────────────────────────
    // 2026-07-25: Verify every frgn?/frgn!/frgn?! call is guarded by fn?.
    brief_compiler::analysis::frgn_guard::check_frgn_guards(&items)
        .map_err(|e| format!("frgn guard error:\n{}", e))?;

    // ── Protocol round-trip verification ──────────────────────────────
    brief_compiler::protocol_verify::verify_roundtrips(&items, &universe)?;

    emit_beast_snapshot(file_path, BeastStage::Verify, BeastPosition::Before, &items, &universe, opts)?;
    pm.run_ast(StageKind::Verified, &mut items, &mut universe)?;
    emit_beast_snapshot(file_path, BeastStage::Verify, BeastPosition::After, &items, &universe, opts)?;

    // ── Slice narrowing ───────────────────────────────────────────────
    // 2026-07-26: Convert constant-bounds Expr::Slice to direct array access.
    brief_compiler::analysis::narrow_slice::narrow_slices(&mut items);

    // ── Allocation strategy analysis ──────────────────────────────────
    let alloc_strategies = brief_compiler::analysis::allocation::analyze_alloc_strategies(&mut items);
    emit_beast_snapshot(file_path, BeastStage::Alloc, BeastPosition::Before, &items, &universe, opts)?;
    pm.run_ast(StageKind::Allocated, &mut items, &mut universe)?;
    emit_beast_snapshot(file_path, BeastStage::Alloc, BeastPosition::After, &items, &universe, opts)?;

    // ── Dangling pointer detection ────────────────────────────────────
    use brief_compiler::analysis::provenance::{check_dangling_ptrs, collect_local_names};
    for item in &items {
        if let brief_compiler::ast::TopLevel::Transaction(txn) = item {
            let local_names = collect_local_names(&txn.body, &txn.parameters);
            let warnings = check_dangling_ptrs(&txn.body, &local_names);
            for w in &warnings {
                eprintln!("{}", w);
            }
        }
    }

    emit_beast_snapshot(file_path, BeastStage::Provenance, BeastPosition::Before, &items, &universe, opts)?;
    pm.run_ast(StageKind::Provenanced, &mut items, &mut universe)?;
    emit_beast_snapshot(file_path, BeastStage::Provenance, BeastPosition::After, &items, &universe, opts)?;

    // 2026-07-16: P4 — Collect extra objects from ForeignBinding FromSpec paths
    // for linking into the final binary.
    let extra_objects = collect_extra_objects(&items, &resolver)?;

    // ── Frgn dispatch resolution ──────────────────────────────────────
    // 2026-07-22: Resolve each frgn declaration's dispatch strategy before
    // codegen. The backend receives the resolved strategies and does not
    // re-implement dispatch logic.
    let glue_targets = brief_compiler::glue::config::load_glue_config(
        opts.glue_config.as_deref().map(Path::new),
    )?;
    let mut resolved_frgns: std::collections::HashMap<
        String, brief_compiler::analysis::frgn_dispatch::ResolvedFrgn,
    > = std::collections::HashMap::new();
    for item in &items {
        let brief_compiler::ast::TopLevel::ForeignBinding(fb) = item else { continue; };
        let ext = fb.from.extension().unwrap_or_default();
        let dispatch = brief_compiler::analysis::frgn_dispatch::resolve_single_frgn(
            fb, &ext, &glue_targets, opts.backend, Some(&universe),
        )?;
        resolved_frgns.insert(fb.effective_brief_name().to_string(), dispatch);
    }

    // 2026-07-26: Collect protocol library names from resolved frgns
    // for passing as -l<lib> flags to clang during linking.
    let protocol_libs: Vec<String> = resolved_frgns.values().filter_map(|rf| {
        if let brief_compiler::analysis::frgn_dispatch::ResolvedFrgn::Inline { protocol_lib: Some(lib), .. } = rf {
            Some(lib.clone())
        } else {
            None
        }
    }).collect();

    // ── Layout optimization (frgn/export boundary) ─────────────────────
    // 2026-07-22: Propose adopting foreign type layouts to minimize
    // protocol transform costs. Only applies to bridge-path frgns.
    // This is additive — removing this pass does not affect correctness.
    let layout_changes = brief_compiler::analysis::layout_optimizer::optimize_layouts(
        &items, &universe, &resolved_frgns, &glue_targets,
    )?;
    for change in &layout_changes {
        brief_compiler::analysis::layout_optimizer::apply_layout_change(&mut items, change)?;
    }
    if !layout_changes.is_empty() {
        eprintln!("layout optimizer: {} change(s) applied", layout_changes.len());
    }

    // ── Diff mode / dry-run ─────────────────────────────────────────────
    // 2026-07-23: If --diff was specified, show what macros changed and exit
    // before codegen/writing. No output file is produced.
    if let Some(ref pre_macro) = pre_macro_items {
        let diff = brief_compiler::macros::diff::compute_diff(pre_macro, &items);
        if diff.is_empty() {
            println!("(no changes)");
        } else {
            println!("\n=== Macro Changes ({} change(s)) ===", diff.len());
            brief_compiler::macros::diff::print_diff(&diff);
            println!("=== End Macro Changes ===");
        }
        return Ok(());
    }

    // ── Code generation ───────────────────────────────────────────────
    // 2026-07-23: Check if any glue target requests native module init.
    let enable_module_init = glue_targets.values().any(|t| t.module_init);

    let (codegen_output, ext) = codegen(&items, &mut universe, &pm, opts, alloc_strategies, resolved_frgns, enable_module_init)?;

    // BEAST/IR snapshot at Codegen stage
    emit_beast_snapshot(file_path, BeastStage::Codegen, BeastPosition::After, &items, &universe, opts)?;

    // ── Generated stage: IR text manipulation ──────────────────────────
    let mut output = codegen_output;
    pm.run_ir(StageKind::Generated, &mut output)?;

    // ── Write output ──────────────────────────────────────────────────
    let out_path = determine_out_path(file_path, opts.out_dir.as_deref())?;
    let out_path = out_path.replace(".ll", ext);

    // 2026-07-15: SPIR-V writes inside codegen (binary format), skip outer write
    // 2026-07-25: Vm backend also writes inside codegen (.lair is binary)
    if opts.backend != BackendKind::Spirv && opts.backend != BackendKind::Vm {
        if let Some(parent) = std::path::Path::new(&out_path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("cannot create output dir '{}': {}", parent.display(), e))?;
            }
        }
        std::fs::write(&out_path, &output)
            .map_err(|e| format!("cannot write '{}': {}", out_path, e))?;
        println!("wrote {}", out_path);
    }

    // ── Optimized stage: final IR validation ──────────────────────────
    pm.run_ir(StageKind::Optimized, &mut output)?;
    emit_beast_snapshot(file_path, BeastStage::Optimize, BeastPosition::After, &items, &universe, opts)?;

    if !opts.emit_ir_only {
        let binary_base = out_path.strip_suffix(ext).unwrap_or(&out_path);
        let binary_path = if opts.shared {
            format!("{}.so", binary_base)
        } else {
            binary_base.to_string()
        };
        if opts.backend == BackendKind::Llvm || opts.backend == BackendKind::Gpu {
            // Merge CLI-provided extra_objects with ones collected from frgn
            // declarations (frgn .c/.cpp sources are auto-compiled to .o).
            // 2026-07-26: Deduplicate — multiple frgns may reference the same
            // .c source (e.g., brief_rt.c), producing identical cached .o paths.
            let mut all_objects = opts.extra_objects.clone();
            all_objects.extend(extra_objects);
            all_objects.sort();
            all_objects.dedup();
            compile_ll_to_binary(&out_path, &binary_path, &all_objects, &protocol_libs, opts.shared)?;
        }
        // 2026-07-26: Phase 5 — Compile LLVM IR to WASM binary for webstack backend.
        // Uses llc to compile the .ll (emitted with wasm32 target triple) to .wasm.
        // Skips C runtime linking — WASM modules are self-contained pure logic.
        if opts.backend == BackendKind::Webstack {
            let wasm_path = format!("{}.wasm", binary_base);
            compile_wasm(&out_path, &wasm_path)?;

            // 2026-07-26: Phase 6b — Write app.css from <style> block content.
            if let Some(ref css) = opts.style_css {
                let css_path = format!("{}.css", binary_base);
                std::fs::write(&css_path, css)
                    .map_err(|e| format!("cannot write '{}': {}", css_path, e))?;
                println!("wrote {}", css_path);
            }

            // 2026-07-26: Phase 6b — Write index.html from <view> block content.
            // Wraps the raw view HTML in a minimal HTML5 boilerplate that
            // links app.css and loads dom-shim.mjs via ES module import.
            if let Some(ref html) = opts.view_html {
                let index_path = format!("{}.html", binary_base);
                let index_content = format!(
                    "<!DOCTYPE html>\n\
                     <html lang=\"en\">\n\
                     <head>\n\
                     <meta charset=\"UTF-8\">\n\
                     <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n\
                     <link rel=\"stylesheet\" href=\"app.css\">\n\
                     <script type=\"module\" src=\"dom-shim.mjs\"></script>\n\
                     </head>\n\
                     <body>\n\
                     {}\n\
                     <script type=\"module\">\n\
                     import {{ createApp }} from './dom-shim.mjs';\n\
                     fetch('{}.wasm').then(r => r.arrayBuffer())\n\
                       .then(bytes => createApp(new Uint8Array(bytes)));\n\
                     </script>\n\
                     </body>\n\
                     </html>\n",
                    html,
                    binary_base,
                );
                std::fs::write(&index_path, &index_content)
                    .map_err(|e| format!("cannot write '{}': {}", index_path, e))?;
                println!("wrote {}", index_path);

                // 2026-07-26: Item 3 — SSR pass. If --ssr is set, replace
                // the standard app.html with an SSR-enabled version that
                // embeds initial state as JSON and pre-renders the view.
                if opts.ssr {
                    let ssr_out = brief_compiler::ssr::render_ssr(
                        html,
                        &items,
                        opts.style_css.as_deref(),
                        binary_base,
                        opts.dev,
                    );
                    std::fs::write(&index_path, &ssr_out.full_html)
                        .map_err(|e| format!("cannot write SSRed '{}': {}", index_path, e))?;
                    println!("ssr {}", index_path);
                }
            }

            // 2026-07-26: Phase 6c — Generate dom-shim.mjs + .d.ts from frgn decls.
            let frgn_decls: Vec<brief_compiler::ast::ForeignBinding> = items.iter()
                .filter_map(|item| {
                    if let brief_compiler::ast::TopLevel::ForeignBinding(fb) = item {
                        if matches!(fb.from, brief_compiler::ast::FromSpec::Protocol(ref p) if p == "#Web") {
                            Some(fb.clone())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect();
            if !frgn_decls.is_empty() || !opts.view_bindings.is_empty() {
                // Build a StateLayout matching what the LLVM backend emits
                // (generation at offset 0, flush buffer at 64, max 16 entries).
                let state_layout = brief_compiler::glue::web_generator::StateLayout {
                    app_name: binary_base.to_string(),
                    generation_offset: 0,
                    flush_buffer_offset: 64,
                    max_flush_entries: 16,
                    fields: vec![],
                };
                let web_gen = brief_compiler::glue::web_generator::GlueWebGenerator::new(
                    Vec::new(), // wasm bytes not needed for stub generation
                    opts.view_bindings.clone(),
                    state_layout,
                    HashMap::new(),
                    frgn_decls,
                );
                match web_gen.generate() {
                    Ok(output) => {
                        let mjs_path = format!("{}.mjs", binary_base);
                        std::fs::write(&mjs_path, &output.dom_shim)
                            .map_err(|e| format!("cannot write '{}': {}", mjs_path, e))?;
                        println!("wrote {}", mjs_path);
                        let dts_path = format!("{}.d.ts", binary_base);
                        std::fs::write(&dts_path, &output.dts)
                            .map_err(|e| format!("cannot write '{}': {}", dts_path, e))?;
                        println!("wrote {}", dts_path);
                    }
                    Err(e) => {
                        return Err(format!("GlueWebGenerator failed: {}", e));
                    }
                }
            }
        }

        // ── Linked stage: binary processing ───────────────────────────
        let bin_path = std::path::Path::new(&binary_path);
        pm.run_bin(bin_path)?;
    }

    // ── VFS dump / flush ────────────────────────────────────────────
    // 2026-07-23: If --dump-vfs was specified, print virtual filesystem contents.
    if opts.dump_vfs && !pm.vfs.is_empty() {
        println!("\n=== Virtual Filesystem Contents ===");
        let mut sorted: Vec<&String> = pm.vfs.keys().collect();
        sorted.sort();
        for path in &sorted {
            let content = &pm.vfs[*path];
            println!("  {} ({} bytes)", path, content.len());
            if let Some(first_line) = content.lines().next() {
                let preview = if first_line.len() > 80 { &first_line[..77] } else { first_line };
                println!("    -> {}", preview);
            }
        }
        println!("=== End VFS ===");
    }

    // ── Expansion traces dump ─────────────────────────────────────────
    // 2026-07-23: If --dump-traces was specified, print macro expansion traces.
    if opts.dump_traces && !pm.expansion_traces.is_empty() {
        println!("\n=== Macro Expansion Traces ===");
        let mut sorted: Vec<(usize, String)> = pm.expansion_traces.iter()
            .map(|(k, v)| (*k, v.clone())).collect();
        sorted.sort_by_key(|(k, _)| *k);
        for (idx, desc) in &sorted {
            println!("  [{}] {}", idx, desc);
        }
        println!("=== End Expansion Traces ===");
    }

    Ok(())
}

/// Type-check only: don't generate code.
pub fn check_source(file_path: &str, source: &str) -> Result<(), String> {
    let default_opts = BuildOptions {
        config_dir: None,
        file_path: file_path.to_string(),
        emit_ir_only: false,
        out_dir: None,
        optimize_budget: 0,
        gpu_offload: false,
        emit_beast_stages: vec![],
        backend: BackendKind::Llvm,
        no_stdlib: false,
        stdlib_path: None,
        disable_plugins: vec![],
        enable_plugins: vec![],
        trg_unresolved_action: TrgUnresolvedAction::Warn,
        extra_objects: vec![],
        shared: false,
        int_bits: 64,
        feature_sso_strings: false,
        feature_svo: false,
        glue_config: None,
        stack_threshold: 4096,
        allow_read: false,
        allow_write: false,
        allow_run: false,
        allow_sys_query: false,
        allow_net: false,
        macro_budget: 0,
        dump_vfs: false,
        update_lockfile: false,
        dump_traces: false,
        diff_mode: false,
        sysquery_overrides: HashMap::new(),
        target: None,
        sysquery_pairs: vec![],
        sysquery_files: vec![],
        style_css: None,
        view_html: None,
        view_bindings: vec![],
        ssr: false,
        dev: false,
    };
    let (_items, _universe) = parse_and_check(file_path, source, &default_opts)?;
    println!("OK");
    Ok(())
}

/// Build the plugin manager for a given file and opts.
/// 2026-07-15: Phase 2 — Discovers system plugins, applies per-extension
/// filtering, and applies CLI overrides. The caller then runs stages at
/// the appropriate pipeline points.
fn build_plugin_manager(file_path: &str, opts: &BuildOptions) -> PluginManager {
    let mut pm = PluginManager::new();

    // Discover system plugins from plugins/{front,mid,post,back}/
    discover_system_plugins(&mut pm);

    // Register built-in Rust plugins
    pm.register(Box::new(brief_compiler::plugin::env_plugin::EnvPlugin));
    pm.register(Box::new(brief_compiler::plugin::print_plugin::PrintPlugin));

    // Apply per-extension filtering from config/targets.toml
    let ext = get_extension(file_path);
    let config = load_target_config(opts);
    pm.filter_for_extension(&ext, &config);

    // Apply CLI overrides
    if !opts.enable_plugins.is_empty() {
        pm = pm.with_enabled_only(opts.enable_plugins.clone());
    }
    if !opts.disable_plugins.is_empty() {
        pm = pm.with_disabled(opts.disable_plugins.clone());
    }
    // --no-std is equivalent to --disable-plugin prelude
    if opts.no_stdlib {
        pm = pm.with_disabled(vec!["prelude".to_string()]);
    }

    // Apply sandbox from CLI flags
    use brief_compiler::macros::eval::Sandbox;
    let sandbox = Sandbox {
        allow_read: opts.allow_read,
        allow_write: opts.allow_write,
        allow_run: opts.allow_run,
        allow_sys_query: opts.allow_sys_query,
        allow_net: opts.allow_net,
        budget: opts.macro_budget,
        remaining: opts.macro_budget,
        sysquery_overrides: opts.sysquery_overrides.clone(),
    };
    pm = pm.with_sandbox(sandbox);

    pm
}

/// Code generation: dispatch to the selected backend, run Post/Back
/// plugin IR stages, and return (output_text, extension).
/// 2026-07-15: Phase 2 — Extracted from compile_source for flat flow.
fn codegen(
    items: &[brief_compiler::ast::TopLevel],
    universe: &mut TypeUniverse,
    pm: &PluginManager,
    opts: &BuildOptions,
    alloc_strategies: std::collections::HashMap<usize, brief_compiler::backend::llvm::AllocStrategy>,
    resolved_frgns: std::collections::HashMap<String, brief_compiler::analysis::frgn_dispatch::ResolvedFrgn>,
    enable_module_init: bool,
) -> Result<(String, &'static str), String> {
    // 2026-07-20: Extract operator definitions from AST for backend dispatch.
    let mut operator_defs: std::collections::HashMap<String, Vec<brief_compiler::ast::top::OperatorDef>> = std::collections::HashMap::new();
    for item in items.iter() {
        if let brief_compiler::ast::TopLevel::TypeDef(td) = item {
            if !td.body.operators.is_empty() {
                operator_defs.insert(td.name.clone(), td.body.operators.clone());
            }
        }
    }

    let output;
    let ext: &str = match opts.backend {
        BackendKind::Llvm => {
            let mut b = LlvmBackend::new()
                .with_int_bits(opts.int_bits)
                .with_alloc_strategies(alloc_strategies)
                .with_sso_strings(opts.feature_sso_strings)
                .with_svo(opts.feature_svo)
                .with_shared_lib(opts.shared)
                .with_stack_threshold(opts.stack_threshold)
                .with_optimize_budget(opts.optimize_budget)
                .with_type_universe(universe.clone())
                .with_operator_defs(operator_defs)
                .with_resolved_frgns(resolved_frgns.clone())
                .with_trg_unresolved_action(opts.trg_unresolved_action)
                .with_module_init(enable_module_init);
            if opts.gpu_offload {
                b = b.with_gpu_offload(true);
                b = b.with_svo(opts.feature_svo);
            }
            // Apply target config if available
            let ext = get_extension(&opts.file_path);
            let target_config = load_target_config(opts);
            if let Some(entry) = target_config.lookup(&ext) {
                if let Some(ref triple) = entry.target_triple {
                    b = b.with_target_triple(triple);
                }
                if let Some(ref dl) = entry.data_layout {
                    b = b.with_data_layout(dl);
                }
            }
            output = b.generate(items, None);
            ".ll"
        }
        BackendKind::Webstack => {
            // 2026-07-26: Phase 4 — Webstack uses LlvmBackend(wasm32) + with_webstack().
            // The old TS emitter path is deprecated. Phase 6 will also invoke
            // GlueWebGenerator to produce the JS shim from view bindings.
            // Phase 5: Extension is .ll — compile_wasm will produce .wasm from it.
            let mut b = LlvmBackend::new()
                .with_webstack(true)
                .with_int_bits(32)
                .with_target_triple("wasm32-unknown-wasi")
                .with_type_universe(universe.clone())
                .with_alloc_strategies(alloc_strategies)
                .with_stack_threshold(opts.stack_threshold)
                .with_optimize_budget(opts.optimize_budget)
                .with_resolved_frgns(resolved_frgns)
                .with_optimize_report(true);
            if opts.gpu_offload {
                b = b.with_gpu_offload(true);
                b = b.with_svo(opts.feature_svo);
            }
            // Apply target config if available
            let ext = get_extension(&opts.file_path);
            let target_config = load_target_config(opts);
            if let Some(entry) = target_config.lookup(&ext) {
                if let Some(ref triple) = entry.target_triple {
                    b = b.with_target_triple(triple);
                }
                if let Some(ref dl) = entry.data_layout {
                    b = b.with_data_layout(dl);
                }
            }
            output = b.generate(items, None);
            ".ll"
        }
        BackendKind::Circt => {
            let mut b = brief_compiler::backend::circt::CirctBackend::new();
            output = b.generate(items);
            ".mlir"
        }
        BackendKind::Webstack => {
            // 2026-07-26: Phase 4 — Webstack uses LlvmBackend(wasm32) + with_webstack().
            // The old TS emitter path is deprecated. Phase 6 will also invoke
            // GlueWebGenerator to produce the JS shim from view bindings.
            // Phase 5: Extension is .ll — compile_wasm will produce .wasm from it.
            let mut b = LlvmBackend::new()
                .with_webstack(true)
                .with_int_bits(32)
                .with_target_triple("wasm32-unknown-wasi")
                .with_type_universe(universe.clone())
                .with_alloc_strategies(alloc_strategies)
                .with_stack_threshold(opts.stack_threshold)
                .with_optimize_budget(opts.optimize_budget)
                .with_resolved_frgns(resolved_frgns)
                .with_optimize_report(true);
            if opts.gpu_offload {
                b = b.with_gpu_offload(true);
            }
            output = b.generate(items, None);
            ".ll"
        }
        BackendKind::Gpu => {
            let mut b = LlvmBackend::new()
                .with_int_bits(opts.int_bits)
                .with_alloc_strategies(alloc_strategies)
                .with_sso_strings(opts.feature_sso_strings)
                .with_svo(opts.feature_svo)
                .with_shared_lib(opts.shared)
                .with_stack_threshold(opts.stack_threshold)
                .with_optimize_budget(opts.optimize_budget)
                .with_type_universe(universe.clone())
                .with_resolved_frgns(resolved_frgns)
                .with_trg_unresolved_action(opts.trg_unresolved_action);
            if opts.gpu_offload {
                b = b.with_gpu_offload(true);
            }
            // Apply target config (same logic as Llvm)
            let ext = get_extension(&opts.file_path);
            let target_config = load_target_config(opts);
            if let Some(entry) = target_config.lookup(&ext) {
                if let Some(ref triple) = entry.target_triple {
                    b = b.with_target_triple(triple);
                }
                if let Some(ref dl) = entry.data_layout {
                    b = b.with_data_layout(dl);
                }
            }
            output = b.generate(items, None);
            ".ll"
        }
        BackendKind::Spirv => {
            // 2026-07-15: SPIR-V backend compiles kernels to binary
            let binary = brief_compiler::backend::spirv::compile_spirv(items, "main")?;
            let out = determine_out_path(&opts.file_path, opts.out_dir.as_deref())?;
            let out_path = out.replace(".ll", ".spv");
            std::fs::write(&out_path, &binary)
                .map_err(|e| format!("cannot write '{}': {}", out_path, e))?;
            println!("wrote {}", out_path);
            output = String::new();
            ".spv"
        }
        BackendKind::Vm => {
            // 2026-07-25: VM backend emits .lair bytecode
            let mut b = brief_compiler::backend::vm::VmBackend::new();
            let lair_data = b.generate(items, universe);
            let out = determine_out_path(&opts.file_path, opts.out_dir.as_deref())?;
            let out_path = out.replace(".ll", ".lair");
            std::fs::write(&out_path, &lair_data)
                .map_err(|e| format!("cannot write '{}': {}", out_path, e))?;
            println!("wrote {}", out_path);
            output = String::new();
            ".lair"
        }
    };

    Ok((output, ext))
}

/// Write a BEAST snapshot at the given pipeline stage and position.
fn emit_beast_snapshot(
    file_path: &str,
    stage: BeastStage,
    position: BeastPosition,
    items: &[brief_compiler::ast::TopLevel],
    universe: &TypeUniverse,
    opts: &BuildOptions,
) -> Result<(), String> {
    // Check if this (stage, position) pair is requested
    let is_requested = opts.emit_beast_stages.iter().any(|f| {
        f.stage == stage && (f.position.is_none() || f.position == Some(position))
    });
    if !is_requested {
        return Ok(());
    }
    let (stage_name, is_ast) = match stage {
        BeastStage::Parse => ("parse", true),
        BeastStage::Resolve => ("resolve", true),
        BeastStage::TypeCheck => ("types", true),
        BeastStage::Normalize => ("normal", true),
        BeastStage::Verify => ("verify", true),
        BeastStage::Alloc => ("alloc", true),
        BeastStage::Provenance => ("prov", true),
        BeastStage::Codegen => ("codegen", false),
        BeastStage::Optimize => ("opt", false),
    };
    let ext = if is_ast { "beast" } else { "ir" };
    let data = brief_compiler::beast::to_beast(items, universe);
    let base = file_path.strip_suffix(".bv").unwrap_or(file_path);
    let priority = position.priority();
    let path = format!("{}.{}.{:03}.{}", base, stage_name, priority, ext);
    std::fs::write(&path, &data)
        .map_err(|e| format!("cannot write '{}': {}", path, e))?;
    eprintln!("wrote {} snapshot: {}", ext, path);
    Ok(())
}

/// Compile a source file up to the $(Typed) stage.
/// Returns items and universe at the Typed stage, ready for beastpack serialization.
/// Used by `briefc bounty` — additive new function, no existing paths modified.
pub fn compile_to_typed(file_path: &str, source: &str, opts: &BuildOptions) -> Result<(Vec<TopLevel>, TypeUniverse), String> {
    let mut pm = build_plugin_manager(file_path, opts);
    let project_root = std::env::current_dir()
        .map_err(|e| format!("cannot determine project root: {}", e))?;
    let project_root_str = project_root.to_string_lossy().to_string();
    if opts.update_lockfile {
        let granted = brief_compiler::macros::lockfile::cli_granted_set(
            opts.allow_read, opts.allow_write, opts.allow_run,
            opts.allow_sys_query, opts.allow_net,
        );
        let lock = brief_compiler::macros::lockfile::generate_lockfile(&granted, None)?;
        brief_compiler::macros::lockfile::save_lockfile(&project_root_str, &lock)?;
    } else if let Some(lock) = brief_compiler::macros::lockfile::load_lockfile(&project_root_str)? {
        brief_compiler::macros::lockfile::validate_and_apply(&lock, &mut pm, None)?;
    }
    let mut source = source.to_string();
    pm.run_source(StageKind::PreLex, &mut source)?;
    let tokens = lex(&source)?;
    let mut items = parse(file_path, &tokens, &source)?;
    extract_inline_stage_blocks(&mut items, &mut pm);
    {
        let mut eval_universe = TypeUniverse::new();
        evaluate_pending_comptime(&mut pm, &mut items, &mut eval_universe)?;
    }
    {
        let mut parsed_universe = TypeUniverse::new();
        pm.run_ast(StageKind::Parsed, &mut items, &mut parsed_universe)?;
    }
    let mut resolver = brief_compiler::import_resolver::ImportResolver::new();
    if let Some(ref stdlib_path) = opts.stdlib_path {
        resolver = resolver.with_stdlib_path(Some(std::path::PathBuf::from(stdlib_path)));
    }
    items = resolver.resolve_imports(items, &std::path::PathBuf::from(file_path))?;
    extract_inline_stage_blocks(&mut items, &mut pm);
    {
        let mut eval_universe = TypeUniverse::new();
        evaluate_pending_comptime(&mut pm, &mut items, &mut eval_universe)?;
    }
    pm.run_ast(StageKind::Resolved, &mut items, &mut TypeUniverse::new())?;
    resolve_comptime_refs(&pm, &mut items)?;
    let mut universe = TypeUniverse::new();
    check_types(&items, &universe)?;
    pm.run_ast(StageKind::Typed, &mut items, &mut universe)?;
    Ok((items, universe))
}

/// Determine the output `.ll` file path from the input path and optional output directory.
fn determine_out_path(file_path: &str, out_dir: Option<&str>) -> Result<String, String> {
    let p = Path::new(file_path);
    let base = p.file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| format!("cannot determine output path from '{}'", file_path))?;

    let parent = match out_dir {
        Some(dir) => dir.trim_end_matches('/').to_string(),
        None => p.parent()
            .map(|d| d.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".to_string()),
    };

    Ok(format!("{}/{}.ll", parent, base))
}

/// 2026-07-16: P4 — Collect extra object files from ForeignBinding FromSpec paths.
/// Each frgn declaration with a .c/.so/.a/etc. path triggers compilation or direct
/// inclusion. The resolver is used to resolve compiler-relative <name> paths.
fn collect_extra_objects(items: &[brief_compiler::ast::TopLevel], resolver: &brief_compiler::import_resolver::ImportResolver) -> Result<Vec<PathBuf>, String> {
    let cache_dir = get_ffi_cache_dir();
    let mut objects = Vec::new();
    for item in items {
        let fb = match item {
            brief_compiler::ast::TopLevel::ForeignBinding(fb) => fb,
            _ => continue,
        };
        let ext = fb.from.extension();
        // 2026-07-26: Check registry directory first for <name> lookups,
        // then fall back to stdlib path, then use the name as a direct path.
        let resolved_path = || -> PathBuf {
            let from_str = fb.from.as_str();
            // Check registry for CompilerRegistry entries (<name>)
            if let brief_compiler::ast::top::FromSpec::CompilerRegistry(_) = &fb.from {
                if let Some(reg_path) = brief_compiler::registry::find_registry_entry(&from_str) {
                    return reg_path;
                }
            }
            resolver.resolve_stdlib_relative_path(&from_str)
                .unwrap_or_else(|| PathBuf::from(from_str))
        };
        match ext.as_deref() {
            Some("c") | Some("cpp") | Some("cc") | Some("cxx") | Some("m") => {
                let src = resolved_path();
                let obj = compile_source_to_object(&src, &cache_dir)?;
                objects.push(obj);
            }
            Some("so") | Some("dylib") | Some("a") | Some("o") => {
                objects.push(resolved_path());
            }
            _ => {}
        }
    }
    Ok(objects)
}

/// 2026-07-16: P4 — Get or create the FFI object cache directory.
fn get_ffi_cache_dir() -> PathBuf {
    let base = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("brief-compiler")
        .join("ffi");
    std::fs::create_dir_all(&base).ok();
    base
}

/// 2026-07-16: P4 — Compile a C/C++ source to a .o object file.
/// Content-hash cached at ~/.cache/brief-compiler/ffi/<hash>.o.
fn compile_source_to_object(source_path: &Path, cache_dir: &Path) -> Result<PathBuf, String> {
    let content = std::fs::read(source_path)
        .map_err(|e| format!("cannot read '{}': {}", source_path.display(), e))?;
    // 2026-07-26: Include compiler flags in the cache key so flag changes
    // (e.g. -flto) produce fresh cache entries instead of reusing stale ones.
    let mut hasher = blake3::Hasher::new();
    hasher.update(&content);
    hasher.update(b":flto");
    let hash = hasher.finalize();
    let cache_path = cache_dir.join(format!("{}.o", hash.to_hex()));
    if cache_path.exists() {
        return Ok(cache_path);
    }
    let ext = source_path.extension().and_then(|s| s.to_str()).unwrap_or("");
    let lang_flag = match ext {
        "c" | "m" => "c",
        "cpp" | "cc" | "cxx" => "c++",
        _ => return Err(format!("unknown source extension '{}' for '{}'", ext, source_path.display())),
    };
    let status = Command::new("clang")
        .args([
            "-O3", "-flto", "-march=native", "-ffast-math",
            "-x", lang_flag,
            "-c",
            source_path.to_str().unwrap(),
            "-o", cache_path.to_str().unwrap(),
        ])
        .status()
        .map_err(|e| format!("failed to invoke clang (is it installed?): {}", e))?;
    if !status.success() {
        return Err(format!("clang failed to compile '{}'", source_path.display()));
    }
    Ok(cache_path)
}

/// Compile a `.ll` file to a binary using clang.
///
/// 2026-07-26: Added `protocol_libs` parameter — library names from
/// `from #System` frgns are passed as `-l<lib>` flags to clang.
fn compile_ll_to_binary(ll_path: &str, binary_path: &str, extra_objects: &[PathBuf], protocol_libs: &[String], shared: bool) -> Result<(), String> {
    let mut cmd = Command::new("clang");
    // 2026-07-26: brief_rt.c is no longer hardcoded here — frgn declarations in
    // stdlib (e.g., `frgn __print_int from "lib/runtime/brief_rt.c"`) are compiled
    // by collect_extra_objects and passed via extra_objects. This removes the
    // duplicate symbol error that occurred when brief_rt.c was compiled twice.
    if shared {
        cmd.args(["-O3", "-flto", "-shared", "-fPIC", ll_path]);
    } else {
        cmd.args(["-O3", "-flto", "-march=native", "-ffast-math", ll_path]);
    }
    for obj in extra_objects {
        cmd.arg(obj.as_os_str());
    }
    // 2026-07-26: Link protocol-based libraries (from #System).
    // The clang driver adds these as -l<name> flags to the linker.
    for lib in protocol_libs {
        cmd.arg(format!("-l{}", lib));
    }
    cmd.args(["-o", binary_path, "-lm"]);
    let status = cmd.status()
        .map_err(|e| format!(
            "failed to invoke clang: {} (is clang installed? use --llvm to emit IR only)",
            e
        ))?;

    if !status.success() {
        return Err(format!(
            "clang failed to compile '{}' to binary '{}'",
            ll_path, binary_path,
        ));
    }

    println!("wrote {}", binary_path);
    Ok(())
}

/// Compile LLVM IR (.ll) to WASM binary (.wasm) using llc.
/// 2026-07-26: Phase 5 — Called for BackendKind::Webstack after codegen.
/// The .ll file must have been emitted with wasm32 target triple.
/// Uses `llc -march=wasm32 -filetype=obj` to produce a .o, then
/// `wasm-ld` to link into .wasm. This avoids needing a wasm32 clang.
fn compile_wasm(ll_path: &str, wasm_path: &str) -> Result<(), String> {
    // Step 1: compile .ll to .wasm object file
    let obj_path = format!("{}.o", wasm_path);
    let mut assemble = Command::new("llc");
    assemble.args(["-march=wasm32", "-filetype=obj", ll_path, "-o", &obj_path]);
    let status = assemble.status()
        .map_err(|e| format!(
            "failed to invoke llc: {} (install llvm-tools or use --emit-ir-only)",
            e
        ))?;
    if !status.success() {
        return Err(format!("llc failed to compile '{}' to WASM object", ll_path));
    }
    // Step 2: link .o to .wasm
    let mut link = Command::new("wasm-ld");
    link.args(["--no-entry", "--allow-undefined", "-o", wasm_path, &obj_path]);
    let status = link.status()
        .map_err(|e| format!(
            "failed to invoke wasm-ld: {} (install wasm-ld or use --emit-ir-only)",
            e
        ))?;
    if !status.success() {
        let _ = std::fs::remove_file(&obj_path);
        return Err(format!("wasm-ld failed to link '{}'", wasm_path));
    }
    // Clean up intermediate object
    let _ = std::fs::remove_file(&obj_path);
    println!("wrote {}", wasm_path);
    Ok(())
}

/// Lex + parse + resolve imports + typecheck, returning items and universe.
fn parse_and_check(file_path: &str, source: &str, opts: &BuildOptions) -> Result<(Vec<brief_compiler::ast::TopLevel>, TypeUniverse), String> {
    let tokens = lex(source)?;
    let items = parse(file_path, &tokens, source)?;

    let mut resolver = brief_compiler::import_resolver::ImportResolver::new();
    if let Some(ref stdlib_path) = opts.stdlib_path {
        resolver = resolver.with_stdlib_path(Some(std::path::PathBuf::from(stdlib_path)));
    }
    let items = resolver.resolve_imports(items, &std::path::PathBuf::from(file_path))?;

    let universe = TypeUniverse::new();
    check_types(&items, &universe)?;
    Ok((items, universe))
}

/// Load TargetConfig, respecting --config-dir when set in opts.
/// 2026-07-16: P1 — Runtime config directory overrides compile-time baked.
fn load_target_config(opts: &BuildOptions) -> TargetConfig {
    match &opts.config_dir {
        Some(dir) => {
            let path = Path::new(dir).join("targets.toml");
            TargetConfig::load_from(&path).unwrap_or_else(|e| {
                eprintln!("warning: cannot load '{}': {} — using baked fallback", path.display(), e);
                TargetConfig::load()
            })
        }
        None => TargetConfig::load(),
    }
}

/// Lex the source into tokens with source spans.
/// 2026-07-16: Fixed — use actual spans from logos instead of 0..0.
/// The spans are needed by read_layout_body for byte-level slicing.
fn lex(source: &str) -> Result<Vec<(Token, std::ops::Range<usize>)>, String> {
    use logos::Logos;
    let mut lexer = Token::lexer(source);
    let mut tokens = Vec::new();
    while let Some(result) = lexer.next() {
        let token = result.map_err(|_| "lex error".to_string())?;
        let span = lexer.span();
        tokens.push((token, span));
    }
    Ok(tokens)
}

/// Parse tokens into an AST.
fn parse(file_path: &str, tokens: &[(Token, std::ops::Range<usize>)], source: &str) -> Result<Vec<brief_compiler::ast::TopLevel>, String> {
    let mut parser = brief_compiler::parser::Parser::new(tokens.to_vec(), source);
    parser.parse_program().map_err(|e| format!("{}: parse error: {}", file_path, e))
}

/// 2026-07-20: Validate type parameter bounds (K: #String, V: #Float).
/// Checks that types declaring bounded type params have at least one
/// operator referencing the bound hashword in their params.
fn validate_constraints(items: &[brief_compiler::ast::TopLevel]) -> Result<(), String> {
    for item in items {
        let brief_compiler::ast::TopLevel::TypeDef(td) = item else { continue; };
        for tp in &td.type_params {
            let brief_compiler::ast::top::TypeParam { name, bound: Some(bound) } = tp else { continue; };
            let bound_category = match bound {
                brief_compiler::ast::Type::HashWord(c) => c.as_str(),
                brief_compiler::ast::Type::HashWordVariant(c, _) => c.as_str(),
                _ => continue,
            };
            // Check at least one operator references this hashword in its params
            let has_op = td.body.operators.iter().any(|op| {
                op.params.iter().any(|p| {
                    matches!(p,
                        brief_compiler::ast::Type::HashWord(c) if c == bound_category
                    ) || matches!(p,
                        brief_compiler::ast::Type::HashWordVariant(c, _) if c == bound_category
                    )
                })
            });
            if !has_op {
                return Err(format!(
                    "constraint '{}: {}' in type '{}' is unsatisfiable — \
                     no operator references {} in its parameters. \
                     Add an op declaration like 'op ...({}, ...)' to use this constraint.",
                    name, bound, td.name, bound, bound
                ));
            }
        }
    }
    Ok(())
}

/// Type-check the program against a TypeUniverse.
fn check_types(items: &[brief_compiler::ast::TopLevel], universe: &TypeUniverse) -> Result<(), String> {
    validate_constraints(items)?;
    brief_compiler::typechecker::check_program(items, universe)
        .map_err(|errors| {
            let msgs: Vec<String> = errors.iter().map(|e| format!("{}", e)).collect();
            format!("type errors:\n  {}", msgs.join("\n  "))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a temporary file with given content, run a function on its path.
    fn with_temp_file<F>(content: &str, f: F)
    where F: FnOnce(&Path)
    {
        let dir = std::env::temp_dir().join("brief_compile_test");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join(format!("test_{}.c", std::process::id()));
        std::fs::write(&path, content).ok();
        f(&path);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_compile_source_to_object_cached() {
        let content = "int foo() { return 42; }";
        with_temp_file(content, |path| {
            let cache_dir = get_ffi_cache_dir();
            // First compilation
            let result1 = compile_source_to_object(path, &cache_dir);
            assert!(result1.is_ok(), "first compile failed: {:?}", result1);
            let obj1 = result1.unwrap();
            assert!(obj1.exists(), "object file not created");
            // Same source → same hash → returns cached path (identical)
            let result2 = compile_source_to_object(path, &cache_dir);
            assert!(result2.is_ok(), "second compile failed: {:?}", result2);
            let obj2 = result2.unwrap();
            assert_eq!(obj1, obj2, "cached path should match");
        });
    }

    #[test]
    fn test_get_ffi_cache_dir_creates_dir() {
        let dir = get_ffi_cache_dir();
        assert!(dir.exists(), "cache directory should be created");
    }

    #[test]
    fn test_compile_source_to_object_bad_ext() {
        let path = Path::new("/tmp/test_bad_ext.xyz");
        std::fs::write(path, "hello").ok();
        let result = compile_source_to_object(path, &get_ffi_cache_dir());
        assert!(result.is_err(), "expected compile error for unknown extension");
        let _ = std::fs::remove_file(path);
    }
}
