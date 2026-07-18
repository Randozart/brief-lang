// ── Brief Compiler CLI Entry Point ────────────────────────────────────
// 2026-07-12: Phase 7 — Clean CLI dispatch.
// Flat code: max 2 nesting. No unqualified unwraps.
// 2026-07-14: Add --llvm, --out, --optimize-budget, --gpu-offload flags to build.

mod compile;
mod library;

use std::env;
use std::path::Path;

use brief_compiler::target::{BackendKind, TargetConfig, get_extension};
use compile::BvirStage;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage(&args[0]);
        return;
    }

    let result = match args[1].as_str() {
        "build" => run_build(&args[2..]),
        "check" => run_check(&args[2..]),
        "derive" => run_derive(&args[2..]),
        "library" | "lib" => library::run_library_mode(&args[2..]),
        "config" => run_config(&args[2..]),
        "init" => run_init(args.get(2).map(|s| s.as_str())),
        "register" => run_register(&args[2..]),
        "help" | "--help" | "-h" => { print_usage(&args[0]); Ok(()) }
        _ => {
            // Default: compile the file
            if args[1].ends_with(".bv") || args[1].ends_with(".rbv") || args[1].ends_with(".abv") {
                run_build(&args[1..])
            } else {
                eprintln!("unknown command: {}", args[1]);
                print_usage(&args[0]);
                Ok(())
            }
        }
    };

    if let Err(e) = result {
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}

fn print_usage(program: &str) {
    let name = Path::new(program).file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("brief-compiler");
    eprintln!("Brief Compiler");
    eprintln!("Usage:");
    eprintln!("  {} build <file.bv>                 Compile a Brief source file", name);
    eprintln!("  {} build <file.bv> --llvm           Emit LLVM IR only, no binary", name);
    eprintln!("  {} build <file.bv> --config-dir <d>  Set config directory", name);
    eprintln!("  {} build <file.bv> --out <dir>      Set output directory", name);
    eprintln!("  {} build <file.bv> --backend <name> Select backend: llvm, circt, webstack, gpu", name);
    eprintln!("  {} build <file.bv> --emit-bvir [ast|mid|post|all]  Emit BVIR snapshots (default: all)", name);
    eprintln!("  {} build <file.bv> --no-std          Disable prelude (equivalent to --disable-plugin prelude)", name);
    eprintln!("  {} build <file.bv> --stdlib-path <p>   Set stdlib search path", name);
    eprintln!("  {} build <file.bv> --disable-plugin <name>  Disable a system plugin by name", name);
    eprintln!("  {} build <file.bv> --enable-plugin <name>   Enable only specific plugins", name);
    eprintln!("  {} check <file.bv>               Type-check only", name);
    eprintln!("  {} derive <file.bv>              Synthesize derivation blocks", name);
    eprintln!("  {} library <file.bv>             Compile to .a library", name);
    eprintln!("  {} config list                   List available config profiles", name);
    eprintln!("  {} config show                   Show active config profile", name);
    eprintln!("  {} config set <name>             Switch to a config profile", name);
    eprintln!("  {} config init <name>            Create a new config profile", name);
    eprintln!("  {} init <name>                   Create a new project", name);
    eprintln!("  {} help                          Show this help", name);
}

