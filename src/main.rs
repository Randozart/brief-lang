use brief_compiler::{
    annotator, ast, desugarer, errors, import_resolver, interpreter, lsp, manifest, parser,
    proof_engine, rbv, typechecker, view_compiler, wasm_gen,
};
use notify::Watcher;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

fn format_type_errors(errors: &[typechecker::TypeError], file_name: &str) -> String {
    let mut output = String::new();
    for err in errors {
        match err {
            typechecker::TypeError::UndefinedVariable { name, available } => {
                output.push_str(&format!(
                    "error[B001]: undefined variable '{}'\n --> {}:?:?\n  |\n",
                    name, file_name
                ));
                if !available.is_empty() {
                    output.push_str(&format!(
                        "  = available variables: {}\n",
                        available.join(", ")
                    ));
                }
            }
            typechecker::TypeError::TypeMismatch {
                expected,
                found,
                context,
            } => {
                output.push_str(&format!(
                    "error[B002]: type mismatch\n --> {}:?:?\n  |\n",
                    file_name
                ));
                output.push_str(&format!(
                    "  = expected {} for {}, but found {}\n",
                    expected, context, found
                ));
            }
            typechecker::TypeError::UninitializedSignal { name } => {
                output.push_str(&format!(
                    "error[B003]: uninitialized signal\n --> {}:?:?\n  |\n",
                    file_name
                ));
                output.push_str(&format!("  = signal '{}' has no initial value\n", name));
                output.push_str(&format!(
                    "  = hint: provide an initial value like let {}: Int = 0;\n",
                    name
                ));
            }
            typechecker::TypeError::OwnershipViolation { var, reason } => {
                output.push_str(&format!(
                    "error[B004]: ownership violation\n --> {}:?:?\n  |\n",
                    file_name
                ));
                output.push_str(&format!("  = {}: {}\n", var, reason));
            }
            typechecker::TypeError::InvalidOperation {
                operation,
                type_name,
            } => {
                output.push_str(&format!(
                    "error[B005]: invalid operation\n --> {}:?:?\n  |\n",
                    file_name
                ));
                output.push_str(&format!(
                    "  = cannot perform '{}' on type {}\n",
                    operation, type_name
                ));
            }
        }
        output.push('\n');
    }
    output
}

fn format_proof_errors(errors: &[proof_engine::ProofError], file_name: &str) -> String {
    let mut output = String::new();
    for err in errors {
        output.push_str(&format!(
            "error[{}]: {}\n --> {}:?:?\n",
            err.code, err.title, file_name
        ));
        if !err.explanation.is_empty() {
            output.push_str(&format!("  |\n  = {}\n", err.explanation));
        }
        if !err.proof_chain.is_empty() {
            output.push_str("  |\n  = proof:\n");
            for step in &err.proof_chain {
                output.push_str(&format!("  =   • {}\n", step));
            }
        }
        if !err.examples.is_empty() {
            output.push_str("  |\n  = example failure:\n");
            for ex in &err.examples {
                output.push_str(&format!("  =   {}\n", ex));
            }
        }
        if !err.hints.is_empty() {
            output.push_str("  |\n  = hint:");
            for hint in &err.hints {
                output.push_str(&format!(" {}\n", hint));
            }
        }
        output.push('\n');
    }
    output
}

fn strip_annotations(source: &str) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let mut output = Vec::new();
    let mut in_block = false;

    for line in lines {
        if line.contains("=== PATH ANALYSIS ===") {
            in_block = true;
            continue;
        }
        if line.contains("=== END PATH ANALYSIS ===") {
            in_block = false;
            continue;
        }
        if in_block {
            continue;
        }
        output.push(line);
    }

    while output.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
        output.pop();
    }

    output.join("\n")
}

fn print_usage(program: &str) {
    eprintln!("Brief Compiler v{}", env!("CARGO_PKG_VERSION"));
    eprintln!();
    eprintln!("Usage: {} <command> [options] [file]", program);
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  check <file>     Type check without execution (fast)");
    eprintln!("  build <file>     Full compilation");
    eprintln!("  init [name]      Create new project");
    eprintln!("  import <name>    Add dependency to project");
    eprintln!("  serve [dir]      Serve static files (default: .)");
    eprintln!("  rbv <file>       Compile RBV to browser-ready files");
    eprintln!("  lsp              Start Language Server (for IDE integration)");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -a, --annotate       Generate path annotations");
    eprintln!("  --skip-proof         Skip proof verification");
    eprintln!("  -v, --verbose        Verbose output");
    eprintln!("  --quiet, --whisper   Minimal output (for CI/automated use)");
    eprintln!("  -h, --help           Show this help");
}

