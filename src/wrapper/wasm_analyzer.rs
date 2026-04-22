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

//! WASM Analyzer - Analyzes WebAssembly modules

use super::AnalyzedFunction;
use std::fs;
use std::path::Path;

/// Analyze a WASM file and extract function signatures
pub fn analyze_wasm(wasm_path: &Path) -> Result<Vec<AnalyzedFunction>, String> {
    let extension = wasm_path.extension().and_then(|e| e.to_str()).unwrap_or("");

    match extension {
        "wat" => analyze_wat(wasm_path),
        "wasm" => {
            // Binary WASM - would need wasmparser crate for full analysis
            // For now, return a placeholder
            Ok(vec![AnalyzedFunction {
                name: "wasm_init".to_string(),
                return_type: "Void".to_string(),
                parameters: vec![],
                is_variadic: false,
                comments: vec!["TODO: Parse binary WASM".to_string()],
            }])
        }
        _ => Err("Unknown WASM file type".to_string()),
    }
}

/// Analyze a WASM Text format (.wat) file
fn analyze_wat(wat_path: &Path) -> Result<Vec<AnalyzedFunction>, String> {
    let content =
        fs::read_to_string(wat_path).map_err(|e| format!("Failed to read .wat file: {}", e))?;

    let mut functions = Vec::new();

    // Simple parsing of wat file - look for (func ...) forms
    let mut in_func = false;
    let mut current_func_name = String::new();
    let mut current_params: Vec<(String, String)> = Vec::new();
    let mut current_results: Vec<String> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        // Track function definitions
        if trimmed.starts_with("(func") || (in_func && trimmed.starts_with("(func")) {
            in_func = true;

            // Extract function name if present
            if let Some(name_start) = trimmed.find("export ") {
                let name_part = &trimmed[name_start + 7..];
                if let Some(name_end) = name_part.find('"') {
                    current_func_name = name_part[..name_end].to_string();
                }
            }

            // Extract params (param ...)
            if trimmed.contains("(param ") {
                let params_section = extract_wat_section(trimmed, "(param ");
                for param in params_section {
                    let parts: Vec<&str> = param.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let name = parts[1].to_string();
                        let type_ = wat_type_to_brief(parts[0]);
                        current_params.push((name, type_));
                    }
                }
            }

            // Extract results (result ...)
            if trimmed.contains("(result ") {
                let results_section = extract_wat_section(trimmed, "(result ");
                for result in results_section {
                    current_results.push(wat_type_to_brief(result.trim()));
                }
            }

            // End of function definition
            if trimmed.contains(')') && !trimmed.contains("(func") {
                if !current_func_name.is_empty() {
                    let return_type = if current_results.is_empty() {
                        "Void".to_string()
                    } else {
                        current_results
                            .first()
                            .cloned()
                            .unwrap_or_else(|| "Void".to_string())
                    };

                    functions.push(AnalyzedFunction {
                        name: current_func_name.clone(),
                        return_type,
                        parameters: current_params.clone(),
                        is_variadic: false,
                        comments: Vec::new(),
                    });
                }

                // Reset for next function
                current_func_name.clear();
                current_params.clear();
                current_results.clear();
                in_func = false;
            }
        }
    }

    if functions.is_empty() {
        // Fallback - create placeholder
        functions.push(AnalyzedFunction {
            name: "wasm_entry".to_string(),
            return_type: "Void".to_string(),
            parameters: vec![],
            is_variadic: false,
            comments: vec!["Parsed from .wat file".to_string()],
        });
    }

    Ok(functions)
}

/// Extract a section from WAT text
fn extract_wat_section(text: &str, prefix: &str) -> Vec<String> {
    let mut results = Vec::new();
    let mut current = String::new();
    let mut in_section = false;
    let mut paren_count = 0;

    for ch in text.chars() {
        if ch == '(' {
            paren_count += 1;
            if text[current.len()..].starts_with(prefix) {
                in_section = true;
            }
        }

        if in_section {
            current.push(ch);
        }

        if ch == ')' {
            paren_count -= 1;
            if in_section && paren_count == 0 {
                // Remove ( and )
                let section = current.trim_start_matches('(').trim_end_matches(')');
                results.push(section.to_string());
                current.clear();
                in_section = false;
            }
        }
    }

    results
}

/// Convert WASM type to Brief type
fn wat_type_to_brief(wat_type: &str) -> String {
    match wat_type.trim() {
        "i32" | "i64" => "Int".to_string(),
        "f32" | "f64" => "Float".to_string(),
        "v128" => "Data".to_string(),
        _ => "Int".to_string(),
    }
}

/// Convert WASM function to Brief frgn sig format
pub fn wasm_func_to_frgn_sig(func: &AnalyzedFunction) -> String {
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
