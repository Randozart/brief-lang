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

use brief_compiler::{
    annotator, ast, backend, desugarer, errors, hardware_validator, import_resolver, interpreter,
    lsp, manifest, parser, proof_engine, rbv, typechecker, view_compiler,
};
use notify::Watcher;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

fn format_hardware_diagnostics(
    diags: &[errors::Diagnostic],
    source: &str,
    file_name: &str,
) -> String {
    let mut output = String::new();
    for diag in diags {
        let severity_prefix = match diag.severity {
            errors::Severity::Error => "error",
            errors::Severity::Warning => "warning",
            errors::Severity::Info => "info",
            errors::Severity::Note => "note",
        };
        output.push_str(&format!(
            "{}[{}]: {}\n",
            severity_prefix, diag.code, diag.title
        ));
        if let Some(span) = diag.span {
            let mut s = span;
            // The format method doesn't take a file name, so we prefix it manually if needed
            // But span.format usually produces " --> file:line:col"
            // We can replace "file" with the actual file name
            let formatted = s
                .format(source)
                .replace(" --> file:", &format!(" --> {}:", file_name));
            output.push_str(&formatted);
            output.push_str("\n");
        }
        for explanation in &diag.explanation {
            output.push_str(&format!("  = {}\n", explanation));
        }
        for hint in &diag.hints {
            output.push_str(&format!("  = hint: {}\n", hint));
        }
        output.push('\n');
    }
    output
}

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
            typechecker::TypeError::FFIError { message } => {
                output.push_str(&format!(
                    "error[F001]: FFI error\n --> {}:?:?\n  |\n",
                    file_name
                ));
                output.push_str(&format!("  = {}\n", message));
            }
        }
        output.push('\n');
    }
    output
}

