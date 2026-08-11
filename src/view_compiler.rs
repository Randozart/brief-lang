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

use crate::ast::{self, Contract, Expr, TopLevel};
use std::collections::{HashMap, HashSet};

const KNOWN_DIRECTIVES: &[&str] = &[
    "b-text", "b-show", "b-hide", "b-on:", "b-trigger:",
    "b-class", "b-attr", "b-style", "b-each",
];

#[derive(Debug, Clone)]
pub struct Binding {
    pub element_id: String,
    pub directive: Directive,
}

#[derive(Debug, Clone)]
pub enum Directive {
    Text {
        signal: String,
    },
    Show {
        expr: String,
    },
    Hide {
        expr: String,
    },
    Trigger {
        event: String,
        txn: String,
        params: Vec<(String, String)>, // parameter name -> value (as string for JS)
    },
    Class {
        pairs: Vec<(String, String)>,
    },
    Attr {
        name: String,
        value: String,
    },
    Style {
        name: String,
        value: String,
    },
    Each {
        iterable: String,
        item_name: String,
        template_html: String,
        container_id: String,
    },
}

pub struct ViewCompiler {
    signals: HashMap<String, usize>,
    transactions: HashMap<String, usize>,
    bindings: Vec<Binding>,
    id_counter: usize,
    each_context: Vec<EachContext>,
    pub diagnostics: Vec<String>,
    /// 2026-08-09 (Phase 14, SPEC 21.4): directive validation errors — `b-if`
    /// rejected, `b-each` without `b-key`, `b-bind:value` on a non-assignable
    /// target. Surfaced by compile() (not silently ignored).
    pub validation_errors: Vec<String>,
    /// Transactions that are triggered by user input (b-trigger:)
    /// These should have preconditions that account for non-deterministic user input
    user_triggered_txns: HashSet<String>,
}

#[derive(Debug, Clone)]
struct EachContext {
    iterable: String,
    item_name: String,
}

impl ViewCompiler {
    pub fn new() -> Self {
        ViewCompiler {
            signals: HashMap::new(),
            transactions: HashMap::new(),
            bindings: Vec::new(),
            id_counter: 0,
            each_context: Vec::new(),
            diagnostics: Vec::new(),
            validation_errors: Vec::new(),
            user_triggered_txns: HashSet::new(),
        }
    }

    pub fn register_signal(&mut self, name: &str, id: usize) {
        self.signals.insert(name.to_string(), id);
    }

    pub fn register_transaction(&mut self, name: &str, id: usize) {
        self.transactions.insert(name.to_string(), id);
    }

    /// Returns transactions that are triggered by user input (b-trigger:)
    /// These should have preconditions that account for non-deterministic user input
    pub fn get_user_triggered_transactions(&self) -> &HashSet<String> {
        &self.user_triggered_txns
    }

    /// Validate that user-triggered transactions have appropriate preconditions
    /// For RBV, preconditions should NOT be too strict since user input is unpredictable
    pub fn validate_user_triggered_preconditions(&self, preconditions: &HashMap<String, String>) -> Vec<String> {
        let mut warnings = Vec::new();

        for txn_name in &self.user_triggered_txns {
            if let Some(pre) = preconditions.get(txn_name) {
                // Warn if precondition is too strict (not accounting for unreliable user input)
                // Common strict patterns that might be problematic:
                // - Preconditions checking external state that user can't guarantee
                // - Preconditions that are only true in specific UI states

                // Check if precondition mentions any variable that's likely user-controlled or external
                let strict_patterns = [
                    "network", "api", "server", "fetch", "http",
                    "database", "db_", "file", "disk", "filesystem",
                ];

                let pre_lower = pre.to_lowercase();
                for pattern in strict_patterns {
                    if pre_lower.contains(pattern) {
                        warnings.push(format!(
                            "Warning[R001]: Transaction '{}' is user-triggered but has precondition referencing '{}' which may not be available when user acts: [{}]",
                            txn_name, pattern, pre
                        ));
                    }
                }
            } else {
                // No precondition found - this might be fine or need checking
                warnings.push(format!(
                    "Info[R002]: Transaction '{}' is user-triggered but has no explicit precondition",
                    txn_name
                ));
            }
        }

        warnings
    }

    fn extract_class_expression(&self, tag: &str) -> Option<String> {
        let tag_lower = tag.to_lowercase();

        if !tag_lower.contains("class=") {
            return None;
        }

        if let Some(cls_pos) = tag_lower.find("class=") {
            let value_start = cls_pos + 6;
            let rest = &tag[value_start..];
            let rest_trimmed = rest.trim_start();

            if !rest_trimmed.starts_with('{') {
                return None;
            }

            let inner = rest_trimmed[1..].trim();
            if let Some(close_pos) = inner.find('}') {
                let inner = &inner[..close_pos];

                if inner.contains('?') && inner.contains(" : ") {
                    return Some(inner.to_string());
                }
            }
        }

        None
    }

    pub fn compile(&mut self, view_html: &str) -> (Vec<Binding>, String, Vec<String>) {
        self.bindings.clear();
        self.diagnostics.clear();
        self.validation_errors.clear();
        let modified_html = self.inject_ids(view_html);
        self.extract_bindings(&modified_html);
        // 2026-08-11: custom component tags (e.g. `<Counter />`) are first-class
        // reactive instances per SPEC 21.3 — mount/unmount wiring is not yet
        // wired into the web runtime, so flag them rather than silently
        // rendering an inert element.
        self.warn_component_tags(&modified_html);
        // 2026-08-09 (Phase 14, SPEC 21.4): directive validation errors are
        // surfaced alongside the existing diagnostics (a rejected directive is
        // not silently ignored).
        let mut all = self.validation_errors.clone();
        all.extend(self.diagnostics.clone());
        (self.bindings.clone(), modified_html, all)
    }

