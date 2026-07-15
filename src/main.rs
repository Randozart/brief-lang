// ── Brief Compiler CLI Entry Point ────────────────────────────────────
// 2026-07-12: Phase 7 — Clean CLI dispatch.
// Flat code: max 2 nesting. No unqualified unwraps.
// 2026-07-14: Add --llvm, --out, --optimize-budget, --gpu-offload flags to build.

mod compile;
mod library;

use std::env;
use std::path::Path;

use brief_compiler::target::{BackendKind, TargetConfig, get_extension};

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
        "init" => run_init(args.get(2).map(|s| s.as_str())),
        "help" | "--help" | "-h" => { print_usage(&args[0]); Ok(()) }
        _ => {
            // Default: compile the file
            if args[1].ends_with(".bv") || args[1].ends_with(".rbv") {
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
    eprintln!("  {} build <file.bv>              Compile a Brief source file", name);
    eprintln!("  {} build <file.bv> --llvm        Emit LLVM IR only, no binary", name);
    eprintln!("  {} build <file.bv> --out <dir>   Set output directory", name);
    eprintln!("  {} build <file.bv> --backend <name>  Select backend: llvm, circt, webstack, gpu", name);
    eprintln!("  {} build <file.bv> --no-std          Disable stdlib auto-import", name);
    eprintln!("  {} build <file.bv> --stdlib-path <p>   Set stdlib search path", name);
    eprintln!("  {} build <file.bv> --emit-bvir    Write .bvir IR files", name);
    eprintln!("  {} check <file.bv>               Type-check only", name);
    eprintln!("  {} derive <file.bv>              Synthesize derivation blocks", name);
    eprintln!("  {} library <file.bv>             Compile to .a library", name);
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
///   --plugin <path>         add a plugin executable to the chain
///   --emit-bvir             write .bvir files before/after plugins
///   --backend <name>        select backend (llvm, circt, webstack, gpu)
fn parse_build_args(args: &[String]) -> Result<compile::BuildOptions, String> {
    let mut file_path: Option<String> = None;
    let mut emit_ir_only = false;
    let mut out_dir: Option<String> = None;
    let mut optimize_budget = 256u64;
    let mut gpu_offload = false;
    let mut plugin_paths = Vec::new();
    let mut emit_bvir = false;
    let mut backend_override: Option<String> = None;
    let mut no_stdlib = false;
    let mut stdlib_path: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--llvm" {
            emit_ir_only = true;
            i += 1;
        } else if arg == "--gpu-offload" {
            gpu_offload = true;
            i += 1;
        } else if arg == "--out" {
            let val = args.get(i + 1).ok_or("--out requires a directory argument")?;
            out_dir = Some(val.clone());
            i += 2;
        } else if arg == "--optimize-budget" {
            let val = args.get(i + 1).ok_or("--optimize-budget requires a number argument")?;
            optimize_budget = val.parse()
                .map_err(|_| format!("invalid --optimize-budget value: '{}'", val))?;
            i += 2;
        } else if arg == "--plugin" {
            let path = args.get(i + 1).ok_or("--plugin requires a path argument")?;
            plugin_paths.push(path.clone());
            i += 2;
        } else if arg == "--emit-bvir" {
            emit_bvir = true;
            i += 1;
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
            let config = TargetConfig::load();
            match config.lookup(&ext) {
                Some(entry) => TargetConfig::resolve(&entry.backend)?,
                None => BackendKind::Llvm,
            }
        }
    };

    Ok(compile::BuildOptions {
        file_path,
        emit_ir_only,
        out_dir,
        optimize_budget,
        gpu_offload,
        plugin_paths,
        emit_bvir,
        backend,
        no_stdlib,
        stdlib_path,
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

fn run_derive(args: &[String]) -> Result<(), String> {
    let file_path = args.first().ok_or("missing file argument")?;
    brief_compiler::derive::handle_derive_command(file_path)
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