fn run_check(
    file_path: &PathBuf,
    verbose: bool,
    annotate: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let source = fs::read_to_string(file_path)?;
    let clean_source = strip_annotations(&source);

    if verbose {
        println!("[Lexer] Tokenizing...");
    }

    let mut parser = parser::Parser::new(&clean_source);
    let program = match parser.parse() {
        Ok(prog) => prog,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            return Err("Parse error".into());
        }
    };

    if verbose {
        println!("[Resolver] Resolving imports...");
    }
    let mut import_resolver = import_resolver::ImportResolver::new();
    let program = match import_resolver.resolve_imports(&program, file_path) {
        Ok(resolved) => resolved,
        Err(e) => {
            eprintln!("Import error: {}", e);
            return Err("Import error".into());
        }
    };

    if verbose {
        println!("[Desugarer] Processing sugared syntax...");
    }
    let mut desug = desugarer::Desugarer::new();
    let program = desug.desugar(&program);

    if verbose {
        println!("[Parser] Successfully parsed {} items", program.items.len());
        println!("[TypeChecker] Running type checks...");
    }

    let mut tc = typechecker::TypeChecker::new();
    let type_errors = tc.check_program(&program);
    if !type_errors.is_empty() {
        eprintln!(
            "{}",
            format_type_errors(&type_errors, file_path.to_str().unwrap_or("main.bv"))
        );
        return Err("Type errors".into());
    }
    if verbose {
        println!("[TypeChecker] No type errors");
    }

    if verbose {
        println!("[ProofEngine] Running proof verification...");
    }
    let mut pe = proof_engine::ProofEngine::new();
    let proof_errors = pe.verify_program(&program);
    if !proof_errors.is_empty() {
        eprintln!(
            "{}",
            format_proof_errors(&proof_errors, file_path.to_str().unwrap_or("main.bv"))
        );
        return Err("Proof errors".into());
    }
    if verbose {
        println!("[ProofEngine] All proofs verified");
    }

    if annotate {
        if verbose {
            println!("[Annotator] Computing call paths...");
        }
        let mut ann = annotator::Annotator::new();
        ann.analyze(&program);
        let annotated = ann.annotate_program(&program);
        println!("\n// === ANNOTATED PROGRAM ===\n");
        println!("{}", annotated);
        println!("// === END ANNOTATED PROGRAM ===");
    }

    println!("✓ All checks passed");
    Ok(())
}

fn run_build(file_path: &PathBuf, verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    let source = fs::read_to_string(file_path)?;
    let clean_source = strip_annotations(&source);

    if verbose {
        println!("[Lexer] Tokenizing...");
    }

    let mut parser = parser::Parser::new(&clean_source);
    let program = match parser.parse() {
        Ok(prog) => prog,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            return Err("Parse error".into());
        }
    };

    if verbose {
        println!("[Resolver] Resolving imports...");
    }
    let mut import_resolver = import_resolver::ImportResolver::new();
    let program = match import_resolver.resolve_imports(&program, file_path) {
        Ok(resolved) => resolved,
        Err(e) => {
            eprintln!("Import error: {}", e);
            return Err("Import error".into());
        }
    };

    if verbose {
        println!("[Desugarer] Processing sugared syntax...");
    }
    let mut desug = desugarer::Desugarer::new();
    let program = desug.desugar(&program);

    if verbose {
        println!("[Parser] Successfully parsed {} items", program.items.len());
        println!("[TypeChecker] Running type checks...");
    }

    let mut tc = typechecker::TypeChecker::new();
    let type_errors = tc.check_program(&program);
    if !type_errors.is_empty() {
        eprintln!(
            "{}",
            format_type_errors(&type_errors, file_path.to_str().unwrap_or("main.bv"))
        );
        return Err("Type errors".into());
    }

    if verbose {
        println!("[ProofEngine] Running proof verification...");
    }
    let mut pe = proof_engine::ProofEngine::new();
    let proof_errors = pe.verify_program(&program);
    if !proof_errors.is_empty() {
        eprintln!(
            "{}",
            format_proof_errors(&proof_errors, file_path.to_str().unwrap_or("main.bv"))
        );
        return Err("Proof errors".into());
    }

    if verbose {
        println!("[Interpreter] Running program...");
    }

    let mut interp = interpreter::Interpreter::new();
    match interp.run(&program) {
        Ok(_) => {
            if verbose {
                println!("[Interpreter] Final state: {:?}", interp.state);
            }
            println!("Execution completed successfully");
        }
        Err(e) => {
            eprintln!("Runtime error: {:?}", e);
            return Err("Runtime error".into());
        }
    }

    Ok(())
}