fn format_proof_errors(errors: &[proof_engine::ProofError], file_name: &str) -> String {
    let mut output = String::new();
    for err in errors {
        let severity = if err.is_warning { "warning" } else { "error" };
        output.push_str(&format!(
            "{}[{}]: {}\n --> {}:?:?\n",
            severity, err.code, err.title, file_name
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

fn strip_codicil_blocks(source: &str) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let mut output = Vec::new();
    let mut in_codicil_block = false;

    for line in lines {
        let trimmed = line.trim();
        if trimmed == "[route]" || trimmed == "[pre]" || trimmed == "[post]" {
            in_codicil_block = true;
            continue;
        }
        if in_codicil_block {
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if !trimmed.starts_with('[')
                && !trimmed.starts_with("method")
                && !trimmed.starts_with("path")
                && !trimmed.starts_with("middleware")
                && !trimmed.starts_with("context")
                && !trimmed.starts_with("handler")
                && !trimmed.starts_with("response")
                && !trimmed.starts_with("params")
            {
                in_codicil_block = false;
            } else {
                continue;
            }
        }
        if !in_codicil_block {
            output.push(line);
        }
    }

    while output.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
        output.pop();
    }

    output.join("\n")
}

fn detect_codicil_project(file_path: &Path) -> bool {
    let mut current = file_path.parent();
    while let Some(dir) = current {
        if dir.join("codicil.toml").exists() || dir.join(".codicil").exists() {
            return true;
        }
        current = dir.parent();
    }
    false
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
    eprintln!("  run <file>       Compile, build WASM, serve, and open browser");
    eprintln!("  map <lib>        Analyze library and show generated bindings (dry-run)");
    eprintln!("  wrap <lib>       Generate FFI bindings for a library");
    eprintln!("  install          Install 'brief' to ~/.local/bin");
    eprintln!("  lsp              Start Language Server (for IDE integration)");
    eprintln!();
    eprintln!("RBV Options:");
    eprintln!("  --out <dir>      Output directory (default: <name>-build)");
    eprintln!("  --no-build       Skip wasm-pack build");
    eprintln!("  --no-cache       Clear build cache before compiling");
    eprintln!("  --port <port>    Port for server (default: 8080)");
    eprintln!("  --no-open        Don't open browser (for 'run' command)");
    eprintln!("  --watch, -w      Watch for changes and rebuild (for 'run' command)");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -a, --annotate       Generate path annotations");
    eprintln!("  --skip-proof         Skip proof verification");
    eprintln!("  --no-stdlib          Disable standard library bindings");
    eprintln!("  --stdlib-path <path> Use custom standard library path");
    eprintln!("  -v, --verbose        Verbose output");
    eprintln!("  --quiet, --whisper   Minimal output (for CI/automated use)");
    eprintln!("  -h, --help           Show this help");
}

const STDLIB_BINDINGS: &[(&str, &str)] = &[
    (
        "collections.toml",
        include_str!("../lib/ffi/bindings/collections.toml"),
    ),
    (
        "encoding.toml",
        include_str!("../lib/ffi/bindings/encoding.toml"),
    ),
    ("http.toml", include_str!("../lib/ffi/bindings/http.toml")),
    ("io.toml", include_str!("../lib/ffi/bindings/io.toml")),
    ("json.toml", include_str!("../lib/ffi/bindings/json.toml")),
    ("math.toml", include_str!("../lib/ffi/bindings/math.toml")),
    (
        "string.toml",
        include_str!("../lib/ffi/bindings/string.toml"),
    ),
    ("time.toml", include_str!("../lib/ffi/bindings/time.toml")),
];

fn run_install() {
    let install_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".local")
        .join("bin");

    let current_exe = std::env::current_exe().expect("Failed to get current executable path");
    let install_path = install_dir.join("brief");

    if !install_dir.exists() {
        fs::create_dir_all(&install_dir).expect("Failed to create install directory");
    }

    fs::copy(&current_exe, &install_path).expect("Failed to copy binary");
    fs::set_permissions(
        &install_path,
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    )
    .expect("Failed to set permissions");

    println!("Installed 'brief' to {}", install_path.display());

    // Metropolitan Installation: Unpack standard library TOMLs to share directory
    if let Some(data_dir) = dirs::data_dir() {
        let brief_data_dir = data_dir.join("brief").join("ffi").join("bindings");
        if let Err(e) = fs::create_dir_all(&brief_data_dir) {
            eprintln!(
                "Warning: Failed to create standard library directory: {}",
                e
            );
        } else {
            println!(
                "Unpacking standard library to {}...",
                brief_data_dir.display()
            );
            for (filename, content) in STDLIB_BINDINGS {
                let file_path = brief_data_dir.join(filename);
                if let Err(e) = fs::write(&file_path, content) {
                    eprintln!(
                        "Warning: Failed to write standard library file {}: {}",
                        filename, e
                    );
                }
            }
        }
    }

    println!("\nAdd to your PATH if needed:");
    println!("  export PATH=\"$PATH:{}\"", install_dir.display());
    println!("\nAdd this line to your ~/.bashrc or ~/.zshrc to make it permanent.");
}

fn run_map_or_wrap(
    lib_path: &Path,
    mapper: Option<&str>,
    output_dir: Option<&Path>,
    force: bool,
    is_wrap: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use brief_compiler::ffi::{MapperInfo, MapperRegistry};
    use brief_compiler::wrapper::{
        analyze_library,
        generator::{
            generate_bindings_toml, generate_lib_bv, preview_generated, write_generated_files,
        },
    };

    let lib_name = lib_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    let mapper_name = mapper.unwrap_or_else(|| {
        if lib_path.extension().and_then(|e| e.to_str()) == Some("rs") {
            "rust"
        } else if lib_path.extension().and_then(|e| e.to_str()) == Some("h") {
            "c"
        } else if lib_path.extension().and_then(|e| e.to_str()) == Some("wasm") {
            "wasm"
        } else {
            "rust"
        }
    });

    let registry = MapperRegistry::new();
    let mapper_info = registry.find_mapper(mapper_name, None);

    println!("  Library: {}", lib_name);
    println!("  Mapper: {}", mapper_name);

    if let Some(info) = mapper_info {
        println!("  Mapper path: {}", info.path.display());
    } else {
        eprintln!("  Warning: Mapper '{}' not found", mapper_name);
        eprintln!("  Available mappers: rust, c, wasm");
    }

    // Try to analyze the library
    let analysis_result = match analyze_library(lib_path, Some(mapper_name)) {
        Ok(result) => {
            println!("  Analyzed {} functions", result.functions.len());
            Some(result)
        }
        Err(e) => {
            eprintln!("  Analysis warning: {}", e);
            eprintln!("  Generating template files instead");
            None
        }
    };

    if is_wrap {
        let out_dir = output_dir
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("lib/ffi/generated").join(lib_name));

        if !out_dir.exists() {
            fs::create_dir_all(&out_dir)?;
        }

        if let Some(result) = analysis_result {
            write_generated_files(&result, &out_dir, force)?;
        } else {
            // Generate template files
            let lib_bv_path = out_dir.join("lib.bv");
            let toml_path = out_dir.join("bindings.toml");

            let lib_bv_content = format!(
                "// Auto-generated wrapper for {}\n// Mapper: {}\n\n// Foreign function declarations (frgn sig)\n// TODO: Add frgn sig declarations\n\n// User MUST define these manually:\n// defn function_name(args) -> ResultType [\n//   true  // precondition - TODO: refine\n// ][\n//   result.valid()  // postcondition - TODO: refine\n// ] {{\n//   __raw_function_name(args)\n// }};\n",
                lib_name, mapper_name
            );

            let toml_content = format!(
                "# Auto-generated bindings for {}\n# Mapper: {}\n\n[[functions]]\nname = \"TODO\"\nlocation = \"{}\"\ntarget = \"native\"\nmapper = \"{}\"\n\n[functions.input]\n# TODO: Add input parameters\n\n[functions.output.success]\n# TODO: Add success output\n\n[functions.output.error]\ntype = \"Error\"\ncode = \"Int\"\nmessage = \"String\"\n",
                lib_name, mapper_name, lib_name, mapper_name
            );

            fs::write(&lib_bv_path, lib_bv_content)?;
            fs::write(&toml_path, toml_content)?;
        }

        println!("\n  Generated files:");
        println!("    {}/lib.bv", out_dir.display());
        println!("    {}/bindings.toml", out_dir.display());
    } else {
        // Dry-run mode - show preview
        if let Some(result) = analysis_result {
            println!("\n=== lib.bv (preview) ===\n");
            println!("{}", generate_lib_bv(&result));
            println!("\n=== bindings.toml (preview) ===\n");
            println!("{}", generate_bindings_toml(&result));
        } else {
            println!("\n  Would generate:");
            println!("    lib/ffi/generated/{}/lib.bv", lib_name);
            println!("    lib/ffi/generated/{}/bindings.toml", lib_name);
        }
    }

    Ok(())
}

