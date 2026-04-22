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

//! JavaScript/Node Analyzer
//!
//! Analyzes JavaScript and TypeScript files to extract function signatures.
//! Supports:
//! - TypeScript declaration files (.d.ts)
//! - JavaScript files with JSDoc comments
//! - ES6 modules with exports

use super::AnalyzedFunction;
use std::path::Path;

/// Analyze a JavaScript/TypeScript file
pub fn analyze_js(path: &Path) -> Result<Vec<AnalyzedFunction>, String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))?;

    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    match extension {
        "d.ts" | "ts" => analyze_typescript(&content),
        "js" | "mjs" | "jsx" => analyze_javascript(&content),
        _ => Err(format!("Unsupported JavaScript file type: {}", extension)),
    }
}

/// Analyze TypeScript declaration or source file
fn analyze_typescript(content: &str) -> Result<Vec<AnalyzedFunction>, String> {
    let mut functions = Vec::new();

    for line in content.lines() {
        let line = line.trim();

        // Skip comments and empty lines
        if line.is_empty() || line.starts_with("//") || line.starts_with("/*") {
            continue;
        }

        // Function declaration: function name(params): returnType;
        // Method declaration: name(params): returnType;
        // Arrow function: (params) => returnType
        // Export: export function name(params): returnType;
        // Export default: export default function name(params): returnType;

        if let Some(func) = parse_ts_function(line) {
            functions.push(func);
        }

        // Interface property: name: type;
        // Type alias: type Name = ...
        // Class: class Name { ... }
    }

    Ok(functions)
}

/// Parse a TypeScript function declaration
fn parse_ts_function(line: &str) -> Option<AnalyzedFunction> {
    let line = line.trim();

    // Remove export modifiers
    let line = line.strip_prefix("export ").unwrap_or(line);
    let line = line.strip_prefix("declare ").unwrap_or(line);
    let line = line.strip_prefix("async ").unwrap_or(line);

    // Function declaration
    if line.starts_with("function ") {
        return parse_ts_function_decl(line, "function ");
    }

    // Method in interface/class: name(): type;
    // Or arrow: (a: Type) => Type
    if line.contains("=>") {
        return parse_ts_arrow_function(line);
    }

    // Interface method: name(params): returnType;
    if line.contains("(") && line.contains(")") && !line.contains("=>") {
        return parse_ts_method(line);
    }

    None
}

/// Parse "function name(params): returnType"
fn parse_ts_function_decl(line: &str, prefix: &str) -> Option<AnalyzedFunction> {
    let line = line.strip_prefix(prefix)?;

    // Extract name
    let name_end = line.find(|c: char| !c.is_alphanumeric() && c != '_')?;
    let name = line[..name_end].to_string();

    let rest = line[name_end..].trim();

    // Find parameters
    let params_start = rest.find('(')?;
    let params_end = rest.find(')')?;
    let params_str = &rest[params_start + 1..params_end];

    let parameters = parse_ts_parameters(params_str);

    // Find return type
    let return_type = if let Some(colon_pos) = rest[params_end + 1..].find(':') {
        let ret_str = rest[params_end + 1 + colon_pos + 1..].trim();
        let ret_type = ret_str.trim_end_matches(';').trim_end_matches(',');
        js_type_to_brief(ret_type)
    } else {
        "Void".to_string()
    };

    Some(AnalyzedFunction {
        name,
        return_type,
        parameters,
        is_variadic: params_str.contains("..."),
        comments: Vec::new(),
    })
}

/// Parse "(a: Type, b: Type) => ReturnType"
fn parse_ts_arrow_function(line: &str) -> Option<AnalyzedFunction> {
    let paren_start = line.find('(')?;
    let paren_end = line.find(')')?;

    let params_str = &line[paren_start + 1..paren_end];
    let parameters = parse_ts_parameters(params_str);

    // Everything after ) => is return type
    let after_arrow = line[paren_end + 1..].trim();
    let return_type = after_arrow
        .strip_prefix("=>")
        .map(|s| s.trim())
        .map(js_type_to_brief)
        .unwrap_or_else(|| "Void".to_string());

    // Generate a name from the context if anonymous
    let name = format!("arrow_{}", parameters.len());

    Some(AnalyzedFunction {
        name,
        return_type,
        parameters,
        is_variadic: params_str.contains("..."),
        comments: Vec::new(),
    })
}

