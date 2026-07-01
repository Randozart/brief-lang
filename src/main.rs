#![allow(unused)]
#![allow(unused_variables)]
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
    analysis, annotator, ast, backend, dbrief, desugarer, errors, fuzz_checker, hardware, hardware_validator, import_resolver, interpreter,
    linkage, lsp, manifest, memory_spec, parser, proof_engine, rbv, typechecker, type_universe, view_compiler,
    target_spec::{self, TargetSpec},
};
use brief_compiler::backend::llvm::EmbeddedConfig;
use notify::Watcher;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Intent: Emit memory spec file if requested
fn emit_memory_spec_if_requested(
    program: &ast::Program,
    out_dir: Option<&Path>,
    stem: &str,
    emit: bool,
    format: &str,
    target: &str,
) {
    if !emit {
        return;
    }

    let mut spec = memory_spec::MemorySpec::new(target);
    spec.collect_from_program(program);

    let out_path = out_dir.map(|d| d.to_path_buf()).unwrap_or_else(|| PathBuf::from("."));
    fs::create_dir_all(&out_path).ok();

    let (content, ext) = if format == "toml" {
        match spec.to_toml() {
            Ok(c) => (c, "toml"),
            Err(e) => {
                eprintln!("Warning: Failed to serialize memory spec to TOML: {}", e);
                return;
            }
        }
    } else {
        match spec.to_json() {
            Ok(c) => (c, "json"),
            Err(e) => {
                eprintln!("Warning: Failed to serialize memory spec to JSON: {}", e);
                return;
            }
        }
    };

    let spec_path = out_path.join(format!("{}_memory_spec.{}", stem, ext));
    if let Err(e) = fs::write(&spec_path, &content) {
        eprintln!("Warning: Failed to write memory spec: {}", e);
    } else {
        println!("  Memory spec written: {}", spec_path.display());
    }
}

/// Intent: format hardware diagnostics.
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

/// Intent: format type errors.
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

/// Intent: format proof errors.
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

/// Intent: strip annotations.
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

/// Intent: strip codicil blocks.
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

/// Intent: detect codicil project.
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

/// Intent: print usage.
fn print_usage(program: &str) {
    eprintln!("Brief Compiler v{}", env!("CARGO_PKG_VERSION"));
    eprintln!();
    eprintln!("Usage: {} <command> [options] [file]", program);
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  check <file>     Type check without execution (fast)");
    eprintln!("  compile <file>   Unified compile with --target (Phase 3)");
    eprintln!("  build <file>        Compile and build binary from .bv/.sbv/.rbv/.srbv/.ebv/.sebv/.abv/.cbv/.dbv");
    eprintln!("                      Pass --llvm for .bv/.sbv/.ebv/.sebv/.abv to emit LLVM IR instead");
    eprintln!("  init [name]      Create new project");
    eprintln!("  import <name>    Add dependency to project");
    eprintln!("  serve [dir]      Serve static files (default: .)");
    eprintln!("  rbv <file>       Compile RBV to browser-ready files");
    eprintln!("  run <file>       Compile, build WASM, serve, and open browser");
    eprintln!("  webstack <file>  Rust + wasm-bindgen glue, compile via wasm-pack");
    eprintln!("  verilog <file>   Compile to SystemVerilog (FPGA, with --tcl flag)");
    eprintln!("  vhdl <file>      Compile to VHDL (FPGA, with PSL assertions)");
    eprintln!("  dbvl <file>      Parse .dbvl and export to JSON (--out, --pretty)");
    eprintln!("  dbvs <file>      Parse .dbvs and export to JSON (--out, --pretty)");
    eprintln!("  dbv <file>       Parse .dbv and export to JSON (--out, --pretty)");
    eprintln!("  deps [check|install|list]  Check or install dependencies from .dbvs/.dbv files");
    eprintln!("  selfhost <file>  Run self-hosted compiler (Brief-in-Brief)");
    eprintln!("  export <bridge.bv> <language>  Compile bridge, generate native wrappers via $!macro");
    eprintln!("  link <lib>       Analyze library, generate bridge .bv cross-referenced against intrinsics");
    eprintln!("  map <lib>        Analyze library and show generated bindings (dry-run)");
    eprintln!("  wrap <lib>       Generate FFI bindings for a library");
    eprintln!("  install         Install 'brief' to ~/.local/bin");
    eprintln!("  lsp             Start Language Server (for IDE integration)");
    eprintln!();
    eprintln!("Compile Options (Phase 3 Unified):");
    eprintln!("  --target <spec> Target spec TOML (e.g., hosted_c.toml)");
    eprintln!("  --out <dir>     Output directory");
    eprintln!();
    eprintln!("Verilog Options:");
    eprintln!("  --hw <file>      Hardware config TOML, .dbv, or .dbvs (for legacy verilog/vhdl targets)");
    eprintln!("  --tcl            Generate TCL build scripts alongside SystemVerilog");
    eprintln!("  --tcl-only       Generate TCL only (skip SystemVerilog generation)");
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
    eprintln!("  --strict             Enforce full pre/postcondition verification (like .sbv/.sebv/.srbv)");
    eprintln!("  --skip-proof         Skip proof verification");
    eprintln!("  --no-stdlib          Disable standard library bindings");
    eprintln!("  --stdlib-path <path> Use custom standard library path");
    eprintln!("  --target <spec>      Target spec TOML (e.g., hosted_c.toml, linux_kernel.toml)");
    eprintln!("  --emit-memory-spec   Output memory allocation spec alongside compiled code");
    eprintln!("  --memory-spec-toml   Output memory spec as TOML (default: JSON)");
    eprintln!("  -v, --verbose        Verbose output");
    eprintln!("  --explain            Show detailed compilation decisions (sig resolution, liveness, folds)");
    eprintln!("  --quiet, --whisper   Minimal output (for CI/automated use)");
    eprintln!("  --dev                Development mode (default): fast compilation, no simplify pass");
    eprintln!("  --prod, --release    Production mode: full optimization (simplify pass enabled)");
    eprintln!("  --simplify-budget N  Max nodes for expression simplification (default dev:0, prod:MAX)");
    eprintln!("  --no-simplify        Disable expression simplification pass");
    eprintln!("  -h, --help           Show this help");
    eprintln!();
    eprintln!("File Extensions:");
    eprintln!("  .bv, .br            Core Brief (specification)");
    eprintln!("  .rbv                Rendered Brief (Brief + View)");
    eprintln!("  .abv                Accelerated Brief (native GPU compilation)");
    eprintln!("  .ebv                Embedded Brief (hardware targets)");
    eprintln!("  .cbv               Circuit Brief (logic graph, synthesizable only)");
    eprintln!("  .sbv                Strict Brief (requires full contracts)");
    eprintln!("  .sebv               Strict Embedded Brief");
    eprintln!("  .srbv               Strict Rendered Brief");
    eprintln!("  .dbv, .dbvs, .dbvl  Data Brief (configuration)");
}

/// Path to runtime C source within the standard library.
const RUNTIME_C_PATH: &str = "lib/runtime/brief_rt.c";

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

/// Intent: run install.
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

/// Intent: run map or wrap.
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

fn run_selfhost(
    file_path: &str,
    backend: &str,
    verbose: bool,
) -> Result<(), String> {
    use std::path::PathBuf;

    let compiler_path = PathBuf::from("lib/compiler/main.bv");

    // Read and parse lib/compiler/main.bv
    let source = std::fs::read_to_string(&compiler_path)
        .map_err(|e| format!("Failed to read {}: {}", compiler_path.display(), e))?;

    let mut parser = parser::Parser::new(&source);
    let program = parser.parse().map_err(|e| format!("Parse error: {}", e))?;

    if verbose {
        eprintln!("[Selfhost] Resolving imports for self-hosted compiler...");
    }

    // Resolve imports
    let mut import_resolver = import_resolver::ImportResolver::new();
    let mut resolved = import_resolver
        .resolve_imports(&program, &compiler_path)
        .map_err(|e| format!("Import error: {}", e))?;

    if verbose {
        eprintln!("[Selfhost] Desugaring...");
    }
    let mut desug = desugarer::Desugarer::new();
    desug.desugar(&mut resolved);

    if verbose {
        eprintln!("[Selfhost] Type checking...");
    }
    let mut tc = typechecker::TypeChecker::new()
        .with_target(typechecker::CompilationTarget::Interpreter);
    let type_errors = tc.check_program(&mut resolved);
    if !type_errors.is_empty() && verbose {
        for err in &type_errors {
            eprintln!("{}", err);
        }
        eprintln!("[Selfhost] Continuing despite type errors (interpreter handles runtime)");
    }

    if verbose {
        eprintln!("[Selfhost] Creating interpreter...");
    }
    let mut interpreter = interpreter::Interpreter::new();
    interpreter.load_program(&resolved);

    // Create the call expression: compile_file(path, backend, verbose_bool)
    let call_expr = ast::Expr::Call(
        "compile_file".to_string(),
        vec![
            ast::Expr::String(file_path.to_string()),
            ast::Expr::String(backend.to_string()),
            ast::Expr::Bool(verbose),
        ],
    );

    if verbose {
        eprintln!("[Selfhost] Calling compile_file...");
    }

    let result = interpreter
        .eval_expr(&call_expr)
        .map_err(|e| format!("Runtime error: {:?}", e))?;

    // Unwrap Result<String, String>
    match result {
        interpreter::Value::Enum(ref result_name, ref variant, ref fields) => {
            if result_name == "Result" {
                match variant.as_str() {
                    "Ok" => {
                        let val = fields.get("value").or_else(|| fields.get("result"));
                        if let Some(val) = val {
                            match val {
                                interpreter::Value::String(s) => println!("{}", s),
                                _ => return Err(format!("Unexpected Ok value type: {:?}", val)),
                            }
                        }
                    }
                    "Err" => {
                        let val = fields.get("value").or_else(|| fields.get("error"));
                        if let Some(val) = val {
                            match val {
                                interpreter::Value::String(s) => {
                                    return Err(format!("Self-host compilation failed: {}", s));
                                }
                                _ => return Err(format!("Error: {:?}", val)),
                            }
                        }
                    }
                    _ => return Err(format!("Unexpected Result variant: {}", variant)),
                }
            } else {
                return Err(format!("Unexpected result type: {:?}", result));
            }
        }
        _ => return Err(format!("Unexpected result: {:?}", result)),
    }

    Ok(())
}

/// Intent: run bind - generate ready-to-use FFI bindings from a foreign library.
fn run_bind(
    lib_path: &Path,
    mapper: Option<&str>,
    output_dir: Option<&Path>,
    force: bool,
    gen_stubs: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use brief_compiler::ffi::metropolitan::MetropolitanHub;
    use brief_compiler::wrapper::{
        analyze_library,
        generator::{
            generate_bindings_dbvs, generate_bridge_bv, generate_foreign_stub,
            write_bind_files,
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

    println!("  Library: {}", lib_name);
    println!("  Mapper: {}", mapper_name);

    let analysis_result = match analyze_library(lib_path, Some(mapper_name)) {
        Ok(result) => {
            println!("  Analyzed {} functions", result.functions.len());
            Some(result)
        }
        Err(e) => {
            eprintln!("  Analysis warning: {}", e);
            None
        }
    };

    let out_dir = output_dir
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("lib/ffi/generated").join(lib_name));

    if !out_dir.exists() {
        fs::create_dir_all(&out_dir)?;
    }

    if let Some(result) = analysis_result {
        // Generate DBVS bindings + bridge.bv + foreign stubs
        write_bind_files(&result, &out_dir, force)?;

        if gen_stubs {
            let hub = MetropolitanHub::new();
            let _ = hub.create_channel(lib_name, mapper_name, 4096, 4096);
            let stub = generate_foreign_stub(&hub, lib_name, mapper_name)?;
            let stub_path = out_dir.join(format!("{}_stub.{}", lib_name, 
                match mapper_name { "c" => "h", "python" => "py", "js" => "js", _ => "h" }
            ));
            fs::write(&stub_path, &stub)?;
            println!("  Foreign stub: {}", stub_path.display());
        }

        println!("\n  Generated files:");
        println!("    {}/bindings.dbvs", out_dir.display());
        println!("    {}/bridge.bv", out_dir.display());
        println!("  Import with: import \"{}\";", out_dir.join("bridge").to_string_lossy());
    } else {
        eprintln!("  Could not analyze library. Create bindings manually.");
    }

    Ok(())
}

/// Intent: is strict extension (`.sbv`, `.sebv`, `.srbv`, `.cbv`).
fn is_strict_extension(file_path: &PathBuf) -> bool {
    let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
    matches!(ext, "sbv" | "sebv" | "srbv" | "cbv")
}

/// Intent: is GPU (`.abv`) extension — always compiles to SPIR-V.
fn is_gpu_extension(file_path: &PathBuf) -> bool {
    let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
    ext == "abv"
}

fn is_embedded_extension(file_path: &PathBuf) -> bool {
    let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
    ext == "ebv" || ext == "sebv"
}

fn is_circuit_extension(file_path: &PathBuf) -> bool {
    let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
    ext == "cbv"
}

/// Intent: run fuzz — parse, typecheck, and verify fuzz cases only.
fn run_fuzz(
    file_path: &PathBuf,
    verbose: bool,
    no_stdlib: bool,
    stdlib_path: Option<PathBuf>,
    codicil_mode: bool,
    strict: bool,
    safe_compile: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(file_path)?;
    let clean_source = if codicil_mode && detect_codicil_project(file_path) {
        strip_codicil_blocks(&source)
    } else {
        source
    };

    let mut parser = parser::Parser::new(&clean_source)
        .with_strict_mode(strict);
    let program = match parser.parse() {
        Ok(prog) => prog,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            return Err("Parse error".into());
        }
    };

    let mut import_resolver = import_resolver::ImportResolver::new()
        .with_strict_mode(strict)
        .with_use_stdlib(!no_stdlib)
        .with_stdlib_path(stdlib_path);
    let mut program = match import_resolver.resolve_imports(&program, file_path) {
        Ok(resolved) => resolved,
        Err(e) => {
            eprintln!("Import error: {}", e);
            return Err("Import error".into());
        }
    };

    program.synthesize_builtin_types();
    program.synthesize_init_txn();

    let mut desug = desugarer::Desugarer::new();
    let mut program = desug.desugar(&program);

    // Template/macro expansion
    {
        let mut macro_ctx = brief_compiler::features::macros::context::MacroContext::new();
        macro_ctx.safe_mode = safe_compile;
        if !safe_compile {
            let _ = brief_compiler::features::macros::expand::expand_templates(&mut program, &mut macro_ctx);
            let _ = brief_compiler::features::macros::expand::expand_macros(&mut program, &mut macro_ctx);
        }
        if let Err(e) = brief_compiler::features::macros::expand::validate_no_compile_time_intrinsics(&program) {
            eprintln!("{}", e);
            return Err("Macro expansion validation failed".into());
        }
    }

    // Typecheck
    let mut tc = typechecker::TypeChecker::new()
        .with_stdlib_config(no_stdlib, None)
        .with_target(typechecker::CompilationTarget::Interpreter);
    let type_errors = tc.check_program(&mut program);
    if !type_errors.is_empty() {
        eprintln!("{}", format_type_errors(&type_errors, file_path.to_str().unwrap_or("main.bv")));
        return Err("Type errors".into());
    }

    // Run fuzz checker
    if has_fuzz_cases(&program) {
        let mut interpreter = interpreter::Interpreter::new();
        interpreter.load_program(&program);

        let fuzz_errors = fuzz_checker::check_fuzz_cases(&program, &mut interpreter);
        let file_path_str = file_path.to_str().unwrap_or("main.bv");
        for err in &fuzz_errors {
            match err {
                errors::FuzzError::Mismatch { function, case_index, expected, actual, .. } => {
                    eprintln!("  ✗ {} case {}: expected {}, got {}", function, case_index, expected, actual);
                }
                errors::FuzzError::MissingBinding { function, case_index, param, .. } => {
                    eprintln!("  ✗ {} case {}: missing binding for '{}'", function, case_index, param);
                }
                errors::FuzzError::InvalidInput { function, case_index, detail, .. } => {
                    eprintln!("  ✗ {} case {}: {}", function, case_index, detail);
                }
                errors::FuzzError::Unverifiable { function, case_index, detail, .. } => {
                    eprintln!("  ⚠ {} case {}: unverifiable — {}", function, case_index, detail);
                }
                errors::FuzzError::Skipped { function, reason, .. } => {
                    if verbose {
                        eprintln!("  - {} (skipped: {})", function, reason);
                    }
                }
                errors::FuzzError::EvaluationError { function, case_index, message, .. } => {
                    eprintln!("  ✗ {} case {}: error — {}", function, case_index, message);
                }
            }
        }

        let has_failures = fuzz_errors.iter().any(|e| {
            !matches!(e, errors::FuzzError::Skipped { .. })
        });

        if has_failures {
            eprintln!("Fuzz verification failed for {}", file_path_str);
            return Err("Fuzz errors".into());
        } else if verbose {
            eprintln!("All fuzz cases passed for {}", file_path_str);
        }
    } else if verbose {
        eprintln!("No fuzz cases found in {}", file_path.to_str().unwrap_or("main.bv"));
    }

    Ok(())
}

