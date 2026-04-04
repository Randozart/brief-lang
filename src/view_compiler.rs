use crate::rbv::RbvFile;
use std::collections::HashMap;

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
    },
    Class {
        pairs: Vec<(String, String)>,
    },
    Attr {
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
}

impl ViewCompiler {
    pub fn new() -> Self {
        ViewCompiler {
            signals: HashMap::new(),
            transactions: HashMap::new(),
            bindings: Vec::new(),
        }
    }

    pub fn register_signal(&mut self, name: &str, id: usize) {
        self.signals.insert(name.to_string(), id);
    }

    pub fn register_transaction(&mut self, name: &str, id: usize) {
        self.transactions.insert(name.to_string(), id);
    }

    pub fn compile(&mut self, view_html: &str) -> Vec<Binding> {
        self.bindings.clear();
        self.extract_bindings(view_html, 0);
        self.bindings.clone()
    }

    fn extract_bindings(&mut self, html: &str, depth: usize) {
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
                    let full_tag_start = pos;
                    let tag_str = String::from_utf8_lossy(&bytes[pos..pos + end_pos]).to_string();

                    let tag_lower = tag_str.to_lowercase();
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
                            let opening_tag = if tag_attrs.is_empty() {
                                format!("<{}>", elem_name)
                            } else {
                                format!("<{} {}>", elem_name, tag_attrs)
                            };
                            let template_html =
                                format!("{} {}</{}>", opening_tag, inner_html, elem_name);
                            let container_id = format!("{}-container", iterable);
                            self.bindings.push(Binding {
                                element_id: elem_id,
                                directive: Directive::Each {
                                    iterable: iterable,
                                    item_name: item_name,
                                    template_html: template_html,
                                    container_id: container_id,
                                },
                            });
                            let total_len = end_pos + inner_html.len() + elem_name.len() + 3;
                            pos += total_len;
                            continue;
                        }
                    }

                    self.extract_directives(&tag_str);
                    pos += end_pos;
                    continue;
                }
            }
            pos += 1;
        }
    }

    fn find_each_inner_html(&self, html: &str, tag: &str) -> String {
        let elem_name = tag.split_whitespace().next().unwrap_or(tag).trim();
        let closing_pattern = format!("</{}>", elem_name);
        if let Some(closing_pos) = html.find(&closing_pattern) {
            if let Some(open_end) = html.find('>') {
                if open_end < closing_pos {
                    let inner = html[open_end + 1..closing_pos].trim();
                    return self.strip_directives_from_html(inner);
                }
            }
        }
        String::new()
    }

    fn strip_directives_from_html(&self, html: &str) -> String {
        let mut result = String::new();
        let chars: Vec<char> = html.chars().collect();
        let mut pos = 0;

        while pos < chars.len() {
            if chars[pos] == '<' {
                if chars
                    .get(pos + 1)
                    .map(|&c| c.is_ascii_alphabetic() || c == '!')
                    .unwrap_or(false)
                {
                    if let Some((tag_str, tag_len)) = self.parse_tag(&html[pos..]) {
                        let stripped_tag = self.strip_directives_from_tag(&tag_str);
                        result.push_str(&format!("<{}>", stripped_tag));
                        pos += tag_len;
                        continue;
                    }
                }
                result.push(chars[pos]);
                pos += 1;
            } else {
                result.push(chars[pos]);
                pos += 1;
            }
        }
        result
    }

    fn strip_directives_from_tag(&self, tag: &str) -> String {
        let parts: Vec<String> = tag
            .split_whitespace()
            .filter(|s| !s.starts_with("b-") && !s.starts_with("B-"))
            .map(|s| s.to_string())
            .collect();
        parts.join(" ")
    }

    fn parse_tag<'a>(&self, s: &'a str) -> Option<(String, usize)> {
        if !s.starts_with('<') {
            return None;
        }

        let end = s.find('>')?;
        let tag = &s[1..end];
        Some((tag.to_string(), end + 1))
    }

    fn extract_directives(&mut self, tag: &str) {
        let tag_lower = tag.to_lowercase();

        for attr in tag_lower.split_whitespace().skip(1) {
            let attr = attr.trim_end_matches('>').trim_end_matches('/');

            if attr.starts_with("b-text") {
                if let Some(expr) = self.extract_attr_value(tag, "b-text") {
                    let elem_id = self.generate_element_id(tag);
                    self.bindings.push(Binding {
                        element_id: elem_id,
                        directive: Directive::Text { signal: expr },
                    });
                }
            } else if attr.starts_with("b-show") {
                if let Some(expr) = self.extract_attr_value(tag, "b-show") {
                    let elem_id = self.generate_element_id(tag);
                    self.bindings.push(Binding {
                        element_id: elem_id,
                        directive: Directive::Show { expr },
                    });
                }
            } else if attr.starts_with("b-hide") {
                if let Some(expr) = self.extract_attr_value(tag, "b-hide") {
                    let elem_id = self.generate_element_id(tag);
                    self.bindings.push(Binding {
                        element_id: elem_id,
                        directive: Directive::Hide { expr },
                    });
                }
            } else if attr.starts_with("b-trigger:") {
                let txn = self.extract_trigger_value(attr);
                let elem_id = self.generate_element_id(tag);
                let event = self.extract_event_suffix(&tag_lower, "b-trigger");
                if let Some(txn_name) = txn {
                    self.bindings.push(Binding {
                        element_id: elem_id,
                        directive: Directive::Trigger {
                            event: event.unwrap_or_else(|| "click".to_string()),
                            txn: txn_name,
                        },
                    });
                }
            } else if attr.starts_with("b-class") {
                if let Some(expr) = self.extract_attr_value(tag, "b-class") {
                    let elem_id = self.generate_element_id(tag);
                    let pairs = self.parse_class_expr(&expr);
                    self.bindings.push(Binding {
                        element_id: elem_id,
                        directive: Directive::Class { pairs },
                    });
                }
            } else if attr.starts_with("b-attr") {
                if let Some(expr) = self.extract_attr_value(tag, "b-attr") {
                    let elem_id = self.generate_element_id(tag);
                    if let Some((name, value)) = self.parse_attr_expr(&expr) {
                        self.bindings.push(Binding {
                            element_id: elem_id,
                            directive: Directive::Attr { name, value },
                        });
                    }
                }
            }
        }
    }

    fn extract_trigger_value(&self, attr: &str) -> Option<String> {
        let after_colon = attr.strip_prefix("b-trigger:")?;
        let after_event = after_colon.find('=')?;
        let value_part = &after_colon[after_event + 1..];

        let value = value_part.trim();
        if value.starts_with('"') {
            let end = value[1..].find('"')?;
            Some(value[1..end + 1].to_string())
        } else if value.starts_with('\'') {
            let end = value[1..].find('\'')?;
            Some(value[1..end + 1].to_string())
        } else {
            let end = value
                .find(|c: char| c.is_whitespace() || c == '>')
                .unwrap_or(value.len());
            Some(value[..end].to_string())
        }
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

    fn generate_element_id(&self, tag: &str) -> String {
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
        let id = format!("rbv-{}", tag_name.replace("<", ""));
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
        let iterable = after_eq.trim_matches('"').trim_matches('\'').to_string();
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
