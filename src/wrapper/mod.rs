//! Library Analyzer - Analyzes foreign libraries to extract function signatures
//!
//! Supports: C headers, Rust crates, WASM modules

use std::path::Path;

pub mod c_analyzer;
pub mod contracts;
pub mod generator;
pub mod interactive;
pub mod rust_analyzer;
pub mod wasm_analyzer;

/// Represents an analyzed foreign function
#[derive(Debug, Clone)]
pub struct AnalyzedFunction {
    pub name: String,
    pub return_type: String,
    pub parameters: Vec<(String, String)>,
    pub is_variadic: bool,
    pub comments: Vec<String>,
}

/// Result of library analysis
#[derive(Debug, Clone)]
pub struct AnalysisResult {
    pub library_name: String,
    pub mapper: String,
    pub functions: Vec<AnalyzedFunction>,
}

/// Detect library type and return appropriate analyzer
pub fn detect_library_type(path: &Path) -> &'static str {
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    match extension {
        "h" | "c" => "c",
        "rs" => "rust",
        "wasm" | "wat" => "wasm",
        "so" | "dylib" | "dll" => "native",
        _ => {
            // Check if it's a directory with Cargo.toml
            if path.join("Cargo.toml").exists() {
                "rust"
            } else {
                "unknown"
            }
        }
    }
}

/// Analyze a library and extract function signatures
pub fn analyze_library(
    path: &Path,
    explicit_mapper: Option<&str>,
) -> Result<AnalysisResult, String> {
    let lib_type = if let Some(m) = explicit_mapper {
        m
    } else {
        detect_library_type(path)
    };

    let library_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    match lib_type {
        "c" | "C" => {
            let functions = c_analyzer::analyze_c_header(path)?;
            Ok(AnalysisResult {
                library_name,
                mapper: "c".to_string(),
                functions,
            })
        }
        "rust" => {
            let functions = rust_analyzer::analyze_rust_crate(path)?;
            Ok(AnalysisResult {
                library_name,
                mapper: "rust".to_string(),
                functions,
            })
        }
        "wasm" => {
            let functions = wasm_analyzer::analyze_wasm(path)?;
            Ok(AnalysisResult {
                library_name,
                mapper: "wasm".to_string(),
                functions,
            })
        }
        _ => Err(format!("Unknown library type: {}", lib_type)),
    }
}

/// Convert C type to Brief type
pub fn c_type_to_brief(c_type: &str) -> String {
    let t = c_type.trim().to_lowercase();

    match t.as_str() {
        "void" => "Void".to_string(),
        "int" | "long" | "short" | "char" | "size_t" | "ssize_t" | "int32_t" | "int64_t" => {
            "Int".to_string()
        }
        "float" | "double" | "float32" | "float64" => "Float".to_string(),
        "char*" | "const char*" | "char []" => "String".to_string(),
        "bool" | "_Bool" => "Bool".to_string(),
        "void*" | "void *" | "int*" | "float*" => "Data".to_string(),
        _ => format!("Custom({})", c_type),
    }
}

/// Check if a C type is a pointer
pub fn c_type_is_pointer(c_type: &str) -> bool {
    c_type.contains('*') || c_type.contains("[]")
}

/// Parse function signature from C declaration
pub fn parse_c_signature(line: &str) -> Option<AnalyzedFunction> {
    let line = line.trim();

    // Skip function-like macros without bodies
    if line.contains("#define") && !line.contains("(") {
        return None;
    }

    // Simple function signature pattern: return_type name(params);
    // Or: return_type name(params);
    let params_start = line.find('(')?;
    let params_end = line.find(')')?;

    let before_params = &line[..params_start];
    let params_str = &line[params_start + 1..params_end];

    // Extract return type and function name
    let parts: Vec<&str> = before_params.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }

    let return_type = parts[..parts.len() - 1].join(" ");
    let name = parts.last()?.to_string();

    // Skip internal functions
    if name.starts_with('_') && !name.starts_with("__") {
        return None;
    }

    // Skip pointers to functions (typedefs)
    if before_params.contains("(*)") || before_params.contains("(*") {
        return None;
    }

    // Parse parameters
    let mut parameters = Vec::new();
    if !params_str.trim().is_empty() && params_str != "void" {
        for param in params_str.split(',') {
            let param = param.trim();
            if param.is_empty() {
                continue;
            }

            // Handle parameter names like "int* name" or "const char* name"
            let param_parts: Vec<&str> = param.split_whitespace().collect();
            if param_parts.len() >= 2 {
                let p_type = param_parts[..param_parts.len() - 1].join(" ");
                let p_name = param_parts.last().unwrap().to_string();
                parameters.push((p_name, p_type));
            } else if param_parts.len() == 1 {
                // Single token - might be just a type like "void"
                parameters.push(("arg".to_string(), param_parts[0].to_string()));
            }
        }
    }

    let is_variadic = params_str.contains("...");

    Some(AnalyzedFunction {
        name,
        return_type,
        parameters,
        is_variadic,
        comments: Vec::new(),
    })
}