/// Parse "name(params): returnType" (interface method style)
fn parse_ts_method(line: &str) -> Option<AnalyzedFunction> {
    let before_paren = line.find('(')?;
    let name = line[..before_paren].trim().to_string();

    // Skip if it looks like a type annotation rather than a function
    if name.is_empty() || name.contains(':') || name.contains('{') {
        return None;
    }

    let after_paren = line[before_paren..].trim();
    if !after_paren.starts_with('(') {
        return None;
    }

    let paren_end = after_paren.find(')')?;
    let params_str = &after_paren[1..paren_end];

    let parameters = parse_ts_parameters(params_str);

    // Find return type after )
    let after = after_paren[paren_end + 1..].trim();
    let return_type = if after.starts_with(':') {
        let ret = after[1..]
            .trim()
            .trim_end_matches(';')
            .trim_end_matches(',');
        js_type_to_brief(ret)
    } else {
        "Void".to_string()
    };

    Some(AnalyzedFunction {
        name,
        return_type,
        parameters,
        is_variadic: params_str.contains("..."),
        comments: Vec::new(),
    })
}

/// Parse TypeScript parameter list "(a: Type, b?: Type)"
fn parse_ts_parameters(params_str: &str) -> Vec<(String, String)> {
    let mut params = Vec::new();

    if params_str.trim().is_empty() {
        return params;
    }

    let mut depth = 0;
    let mut param_start = 0;

    for (i, c) in params_str.char_indices() {
        match c {
            '(' | '[' | '<' => depth += 1,
            ')' | ']' | '>' => depth -= 1,
            ',' if depth == 0 => {
                if let Some(param) = parse_ts_single_param(&params_str[param_start..i]) {
                    params.push(param);
                }
                param_start = i + 1;
            }
            _ => {}
        }
    }

    // Last parameter
    let last = params_str[param_start..].trim();
    if !last.is_empty() {
        if let Some(param) = parse_ts_single_param(last) {
            params.push(param);
        }
    }

    params
}

/// Parse single parameter "name: Type" or "name?: Type"
fn parse_ts_single_param(param: &str) -> Option<(String, String)> {
    let param = param.trim();

    // Handle rest parameter: ...args: Type[]
    let is_variadic = param.starts_with("...");
    let param = if is_variadic {
        param.strip_prefix("...").unwrap_or(param)
    } else {
        param
    };

    // Remove optional marker
    let param = param.strip_suffix("?").unwrap_or(param);

    // Split on colon
    if let Some(colon_pos) = param.find(':') {
        let name = param[..colon_pos].trim().to_string();
        let type_str = param[colon_pos + 1..].trim().to_string();

        // Handle union types: Type1 | Type2 - take first
        let type_str = type_str.split('|').next().unwrap_or(&type_str).trim();

        // Handle array types: Type[]
        let type_str = type_str.strip_suffix("[]").unwrap_or(type_str).trim();

        // Handle generic types: Array<T> -> List<Value>
        let type_str = if type_str.starts_with("Array<") {
            "List<Value>"
        } else if type_str.starts_with("Promise<") {
            "Void" // Async would need special handling
        } else {
            type_str
        };

        Some((name, js_type_to_brief(type_str)))
    } else {
        // No type annotation - assume Int
        Some((param.to_string(), "Int".to_string()))
    }
}

/// Analyze JavaScript file with JSDoc comments
fn analyze_javascript(content: &str) -> Result<Vec<AnalyzedFunction>, String> {
    let mut functions = Vec::new();
    let mut current_comment = Vec::new();
    let mut in_multiline_comment = false;

    for line in content.lines() {
        let line_trimmed = line.trim();

        // Handle multiline comments
        if in_multiline_comment {
            if line_trimmed.contains("*/") {
                in_multiline_comment = false;
            }
            continue;
        }

        if line_trimmed.starts_with("/*") {
            in_multiline_comment = true;
            continue;
        }

        // Single line comment
        if line_trimmed.starts_with("//") {
            continue;
        }

        // Check for JSDoc before function
        if line_trimmed.contains("/**") {
            // Collect JSDoc lines until we find the function
            current_comment.clear();
            continue;
        }

        if line_trimmed.starts_with(" *") || line_trimmed.starts_with("*") {
            let doc_line = line_trimmed
                .strip_prefix(" *")
                .or_else(|| line_trimmed.strip_prefix("*"))
                .unwrap_or(line_trimmed);
            current_comment.push(doc_line.to_string());
            continue;
        }

        // Check if this line is a function
        if let Some(func) = parse_js_function(line_trimmed, &current_comment) {
            functions.push(func);
            current_comment.clear();
        }
    }

    Ok(functions)
}

