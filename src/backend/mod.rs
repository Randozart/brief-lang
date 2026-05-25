pub mod aarch64;
pub mod c;
pub mod rust;
pub mod verilog;
pub mod vhdl;
pub mod wasm;
pub mod webstack;
pub mod x86_64;
pub mod tcl_generator;
pub mod cobol;

use crate::ast::Hashtag;

/// Returns the list of hashtags supported by a given backend name.
/// Backend names match the subcommand (e.g. "c", "rust", "wasm", "verilog", "vhdl", "x86_64", "aarch64", "cobol").
pub fn supported_hashtags(backend: &str) -> Vec<&'static str> {
    match backend {
        "c" | "x86_64" | "aarch64" => {
            vec!["volatile", "sfence", "lfence", "mfence", "aligned", "packed"]
        }
        "rust" => {
            vec!["volatile", "sync", "aligned", "repr", "packed"]
        }
        "wasm" | "webstack" => {
            vec!["volatile", "aligned"]
        }
        "verilog" | "vhdl" => {
            vec!["clock", "register", "gate", "posedge", "negedge"]
        }
        "cobol" => {
            vec!["volatile", "packed", "aligned"]
        }
        _ => {
            vec![] // unknown backend — no known support
        }
    }
}

/// Result of validating a single hashtag against a backend.
#[derive(Debug, Clone, PartialEq)]
pub enum HashtagValidation {
    Supported,
    UnsupportedAdvisory(String),
    UnsupportedMandatory(String),
}

/// Validate a list of hashtags against a given backend.
/// Returns a list of validation results — callers should emit
/// warnings for `UnsupportedAdvisory` and errors for `UnsupportedMandatory`.
pub fn validate_hashtags(hashtags: &[Hashtag], backend: &str) -> Vec<HashtagValidation> {
    let supported = supported_hashtags(backend);
    let mut results = Vec::new();

    for tag in hashtags {
        // Check scoped tags: only validate if scope matches the current backend
        if let Some(ref scope) = tag.scoped {
            if scope != backend {
                continue; // not our concern
            }
        }

        let is_supported = supported.iter().any(|s| *s == tag.name);

        if is_supported {
            results.push(HashtagValidation::Supported);
        } else if tag.mandatory {
            // Check fallback chain
            let fallback_supported = tag.fallback.iter().any(|f| supported.contains(&f.as_str()));
            if fallback_supported {
                results.push(HashtagValidation::Supported);
            } else {
                results.push(HashtagValidation::UnsupportedMandatory(tag.name.clone()));
            }
        } else {
            results.push(HashtagValidation::UnsupportedAdvisory(tag.name.clone()));
        }
    }

    results
}

use crate::ast::{Program, TopLevel, Transaction, Definition, Statement, StructDefinition};

fn collect_hashtags_from_body(body: &[Statement]) -> Vec<crate::ast::Hashtag> {
    let mut tags = Vec::new();
    for stmt in body {
        match stmt {
            Statement::Assignment { modifiers, .. } => tags.extend(modifiers.clone()),
            Statement::Let { modifiers, .. } => tags.extend(modifiers.clone()),
            Statement::Term { modifiers, .. } => tags.extend(modifiers.clone()),
            Statement::Guarded { statements, .. } => tags.extend(collect_hashtags_from_body(statements)),
            Statement::OnExit { body, .. } => tags.extend(collect_hashtags_from_body(body)),
            _ => {}
        }
    }
    tags
}