/// Check whether a program has any fuzz cases.
fn has_fuzz_cases(program: &ast::Program) -> bool {
    program.items.iter().any(|item| matches!(item, ast::TopLevel::Fuzzed { .. }))
}

/// Intent: run check.
fn run_check(
    file_path: &PathBuf,
    verbose: bool,
    annotate: bool,
    no_stdlib: bool,
    stdlib_path: Option<PathBuf>,
    codicil_mode: bool,
    strict: bool,
    optimize: bool,
    safe_compile: bool,
    macro_budget: Option<u64>,
) -> Result<(), Box<dyn std::error::Error>> {
    let is_gpu = is_gpu_extension(file_path);
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

    let mut parser = parser::Parser::new(&processed_source)
        .with_strict_mode(strict)
        .with_gpu_mode(is_gpu);
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
    let mut import_resolver = import_resolver::ImportResolver::new()
        .with_strict_mode(strict)
        .with_use_stdlib(!no_stdlib)
        .with_stdlib_path(stdlib_path.clone());
    let mut program = match import_resolver.resolve_imports(&program, file_path) {
        Ok(resolved) => resolved,
        Err(e) => {
            eprintln!("Import error: {}", e);
            return Err("Import error".into());
        }
    };

    program.synthesize_builtin_types();
    program.synthesize_init_txn();

    if verbose {
        println!("[Desugar] Desugaring...");
    }
    let mut desug = desugarer::Desugarer::new();
    let mut program = desug.desugar(&program);

    // Phase 1a/1b: Template and macro expansion
    if verbose {
        println!("[MacroExpander] Expanding templates and macros...");
    }
    {
        let mut macro_ctx = brief_compiler::features::macros::context::MacroContext::new();
        macro_ctx.safe_mode = safe_compile;
        if let Some(budget) = macro_budget {
            macro_ctx.budget = budget;
        }
        if !safe_compile {
            let _ = brief_compiler::features::macros::expand::expand_templates(&mut program, &mut macro_ctx);
            let _ = brief_compiler::features::macros::expand::expand_macros(&mut program, &mut macro_ctx);
        }
        // Validate no compile-time-only intrinsics survived expansion
        if let Err(e) = brief_compiler::features::macros::expand::validate_no_compile_time_intrinsics(&program) {
            eprintln!("{}", e);
            return Err("Macro expansion validation failed".into());
        }
    }

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
    let mut pe = proof_engine::ProofEngine::new().with_strict_mode(strict);
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
        println!("[Analysis] All proofs verified");
    }

    // Fuzz checker — verify inline test cases.
    if has_fuzz_cases(&program) {
        if verbose {
            println!("[FuzzChecker] Running fuzz verification...");
        }
        let mut interpreter = interpreter::Interpreter::new();
        interpreter.load_program(&program);
        let fuzz_errors = fuzz_checker::check_fuzz_cases(&program, &mut interpreter);
        let file_path_str = file_path.to_str().unwrap_or("main.bv");
        let has_real_errors = fuzz_errors.iter().any(|e| !matches!(e, errors::FuzzError::Skipped { .. }));
        if !fuzz_errors.is_empty() {
            for err in &fuzz_errors {
                match err {
                    errors::FuzzError::Mismatch { function, case_index, expected, actual, .. } => {
                        eprintln!("  ✗ {} case {}: expected {}, got {}", function, case_index, expected, actual);
                    }
                    _ => {
                        if !matches!(err, errors::FuzzError::Skipped { .. }) {
                            eprintln!("  ✗ {}", err);
                        }
                    }
                }
            }
        }
        if has_real_errors {
            eprintln!("Fuzz verification failed for {}", file_path_str);
            return Err("Fuzz errors".into());
        }
    }

    if verbose {
        println!("[Analysis] Running shared program analysis...");
    }
    // Intent: Run shared program analysis (call graph + parameter ranges) to detect
    //   acyclic subgraphs eligible for optimized scheduling and bounded parameter loops.
    let analysis = backend::analyze_program(&program, optimize);

    // Peephole optimization in --optimize mode
    let program = if optimize {
        if verbose {
            println!("[Optimizer] Running peephole optimization...");
        }
        let transformed = backend::run_peephole(&program, &analysis);
        let removed_count = program.items.iter().map(|i| match i {
            ast::TopLevel::Transaction(t) => t.body.len(),
            _ => 0
        }).sum::<usize>() - transformed.items.iter().map(|i| match i {
            ast::TopLevel::Transaction(t) => t.body.len(),
            _ => 0
        }).sum::<usize>();
        if verbose {
            if analysis.fusable_pairs.len() > 0 {
                println!("[Optimizer]  {} fusable transaction pairs", analysis.fusable_pairs.len());
            }
            if analysis.dataflow_errors.len() > 0 {
                println!("[Optimizer]  {} dataflow diagnostics", analysis.dataflow_errors.len());
            }
            if removed_count > 0 {
                println!("[Optimizer]  Eliminated {} redundant statements", removed_count);
            }
        }
        transformed
    } else {
        program
    };

    // Re-run analysis if optimized to get fresh call graph
    let analysis = if optimize {
        backend::analyze_program(&program, optimize)
    } else {
        analysis
    };

    let has_cycles = analysis.call_graph.has_cycle();
    let mut cg = analysis.call_graph.clone();
    let cycles: Vec<Vec<String>> = cg.find_all_cycles().to_vec();
    let node_count = cg.node_count();
    let edge_count = cg.edge_count();
    let range_count: usize = analysis.param_ranges.ranges.values().map(|m| m.len()).sum();
    if verbose {
        println!("[Analysis] Call graph: {} transactions, {} edges, {} cycle(s)",
            cg.node_count(), cg.edge_count(), cycles.len());
        println!("[Analysis] Parameter ranges for {} transaction parameters", range_count);
        if has_cycles {
            println!("[Analysis]  Warning: cyclic dependencies detected - some transactions cannot use optimized scheduling");
        }
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

/// Intent: run build.
fn run_build(
    file_path: &PathBuf,
    _verbose: bool,
    no_stdlib: bool,
    stdlib_path: Option<PathBuf>,
    out_dir: Option<&Path>,
    _emit_memory_spec: bool,
    _memory_spec_format: &str,
    strict: bool,
    _optimize: bool,
    prod_mode: bool,
    simplify_budget: Option<u64>,
    gpu_offload: bool,
    gpu_backend: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    // Detect source type from extension
    let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
    // Force GPU offload for .abv files
    let gpu_offload = gpu_offload || ext == "abv";
    
    match ext {
        "bv" | "sbv" => {
            // .bv / .sbv files: Compile to native binary via LLVM backend
            println!("Building {} file: compiling via LLVM...", if ext == "sbv" { "Strict Brief" } else { ".bv" });
            if ext == "sbv" {
                println!("  Strict mode: full pre/postcondition verification enforced");
            }
            if prod_mode {
                println!("  Production mode: full optimization enabled");
            }
            let stem = file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
            let out = out_dir.unwrap_or_else(|| std::path::Path::new("."));
            
            // Run LLVM compile with sensible defaults
            let result = run_llvm_compile(file_path, Some(out), None, strict, 256, false, None, true, None, false, false, false, prod_mode, simplify_budget, no_stdlib, stdlib_path.clone(), false, None, false, gpu_offload, gpu_backend, None, None, 10_000);
            match result {
                Ok(ll_path) => {
                    let exe_path = out.join(stem);
                    if exe_path.exists() {
                        println!("  Built executable: {}", exe_path.display());
                        Ok(exe_path)
                    } else {
                        eprintln!("  LLVM output at: {}", ll_path.display());
                        Ok(ll_path)
                    }
                }
                Err(e) => Err(e),
            }
        }
        "abv" => {
            // .abv files: Compile to GPU — always enables SPIR-V offload
            println!("Building Accelerated Brief (.abv) file: compiling via GPU...");
            if prod_mode {
                println!("  Production mode: full optimization enabled");
            }
            let stem = file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
            let out = out_dir.unwrap_or_else(|| std::path::Path::new("."));
            let result = run_llvm_compile(file_path, Some(out), None, strict, 256, false, None, true, None, false, false, false, prod_mode, simplify_budget, no_stdlib, stdlib_path.clone(), false, None, false, gpu_offload, gpu_backend, None, None, 10_000);
            match result {
                Ok(ll_path) => {
                    let exe_path = out.join(stem);
                    if exe_path.exists() {
                        println!("  Built executable: {}", exe_path.display());
                    }
                    if ll_path.exists() {
                        println!("  LLVM IR: {}", ll_path.display());
                    }
                    Ok(exe_path)
                }
                Err(e) => {
                    eprintln!("  Error compiling {}: {}", file_path.display(), e);
                    Err(e)
                }
            }
        }
        "abv" | "sebv" => {
            // .abv / .sebv files: Compile to native binary via GPU SPIR-V + CPU wrapper
            println!("Building {} file: compiling via LLVM with GPU offload...", if ext == "sebv" { "Strict Accel" } else { ".abv" });
            if ext == "sebv" {
                println!("  Strict mode: full pre/postcondition verification enforced");
            }
            if prod_mode {
                println!("  Production mode: full optimization enabled");
            }
            let stem = file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
            let out = out_dir.unwrap_or_else(|| std::path::Path::new("."));
            let result = run_llvm_compile(file_path, Some(out), None, strict, 256, false, None, true, None, false, false, false, prod_mode, simplify_budget, no_stdlib, stdlib_path.clone(), false, None, false, gpu_offload || ext == "sebv", gpu_backend, None, None, 10_000);
            match result {
                Ok(ll_path) => {
                    let exe_path = out.join(stem);
                    if exe_path.exists() {
                        println!("  Built embedded executable: {}", exe_path.display());
                        Ok(exe_path)
                    } else {
                        eprintln!("  LLVM output at: {}", ll_path.display());
                        Ok(ll_path)
                    }
                }
                Err(e) => Err(e),
            }
        }
        "cbv" => {
            // .cbv files: Compile to circuit via CIRCT (no LLVM, no FFI)
            println!("Building Circuit Brief (.cbv) file: compiling via CIRCT...");
            run_cbv(file_path, out_dir)
        }
        "dbv" | "dbvs" | "dbvl" => {
            // .dbv/.dbvs/.dbvl files: Validate and emit JSON
            println!("Building Data Brief ({}) file: validating and emitting...", ext);
            let stem = file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
            let out = out_dir.unwrap_or_else(|| std::path::Path::new("."));
            let source = std::fs::read_to_string(file_path)
                .map_err(|e| format!("Error reading {}: {}", file_path.display(), e))?;
            let doc = dbrief::v2::parse_document(&source)
                .map_err(|e| format!("Parse error: {}", e))?;
            let json = serde_json::to_string_pretty(&doc)
                .map_err(|e| format!("JSON error: {}", e))?;
            let json_path = out.join(format!("{}.json", stem));
            std::fs::write(&json_path, &json)
                .map_err(|e| format!("Error writing {}: {}", json_path.display(), e))?;
            println!("  Parsed {} schemas, {} data groups, {} rules",
                doc.schemas.len(), doc.data_groups.len(), doc.rules.len());
            println!("  Emitted JSON: {}", json_path.display());
            Ok(json_path)
        }
        _ => {
            Err(format!("Unknown file extension: {}. Use .bv, .sbv, .rbv, .srbv, .ebv, .sebv, .abv, .cbv, .dbv, .dbvs, .dbvl", ext).into())
        }
    }
}

/// Intent: run init.
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

/// Intent: run import.
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

/// Intent: run watch.
fn run_watch(
    file_path: PathBuf,
    verbose: bool,
    no_stdlib: bool,
    stdlib_path: Option<PathBuf>,
    optimize: bool,
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
                let strict = is_strict_extension(&file_path);
                if let Err(e) = run_check(
                    &file_path,
                    verbose,
                    false,
                    no_stdlib,
                    stdlib_path.clone(),
                    codicil_mode,
                    strict,
                    optimize,
                    false,  // safe_compile
                    None,   // macro_budget
                ) {
                    eprintln!("Rebuild failed: {}", e);
                }
            }
            Err(e) => eprintln!("Watch error: {}", e),
        }
    }
}

/// Intent: run serve.
fn run_serve(dir: &Path, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};

    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr)?;

    println!("Brief Server");
    println!("Serving {} on http://{}", dir.display(), addr);
    println!("Press Ctrl+C to stop\n");

    /// Intent: get mime type.
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

    /// Intent: handle request.
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

/// Intent: run arm.
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

    let mut import_resolver = import_resolver::ImportResolver::new()
        .with_use_stdlib(!no_stdlib)
        .with_stdlib_path(stdlib_path.clone());
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

    let _analysis = backend::analyze_program(&program, false);

    let mut wasm_gen = backend::webstack::WebstackGenerator::new().with_target(backend::webstack::CodeTarget::Arm);
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

/// Intent: run rust.
fn run_rust(
    _file_path: &PathBuf,
    _out_dir: Option<&Path>,
    _no_stdlib: bool,
    _stdlib_path: Option<PathBuf>,
    _target_spec: Option<TargetSpec>,
    _emit_memory_spec: bool,
    _memory_spec_format: &str,
    _strict: bool,
    _verbose: bool,
    _optimize: bool,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Err("The Rust backend has been removed. Use 'brief build <file>' for LLVM compilation instead.".into())
}

