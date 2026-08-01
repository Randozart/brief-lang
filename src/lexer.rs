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

// 2026-07-12: Phase 0.1 — Full rewrite for new architecture.
// Changes from old lexer:
// - # is now a valid identifier character (Sqrt# -> single token)
// - Standalone # token removed — [ # ] uses Identifier("#")
// - inop/inop! removed (intrinsic architecture: all ops are # intrinsics)
// - Added When, Input, Output tokens
// - Export, ColonEq already existed (kept as-is)

use logos::Logos;

#[derive(Logos, Debug, PartialEq, Clone)]
#[logos(skip r"[ \t\n\r]+")]
#[logos(skip r"//[^\n]*")]
#[logos(skip r"/\*[^*]*\*+(?:[^/*][^*]*\*+)*/")]
pub enum Token {
    // ── Doc comments (defined before //-skip to win ties) ─────────────
    #[regex(r"///[^\n]*", |lex| lex.slice()[3..].to_string())]
    DocComment(String),
    #[regex(r"//![^\n]*", |lex| lex.slice()[3..].to_string())]
    DocCommentBang(String),

    // ── Keywords ──────────────────────────────────────────────────────
    #[token("sig")]
    Sig,

    /// Phase 15: export defn — replaces #export pragma
    #[token("export")]
    Export,

    #[token("defn")]
    Defn,

    #[token("let")]
    Let,

    #[token("const")]
    Const,

    #[token("txn")]
    Txn,

    #[token("node")]
    Node,

    #[token("async")]
    Async,

    /// 2026-08-01 (E): `seq` — ordering/layout/sequence modifier (prefix).
    /// `seq struct` bypasses apply_field_modes; `seq node`/`seq txn` use
    /// sequential dispatch; `seq Int[x]`/`seq foreach` disable vectorization.
    #[token("seq")]
    Seq,

    /// 2026-08-01 (E): `vol` — memory-visibility modifier (prefix).
    /// `vol let x` emits volatile load/store.
    #[token("vol")]
    Vol,

    #[token("await")]
    Await,

    #[token("term")]
    Term,

    #[token("term!")]
    TermBang,

    #[token("escape")]
    Escape,

    #[token("uni")]
    Uni,

    #[token("is")]
    Is,

    #[token("like")]
    Like,

    #[token("import")]
    Import,

    #[token("from")]
    From,

    #[token("as")]
    As,

    #[token("frgn")]
    Frgn,

    #[token("frgn!")]
    FrgnBang,

    // 2026-07-12: inop/inop! removed — all ops are # intrinsics
    #[token("meld")]
    Meld,

    #[token("syscall!")]

    #[token("reg")]
    Reg,

    #[token("op")]
    Op,

    #[token("prop")]
    Prop,

    #[token("type")]
    Type,

    #[token("cell")]
    Cell,

    #[token("obj")]
    Obj,
    #[token("struct")]
    Struct,

    #[token("rstruct")]
    Rstruct,

    #[token("render")]
    Render,

    #[token("enum")]
    Enum,

    #[token("trg")]
    Trg,





    #[token("within")]
    Within,


    #[token("Ptr!")]
    PtrBang,

    #[token("Ok")]
    Ok,

    #[token("Err")]
    Err,

    #[token("match")]
    Match,

    // 2026-07-15: template/macro removed — replaced by $(Stage) blocks

    #[token("quote")]
    Quote,

    #[token("$")]
    Dollar,

    #[token("$!")]
    DollarBang,

    #[token("foreach")]
    Foreach,

    #[token("pvt")]
    Pvt,

    #[token("sed")]
    Sed,

    #[token("sync")]
    Sync,

    #[token("some")]
    Some,

    #[token("none")]
    None,

    #[token("true")]
    BoolTrue,

    #[token("false")]
    BoolFalse,

    #[token("cycles")]
    Cycles,

    #[token("cyc")]
    Cyc,

    #[token("ms")]
    Ms,

    #[token("sec")]
    #[token("seconds")]
    Seconds,

    #[token("minute")]
    Minute,

    #[token("minutes")]
    Minutes,

    #[token("nanoseconds")]
    Nanoseconds,

    // ── Phase 16A/16D: Cell file keywords ─────────────────────
    /// 2026-07-12: .c.bv cell parameter declaration
    #[token("input")]
    Input,