    /// 2026-08-11: warn about custom component tags (`<Name .../>`, PascalCase)
    /// whose mount lifecycle is a Phase 2 component-model feature. Standard
    /// (lowercase) HTML elements are unaffected.
    fn warn_component_tags(&mut self, html: &str) {
        let mut pos = 0;
        while let Some(rel) = html[pos..].find('<') {
            let start = pos + rel;
            let rest = &html[start + 1..];
            let next = rest.as_bytes().first().copied();
            if let Some(c) = next {
                if c.is_ascii_uppercase() {
                    let tag_end = rest
                        .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
                        .unwrap_or(rest.len());
                    let name = &rest[..tag_end];
                    self.diagnostics.push(format!(
                        "component tag '<{name}>' mounts a first-class reactive \
                         instance (SPEC 21.3); mount/unmount wiring lands with \
                         the Phase 2 component plan — rendered inert for now"
                    ));
                }
            }
            pos = start + 1;
        }
    }

    fn inject_ids(&mut self, html: &str) -> String {
        let mut result = String::new();
        let mut pos = 0;
        let bytes = html.as_bytes();

        while pos < bytes.len() {
            if bytes[pos] == b'<'
                && bytes
                    .get(pos + 1)
                    .map(|&b| b.is_ascii_alphabetic() || b == b'!')
                    .unwrap_or(false)
            {
                if let Some((tag, end_pos)) = self.parse_tag(&html[pos..]) {
                    let tag_str = &html[pos..pos + end_pos];
                    let tag_lower = tag_str.to_lowercase();

                    if tag_lower.starts_with('/')
                        || tag_lower.starts_with('!')
                        || tag_lower.ends_with("/>")
                    {
                        result.push_str(tag_str);
                        pos += end_pos;
                        continue;
                    }

                    let tag_process = tag_str;

                    // 2026-08-09 (Phase 14, SPEC 21.4): directive validation —
                    // `b-if` is invalid; `b-each` requires a stable `b-key`
                    // (dynamic children may be inserted/removed/reordered);
                    // `b-bind:value` targets an assignable field. Errors are
                    // collected (the compile() caller reports them) — a
                    // rejected directive is not silently ignored.
                    if let Some(verr) = self.validate_directives(&tag_str) {
                        self.validation_errors.push(verr);
                    }

                    let has_class_expr = self.extract_class_expression(&tag_str);
                    let has_b_class = tag_lower.contains("b-class");
                    let has_directive = tag_lower.contains("b-text")
                        || tag_lower.contains("b-show")
                        || tag_lower.contains("b-hide")
                        || tag_lower.contains("b-trigger")
                        || tag_lower.contains("b-on")
                        || has_b_class
                        || tag_lower.contains("b-attr")
                        || tag_lower.contains("b-style")
                        || tag_lower.contains("b-each")
                        || has_class_expr.is_some();

                    if has_directive {
                        // Use preprocessed tag for directive processing
                        let elem_id = if !tag_lower.contains("id=") {
                            self.generate_element_id(&tag_process)
                        } else {
                            self.extract_id_from_tag(&tag_process)
                                .unwrap_or_else(|| self.generate_element_id(&tag_process))
                        };
                        let tag_name = tag_process.split_whitespace().next().unwrap_or("");
                        let tag_name_stripped = tag_name.trim_start_matches('<');

                        let tag_with_id = if !tag_lower.contains("id=") {
                            let rest = &tag_process[tag_name.len()..];
                            format!("<{} id=\"{}\"{}", tag_name_stripped, elem_id, rest)
                        } else {
                            tag_process.to_string()
                        };

                        result.push_str(&tag_with_id);

                        // Pass the computed elem_id to extract_directives so it uses consistent IDs
                        self.extract_directives(&tag_with_id, &elem_id);
                    } else {
                        result.push_str(tag_str);
                    }
                    pos += end_pos;
                    continue;
                }
            }
            result.push(html.chars().nth(pos).unwrap_or(' '));
            pos += 1;
        }

        result
    }