fn run_init(name: Option<&str>, verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    let project_name = name.unwrap_or("my-brief-project").to_string();
    let project_dir = PathBuf::from(&project_name);

    if project_dir.exists() {
        eprintln!("Error: Directory '{}' already exists", project_name);
        return Err("Directory exists".into());
    }

    if verbose {
        println!("Creating project '{}'...", project_name);
    }

    std::fs::create_dir_all(project_dir.join("lib"))?;

    let manifest_content = format!(
        r#"[project]
name = "{}"
version = "0.1.0"
entry = "main.bv"

[dependencies]
"#,
        project_name
    );

    std::fs::write(project_dir.join("brief.toml"), manifest_content)?;

    let main_content = r#"# Welcome to Brief!
# Your main entry point

let ready: Bool = false;

rct txn init [true][ready == true] {
  &ready = true;
  term;
};
"#;

    std::fs::write(project_dir.join("main.bv"), main_content)?;

    if verbose {
        println!("Created project structure:");
        println!("  {}/", project_name);
        println!("  {}/brief.toml", project_name);
        println!("  {}/main.bv", project_name);
        println!("  {}/lib/", project_name);
    }

    println!("✓ Project '{}' created successfully", project_name);
    println!("  Run: cd {} && brief check main.bv", project_name);

    Ok(())
}

fn run_import(
    name: &str,
    path: Option<&str>,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let manifest_path = manifest::find_manifest(&std::env::current_dir()?)
        .ok_or("No brief.toml found. Run 'brief init' first.")?;

    if verbose {
        println!("Found manifest at: {}", manifest_path.display());
    }

    let mut manifest = manifest::Manifest::load(&manifest_path)?;

    let dep_path: PathBuf = if let Some(p) = path {
        PathBuf::from(p)
    } else {
        let search_paths = ["lib", "imports", "."];
        let file_name = format!("{}.bv", name);

        let project_root = manifest_path.parent().unwrap_or(std::path::Path::new("."));

        let mut found = None;
        for search_dir in &search_paths {
            let candidate = project_root.join(search_dir).join(&file_name);
            if candidate.exists() {
                found = Some(candidate);
                break;
            }
        }

        found.ok_or_else(|| {
            format!(
                "Could not find '{}'. Looked in: lib/{}.bv, imports/{}.bv, ./{}.bv\n\
                Or specify path: brief import {} --path <path>",
                name, name, name, name, name
            )
        })?
    };

    let relative_path = if let Ok(rel) =
        dep_path.strip_prefix(manifest_path.parent().unwrap_or(std::path::Path::new(".")))
    {
        rel.to_path_buf()
    } else {
        dep_path.clone()
    };

    manifest.add_dependency(
        name.to_string(),
        manifest::Dependency::Path(manifest::PathDependency {
            path: relative_path,
        }),
    );

    manifest.save(&manifest_path)?;

    if verbose {
        println!("Added dependency '{}' = '{}'", name, dep_path.display());
    }

    println!("✓ Added '{}' to dependencies", name);

    Ok(())
}

fn run_watch(file_path: PathBuf, verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    println!("Watching for changes... (Ctrl+C to stop)");

    let source = fs::read_to_string(&file_path)?;
    let clean_source = strip_annotations(&source);

    let mut parser = parser::Parser::new(&clean_source);
    let program = match parser.parse() {
        Ok(prog) => prog,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            return Err("Parse error".into());
        }
    };

    let mut import_resolver = import_resolver::ImportResolver::new();
    let program = match import_resolver.resolve_imports(&program, &file_path) {
        Ok(resolved) => resolved,
        Err(e) => {
            eprintln!("Import error: {}", e);
            return Err("Import error".into());
        }
    };

    let mut desug = desugarer::Desugarer::new();
    let program = desug.desugar(&program);

    let mut tc = typechecker::TypeChecker::new();
    let type_errors = tc.check_program(&program);
    if !type_errors.is_empty() {
        eprintln!(
            "{}",
            format_type_errors(&type_errors, file_path.to_str().unwrap_or("main.bv"))
        );
        return Err("Type errors".into());
    }

    let mut pe = proof_engine::ProofEngine::new();
    let proof_errors = pe.verify_program(&program);
    if !proof_errors.is_empty() {
        eprintln!(
            "{}",
            format_proof_errors(&proof_errors, file_path.to_str().unwrap_or("main.bv"))
        );
        return Err("Proof errors".into());
    }

    println!("✓ Initial check passed - watching for changes...");

    let watch_path = file_path.clone();
    let mut watcher =
        notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| match res {
            Ok(event) => {
                if event.kind.is_modify() || event.kind.is_create() {
                    println!("\n[Change detected] Rebuilding...");
                    if let Err(e) = run_check(&watch_path, true, false) {
                        eprintln!("Build failed: {}", e);
                    } else {
                        println!("Watch mode active");
                    }
                }
            }
            Err(e) => eprintln!("Watch error: {:?}", e),
        })?;

    let source_dir = file_path.parent().unwrap_or(std::path::Path::new("."));
    watcher.watch(source_dir, notify::RecursiveMode::Recursive)?;

    loop {
        std::thread::sleep(Duration::from_secs(1));
    }
}