/// Validate all hashtags in a program against the target backend.
/// Returns true if there are NO unsupported mandatory tag errors.
/// Prints warnings/eprintfs for unsupported tags.
pub fn validate_hashtags_in_program(program: &Program, backend: &str, strict: bool) -> bool {
    let mut all_tags: Vec<crate::ast::Hashtag> = Vec::new();

    for item in &program.items {
        match item {
            TopLevel::Transaction(txn) => {
                all_tags.extend(txn.modifiers.clone());
                all_tags.extend(collect_hashtags_from_body(&txn.body));
                for (_, variant_body) in &txn.variant_bodies {
                    all_tags.extend(collect_hashtags_from_body(variant_body));
                }
            }
            TopLevel::Definition(defn) => {
                all_tags.extend(defn.modifiers.clone());
                all_tags.extend(collect_hashtags_from_body(&defn.body));
                for (_, variant_body) in &defn.variant_bodies {
                    all_tags.extend(collect_hashtags_from_body(variant_body));
                }
            }
            TopLevel::Struct(sdef) => {
                all_tags.extend(sdef.modifiers.clone());
            }
            TopLevel::StateDecl(..) => {} // top-level let, no hashtags
            _ => {}
        }
    }

    let results = validate_hashtags(&all_tags, backend);
    let mut has_errors = false;

    for result in &results {
        match result {
            HashtagValidation::Supported => {}
            HashtagValidation::UnsupportedAdvisory(name) => {
                eprintln!("warning: Hashtag #{} is not supported by {} backend (advisory, ignored)", name, backend);
            }
            HashtagValidation::UnsupportedMandatory(name) => {
                eprintln!("error: Mandatory hashtag #!{} is not supported by {} backend", name, backend);
                if strict {
                    eprintln!("  Hint: Use a different backend, remove the tag, or add fallbacks with #!A|B|C");
                }
                has_errors = true;
            }
        }
    }

    !has_errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Hashtag;

    #[test]
    fn test_c_backend_supports_volatile() {
        let tag = Hashtag { name: "volatile".into(), value: None, mandatory: false, fallback: vec![], scoped: None };
        let results = validate_hashtags(&[tag], "c");
        assert_eq!(results[0], HashtagValidation::Supported);
    }

    #[test]
    fn test_c_backend_rejects_unknown_advisory() {
        let tag = Hashtag { name: "thermal_sense".into(), value: None, mandatory: false, fallback: vec![], scoped: None };
        let results = validate_hashtags(&[tag], "c");
        assert_eq!(results[0], HashtagValidation::UnsupportedAdvisory("thermal_sense".to_string()));
    }

    #[test]
    fn test_c_backend_rejects_unknown_mandatory() {
        let tag = Hashtag { name: "thermal_sense".into(), value: None, mandatory: true, fallback: vec![], scoped: None };
        let results = validate_hashtags(&[tag], "c");
        assert_eq!(results[0], HashtagValidation::UnsupportedMandatory("thermal_sense".to_string()));
    }

    #[test]
    fn test_fallback_chain_tries_alternatives() {
        let tag = Hashtag {
            name: "unknown_op".into(),
            value: None,
            mandatory: true,
            fallback: vec!["lfence".to_string(), "mfence".to_string()],
            scoped: None,
        };
        let results = validate_hashtags(&[tag], "c");
        assert_eq!(results[0], HashtagValidation::Supported);
    }

    #[test]
    fn test_fallback_chain_all_unknown() {
        let tag = Hashtag {
            name: "unknown_op".into(),
            value: None,
            mandatory: true,
            fallback: vec!["nope1".to_string(), "nope2".to_string()],
            scoped: None,
        };
        let results = validate_hashtags(&[tag], "c");
        assert_eq!(results[0], HashtagValidation::UnsupportedMandatory("unknown_op".to_string()));
    }

    #[test]
    fn test_scoped_tag_skipped_for_wrong_backend() {
        let tag = Hashtag {
            name: "volatile".into(),
            value: None,
            mandatory: false,
            fallback: vec![],
            scoped: Some("verilog".to_string()),
        };
        let results = validate_hashtags(&[tag], "c");
        assert_eq!(results.len(), 0, "Scoped tag should be skipped for wrong backend");
    }

    #[test]
    fn test_scoped_tag_validated_for_correct_backend() {
        let tag = Hashtag {
            name: "clock".into(),
            value: None,
            mandatory: false,
            fallback: vec![],
            scoped: Some("verilog".to_string()),
        };
        let results = validate_hashtags(&[tag], "verilog");
        assert_eq!(results[0], HashtagValidation::Supported);
    }
}
