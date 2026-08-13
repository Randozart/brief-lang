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

//! Canonical language vocabulary.
//!
//! 2026-08-05 (normative spec Phase 1): one machine-readable source of truth
//! for keywords, reserved words, operators, sigils, file extensions/profiles,
//! casing conventions, and staged/removed language surface. The lexer, LSP,
//! TextMate grammar, formatter, and diagnostics must agree with this module.
//! See `spec/SPEC.md` §4, §23 and the implementation plan Phase 1.
//!
//! Status semantics:
//! - `Canonical`: valid, exact-spelling vocabulary in the normative grammar.
//! - `Removed`: no longer part of Briev; the parser must reject it (Phase 3)
//!   with migration guidance. Retained here only so diagnostics can explain.
//! - `Reserved`: intentionally unavailable to user identifiers for a future
//!   language contract (`sed`, `pvt`, `reg`).
//!
//! Compiler-known vocabulary (keywords, hashwords, intrinsics, operation
//! identities, stages) requires exact spelling/casing; violations are errors.
//! User-declared identifiers that violate casing conventions are only
//! informational/warning diagnostics.

use serde::{Deserialize, Serialize};

/// 2026-08-05: lifecycle status of a vocabulary entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VocabStatus {
    Canonical,
    Removed,
    Reserved,
}

/// 2026-08-05: broad grammar role used by the LSP/highlighter/manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeywordContext {
    Declaration,
    Statement,
    Modifier,
    Literal,
    Operator,
    CompileTime,
    Reactive,
    Foreign,
    Render,
    Ownership,
    Reserved,
}

/// 2026-08-05: one keyword entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Keyword {
    pub name: String,
    pub status: VocabStatus,
    pub context: KeywordContext,
}

/// 2026-08-05: identifier casing conventions. User-declared violations are
/// warning/info only; compiler-known vocabulary is exact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Casing {
    PascalCase,
    SnakeCase,
    PascalCaseHash,
}

/// 2026-08-05: the full canonical vocabulary, serializable for tooling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageVocab {
    pub version: String,
    pub keywords: Vec<Keyword>,
    pub operators: Vec<String>,
    pub sigils: Vec<String>,
    pub extensions: Vec<String>,
    pub profiles: Vec<String>,
    pub hashwords: Vec<String>,
    pub intrinsics: Vec<String>,
    pub operation_identities: Vec<String>,
    pub stages: Vec<String>,
    pub staged_features: Vec<String>,
    pub casing: Vec<(String, Casing)>,
}

impl Default for LanguageVocab {
    fn default() -> Self {
        Self::canonical()
    }
}