fn run_serve(dir: &Path, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};

    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr)?;

    println!("Brief Server");
    println!("Serving {} on http://{}", dir.display(), addr);
    println!("Press Ctrl+C to stop\n");

    fn get_mime_type(path: &Path) -> &'static str {
        match path.extension().and_then(|e| e.to_str()) {
            Some("html") => "text/html",
            Some("css") => "text/css",
            Some("js") => "application/javascript",
            Some("wasm") => "application/wasm",
            Some("json") => "application/json",
            Some("png") => "image/png",
            Some("jpg") | Some("jpeg") => "image/jpeg",
            Some("svg") => "image/svg+xml",
            Some("ico") => "image/x-icon",
            _ => "application/octet-stream",
        }
    }

    fn handle_request(mut stream: TcpStream, root_dir: &Path) {
        let mut buffer = [0u8; 8192];
        let bytes_read = match stream.read(&mut buffer) {
            Ok(n) => n,
            Err(_) => return,
        };

        let request = String::from_utf8_lossy(&buffer[..bytes_read]);
        let first_line = request.lines().next();

        let path = if let Some(line) = first_line {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                parts[1].trim_start_matches('/')
            } else {
                "index.html"
            }
        } else {
            "index.html"
        };

        let file_path = root_dir.join(path);
        let file_path = if file_path.is_dir() {
            file_path.join("index.html")
        } else {
            file_path
        };

        let (status, content_type, body) = if file_path.exists() && file_path.is_file() {
            match fs::read(&file_path) {
                Ok(data) => ("200 OK", get_mime_type(&file_path), data),
                Err(_) => (
                    "500 Internal Server Error",
                    "text/plain",
                    b"Error reading file".to_vec(),
                ),
            }
        } else {
            ("404 Not Found", "text/plain", b"File not found".to_vec())
        };

        let response = format!(
            "HTTP/1.1 {}\r\n\
            Content-Type: {}\r\n\
            Content-Length: {}\r\n\
            Connection: close\r\n\
            \r\n",
            status,
            content_type,
            body.len()
        );

        let _ = stream.write_all(response.as_bytes());
        let _ = stream.write_all(&body);
    }

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let dir = dir.to_path_buf();
                std::thread::spawn(move || {
                    handle_request(stream, &dir);
                });
            }
            Err(e) => {
                eprintln!("Connection error: {}", e);
            }
        }
    }

    Ok(())
}

