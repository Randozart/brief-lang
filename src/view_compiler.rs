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
        let modified_html = self.inject_ids(view_html);
        self.extract_bindings(&modified_html);
        (self.bindings.clone(), modified_html, self.diagnostics.clone())
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
                            let tag_attrs: String = tag
                                .split_whitespace()
                                .skip(1)
                                .filter(|s| !s.starts_with("b-"))
                                .collect::<Vec<_>>()
                                .join(" ");
                            let template_html = inner_html.clone();

                            let container_id =
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
            let first_quote = rest.find(quote_char)? + 1;
            let rest_after = &rest[first_quote..];
            let end_quote = find_closing_quote(rest_after, quote_char).unwrap_or(rest_after.len());
            rest[..first_quote + end_quote + 1].to_string()
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

fn strip_surrounding_quotes(s: &str) -> String {
    let trimmed = s.trim();
    if (trimmed.starts_with('\'') && trimmed.ends_with('\'')) || (trimmed.starts_with('"') && trimmed.ends_with('"')) {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}