    fn extract_bindings(&mut self, html: &str) {
        // Note: b-trigger extraction now happens in inject_ids -> extract_directives
        // This function handles b-each and other directives that need full HTML parsing
        let mut pos = 0;
        let bytes = html.as_bytes();
        let mut element_stack: Vec<(String, usize)> = Vec::new();

        while pos < bytes.len() {
            if bytes[pos] == b'<'
                && bytes
                    .get(pos + 1)
                    .map(|&b| b.is_ascii_alphabetic() || b == b'!')
                    .unwrap_or(false)
            {
                if let Some((tag, end_pos)) = self.parse_tag(&html[pos..]) {
                    let tag_str = String::from_utf8_lossy(&bytes[pos..pos + end_pos]).to_string();
                    let tag_lower = tag_str.to_lowercase();

                    if tag_lower.starts_with('/') {
                        let closing_name = tag_lower
                            .trim_start_matches('/')
                            .split_whitespace()
                            .next()
                            .unwrap_or("");
                        if let Some(pos_in_stack) = element_stack
                            .iter()
                            .position(|(name, _)| name == closing_name)
                        {
                            element_stack.truncate(pos_in_stack);
                        }
                        pos += end_pos;
                        continue;
                    }

                    if !tag_lower.ends_with("/>") && !tag_lower.ends_with("?") {
                        let elem_name = tag.split_whitespace().next().unwrap_or("div").to_string();
                        element_stack.push((elem_name, pos));
                    }

                    let has_each = tag_lower.contains("b-each:");

                    if has_each {
                        let each_attr = tag_lower
                            .split_whitespace()
                            .find(|s| s.contains("b-each:"))
                            .unwrap_or("");
                        if let Some((item_name, iterable)) = self.extract_each_value(each_attr) {
                            let elem_id = self.generate_element_id(&tag_str);
                            let inner_html = self.find_each_inner_html(&html[pos..], &tag);
                            let elem_name = tag.split_whitespace().next().unwrap_or(&tag).trim();
                            let _tag_attrs: String = tag
                                .split_whitespace()
                                .skip(1)
                                .filter(|s| !s.starts_with("b-"))
                                .collect::<Vec<_>>()
                                .join(" ");
                            let template_html = inner_html.clone();

                            let _container_id =
                                if let Some((_, parent_pos)) = element_stack.iter().rev().nth(0) {
                                    let parent_html = &html[*parent_pos..];
                                    if let Some((parent_tag, _)) = self.parse_tag(parent_html) {
                                        if let Some(id) = self.extract_id_from_tag(&parent_tag) {
                                            id
                                        } else {
                                            format!(
                                                "rbv-{}",
                                                parent_tag
                                                    .split_whitespace()
                                                    .next()
                                                    .unwrap_or("container")
                                            )
                                        }
                                    } else {
                                        "rbv-container".to_string()
                                    }
                                } else {
                                    "rbv-container".to_string()
                                };

                            self.bindings.push(Binding {
                                element_id: elem_id.clone(),
                                directive: Directive::Each {
                                    iterable: iterable,
                                    item_name: item_name,
                                    template_html: template_html,
                                    container_id: elem_id,
                                },
                            });
                            let total_len = end_pos + inner_html.len() + elem_name.len() + 3;
                            pos += total_len;
                            continue;
                        }
                    }

                    // extract_directives already called in inject_ids - skip to avoid duplicates
                    pos += end_pos;
                    continue;
                }
            }
            pos += 1;
        }
    }

    fn extract_id_from_tag(&self, tag: &str) -> Option<String> {
        let tag_lower = tag.to_lowercase();
        if let Some(id_pos) = tag_lower.find("id=") {
            let after = &tag[id_pos + 3..];
            let trimmed = after
                .trim_start_matches('=')
                .trim_start_matches('\"')
                .trim_start_matches('\'');
            let end = trimmed
                .find(|c: char| c.is_whitespace() || c == '\"' || c == '\'' || c == '>')
                .unwrap_or(trimmed.len());
            return Some(trimmed[..end].to_string());
        }
        None
    }

    fn find_each_inner_html(&self, html: &str, tag: &str) -> String {
        let elem_name = tag.split_whitespace().next().unwrap_or(tag).trim();
        let closing_pattern = format!("</{}>", elem_name);
        if let Some(closing_pos) = html.find(&closing_pattern) {
            if let Some(open_end) = html.find('>') {
                if open_end < closing_pos {
                    return html[open_end + 1..closing_pos].trim().to_string();
                }
            }
        }
        String::new()
    }

