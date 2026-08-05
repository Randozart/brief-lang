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

use thiserror::Error;

#[derive(Error, Debug)]
pub enum RbvError {
    #[error("Missing <view> block")]
    MissingView,
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
pub struct RbvFile {
    pub briv_source: String,
    pub view_html: String,
    pub style_css: Option<String>,
}

impl RbvFile {
    /// Parse an `.rbv` file.
    ///
    /// Briv code is the default content — everything outside `<view>` and
    /// `<style>` tags is treated as Briv source.
    ///
    /// Backward compatible: if `<script>` or `<script type="briv">` tags are
    /// present, their content is used instead (old format).
    pub fn parse(source: &str) -> Result<Self, RbvError> {
        let view = extract_tag(source, "<view>", "</view>").ok_or(RbvError::MissingView)?;

        let style = extract_tag(source, "<style>", "</style>");

        let briv_source = if let Some(script) = extract_script_tags(source) {
            script.trim().to_string()
        } else {
            let stripped = strip_known_blocks(source);
            stripped.trim().to_string()
        };

        Ok(RbvFile {
            briv_source,
            view_html: view.trim().to_string(),
            style_css: style.map(|s| s.trim().to_string()),
        })
    }
}

fn extract_tag(source: &str, start_tag: &str, end_tag: &str) -> Option<String> {
    let start = source.find(start_tag)? + start_tag.len();
    let end = source.find(end_tag)?;
    Some(source[start..end].to_string())
}

fn extract_script_tags(source: &str) -> Option<String> {
    extract_tag(source, "<script type=\"briv\">", "</script>")
        .or_else(|| extract_tag(source, "<script>", "</script>"))
}

fn strip_known_blocks(source: &str) -> String {
    let known_blocks: [(&str, &str); 4] = [
        ("<view>", "</view>"),
        ("<style>", "</style>"),
        ("<script>", "</script>"),
        ("<script type=\"briv\">", "</script>"),
    ];
    let mut result = source.to_string();
    for &(start_tag, end_tag) in &known_blocks {
        while let (Some(start), Some(end)) = (result.find(start_tag), result.find(end_tag)) {
            let block_end = end + end_tag.len();
            result.drain(start..block_end);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rbv_script_backward_compat() {
        let source = r#"
<script type="briv">
let count: Int = 0;
</script>

<view>
<p b-text="count">0</p>
</view>

<style>
p { color: red; }
</style>
"#;
        let rbv = RbvFile::parse(source).unwrap();
        assert!(rbv.briv_source.contains("count"));
        assert!(rbv.view_html.contains("b-text"));
        assert!(rbv.style_css.is_some());
    }

    #[test]
    fn test_parse_rbv_briv_as_default() {
        let source = r#"
let count: Int = 0;

txn increment [true][@count + 1 == count] {
    &count = count + 1;
    term;
};

<view>
<p b-text="count">0</p>
<button b-trigger:click="increment">+</button>
</view>
"#;
        let rbv = RbvFile::parse(source).unwrap();
        assert!(rbv.briv_source.contains("count"));
        assert!(rbv.briv_source.contains("increment"));
        assert!(rbv.view_html.contains("b-text"));
        assert!(rbv.style_css.is_none());
    }

    #[test]
    fn test_parse_rbv_no_script_style_is_extracted() {
        // Briv code interleaved with view — everything outside <view> is source
        let source = r#"
let x: Int = 42;

<view>
<span b-text="x">42</span>
</view>

txn double [true][@x * 2 == x] {
    &x = x * 2;
    term;
};
"#;
        let rbv = RbvFile::parse(source).unwrap();
        assert!(rbv.briv_source.contains("let x: Int = 42"));
        assert!(rbv.briv_source.contains("txn double"));
        assert!(rbv.view_html.contains("b-text"));
    }
}