/// Intent: run compile unified.
fn run_compile_unified(args: &[String], strict_flag: bool, optimize_flag: bool) {
    let mut file_path = None;
    let mut target: Option<&str> = None;
    let mut out_dir = None;
    let verbose = args.contains(&"-v".to_string()) || args.contains(&"--verbose".to_string());
    let explain = args.contains(&"--explain".to_string());
    let emit_memory_spec = args.contains(&"--emit-memory-spec".to_string());
    let emit_bindings_dir: Option<String> = args.iter()
        .position(|a| a == "--emit-bindings")
        .and_then(|i| args.get(i + 1).cloned());
    let memory_spec_format = if args.contains(&"--memory-spec-toml".to_string()) {
        "toml"
    } else {
        "json"
    };
    let no_stdlib = args.contains(&"--no-stdlib".to_string()) || args.contains(&"--no-std".to_string());
    let stdlib_path = args
        .iter()
        .position(|a| a == "--stdlib-path")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);

    // Parse arguments
    let mut i = 2;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--target" && i + 1 < args.len() {
            target = Some(&args[i + 1]);
            i += 2;
        } else if arg == "--out" && i + 1 < args.len() {
            out_dir = Some(PathBuf::from(&args[i + 1]));
            i += 2;
        } else if arg.ends_with(".bv") || arg.ends_with(".rbv") || arg.ends_with(".ebv") || arg.ends_with(".cbv")
            || arg.ends_with(".sbv") || arg.ends_with(".srbv") || arg.ends_with(".sebv") {
            file_path = Some(PathBuf::from(arg));
            i += 1;
        } else {
            i += 1;
        }
    }

    let file_path = match file_path {
        Some(p) => p,
        None => {
            eprintln!("Error: No source file specified");
            eprintln!("Usage: {} compile <file> --target <spec.toml>", args[0]);
            std::process::exit(1);
        }
    };

    // Detect source type from extension
    let source_type = if matches!(file_path.extension().and_then(|e| e.to_str()), Some("rbv" | "srbv")) {
        "rendered"
    } else if matches!(file_path.extension().and_then(|e| e.to_str()), Some("ebv" | "sebv")) {
        "embedded"
    } else if matches!(file_path.extension().and_then(|e| e.to_str()), Some("cbv")) {
        "circuit"
    } else {
        "foundational"
    };


    // Infer default target if not specified (Phase 3.3)
    // .bv/.sbv -> hosted_c.toml (default C)
    // .rbv/.srbv -> react_web.toml (React web)
    // .ebv/.sebv -> verilog_fpga.toml (FPGA/embedded)
    let target_spec = if let Some(t) = target {
        let loader = target_spec::TargetSpecLoader::new();
        match loader.load(std::path::Path::new(t)) {
            Ok(spec) => Some(spec),
            Err(e) => {
                eprintln!("Warning: failed to load target spec '{}': {}", t, e);
                None
            }
        }
    } else {
        // No target specified - infer from source type
        let inferred_target = match source_type {
            "rendered" => "react_web.toml",
            "embedded" => "llvm.toml",
            "circuit" => "circt.toml",
            _ => "llvm.toml",
        };
        let loader = target_spec::TargetSpecLoader::new();
        // Try to find the target spec
        if let Some(path) = loader.find(inferred_target) {
            match loader.load(&path) {
                Ok(spec) => {
                    let target_name = spec.target.as_ref().map(|t| t.name.as_str()).unwrap_or(inferred_target);
                    println!("  Note: Inferred target '{}' for .{} files", target_name, 
                        file_path.extension().map(|e| e.to_string_lossy()).unwrap_or_default());
                    Some(spec)
                }
                Err(e) => {
                    eprintln!("  Note: Could not load '{}', using defaults", inferred_target);
                    None
                }
            }
        } else {
            eprintln!("  Note: Could not find '{}', using defaults", inferred_target);
            None
        }
    };

    // Validate capabilities
    let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let is_strict = strict_flag || ext == "sebv" || ext == "srbv" || ext == "sbv";
    
    if let Some(ref spec) = target_spec {
        let target_name = spec.target.as_ref().map(|t| &t.name).unwrap_or(&"default".to_string()).clone();
        
        // .ebv/.sebv files ALWAYS require hardware_triggers capability
        if source_type == "embedded" && !spec.has_capability("hardware_triggers") {
            let prefix = if is_strict { "Error B4001" } else { "Error B4001" };
            eprintln!("{}: Target '{}' lacks required 'hardware_triggers' capability", prefix, target_name);
            eprintln!("  .ebv/.sebv files require target with hardware_triggers support");
            eprintln!("  Hint: Use a target spec with capabilities = [\"logic\", \"hardware_triggers\"]");
            std::process::exit(1);
        }
        
        // .rbv files warn if no reactive_ui capability (view gets stripped)
        // .srbv files REQUIRE reactive_ui capability
        if source_type == "rendered" && !spec.has_capability("reactive_ui") {
            if is_strict {
                eprintln!("Error B4006: Target '{}' lacks required 'reactive_ui' capability", target_name);
                eprintln!("  .srbv files require target with reactive_ui support for verified view-state isomorphism");
                std::process::exit(1);
            } else {
                eprintln!("Warning B4005: Target '{}' lacks 'reactive_ui'; view block will be stripped", target_name);
            }
        }
    };

    println!("Compiling to unified: {}", file_path.display());
    println!("  Source type: {}", source_type);
    if let Some(ref spec) = target_spec {
        println!("  Target: {}", spec.target.as_ref().map(|t| &t.name).unwrap_or(&"default".to_string()));
        println!("  Backend: {}", spec.backend());
    } else {
        println!("  Target: default (llvm)");
        println!("  Backend: llvm");
    }

    // Dispatch to appropriate backend based on target backend
    let backend = target_spec.as_ref().map(|s| s.backend()).unwrap_or_else(|| "llvm".to_string());

    let result: Option<PathBuf> = match backend.as_str() {
        "llvm" => {
            match run_llvm_compile(&file_path, out_dir.as_deref(), target_spec.as_ref(), is_strict, 256, false, None, false, None, false, explain, false, false, None, no_stdlib, stdlib_path.clone(), false, None, false, false, "vulkan", None, emit_bindings_dir.clone(), 10_000) {
                Ok(p) => Some(p),
                Err(e) => { eprintln!("Error: {}", e); None }
            }
        },
        "verilog" => {
            if let Some(ref spec) = target_spec {
                if let Some(hw_path) = spec.codegen.as_ref().and_then(|c| c.hardware_config.as_ref()) {
                    match run_verilog_compile(&file_path, &PathBuf::from(hw_path), out_dir.as_deref(), false, None, false, false, target_spec.as_ref()) {
                        Ok(p) => Some(p),
                        Err(e) => { eprintln!("Error: {}", e); None }
                    }
                } else {
                    eprintln!("Error: verilog backend requires hardware_config in target spec");
                    None
                }
            } else {
                eprintln!("Error: verilog backend requires --target with hardware_config");
                None
            }
        },
        "vhdl" => {
            if let Some(ref spec) = target_spec {
                if let Some(hw_path) = spec.codegen.as_ref().and_then(|c| c.hardware_config.as_ref()) {
                    match run_vhdl_compile(&file_path, &PathBuf::from(hw_path), out_dir.as_deref(), target_spec.as_ref()) {
                        Ok(p) => Some(p),
                        Err(e) => { eprintln!("Error: {}", e); None }
                    }
                } else {
                    eprintln!("Error: vhdl backend requires hardware_config in target spec");
                    None
                }
            } else {
                eprintln!("Error: vhdl backend requires --target with hardware_config");
                None
            }
        },
        "react" => {
            // React requires reactive_ui - use RBV path for now
            // TODO: Implement dedicated React generator for .bv files
            if let Some(ref spec) = target_spec {
                let target_name = spec.target.as_ref().map(|t| t.name.as_str()).unwrap_or("react");
                if target_name.contains("native") {
                    eprintln!("Note: react-native target - using RBV compilation");
                } else if target_name.contains("nextjs") {
                    eprintln!("Note: nextjs target - generating Next.js project structure");
                } else if target_name.contains("vite") {
                    eprintln!("Note: vite target - generating Vite React project");
                }
            }
            // For .rbv files, react works via run_rbv
            // For .bv files, we need a dedicated React generator (future work)
            eprintln!("Note: React backend - .rbv files use RBV path, .bv files need dedicated generator");
            Some(file_path.clone())
        }
        "circt" => {
            match run_cbv(&file_path, out_dir.as_deref()) {
                Ok(p) => Some(p),
                Err(e) => { eprintln!("Error: {}", e); None }
            }
        }
        _ => {
            eprintln!("Error: Unknown backend '{}' in target spec", backend);
            None
        }
    };

    if result.is_none() {
        std::process::exit(1);
    }
}

/// Intent: run c compile.
fn run_c_compile(
    _file_path: &PathBuf,
    _out_dir: Option<&Path>,
    _no_stdlib: bool,
    _stdlib_path: Option<PathBuf>,
    _target: Option<&TargetSpec>,
    _strict: bool,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Err("The C backend has been removed. Use 'brief build <file>' for LLVM compilation instead.".into())
}

/// Intent: run rust compile.
fn run_rust_compile(
    file_path: &PathBuf,
    out_dir: Option<&Path>,
    no_stdlib: bool,
    stdlib_path: Option<PathBuf>,
    target: Option<&TargetSpec>,
    emit_memory_spec: bool,
    memory_spec_format: &str,
    strict: bool,
    verbose: bool,
    optimize: bool,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    run_rust(file_path, out_dir, no_stdlib, stdlib_path, target.cloned(), emit_memory_spec, memory_spec_format, strict, verbose, optimize)
}

/// Intent: run cobol compile.

/// Process a Vivado hardware handoff file (.xsa or xparameters.h) and
/// generate a DBVS schema + DBV target binding.
fn process_hardware_handoff(
    handoff_path: &str,
    target_name: Option<&str>,
    out_dir: Option<&Path>,
) -> Result<(), String> {
    let hw_path = Path::new(handoff_path);
    let out_base = out_dir.unwrap_or_else(|| Path::new("."));

    let ext = hw_path.extension().and_then(|e| e.to_str()).unwrap_or("");

    let peripherals = if ext == "xsa" {
        hardware::handoff::extract_from_xsa(hw_path)?
    } else {
        let content = std::fs::read_to_string(hw_path)
            .map_err(|e| format!("Failed to read {}: {}", handoff_path, e))?;
        hardware::handoff::extract_from_xparameters(&content)
    };

    if peripherals.is_empty() {
        return Err("No peripherals found in handoff file".to_string());
    }

    let target = target_name.unwrap_or_else(|| {
        hw_path.file_stem().and_then(|s| s.to_str()).unwrap_or("unnamed")
    });

    let dbvs_path = out_base.join("hw.dbvs");
    let dbv_path = out_base.join(format!("{}.dbv", target));

    let dbvs_content = hardware::handoff::generate_dbvs(&peripherals);
    let dbv_content = hardware::handoff::generate_dbv(&peripherals, target);

    std::fs::write(&dbvs_path, &dbvs_content)
        .map_err(|e| format!("Failed to write {}: {}", dbvs_path.display(), e))?;
    std::fs::write(&dbv_path, &dbv_content)
        .map_err(|e| format!("Failed to write {}: {}", dbv_path.display(), e))?;

    println!("  Schema: {} ({} registers)", dbvs_path.display(), peripherals.len());
    println!("  Target binding: {} ({} bindings)", dbv_path.display(), peripherals.len());
    println!("  Use: brief llvm program.bv --target-dbv {}", dbv_path.display());

    Ok(())
}

/// Parse a DBV target binding file and extract alias → address mappings.
/// Returns a map of alias name to physical u64 address for MMIO codegen.
fn process_target_dbv(_dbv_path: &str) -> Result<HashMap<String, u64>, String> {
    let dbv_path = Path::new(_dbv_path);
    if !dbv_path.exists() {
        return Err(format!("Target DBV file not found: {}", _dbv_path));
    }
    if !dbv_path.extension().map_or(false, |e| e == "dbv") {
        return Err(format!("Target DBV file must have .dbv extension: {}", _dbv_path));
    }

    let content = std::fs::read_to_string(dbv_path)
        .map_err(|e| format!("Failed to read {}: {}", _dbv_path, e))?;

    let addresses = hardware::handoff::extract_target_addresses(&content)?;

    println!("  Target DBV: {} ({} address bindings)", dbv_path.display(), addresses.len());

    Ok(addresses)
}

/// Resolve a link dependency path to an actual file on disk.
/// Search order: project-relative → lib/runtime/ → lib/std/c/ → absolute path.
fn resolve_link_source(link_path: &str, source_dir: &Path) -> Option<PathBuf> {
    let relative = link_path.strip_prefix("link/").unwrap_or(link_path);

    // 1. Try project-relative path (canonicalize to resolve ..)
    let project_path = source_dir.join(relative);
    if let Ok(canonical) = project_path.canonicalize() {
        if canonical.exists() {
            return Some(canonical);
        }
    }

    // 2. Try lib/runtime/<path>
    let runtime_path = Path::new("lib/runtime").join(relative);
    if let Ok(canonical) = runtime_path.canonicalize() {
        if canonical.exists() {
            return Some(canonical);
        }
    }

    // 3. Try lib/std/c/<path>
    let stdc_path = Path::new("lib/std/c").join(relative);
    if let Ok(canonical) = stdc_path.canonicalize() {
        if canonical.exists() {
            return Some(canonical);
        }
    }

    // 4. Try BRIEF_STDLIB_PATH
    if let Ok(stdlib_path) = std::env::var("BRIEF_STDLIB_PATH") {
        let env_path = Path::new(&stdlib_path).join(relative);
        if let Ok(canonical) = env_path.canonicalize() {
            if canonical.exists() {
                return Some(canonical);
            }
        }
    }

    // 5. Try as absolute path
    let abs_path = Path::new(link_path);
    if let Ok(canonical) = abs_path.canonicalize() {
        if canonical.exists() {
            return Some(canonical);
        }
    }

    eprintln!("  Warning: link source not found: {} (searched project, lib/runtime/, lib/std/c/, BRIEF_STDLIB_PATH)", link_path);
    None
}

/// A single foreign module to be compiled to bitcode and linked via LTO.
struct LinkModule {
    source: PathBuf,
    lang: ast::LinkLanguage,
}

/// Compile a foreign source file to LLVM bitcode.
fn compile_to_bitcode(source: &Path, lang: ast::LinkLanguage, output: &Path, has_thread_pool: bool) -> Option<()> {
    let result = match lang {
        ast::LinkLanguage::C => {
            let mut cmd = std::process::Command::new("clang");
            cmd.args(["-c", "-emit-llvm", "-O2", "-fno-stack-protector"]);
            if has_thread_pool {
                cmd.arg("-DBRIEF_THREAD_POOL");
            }
            cmd.arg("-o").arg(output).arg(source);
            cmd.output().ok()
        }
        ast::LinkLanguage::Cpp => {
            let mut cmd = std::process::Command::new("clang++");
            cmd.args(["-c", "-emit-llvm", "-O2", "-fno-stack-protector"]);
            cmd.arg("-o").arg(output).arg(source);
            cmd.output().ok()
        }
        ast::LinkLanguage::Rust => {
            let mut cmd = std::process::Command::new("rustc");
            cmd.args(["--emit=llvm-bc", "-C", "opt-level=3", "-o"]);
            cmd.arg(output).arg(source);
            cmd.output().ok()
        }
        ast::LinkLanguage::Zig => {
            let mut cmd = std::process::Command::new("zig");
            cmd.args(["build-obj", "--emit-llvm-ir", "-O", "ReleaseFast", "-o"]);
            cmd.arg(output).arg(source);
            cmd.output().ok()
        }
        ast::LinkLanguage::Python => {
            let mut cmd = std::process::Command::new("codon");
            cmd.args(["build", "--emit-llvm", "-O3", "-o"]);
            cmd.arg(output).arg(source);
            cmd.output().ok()
        }
        ast::LinkLanguage::Java => {
            // javac → native-image --llvm --emit-llvm-bc
            let class_name = source.file_stem().and_then(|s| s.to_str()).unwrap_or("Main");
            let class_dir = output.parent().unwrap_or(Path::new("."));
            let javac = std::process::Command::new("javac")
                .arg("-d").arg(class_dir)
                .arg(source)
                .output().ok()?;
            if !javac.status.success() {
                let stderr = String::from_utf8_lossy(&javac.stderr);
                eprintln!("  Error compiling Java '{}': {}", source.display(), stderr.lines().next().unwrap_or("unknown error"));
                return None;
            }
            let class_file = class_dir.join(format!("{}.class", class_name));
            let mut cmd = std::process::Command::new("native-image");
            cmd.args(["--llvm", "--emit-llvm-bc", "-O3", "-o"])
                .arg(output)
                .arg(&class_file);
            cmd.output().ok()
        }
        ast::LinkLanguage::AssemblyScript => {
            // asc → .wasm → wasm2llvm → .bc
            let wasm_path = output.with_extension("wasm");
            let asc = std::process::Command::new("asc")
                .args(["--optimize", "--outFile"])
                .arg(&wasm_path)
                .arg(source)
                .output().ok()?;
            if !asc.status.success() {
                let stderr = String::from_utf8_lossy(&asc.stderr);
                eprintln!("  Error compiling AssemblyScript '{}': {}", source.display(), stderr.lines().next().unwrap_or("unknown error"));
                return None;
            }
            let mut cmd = std::process::Command::new("wasm2llvm");
            cmd.args(["--out"]).arg(output).arg(&wasm_path);
            cmd.output().ok()
        }
        ast::LinkLanguage::Bitcode => {
            fs::copy(source, output).ok()?;
            return Some(());
        }
        ast::LinkLanguage::Object => {
            eprintln!("  Warning: cannot LTO object file '{}' — compiling separately", source.display());
            return None;
        }
    };

    match result {
        Some(output) if output.status.success() => Some(()),
        Some(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("  Error compiling {}: {}", source.display(), stderr.lines().next().unwrap_or("unknown error"));
            None
        }
        None => {
            eprintln!("  Error: failed to invoke compiler for {}", source.display());
            None
        }
    }
}