    fn parse_tag<'a>(&self, s: &'a str) -> Option<(String, usize)> {
        if !s.starts_with('<') {
            return None;
        }

        let end = s.find('>')?;
        let tag = &s[1..end];
        Some((tag.to_string(), end + 1))
    }

    fn extract_directives(&mut self, tag: &str, elem_id: &str) {
        let tag_lower = tag.to_lowercase();
        // Check for bare class={expr} syntax
        if let Some(expr) = self.extract_class_expression(tag) {
            let pairs = self.parse_class_expr(&expr);
            self.bindings.push(Binding {
                element_id: elem_id.to_string(),
                directive: Directive::Class { pairs },
            });
        }

        // First pass: validate directive prefixes
        for attr in tag_lower.split_whitespace().skip(1) {
            let attr = attr.trim_end_matches('>').trim_end_matches('/');
            if attr.starts_with("b-") {
                let prefix = if let Some(idx) = attr.find('=') {
                    attr[..idx].to_string()
                } else {
                    attr[..].to_string()
                };
                let is_known = KNOWN_DIRECTIVES.iter().any(|k| {
                    prefix == *k || prefix.starts_with(k.trim_end_matches(':'))
                });
                if !is_known {
                    self.diagnostics.push(format!(
                        "warning[RBV001]: unknown directive '{}' in tag '{}'",
                        prefix,
                        tag.split_whitespace().next().unwrap_or("<tag>")
                    ));
                }
            }
        }

        for attr in tag_lower.split_whitespace().skip(1) {
            let attr = attr.trim_end_matches('>').trim_end_matches('/');

if attr.starts_with("b-text") {
                if let Some(expr) = self.extract_attr_value(tag, "b-text") {
                    self.bindings.push(Binding {
                        element_id: elem_id.to_string(),
                        directive: Directive::Text { signal: expr },
                    });
                }
            } else if attr.starts_with("b-show") {
                if let Some(expr) = self.extract_attr_value(tag, "b-show") {
                    self.bindings.push(Binding {
                        element_id: elem_id.to_string(),
                        directive: Directive::Show { expr },
                    });
                }
            } else if attr.starts_with("b-hide") {
                if let Some(expr) = self.extract_attr_value(tag, "b-hide") {
                    self.bindings.push(Binding {
                        element_id: elem_id.to_string(),
                        directive: Directive::Hide { expr },
                    });
                }
            } else if attr.starts_with("b-trigger:") || attr.starts_with("b-on:") {
                let prefix = if attr.starts_with("b-trigger:") { "b-trigger:" } else { "b-on:" };
                let result = self.extract_trigger_value_from_tag(tag, prefix);
                let event = self.extract_event_suffix(&tag_lower, prefix.trim_end_matches(':'));
                if let Some((txn_name, params)) = result {
                    // Track user-triggered transactions for linting
                    // These should have preconditions that account for non-deterministic user input
                    self.user_triggered_txns.insert(txn_name.clone());
                    self.bindings.push(Binding {
                        element_id: elem_id.to_string(),
                        directive: Directive::Trigger {
                            event: event.unwrap_or_else(|| "click".to_string()),
                            txn: txn_name,
                            params,
                        },
                    });
                }
            } else if attr.starts_with("b-class") {
                if let Some(expr) = self.extract_attr_value(tag, "b-class") {
                    let pairs = self.parse_class_expr(&expr);
                    self.bindings.push(Binding {
                        element_id: elem_id.to_string(),
                        directive: Directive::Class { pairs },
                    });
                }
            } else if attr.starts_with("b-attr") {
                if let Some(expr) = self.extract_attr_value(tag, "b-attr") {
                    if let Some((name, value)) = self.parse_attr_expr(&expr) {
                        self.bindings.push(Binding {
                            element_id: elem_id.to_string(),
                            directive: Directive::Attr { name, value },
                        });
                    }
                }
            } else if attr.starts_with("b-style") {
                if let Some(expr) = self.extract_attr_value(tag, "b-style") {
                    if let Some((name, value)) = self.parse_attr_expr(&expr) {
                        self.bindings.push(Binding {
                            element_id: elem_id.to_string(),
                            directive: Directive::Style { name, value },
                        });
                    }
                }
            }
        }
    }

    fn extract_trigger_value(&self, attr: &str) -> Option<(String, Vec<(String, String)>)> {
        let after_colon = attr.strip_prefix("b-trigger:")
            .or_else(|| attr.strip_prefix("b-on:"))?;
        let after_event = after_colon.find('=')?;
        let value_part = &after_colon[after_event + 1..];

        let value = value_part.trim();

        let extracted = if value.starts_with('"') {
            let end = value[1..].find('"')?;
            value[1..end + 1].to_string()
        } else if value.starts_with('\'') {
            let end = value[1..].find('\'')?;
            value[1..end + 1].to_string()
} else if value.contains('(') {
            // Function call: extract up to the matching closing paren
            let open_pos = value.find('(')?;
            let rest = &value[open_pos..];
            let close_pos = if let Some(p) = find_closing_paren(rest) {
                open_pos + p + 1
            } else {
                open_pos + rest.len()
            };
            value[..close_pos].to_string()
        } else {
            let end = value
                .find(|c: char| c.is_whitespace() || c == '>')
                .unwrap_or(value.len());
            value[..end].to_string()
        };

        let mut params = Vec::new();

        if let Some(paren_start) = extracted.find('(') {
            let func_name = extracted[..paren_start].to_string();
            let inner = &extracted[paren_start + 1..];
            let inner_trimmed = inner.trim_end_matches(')');

            if !inner_trimmed.is_empty() {
                if inner_trimmed.contains(':') {
                    for pair in inner_trimmed.split(',') {
                        let pair = pair.trim();
                        if let Some(colon_pos) = pair.find(':') {
                            let param_name = pair[..colon_pos].trim().to_string();
                            let raw_value = pair[colon_pos + 1..].trim().to_string();
                            let param_value = strip_surrounding_quotes(&raw_value);
                            params.push((param_name, param_value));
                        }
                    }
                } else {
                    for (i, param) in inner_trimmed.split(',').enumerate() {
                        let raw_value = param.trim().to_string();
                        let param_value = strip_surrounding_quotes(&raw_value);
                        params.push((format!("_{}", i), param_value));
                    }
                }
            }

            Some((func_name, params))
        } else {
            Some((extracted, params))
        }
    }

    /// 2026-08-09 (Phase 14, SPEC 21.4): validate a tag's directives.
    /// - `b-if` is INVALID (structural conditionals use `b-when`).
    /// - `b-each:name` requires a stable `b-key` when children may be
    ///   inserted/removed/reordered (SPEC 21.4: dynamic repetition requires a
    ///   stable key).
    /// - `b-bind:value="target"` must target an assignable field (a computed
    ///   expression is invalid — separate value/trigger handlers are used).
    /// Returns a human-readable error, or None if the directives are valid.
    fn validate_directives(&self, tag: &str) -> Option<String> {
        let tag_lower = tag.to_lowercase();
        if tag_lower.contains("b-if") {
            return Some(
                "`b-if` is invalid (SPEC 21.4) — use `b-when` for structural \
                 mount/unmount and `b-show` for presentation-only visibility"
                    .into(),
            );
        }
        let has_each = tag_lower.contains("b-each:");
        if has_each && !tag_lower.contains("b-key") {
            return Some(
                "`b-each` requires a stable `b-key` (SPEC 21.4) — dynamic children \
                 may be inserted, removed, or reordered and need a stable identity"
                    .into(),
            );
        }
        if tag_lower.contains("b-bind:value") {
            // `b-bind:value="field"` must be an assignable field — an
            // expression (with operators) is invalid per SPEC 21.4.
            if let Some(target) = self.extract_attr_value(tag, "b-bind:value") {
                let trimmed = target.trim();
                let is_identifier = !trimmed.is_empty()
                    && trimmed
                        .chars()
                        .all(|c| c.is_alphanumeric() || c == '_' || c == '.');
                if !is_identifier {
                    return Some(
                        "`b-bind:value` accepts only an assignable field (SPEC 21.4) — \
                         computed expressions use separate value and trigger handlers"
                            .into(),
                    );
                }
            }
        }
        // 2026-08-09 (Phase 14, SPEC 21.5): view expressions are pure and
        // read-only — mutation, FFI, allocation, and spawning occur only in
        // explicit event handlers. A view-binding directive expression
        // containing a mutation/FFI construct is rejected.
        for directive in ["b-text", "b-show", "b-when", "b-class", "b-style"] {
            if let Some(expr) = self.extract_attr_value(tag, directive) {
                if let Some(impure) = Self::impure_view_expr(&expr) {
                    return Some(format!(
                        "view expression is not pure (SPEC 21.5): `{}={}` contains {} — \
                         mutation/FFI/allocation belongs in an explicit event handler",
                        directive, expr, impure
                    ));
                }
            }
        }
        None
    }

    /// Conservative purity check for a view-binding expression: rejects
    /// obvious mutation (`=`/`<-`/`~`), FFI (`frgn`/`spawn`/`Malloc#`), and
    /// the trigger-ish `@` write prefix. A plain identifier / arithmetic /
    /// field access expression passes.
    fn impure_view_expr(expr: &str) -> Option<&'static str> {
        let e = expr.trim();
        if e.contains("<-") || e.contains("~=") || e.contains("~<-") {
            return Some("a mutation/assignment");
        }
        if e.contains("Malloc#") || e.contains("spawn ") || e.contains("frgn") {
            return Some("an allocation or FFI call");
        }
        // Bare `=` (e.g. `x = y`) as an assignment — but `==`/`>=`/`<=`/`!=`
        // comparisons are fine. A single `=` not part of a comparison is an
        // assignment.
        let mut chars = e.char_indices();
        while let Some((i, c)) = chars.next() {
            if c == '=' {
                let prev = e[..i].chars().next_back();
                let next = chars.clone().next().map(|(_, c)| c);
                if !matches!(prev, Some('=') | Some('!') | Some('<') | Some('>'))
                    && !matches!(next, Some('=') | Some('>') | Some('<'))
                {
                    return Some("an assignment");
                }
            }
        }
        None
    }

    fn extract_trigger_value_from_tag(
        &self,
        tag: &str,
        prefix: &str,
    ) -> Option<(String, Vec<(String, String)>)> {
        let tag_lower = tag.to_lowercase();
        let prefix_lower = prefix.to_lowercase();

        // Find the attribute start
        let attr_start = tag_lower.find(prefix_lower.as_str())? + prefix_lower.len();
        let after_prefix = &tag[attr_start..];

        // Extract event (e.g., "click" from "b-trigger:click")
        let mut event_end = 0;
        for (i, c) in after_prefix.chars().enumerate() {
            if c == '=' || c.is_whitespace() {
                event_end = i;
                break;
            }
        }
        let event = if event_end > 0 {
            after_prefix[..event_end].to_string()
        } else {
            "click".to_string()
        };

        // Find the equals sign after event
        let eq_pos = after_prefix.find('=')?;
        let value_start = eq_pos + 1;
        let value_raw = &after_prefix[value_start..];

        // Extract the full quoted value
        let value = if value_raw.trim_start().starts_with('"') || value_raw.trim_start().starts_with('\'') {
            let rest = value_raw.trim_start();
            let quote_char = if rest.starts_with('"') { '"' } else { '\'' };
            // 2026-08-11: find_closing_quote expects the string to BEGIN with
            // the opening quote. The previous code passed the tail AFTER the
            // opening quote, so the first (and only) quote it saw was the
            // closing one — mistaken for an opening quote → None → the
            // unwrap_or(len) slice ran out of bounds (panic). Pass `rest`
            // (which includes the opening quote) and slice to the closing
            // quote inclusive.
            match find_closing_quote(rest, quote_char) {
                Some(end_quote) => rest[..end_quote + 1].to_string(),
                None => rest.to_string(),
            }
        } else {
            let end = value_raw
                .find(|c: char| c.is_whitespace() || c == '>')
                .unwrap_or(value_raw.len());
            value_raw[..end].trim().to_string()
        };

        // Now parse the extracted value using the existing logic
        let attr_for_parsing = format!("{}:{}={}", prefix.trim_end_matches(':'), event, value);
        self.extract_trigger_value(&attr_for_parsing)
    }

    fn extract_attr_value(&self, tag: &str, attr_name: &str) -> Option<String> {
        let tag_lower = tag.to_lowercase();
        let start = tag_lower.find(attr_name)? + attr_name.len();

        let remaining = &tag[start..];
        let remaining = remaining.trim_start();

        if remaining.starts_with('=') {
            let remaining = remaining[1..].trim_start();

            if remaining.starts_with('\"') {
                let end = remaining[1..].find('\"')?;
                Some(remaining[1..end + 1].to_string())
            } else if remaining.starts_with('\'') {
                let end = remaining[1..].find('\'')?;
                Some(remaining[1..end + 1].to_string())
            } else {
                let end = remaining.find(|c: char| c.is_whitespace() || c == '>')?;
                Some(remaining[..end].to_string())
            }
        } else {
            None
        }
    }

    fn extract_event_suffix(&self, tag_lower: &str, attr_name: &str) -> Option<String> {
        let attr_idx = tag_lower.find(attr_name)?;
        let after = &tag_lower[attr_idx + attr_name.len()..];

        if after.starts_with(':') {
            let end = after[1..].find(|c: char| !c.is_alphanumeric() && c != '_')?;
            Some(after[1..end + 1].to_string())
        } else {
            None
        }
    }

    fn generate_element_id(&mut self, tag: &str) -> String {
        if let Some(id_pos) = tag.to_lowercase().find("id=") {
            let after = &tag[id_pos + 3..];
            let trimmed = after
                .trim_start_matches('=')
                .trim_start_matches('\"')
                .trim_start_matches('\'');
            let end = trimmed
                .find(|c: char| c.is_whitespace() || c == '\"' || c == '\'' || c == '>')
                .unwrap_or(trimmed.len());
            return trimmed[..end].to_string();
        }

        let tag_name = tag.split_whitespace().next().unwrap_or("elem").to_string();
        let id = format!("rbv-{}-{}", tag_name.replace("<", ""), self.id_counter);
        self.id_counter += 1;
        id
    }

    fn parse_class_expr(&self, expr: &str) -> Vec<(String, String)> {
        let mut pairs = Vec::new();

        for part in expr.split(',') {
            let part = part.trim();
            if let Some(colon_pos) = part.find(':') {
                let signal = part[..colon_pos].trim().to_string();
                let class = part[colon_pos + 1..]
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string();
                pairs.push((signal, class));
            }
        }

        pairs
    }

    fn parse_attr_expr(&self, expr: &str) -> Option<(String, String)> {
        if let Some(colon_pos) = expr.find(':') {
            let name = expr[..colon_pos].trim().to_string();
            let value = expr[colon_pos + 1..]
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
            Some((name, value))
        } else {
            None
        }
    }

    fn extract_each_value(&self, attr: &str) -> Option<(String, String)> {
        let after_prefix = attr.strip_prefix("b-each:")?;
        let (item_name, after_item) = after_item_name(after_prefix)?;
        if !after_item.starts_with('=') {
            return None;
        }
        let after_eq = &after_item[1..].trim();
        let mut iterable = after_eq.trim_matches('"').trim_matches('\'').to_string();
        if iterable.ends_with('>') {
            iterable.pop();
            if let Some(c) = iterable.chars().last() {
                if c == '"' || c == '\'' {
                    iterable.pop();
                }
            }
        }
        Some((item_name.to_string(), iterable))
    }
}