fn run_rbv(file_path: &PathBuf, out_dir: Option<&Path>) -> Result<(), Box<dyn std::error::Error>> {
    println!("Compiling RBV: {}", file_path.display());

    let source = fs::read_to_string(file_path)?;

    let rbv_file = rbv::RbvFile::parse(&source).map_err(|e| format!("RBV parse error: {}", e))?;

    println!("  Brief source: {} chars", rbv_file.brief_source.len());

    let mut parser = parser::Parser::new(&rbv_file.brief_source);
    let program = parser
        .parse()
        .map_err(|e| format!("Brief parse error: {}", e))?;

    println!("  Parsed {} items", program.items.len());

    let mut import_resolver = import_resolver::ImportResolver::new();
    let program = import_resolver
        .resolve_imports(&program, file_path)
        .map_err(|e| format!("Import error: {}", e))?;

    println!("  Resolved imports");

    let mut desug = desugarer::Desugarer::new();
    let program = desug.desugar(&program);

    let mut tc = typechecker::TypeChecker::new();
    println!("  Type checking...");
    let type_errors = tc.check_program(&program);
    if !type_errors.is_empty() {
        eprintln!(
            "{}",
            format_type_errors(&type_errors, file_path.to_str().unwrap_or("main.rbv"))
        );
        return Err("Type errors".into());
    }
    println!("  Type checked OK");

    let mut pe = proof_engine::ProofEngine::new();
    println!("  Proof engine running...");
    let proof_errors = pe.verify_program(&program);
    println!("  Proof engine done");
    if !proof_errors.is_empty() {
        eprintln!(
            "{}",
            format_proof_errors(&proof_errors, file_path.to_str().unwrap_or("main.rbv"))
        );
        return Err("Proof errors".into());
    }

    let mut view_compiler = view_compiler::ViewCompiler::new();
    println!("  Compiling view...");
    for (i, item) in program.items.iter().enumerate() {
        if let ast::TopLevel::StateDecl(d) = item {
            view_compiler.register_signal(&d.name, i);
        }
        if let ast::TopLevel::Transaction(t) = item {
            view_compiler.register_transaction(&t.name, i);
        }
    }
    let bindings = view_compiler.compile(&rbv_file.view_html);
    println!("  View compiled: {} bindings", bindings.len());

    let output_path = if let Some(p) = out_dir {
        p.to_path_buf()
    } else if file_path.is_absolute() {
        file_path.parent().unwrap_or(&file_path).to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    };
    let stem = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    let mut wasm_gen = wasm_gen::WasmGenerator::new();
    println!("  Generating WASM...");
    let output = wasm_gen.generate(&program, &bindings, stem);
    println!("  WASM generated");

    println!("  Output path: {:?}", output_path);

    let js_path = output_path.join(format!("{}_glue.js", stem));
    fs::write(&js_path, &output.js_glue)?;
    println!("  Generated: {}", js_path.display());

    if let Some(css) = &rbv_file.style_css {
        let css_path = output_path.join(format!("{}.css", stem));
        fs::write(&css_path, css)?;
        println!("  Generated: {}", css_path.display());
    }

    let html_path = output_path.join(format!("{}.html", stem));
    let html = generate_html(stem, &rbv_file.view_html);
    fs::write(&html_path, &html)?;
    println!("  Generated: {}", html_path.display());

    let src_dir = output_path.join("src");
    fs::create_dir_all(&src_dir)?;

    let lib_rs = format!("mod {};\npub use {}::*;\n", stem, stem);
    fs::write(src_dir.join("lib.rs"), lib_rs)?;

    let wasm_rs = output.rust_code.clone();
    fs::write(src_dir.join(format!("{}.rs", stem)), wasm_rs)?;

    let lib_rs = format!("mod {};\npub use {}::{{State}};\n", stem, stem);
    fs::write(src_dir.join("lib.rs"), lib_rs)?;

    let main_rs = format!("fn main() {{}}\n");
    fs::write(src_dir.join("main.rs"), main_rs)?;

    let cargo_toml = format!(
        r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
wasm-bindgen = "0.2"
js-sys = "0.3"

[profile.release]
opt-level = "s"
lto = true
js-sys = "0.3"
"#,
        stem
    );
    fs::write(output_path.join("Cargo.toml"), cargo_toml)?;
    println!("  Generated: {}/Cargo.toml", output_path.display());
    println!("  Generated: {}/src/lib.rs", output_path.display());
    println!("  Generated: {}/src/main.rs", output_path.display());

    println!("\n✓ RBV compiled successfully");
    println!(
        "  Signals: {}, Transactions: {}",
        output.signal_count, output.txn_count
    );
    println!("  Bindings: {}", bindings.len());
    println!("\n  To build WASM, run:");
    println!(
        "    cd {} && wasm-pack build --target web",
        output_path.display()
    );

    Ok(())
}