/// Parse `build` subcommand arguments into a `BuildOptions`.
///
/// Accepts:
///   <file.bv>               (positional, required)
///   --llvm                  emit IR only, no binary
///   --out <dir>             output directory
///   --optimize-budget <N>   simulation budget (default 256)
///   --gpu-offload           enable GPU offload
///   --backend <name>        select backend (llvm, circt, webstack, gpu)
///   --emit-bvir [stage]     emit BVIR snapshots (ast, mid, post, all; default all)
fn parse_build_args(args: &[String]) -> Result<compile::BuildOptions, String> {
    let mut file_path: Option<String> = None;
    let mut emit_ir_only = false;
    let mut config_dir: Option<String> = None;
    let mut out_dir: Option<String> = None;
    let mut optimize_budget = 256u64;
    let mut gpu_offload = false;
    let mut emit_bvir: Vec<BvirStage> = Vec::new();
    let mut backend_override: Option<String> = None;
    let mut no_stdlib = false;
    let mut stdlib_path: Option<String> = None;
    let mut disable_plugins = Vec::new();
    let mut enable_plugins = Vec::new();
    let mut trg_unresolved_action = compile::TrgUnresolvedAction::Warn;

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--llvm" {
            emit_ir_only = true;
            i += 1;
        } else if arg == "--gpu-offload" {
            gpu_offload = true;
            i += 1;
        } else if arg == "--config-dir" {
            let val = args.get(i + 1).ok_or("--config-dir requires a directory argument")?;
            config_dir = Some(val.clone());
            i += 2;
        } else if arg == "--out" {
            let val = args.get(i + 1).ok_or("--out requires a directory argument")?;
            out_dir = Some(val.clone());
            i += 2;
        } else if arg == "--optimize-budget" {
            let val = args.get(i + 1).ok_or("--optimize-budget requires a number argument")?;
            optimize_budget = val.parse()
                .map_err(|_| format!("invalid --optimize-budget value: '{}'", val))?;
            i += 2;
        } else if arg == "--emit-bvir" {
            // --emit-bvir with optional stage arg: "ast", "mid", "post"
            // If no arg or "all", emit all stages.
            let next = args.get(i + 1);
            let stage_str = next.filter(|s| !s.starts_with('-')).map(|s| s.as_str());
            match stage_str {
                Some("ast") => emit_bvir.push(BvirStage::Ast),
                Some("mid") => emit_bvir.push(BvirStage::Mid),
                Some("post") => emit_bvir.push(BvirStage::Post),
                Some("all") | None => {
                    emit_bvir.push(BvirStage::Ast);
                    emit_bvir.push(BvirStage::Mid);
                    emit_bvir.push(BvirStage::Post);
                }
                Some(other) => return Err(format!("unknown BVIR stage '{}'. Use: ast, mid, post, all", other)),
            }
            i += if stage_str.is_some() { 2 } else { 1 };
        } else if arg == "--backend" {
            let name = args.get(i + 1).ok_or("--backend requires a name argument (llvm, circt, webstack, gpu)")?;
            backend_override = Some(name.clone());
            i += 2;
        } else if arg == "--no-std" {
            no_stdlib = true;
            i += 1;
        } else if arg == "--stdlib-path" {
            let val = args.get(i + 1).ok_or("--stdlib-path requires a path argument")?;
            stdlib_path = Some(val.clone());
            i += 2;
        } else if arg == "--disable-plugin" {
            let name = args.get(i + 1).ok_or("--disable-plugin requires a plugin name argument")?;
            disable_plugins.push(name.clone());
            i += 2;
        } else if arg == "--enable-plugin" {
            let name = args.get(i + 1).ok_or("--enable-plugin requires a plugin name argument")?;
            enable_plugins.push(name.clone());
            i += 2;
        } else if arg == "--warn-unresolved-trg" {
            trg_unresolved_action = compile::TrgUnresolvedAction::Warn;
            i += 1;
        } else if arg == "--error-unresolved-trg" {
            trg_unresolved_action = compile::TrgUnresolvedAction::Error;
            i += 1;
        } else if arg.starts_with('-') {
            return Err(format!("unknown flag: {}", arg));
        } else if file_path.is_some() {
            return Err(format!("unexpected positional argument: '{}'", arg));
        } else {
            file_path = Some(arg.clone());
            i += 1;
        }
    }

    let file_path = file_path.ok_or("missing file argument")?;

    // Resolve backend: CLI override or extension lookup
    let backend = match &backend_override {
        Some(name) => TargetConfig::resolve(name)?,
        None => {
            let ext = get_extension(&file_path);
            let config = load_target_config(config_dir.as_deref());
            match config.lookup(&ext) {
                Some(entry) => TargetConfig::resolve(&entry.backend)?,
                None => BackendKind::Llvm,
            }
        }
    };

    Ok(compile::BuildOptions {
        config_dir,
        file_path,
        emit_ir_only,
        out_dir,
        optimize_budget,
        gpu_offload,
        emit_bvir_stages: emit_bvir,
        backend,
        no_stdlib,
        stdlib_path,
        disable_plugins,
        enable_plugins,
        trg_unresolved_action,
        extra_objects: vec![],
        feature_sso_strings: false,
    })
}