    /// 2026-07-12: .c.bv cell output declaration
    #[token("output")]
    Output,

    // ── Phase 8.6: Guard/derivation keywords ─────────────────
    /// 2026-07-12: Guard statement keyword
    #[token("when")]
    When,

    // ── Operators ─────────────────────────────────────────────
    #[token("=>")]
    FatArrow,

    #[token("=")]
    Eq,

    #[token("&")]
    Ampersand,

    #[token("@")]
    At,

    #[token("==")]
    EqEq,

    #[token("!=")]
    Ne,

    #[token("<")]
    Lt,

    #[token("</")]
    LtSlash,

    #[token("<=")]
    Le,

    #[token(">")]
    Gt,

    #[token(">=")]
    Ge,

    #[token("<<")]
    Shl,

    #[token(">>")]
    Shr,

    #[token("|>")]
    PipeGreater,

    #[token("|")]
    Pipe,

    #[token("||")]
    OrOr,

    #[token("&&")]
    AndAnd,

    #[token("!")]
    Not,

    #[token("?")]
    Question,

    #[token("-=")]
    MinusEq,
    #[token("-")]
    Minus,

    #[token("~?")]
    TildeQuestion,

    #[token("~/")]
    TildeSlash,

    #[token("~")]
    Tilde,

    #[token("++")]
    PlusPlus,

    #[token("+=")]
    PlusEq,

    #[token("+")]
    Plus,

    #[token("*=")]
    StarEq,
    #[token("*")]
    Star,

    #[token("/=")]
    SlashEq,
    #[token("/")]
    Slash,

    #[token("^")]
    BitXor,

    #[token("->")]
    Arrow,

    #[token("<-")]
    ArrowLeft,

/// !> — metadata assignment operator
#[token("!>")]
ExclaimArrow,

    #[token("_")]
    Underscore,

    // ── Pragma tokens (kept for backward compat, to migrate) ──
    #[token("#[")]
    HashBracket,

    #[token("#![")]
    HashBangBracket,

    #[token("#pragma")]
    Pragma,

    #[token("#!pragma")]
    PragmaBang,

    #[token("#?")]
    HashQuestion,

    /// 2026-07-12: #! kept for backward compat. # alone is now
    /// an identifier character (e.g. Sqrt# -> single ident).
    #[token("#!")]
    HashBang,

    // 2026-07-18: Compiler-internal hash words for strategy op bindings.
    // #L = left operand, #R = right operand, #T = type parameter.
    #[token("#L")]
    HashL,
    #[token("#R")]
    HashR,
    #[token("#T")]
    HashT,

    // 2026-07-23: #Self — self-reference hashword for protocol contracts.
    // Reserved for this use case. Matched before the identifier regex.
    #[token("#Self")]
    HashSelf,

    // ── Punctuation ───────────────────────────────────────────
    #[token(";")]
    Semicolon,

    /// `.^^` — compile-time reflection access (`x.^^Size`). Must lex before
    /// `.^` so logos longest-match handles the triple-char form; order is
    /// otherwise irrelevant (logos picks the longest match).
    #[token(".^^")]
    DotCaretCaret,

    /// `.^` — runtime reflection access (`x.^Len`, `x.^Ptr`).
    /// The caret alone remains bitwise XOR (`a ^ b`); the dot disambiguates.
    #[token(".^")]
    DotCaret,

    /// := — derivation / compile-time assertion block
    #[token(":=")]
    ColonEq,

    #[token(":")]
    Colon,

    #[token("::")]
    ColonColon,

    #[token("{")]
    LBrace,

    #[token("}")]
    RBrace,

    #[token("(")]
    LParen,

    #[token(")")]
    RParen,

    #[token("[")]
    LBracket,

    #[token("]")]
    RBracket,

    #[token(",")]
    Comma,

    #[token("%")]
    Percent,

    #[token("...")]
    Ellipsis,

    #[token("..")]
    DotDot,

    #[token(".")]
    Dot,

    // ── Literals ──────────────────────────────────────────────
    #[regex(r"0x[0-9a-fA-F]+", |lex| i64::from_str_radix(&lex.slice()[2..], 16).ok())]
    #[regex(r"[0-9]+", |lex| lex.slice().parse().ok())]
    Integer(i64),

    #[regex(r"[0-9]+i8", |lex| lex.slice().trim_end_matches("i8").parse().ok())]
    IntegerI8(i64),