/// Parse JavaScript function declaration
fn parse_js_function(line: &str, jsdoc: &[String]) -> Option<AnalyzedFunction> {
    let line = line.trim();

    // Remove async modifier
    let is_async = line.starts_with("async ");
    let line = line.strip_prefix("async ").unwrap_or(line);

    // Function declaration: function name(params) { ... }
    if let Some(name) = line.strip_prefix("function ") {
        let name = name
            .trim_start_matches(|c: char| c.is_alphanumeric() || c == '_')
            .trim_start()
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .next()
            .unwrap_or("");

        if name.is_empty() {
            return None;
        }

        return parse_js_function_params(line, name, is_async, jsdoc);
    }

    // Arrow function: const name = (...) => ...
    // or: let name = (...) => ...
    if line.starts_with("const ") || line.starts_with("let ") || line.starts_with("var ") {
        if let Some((name, rest)) = line.split_once('=') {
            let name = name
                .trim_start_matches("const ")
                .trim_start_matches("let ")
                .trim_start_matches("var ");
            let rest = rest.trim();

            if rest.starts_with("(") {
                let params_str = extract_arrow_params(rest)?;
                let parameters = parse_js_parameters(params_str);
                let return_type = infer_js_return_type(jsdoc);

                return Some(AnalyzedFunction {
                    name: name.to_string(),
                    return_type,
                    parameters,
                    is_variadic: params_str.contains("..."),
                    comments: jsdoc.iter().map(|s| s.trim().to_string()).collect(),
                });
            }
        }
    }

    // Export: export function name(params) { ... }
    // or: export const name = (...) => ...
    if let Some(export_line) = line.strip_prefix("export ") {
        return parse_js_function(export_line, jsdoc);
    }

    None
}

/// Parse parameters from "(a, b, c)" or "(a: Type, b: Type)"
fn parse_js_function_params(
    line: &str,
    name: &str,
    is_async: bool,
    jsdoc: &[String],
) -> Option<AnalyzedFunction> {
    let paren_start = line.find('(')?;
    let paren_end = line.find(')')?;

    let params_str = &line[paren_start + 1..paren_end];
    let parameters = parse_js_parameters(params_str);

    let return_type = infer_js_return_type(jsdoc);

    Some(AnalyzedFunction {
        name: name.to_string(),
        return_type,
        parameters,
        is_variadic: params_str.contains("..."),
        comments: jsdoc.iter().map(|s| s.trim().to_string()).collect(),
    })
}

/// Extract parameters from arrow function "(a, b) => ..."
fn extract_arrow_params(rest: &str) -> Option<&str> {
    let paren_start = rest.find('(')?;
    let paren_end = rest.find(')')?;
    Some(&rest[paren_start + 1..paren_end])
}

/// Parse JavaScript parameters (no type annotations)
fn parse_js_parameters(params_str: &str) -> Vec<(String, String)> {
    let mut params = Vec::new();

    if params_str.trim().is_empty() {
        return params;
    }

    for param in params_str.split(',') {
        let param = param.trim();
        if param.is_empty() {
            continue;
        }

        // Remove default value: "a = 1" -> "a"
        let param = param.split('=').next().unwrap_or(param).trim();

        // Remove rest operator: "...args" -> "args"
        let param = if param.starts_with("...") {
            param.strip_prefix("...").unwrap_or(param)
        } else {
            param
        };

        params.push((param.to_string(), "Int".to_string())); // Default to Int for untyped JS
    }

    params
}

/// Infer return type from JSDoc
fn infer_js_return_type(jsdoc: &[String]) -> String {
    for line in jsdoc {
        if line.contains("@returns") || line.contains("@return") {
            // Extract type from @returns {Type}
            if let Some(start) = line.find('@') {
                let after_at = &line[start..];
                if let Some(type_start) = after_at.find('{') {
                    if let Some(type_end) = after_at.find('}') {
                        let type_str = &after_at[type_start + 1..type_end];
                        return js_type_to_brief(type_str.trim());
                    }
                }
            }
        }
    }
    "Void".to_string() // Default if no JSDoc
}

