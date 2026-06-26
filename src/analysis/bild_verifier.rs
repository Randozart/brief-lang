use crate::ast::InopDeclaration;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub enum BildError {
    UndefinedRegister { name: String, line: usize },
    MissingTerminator,
}

impl BildError {
    pub fn to_diagnostic(&self, inop_name: &str) -> crate::errors::Diagnostic {
        match self {
            BildError::UndefinedRegister { name, line } => {
                crate::errors::Diagnostic::new(
                    "B001",
                    crate::errors::Severity::Error,
                    "undefined register in BILD body",
                )
                .with_explanation(&format!(
                    "inop `{}`: register `%{}` is used but not defined at line {}",
                    inop_name, name, line
                ))
            }
            BildError::MissingTerminator => {
                crate::errors::Diagnostic::new(
                    "B002",
                    crate::errors::Severity::Error,
                    "missing terminator in BILD body",
                )
                .with_explanation(&format!(
                    "inop `{}`: BILD body must end with `term` or `term!`",
                    inop_name
                ))
            }
        }
    }
}

pub fn check_bild(inop: &InopDeclaration) -> Vec<BildError> {
    let mut errors = Vec::new();

    let mut defined: HashSet<String> = HashSet::new();
    for (name, _) in &inop.params {
        defined.insert(format!("%{}", name));
    }

    let mut has_term = false;

    for (i, line) in inop.llvm_body.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed == "}" {
            continue;
        }

        // Use source span line number if available
        let source_line = inop.llvm_body_spans.get(i).map(|s| s.line).unwrap_or(i);

        let after_semi = trimmed.strip_suffix(';').unwrap_or(trimmed);

        if after_semi.starts_with("term") || after_semi.starts_with("term!") {
            has_term = true;
            let after_keyword = if after_semi.starts_with("term!") {
                &after_semi[5..]
            } else {
                &after_semi[4..]
            };
            check_regs(after_keyword, &defined, &mut errors, source_line);
            continue;
        }

        if let Some(eq_pos) = after_semi.find(" = ") {
            let lhs = after_semi[..eq_pos].trim();
            if lhs.starts_with('%') && !lhs.contains(' ') {
                let rhs = &after_semi[eq_pos + 3..];
                check_regs(rhs, &defined, &mut errors, source_line);
                defined.insert(lhs.to_string());
            }
        } else {
            check_regs(after_semi, &defined, &mut errors, source_line);
        }
    }

    if !has_term && !inop.llvm_body.is_empty() {
        errors.push(BildError::MissingTerminator);
    }

    errors
}

fn check_regs(s: &str, defined: &HashSet<String>, errors: &mut Vec<BildError>, line: usize) {
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            let mut name = String::from('%');
            while let Some(&nc) = chars.peek() {
                if nc.is_alphanumeric() || nc == '_' {
                    name.push(nc);
                    chars.next();
                } else {
                    break;
                }
            }
            if name.len() > 1
                && name != "%state"
                && !defined.contains(&name)
            {
                errors.push(BildError::UndefinedRegister {
                    name: name[1..].to_string(),
                    line,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Contract, Expr, InopDeclaration, Type};
    use std::collections::HashMap;

    fn make_inop(name: &str, params: Vec<(&str, Type)>, body: Vec<&str>) -> InopDeclaration {
        InopDeclaration {
            name: name.to_string(),
            type_params: vec![],
            params: params.into_iter().map(|(n, t)| (n.to_string(), t)).collect(),
            outputs: vec![Type::Int],
            contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
            llvm_body: body.into_iter().map(|s| s.to_string()).collect(),
            fallback: None,
            has_side_effects: false,
            has_state_access: false,
            section: None,
            llvm_body_spans: vec![],
            span: None,
        }
    }

    #[test]
    fn test_valid_bild_body() {
        let inop = make_inop("sadd", vec![("a", Type::Int), ("b", Type::Int)], vec![
            "%res = add i64 %a, %b;",
            "term %res;",
        ]);
        let errors = check_bild(&inop);
        assert!(errors.is_empty(), "expected no errors, got: {:?}", errors);
    }

    #[test]
    fn test_undefined_register() {
        let inop = make_inop("bad", vec![("a", Type::Int)], vec![
            "%res = add i64 %a, %b;",
            "term %res;",
        ]);
        let errors = check_bild(&inop);
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], BildError::UndefinedRegister { name, .. } if name == "b"));
    }

    #[test]
    fn test_missing_terminator() {
        let inop = make_inop("noterm", vec![("a", Type::Int)], vec![
            "%res = add i64 %a, %a;",
        ]);
        let errors = check_bild(&inop);
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], BildError::MissingTerminator));
    }

    #[test]
    fn test_forward_reference_is_valid() {
        let inop = make_inop("valid", vec![("a", Type::Int)], vec![
            "%t1 = add i64 %a, %a;",
            "%res = add i64 %t1, %a;",
            "term %res;",
        ]);
        let errors = check_bild(&inop);
        assert!(errors.is_empty(), "forward reference should be valid: {:?}", errors);
    }

    #[test]
    fn test_term_bang_valid() {
        let inop = make_inop("side", vec![("x", Type::Int)], vec![
            "term!;",
        ]);
        let errors = check_bild(&inop);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_empty_body_no_errors() {
        let inop = make_inop("empty", vec![("a", Type::Int)], vec![]);
        let errors = check_bild(&inop);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_undefined_self_reference() {
        let inop = make_inop("selfref", vec![("a", Type::Int)], vec![
            "%res = add i64 %res, %a;",
            "term %res;",
        ]);
        let errors = check_bild(&inop);
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], BildError::UndefinedRegister { name, .. } if name == "res"));
    }

    #[test]
    fn test_state_register_not_flagged() {
        let inop = make_inop("has_state", vec![("a", Type::Int)], vec![
            "%val = load i64, i8* %state;",
            "term %val;",
        ]);
        let errors = check_bild(&inop);
        assert!(errors.is_empty(), "%state should not be flagged: {:?}", errors);
    }
}