    #[regex(r"[0-9]+i16", |lex| lex.slice().trim_end_matches("i16").parse().ok())]
    IntegerI16(i64),

    #[regex(r"[0-9]+i32", |lex| lex.slice().trim_end_matches("i32").parse().ok())]
    IntegerI32(i64),

    #[regex(r"[0-9]+i64", |lex| lex.slice().trim_end_matches("i64").parse().ok())]
    IntegerI64(i64),

    #[regex(r"[0-9]+u8", |lex| lex.slice().trim_end_matches("u8").parse().ok())]
    IntegerU8(i64),

    #[regex(r"[0-9]+u16", |lex| lex.slice().trim_end_matches("u16").parse().ok())]
    IntegerU16(i64),

    #[regex(r"[0-9]+u32", |lex| lex.slice().trim_end_matches("u32").parse().ok())]
    IntegerU32(i64),

    #[regex(r"[0-9]+u64", |lex| lex.slice().trim_end_matches("u64").parse().ok())]
    IntegerU64(i64),

    #[regex(r"[0-9]+\.[0-9]+", |lex| lex.slice().parse().ok())]
    Float(f64),

    #[regex(r"[0-9]+\.[0-9]+f32", |lex| lex.slice().trim_end_matches("f32").parse().ok())]
    Float32(f64),

    #[regex(r"[0-9]+\.[0-9]+f64", |lex| lex.slice().trim_end_matches("f64").parse().ok())]
    Float64(f64),