fn after_item_name(s: &str) -> Option<(&str, &str)> {
    let end = s.find(|c: char| !c.is_alphanumeric() && c != '_')?;
    Some((&s[..end], &s[end..]))
}

impl Default for ViewCompiler {
    fn default() -> Self {
        Self::new()
    }
}

fn find_closing_paren(s: &str) -> Option<usize> {
    let mut depth = 0;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    for (i, c) in s.chars().enumerate() {
        if !in_double_quote && c == '\'' && !in_single_quote {
            in_single_quote = true;
        } else if !in_double_quote && c == '\'' && in_single_quote {
            in_single_quote = false;
        } else if !in_single_quote && c == '"' && !in_double_quote {
            in_double_quote = true;
        } else if !in_single_quote && c == '"' && in_double_quote {
            in_double_quote = false;
        } else if !in_single_quote && !in_double_quote {
            if c == '(' {
                depth += 1;
            } else if c == ')' {
                if depth == 0 {
                    return Some(i);
                }
                depth -= 1;
            }
        }
        // ignore ( and ) inside strings
    }
    None
}

fn find_closing_quote(s: &str, quote_char: char) -> Option<usize> {
    let chars: Vec<char> = s.chars().collect();
    let mut first_quote_pos = None;
    for (i, c) in chars.iter().enumerate() {
        if *c == quote_char {
            if first_quote_pos.is_none() {
                first_quote_pos = Some(i);
            } else {
                // This is a candidate closing quote
                if let Some(next_char) = chars.get(i + 1).copied() {
                    if next_char.is_whitespace() || next_char == '>' || next_char == '/' {
                        // This quote terminates the attribute value
                        return Some(i);
                    }
                    // Not a terminator, continue looking for the next quote
                } else {
                    return Some(i);
                }
            }
        }
    }
    None
}