/// Convert JavaScript/TypeScript type to Brief type
pub fn js_type_to_brief(js_type: &str) -> String {
    let t = js_type.trim();

    // Handle union types (take first)
    if let Some(first) = t.split('|').next() {
        let t = first.trim();

        match t {
            // Void/Null/Undefined
            "void" | "undefined" | "null" | "never" => "Void".to_string(),

            // Primitive types
            "string" | "String" => "String".to_string(),
            "number" | "Number" | "bigint" | "BigInt" => "Int".to_string(), // JS numbers are typically Int
            "boolean" | "Boolean" => "Bool".to_string(),
            "symbol" | "Symbol" => "Custom(Symbol)".to_string(),
            "any" | "unknown" => "Value".to_string(),

            // Object types
            "object" | "Object" | "{}" => "Data".to_string(),
            "Array" | "array" => "List<Value>".to_string(),
            "Map" | "Set" | "WeakMap" | "WeakSet" => "Data".to_string(),
            "Promise" | "AsyncIterator" => "Void".to_string(), // Async - simplified
            "Buffer" | "ArrayBuffer" | "TypedArray" | "DataView" => "Data".to_string(),

            // Date/Time
            "Date" => "Custom(Date)".to_string(),
            "Error" | "EvalError" | "RangeError" | "ReferenceError" | "SyntaxError"
            | "TypeError" => "Error".to_string(),

            // Functions
            "Function" | "Callback" | "() => void" => "Value".to_string(),

            // DOM types
            "Element" | "HTMLElement" | "HTMLInputElement" | "HTMLButtonElement" => {
                "Custom(Element)".to_string()
            }
            "Event" | "MouseEvent" | "KeyboardEvent" => "Custom(Event)".to_string(),
            "Node" | "Document" | "Window" => "Custom(Node)".to_string(),

            // RegExp
            "RegExp" => "Custom(RegExp)".to_string(),

            // JSON
            "JSON" => "String".to_string(), // JSON.parse returns string

            // Default - pass through
            other => {
                // Handle Array<T> -> List<Value>
                if other.starts_with("Array<") {
                    return "List<Value>".to_string();
                }
                // Handle Map<K, V> -> Data
                if other.starts_with("Map<") || other.starts_with("Set<") {
                    return "Data".to_string();
                }
                // Handle Promise<T>
                if other.starts_with("Promise<") {
                    return "Void".to_string();
                }
                // Handle union types in brackets
                if other.starts_with('(') {
                    return "Value".to_string();
                }
                other.to_string()
            }
        }
    } else {
        "Value".to_string()
    }
}

/// Generate frgn sig from analyzed function (for generator.rs)
pub fn js_func_to_frgn_sig(func: &AnalyzedFunction) -> String {
    let params: Vec<String> = func
        .parameters
        .iter()
        .map(|(n, t)| format!("{}: {}", n, t))
        .collect();

    format!(
        "frgn sig {}: {} -> {};",
        func.name,
        params.join(", "),
        func.return_type
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_js_type_to_brief() {
        assert_eq!(js_type_to_brief("string"), "String");
        assert_eq!(js_type_to_brief("number"), "Int");
        assert_eq!(js_type_to_brief("boolean"), "Bool");
        assert_eq!(js_type_to_brief("string | null"), "String");
        assert_eq!(js_type_to_brief("Array<string>"), "List<Value>");
        assert_eq!(js_type_to_brief("Promise<number>"), "Void");
    }

    #[test]
    fn test_parse_ts_function() {
        let result = parse_ts_function("function add(a: number, b: number): number");
        assert!(result.is_some());
        let func = result.unwrap();
        assert_eq!(func.name, "add");
        assert_eq!(func.return_type, "Int");
        assert_eq!(func.parameters.len(), 2);
    }

    #[test]
    fn test_parse_ts_arrow() {
        let result = parse_ts_arrow_function("(a: string, b: number) => boolean");
        assert!(result.is_some());
        let func = result.unwrap();
        assert_eq!(func.return_type, "Bool");
    }
}