    #[regex(r#""([^"\\]|\\.)*""#, |lex| {
        let s = lex.slice();
        let inner = &s[1..s.len()-1];
        let mut out = String::with_capacity(inner.len());
        let mut chars = inner.chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some('\\') => out.push('\\'),
                    Some('"') => out.push('"'),
                    Some('0') => out.push('\0'),
                    Some('x') => {
                        let hex_str: String = chars.by_ref().take(2).collect();
                        if let Ok(h) = u8::from_str_radix(&hex_str, 16) {
                            out.push(h as char);
                        }
                    }
                    Some('u') => {
                        if chars.next() == Some('{') {
                            let mut hex = String::new();
                            while let Some(h) = chars.next() {
                                if h == '}' { break; }
                                hex.push(h);
                            }
                            if let Ok(cp) = u32::from_str_radix(&hex, 16) {
                                out.push(char::from_u32(cp).unwrap_or('?'));
        }
    }
                    }
                    Some(c) => { out.push('\\'); out.push(c); }
                    None => out.push('\\'),
                }
            } else {
                out.push(c);
            }
        }
        Some(out)
    })]
    String(String),

    #[regex(r"'([^'\\]|\\.)*'", |lex| {
        let s = lex.slice();
        let inner = &s[1..s.len()-1];
        if inner.is_empty() {
            return Some(' ');
        }
        if inner.len() == 1 {
            return Some(inner.chars().next().unwrap());
        }
        if inner == "\\0" { return Some('\0'); }
        if inner == "\\n" { return Some('\n'); }
        if inner == "\\t" { return Some('\t'); }
        if inner == "\\\\" { return Some('\\'); }
        if inner == "\\'" { return Some('\''); }
        if inner.len() == 4 && inner.starts_with("\\x") {
            if let Ok(h) = u8::from_str_radix(&inner[2..], 16) {
                return Some(h as char);
            }
        }
        if inner.starts_with("\\u{") && inner.ends_with('}') {
            if let Ok(cp) = u32::from_str_radix(&inner[3..inner.len()-1], 16) {
                return Some(char::from_u32(cp).unwrap_or('?'));
            }
        }
        Some(inner.chars().next().unwrap_or(' '))
    })]
    Char(char),

    // ── Identifiers (including PascalCase# intrinsics) ────────
    // 2026-07-12: # is a valid identifier character.
    // This allows Sqrt#, AddI64#, PrintInt# as single tokens.
    // 2026-07-15: $ is also a valid identifier character.
    // This allows InsertRegistryImport$ as a single token.
    // $(Front) parses as Dollar LParen Identifier("Front") RParen
    // because $ standalone (with non-identifier char after) matches
    // the exact #[token("$")] before the identifier regex.
    // Specific multi-char hash tokens (#[, #![, #pragma, etc.)
    // are matched BEFORE this regex due to logos priority rules.
    #[regex(r"[a-zA-Z_#$][a-zA-Z0-9_#$]*", |lex| lex.slice().to_string())]
    Identifier(String),
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Sig => write!(f, "sig"),
            Token::Export => write!(f, "export"),
            Token::Defn => write!(f, "defn"),
            Token::Let => write!(f, "let"),
            Token::Const => write!(f, "const"),
            Token::Txn => write!(f, "txn"),
            Token::Node => write!(f, "node"),
            Token::Async => write!(f, "async"),
            Token::Seq => write!(f, "seq"),
            Token::Vol => write!(f, "vol"),
            Token::Await => write!(f, "await"),
            Token::Term => write!(f, "term"),
            Token::TermBang => write!(f, "term!"),
            Token::Escape => write!(f, "escape"),
            Token::Uni => write!(f, "uni"),
            Token::Is => write!(f, "is"),
            Token::Like => write!(f, "like"),
            Token::Import => write!(f, "import"),
            Token::From => write!(f, "from"),
            Token::As => write!(f, "as"),
            Token::Frgn => write!(f, "frgn"),
            Token::FrgnBang => write!(f, "frgn!"),
            Token::Meld => write!(f, "meld"),
            Token::Reg => write!(f, "reg"),
            Token::Op => write!(f, "op"),
            Token::Prop => write!(f, "prop"),
            Token::Type => write!(f, "type"),
            Token::Cell => write!(f, "cell"),
            Token::Obj => write!(f, "obj"),
            Token::Struct => write!(f, "struct"),
            Token::Rstruct => write!(f, "rstruct"),
            Token::Render => write!(f, "render"),
            Token::Enum => write!(f, "enum"),
            Token::Trg => write!(f, "trg"),
            Token::Within => write!(f, "within"),
            Token::PtrBang => write!(f, "Ptr!"),
            Token::Ok => write!(f, "Ok"),
            Token::Err => write!(f, "Err"),
            Token::Match => write!(f, "match"),
            // 2026-07-15: Template/Macro tokens removed

            Token::Quote => write!(f, "quote"),
            Token::Dollar => write!(f, "$"),
            Token::DollarBang => write!(f, "$!"),
            Token::Foreach => write!(f, "foreach"),
            Token::Pvt => write!(f, "pvt"),
            Token::Sed => write!(f, "sed"),
            Token::Sync => write!(f, "sync"),
            Token::Some => write!(f, "some"),
            Token::None => write!(f, "none"),
            Token::BoolTrue => write!(f, "true"),
            Token::BoolFalse => write!(f, "false"),
            Token::Cycles => write!(f, "cycles"),
            Token::Cyc => write!(f, "cyc"),
            Token::Ms => write!(f, "ms"),
            Token::Seconds => write!(f, "seconds"),
            Token::Minute => write!(f, "minute"),
            Token::Minutes => write!(f, "minutes"),
            Token::Nanoseconds => write!(f, "nanoseconds"),
            Token::Input => write!(f, "input"),
            Token::Output => write!(f, "output"),
            Token::When => write!(f, "when"),
            Token::FatArrow => write!(f, "=>"),
            Token::Eq => write!(f, "="),
            Token::EqEq => write!(f, "=="),
            Token::Ne => write!(f, "!="),
            Token::Lt => write!(f, "<"),
            Token::LtSlash => write!(f, "</"),
            Token::Le => write!(f, "<="),
            Token::Gt => write!(f, ">"),
            Token::Ge => write!(f, ">="),
            Token::Shl => write!(f, "<<"),
            Token::Shr => write!(f, ">>"),
            Token::PipeGreater => write!(f, "|>"),
            Token::Pipe => write!(f, "|"),
            Token::OrOr => write!(f, "||"),
            Token::AndAnd => write!(f, "&&"),
            Token::Not => write!(f, "!"),
            Token::Question => write!(f, "?"),
            Token::Minus => write!(f, "-"),
            Token::MinusEq => write!(f, "-="),
            Token::TildeQuestion => write!(f, "~?"),
            Token::TildeSlash => write!(f, "~/"),
            Token::Tilde => write!(f, "~"),
            Token::PlusPlus => write!(f, "++"),
            Token::PlusEq => write!(f, "+="),
            Token::Plus => write!(f, "+"),
            Token::StarEq => write!(f, "*="),
            Token::Star => write!(f, "*"),
            Token::SlashEq => write!(f, "/="),
            Token::BitXor => write!(f, "^"),
            Token::Arrow => write!(f, "->"),
            Token::ArrowLeft => write!(f, "<-"),
            Token::ExclaimArrow => write!(f, "!>"),
            Token::Underscore => write!(f, "_"),
            Token::HashBracket => write!(f, "#["),
            Token::HashBangBracket => write!(f, "#!["),
            Token::Pragma => write!(f, "#pragma"),
            Token::Semicolon => write!(f, ";"),
            Token::DotCaretCaret => write!(f, ".^^"),
            Token::DotCaret => write!(f, ".^"),
            Token::ColonEq => write!(f, ":="),
            Token::Colon => write!(f, ":"),
            Token::ColonColon => write!(f, "::"),
            Token::LBrace => write!(f, "{{"),
            Token::RBrace => write!(f, "}}"),
            Token::LParen => write!(f, "("),
            Token::RParen => write!(f, ")"),
            Token::LBracket => write!(f, "["),
            Token::RBracket => write!(f, "]"),
            Token::Comma => write!(f, ","),
            Token::Percent => write!(f, "%"),
            Token::Ellipsis => write!(f, "..."),
            Token::DotDot => write!(f, ".."),
            Token::Dot => write!(f, "."),
            Token::Ampersand => write!(f, "&"),
            Token::At => write!(f, "@"),
            Token::Integer(n) => write!(f, "{}", n),
            Token::IntegerI8(n) => write!(f, "{}i8", n),
            Token::IntegerI16(n) => write!(f, "{}i16", n),
            Token::IntegerI32(n) => write!(f, "{}i32", n),
            Token::IntegerI64(n) => write!(f, "{}i64", n),
            Token::IntegerU8(n) => write!(f, "{}u8", n),
            Token::IntegerU16(n) => write!(f, "{}u16", n),
            Token::IntegerU32(n) => write!(f, "{}u32", n),
            Token::IntegerU64(n) => write!(f, "{}u64", n),
            Token::Float(n) => write!(f, "{}", n),
            Token::Float32(n) => write!(f, "{}f32", n),
            Token::Float64(n) => write!(f, "{}f64", n),
            Token::String(s) => write!(f, "\"{}\"", s),
            Token::Char(c) => write!(f, "'{}'", c),
            Token::Identifier(s) => write!(f, "{}", s),
            Token::DocComment(s) => write!(f, "///{}", s),
            Token::DocCommentBang(s) => write!(f, "//!{}", s),
            Token::Slash => write!(f, "/"),
            Token::HashBang => write!(f, "#!"),
            Token::HashQuestion => write!(f, "#?"),
            Token::HashL => write!(f, "#L"),
            Token::HashR => write!(f, "#R"),
            Token::HashT => write!(f, "#T"),
            Token::HashSelf => write!(f, "#Self"),
            Token::PragmaBang => write!(f, "#pragma!"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer() {
        let mut lexer = Token::lexer("sig fetch: Int -> Int;");
        assert_eq!(lexer.next(), Some(Ok(Token::Sig)));
        assert_eq!(
            lexer.next(),
            Some(Ok(Token::Identifier("fetch".to_string())))
        );
        assert_eq!(lexer.next(), Some(Ok(Token::Colon)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("Int".to_string()))));
        assert_eq!(lexer.next(), Some(Ok(Token::Arrow)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("Int".to_string()))));
        assert_eq!(lexer.next(), Some(Ok(Token::Semicolon)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_char_literals() {
        let mut lexer = Token::lexer("'a'");
        assert_eq!(lexer.next(), Some(Ok(Token::Char('a'))));

        let mut lexer = Token::lexer("'\\n'");
        assert_eq!(lexer.next(), Some(Ok(Token::Char('\n'))));

        let mut lexer = Token::lexer("'\\t'");
        assert_eq!(lexer.next(), Some(Ok(Token::Char('\t'))));

        let mut lexer = Token::lexer("'\\\\'");
        assert_eq!(lexer.next(), Some(Ok(Token::Char('\\'))));

        let mut lexer = Token::lexer("'\\''");
        assert_eq!(lexer.next(), Some(Ok(Token::Char('\''))));

        let mut lexer = Token::lexer("'\\u{1F600}'");
        assert_eq!(lexer.next(), Some(Ok(Token::Char('😀'))));

        let mut lexer = Token::lexer("let c: Char = 'x';");
        assert_eq!(lexer.next(), Some(Ok(Token::Let)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("c".to_string()))));
        assert_eq!(lexer.next(), Some(Ok(Token::Colon)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("Char".to_string()))));
        assert_eq!(lexer.next(), Some(Ok(Token::Eq)));
        assert_eq!(lexer.next(), Some(Ok(Token::Char('x'))));
        assert_eq!(lexer.next(), Some(Ok(Token::Semicolon)));
    }

    #[test]
    fn test_nested_generic_tokens() {
        let mut lexer = Token::lexer("List<List<Int>>");
        assert_eq!(
            lexer.next(),
            Some(Ok(Token::Identifier("List".to_string())))
        );
        assert_eq!(lexer.next(), Some(Ok(Token::Lt)));
        assert_eq!(
            lexer.next(),
            Some(Ok(Token::Identifier("List".to_string())))
        );
        assert_eq!(lexer.next(), Some(Ok(Token::Lt)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("Int".to_string()))));
        assert_eq!(lexer.next(), Some(Ok(Token::Shr)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_dollar_tokens() {
        // 2026-07-15: $ is a valid identifier character, so $unless is a
        // single Identifier token (longest match wins). $! is still a
        // single DollarBang token (exact token match beats identifier).
        let mut lexer = Token::lexer("$unless $!circular_buffer");
        assert_eq!(
            lexer.next(),
            Some(Ok(Token::Identifier("$unless".to_string())))
        );
        assert_eq!(lexer.next(), Some(Ok(Token::DollarBang)));
        assert_eq!(
            lexer.next(),
            Some(Ok(Token::Identifier("circular_buffer".to_string())))
        );
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_dollar_bang_as_single_token() {
        let mut lexer = Token::lexer("$!x");
        assert_eq!(lexer.next(), Some(Ok(Token::DollarBang)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("x".to_string()))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_hash_question_as_single_token() {
        let mut lexer = Token::lexer("#?inline");
        assert_eq!(lexer.next(), Some(Ok(Token::HashQuestion)));
        assert_eq!(
            lexer.next(),
            Some(Ok(Token::Identifier("inline".to_string())))
        );
        assert_eq!(lexer.next(), None);
    }

    // 2026-07-12: Updated for new #-as-ident-char behavior.
    // #volatile is now a single identifier, not Hash + volatile.
    #[test]
    fn test_hash_as_identifier_char() {
        let mut lexer = Token::lexer("#volatile");
        assert_eq!(
            lexer.next(),
            Some(Ok(Token::Identifier("#volatile".to_string())))
        );
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_exclaim_arrow_as_single_token() {
        let mut lexer = Token::lexer("x !> 5");
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("x".to_string()))));
        assert_eq!(lexer.next(), Some(Ok(Token::ExclaimArrow)));
        assert_eq!(lexer.next(), Some(Ok(Token::Integer(5))));
        assert_eq!(lexer.next(), None);

        let mut lexer2 = Token::lexer("x < 5");
        assert_eq!(lexer2.next(), Some(Ok(Token::Identifier("x".to_string()))));
        assert_eq!(lexer2.next(), Some(Ok(Token::Lt)));
        assert_eq!(lexer2.next(), Some(Ok(Token::Integer(5))));
        assert_eq!(lexer2.next(), None);
    }

    // ── New tests for Phase 0.1 features ──────────────────────

    #[test]
    fn test_intrinsic_identifier() {
        let mut lexer = Token::lexer("Sqrt#(x)");
        assert_eq!(
            lexer.next(),
            Some(Ok(Token::Identifier("Sqrt#".to_string())))
        );
        assert_eq!(lexer.next(), Some(Ok(Token::LParen)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("x".to_string()))));
        assert_eq!(lexer.next(), Some(Ok(Token::RParen)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_entry_hash_bracket() {
        // 2026-08-01 (Phase 2): `[#]` is no longer a contract marker — the
        // entry!/args! macros (Phase 3) replace it. The lexer still produces
        // Identifier("#") inside brackets; the PARSER now rejects `[#]` as a
        // removed syntax. This test pins the tokenization only.
        let mut lexer = Token::lexer("[#]");
        assert_eq!(lexer.next(), Some(Ok(Token::LBracket)));
        // # is now an identifier character, so [#] -> LBracket, Identifier("#"), RBracket
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("#".to_string()))));
        assert_eq!(lexer.next(), Some(Ok(Token::RBracket)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_hash_in_identifier_middle() {
        let mut lexer = Token::lexer("foo#bar");
        assert_eq!(
            lexer.next(),
            Some(Ok(Token::Identifier("foo#bar".to_string())))
        );
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_keyword_when() {
        let mut lexer = Token::lexer("when");
        assert_eq!(lexer.next(), Some(Ok(Token::When)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_keyword_input() {
        let mut lexer = Token::lexer("input port: Int;");
        assert_eq!(lexer.next(), Some(Ok(Token::Input)));
        assert_eq!(
            lexer.next(),
            Some(Ok(Token::Identifier("port".to_string())))
        );
        assert_eq!(lexer.next(), Some(Ok(Token::Colon)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("Int".to_string()))));
        assert_eq!(lexer.next(), Some(Ok(Token::Semicolon)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_keyword_output() {
        let mut lexer = Token::lexer("output status: Int;");
        assert_eq!(lexer.next(), Some(Ok(Token::Output)));
        assert_eq!(
            lexer.next(),
            Some(Ok(Token::Identifier("status".to_string())))
        );
        assert_eq!(lexer.next(), Some(Ok(Token::Colon)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("Int".to_string()))));
        assert_eq!(lexer.next(), Some(Ok(Token::Semicolon)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_inop_removed() {
        // inop should now lex as a regular identifier, not a keyword
        let mut lexer = Token::lexer("inop");
        assert_eq!(
            lexer.next(),
            Some(Ok(Token::Identifier("inop".to_string())))
        );
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_export_keyword() {
        let mut lexer = Token::lexer("export defn add(a: Int) -> Int;");
        assert_eq!(lexer.next(), Some(Ok(Token::Export)));
        assert_eq!(lexer.next(), Some(Ok(Token::Defn)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("add".to_string()))));
    }

    #[test]
    fn test_hash_bracket_still_works() {
        // #[...] attribute syntax should still lex as HashBracket
        let mut lexer = Token::lexer("#[inline]");
        assert_eq!(lexer.next(), Some(Ok(Token::HashBracket)));
        assert_eq!(
            lexer.next(),
            Some(Ok(Token::Identifier("inline".to_string())))
        );
        assert_eq!(lexer.next(), Some(Ok(Token::RBracket)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_colon_eq_derivation() {
        let mut lexer = Token::lexer("defn add(a: Int, b: Int) -> Int := { 1, 2 -> 3 };");
        assert_eq!(lexer.next(), Some(Ok(Token::Defn)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("add".to_string()))));
    }

    #[test]
    fn test_display_roundtrip() {
        let source = "defn main() -> Int { term 0; };";
        let mut lexer = Token::lexer(source);
        let mut found = Vec::new();
        while let Some(Ok(tok)) = lexer.next() {
            found.push(tok);
        }
        // Just verify that all tokens have Display impls that don't panic
        for tok in &found {
            let _ = format!("{}", tok);
        }
    }

    #[test]
    fn test_underscore_identifier() {
        let mut lexer = Token::lexer("_ _foo __bar");
        assert_eq!(lexer.next(), Some(Ok(Token::Underscore)));
        assert_eq!(
            lexer.next(),
            Some(Ok(Token::Identifier("_foo".to_string())))
        );
        assert_eq!(
            lexer.next(),
            Some(Ok(Token::Identifier("__bar".to_string())))
        );
        assert_eq!(lexer.next(), None);
    }
}

/// Convenience: tokenize a source string into (Token, Range) pairs.
/// Returns Ok(tokens) on success, Err on lex failure.
/// 2026-07-15: Phase 2 — Added for system plugin discovery (plugin loader)
/// and other programmatic use outside the compile pipeline.
pub fn tokenize(source: &str) -> Result<Vec<(Token, std::ops::Range<usize>)>, String> {
    let mut lexer = Token::lexer(source);
    let mut tokens = Vec::new();
    while let Some(result) = lexer.next() {
        let token = result.map_err(|_| "lex error".to_string())?;
        let span = lexer.span();
        tokens.push((token, span));
    }
    Ok(tokens)
}
