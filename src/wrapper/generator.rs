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

//! File Generator - Generates Brief FFI files from analysis results

use super::c_analyzer::{c_func_to_frgn_sig, suggest_postconditions, suggest_preconditions};
use super::js_analyzer::js_func_to_frgn_sig;
use super::python_analyzer::py_func_to_frgn_sig;
use super::rust_analyzer::rust_func_to_frgn_sig;
use super::wasm_analyzer::wasm_func_to_frgn_sig;
use super::{AnalysisResult, AnalyzedFunction};
use std::fs;
use std::path::Path;

/// Generate lib.bv content from analysis results
pub fn generate_lib_bv(result: &AnalysisResult) -> String {
    let mut output = String::new();

    output.push_str(&format!(
        "// Auto-generated wrapper for {}\n// Mapper: {}\n// Generated functions: {}\n\n",
        result.library_name,
        result.mapper,
        result.functions.len()
    ));

    output.push_str("// Foreign function declarations (frgn sig)\n");

    for func in &result.functions {
        let frgn_sig = match result.mapper.as_str() {
            "c" => c_func_to_frgn_sig(func),
            "rust" => rust_func_to_frgn_sig(func),
            "wasm" => wasm_func_to_frgn_sig(func),
            "js" => js_func_to_frgn_sig(func),
            "python" => py_func_to_frgn_sig(func),
            _ => format!("// Unknown mapper: {}", result.mapper),
        };

        // Add comments if present
        for comment in &func.comments {
            output.push_str(&format!("// {}\n", comment));
        }

        output.push_str(&frgn_sig);
        output.push_str("\n\n");
    }

    output.push_str(
        "// =============================================================================\n",
    );
    output.push_str("// User-defined implementations\n");
    output.push_str(
        "// =============================================================================\n\n",
    );

    // Generate template defns with suggested contracts
    for func in &result.functions {
        let preconditions = match result.mapper.as_str() {
            "c" => suggest_preconditions(func),
            _ => vec!["true".to_string()],
        };

        let postconditions = match result.mapper.as_str() {
            "c" => suggest_postconditions(func),
            _ => vec!["true".to_string()],
        };

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

        output.push_str(&format!(
            "// {}Implementation for {}\ndefn {}({}) -> {} [\n  {}  // precondition\n][\n  {}  // postcondition\n] {{\n  __raw_{}({})\n}};\n\n",
            if func.comments.is_empty() { "" } else { "\n" },
            func.name,
            func.name,
            params_str.trim_start_matches(", "),
            func.return_type,
            preconditions.join(" && "),
            postconditions.join(" && "),
            func.name,
            if params.is_empty() { "".to_string() } else { params_str.trim_start_matches(", ").to_string() }
        ));
    }

    output
}

/// Generate bindings.toml content from analysis results
pub fn generate_bindings_toml(result: &AnalysisResult) -> String {
    let mut output = String::new();

    output.push_str(&format!(
        "# Auto-generated bindings for {}\n# Mapper: {}\n\n",
        result.library_name, result.mapper,
    ));

    for func in &result.functions {
        output.push_str(&format!("[[functions]]\n"));
        output.push_str(&format!("name = \"{}\"\n", func.name));
        output.push_str(&format!(
            "location = \"{}::{}\"\n",
            result.library_name.replace('-', "_"),
            func.name
        ));
        output.push_str(&format!("target = \"{}\"\n", detect_target(&result.mapper)));
        output.push_str(&format!("mapper = \"{}\"\n", result.mapper));

        if let Some(desc) = func.comments.first() {
            output.push_str(&format!("description = \"{}\"\n", desc));
        }

        output.push_str("\n[functions.input]\n");
        for (name, brief_type) in &func.parameters {
            output.push_str(&format!("{} = \"{}\"\n", name, brief_type));
        }

        output.push_str("\n[functions.output.success]\n");
        if func.return_type != "Void" {
            output.push_str(&format!("result = \"{}\"\n", func.return_type));
        }

        output.push_str("\n[functions.output.error]\n");
        output.push_str(&format!("type = \"{}Error\"\n", func.name));
        output.push_str("code = \"Int\"\n");
        output.push_str("message = \"String\"\n");

        output.push('\n');
    }

    output
}

/// Detect target from mapper
fn detect_target(mapper: &str) -> &str {
    match mapper {
        "wasm" => "wasm",
        "c" => "native",
        "rust" => "native",
        "js" => "native",     // JavaScript uses native FFI via Node.js or WASM
        "python" => "native", // Python uses native FFI via CPython
        _ => "native",
    }
}

/// Write generated files to directory
pub fn write_generated_files(
    result: &AnalysisResult,
    output_dir: &Path,
    force: bool,
) -> Result<(), String> {
    if !output_dir.exists() {
        fs::create_dir_all(output_dir)
            .map_err(|e| format!("Failed to create output directory: {}", e))?;
    }

    let lib_bv_path = output_dir.join("lib.bv");
    let toml_path = output_dir.join("bindings.toml");

    if lib_bv_path.exists() && !force {
        return Err(format!("lib.bv already exists (use --force to overwrite)"));
    }

    if toml_path.exists() && !force {
        return Err(format!(
            "bindings.toml already exists (use --force to overwrite)"
        ));
    }

    let lib_bv_content = generate_lib_bv(result);
    let toml_content = generate_bindings_toml(result);

    fs::write(&lib_bv_path, lib_bv_content)
        .map_err(|e| format!("Failed to write lib.bv: {}", e))?;

    fs::write(&toml_path, toml_content)
        .map_err(|e| format!("Failed to write bindings.toml: {}", e))?;

    Ok(())
}

/// Preview generated content without writing files
pub fn preview_generated(result: &AnalysisResult) -> String {
    let mut output = String::new();

    output.push_str("=== lib.bv (preview) ===\n\n");
    output.push_str(&generate_lib_bv(result));
    output.push_str("\n=== bindings.toml (preview) ===\n\n");
    output.push_str(&generate_bindings_toml(result));

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_bindings_toml() {
        let result = AnalysisResult {
            library_name: "test".to_string(),
            mapper: "c".to_string(),
            functions: vec![AnalyzedFunction {
                name: "add".to_string(),
                return_type: "Int".to_string(),
                parameters: vec![
                    ("a".to_string(), "Int".to_string()),
                    ("b".to_string(), "Int".to_string()),
                ],
                is_variadic: false,
                comments: vec![],
            }],
        };

        let toml = generate_bindings_toml(&result);
        assert!(toml.contains("[[functions]]"));
        assert!(toml.contains("name = \"add\""));
    }
}