impl LanguageVocab {
    /// 2026-08-05: the canonical vocabulary defined here is the single source
    /// of truth. When a keyword/operator is added or removed, update this list
    /// and the Phase 1 parity tests.
    pub fn canonical() -> Self {
        let kw = |name: &str, status: VocabStatus, context: KeywordContext| Keyword {
            name: name.to_string(),
            status,
            context,
        };
        let ss = |names: &[&str]| names.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        LanguageVocab {
            version: "2026-08-05.1".to_string(),
            keywords: vec![
                // Declarations
                kw("let", VocabStatus::Canonical, KeywordContext::Declaration),
                kw("const", VocabStatus::Canonical, KeywordContext::Declaration),
                kw("init", VocabStatus::Canonical, KeywordContext::Declaration),
                kw("type", VocabStatus::Canonical, KeywordContext::Declaration),
                kw("trait", VocabStatus::Canonical, KeywordContext::Declaration),
                kw("proto", VocabStatus::Canonical, KeywordContext::Declaration),
                kw("struct", VocabStatus::Canonical, KeywordContext::Declaration),
                kw("seq", VocabStatus::Canonical, KeywordContext::Modifier),
                // 2026-08-13 (layout-keywords plan): `pack` — bit-contiguous,
                // zero-padding struct modifier (`pack struct`). Disclosed, like
                // `seq`; never a speed win over the default representation.
                kw("pack", VocabStatus::Canonical, KeywordContext::Modifier),
                // 2026-08-13 (layout-keywords plan): `spec` — physical-layout
                // metadata statement (`spec Bits: 64;`). Declared layout, the
                // disclosed sibling of the `!>` annotation form.
                kw("spec", VocabStatus::Canonical, KeywordContext::Modifier),
                kw("enum", VocabStatus::Canonical, KeywordContext::Declaration),
                kw("impl", VocabStatus::Canonical, KeywordContext::Declaration),
                kw("obj", VocabStatus::Canonical, KeywordContext::Declaration),
                kw("cell", VocabStatus::Canonical, KeywordContext::Declaration),
                kw("defn", VocabStatus::Canonical, KeywordContext::Declaration),
                kw("txn", VocabStatus::Canonical, KeywordContext::Declaration),
                kw("node", VocabStatus::Canonical, KeywordContext::Declaration),
                kw("asm", VocabStatus::Canonical, KeywordContext::Declaration),
                kw("render", VocabStatus::Canonical, KeywordContext::Render),
                kw("frgn", VocabStatus::Canonical, KeywordContext::Foreign),
                kw("optional", VocabStatus::Canonical, KeywordContext::Foreign),
                kw("export", VocabStatus::Canonical, KeywordContext::Declaration),
                kw("import", VocabStatus::Canonical, KeywordContext::Declaration),
                kw("from", VocabStatus::Canonical, KeywordContext::Foreign),
                kw("as", VocabStatus::Canonical, KeywordContext::Operator),
                kw("op", VocabStatus::Canonical, KeywordContext::Declaration),
                // Statements and control flow
                kw("term", VocabStatus::Canonical, KeywordContext::Statement),
                kw("endprogram", VocabStatus::Canonical, KeywordContext::Statement),
                kw("beginprogram", VocabStatus::Canonical, KeywordContext::Statement),
                kw("program", VocabStatus::Canonical, KeywordContext::Statement),
                kw("defer", VocabStatus::Canonical, KeywordContext::Statement),
                kw("rollback", VocabStatus::Canonical, KeywordContext::Statement),
                kw("mutex", VocabStatus::Canonical, KeywordContext::Statement),
                kw("barrier", VocabStatus::Canonical, KeywordContext::Statement),
                kw("match", VocabStatus::Canonical, KeywordContext::Statement),
                kw("when", VocabStatus::Canonical, KeywordContext::Statement),
                kw("foreach", VocabStatus::Canonical, KeywordContext::Statement),
                kw("spawn", VocabStatus::Canonical, KeywordContext::Reactive),
                kw("await", VocabStatus::Canonical, KeywordContext::Reactive),
                kw("free", VocabStatus::Canonical, KeywordContext::Ownership),
                kw("keep", VocabStatus::Canonical, KeywordContext::Ownership),
                kw("trg", VocabStatus::Canonical, KeywordContext::Reactive),
                kw("within", VocabStatus::Canonical, KeywordContext::Reactive),
                // Concurrency/reactive classification
                kw("async", VocabStatus::Canonical, KeywordContext::Modifier),
                kw("sync", VocabStatus::Canonical, KeywordContext::Modifier),
                // Modifiers
                kw("vol", VocabStatus::Canonical, KeywordContext::Modifier),
                kw("out", VocabStatus::Canonical, KeywordContext::Modifier),
                // Storage/layout strategy (2026-08-09, Phase 5): `box` marks a
                // spawned value as per-instance-heap (not pooled) when the pool
                // decoder is ambiguous; `spill` allows growth into a growable
                // buffer when a static pool column can't hold the worst case.
                // Contextual (spawn position only) — both stay legal
                // identifiers elsewhere (the compiler backend's own .bv uses
                // `spill` as a register word).
                kw("box", VocabStatus::Canonical, KeywordContext::Modifier),
                kw("spill", VocabStatus::Canonical, KeywordContext::Modifier),
                // Ownership algebra (SPEC §14)
                kw("borrow", VocabStatus::Canonical, KeywordContext::Ownership),
                kw("consume", VocabStatus::Canonical, KeywordContext::Ownership),
                kw("owned", VocabStatus::Canonical, KeywordContext::Ownership),
                kw("shared", VocabStatus::Canonical, KeywordContext::Ownership),
                kw("borrowed", VocabStatus::Canonical, KeywordContext::Ownership),
                // Literals
                kw("true", VocabStatus::Canonical, KeywordContext::Literal),
                kw("false", VocabStatus::Canonical, KeywordContext::Literal),
                // Duration units (canonical abbreviations)
                kw("cyc", VocabStatus::Canonical, KeywordContext::Literal),
                kw("ms", VocabStatus::Canonical, KeywordContext::Literal),
                kw("s", VocabStatus::Canonical, KeywordContext::Literal),
                kw("min", VocabStatus::Canonical, KeywordContext::Literal),
                kw("ns", VocabStatus::Canonical, KeywordContext::Literal),
                // Compile-time
                kw("quote", VocabStatus::Canonical, KeywordContext::CompileTime),
                // Reserved for future contracts
                kw("sed", VocabStatus::Reserved, KeywordContext::Reserved),
                kw("pvt", VocabStatus::Reserved, KeywordContext::Reserved),
                kw("reg", VocabStatus::Reserved, KeywordContext::Reserved),
                // Removed surface (Phase 3 removes from lexer/parser)
                kw("sig", VocabStatus::Removed, KeywordContext::Declaration),
                kw("state", VocabStatus::Removed, KeywordContext::Declaration),
                kw("rstruct", VocabStatus::Removed, KeywordContext::Render),
                kw("uni", VocabStatus::Removed, KeywordContext::Statement),
                kw("is", VocabStatus::Removed, KeywordContext::Operator),
                kw("like", VocabStatus::Removed, KeywordContext::Operator),
                kw("prop", VocabStatus::Removed, KeywordContext::Declaration),
                kw("meld", VocabStatus::Removed, KeywordContext::Declaration),
                kw("syscall", VocabStatus::Removed, KeywordContext::Foreign),
                kw("escape", VocabStatus::Removed, KeywordContext::Statement),
                kw("term!", VocabStatus::Removed, KeywordContext::Statement),
                kw("trg!", VocabStatus::Removed, KeywordContext::Reactive),
                kw("cell!", VocabStatus::Removed, KeywordContext::Reactive),
                kw("sync!", VocabStatus::Removed, KeywordContext::Statement),
                kw("frgn!", VocabStatus::Removed, KeywordContext::Foreign),
                kw("syscall!", VocabStatus::Removed, KeywordContext::Foreign),
                kw("Ptr!", VocabStatus::Removed, KeywordContext::Declaration),
                kw("Ok", VocabStatus::Removed, KeywordContext::Declaration),
                kw("Err", VocabStatus::Removed, KeywordContext::Declaration),
                kw("Some", VocabStatus::Removed, KeywordContext::Declaration),
                kw("None", VocabStatus::Removed, KeywordContext::Declaration),
                kw("some", VocabStatus::Removed, KeywordContext::Declaration),
                kw("none", VocabStatus::Removed, KeywordContext::Declaration),
                // Deprecated duration-unit aliases (canonical abbreviations are
                // cyc/ns/ms/s/min; SPEC §16.1)
                kw("cycles", VocabStatus::Removed, KeywordContext::Literal),
                kw("seconds", VocabStatus::Removed, KeywordContext::Literal),
                kw("minute", VocabStatus::Removed, KeywordContext::Literal),
                kw("minutes", VocabStatus::Removed, KeywordContext::Literal),
                kw("nanoseconds", VocabStatus::Removed, KeywordContext::Literal),
            ],
            operators: ss(&[
                "+", "-", "*", "/", "%", "==", "!=", "<", "<=", ">", ">=", "&&", "||",
                "!", "&", "|", "^", "~", "<<", ">>", "->", "<-", "~<-", "=>", "..",
                "..=", "+=", "-=", "*=", "/=", "=", "@", "$",
            ]),
            sigils: ss(&[
                "#Category", "Intrinsic#", "$name", "name!(...)", "$(Stage)",
                ".^Field", ".^^Field", "!value", "?",
            ]),
            extensions: ss(&["bv", "ebv", "abv", "cbv", "rbv", "dbv", "dbvl"]),
            profiles: ss(&["s", "f"]),
            hashwords: ss(&[
                "Int", "Float", "Bool", "String", "Char", "Bits", "Ptr", "Void",
                "Bit", "Link", "System", "L", "R", "T", "Self", "r", "b",
            ]),
            intrinsics: ss(&[
                "Abs#", "BitReverse#", "Popcount#", "LeadingZeros#", "TrailingZeros#",
                "SysCall#", "Malloc#", "Free#", "Print#", "Sqrt#",
            ]),
            operation_identities: ss(&[
                "Add", "Sub", "Mul", "Div", "Mod", "Eq", "Ne", "Lt", "Le", "Gt", "Ge",
                "And", "Or", "Not", "BitAnd", "BitOr", "BitXor", "BitNot", "Shl", "Shr",
                "At", "Slice", "InsertAt", "ExtractFrom", "CopyFrom", "Append", "Prepend",
            ]),
            stages: ss(&[
                "PreLex", "Parsed", "Resolved", "Typed", "Normalized", "Verified",
                "Allocated", "Provenanced", "Generated", "Optimized", "Linked",
            ]),
            staged_features: ss(&[
                "dyn Trait", "const generics", "spawn/await handles",
                "rollback", "endprogram", "defer", "mutex", "barrier",
                ".f strict indentation", "generic semantic Value",
            ]),
            casing: vec![
                ("types".to_string(), Casing::PascalCase),
                ("traits".to_string(), Casing::PascalCase),
                ("structs".to_string(), Casing::PascalCase),
                ("enums".to_string(), Casing::PascalCase),
                ("objs".to_string(), Casing::PascalCase),
                ("cells".to_string(), Casing::PascalCase),
                ("protocol variants".to_string(), Casing::PascalCase),
                ("operation identities".to_string(), Casing::PascalCase),
                ("functions".to_string(), Casing::SnakeCase),
                ("fields".to_string(), Casing::SnakeCase),
                ("nodes".to_string(), Casing::SnakeCase),
                ("variables".to_string(), Casing::SnakeCase),
                ("macros".to_string(), Casing::SnakeCase),
                ("intrinsics".to_string(), Casing::PascalCaseHash),
            ],
        }
    }

