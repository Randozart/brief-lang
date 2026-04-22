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

//! Rust Crate Analyzer - Analyzes Rust crates to extract FFI functions

use super::AnalyzedFunction;
use std::fs;
use std::path::Path;

/// Analyze a Rust crate and extract FFI functions
pub fn analyze_rust_crate(cargo_path: &Path) -> Result<Vec<AnalyzedFunction>, String> {
    let mut functions = Vec::new();

    // First, check if this is a Cargo.toml directory
    if !cargo_path.join("Cargo.toml").exists() {
        return Err("Not a Rust crate (no Cargo.toml found)".to_string());
    }

    // Try to read src/lib.rs and src/main.rs
    let src_files = vec![
        cargo_path.join("src/lib.rs"),
        cargo_path.join("src/main.rs"),
    ];

    for src_path in src_files {
        if src_path.exists() {
            if let Ok(functions_from_file) = extract_ffi_functions(&src_path) {
                functions.extend(functions_from_file);
            }
        }
    }

    Ok(functions)
}

/// Extract FFI functions from a Rust source file
fn extract_ffi_functions(src_path: &Path) -> Result<Vec<AnalyzedFunction>, String> {
    let content =
        fs::read_to_string(src_path).map_err(|e| format!("Failed to read source: {}", e))?;

    let mut functions = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim();

        // Look for #[no_mangle] or extern blocks
        if line.contains("#[no_mangle]") || line.contains("extern") {
            let mut func_line = line.to_string();
            let mut j = i + 1;

            // Collect multi-line function signature
            while j < lines.len() && !lines[j].contains('{') {
                func_line.push(' ');
                func_line.push_str(lines[j].trim());
                j += 1;
            }

            // Parse the function
            if let Some(func) = parse_rust_function(&func_line) {
                functions.push(func);
            }
        }

        i += 1;
    }

    Ok(functions)
}

/// Parse a Rust function signature with FFI markers
fn parse_rust_function(line: &str) -> Option<AnalyzedFunction> {
    // Skip non-function lines
    if !line.contains("fn ") && !line.contains("pub fn ") {
        return None;
    }

    // Extract function name and signature
    let fn_start = line.find("fn ").or(line.find("pub fn "))? + 3;
    let rest = &line[fn_start..];

    let name_end = rest.find('(')?;
    let name = rest[..name_end].trim().to_string();

    // Skip internal functions
    if name.starts_with('_') {
        return None;
    }

    // Extract parameters
    let params_start = name_end + 1;
    let params_end = rest.find(')').unwrap_or(params_start);
    let params_str = &rest[params_start..params_end];

    let mut parameters = Vec::new();
    if !params_str.trim().is_empty() {
        for param in params_str.split(',') {
            let param = param.trim();
            if param.is_empty() || param == "self" || param.starts_with("self,") {
                continue;
            }

            // Format: name: Type or mut name: Type
            let parts: Vec<&str> = param.split(':').collect();
            if parts.len() >= 2 {
                let p_name = parts[0].trim().trim_start_matches("mut ").to_string();
                let p_type = parts[1].trim().to_string();
                parameters.push((p_name, rust_type_to_brief(&p_type)));
            }
        }
    }

    // Extract return type
    let return_type = if let Some(arrow_pos) = rest.find("->") {
        let ret = &rest[arrow_pos + 2..];
        let ret_end = ret
            .find(',')
            .unwrap_or(ret.len())
            .min(ret.find('{').unwrap_or(ret.len()));
        rust_type_to_brief(&ret[..ret_end].trim())
    } else {
        "Void".to_string()
    };

    Some(AnalyzedFunction {
        name,
        return_type,
        parameters,
        is_variadic: false,
        comments: Vec::new(),
    })
}

/// Convert Rust type to Brief type
fn rust_type_to_brief(rust_type: &str) -> String {
    let t = rust_type.trim();

    match t {
        "()" => "Void".to_string(),
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" => "Int".to_string(),
        "u8" | "u16" | "u32" | "u64" | "u128" | "usize" => "Int".to_string(),
        "f32" | "f64" => "Float".to_string(),
        "bool" => "Bool".to_string(),
        "String" | "&str" => "String".to_string(),
        "Vec<u8>" | "&[u8]" => "Data".to_string(),
        "Vec<String>" => "List<String>".to_string(),
        _ => {
            if t.starts_with("Vec<") {
                let inner = &t[4..t.len() - 1];
                format!("List<{}>", rust_type_to_brief(inner))
            } else if t.starts_with("Option<") {
                let inner = &t[7..t.len() - 1];
                format!("Maybe<{}>", rust_type_to_brief(inner))
            } else {
                format!("Custom({})", t)
            }
        }
    }
}

/// Convert Rust function to Brief frgn sig format
pub fn rust_func_to_frgn_sig(func: &AnalyzedFunction) -> String {
    let params: Vec<String> = func
        .parameters
        .iter()
        .map(|(n, t)| format!("{}: {}", n, t))
        .collect();

    let params_str = if params.is_empty() {
        "".to_string()
    } else {
        format!(", {}", params.join(", "))
    };

    format!(
        "frgn sig {}({}) -> {};",
        func.name,
        params_str.trim_start_matches(", "),
        func.return_type
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rust_function() {
        let line = "pub fn add(a: i32, b: i32) -> i32";
        let func = parse_rust_function(line).unwrap();
        assert_eq!(func.name, "add");
        assert_eq!(func.return_type, "Int");
        assert_eq!(func.parameters.len(), 2);
    }
}