fn run_build(args: &[String]) -> Result<(), String> {
    let opts = parse_build_args(args)?;
    let source = std::fs::read_to_string(&opts.file_path)
        .map_err(|e| format!("cannot read '{}': {}", opts.file_path, e))?;
    compile::compile_source(&opts.file_path, &source, &opts)
}

fn run_check(args: &[String]) -> Result<(), String> {
    let file_path = args.first().ok_or("missing file argument")?;
    let source = std::fs::read_to_string(file_path)
        .map_err(|e| format!("cannot read '{}': {}", file_path, e))?;
    compile::check_source(file_path, &source)
}

/// `brief-compiler register <name>` — register a project/target schema.
/// 2026-07-15: Phase 7 — Stub implementation.
fn run_register(_args: &[String]) -> Result<(), String> {
    eprintln!("register: not yet implemented — schema registration is a future feature");
    Ok(())
}

fn run_derive(args: &[String]) -> Result<(), String> {
    let file_path = args.first().ok_or("missing file argument")?;
    brief_compiler::derive::handle_derive_command(file_path)
}

/// Load TargetConfig with optional --config-dir override.
/// 2026-07-16: P1 — Respects runtime config directory when set.
fn load_target_config(config_dir: Option<&str>) -> TargetConfig {
    match config_dir {
        Some(dir) => {
            let path = std::path::Path::new(dir).join("targets.toml");
            TargetConfig::load_from(&path).unwrap_or_else(|e| {
                eprintln!("warning: cannot load '{}': {} — using baked fallback", path.display(), e);
                TargetConfig::load()
            })
        }
        None => TargetConfig::load(),
    }
}

/// `brief-compiler config <subcommand>` — manage config profiles.
/// Subcommands: list, show, set <name>, init <name>
fn run_config(args: &[String]) -> Result<(), String> {
    let sub = args.first().map(|s| s.as_str()).unwrap_or("show");
    match sub {
        "list" => {
            let profiles = brief_compiler::config_resolver::list_profiles()?;
            if profiles.is_empty() {
                println!("no profiles configured");
            } else {
                println!("available profiles:");
                for p in &profiles {
                    println!("  {}", p);
                }
            }
            Ok(())
        }
        "show" => brief_compiler::config_resolver::show_active_profile(),
        "set" => {
            let name = args.get(1).ok_or("usage: brief-compiler config set <profile-name>")?;
            brief_compiler::config_resolver::set_active_profile(name)
        }
        "init" => {
            let name = args.get(1).ok_or("usage: brief-compiler config init <profile-name>")?;
            brief_compiler::config_resolver::init_profile(name)
        }
        _ => Err(format!("unknown config subcommand '{}'. Use: list, show, set <name>, init <name>", sub)),
    }
}

fn run_init(name: Option<&str>) -> Result<(), String> {
    let name = name.unwrap_or("my_project");
    let dir = Path::new(name);
    std::fs::create_dir_all(dir.join("src"))
        .map_err(|e| format!("cannot create project: {}", e))?;
    let main_bv = format!(r#"defn main() -> Int [#] {{
    term 0;
}};
"#);
    std::fs::write(dir.join("src").join("main.bv"), main_bv)
        .map_err(|e| format!("cannot write main.bv: {}", e))?;
    println!("Created project '{}'", name);
    Ok(())
}
