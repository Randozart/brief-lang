// ── Brief Compiler CLI Entry Point ────────────────────────────────────
// 2026-07-12: Phase 7 — Clean CLI dispatch.
// Flat code: max 2 nesting. No unqualified unwraps.

mod compile;
mod library;

use std::env;
use std::path::Path;

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
    eprintln!("  {} build <file.bv>     Compile a Brief source file", name);
    eprintln!("  {} check <file.bv>     Type-check only", name);
    eprintln!("  {} derive <file.bv>    Synthesize derivation blocks", name);
    eprintln!("  {} library <file.bv>   Compile to .a library", name);
    eprintln!("  {} init <name>         Create a new project", name);
    eprintln!("  {} help                Show this help", name);
}

fn run_build(args: &[String]) -> Result<(), String> {
    let file_path = args.first().ok_or("missing file argument")?;
    let source = std::fs::read_to_string(file_path)
        .map_err(|e| format!("cannot read '{}': {}", file_path, e))?;
    compile::compile_source(file_path, &source)
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
