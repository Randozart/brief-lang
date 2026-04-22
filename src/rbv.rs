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
    #[error("Missing <script> block")]
    MissingScript,
    #[error("Missing <view> block")]
    MissingView,
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
pub struct RbvFile {
    pub brief_source: String,
    pub view_html: String,
    pub style_css: Option<String>,
}

impl RbvFile {
    pub fn parse(source: &str) -> Result<Self, RbvError> {
        let script = extract_tag(source, "<script>", "</script>")
            .or_else(|| extract_tag(source, "<script type=\"brief\">", "</script>"))
            .ok_or(RbvError::MissingScript)?;

        let view = extract_tag(source, "<view>", "</view>").ok_or(RbvError::MissingView)?;

        let style = extract_tag(source, "<style>", "</style>");

        Ok(RbvFile {
            brief_source: script.trim().to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rbv() {
        let source = r#"
<script type="brief">
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
        assert!(rbv.brief_source.contains("count"));
        assert!(rbv.view_html.contains("b-text"));
        assert!(rbv.style_css.is_some());
    }
}