/// Split a view signal into its root field name and any `.^X` reflection
/// projection suffixes. `count` → (`count`, []); `items.^Size` →
/// (`items`, ["Size"]); `a.^Size.^Len` → (`a`, ["Size", "Len"]).
///
/// 2026-08-11: single definition — reused by the web generator's
/// `field_handle_for_signal` (handle lookup binds the root field), by
/// `verify_srbv` (SRBV checks the root signal exists), and by the brivc
/// frontend (view-bound fields protect %State slots from dead-field
/// elimination). The `.^X` suffix is a projection on top of the field's
/// value, never a separate signal.
pub fn root_signal<'a>(signal: &'a str) -> (&'a str, Vec<&'a str>) {
    let signal = signal.trim();
    let mut proj: Vec<&str> = Vec::new();
    let mut head = signal;
    loop {
        if let Some(dot) = head.rfind(".^") {
            let suffix = &head[dot + 2..];
            if !suffix.is_empty()
                && suffix.chars().all(|c| c.is_ascii_alphabetic() || c == '_')
            {
                proj.push(suffix);
                head = &head[..dot];
            } else {
                break;
            }
        } else {
            break;
        }
    }
    proj.reverse();
    (head, proj)
}

/// Verify the `.s` strict profile (SPEC §3.2, `ui.s.rbv`) view-state
/// isomorphism: every signal/transaction referenced in the view bindings must
/// exist and carry non-trivial contracts (not `[true]` on both sides).
/// Returns list of verification errors.
pub fn verify_srbv(
    bindings: &[Binding],
    program: &[ast::TopLevel],
) -> Vec<String> {
    let mut errors = Vec::new();

    // Build lookup maps from the program
    let mut state_vars: HashSet<String> = HashSet::new();
    let mut txn_contracts: HashMap<String, &Contract> = HashMap::new();

    for item in program {
        match item {
            TopLevel::StateDecl(state) => {
                state_vars.insert(state.name.clone());
            }
            // 2026-08-11: a top-level `let name: Type = ...;` is the standard
            // `.s.rbv` state shape (parsed as a Statement::Let, not a
            // StateDecl) — without this, `b-text="count"` on a `let count`
            // fails SRBV001.
            TopLevel::Statement(stmt) => {
                if let ast::Statement::Let { name, names, .. } = stmt.as_ref() {
                    state_vars.insert(name.clone());
                    state_vars.extend(names.iter().cloned());
                }
            }
            TopLevel::Transaction(txn) => {
                txn_contracts.insert(txn.name.clone(), &txn.contract);
            }
            TopLevel::Definition(defn) => {
                txn_contracts.insert(defn.name.clone(), &defn.contract);
            }
            _ => {}
        }
    }

    for binding in bindings {
        match &binding.directive {
            Directive::Text { signal } => {
                // 2026-08-11: `items.^Size` binds the root field `items` —
                // the reflection suffix is a projection, not a separate signal.
                let (root, _) = root_signal(signal);
                if !state_vars.contains(root) && !txn_contracts.contains_key(root) {
                    errors.push(format!(
                        "error[SRBV001]: view references undefined signal '{}' in b-text",
                        signal
                    ));
                }
                if let Some(contract) = txn_contracts.get(root) {
                    if matches!(&contract.pre_condition, Expr::Bool(true))
                        && matches!(&contract.post_condition, Expr::Bool(true))
                    {
                        errors.push(format!(
                            "error[SRBV002]: view references '{}' which has trivial [true][true] contract",
                            signal
                        ));
                    }
                }
            }
            Directive::Show { expr } | Directive::Hide { expr } => {
                // Check that referenced variables in the expression exist
                let var_name = expr.trim();
                if !state_vars.contains(var_name) && !txn_contracts.contains_key(var_name) {
                    // Could be a compound expression - just warn
                    errors.push(format!(
                        "error[SRBV003]: view expression '{}' references undefined variable",
                        expr
                    ));
                }
            }
            Directive::Trigger { txn, .. } => {
                if !txn_contracts.contains_key(txn) {
                    errors.push(format!(
                        "error[SRBV004]: view references undefined transaction '{}' in trigger",
                        txn
                    ));
                } else if let Some(contract) = txn_contracts.get(txn) {
                    if matches!(&contract.pre_condition, Expr::Bool(true))
                        && matches!(&contract.post_condition, Expr::Bool(true))
                    {
                        errors.push(format!(
                            "error[SRBV005]: triggered transaction '{}' has trivial [true][true] contract",
                            txn
                        ));
                    }
                }
            }
            Directive::Class { pairs } => {
                for (_, expr) in pairs {
                    if !state_vars.contains(expr.as_str()) && !txn_contracts.contains_key(expr.as_str()) {
                        errors.push(format!(
                            "error[SRBV006]: view class expression references undefined '{}'",
                            expr
                        ));
                    }
                }
            }
            Directive::Attr { value, .. } => {
                if !state_vars.contains(value.as_str()) && !txn_contracts.contains_key(value.as_str()) {
                    errors.push(format!(
                        "error[SRBV007]: view attribute references undefined '{}'",
                        value
                    ));
                }
            }
            Directive::Style { value, .. } => {
                if !state_vars.contains(value.as_str()) && !txn_contracts.contains_key(value.as_str()) {
                    errors.push(format!(
                        "error[SRBV008]: view style references undefined '{}'",
                        value
                    ));
                }
            }
            Directive::Each { iterable, .. } => {
                if !state_vars.contains(iterable.as_str()) && !txn_contracts.contains_key(iterable.as_str()) {
                    errors.push(format!(
                        "error[SRBV009]: view b-each references undefined '{}'",
                        iterable
                    ));
                }
            }
        }
    }

    // Verify state-DOM correspondence: every state mutation should have a view binding
    // For now, check that at least some bindings exist for rendered state
    if !state_vars.is_empty() && bindings.is_empty() {
        errors.push(
            "error[SRBV010]: state variables declared but no view bindings found — view may not reflect state".to_string()
        );
    }

    // Check that user-triggered transactions have non-trivial preconditions
    for binding in bindings {
        if let Directive::Trigger { txn, .. } = &binding.directive {
            if let Some(contract) = txn_contracts.get(txn) {
                if matches!(&contract.pre_condition, Expr::Bool(true)) {
                    errors.push(format!(
                        "error[SRBV011]: user-triggered transaction '{}' has precondition [true]; should specify when it can fire",
                        txn
                    ));
                }
            }
        }
    }

    errors
}

