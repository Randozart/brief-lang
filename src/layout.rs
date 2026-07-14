// ── Layout Preprocessor ────────────────────────────────────────────────
// 2026-07-12: Phase 1.7 — Converts indentation-based syntax to brace-based.
// For .f.bv files (Fortran-style layout).
// Detects mixed tabs/spaces and reports an error.

use crate::errors::SyntaxError;

#[derive(Debug, Clone)]
pub struct LayoutPreprocessor;

impl LayoutPreprocessor {
    /// Process indentation-based source into brace-based source.
    /// Algorithm: track indentation stack, emit { on increase, } on decrease.
    pub fn process(source: &str) -> Result<String, SyntaxError> {
        let mut output = String::new();
        let mut indent_stack: Vec<usize> = vec![0];
        let mut had_tabs = false;
        let mut had_spaces = false;

        for line in source.lines() {
            let trimmed = line.trim_start();
            if trimmed.is_empty() || trimmed.starts_with("//") {
                output.push_str(line);
                output.push('\n');
                continue;
            }

            // Detect mixed indentation
            let indent = line.len() - trimmed.len();
            if indent > 0 {
                // 2026-07-14: First char is at index 0, not `indent`
                let first_char = line.as_bytes().first().copied().unwrap_or(0);
                if first_char == b'\t' {
                    had_tabs = true;
                } else if first_char == b' ' {
                    had_spaces = true;
                }
            }
            if had_tabs && had_spaces {
                return Err(SyntaxError::InvalidExpression {
                    reason: "mixed tabs and spaces in indentation".into(),
                    span: crate::errors::Span::dummy(),
                });
            }

            // Adjust indentation stack
            while indent < *indent_stack.last().unwrap_or(&0) {
                output.push_str("}\n");
                indent_stack.pop();
            }
            if indent > *indent_stack.last().unwrap_or(&0) {
                output.push_str("{\n");
                indent_stack.push(indent);
            }

            output.push_str(trimmed);
            // Emit semicolon at end of line (unless it's a block opener or closer)
            if !trimmed.ends_with('{') && !trimmed.ends_with('}') && !trimmed.ends_with(';') {
                output.push(';');
            }
            output.push('\n');
        }

        // Close remaining open blocks
        while indent_stack.len() > 1 {
            output.push_str("}\n");
            indent_stack.pop();
        }

        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_function() {
        let input = "defn main() -> Int\n    term 0\n";
        let result = LayoutPreprocessor::process(input).unwrap();
        assert!(result.contains("defn main() -> Int\n{"));
        assert!(result.contains("term 0;"));
        assert!(result.contains("}"));
    }

    #[test]
    fn test_nested_blocks() {
        let input = "defn f() -> Int\n    if true\n        term 1\n    term 0\n";
        let result = LayoutPreprocessor::process(input).unwrap();
        assert!(result.contains("{"));
        assert!(result.contains("term 1;"));
    }

    #[test]
    fn test_mixed_indentation_error() {
        let input = "\tdefn foo()\n    term 0;\n";
        assert!(LayoutPreprocessor::process(input).is_err());
    }

    #[test]
    fn test_empty_lines_preserved() {
        let input = "defn f() -> Int\n    term 0\n\n    term 1\n";
        let result = LayoutPreprocessor::process(input).unwrap();
        assert!(result.contains("term 0;\n\n"));
    }
}