    pub fn is_canonical_keyword(&self, name: &str) -> bool {
        self.keywords
            .iter()
            .any(|k| k.name == name && k.status == VocabStatus::Canonical)
    }

    pub fn is_removed_keyword(&self, name: &str) -> bool {
        self.keywords
            .iter()
            .any(|k| k.name == name && k.status == VocabStatus::Removed)
    }

    pub fn is_reserved(&self, name: &str) -> bool {
        self.keywords
            .iter()
            .any(|k| k.name == name && k.status == VocabStatus::Reserved)
    }

    pub fn keyword_status(&self, name: &str) -> Option<VocabStatus> {
        self.keywords.iter().find(|k| k.name == name).map(|k| k.status)
    }

    pub fn canonical_keywords(&self) -> impl Iterator<Item = &Keyword> {
        self.keywords.iter().filter(|k| k.status == VocabStatus::Canonical)
    }

    pub fn removed_keywords(&self) -> impl Iterator<Item = &Keyword> {
        self.keywords.iter().filter(|k| k.status == VocabStatus::Removed)
    }
}

/// 2026-08-05: serialize the canonical vocabulary for tooling consumption.
/// Emitted as TOML; the LSP/highlighter generators and CI parity tests read it.
pub fn serialize_vocab(vocab: &LanguageVocab) -> Result<String, toml::ser::Error> {
    toml::to_string_pretty(vocab)
}