fn generate_html(name: &str, view_html: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>{}</title>
    <link rel="stylesheet" href="{}.css">
</head>
<body>
{}
    <script type="module" src="{}_glue.js"></script>
</body>
</html>
"#,
        name, name, view_html, name
    )
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage(&args[0]);
        return;
    }

    let command = &args[1];

    match command.as_str() {
        "check" | "c" => {
            let verbose =
                args.contains(&"-v".to_string()) || args.contains(&"--verbose".to_string());
            let annotate =
                args.contains(&"-a".to_string()) || args.contains(&"--annotate".to_string());

            let file_path = args
                .iter()
                .skip(2)
                .find(|a| a.ends_with(".bv"))
                .map(PathBuf::from);

            if let Some(path) = file_path {
                if let Err(_e) = run_check(&path, verbose, annotate) {
                    std::process::exit(1);
                }
            } else {
                eprintln!("Error: No .bv file specified");
                eprintln!("Usage: {} check <file.bv>", args[0]);
                std::process::exit(1);
            }
        }

        "build" | "b" => {
            let verbose =
                args.contains(&"-v".to_string()) || args.contains(&"--verbose".to_string());

            let file_path = args
                .iter()
                .skip(2)
                .find(|a| a.ends_with(".bv"))
                .map(PathBuf::from);

            if let Some(path) = file_path {
                if let Err(_e) = run_build(&path, verbose) {
                    std::process::exit(1);
                }
            } else {
                eprintln!("Error: No .bv file specified");
                eprintln!("Usage: {} build <file.bv>", args[0]);
                std::process::exit(1);
            }
        }

        "watch" | "w" => {
            let verbose =
                args.contains(&"-v".to_string()) || args.contains(&"--verbose".to_string());

            let file_path = args
                .iter()
                .skip(2)
                .find(|a| a.ends_with(".bv"))
                .map(PathBuf::from);

            if let Some(path) = file_path {
                if let Err(e) = run_watch(path, verbose) {
                    eprintln!("Watch error: {}", e);
                    std::process::exit(1);
                }
            } else {
                eprintln!("Error: No .bv file specified");
                eprintln!("Usage: {} watch <file.bv>", args[0]);
                std::process::exit(1);
            }
        }

        "init" => {
            let name = args.get(2).map(|s| s.as_str());
            if let Err(e) = run_init(name, true) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }

        "import" => {
            if args.len() < 3 {
                eprintln!("Error: No dependency name specified");
                eprintln!("Usage: {} import <name> [--path <path>]", args[0]);
                std::process::exit(1);
            }

            let name = &args[2];
            let path = args
                .iter()
                .skip(3)
                .skip_while(|a| a.as_str() != "--path")
                .nth(1)
                .map(|s| s.as_str());

            if let Err(e) = run_import(name, path, true) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }

        "serve" => {
            let mut dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let mut port: Option<u16> = None;

            let mut i = 2;
            while i < args.len() {
                let arg = &args[i];
                if arg == "--port" && i + 1 < args.len() {
                    if let Ok(p) = args[i + 1].parse() {
                        port = Some(p);
                    }
                    i += 2;
                } else if arg.starts_with("--port=") {
                    if let Ok(p) = arg.strip_prefix("--port=").unwrap_or("").parse() {
                        port = Some(p);
                    }
                    i += 1;
                } else if !arg.starts_with("-") {
                    dir = PathBuf::from(arg);
                    i += 1;
                } else {
                    i += 1;
                }
            }

            let port = port.unwrap_or(8080);

            if let Err(e) = run_serve(&dir, port) {
                eprintln!("Server error: {}", e);
                std::process::exit(1);
            }
        }

        "rbv" => {
            let mut out_dir = None;
            let mut file_path = None;

            let mut i = 2;
            while i < args.len() {
                let arg = &args[i];
                if arg == "--out" && i + 1 < args.len() {
                    out_dir = Some(PathBuf::from(&args[i + 1]));
                    i += 2;
                } else if arg.ends_with(".rbv") {
                    file_path = Some(PathBuf::from(arg));
                    i += 1;
                } else {
                    i += 1;
                }
            }

            if let Some(path) = file_path {
                if let Err(e) = run_rbv(&path, out_dir.as_deref()) {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            } else {
                eprintln!("Error: No .rbv file specified");
                eprintln!("Usage: {} rbv <file.rbv> [--out <dir>]", args[0]);
                std::process::exit(1);
            }
        }

        "lsp" => {
            let quiet =
                args.contains(&"--quiet".to_string()) || args.contains(&"--whisper".to_string());
            let mode = if quiet {
                errors::ErrorMode::Whisper
            } else {
                errors::ErrorMode::Verbose
            };
            lsp::run_lsp_server(mode);
        }

        "-h" | "--help" | "help" => {
            print_usage(&args[0]);
        }

        _ => {
            if command.ends_with(".bv") {
                if let Err(_e) = run_check(&PathBuf::from(command), false, false) {
                    std::process::exit(1);
                }
            } else {
                eprintln!("Unknown command: {}", command);
                print_usage(&args[0]);
                std::process::exit(1);
            }
        }
    }
}