fn run_check(
    file_path: &PathBuf,
    verbose: bool,
    annotate: bool,
    no_stdlib: bool,
    stdlib_path: Option<PathBuf>,
    codicil_mode: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let source = fs::read_to_string(file_path)?;
    let clean_source = strip_annotations(&source);

    let processed_source = if codicil_mode && detect_codicil_project(file_path) {
        println!("[Info] Codicil mode enabled - ignoring [route], [pre], [post] blocks");
        strip_codicil_blocks(&clean_source)
    } else {
        clean_source
    };

    if verbose {
        println!("[Lexer] Tokenizing...");
    }

    let mut parser = parser::Parser::new(&processed_source);
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
    let mut program = match import_resolver.resolve_imports(&program, file_path) {
        Ok(resolved) => resolved,
        Err(e) => {
            eprintln!("Import error: {}", e);
            return Err("Import error".into());
        }
    };

    if verbose {
        println!("[Desugar] Desugaring...");
    }
    let mut desug = desugarer::Desugarer::new();
    let mut program = desug.desugar(&program);

    if verbose {
        println!("[TypeChecker] Running type checks...");
    }

    let mut tc = typechecker::TypeChecker::new()
        .with_stdlib_config(no_stdlib, stdlib_path)
        .with_target(typechecker::CompilationTarget::Interpreter);
    let type_errors = tc.check_program(&mut program);
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
    let has_errors = proof_errors.iter().any(|e| !e.is_warning);
    if has_errors {
        eprintln!(
            "{}",
            format_proof_errors(&proof_errors, file_path.to_str().unwrap_or("main.bv"))
        );
        return Err("Proof errors".into());
    }
    if !proof_errors.is_empty() {
        eprintln!(
            "{}",
            format_proof_errors(&proof_errors, file_path.to_str().unwrap_or("main.bv"))
        );
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

    println!("All checks passed");
    Ok(())
}

fn run_build(
    file_path: &PathBuf,
    verbose: bool,
    no_stdlib: bool,
    stdlib_path: Option<PathBuf>,
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
    let mut program = match import_resolver.resolve_imports(&program, file_path) {
        Ok(resolved) => resolved,
        Err(e) => {
            eprintln!("Import error: {}", e);
            return Err("Import error".into());
        }
    };

    if verbose {
        println!("[Desugar] Desugaring...");
    }
    let mut desug = desugarer::Desugarer::new();
    let mut program = desug.desugar(&program);

    if verbose {
        println!("[TypeChecker] Running type checks...");
    }

    let mut tc = typechecker::TypeChecker::new()
        .with_stdlib_config(no_stdlib, stdlib_path)
        .with_target(typechecker::CompilationTarget::Interpreter);
    let type_errors = tc.check_program(&mut program);
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
    let has_errors = proof_errors.iter().any(|e| !e.is_warning);
    if has_errors {
        eprintln!(
            "{}",
            format_proof_errors(&proof_errors, file_path.to_str().unwrap_or("main.bv"))
        );
        return Err("Proof errors".into());
    }
    if !proof_errors.is_empty() {
        eprintln!(
            "{}",
            format_proof_errors(&proof_errors, file_path.to_str().unwrap_or("main.bv"))
        );
    }

    if verbose {
        println!("[Interpreter] Running program...");
    }

    eprintln!("[MAIN] Creating interpreter...");
    let mut interp = interpreter::Interpreter::new();
    eprintln!("[MAIN] Interpreter created, loading program...");
    interp.load_program(&program);
    eprintln!("[MAIN] Program loaded, running program...");
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
entry = "main.rbv"

[dependencies]
"#,
        project_name
    );

    std::fs::write(project_dir.join("brief.toml"), manifest_content)?;

    // Pure Brief - Specification only (no UI)
    let main_bv_content = r#"# =============================================================================
# Welcome to Brief!
# =============================================================================
# This is a pure Brief file - state and transactions without UI.
# Use this for: business logic, state machines, reactive systems.
#
# To delete this file and use only .rbv: rm main.bv
# =============================================================================

let count: Int = 0;

# A reactive transaction that fires automatically
rct txn auto_increment [count < 10][count == @count + 1] {
  &count = count + 1;
  term;
};

# Transaction triggered by external events
txn increment [true][count == @count + 1] {
  &count = count + 1;
  term;
};
"#;

    // Rendered Brief - With Web UI
    let main_rbv_content = r#"# =============================================================================
# Welcome to Brief!
# =============================================================================
# This is a Rendered Brief file - state + transactions + web UI.
# Use this for: web apps, interactive UIs.
#
# To delete this file and use only .bv: rm main.rbv
# =============================================================================

<script>
rstruct Counter {
  count: Int;

  txn Counter.increment [true][count == @count + 1] {
    &count = count + 1;
    term;
  };

  txn Counter.decrement [count > 0][count == @count - 1] {
    &count = count - 1;
    term;
  };

  txn Counter.reset [true][count == 0] {
    &count = 0;
    term;
  };

  <div class="counter">
    <h2>Brief Counter</h2>
    <span class="count" b-text="count">0</span>
    <div class="buttons">
      <button b-trigger:click="increment">+</button>
      <button b-trigger:click="decrement">-</button>
      <button b-trigger:click="reset">Reset</button>
    </div>
  </div>
}
</script>

<view>
  <Counter />
</view>

<style>
  * { box-sizing: border-box; margin: 0; padding: 0; }
  body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    background: linear-gradient(135deg, #667eea, #764ba2);
    min-height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .counter {
    background: white;
    padding: 40px;
    border-radius: 16px;
    box-shadow: 0 20px 60px rgba(0,0,0,0.3);
    text-align: center;
  }
  .counter h2 { color: #333; margin-bottom: 20px; }
  .count {
    display: block;
    font-size: 72px;
    font-weight: bold;
    color: #667eea;
    margin: 20px 0;
  }
  .buttons { display: flex; gap: 10px; justify-content: center; }
  .buttons button {
    padding: 12px 24px;
    font-size: 24px;
    border: none;
    border-radius: 8px;
    background: #667eea;
    color: white;
    cursor: pointer;
    transition: transform 0.2s;
  }
  .buttons button:hover { transform: scale(1.1); }
</style>
"#;

    std::fs::write(project_dir.join("main.bv"), main_bv_content)?;
    std::fs::write(project_dir.join("main.rbv"), main_rbv_content)?;

    if verbose {
        println!("Created project structure:");
        println!("  {}/", project_name);
        println!("  {}/brief.toml", project_name);
        println!("  {}/main.bv", project_name);
        println!("  {}/main.rbv", project_name);
        println!("  {}/lib/", project_name);
    }

    println!("Project '{}' created successfully", project_name);
    println!("  Files created:");
    println!("    main.bv  - Pure Brief (specification only, no UI)");
    println!("    main.rbv - Rendered Brief (with web UI)");
    println!("  Delete whichever you don't need.");
    println!("");
    println!("  Run: cd {} && brief run", project_name);

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

    println!("Added '{}' to dependencies", name);

    Ok(())
}

fn run_watch(
    file_path: PathBuf,
    verbose: bool,
    no_stdlib: bool,
    stdlib_path: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (tx, rx) = std::sync::mpsc::channel();

    let mut watcher = notify::RecommendedWatcher::new(tx, notify::Config::default())?;

    watcher.watch(&file_path, notify::RecursiveMode::NonRecursive)?;

    println!("Watching {} for changes...", file_path.display());

    loop {
        match rx.recv() {
            Ok(_) => {
                println!("File changed, rebuilding...");
                let codicil_mode = detect_codicil_project(&file_path);
                if let Err(e) = run_check(
                    &file_path,
                    verbose,
                    false,
                    no_stdlib,
                    stdlib_path.clone(),
                    codicil_mode,
                ) {
                    eprintln!("Rebuild failed: {}", e);
                }
            }
            Err(e) => eprintln!("Watch error: {}", e),
        }
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

fn run_arm(
    file_path: &PathBuf,
    out_dir: Option<&Path>,
    no_stdlib: bool,
    stdlib_path: Option<PathBuf>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    println!("Compiling to ARM Rust: {}", file_path.display());

    let source = fs::read_to_string(file_path)?;
    let clean_source = strip_annotations(&source);

    let mut parser = parser::Parser::new(&clean_source);
    let mut program = parser
        .parse()
        .map_err(|e| format!("Brief parse error: {}", e))?;

    let mut import_resolver = import_resolver::ImportResolver::new();
    let mut program = import_resolver
        .resolve_imports(&program, file_path)
        .map_err(|e| format!("Import error: {}", e))?;

    let mut desug = desugarer::Desugarer::new();
    let mut program = desug.desugar(&program);

    let mut tc = typechecker::TypeChecker::new()
        .with_stdlib_config(no_stdlib, stdlib_path)
        .with_target(typechecker::CompilationTarget::Interpreter);
    let type_errors = tc.check_program(&mut program);
    if !type_errors.is_empty() {
        return Err(format!("Type errors: {}", format_type_errors(&type_errors, file_path.to_str().unwrap_or("main.bv"))).into());
    }

    let mut pe = proof_engine::ProofEngine::new();
    let proof_errors = pe.verify_program(&program);
    if !proof_errors.is_empty() {
        eprintln!("  Warning: Proof errors (continuing anyway)");
    }

    let mut wasm_gen = backend::wasm::WasmGenerator::new().with_target(backend::wasm::CodeTarget::Arm);
    let output = wasm_gen.generate(&program, &[], "kernel");

    let stem = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    let out_path = if let Some(dir) = out_dir {
        let d = dir.to_path_buf();
        fs::create_dir_all(&d)?;
        d.join(format!("{}.rs", stem))
    } else {
        PathBuf::from(format!("{}.rs", stem))
    };

    fs::write(&out_path, &output.rust_code)?;
    println!("  ARM Rust generated: {}", out_path.display());

    Ok(out_path)
}

fn run_verilog(
    file_path: &PathBuf,
    hw_config_path: &PathBuf,
    out_dir: Option<&Path>,
    no_stdlib: bool,
    stdlib_path: Option<PathBuf>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    println!("Compiling to SystemVerilog: {}", file_path.display());

    // Load HW config (required)
    if hw_config_path.to_str() == Some("/dev/null") {
        return Err("Hardware config (--hw) is REQUIRED for Verilog compilation".into());
    }

    let hw_config = parser::parse_hardware_config(hw_config_path)?;

    // Standard Brief pipeline
    let source = fs::read_to_string(file_path)?;
    let mut parser = parser::Parser::new(&source);
    let mut program = parser
        .parse()
        .map_err(|e| format!("Brief parse error: {}", e))?;

    let mut import_resolver = import_resolver::ImportResolver::new();
    let mut program = import_resolver
        .resolve_imports(&program, file_path)
        .map_err(|e| format!("Import error: {}", e))?;

    let mut desug = desugarer::Desugarer::new();
    let mut program = desug.desugar(&program);

    let mut tc = typechecker::TypeChecker::new()
        .with_stdlib_config(no_stdlib, stdlib_path)
        .with_target(typechecker::CompilationTarget::Verilog);
    let type_errors = tc.check_program(&mut program);
    if !type_errors.is_empty() {
        eprintln!(
            "{}",
            format_type_errors(&type_errors, file_path.to_str().unwrap_or("main.ebv"))
        );
        return Err("Type errors".into());
    }

    // Hardware validation
    let is_ebv = file_path.extension().map(|e| e == "ebv").unwrap_or(false);
    let hw_diagnostics = hardware_validator::HardwareValidator::validate(
        &program,
        Some(&hw_config),
        "verilog",
        is_ebv,
    );

    if !hw_diagnostics.is_empty() {
        eprintln!(
            "{}",
            format_hardware_diagnostics(
                &hw_diagnostics,
                &source,
                file_path.to_str().unwrap_or("main.ebv")
            )
        );
        let has_errors = hw_diagnostics
            .iter()
            .any(|d| d.severity == errors::Severity::Error);
        if is_ebv && has_errors {
            return Err("Hardware validation failed for .ebv".into());
        }
    }

    // Verilog generation
    let stem = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("top");
    let mut verilog_gen = backend::verilog::VerilogGenerator::new(stem, hw_config);
    let verilog_code = verilog_gen.generate(&program);
    let tb_code = verilog_gen.generate_testbench(&program);

    // Write output
    let out_path = out_dir
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    if !out_path.exists() {
        fs::create_dir_all(&out_path)?;
    }
    let output_file = out_path.join(format!("{}.sv", stem));
    fs::write(&output_file, verilog_code)?;

    let tb_file = out_path.join(format!("{}_tb.sv", stem));
    fs::write(&tb_file, tb_code)?;

    println!("  Generated: {}", output_file.display());
    println!("  Generated: {}", tb_file.display());
    Ok(output_file)
}

fn run_rbv(
    file_path: &PathBuf,
    out_dir: Option<&Path>,
    build_wasm: bool,
    no_stdlib: bool,
    stdlib_path: Option<PathBuf>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    println!("Compiling RBV: {}", file_path.display());

    let source = fs::read_to_string(file_path)?;

    let rbv_file = rbv::RbvFile::parse(&source).map_err(|e| format!("RBV parse error: {}", e))?;

    println!("  Brief source: {} chars", rbv_file.brief_source.len());

    let mut parser = parser::Parser::new(&rbv_file.brief_source);
    let mut program = parser
        .parse()
        .map_err(|e| format!("Brief parse error: {}", e))?;

    println!("  Parsed {} items", program.items.len());

    let mut import_resolver = import_resolver::ImportResolver::new();
    let mut program = import_resolver
        .resolve_imports(&program, file_path)
        .map_err(|e| format!("Import error: {}", e))?;

    // Extract CSS from Stylesheet imports
    let mut css_content = String::new();
    let mut stylesheet_items: Vec<usize> = Vec::new();
    for (i, item) in program.items.iter().enumerate() {
        if let ast::TopLevel::Stylesheet(css) = item {
            println!("  Found stylesheet import");
            css_content.push_str(css);
            css_content.push('\n');
            stylesheet_items.push(i);
        }
    }
    // Remove stylesheet items from program (they're not Brief code)
    for i in stylesheet_items.iter().rev() {
        program.items.remove(*i);
    }

    // Process SvgComponent items
    let mut render_blocks = HashMap::new();
    let mut svg_items: Vec<usize> = Vec::new();
    for (i, item) in program.items.iter().enumerate() {
        if let ast::TopLevel::SvgComponent { name, content } = item {
            render_blocks.insert(name.clone(), content.clone());
            svg_items.push(i);
        }
    }
    // Remove SVG items from program (they're not Brief code)
    for i in svg_items.iter().rev() {
        program.items.remove(*i);
    }

    println!("  Resolved imports");

    let mut desug = desugarer::Desugarer::new();
    let mut program = desug.desugar(&program);

    let mut tc = typechecker::TypeChecker::new()
        .with_stdlib_config(no_stdlib, stdlib_path)
        .with_target(typechecker::CompilationTarget::Wasm);
    println!("  Type checking...");
    let type_errors = tc.check_program(&mut program);
    if !type_errors.is_empty() {
        eprintln!(
            "{}",
            format_type_errors(&type_errors, file_path.to_str().unwrap_or("main.rbv"))
        );
        return Err("Type errors".into());
    }
    println!("  Type checked OK");

    // Merge RenderBlock into corresponding StructDefinition
    let mut program = program;
    program.items.retain(|item| {
        if let ast::TopLevel::RenderBlock(rb) = item {
            render_blocks.insert(rb.struct_name.clone(), rb.view_html.clone());
            false
        } else {
            true
        }
    });
    for (name, html) in &render_blocks {
        for item in &mut program.items {
            if let ast::TopLevel::Struct(s) = item {
                if s.name == *name {
                    s.view_html = Some(html.clone());
                    break;
                }
            }
        }
    }

    // Expand component tags in view HTML
    let mut expanded_view = rbv_file.view_html.clone();
    let mut changed = true;
    while changed {
        changed = false;
        for (name, html) in &render_blocks {
            let tag = format!("<{} />", name);
            if expanded_view.contains(&tag) {
                expanded_view = expanded_view.replace(&tag, html);
                changed = true;
            }
            let tag2 = format!("<{}/>", name);
            if expanded_view.contains(&tag2) {
                expanded_view = expanded_view.replace(&tag2, html);
                changed = true;
            }
        }
    }

    let mut pe = proof_engine::ProofEngine::new();
    println!("  Proof engine running...");
    let proof_errors = pe.verify_program(&program);
    println!("  Proof engine done");
    let has_errors = proof_errors.iter().any(|e| !e.is_warning);
    if has_errors {
        eprintln!(
            "{}",
            format_proof_errors(&proof_errors, file_path.to_str().unwrap_or("main.rbv"))
        );
        return Err("Proof errors".into());
    }
    if !proof_errors.is_empty() {
        eprintln!(
            "{}",
            format_proof_errors(&proof_errors, file_path.to_str().unwrap_or("main.rbv"))
        );
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
    let (bindings, html_with_ids, view_diagnostics) = view_compiler.compile(&expanded_view);
    println!("  View compiled: {} bindings", bindings.len());
    for diag in view_diagnostics {
        eprintln!("  {}", diag);
    }

    let output_path = if let Some(p) = out_dir {
        p.to_path_buf()
    } else {
        let stem = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");
        std::env::current_dir()?.join(format!("{}-build", stem))
    };

    fs::create_dir_all(&output_path)?;

    let stem = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    let mut wasm_gen = backend::wasm::WasmGenerator::new();
    if let Some(speed) = program.reactor_speed {
        wasm_gen.set_reactor_speed(speed);
    }

    let output = wasm_gen.generate(&program, &bindings, stem);
    println!("  WASM generated");

    println!("  Output path: {:?}", output_path);

    let js_path = output_path.join(format!("{}_glue.js", stem));
    fs::write(&js_path, &output.js_glue)?;
    println!("  Generated: {}", js_path.display());

    // Write CSS file (combine inline styles + imported stylesheets)
    let final_css = if let Some(inline_css) = &rbv_file.style_css {
        if css_content.is_empty() {
            Some(inline_css.clone())
        } else {
            Some(format!("{}\n{}", inline_css, css_content))
        }
    } else if !css_content.is_empty() {
        Some(css_content)
    } else {
        None
    };

    if let Some(css) = &final_css {
        let css_path = output_path.join(format!("{}.css", stem));
        fs::write(&css_path, css)?;
        println!("  Generated: {}", css_path.display());
    }

    let html_path = output_path.join(format!("{}.html", stem));
    let html = generate_html(stem, &html_with_ids);
    fs::write(&html_path, &html)?;
    println!("  Generated: {}", html_path.display());

    let src_dir = output_path.join("src");
    fs::create_dir_all(&src_dir)?;

    let wasm_rs = output.rust_code.clone();
    let module_name = if stem == "main" { "app" } else { stem };
    fs::write(src_dir.join(format!("{}.rs", module_name)), wasm_rs)?;

    let lib_rs = format!(
        "mod {};\npub use {}::{{State}};\n",
        module_name, module_name
    );
    fs::write(src_dir.join("lib.rs"), lib_rs)?;

    fs::write(src_dir.join("main.rs"), "fn main() {}\n")?;

    let cargo_toml = format!(
        r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[workspace]

[lib]
crate-type = ["cdylib"]

[dependencies]
wasm-bindgen = "0.2"
js-sys = "0.3"

[profile.release]
opt-level = "s"
lto = true

[package.metadata.wasm-pack.profile.release]
wasm-opt = false
"#,
        stem
    );
    fs::write(output_path.join("Cargo.toml"), cargo_toml)?;
    println!("  Generated: {}/Cargo.toml", output_path.display());
    println!("  Generated: {}/src/lib.rs", output_path.display());
    println!("  Generated: {}/src/main.rs", output_path.display());

    if build_wasm {
        println!("\n  Building WASM with wasm-pack...");
        let output_dir = output_path.join("pkg");

        // Check if WASM needs rebuild by comparing source timestamps
        let src_file = src_dir.join(format!("{}.rs", stem));
        let wasm_bin = output_dir.join(format!("{}_bg.wasm", stem));

        let needs_rebuild = !wasm_bin.exists() || {
            // Check if source is newer than WASM binary
            if let (Ok(src_meta), Ok(wasm_meta)) =
                (fs::metadata(&src_file), fs::metadata(&wasm_bin))
            {
                if let (Ok(src_modified), Ok(wasm_modified)) =
                    (src_meta.modified(), wasm_meta.modified())
                {
                    src_modified > wasm_modified
                } else {
                    true
                }
            } else {
                true
            }
        };

        if !needs_rebuild {
            println!("  WASM already built and source unchanged");
        } else {
            // Remove old pkg directory to force clean rebuild
            if output_dir.exists() {
                fs::remove_dir_all(&output_dir)?;
            }

            let wasm_pack_path = if let Ok(home) = std::env::var("HOME") {
                format!("{}/.cargo/bin/wasm-pack", home)
            } else {
                "wasm-pack".to_string()
            };

            let status = std::process::Command::new(&wasm_pack_path)
                .args(["build", "--target", "web"])
                .current_dir(&output_path)
                .status()?;

            if !status.success() {
                return Err(
                    format!("wasm-pack build failed with exit code: {:?}", status.code()).into(),
                );
            }
            println!("  WASM build complete");
        }
    }

    println!("\nRBV compiled successfully");
    println!(
        "  Signals: {}, Transactions: {}",
        output.signal_count, output.txn_count
    );
    println!("  Bindings: {}", bindings.len());
    println!("\n  Output: {}", output_path.display());

    Ok(output_path)
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

    let verbose = args.contains(&"-v".to_string()) || args.contains(&"--verbose".to_string());
    let no_stdlib = args.contains(&"--no-stdlib".to_string());
    let stdlib_path = args
        .iter()
        .position(|a| a == "--stdlib-path")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);

    match command.as_str() {
        "check" | "c" => {
            let annotate =
                args.contains(&"-a".to_string()) || args.contains(&"--annotate".to_string());

            let file_path = args
                .iter()
                .skip(2)
                .find(|a| a.ends_with(".bv"))
                .map(PathBuf::from);

            if let Some(path) = file_path {
                let codicil_mode = detect_codicil_project(&path);
                if let Err(_e) = run_check(
                    &path,
                    verbose,
                    annotate,
                    no_stdlib,
                    stdlib_path,
                    codicil_mode,
                ) {
                    std::process::exit(1);
                }
            } else {
                eprintln!("Error: No .bv file specified");
                eprintln!("Usage: {} check <file.bv>", args[0]);
                std::process::exit(1);
            }
        }

        "build" | "b" => {
            let file_path = args
                .iter()
                .skip(2)
                .find(|a| a.ends_with(".bv"))
                .map(PathBuf::from);

            if let Some(path) = file_path {
                if let Err(_e) = run_build(&path, verbose, no_stdlib, stdlib_path) {
                    std::process::exit(1);
                }
            } else {
                eprintln!("Error: No .bv file specified");
                eprintln!("Usage: {} build <file.bv>", args[0]);
                std::process::exit(1);
            }
        }

        "arm" | "a" => {
            let mut file_path = None;
            let mut out_dir = None;

            let mut i = 2;
            while i < args.len() {
                let arg = &args[i];
                if arg == "--out" && i + 1 < args.len() {
                    out_dir = Some(PathBuf::from(&args[i + 1]));
                    i += 2;
                } else if arg.ends_with(".bv") || arg.ends_with(".ebv") {
                    file_path = Some(PathBuf::from(arg));
                    i += 1;
                } else {
                    i += 1;
                }
            }

            if let Some(path) = file_path {
                if let Err(e) = run_arm(&path, out_dir.as_deref(), no_stdlib, stdlib_path.clone()) {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            } else {
                eprintln!("Error: No .bv or .ebv file specified");
                eprintln!("Usage: {} arm <file.bv|file.ebv> [--out <dir>]", args[0]);
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
                if let Err(e) = run_watch(path, verbose, no_stdlib, stdlib_path.clone()) {
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

        "verilog" | "sv" => {
            let mut file_path = None;
            let mut out_dir = None;
            let mut hw_config = None;

            let mut i = 2;
            while i < args.len() {
                let arg = &args[i];
                if arg == "--out" && i + 1 < args.len() {
                    out_dir = Some(PathBuf::from(&args[i + 1]));
                    i += 2;
                } else if arg == "--hw" && i + 1 < args.len() {
                    hw_config = Some(PathBuf::from(&args[i + 1]));
                    i += 2;
                } else if arg.ends_with(".ebv") {
                    file_path = Some(PathBuf::from(arg));
                    i += 1;
                } else if arg.ends_with(".bv") {
                    if hw_config.is_some() {
                        eprintln!("Warning: --hw flag is ignored for .bv files. Use .ebv for hardware mapping.");
                    }
                    file_path = Some(PathBuf::from(arg));
                    i += 1;
                } else {
                    i += 1;
                }
            }

            if let Some(path) = file_path {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if ext == "ebv" {
                    if let Some(hw) = hw_config {
                        if let Err(e) = run_verilog(
                            &path,
                            &hw,
                            out_dir.as_deref(),
                            no_stdlib,
                            stdlib_path.clone(),
                        ) {
                            eprintln!("Error: {}", e);
                            std::process::exit(1);
                        }
                    } else {
                        eprintln!("Error: .ebv files require --hw <hardware.toml>");
                        eprintln!(
                            "Usage: {} verilog <file.ebv> --hw <hardware.toml> [--out <dir>]",
                            args[0]
                        );
                        std::process::exit(1);
                    }
                } else if ext == "bv" {
                    let hw_path = hw_config.unwrap_or_else(|| PathBuf::from("/dev/null"));
                    if let Err(e) = run_verilog(
                        &path,
                        &hw_path,
                        out_dir.as_deref(),
                        no_stdlib,
                        stdlib_path.clone(),
                    ) {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                eprintln!("Error: Missing .bv or .ebv file");
                eprintln!(
                    "Usage: {} verilog <file.bv|file.ebv> [--hw <hardware.toml>] [--out <dir>]",
                    args[0]
                );
                std::process::exit(1);
            }
        }

        "rbv" => {
            let mut file_path = None;
            let mut out_dir = None;
            let mut build_wasm = true;
            let mut no_cache = false;

            let mut i = 2;
            while i < args.len() {
                let arg = &args[i];
                if arg == "--out" && i + 1 < args.len() {
                    out_dir = Some(PathBuf::from(&args[i + 1]));
                    i += 2;
                } else if arg == "--no-build" {
                    build_wasm = false;
                    i += 1;
                } else if arg == "--no-cache" {
                    no_cache = true;
                    i += 1;
                } else if arg.ends_with(".rbv") {
                    file_path = Some(PathBuf::from(arg));
                    i += 1;
                } else {
                    i += 1;
                }
            }

            // Clear cache if --no-cache is specified
            if no_cache {
                if let Some(ref path) = file_path {
                    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("app");
                    let build_dir = out_dir
                        .clone()
                        .unwrap_or_else(|| PathBuf::from(format!("{}-build", stem)));
                    if build_dir.exists() {
                        println!("Clearing cache: {}", build_dir.display());
                        let _ = std::fs::remove_dir_all(&build_dir);
                    }
                }
            }

            if let Some(path) = file_path {
                match run_rbv(
                    &path,
                    out_dir.as_deref(),
                    build_wasm,
                    no_stdlib,
                    stdlib_path.clone(),
                ) {
                    Ok(output_path) => {
                        if build_wasm {
                            println!("\n  Ready to serve! Run:");
                            println!("    brief serve {}", output_path.display());
                        }
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                eprintln!("Error: No .rbv file specified");
                eprintln!(
                    "Usage: {} rbv <file.rbv> [--out <dir>] [--no-build]",
                    args[0]
                );
                std::process::exit(1);
            }
        }

        "run" => {
            let mut file_path = None;
            let mut port = None::<u16>;
            let mut open_browser = true;
            let mut watch_mode = false;
            let mut no_cache = false;

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
                } else if arg == "--no-open" {
                    open_browser = false;
                    i += 1;
                } else if arg == "--watch" || arg == "-w" {
                    watch_mode = true;
                    i += 1;
                } else if arg == "--no-cache" {
                    no_cache = true;
                    i += 1;
                } else if arg.ends_with(".rbv") {
                    file_path = Some(PathBuf::from(arg));
                    i += 1;
                } else {
                    i += 1;
                }
            }

            if let Some(path) = file_path {
                // Clear cache if --no-cache is specified
                if no_cache {
                    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("app");
                    let build_dir = std::env::temp_dir().join(format!("brief-run-{}", stem));
                    if build_dir.exists() {
                        println!("Clearing cache: {}", build_dir.display());
                        let _ = std::fs::remove_dir_all(&build_dir);
                    }
                }

                let out_dir = std::env::temp_dir().join(format!(
                    "brief-run-{}",
                    path.file_stem().and_then(|s| s.to_str()).unwrap_or("app")
                ));

                match run_rbv(&path, Some(&out_dir), true, no_stdlib, stdlib_path.clone()) {
                    Ok(output_path) => {
                        let port = port.unwrap_or(8080);
                        let html_file = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("output");
                        let url = format!("http://localhost:{}/{}.html", port, html_file);

                        if open_browser {
                            println!("  Opening browser at {}", url);
                            let _ = open::that(&url);
                        }

                        println!("\n  Server running on http://localhost:{}", port);
                        if watch_mode {
                            println!("  Watch mode enabled - rebuilding on file changes");
                        }
                        println!("  Press Ctrl+C to stop");
                        if let Err(e) = run_serve(&output_path, port) {
                            eprintln!("Server error: {}", e);
                            std::process::exit(1);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                eprintln!("Error: No .rbv file specified");
                eprintln!(
                    "Usage: {} run <file.rbv> [--port <port>] [--no-open]",
                    args[0]
                );
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

        "map" | "wrap" => {
            let is_wrap = command == "wrap";
            let mut mapper = None;
            let mut output_dir = None;
            let mut force = false;
            let mut lib_path = None;

            let mut i = 2;
            while i < args.len() {
                let arg = &args[i];
                if arg == "--mapper" && i + 1 < args.len() {
                    mapper = Some(args[i + 1].clone());
                    i += 2;
                } else if arg == "--out" && i + 1 < args.len() {
                    output_dir = Some(PathBuf::from(&args[i + 1]));
                    i += 2;
                } else if arg == "--force" {
                    force = true;
                    i += 1;
                } else if !arg.starts_with('-') {
                    lib_path = Some(PathBuf::from(arg));
                    i += 1;
                } else {
                    i += 1;
                }
            }

            if let Some(path) = lib_path {
                match run_map_or_wrap(
                    &path,
                    mapper.as_deref(),
                    output_dir.as_deref(),
                    force,
                    is_wrap,
                ) {
                    Ok(_) => {
                        if !is_wrap {
                            println!("  (dry-run complete - no files written)");
                        }
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                eprintln!("Error: No library path specified");
                eprintln!(
                    "Usage: {} {} <library_path> [--mapper <name>] [--out <dir>] [--force]",
                    args[0], command
                );
                std::process::exit(1);
            }
        }

        "install" => {
            run_install();
        }

        "-h" | "--help" | "help" => {
            print_usage(&args[0]);
        }

        _ => {
            if command.ends_with(".bv") {
                let path = PathBuf::from(command);
                let codicil_mode = detect_codicil_project(&path);
                if let Err(_e) = run_check(&path, false, false, false, None, codicil_mode) {
                    std::process::exit(1);
                }
            } else if command.ends_with(".rbv") {
                if let Err(_e) = run_rbv(&PathBuf::from(command), None, true, false, None) {
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