/// 2026-08-05: regenerate the TextMate grammar keyword patterns from the
/// canonical vocab so the highlighter stops teaching removed syntax. Canonical
/// keywords become control/declaration keywords; intrinsics and bootstrap type
/// names are emitted as their own support categories. Removed/reserved words
/// are deliberately absent (they lex as ordinary identifiers).
pub fn regenerate_highlighter_grammar(grammar_path: &std::path::Path) -> Result<(), String> {
    let text = std::fs::read_to_string(grammar_path)
        .map_err(|e| format!("failed to read grammar '{}': {}", grammar_path.display(), e))?;
    let mut grammar: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("failed to parse grammar JSON: {}", e))?;

    let vocab = LanguageVocab::canonical();
    let mut keywords: Vec<serde_json::Value> = Vec::new();

    let kw_pattern = |names: &[&str], scope: &str| {
        let joined = names.join("|");
        serde_json::json!({
            "name": scope,
            "match": format!("\\b({})\\b", joined)
        })
    };

    // Canonical declaration/statement/modifier/control keywords.
    let control: Vec<&str> = vocab
        .canonical_keywords()
        .map(|k| k.name.as_str())
        .filter(|n| *n != "true" && *n != "false" && *n != "cyc" && *n != "ms"
            && *n != "s" && *n != "min" && *n != "ns")
        .collect();
    keywords.push(kw_pattern(&control, "keyword.control.briev"));

    // Boolean and duration literals.
    keywords.push(kw_pattern(&["true", "false"], "constant.language.briev"));
    keywords.push(kw_pattern(&["cyc", "ns", "ms", "s", "min"], "keyword.other.time-unit.briev"));

    // Intrinsics: `Name#`.
    let intrinsics: Vec<&str> = vocab.intrinsics.iter().map(|s| s.as_str()).collect();
    if !intrinsics.is_empty() {
        let joined = intrinsics
            .iter()
            .map(|s| s.replace('#', "\\#"))
            .collect::<Vec<_>>()
            .join("|");
        keywords.push(serde_json::json!({
            "name": "support.function.intrinsic.briev",
            "match": format!("\\b({})\\b", joined)
        }));
    }

    // Bootstrap type names / primitive hashwords.
    let types: Vec<&str> = vec![
        "Int", "Float", "Bool", "String", "Char", "Void", "Ptr", "Bits",
    ];
    keywords.push(kw_pattern(&types, "support.type.primitive.briev"));

    // Guard: canonical keywords must be exact-lowercase; no uppercase aliases.
    assert!(
        control.iter().all(|k| *k == k.to_lowercase()),
        "highlighter regeneration: canonical keywords must be lowercase"
    );

    grammar["repository"]["keywords"]["patterns"] = serde_json::Value::Array(keywords);
    grammar["repository"]["types"]["patterns"] = serde_json::Value::Array(vec![
        serde_json::json!({
            "name": "support.type.primitive.briev",
            "match": format!("\\b({})\\b", types.join("|"))
        }),
        serde_json::json!({
            "name": "entity.name.type.custom.briev",
            "match": "\\b[A-Z][a-zA-Z0-9_]*\\b"
        }),
    ]);

    let out = serde_json::to_string_pretty(&grammar)
        .map_err(|e| format!("failed to serialize grammar: {}", e))?;
    std::fs::write(grammar_path, out + "\n")
        .map_err(|e| format!("failed to write grammar: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_keywords_are_lowercase() {
        let vocab = LanguageVocab::canonical();
        for kw in vocab.canonical_keywords() {
            assert_eq!(
                kw.name,
                kw.name.to_lowercase(),
                "canonical keyword '{}' must be lowercase (SPEC §4.1)",
                kw.name
            );
        }
    }

    #[test]
    fn no_duplicate_vocab_names() {
        let vocab = LanguageVocab::canonical();
        let mut seen = std::collections::HashSet::new();
        for kw in &vocab.keywords {
            assert!(seen.insert(&kw.name), "duplicate vocab keyword '{}'", kw.name);
        }
    }

    #[test]
    fn reserved_set_is_exactly_sed_pvt_reg() {
        let vocab = LanguageVocab::canonical();
        let reserved: Vec<&str> = vocab
            .keywords
            .iter()
            .filter(|k| k.status == VocabStatus::Reserved)
            .map(|k| k.name.as_str())
            .collect();
        assert_eq!(reserved, vec!["sed", "pvt", "reg"]);
    }

    #[test]
    fn removed_surface_is_recorded() {
        let vocab = LanguageVocab::canonical();
        for name in ["sig", "state", "rstruct", "uni", "meld", "escape", "term!", "frgn!", "Ptr!"] {
            assert_eq!(
                vocab.keyword_status(name),
                Some(VocabStatus::Removed),
                "'{}' should be recorded as removed",
                name
            );
        }
    }

    #[test]
    fn vocab_serializes_and_round_trips() {
        let vocab = LanguageVocab::canonical();
        let text = serialize_vocab(&vocab).expect("vocab serialization must succeed");
        let parsed: LanguageVocab = toml::from_str(&text).expect("vocab round-trip must succeed");
        assert_eq!(parsed, vocab);
    }

    #[test]
    fn canonical_and_removed_are_disjoint() {
        let vocab = LanguageVocab::canonical();
        let mut canonical = std::collections::HashSet::new();
        let mut removed = std::collections::HashSet::new();
        for kw in &vocab.keywords {
            match kw.status {
                VocabStatus::Canonical => { canonical.insert(&kw.name); }
                VocabStatus::Removed => { removed.insert(&kw.name); }
                VocabStatus::Reserved => {}
            }
        }
        assert!(
            canonical.is_disjoint(&removed),
            "a name cannot be both canonical and removed"
        );
    }
}