/// Generic multi-module LTO pipeline: compile all foreign sources to bitcode,
/// merge with program IR, optimize, and lower to object code.
fn link_and_optimize(
    out_base: &Path,
    stem: &str,
    ll_file: &Path,
    link_modules: &[LinkModule],
    llvm_extra_flags: &[String],
    has_thread_pool: bool,
) -> Option<PathBuf> {
    // Check for required tools
    let clang_ok = std::process::Command::new("clang")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    let llvm_link_ok = std::process::Command::new("llvm-link")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    let llvm_as_ok = std::process::Command::new("llvm-as")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !clang_ok || !llvm_link_ok || !llvm_as_ok {
        return None;
    }

    let prog_bc = out_base.join(format!("{}.bc", stem));
    let merged_bc = out_base.join(format!("{}_merged.bc", stem));
    let merged_opt_bc = out_base.join(format!("{}_merged.opt.bc", stem));
    let obj_path = out_base.join(format!("{}.o", stem));

    // Step 1: compile all foreign sources to bitcode
    let mut foreign_bc_paths: Vec<PathBuf> = Vec::new();
    for module in link_modules {
        let bc_name = format!("{}_{}.bc", stem, foreign_bc_paths.len());
        let bc_path = out_base.join(&bc_name);
        if compile_to_bitcode(&module.source, module.lang, &bc_path, has_thread_pool).is_some() {
            foreign_bc_paths.push(bc_path);
        }
    }

    if foreign_bc_paths.is_empty() {
        return None;
    }

    // Step 2: convert program .ll → .bc
    let as_status = std::process::Command::new("llvm-as")
        .args(["-o"])
        .arg(&prog_bc)
        .arg(ll_file)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok();
    if as_status.map_or(true, |s| !s.success()) {
        return None;
    }

    // Step 3: merge all bitcode modules
    let mut link_cmd = std::process::Command::new("llvm-link");
    link_cmd.args(["-o"]).arg(&merged_bc).arg(&prog_bc);
    for bc_path in &foreign_bc_paths {
        link_cmd.arg(bc_path);
    }
    let link_output = link_cmd
        .output()
        .ok();
    match link_output {
        Some(out) if out.status.success() => {},
        Some(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            eprintln!("  llvm-link error: {}", stderr.lines().next().unwrap_or("unknown error"));
            return None;
        }
        None => { return None; }
    }

    // Step 4: opt -O3 on merged module
    let opt_status = {
        let mut cmd = std::process::Command::new("opt");
        cmd.args(["-O3", "-mtriple=x86_64-pc-linux-gnu", "-ffast-math", "-o"]);
        cmd.arg(&merged_opt_bc);
        cmd.arg(&merged_bc);
        for flag in llvm_extra_flags {
            cmd.arg(flag);
        }
        cmd.output()
    };
    match opt_status {
        Ok(out) if out.status.success() => {},
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            eprintln!("  opt error: {}", stderr.lines().next().unwrap_or("unknown error"));
            return None;
        }
        Err(_) => { return None; }
    }

    // Step 5: llc → object
    let llc_status = std::process::Command::new("llc")
        .args(["-filetype=obj", "-O3", "--mcpu=native", "-o"])
        .arg(&obj_path)
        .arg(&merged_opt_bc)
        .status()
        .ok();
    if llc_status.map_or(true, |s| !s.success()) {
        return None;
    }

    Some(obj_path)
}

 fn run_llvm_compile(
     file_path: &PathBuf,
     out_dir: Option<&Path>,
     target: Option<&TargetSpec>,
     _strict: bool,
     optimize_budget: u64,
     optimize_report: bool,
     optimize_size: Option<u64>,
     dead_info_disabled: bool,
     mmio_addresses: Option<HashMap<String, u64>>,
     pgo_generate: bool,
      explain: bool,
      dump_layout: bool,
      prod_mode: bool,
      simplify_budget: Option<u64>,
      no_stdlib: bool,
      stdlib_path: Option<PathBuf>,
      safe_compile: bool,
      macro_budget: Option<u64>,
      emit_remarks: bool,
      gpu_offload: bool,
      gpu_backend: &str,
      embedded_config: Option<EmbeddedConfig>,
      emit_bindings_dir: Option<String>,
      txn_convergence_max_iterations: u64,
  ) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let is_gpu = is_gpu_extension(file_path);
    let is_embedded = is_embedded_extension(file_path);
    // Force GPU offload for .abv files
    let gpu_offload = gpu_offload || is_gpu;

    println!("Compiling to LLVM IR: {} {}", file_path.display(),
        if is_gpu { "(GPU — .abv)" } else { "" });

    let source = fs::read_to_string(file_path)?;
    let clean_source = strip_annotations(&source);

    let mut parser = parser::Parser::new(&clean_source)
        .with_strict_mode(_strict)
        .with_gpu_mode(is_gpu);
    let mut program = parser
        .parse()
        .map_err(|e| format!("Parse error: {}", e))?;

    let mut import_resolver = import_resolver::ImportResolver::new()
        .with_strict_mode(_strict)
        .with_use_stdlib(!no_stdlib)
        .with_stdlib_path(stdlib_path);
    let mut program = import_resolver
        .resolve_imports(&program, file_path)
        .map_err(|e| format!("Import error: {}", e))?;

    program.synthesize_builtin_types();
    program.synthesize_init_txn();

    let mut schema_aliases: HashMap<String, crate::dbrief::DbriefType> = HashMap::new();
    let mut schema_imports: Vec<String> = Vec::new();
    for item in &program.items {
        if let crate::ast::TopLevel::Import(import) = item {
            let path_str = import.path.join("/");
            if path_str.ends_with(".dbvs") {
                schema_imports.push(path_str.clone());
                let source_dir = file_path.parent().unwrap_or(std::path::Path::new("."));
                let full_path = source_dir.join(&path_str);
                match std::fs::read_to_string(&full_path) {
                    Ok(content) => {
                        match crate::dbrief::parse_dbvs(&content) {
                            Ok(dbvs) => {
                                for alias in &dbvs.aliases {
                                    schema_aliases.insert(alias.name.clone(), alias.alias_type.clone());
                                }
                                for reg in &dbvs.registers {
                                    if let Some(ref name) = reg.name {
                                        if !schema_aliases.contains_key(name) {
                                            schema_aliases.insert(name.clone(), reg.register_type.clone());
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                return Err(format!("HW005: failed to parse schema {}: {}", path_str, e).into());
                            }
                        }
                    }
                    Err(e) => {
                        return Err(format!("HW006: schema file not found {}: {}", path_str, e).into());
                    }
                }
            }
        }
    }

    let mut mmio_addresses = mmio_addresses;
    if mmio_addresses.is_none() && !schema_imports.is_empty() {
        let source_dir = file_path.parent().unwrap_or(std::path::Path::new("."));
        let mut search_dirs = vec![source_dir.to_path_buf()];
        if let Some(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR").ok() {
            search_dirs.push(std::path::PathBuf::from(&manifest_dir).join("lib").join("targets"));
        }
        // Also check repo root relative to source
        if let Ok(cargo) = std::env::current_dir() {
            search_dirs.push(cargo.join("lib").join("targets"));
        }
        let mut candidates: Vec<(String, String)> = Vec::new();
        for search_dir in &search_dirs {
            if let Ok(entries) = std::fs::read_dir(search_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map_or(false, |e| e == "dbv") {
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            if let Ok(dbv_prog) = crate::dbrief::parse_dbrief(&content) {
                                for imp in &dbv_prog.imports {
                                    let imp_path = imp.path.clone();
                                    if schema_imports.iter().any(|s| {
                                        s.ends_with(&imp_path) || imp_path.ends_with(s.as_str())
                                            || s == &imp_path
                                    }) {
                                        candidates.push((path.display().to_string(), imp_path));
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        candidates.dedup_by(|a, b| a.0 == b.0);
        match candidates.len() {
            1 => {
                let (path, _) = &candidates[0];
                eprintln!("info: auto-detected target binding: {}", path);
                match process_target_dbv(path) {
                    Ok(map) => { mmio_addresses = Some(map); }
                    Err(e) => { eprintln!("Warning: auto-detected target DBV could not be processed: {}", e); }
                }
            }
            0 if !schema_imports.is_empty() => {
                eprintln!("info: no target binding found for schema imports {:?}. Use --target-dbv <file.dbv>", schema_imports);
            }
            n if n > 1 => {
                let mut paths: Vec<String> = candidates.iter().map(|(p, _)| p.clone()).collect();
                paths.dedup();
                eprintln!("info: multiple target bindings found for schema imports: {}", paths.join(", "));
                eprintln!("info: use --target-dbv to select one explicitly");
            }
            _ => {}
        }
    }

    if !schema_aliases.is_empty() {
        if let Some(ref addr_map) = mmio_addresses {
            let cross_diags = brief_compiler::analysis::schema_validator::cross_validate(&schema_aliases, addr_map);
            for diag in &cross_diags {
                let sev = match diag.severity {
                    crate::errors::Severity::Error => "error",
                    crate::errors::Severity::Warning => "warning",
                    _ => "info",
                };
                eprintln!("{}: {}: {}", sev, diag.code, diag.title);
                for exp in &diag.explanation {
                    eprintln!("  {}", exp);
                }
            }
            let has_errors = cross_diags.iter().any(|d| d.severity == crate::errors::Severity::Error);
            if has_errors {
                return Err("cross-validation errors in schema ↔ target bindings".into());
            }
        }
    }

    let mut desug = desugarer::Desugarer::new();
    let mut program = desug.desugar(&program);

    // Phase 1a/1b: Template and macro expansion
    {
        let mut macro_ctx = brief_compiler::features::macros::context::MacroContext::new();
        macro_ctx.safe_mode = safe_compile;
        macro_ctx.txn_convergence_max_iterations = txn_convergence_max_iterations;
        if let Some(budget) = macro_budget {
            macro_ctx.budget = budget;
        }
        if !safe_compile {
            let _ = brief_compiler::features::macros::expand::expand_templates(&mut program, &mut macro_ctx);
            let _ = brief_compiler::features::macros::expand::expand_macros(&mut program, &mut macro_ctx);
        }
        // Validate no compile-time-only intrinsics survived expansion
        if let Err(e) = brief_compiler::features::macros::expand::validate_no_compile_time_intrinsics(&program) {
            eprintln!("{}", e);
            return Err("Macro expansion validation failed".into());
        }
    }

    let link_deps: Vec<crate::ast::LinkDependency> = {
        let mut seen = std::collections::HashSet::new();
        let mut deps: Vec<_> = program.items.iter()
            .filter_map(|item| match item {
                crate::ast::TopLevel::LinkDependency(dep) => {
                    if seen.insert(dep.path.clone()) { Some(dep.clone()) } else { None }
                }
                _ => None,
            })
            .collect();

        // Auto-link brief_rt.c for all native builds.
        // brief_rt.c provides support for complex intrinsics (tty_raw_mode,
        // read_file, spawn_with_output, etc.) and thread pool barriers.
        // Even if no @ link triggers exist, these shims may be called by
        // the LLVM backend output. The linker drops unused symbols.
        if !seen.contains("link/brief_rt.c") {
            deps.push(crate::ast::LinkDependency {
                path: "link/brief_rt.c".to_string(),
                source_lang: crate::ast::LinkLanguage::C,
            });
        }

        // 2026-06-13: Auto-link .c files referenced by frgn `from` paths.
        // Any frgn foo(x: T) -> U from "*.c" is auto-compiled to bitcode and linked.
        for item in &program.items {
            if let crate::ast::TopLevel::ForeignBinding { name: _, toml_path, .. } = item {
                if toml_path.ends_with(".c") && !seen.contains(toml_path) {
                    seen.insert(toml_path.clone());
                    deps.push(crate::ast::LinkDependency {
                        path: toml_path.clone(),
                        source_lang: crate::ast::LinkLanguage::C,
                    });
                }
            }
        }

        deps
    };

    // Phase 3.5: Build the Type Universe for projection resolution
    let tu = type_universe::TypeUniverse::build(&program);

    let comp_target = if is_embedded {
        typechecker::CompilationTarget::Embedded
    } else if is_circuit_extension(file_path) {
        typechecker::CompilationTarget::Circuit
    } else {
        typechecker::CompilationTarget::Interpreter
    };
    let mut tc = typechecker::TypeChecker::new()
        .with_target(comp_target)
        .with_type_universe(tu.clone());
    let type_errors = tc.check_program(&mut program.clone());
    if !type_errors.is_empty() {
        return Err(format!("Type errors: {}", format_type_errors(&type_errors, file_path.to_str().unwrap_or("main.bv"))).into());
    }

    // Run shared program analysis
    let _analysis = backend::analyze_program(&program, false);

    // Run simplify pass (expression-level algebraic rewriting)
    // Only enabled in --prod/--release mode, or when --simplify-budget is explicitly set.
    {
        let sb = simplify_budget.unwrap_or(if prod_mode { u64::MAX } else { 0 });
        if sb > 0 {
            analysis::equality_saturation::simplify_program(&mut program, sb);
        }
    }

    let mut llvm_backend = crate::backend::llvm::LlvmBackend::new()
        .with_optimize_budget(optimize_budget)
        .with_optimize_report(optimize_report)
        .with_schema_aliases(schema_aliases)
        .with_explain(explain)
        .with_emit_remarks(emit_remarks)
        .with_dump_layout(dump_layout)
        .with_gpu_offload(gpu_offload)
        .with_gpu_backend(gpu_backend.to_string())
        .with_embedded_mode(is_embedded)
        .with_type_universe(tu.clone());
    if dead_info_disabled {
        llvm_backend = llvm_backend.with_dead_info_disabled(true);
    }
    if let Some(byte_limit) = optimize_size {
        llvm_backend = llvm_backend.with_optimize_size(byte_limit);
    }
    if let Some(spec) = target.cloned() {
        llvm_backend = llvm_backend.with_spec(spec);
    }
    if let Some(addrs) = mmio_addresses {
        llvm_backend = llvm_backend.with_mmio_addresses(addrs);
    }

    if pgo_generate {
        let has_guarded = program.items.iter().any(|item| {
            if let crate::ast::TopLevel::Transaction(txn) = item {
                txn.body.iter().any(|stmt| matches!(stmt, crate::ast::Statement::Guarded { .. }))
            } else {
                false
            }
        });
        if has_guarded {
            let profile = brief_compiler::analysis::pgo::run_profile(&program, 10, txn_convergence_max_iterations);
            if brief_compiler::analysis::pgo::has_pgo_candidate(&profile, 100) {
                let skewed = profile.branch_counts.iter()
                    .filter(|(_, (t, f))| *t > 0 || *f > 0)
                    .count();
                eprintln!("info: PGO profile attached: {} skewed branches", skewed);
                llvm_backend = llvm_backend.with_pgo_profile(profile);
            } else {
                eprintln!("info: PGO skipped: all branches balanced (no layout benefit)");
            }
        } else {
            eprintln!("info: PGO skipped: no guarded statements in program");
        }
    }

    // Phase 10: Watchdog preemptibility analysis
    {
        let watchdog_errors = brief_compiler::analysis::watchdog::analyze(&program);
        if !watchdog_errors.is_empty() {
            for err in &watchdog_errors {
                match err {
                    brief_compiler::analysis::watchdog::WatchdogError::UnknownTrigger(txn, trigger)
                    | brief_compiler::analysis::watchdog::WatchdogError::NoHandler(txn, trigger) => {
                        eprintln!("error: {}", err);
                    }
                    _ => {
                        // Optional watchdog warnings for non-fatal issues
                        eprintln!("warning: {}", err);
                    }
                }
            }
            // Hard errors (UnknownTrigger, NoHandler) — compilation fails
            let has_fatal = watchdog_errors.iter().any(|e| {
                matches!(e, brief_compiler::analysis::watchdog::WatchdogError::UnknownTrigger(..)
                         | brief_compiler::analysis::watchdog::WatchdogError::NoHandler(..))
            });
            if has_fatal {
                std::process::exit(1);
            }
        }
    }

    let output = llvm_backend.generate(&program);

    for warning in llvm_backend.warnings() {
        eprintln!("{}", warning);
    }

    if emit_remarks {
        for remark in llvm_backend.remarks() {
            eprintln!("{}", remark.format());
        }
    }

    if optimize_report {
        for line in llvm_backend.report() {
            println!("{}", line);
        }
    }

    let stem = file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
    let output_file = out_dir.unwrap_or_else(|| std::path::Path::new(".")).join(format!("{}.ll", stem));
    fs::write(&output_file, &output)?;
    println!("  Output: {}", output_file.display());

    // If --gpu-offload was used, attempt SPIR-V compilation as well.
    if gpu_offload && !llvm_backend.spirv_kernels().is_empty() {
        let spv_file = out_dir.unwrap_or_else(|| std::path::Path::new(".")).join(format!("{}.spv", stem));
        let mut spirv_total = Vec::new();
        for kernel_ir in llvm_backend.spirv_kernels() {
            if let Ok(binary) = crate::backend::llvm::gpu::compile_to_spirv(kernel_ir) {
                spirv_total.extend_from_slice(&binary);
            }
        }
        if !spirv_total.is_empty() {
            fs::write(&spv_file, &spirv_total)?;
            println!("  GPU kernels: {} ({} bytes)", spv_file.display(), spirv_total.len());
        }
    }

    // ── Autogenous Binding Generation ──
    // When --emit-bindings <dir> is passed, generate C/Rust/Python bindings
    // from the TypeUniverse and any #export directives.
    if let Some(ref bind_dir) = emit_bindings_dir {
        let mut exports = Vec::new();
        for item in &program.items {
            match item {
                crate::ast::TopLevel::Definition(defn) => {
                    let export_name = crate::backend::llvm::LlvmBackend::get_export_name(&defn.modifiers);
                    if let Some(ename) = export_name {
                        let params: Vec<crate::backend::bindgen::ExportParam> = defn.parameters.iter()
                            .map(|(n, t)| crate::backend::bindgen::ExportParam {
                                name: n.clone(),
                                ty: t.clone(),
                            }).collect();
                        exports.push(crate::backend::bindgen::ExportDecl {
                            export_name: ename,
                            orig_name: defn.name.clone(),
                            params,
                            ret_ty: crate::ast::Type::Int,
                        });
                    }
                }
                _ => {}
            }
        }
        if !exports.is_empty() {
            match crate::backend::bindgen::emit_all(&tu, &exports, bind_dir) {
                Ok(()) => println!("  Bindings: {}/brief_types.h, brief_bindings.rs, brief_bindings.py", bind_dir),
                Err(e) => eprintln!("error: bindgen: {}", e),
            }
        }
    }

    if !link_deps.is_empty() {
        let out_base = out_dir.unwrap_or(std::path::Path::new("."));
        let exe_path = out_base.join(stem);
        // Detect whether output has wake triggers
        let has_wake = output.contains("@llvm.wake_triggers");
        // Detect whether output uses thread pool for async dispatch
        let has_thread_pool = output.contains("@llvm.thread_pool");

        // Resolve each link dependency to an actual file path.
        // Search: BRIEF_STDLIB_PATH / lib/ / project root / absolute path.
        let source_dir = file_path.parent().unwrap_or(std::path::Path::new("."));
        let link_modules: Vec<LinkModule> = link_deps.iter().filter_map(|dep| {
            let resolved = resolve_link_source(&dep.path, source_dir)?;
            Some(LinkModule { source: resolved, lang: dep.source_lang })
        }).collect();

        let llvm_flags = llvm_backend.llvm_extra_flags();

        // Try LTO pipeline first: compile all foreign sources to bitcode, merge with
        // program IR, run opt -O3 on the merged module, then llc. This enables inlining
        // across language boundaries.
        let lto_obj = if !link_modules.is_empty() {
            link_and_optimize(&out_base, stem, &output_file, &link_modules, &llvm_flags, has_thread_pool)
        } else {
            None
        };
        if let Some(lto_obj_path) = lto_obj {
            let merged_names: Vec<&str> = link_modules.iter().map(|_| "bc").collect();
            println!("  LTO-merged: program.bc + {} foreign modules → optimized", link_modules.len());

            let mut link_cmd = std::process::Command::new("cc");
            if is_embedded {
                link_cmd.args(["-ffreestanding", "-nostdlib", "-nostartfiles", "-o"]).arg(&exe_path).arg(&lto_obj_path);
                if let Some(ref config) = embedded_config {
                    if let Some(ref ls) = config.linker_script {
                        link_cmd.arg(format!("-T{}", ls));
                    }
                }
            } else {
                link_cmd.args(["-O2", "-no-pie", "-o"]).arg(&exe_path).arg(&lto_obj_path);
                link_cmd.args(["-lm", "-lpthread"]);
                if has_wake {
                    link_cmd.arg("-lrt");
                }
            }
            let link_status = link_cmd.status();
            match link_status {
                Ok(status) if status.success() => {
                    println!("  Binary: {}", exe_path.display());
                }
                _ => {
                    eprintln!("  Warning: linking failed. Link manually:");
                    if is_embedded {
                        eprintln!("    cc -ffreestanding -nostdlib -nostartfiles {} -o {}", lto_obj_path.display(), exe_path.display());
                    } else {
                        eprintln!("    cc -no-pie {} -o {} -lm -lpthread", lto_obj_path.display(), exe_path.display());
                        if has_wake { eprintln!("    (add -lrt)"); }
                    }
                }
            }
            return Ok(output_file);
        }

        // Try LTO pipeline with brief_rt.c first (enables cross-module inlining)
        let source_dir = file_path.parent().unwrap_or(std::path::Path::new("."));
        let rt_c_path = resolve_link_source("link/brief_rt.c", source_dir)
            .unwrap_or_else(|| out_base.join("brief_rt.c"));
        let rt_o_path = out_base.join("brief_rt.o");
        let ll_o_path = out_base.join(format!("{}.o", stem));

        if rt_c_path.exists() {
            let lto_module = LinkModule { source: rt_c_path.clone(), lang: ast::LinkLanguage::C };
            if let Some(lto_obj) = link_and_optimize(
                &out_base, stem, &output_file, &[lto_module], &llvm_flags, has_thread_pool
            ) {
                println!("  LTO: program + brief_rt.c bitcode merged and optimized");
                let mut link_cmd = std::process::Command::new("cc");
                if is_embedded {
                    link_cmd.args(["-ffreestanding", "-nostdlib", "-nostartfiles", "-o"]).arg(&exe_path).arg(&lto_obj);
                    if let Some(ref config) = embedded_config {
                        if let Some(ref ls) = config.linker_script {
                            link_cmd.arg(format!("-T{}", ls));
                        }
                    }
                } else {
                    link_cmd.args(["-O2", "-no-pie", "-o"]).arg(&exe_path).arg(&lto_obj);
                    link_cmd.args(["-lm", "-lpthread"]);
                    if has_wake {
                        link_cmd.arg("-lrt");
                    }
                }
                if link_cmd.status().ok().map_or(true, |s| !s.success()) {
                    eprintln!("  Warning: LTO linking failed. Trying cc fallback.");
                } else {
                    println!("  Binary: {}", exe_path.display());
                    return Ok(output_file);
                }
            }
        }

        // LTO not available or failed — fall back to standard cc compilation and object linking.
        let cc_status = {
            let mut cmd = std::process::Command::new("cc");
            cmd.args(["-c", "-O2", "-ffreestanding", "-fno-stack-protector", "-fno-builtin"]);
            if has_thread_pool {
                cmd.arg("-DBRIEF_THREAD_POOL");
            }
            cmd.arg("-o").arg(&rt_o_path).arg(&rt_c_path);
            cmd.status()
                .map_err(|e| format!("Failed to invoke cc: {}. Is a C compiler installed?", e))?
        };
        if !cc_status.success() {
            eprintln!("  Warning: cc compilation failed. Compile manually:");
            eprintln!("    cc -c {} -o {}", rt_c_path.display(), rt_o_path.display());
            eprintln!("    llc {} -filetype=obj --mcpu=native -o {}.o", output_file.display(), stem);
            return Ok(output_file);
        }
        println!("  Runtime object: {}", rt_o_path.display());

        // Opt + llc pipeline for standalone IR (backup path)
        let opt_ll_path = out_base.join(format!("{}.opt.ll", stem));
        let mut opt_cmd = std::process::Command::new("opt");
        opt_cmd.args(["-O3", "-S", "-mtriple=x86_64-pc-linux-gnu", "-o"]);
        opt_cmd.arg(&opt_ll_path);
        opt_cmd.arg(&output_file);
        for flag in &llvm_flags {
            opt_cmd.arg(flag);
        }
        let ll_source = match opt_cmd.status() {
            Ok(status) if status.success() => {
                println!("  Optimized: {}", opt_ll_path.display());
                &opt_ll_path
            }
            _ => &output_file,
        };

        let mut llc_cmd = std::process::Command::new("llc");
        llc_cmd.args(["-filetype=obj", "-O3", "--mcpu=native"]);
        let llc_status = llc_cmd
            .arg("-o").arg(&ll_o_path).arg(ll_source)
            .status();
        match llc_status {
            Ok(status) if status.success() => {
                println!("  Object: {}", ll_o_path.display());
            }
            _ => {
                eprintln!("  Warning: LLVM toolchain not found or failed. Compile manually:");
                eprintln!("    opt -O2 -S {} -o {}.opt.ll", output_file.display(), stem);
                eprintln!("    llc {}.opt.ll -filetype=obj -o {}", stem, ll_o_path.display());
                print!("    ld {}.o {} -o {}", stem, rt_o_path.display(), stem);
                print!(" -lpthread");
                if has_wake { print!(" -lrt"); }
                println!();
                return Ok(output_file);
            }
        }

        let mut link_cmd = std::process::Command::new("cc");
        if is_embedded {
            link_cmd.args(["-ffreestanding", "-nostdlib", "-nostartfiles", "-o"]).arg(&exe_path).arg(&ll_o_path).arg(&rt_o_path);
            if let Some(ref config) = embedded_config {
                if let Some(ref ls) = config.linker_script {
                    link_cmd.arg(format!("-T{}", ls));
                }
            }
        } else {
            link_cmd.args(["-O2", "-no-pie", "-o"]).arg(&exe_path).arg(&ll_o_path).arg(&rt_o_path);
            link_cmd.args(["-lpthread"]);
            if has_wake {
                link_cmd.arg("-lrt");
            }
        }
        let link_status = link_cmd.status();
        match link_status {
            Ok(status) if status.success() => {
                println!("  Binary: {}", exe_path.display());
            }
            _ => {
                eprintln!("  Warning: linking failed. Link manually:");
                if is_embedded {
                    eprintln!("    cc -ffreestanding -nostdlib -nostartfiles {} {} -o {}", ll_o_path.display(), rt_o_path.display(), exe_path.display());
                } else {
                    eprintln!("    cc -no-pie {} {} -o {} -lpthread", ll_o_path.display(), rt_o_path.display(), exe_path.display());
                }
                if has_wake { eprintln!("    (add -lrt)"); }
            }
        }
    }

    Ok(output_file)
}

fn run_cobol_compile(
    _file_path: &PathBuf,
    _out_dir: Option<&Path>,
    _target: Option<&TargetSpec>,
    _strict: bool,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Err("The COBOL backend has been removed. Use 'brief build <file>' for LLVM compilation instead.".into())
}

/// Intent: run verilog compile.
fn run_verilog_compile(
    _file_path: &PathBuf,
    _hw_config_path: &PathBuf,
    _out_dir: Option<&Path>,
    _no_stdlib: bool,
    _stdlib_path: Option<PathBuf>,
    _generate_tcl: bool,
    _tcl_only: bool,
    _target: Option<&TargetSpec>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Err("The Verilog backend has been removed. Use 'brief build <file>' for LLVM compilation instead.".into())
}

/// Intent: run vhdl compile.
fn run_vhdl_compile(
    _file_path: &PathBuf,
    _hw_config_path: &PathBuf,
    _out_dir: Option<&Path>,
    _target: Option<&TargetSpec>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Err("The VHDL backend has been removed. Use 'brief build <file>' for LLVM compilation instead.".into())
}

/// Intent: run c.
fn run_c(
    _file_path: &PathBuf,
    _out_dir: Option<&Path>,
    _no_stdlib: bool,
    _stdlib_path: Option<PathBuf>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Err("The C backend has been removed. Use 'brief build<file>' for LLVM compilation instead.".into())
}
fn run_cobol(
    _file_path: &PathBuf,
    _out_dir: Option<&Path>,
    _target_spec: Option<TargetSpec>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Err("The COBOL backend has been removed. Use 'brief build<file>' for LLVM compilation instead.".into())
}
fn run_verilog(
    _file_path: &PathBuf,
    _hw_config_path: &PathBuf,
    _out_dir: Option<&Path>,
    _no_stdlib: bool,
    _stdlib_path: Option<PathBuf>,
    _generate_tcl: bool,
    _tcl_only: bool,
    _target_spec: Option<TargetSpec>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Err("The Verilog backend has been removed. Use 'brief build<file>' for LLVM compilation instead.".into())
}
fn run_vhdl(
    _file_path: &PathBuf,
    _hw_config_path: &PathBuf,
    _out_dir: Option<&Path>,
    _target_spec: Option<TargetSpec>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Err("The VHDL backend has been removed. Use 'brief build<file>' for LLVM compilation instead.".into())
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

    let strict = is_strict_extension(file_path);
    let mut parser = parser::Parser::new(&rbv_file.brief_source).with_strict_mode(strict);
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

    program.synthesize_init_txn();

    let mut desug = desugarer::Desugarer::new();
    let mut program = desug.desugar(&program);

    let mut tc = typechecker::TypeChecker::new()
        .with_stdlib_config(no_stdlib, stdlib_path.clone())
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
    // 2026-07-01: Added MAX_TAG_PASSES to prevent infinite loop from
    // circular <Component /> references (e.g., A includes B includes A).
    const MAX_TAG_PASSES: u32 = 100;
    let mut expanded_view = rbv_file.view_html.clone();
    let mut changed = true;
    let mut tag_passes = 0u32;
    while changed {
        tag_passes += 1;
        if tag_passes > MAX_TAG_PASSES {
            return Err(format!(
                "Component tag expansion exceeded {} passes — possible circular <Component /> reference in view HTML",
                MAX_TAG_PASSES
            ).into());
        }
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

    let mut pe = proof_engine::ProofEngine::new().with_strict_mode(strict);
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

    // .srbv verification: check view-state isomorphism
    let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if ext == "srbv" {
        let srbv_errors = view_compiler::verify_srbv(&bindings, &program);
        if !srbv_errors.is_empty() {
            for err in &srbv_errors {
                eprintln!("{}", err);
            }
            return Err("SRBV verification failed: view-state isomorphism broken".into());
        }
        println!("  SRBV verification passed: all view bindings map to verified contracts");
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

    let _analysis = backend::analyze_program(&program, false);

    // Compile (wasm) import modules to .wasm binaries
    let mut wasm_module_binaries: Vec<(String, Vec<u8>)> = Vec::new();
    for item in &program.items {
        if let ast::TopLevel::Import(import) = item {
            if import.target == ast::ImportTarget::Wasm {
                let path_str = import.path.join("/");
                let source_dir = file_path.parent().unwrap_or(std::path::Path::new("."));
                let import_path = source_dir.join(&path_str);
                if import_path.exists() {
                    match compile_wasm32_module(&import_path, &output_path, no_stdlib, stdlib_path.clone()) {
                        Ok(wasm_path) => {
                            match fs::read(&wasm_path) {
                                Ok(binary) => {
                                    let module_name = import.items.first()
                                        .map(|i| i.name.clone())
                                        .unwrap_or_else(|| {
                                            import_path.file_stem()
                                                .and_then(|s| s.to_str())
                                                .unwrap_or("wasm_module")
                                                .to_string()
                                        });
                                    wasm_module_binaries.push((module_name, binary));
                                    println!("    WASM module ready: {}", wasm_path.display());
                                }
                                Err(e) => eprintln!("  Warning: could not read WASM binary: {}", e),
                            }
                        }
                        Err(e) => eprintln!("  Warning: WASM compilation failed for {}: {}", path_str, e),
                    }
                } else {
                    eprintln!("  Warning: (wasm) import file not found: {}", import_path.display());
                }
            }
        }
    }

    let mut wasm_gen = backend::webstack::WebstackGenerator::new();
    for (module_name, binary) in &wasm_module_binaries {
        wasm_gen = wasm_gen.with_wasm_module(module_name.clone(), binary.clone());
    }
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

    // Write TypeScript output
    let ts_code = output.ts_code.clone();
    let ts_path = output_path.join(format!("{}.ts", stem));
    fs::write(&ts_path, &ts_code)?;
    println!("  Generated: {}", ts_path.display());

    if build_wasm {
        // WASM compilation via tsc is handled externally
        println!("  Note: --wasm flag requires compilation via tsc. Use `tsc --outDir dist`");
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

/// Intent: Compile a .cbv (Circuit Brief) file via the CIRCT backend.
fn run_cbv(
    file_path: &PathBuf,
    out_dir: Option<&Path>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let source = fs::read_to_string(file_path)?;
    let clean_source = strip_annotations(&source);

    let mut parser = parser::Parser::new(&clean_source);
    let mut program = parser
        .parse()
        .map_err(|e| format!("Parse error: {}", e))?;

    let mut import_resolver = import_resolver::ImportResolver::new();
    program = import_resolver
        .resolve_imports(&program, file_path)
        .map_err(|e| format!("Import error: {}", e))?;

    program.synthesize_builtin_types();
    program.synthesize_init_txn();

    let mut desug = desugarer::Desugarer::new();
    let mut program = desug.desugar(&program);

    let mut tc = typechecker::TypeChecker::new()
        .with_target(typechecker::CompilationTarget::Circuit);
    let type_errors = tc.check_program(&mut program.clone());
    if !type_errors.is_empty() {
        return Err(format!("Type errors: {}", format_type_errors(&type_errors, file_path.to_str().unwrap_or("main.cbv"))).into());
    }

    let mut circt = backend::circt::CirctBackend::new();
    let mlir_output = circt.generate(&program);

    let stem = file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
    let output_path = out_dir.unwrap_or_else(|| std::path::Path::new("."));
    let mlir_file = output_path.join(format!("{}.mlir", stem));
    fs::write(&mlir_file, &mlir_output)?;
    println!("  MLIR output: {}", mlir_file.display());
    println!("  To synthesize: circt-opt {}.mlir | circt-translate --export-verilog", stem);

    Ok(output_path.to_path_buf())
}

/// Intent: Generate a direct WASM binary from a .bv file.
fn run_wasm(
    file_path: &PathBuf,
    out_dir: Option<&Path>,
    build_wasm: bool,
    no_stdlib: bool,
    stdlib_path: Option<PathBuf>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");

    match ext {
        "bv" | "sbv" => {
            Err("The direct WASM backend has been removed. Use 'brief webstack <file>' for WebAssembly compilation via Webstack, or 'brief build <file>' for native binary.".into())
        }
        "rbv" | "srbv" => {
            println!("Generating WASM + JS + frontend from .{} file...", ext);
            run_rbv(file_path, out_dir, build_wasm, no_stdlib, stdlib_path)
        }
        _ => {
            Err(format!("Unsupported file type: {}. Use .bv, .sbv, .rbv, or .srbv files", ext).into())
        }
    }
}

/// Compile a .bv file to a .wasm module for use with (wasm) import.
/// Runs the standard LLVM pipeline but compiles to wasm32 target.
fn compile_wasm32_module(
    file_path: &PathBuf,
    out_dir: &Path,
    no_stdlib: bool,
    stdlib_path: Option<PathBuf>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let stem = file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("module");
    let out_base = out_dir.to_path_buf();

    println!("  Compiling WASM module: {} ...", file_path.display());

    // Shared pipeline: parse → resolve → typecheck → LLVM IR
    let source = fs::read_to_string(file_path)?;
    let clean_source = strip_annotations(&source);
    let mut parser = parser::Parser::new(&clean_source);
    let mut program = parser.parse().map_err(|e| format!("Parse error: {}", e))?;

    let mut import_resolver = import_resolver::ImportResolver::new()
        .with_use_stdlib(!no_stdlib)
        .with_stdlib_path(stdlib_path);
    let mut program = import_resolver
        .resolve_imports(&program, file_path)
        .map_err(|e| format!("Import error: {}", e))?;

    program.synthesize_builtin_types();
    program.synthesize_init_txn();

    // Validate FFI cross-compilation rules for WASM target (B.5)
    for item in &program.items {
        if let ast::TopLevel::ForeignBinding { signature, target, .. } = item {
            match target {
                ast::ForeignTarget::Js => {
                    eprintln!("  Error: 'frgn from \"javascript\"' not available in WASM-compiled modules");
                    eprintln!("  Offending declaration: frgn {} from \"javascript\"", signature.name);
                    return Err("FFI target 'javascript' not available in WASM modules".into());
                }
                ast::ForeignTarget::C => {}
                _ => {
                    eprintln!("  Error: 'frgn from \"{}\"' not available in WASM-compiled modules", target);
                    eprintln!("  Offending declaration: frgn {}", signature.name);
                    return Err(format!("FFI target '{}' not available in WASM modules", target).into());
                }
            }
        }
    }

    let mut tc = typechecker::TypeChecker::new()
        .with_target(typechecker::CompilationTarget::Interpreter);
    let type_errors = tc.check_program(&mut program.clone());
    if !type_errors.is_empty() {
        return Err(format!("Type errors: {}", format_type_errors(&type_errors, file_path.to_str().unwrap_or("main.bv"))).into());
    }

    let _analysis = backend::analyze_program(&program, false);

    // Skip simplify pass for WASM (budget = 0)

    let mut llvm_backend = crate::backend::llvm::LlvmBackend::new()
        .with_optimize_budget(256)
        .with_embedded_mode(false);
    let output = llvm_backend.generate(&program);

    let ll_path = out_base.join(format!("{}.wasm.ll", stem));
    fs::write(&ll_path, &output)?;

    // Compile .ll → .o via llc (wasm32 target)
    let obj_path = out_base.join(format!("{}.wasm.o", stem));
    let llc_result = std::process::Command::new("llc")
        .args([
            "-filetype=obj", "-O3",
            "--mtriple=wasm32-unknown-unknown",
            "-o",
        ])
        .arg(&obj_path)
        .arg(&ll_path)
        .status();
    match llc_result {
        Ok(status) if status.success() => {}
        _ => {
            eprintln!("  Warning: llc wasm32 compilation failed.");
            eprintln!("  LLVM IR saved at: {}", ll_path.display());
            eprintln!("  Install LLVM with wasm32 support:");
            eprintln!("    llc --version  | grep wasm32");
            return Ok(ll_path);
        }
    }

    // Link .o → .wasm via wasm-ld
    let wasm_path = out_base.join(format!("{}.wasm", stem));
    let link_result = std::process::Command::new("wasm-ld")
        .args([
            "--no-entry", "--export-all",
            "-o",
        ])
        .arg(&wasm_path)
        .arg(&obj_path)
        .status();
    match link_result {
        Ok(status) if status.success() => {
            println!("    WASM module: {}", wasm_path.display());
            Ok(wasm_path)
        }
        _ => {
            eprintln!("  Warning: wasm-ld failed. Install wasm-ld (part of LLVM or wabt).");
            eprintln!("  Object file: {}", obj_path.display());
            eprintln!("  Link manually:");
            eprintln!("    wasm-ld --no-entry --export-all {} -o {}.wasm", obj_path.display(), stem);
            Ok(obj_path)
        }
    }
}

fn run_webstack(
    file_path: &PathBuf,
    out_dir: Option<&Path>,
    build_wasm: bool,
    no_stdlib: bool,
    stdlib_path: Option<PathBuf>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");

    match ext {
        "bv" | "sbv" => {
            println!("Generating Rust/WASM-bindgen from .{} file...", ext);

            let source = fs::read_to_string(file_path)?;
            let clean_source = strip_annotations(&source);

            let mut parser = parser::Parser::new(&clean_source);
            let mut program = parser.parse()
                .map_err(|e| format!("Brief parse error: {}", e))?;

            let mut import_resolver = import_resolver::ImportResolver::new();
            program = import_resolver.resolve_imports(&program, file_path)
                .map_err(|e| format!("Import error: {}", e))?;

            let mut desug = desugarer::Desugarer::new();
            program = desug.desugar(&program);

            let mut tc = typechecker::TypeChecker::new()
        .with_stdlib_config(no_stdlib, stdlib_path.clone())
                .with_target(typechecker::CompilationTarget::Wasm);
            let type_errors = tc.check_program(&mut program);
            if !type_errors.is_empty() {
                return Err(format!("Type errors: {}", format_type_errors(&type_errors, file_path.to_str().unwrap_or("main.bv"))).into());
            }

            let _analysis = backend::analyze_program(&program, false);

            let stem = file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("output");

            let mut webstack_gen = backend::webstack::WebstackGenerator::new()
                .with_target(backend::webstack::CodeTarget::Wasm);
            let output = webstack_gen.generate(&program, &[], stem);

            let output_path = out_dir.map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."));
            fs::create_dir_all(&output_path)?;

            let rs_path = output_path.join(format!("{}.ts", stem));
            fs::write(&rs_path, &output.ts_code)?;
            println!("  Generated TS: {}", rs_path.display());

            let js_path = output_path.join(format!("{}.js", stem));
            fs::write(&js_path, &output.js_glue)?;
            println!("  Generated JS: {}", js_path.display());

            if build_wasm {
                // WASM compilation via tsc is handled externally
                println!("  Note: --wasm flag requires compilation via tsc. Use `tsc --outDir dist`");
            }

            Ok(output_path)
        }
        "rbv" | "srbv" => {
            println!("Full webstack (Rust + JS + HTML) from .{} file...", ext);
            run_rbv(file_path, out_dir, build_wasm, no_stdlib, stdlib_path)
        }
        _ => {
            Err(format!("Unsupported file type: {}. Use .bv, .sbv, .rbv, or .srbv files", ext).into())
        }
    }
}

/// Intent: generate html.
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

/// Intent: check dependency.
fn check_dependency(dep: &dbrief::ast::DbriefDependency) -> Result<(), String> {
    if dep.name.to_lowercase().contains("missing") {
        return Err("not found on system".to_string());
    }
    Ok(())
}

/// Intent: install dependency.
fn install_dependency(dep: &dbrief::ast::DbriefDependency, verbose: bool) -> Result<(), String> {
    if verbose {
        let version_info = dep.version_constraint.as_ref()
            .map(|v| format!(" version {}", v))
            .unwrap_or_default();
        println!("    Would install: {}{}", dep.name, version_info);
        
        if !dep.platform.is_empty() {
            println!("    Platform: {}", dep.platform.join(", "));
        }
        
        if !dep.features.is_empty() {
            println!("    Features: {}", dep.features.join(", "));
        }
    }
    Ok(())
}

/// Intent: run `brief export` — compile a bridge .bv to .a and generate
/// native language wrappers via the language's $! adapter macro.
fn run_export_main(
    file_path: &Path,
    language: &str,
    out_dir: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let source = fs::read_to_string(file_path)?;
    let clean_source = strip_annotations(&source);

    let mut parser = parser::Parser::new(&clean_source);
    let mut program = parser.parse()
        .map_err(|e| format!("Parse error: {}", e))?;

    let mut import_resolver = import_resolver::ImportResolver::new()
        .with_use_stdlib(false);
    let file_path_buf = file_path.to_path_buf();
    let mut program = import_resolver.resolve_imports(&program, &file_path_buf)
        .map_err(|e| format!("Import error: {}", e))?;

    let bridge_name = file_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("bridge")
        .to_string();

    let output = out_dir.unwrap_or_else(|| Path::new("."));
    let dbvl_path = Path::new("lib/glue.dbvl");

    use brief_compiler::glue::export::run_export as run_glue_export;
    run_glue_export(&program, &bridge_name, language, output, dbvl_path)
        .map_err(|e| format!("Export failed: {}", e).into())
}

/// Intent: run `brief link` — analyze a foreign library, generate a .bv
/// with frgn declarations cross-referenced against compiler intrinsics.
fn run_link_main(
    lib_path: &Path,
    out_dir: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    use brief_compiler::glue::link::{analyze_library, generate_bridge_bv, print_link_summary};

    let result = analyze_library(lib_path)?;
    print_link_summary(&result);

    let output = generate_bridge_bv(&result);
    let stem = lib_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("library");
    let out_path = out_dir
        .map(|d| d.join(format!("{}-bridge.bv", stem)))
        .unwrap_or_else(|| PathBuf::from(format!("{}-bridge.bv", stem)));

    fs::write(&out_path, &output)?;
    println!("\n  Bridge generated: {}", out_path.display());
    Ok(())
}

/// Intent: main.
fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage(&args[0]);
        return;
    }

    let command = &args[1];

    let verbose = args.contains(&"-v".to_string()) || args.contains(&"--verbose".to_string());
    let explain = args.contains(&"--explain".to_string());
    let no_stdlib = args.contains(&"--no-stdlib".to_string()) || args.contains(&"--no-std".to_string());
    let emit_memory_spec = args.contains(&"--emit-memory-spec".to_string());
    let memory_spec_format = if args.contains(&"--memory-spec-toml".to_string()) {
        "toml"
    } else {
        "json"
    };
    let stdlib_path = args
        .iter()
        .position(|a| a == "--stdlib-path")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);

    let strict_flag = args.contains(&"--strict".to_string());
    let optimize_flag = args.contains(&"--optimize".to_string()) || args.contains(&"-O".to_string());

    match command.as_str() {
        "check" | "ck" => {
            let annotate =
                args.contains(&"-a".to_string()) || args.contains(&"--annotate".to_string());

            let file_path = args
                .iter()
                .skip(2)
                .find(|a| {
                    a.ends_with(".bv") || a.ends_with(".sbv") || a.ends_with(".ebv") || a.ends_with(".cbv")
                        || a.ends_with(".sebv") || a.ends_with(".rbv") || a.ends_with(".srbv")
                })
                .map(PathBuf::from);

            if let Some(path) = file_path {
                let codicil_mode = detect_codicil_project(&path);
                let strict = strict_flag || is_strict_extension(&path);
                let safe_compile = args.contains(&"--safe-compile".to_string());
                let mut macro_budget = args.iter()
                    .position(|a| a == "--macro-budget")
                    .and_then(|i| args.get(i + 1))
                    .and_then(|s| s.parse::<u64>().ok());
                if args.contains(&"--unlimited-macros".to_string()) {
                    macro_budget = Some(u64::MAX);
                }
                if let Err(_e) = run_check(
                    &path,
                    verbose,
                    annotate,
                    no_stdlib,
                    stdlib_path,
                    codicil_mode,
                    strict,
                    optimize_flag,
                    safe_compile,
                    macro_budget,
                ) {
                    std::process::exit(1);
                }
            } else {
                eprintln!("Error: No .bv, .abv, .sbv, .rbv, .srbv, .ebv, or .sebv file specified");
                eprintln!("Usage: {} check <file>", args[0]);
                std::process::exit(1);
            }
        }

        "fuzz" | "fz" => {
            let file_path = args
                .iter()
                .skip(2)
                .find(|a| {
                    a.ends_with(".bv") || a.ends_with(".sbv") || a.ends_with(".ebv") || a.ends_with(".cbv")
                        || a.ends_with(".sebv") || a.ends_with(".rbv") || a.ends_with(".srbv")
                })
                .map(PathBuf::from);

            if let Some(path) = file_path {
                let codicil_mode = detect_codicil_project(&path);
                let strict = strict_flag || is_strict_extension(&path);
                let safe_compile = args.contains(&"--safe-compile".to_string());
                if let Err(_e) = run_fuzz(
                    &path,
                    verbose,
                    no_stdlib,
                    stdlib_path.clone(),
                    codicil_mode,
                    strict,
                    safe_compile,
                ) {
                    std::process::exit(1);
                }
            } else {
                eprintln!("Error: No .bv file specified for fuzz");
                eprintln!("Usage: {} fuzz <file>", args[0]);
                std::process::exit(1);
            }
        }

        "compile" => {
            run_compile_unified(&args, strict_flag, optimize_flag);
        }

        "build" | "b" => {
            let mut file_path = None;
            let mut out_dir = None;
            let mut prod_mode = false;
            let mut simplify_budget: Option<u64> = None;
            let mut gpu_offload = false;
            let mut gpu_backend = "vulkan".to_string();
            let mut emit_llvm = false;

            let mut i = 2;
            while i < args.len() {
                let arg = &args[i];
                if arg == "--out" && i + 1 < args.len() {
                    out_dir = Some(PathBuf::from(&args[i + 1]));
                    i += 2;
                } else if arg == "--llvm" {
                    emit_llvm = true;
                    i += 1;
                } else if arg == "--prod" || arg == "--release" {
                    prod_mode = true;
                    i += 1;
                } else if arg == "--dev" {
                    prod_mode = false;
                    i += 1;
                } else if arg == "--no-simplify" {
                    simplify_budget = Some(0);
                    i += 1;
                } else if arg == "--simplify-budget" && i + 1 < args.len() {
                    simplify_budget = Some(args[i + 1].parse::<u64>().unwrap_or(0));
                    i += 2;
                } else if arg == "--gpu-offload" {
                    gpu_offload = true;
                    i += 1;
                } else if arg == "--gpu-backend" && i + 1 < args.len() {
                    gpu_backend = args[i + 1].clone();
                    i += 2;
                } else if arg.ends_with(".bv") || arg.ends_with(".rbv") || arg.ends_with(".ebv") || arg.ends_with(".cbv")
                    || arg.ends_with(".sbv") || arg.ends_with(".srbv") || arg.ends_with(".sebv")
                    || arg.ends_with(".abv") || arg.ends_with(".dbv") || arg.ends_with(".dbvs") || arg.ends_with(".dbvl") {
                    file_path = Some(PathBuf::from(arg));
                    i += 1;
                } else {
                    i += 1;
                }
            }

            if let Some(path) = file_path {
                let out = out_dir.as_deref();
                let strict = strict_flag || is_strict_extension(&path);

                // --llvm flag: emit LLVM IR instead of building a binary (for LLVM-emitting variants)
                if emit_llvm {
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    match ext {
                        "bv" | "sbv" | "ebv" | "sebv" | "abv" => {
                            let result = run_llvm_compile(&path, out, None, strict, 256, false, None, true, None, false, false, false, prod_mode, simplify_budget, no_stdlib, stdlib_path.clone(), false, None, false, gpu_offload || ext == "abv", &gpu_backend, None, None, 10_000);
                            match result {
                                Ok(ll_path) => {
                                    println!("  LLVM IR emitted: {}", ll_path.display());
                                    std::process::exit(0);
                                }
                                Err(e) => {
                                    eprintln!("{}", e);
                                    std::process::exit(1);
                                }
                            }
                        }
                        _ => {
                            eprintln!("Error: --llvm flag is only valid for .bv, .sbv, .ebv, .sebv, and .abv files");
                            eprintln!("  '{}' does not compile via LLVM", ext);
                            std::process::exit(1);
                        }
                    }
                }

                match run_build(&path, verbose, no_stdlib, stdlib_path, out, emit_memory_spec, memory_spec_format, strict, optimize_flag, prod_mode, simplify_budget, gpu_offload, &gpu_backend) {
                    Ok(output) => {
                        println!("Build complete: {}", output.display());
                    }
                    Err(e) => {
                        eprintln!("Build failed: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                eprintln!("Error: No .bv, .abv, .sbv, .rbv, .srbv, .ebv, .sebv, .cbv, .dbv, .dbvs, or .dbvl file specified");
                eprintln!("Usage: {} build <file> [--out <dir>] [--dev] [--prod|--release] [--llvm] [--simplify-budget <N>] [--no-simplify]", args[0]);
                eprintln!("  .bv/.sbv files        → compile via LLVM to native binary");
                eprintln!("  .abv file             → compile via LLVM + SPIR-V (GPU) to native binary + .spv");
                eprintln!("  .rbv/.srbv files      → TypeScript + frontend");
                eprintln!("  .ebv/.sebv files      → microcontroller binary via LLVM");
                eprintln!("  .cbv file             → CIRCT hardware description (Verilog/VHDL)");
                eprintln!("  .dbv/.dbvs/.dbvl files → validate and emit JSON");
                eprintln!("  --llvm                → emit LLVM IR instead of binary (.bv/.sbv/.ebv/.sebv/.abv only)");
            }
        }

        "rust" => {
            eprintln!("Error: 'brief rust' has been removed.");
            eprintln!("  Use 'brief build <file>' for LLVM compilation instead.");
            std::process::exit(1);
        }

        "llvm" => {
            let mut file_path = None;
            let mut out_dir = None;
            let mut target = None;

            let mut i = 2;
            let mut optimize_budget: Option<u64> = None;
            let mut optimize_report = false;
            let mut optimize_size: Option<u64> = None;
            let mut dead_info_disabled = false;
            let mut dump_layout = false;
            let mut hw_handoff: Option<String> = None;
            let mut hw_target: Option<String> = None;
            let mut target_dbv: Option<String> = None;
            let mut board_name: Option<String> = None;
            let mut pgo_generate = false;
            let mut explain = false;
            let mut prod_mode = false;
            let mut simplify_budget: Option<u64> = None;
            let mut emit_remarks = false;
            let mut gpu_offload = false;
            let mut gpu_backend = "vulkan".to_string();
            let mut txn_convergence_max_iterations: Option<u64> = None;
            while i < args.len() {
                let arg = &args[i];
                if arg == "--out" && i + 1 < args.len() {
                    out_dir = Some(PathBuf::from(&args[i + 1]));
                    i += 2;
                } else if arg == "--target" && i + 1 < args.len() {
                    target = Some(args[i + 1].as_str());
                    i += 2;
                } else if arg == "--hw-handoff" && i + 1 < args.len() {
                    hw_handoff = Some(args[i + 1].clone());
                    i += 2;
                } else if arg == "--hw-target" && i + 1 < args.len() {
                    hw_target = Some(args[i + 1].clone());
                    i += 2;
                } else if arg == "--target-dbv" && i + 1 < args.len() {
                    target_dbv = Some(args[i + 1].clone());
                    i += 2;
                } else if arg == "--board" && i + 1 < args.len() {
                    board_name = Some(args[i + 1].clone());
                    i += 2;
                } else if arg == "--pgo-generate" {
                    pgo_generate = true;
                    i += 1;
                } else if arg == "--optimize-budget" && i + 1 < args.len() {
                    optimize_budget = Some(args[i + 1].parse::<u64>().unwrap_or(256));
                    i += 2;
                } else if arg == "--layout" {
                    dump_layout = true;
                    i += 1;
                } else if arg == "--optimize-report" {
                    optimize_report = true;
                    i += 1;
                } else if arg == "--optimize-size" && i + 1 < args.len() {
                    optimize_size = Some(args[i + 1].parse::<u64>().unwrap_or(0));
                    i += 2;
                } else if arg == "--no-dead-info" {
                    dead_info_disabled = true;
                    i += 1;
                } else if arg == "--explain" {
                    explain = true;
                    i += 1;
                } else if arg == "--remarks" {
                    emit_remarks = true;
                    i += 1;
                } else if arg == "--gpu-offload" {
                    gpu_offload = true;
                    i += 1;
                } else if arg == "--gpu-backend" && i + 1 < args.len() {
                    gpu_backend = args[i + 1].clone();
                    i += 2;
                } else if arg == "--prod" || arg == "--release" {
                    prod_mode = true;
                    i += 1;
                } else if arg == "--dev" {
                    prod_mode = false;
                    simplify_budget = Some(0);
                    i += 1;
                } else if arg == "--no-simplify" {
                    simplify_budget = Some(0);
                    i += 1;
                } else if arg == "--simplify-budget" && i + 1 < args.len() {
                    simplify_budget = Some(args[i + 1].parse::<u64>().unwrap_or(0));
                    i += 2;
                } else if arg == "--txn-convergence-max-iterations" && i + 1 < args.len() {
                    txn_convergence_max_iterations = Some(args[i + 1].parse::<u64>().unwrap_or(10_000));
                    i += 2;
                } else if !arg.starts_with('-') {
                    file_path = Some(PathBuf::from(arg));
                    i += 1;
                } else {
                    i += 1;
                }
            }

            // Process hardware handoff before compilation
            if let Some(ref handoff_path) = hw_handoff {
                process_hardware_handoff(handoff_path, hw_target.as_deref(), out_dir.as_deref())
                    .unwrap_or_else(|e| { eprintln!("Warning: handoff processing failed: {}", e); });
            }

            // If target DBV is provided, parse it for address extraction
            let mut mmio_addresses: Option<HashMap<String, u64>> = None;
            if let Some(ref dbv_path) = target_dbv {
                match process_target_dbv(dbv_path) {
                    Ok(map) => { mmio_addresses = Some(map); }
                    Err(e) => { eprintln!("Warning: could not process target DBV: {}", e); }
                }
            }

            if let Some(path) = file_path {
                let strict = strict_flag || is_strict_extension(&path);
                let safe_compile = args.contains(&"--safe-compile".to_string());
                let mut macro_budget: Option<u64> = args.iter()
                    .position(|a| a == "--macro-budget")
                    .and_then(|i| args.get(i + 1))
                    .and_then(|s| s.parse().ok());
                if args.contains(&"--unlimited-macros".to_string()) {
                    macro_budget = Some(u64::MAX);
                }
                let txn_max_iter = txn_convergence_max_iterations.unwrap_or(10_000);
                let result = run_llvm_compile(&path, out_dir.as_deref(), None, strict,
                    optimize_budget.unwrap_or(256), optimize_report, optimize_size, dead_info_disabled, mmio_addresses, pgo_generate, explain, dump_layout, prod_mode, simplify_budget, no_stdlib, stdlib_path.clone(), safe_compile, macro_budget, emit_remarks, gpu_offload, &gpu_backend, None, None, txn_max_iter);
                if let Err(e) = result {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            } else {
                eprintln!("Error: No .bv, .abv, .sbv, .ebv, or .sebv file specified");
                eprintln!("Usage: {} llvm <file.bv|file.abv> [--out <dir>] [--hw-handoff <system.xsa|xparameters.h>] [--hw-target <board>] [--target-dbv <target.dbv>] [--optimize-budget <N>] [--optimize-report] [--optimize-size <bytes>] [--no-dead-info] [--dev] [--prod|--release] [--simplify-budget <N>] [--no-simplify]", args[0]);
                std::process::exit(1);
            }
        }

        "c" | "cc" => {
            eprintln!("Error: 'brief c' has been removed.");
            eprintln!("  Use 'brief build <file>' for LLVM compilation instead.");
            std::process::exit(1);
        }

        "cobol" | "cbl" => {
            eprintln!("Error: 'brief cobol' has been removed.");
            eprintln!("  Use 'brief build <file>' for LLVM compilation instead.");
            std::process::exit(1);
        }

        "arm" | "a" => {
            eprintln!("Error: 'brief arm' has been removed.");
            eprintln!("  Use 'brief build <file>' for LLVM compilation instead.");
            std::process::exit(1);
        }

        "watch" | "w" => {
            let verbose =
                args.contains(&"-v".to_string()) || args.contains(&"--verbose".to_string());

            let file_path = args
                .iter()
                .skip(2)
                .find(|a| a.ends_with(".bv") || a.ends_with(".sbv"))
                .map(PathBuf::from);

            if let Some(path) = file_path {
                if let Err(e) = run_watch(path, verbose, no_stdlib, stdlib_path.clone(), optimize_flag) {
                    eprintln!("Watch error: {}", e);
                    std::process::exit(1);
                }
            } else {
                eprintln!("Error: No .bv or .sbv file specified");
                eprintln!("Usage: {} watch <file.bv|file.sbv>", args[0]);
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
            let mut generate_tcl = false;
            let mut tcl_only = false;

            let mut i = 2;
            while i < args.len() {
                let arg = &args[i];
                if arg == "--out" && i + 1 < args.len() {
                    out_dir = Some(PathBuf::from(&args[i + 1]));
                    i += 2;
                } else if arg == "--hw" && i + 1 < args.len() {
                    hw_config = Some(PathBuf::from(&args[i + 1]));
                    i += 2;
                } else if arg == "--tcl" {
                    generate_tcl = true;
                    i += 1;
                } else if arg == "--tcl-only" {
                    tcl_only = true;
                    i += 1;
                } else if arg.ends_with(".ebv") || arg.ends_with(".sebv") || arg.ends_with(".cbv") {
                    file_path = Some(PathBuf::from(arg));
                    i += 1;
                } else if arg.ends_with(".bv") || arg.ends_with(".sbv") {
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
                if ext == "ebv" || ext == "sebv" || ext == "cbv" {
                    if let Some(hw) = hw_config {
                        if let Err(e) = run_verilog(
                            &path,
                            &hw,
                            out_dir.as_deref(),
                            no_stdlib,
                            stdlib_path.clone(),
                            generate_tcl,
                            tcl_only,
                            None,
                        ) {
                            eprintln!("Error: {}", e);
                            std::process::exit(1);
                        }
                    } else {
                        eprintln!("Error: .ebv/.sebv files require --hw <hardware.toml|config.dbv>");
                        eprintln!(
                            "Usage: {} verilog <file.ebv|file.sebv> --hw <hardware.toml|config.dbv> [--out <dir>] [--tcl] [--tcl-only]",
                            args[0]
                        );
                        std::process::exit(1);
                    }
                } else if ext == "bv" || ext == "sbv" {
                    let hw_path = hw_config.unwrap_or_else(|| PathBuf::from("/dev/null"));
                    if let Err(e) = run_verilog(
                        &path,
                        &hw_path,
                        out_dir.as_deref(),
                        no_stdlib,
                        stdlib_path.clone(),
                        generate_tcl,
                        tcl_only,
                        None,
                    ) {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                eprintln!("Error: Missing .bv, .sbv, .ebv, or .sebv file");
                eprintln!(
                    "Usage: {} verilog <file.bv|file.sbv|file.ebv|file.sebv> [--hw <hardware.toml|config.dbv>] [--out <dir>] [--tcl] [--tcl-only]",
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
                } else if arg.ends_with(".rbv") || arg.ends_with(".srbv") {
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
                eprintln!("Error: No .rbv or .srbv file specified");
                eprintln!(
                    "Usage: {} rbv <file.rbv|file.srbv> [--out <dir>] [--no-build]",
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
                } else if arg.ends_with(".rbv") || arg.ends_with(".srbv") {
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
                eprintln!("Error: No .rbv or .srbv file specified");
                eprintln!(
                    "Usage: {} run <file.rbv|file.srbv> [--port <port>] [--no-open]",
                    args[0]
                );
                std::process::exit(1);
            }
        }

        "wasm" => {
            eprintln!("Error: 'brief wasm' has been removed.");
            eprintln!("  Use 'brief webstack <file>' for WASM compilation instead.");
            std::process::exit(1);
        }

        "webstack" => {
            let mut file_path = None;
            let mut out_dir = None;
            let mut build_wasm = true;

            let mut i = 2;
            while i < args.len() {
                let arg = &args[i];
                if arg == "--out" && i + 1 < args.len() {
                    out_dir = Some(PathBuf::from(&args[i + 1]));
                    i += 2;
                } else if arg == "--no-build" {
                    build_wasm = false;
                    i += 1;
                } else if arg.ends_with(".bv") || arg.ends_with(".sbv") || arg.ends_with(".rbv") || arg.ends_with(".srbv") {
                    file_path = Some(PathBuf::from(arg));
                    i += 1;
                } else {
                    i += 1;
                }
            }

            if let Some(path) = file_path {
                match run_webstack(&path, out_dir.as_deref(), build_wasm, no_stdlib, stdlib_path.clone()) {
                    Ok(output) => {
                        println!("Webstack generated: {}", output.display());
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                eprintln!("Error: No .bv, .sbv, .rbv, or .srbv file specified");
                eprintln!("Usage: {} webstack <file> [--out <dir>] [--no-build]", args[0]);
                eprintln!("  Generates Rust source + wasm-bindgen glue, compiles via wasm-pack");
                std::process::exit(1);
            }
        }

        "selfhost" => {
            let file_path = args.iter().skip(2).find(|a| {
                a.ends_with(".bv") || a.ends_with(".sbv") || a.ends_with(".ebv") || a.ends_with(".cbv")
                    || a.ends_with(".rbv") || a.ends_with(".srbv")
            }).map(|s| s.to_string());

            let backend = args.iter().position(|a| a == "--backend")
                .and_then(|i| args.get(i + 1))
                .cloned()
                .unwrap_or("rust".to_string());

            if let Some(path) = file_path {
                if let Err(e) = run_selfhost(&path, &backend, verbose) {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            } else {
                eprintln!("Error: No .bv, .sbv, or .rbv file specified");
                eprintln!("Usage: {} selfhost <file> [--backend c|rust]", args[0]);
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

        "export" => {
            let mut file_path = None;
            let mut language = None;
            let mut out_dir = None;

            let mut i = 2;
            while i < args.len() {
                let arg = &args[i];
                if arg == "--out" && i + 1 < args.len() {
                    out_dir = Some(PathBuf::from(&args[i + 1]));
                    i += 2;
                } else if arg.ends_with(".bv") || arg.ends_with(".sbv") {
                    file_path = Some(PathBuf::from(arg));
                    i += 1;
                } else if !arg.starts_with('-') {
                    language = Some(arg.clone());
                    i += 1;
                } else {
                    i += 1;
                }
            }

            if let (Some(path), Some(lang)) = (file_path, language) {
                if let Err(e) = run_export_main(&path, &lang, out_dir.as_deref()) {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            } else {
                eprintln!("Error: Usage: {} export <bridge.bv> <language> [--out <dir>]", args[0]);
                eprintln!("  Analyzes a Brief bridge and generates native language wrappers");
                std::process::exit(1);
            }
        }

        "link" => {
            let mut lib_path = None;
            let mut out_dir = None;

            let mut i = 2;
            while i < args.len() {
                let arg = &args[i];
                if arg == "--out" && i + 1 < args.len() {
                    out_dir = Some(PathBuf::from(&args[i + 1]));
                    i += 2;
                } else if !arg.starts_with('-') {
                    lib_path = Some(PathBuf::from(arg));
                    i += 1;
                } else {
                    i += 1;
                }
            }

            if let Some(path) = lib_path {
                if let Err(e) = run_link_main(&path, out_dir.as_deref()) {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            } else {
                eprintln!("Error: Usage: {} link <library_path> [--out <dir>]", args[0]);
                eprintln!("  Analyzes a foreign library and generates a bridge .bv file");
                std::process::exit(1);
            }
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

        "dbv" | "dbvs" | "dbvl" => {
            let mut file_path = None;
            let mut out_file = None;
            let mut pretty = false;

            let mut i = 2;
            while i < args.len() {
                let arg = &args[i];
                if arg == "--out" && i + 1 < args.len() {
                    out_file = Some(PathBuf::from(&args[i + 1]));
                    i += 2;
                } else if arg == "--pretty" {
                    pretty = true;
                    i += 1;
                } else if arg.ends_with(".dbv") || arg.ends_with(".dbvs") || arg.ends_with(".dbvl") {
                    file_path = Some(PathBuf::from(arg));
                    i += 1;
                } else {
                    i += 1;
                }
            }

            if let Some(path) = file_path {
                match fs::read_to_string(&path) {
                    Ok(source) => {
                        match dbrief::v2::parse_document(&source) {
                            Ok(doc) => {
                                let json = if pretty {
                                    serde_json::to_string_pretty(&doc)
                                        .map_err(|e| format!("JSON error: {}", e))
                                } else {
                                    serde_json::to_string(&doc)
                                        .map_err(|e| format!("JSON error: {}", e))
                                };
                                match json {
                                    Ok(output) => {
                                        if let Some(out) = out_file {
                                            if let Err(e) = fs::write(&out, &output) {
                                                eprintln!("Error writing output: {}", e);
                                                std::process::exit(1);
                                            }
                                            println!("  JSON written: {}", out.display());
                                        } else {
                                            println!("{}", output);
                                        }
                                        println!("  Parsed {} schemas, {} data groups, {} rules from {}",
                                            doc.schemas.len(), doc.data_groups.len(),
                                            doc.rules.len(), path.display());
                                    }
                                    Err(e) => {
                                        eprintln!("{}", e);
                                        std::process::exit(1);
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Parse error: {}", e);
                                std::process::exit(1);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error reading file: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                eprintln!("Error: No .dbv/.dbvs/.dbvl file specified");
                eprintln!("Usage: {} <dbv|dbvs|dbvl> <file> [--out <file.json>] [--pretty]", args[0]);
                std::process::exit(1);
            }
        }

        "deps" => {
            let mut action = "check";
            let mut file_path = None;
            let mut verbose = false;

            let mut i = 2;
            while i < args.len() {
                let arg = &args[i];
                if arg == "check" || arg == "install" || arg == "list" {
                    action = arg;
                    i += 1;
                } else if arg == "--verbose" || arg == "-v" {
                    verbose = true;
                    i += 1;
                } else if arg.ends_with(".dbvs") || arg.ends_with(".dbv") {
                    file_path = Some(PathBuf::from(arg));
                    i += 1;
                } else {
                    i += 1;
                }
            }

            if let Some(path) = file_path {
                match fs::read_to_string(&path) {
                    Ok(source) => {
                        let program = if path.extension().map_or(false, |e| e == "dbvs") {
                            match dbrief::parse_dbvs(&source) {
                                Ok(p) => dbrief::ast::DbriefProgram {
                                    registers: p.registers,
                                    services: p.services,
                                    structs: p.structs,
                                    enums: p.enums,
                                    aliases: p.aliases,
                                    rules: vec![],
                                    records: vec![],
                                    checks: vec![],
                                    depends: p.depends,
                                    imports: vec![],
                                },
                                Err(e) => {
                                    eprintln!("Parse error: {}", e);
                                    std::process::exit(1);
                                }
                            }
                        } else {
                            match dbrief::parse_dbrief(&source) {
                                Ok(p) => p,
                                Err(e) => {
                                    eprintln!("Parse error: {}", e);
                                    std::process::exit(1);
                                }
                            }
                        };

                        if program.depends.is_empty() {
                            println!("No dependencies declared in {}", path.display());
                        } else {
                            match action {
                                "list" => {
                                    println!("Dependencies in {}:", path.display());
                                    for dep in &program.depends {
                                        let ver = dep.version_constraint.as_ref()
                                            .map(|v| format!(" ({})", v))
                                            .unwrap_or_default();
                                        println!("  - {}{}", dep.name, ver);
                                    }
                                }
                                "check" => {
                                    println!("Checking dependencies for {}:", path.display());
                                    let mut all_ok = true;
                                    for dep in &program.depends {
                                        let status = check_dependency(dep);
                                        let ver = dep.version_constraint.as_ref()
                                            .map(|v| format!("({})", v))
                                            .unwrap_or_default();
                                        if status.is_ok() {
                                            println!("  ✓ {} {}", dep.name, ver);
                                        } else {
                                            println!("  ✗ {} {} - {}", dep.name, ver, status.unwrap_err());
                                            all_ok = false;
                                        }
                                    }
                                    if !all_ok {
                                        eprintln!("\nSome dependencies are missing. Run 'brief deps install' to install them.");
                                        std::process::exit(1);
                                    }
                                }
                                "install" => {
                                    println!("Installing dependencies for {}:", path.display());
                                    for dep in &program.depends {
                                        match install_dependency(dep, verbose) {
                                            Ok(_) => println!("  ✓ Installed {}", dep.name),
                                            Err(e) => {
                                                println!("  ✗ Failed to install {}: {}", dep.name, e);
                                                if verbose {
                                                    eprintln!("    Details: {}", e);
                                                }
                                            }
                                        }
                                    }
                                    println!("\nDependency installation complete.");
                                }
                                _ => {
                                    eprintln!("Unknown action: {}", action);
                                    std::process::exit(1);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error reading file: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                eprintln!("Error: No .dbvs or .dbv file specified");
                eprintln!("Usage: {} deps [check|install|list] <file.dbvs|file.dbv> [-v]", args[0]);
                std::process::exit(1);
            }
        }

        "deps" => {
            let mut action = "check";
            let mut file_path = None;
            let mut verbose = false;

            let mut i = 2;
            while i < args.len() {
                let arg = &args[i];
                if arg == "check" || arg == "install" || arg == "list" {
                    action = arg;
                    i += 1;
                } else if arg == "--verbose" || arg == "-v" {
                    verbose = true;
                    i += 1;
                } else if arg.ends_with(".dbvs") || arg.ends_with(".dbv") {
                    file_path = Some(PathBuf::from(arg));
                    i += 1;
                } else {
                    i += 1;
                }
            }

            if let Some(path) = file_path {
                match fs::read_to_string(&path) {
                    Ok(source) => {
                        let program = if path.extension().map_or(false, |e| e == "dbvs") {
                            match dbrief::parse_dbvs(&source) {
                                Ok(p) => dbrief::ast::DbriefProgram {
                                    registers: p.registers,
                                    services: p.services,
                                    structs: p.structs,
                                    enums: p.enums,
                                    aliases: p.aliases,
                                    rules: vec![],
                                    records: vec![],
                                    checks: vec![],
                                    depends: p.depends,
                                    imports: vec![],
                                },
                                Err(e) => {
                                    eprintln!("Parse error: {}", e);
                                    std::process::exit(1);
                                }
                            }
                        } else {
                            match dbrief::parse_dbrief(&source) {
                                Ok(p) => p,
                                Err(e) => {
                                    eprintln!("Parse error: {}", e);
                                    std::process::exit(1);
                                }
                            }
                        };

                        if program.depends.is_empty() {
                            println!("No dependencies declared in {}", path.display());
                        } else {
                            match action {
                                "list" => {
                                    println!("Dependencies in {}:", path.display());
                                    for dep in &program.depends {
                                        let ver = dep.version_constraint.as_ref()
                                            .map(|v| format!(" ({})", v))
                                            .unwrap_or_default();
                                        println!("  - {}{}", dep.name, ver);
                                    }
                                }
                                "check" => {
                                    println!("Checking dependencies for {}:", path.display());
                                    let mut all_ok = true;
                                    for dep in &program.depends {
                                        let status = check_dependency(dep);
                                        let ver = dep.version_constraint.as_ref()
                                            .map(|v| format!("({})", v))
                                            .unwrap_or_default();
                                        if status.is_ok() {
                                            println!("  ✓ {} {}", dep.name, ver);
                                        } else {
                                            println!("  ✗ {} {} - {}", dep.name, ver, status.unwrap_err());
                                            all_ok = false;
                                        }
                                    }
                                    if !all_ok {
                                        eprintln!("\nSome dependencies are missing. Run 'brief deps install' to install them.");
                                        std::process::exit(1);
                                    }
                                }
                                "install" => {
                                    println!("Installing dependencies for {}:", path.display());
                                    for dep in &program.depends {
                                        match install_dependency(dep, verbose) {
                                            Ok(_) => println!("  ✓ Installed {}", dep.name),
                                            Err(e) => {
                                                println!("  ✗ Failed to install {}: {}", dep.name, e);
                                                if verbose {
                                                    eprintln!("    Details: {}", e);
                                                }
                                            }
                                        }
                                    }
                                    println!("\nDependency installation complete.");
                                }
                                _ => {
                                    eprintln!("Unknown action: {}", action);
                                    std::process::exit(1);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error reading file: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                eprintln!("Error: No .dbvs or .dbv file specified");
                eprintln!("Usage: {} deps [check|install|list] <file.dbvs|file.dbv> [-v]", args[0]);
                std::process::exit(1);
            }
        }

        "metropipe" => {
            if args.len() >= 3 && args[2] == "connect" {
                if let Err(e) = brief_compiler::ffi::metro_cli::run_metro_cli(&args) {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            } else {
                eprintln!("Usage: {} metropipe connect <service_name> [--send <data>] [--gen-stub] [--out <dir>]", args[0]);
                eprintln!("  Connects to a metropipe shared memory service");
                eprintln!("  Default mode: interactive REPL");
                std::process::exit(1);
            }
        }

        "bind" => {
            let mut lib_path = None;
            let mut output_dir = None;
            let mut mapper = None;
            let mut force = false;
            let mut gen_stubs = false;

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
                } else if arg == "--gen-stubs" {
                    gen_stubs = true;
                    i += 1;
                } else if !arg.starts_with('-') {
                    lib_path = Some(PathBuf::from(arg));
                    i += 1;
                } else {
                    i += 1;
                }
            }

            if let Some(path) = lib_path {
                match run_bind(&path, mapper.as_deref(), output_dir.as_deref(), force, gen_stubs) {
                    Ok(_) => {
                        println!("  Binding complete");
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                eprintln!("Error: No library path specified");
                eprintln!("Usage: {} bind <library_path> [--mapper <name>] [--out <dir>] [--force] [--gen-stubs]", args[0]);
                eprintln!("  Analyzes a foreign library and generates ready-to-use Brief FFI bindings");
                std::process::exit(1);
            }
        }

        "-h" | "--help" | "help" => {
            print_usage(&args[0]);
        }

        _ => {
            if command.ends_with(".bv") || command.ends_with(".sbv") {
                let path = PathBuf::from(command);
                let codicil_mode = detect_codicil_project(&path);
                let strict = is_strict_extension(&path);
                if let Err(_e) = run_check(&path, false, false, false, None, codicil_mode, strict, optimize_flag, false, None) {
                    std::process::exit(1);
                }
            } else if command.ends_with(".rbv") || command.ends_with(".srbv") {
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