fn strip_surrounding_quotes(s: &str) -> String {
    let trimmed = s.trim();
    if (trimmed.starts_with('\'') && trimmed.ends_with('\'')) || (trimmed.starts_with('"') && trimmed.ends_with('"')) {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_signal_and_transaction() {
        let mut vc = ViewCompiler::new();
        vc.register_signal("sig_a", 0);
        vc.register_transaction("txn_b", 1);
        assert!(vc.signals.contains_key("sig_a"));
        assert!(vc.transactions.contains_key("txn_b"));
        assert_eq!(vc.transactions.get("txn_b"), Some(&1));
    }

    #[test]
    fn test_compile_basic_html_no_directives() {
        let mut vc = ViewCompiler::new();
        let (bindings, html, _) = vc.compile("<div>hello</div>");
        assert!(bindings.is_empty(), "No directives = no bindings");
        assert!(!html.is_empty());
        // No directives so no IDs injected
        assert!(!html.contains("id="));
    }

    #[test]
    fn test_compile_b_text_directive() {
        let mut vc = ViewCompiler::new();
        let (bindings, html, _) = vc.compile(r#"<div b-text="name">text</div>"#);
        assert!(!bindings.is_empty(), "Should create text binding");
        let has_text = bindings.iter().any(|b| matches!(&b.directive, Directive::Text { .. }));
        assert!(has_text, "Should have Text directive");
    }

    #[test]
    fn test_compile_b_show_directive() {
        let mut vc = ViewCompiler::new();
        let (bindings, html, _) = vc.compile(r#"<div b-show="isVisible">text</div>"#);
        let has_show = bindings.iter().any(|b| matches!(&b.directive, Directive::Show { .. }));
        assert!(has_show, "Should have Show directive");
    }

    #[test]
    fn test_compile_b_hide_directive() {
        let mut vc = ViewCompiler::new();
        let (bindings, html, _) = vc.compile(r#"<div b-hide="isHidden">text</div>"#);
        let has_hide = bindings.iter().any(|b| matches!(&b.directive, Directive::Hide { .. }));
        assert!(has_hide, "Should have Hide directive");
    }

    #[test]
    fn test_compile_inject_ids_adds_id_attr() {
        let mut vc = ViewCompiler::new();
        let (_, html, _) = vc.compile(r#"<div b-text="x">text</div>"#);
        assert!(html.contains("id=\""), "Should inject id for directive elements");
    }

    #[test]
    fn test_compile_empty_html() {
        let mut vc = ViewCompiler::new();
        let (bindings, html, _) = vc.compile("");
        assert!(bindings.is_empty());
        assert!(html.is_empty());
    }

    // ── 2026-08-09 (Phase 14, SPEC 21.4): directive validation ────────

    #[test]
    fn b_if_is_rejected() {
        let mut vc = ViewCompiler::new();
        let (_, _, diagnostics) = vc.compile(
            r#"<div b-if="x > 0">hi</div>"#,
        );
        assert!(
            diagnostics.iter().any(|d| d.contains("`b-if` is invalid")),
            "{:?}",
            diagnostics
        );
    }

    #[test]
    fn b_each_requires_b_key() {
        let mut vc = ViewCompiler::new();
        let (_, _, diagnostics) = vc.compile(
            r#"<li b-each:item="items">x</li>"#,
        );
        assert!(
            diagnostics.iter().any(|d| d.contains("requires a stable `b-key`")),
            "{:?}",
            diagnostics
        );
    }

    #[test]
    fn b_each_with_b_key_is_valid() {
        let mut vc = ViewCompiler::new();
        let (_, _, diagnostics) = vc.compile(
            r#"<li b-each:item="items" b-key="item.id">x</li>"#,
        );
        assert!(
            !diagnostics.iter().any(|d| d.contains("`b-key`")),
            "{:?}",
            diagnostics
        );
    }

    #[test]
    fn b_bind_value_must_be_assignable_field() {
        let mut vc = ViewCompiler::new();
        let (_, _, diagnostics) = vc.compile(
            r#"<input b-bind:value="count + 1">"#,
        );
        assert!(
            diagnostics.iter().any(|d| d.contains("`b-bind:value` accepts only an assignable field")),
            "{:?}",
            diagnostics
        );
    }

    // ── 2026-08-09 (Phase 14, SPEC 21.5): view-expression purity ──────

    #[test]
    fn pure_view_expression_passes() {
        let mut vc = ViewCompiler::new();
        let (_, _, diagnostics) = vc.compile(
            r#"<span b-text="count + 1">0</span>"#,
        );
        assert!(
            !diagnostics.iter().any(|d| d.contains("not pure")),
            "{:?}",
            diagnostics
        );
    }

    #[test]
    fn mutation_in_view_expression_rejected() {
        let mut vc = ViewCompiler::new();
        let (_, _, diagnostics) = vc.compile(
            r#"<span b-text="count = 5">0</span>"#,
        );
        assert!(
            diagnostics.iter().any(|d| d.contains("not pure") && d.contains("assignment")),
            "{:?}",
            diagnostics
        );
    }

    #[test]
    fn ffi_in_view_expression_rejected() {
        let mut vc = ViewCompiler::new();
        let (_, _, diagnostics) = vc.compile(
            r#"<span b-text="Malloc#(64)">0</span>"#,
        );
        assert!(
            diagnostics.iter().any(|d| d.contains("not pure") && d.contains("allocation")),
            "{:?}",
            diagnostics
        );
    }

    #[test]
    fn comparison_in_view_expression_is_pure() {
        // `==`/`>=` are comparisons, not assignments — they must pass purity.
        let mut vc = ViewCompiler::new();
        let (_, _, diagnostics) = vc.compile(
            r#"<div b-show="count == 5">x</div>"#,
        );
        assert!(
            !diagnostics.iter().any(|d| d.contains("not pure")),
            "{:?}",
            diagnostics
        );
    }
}
