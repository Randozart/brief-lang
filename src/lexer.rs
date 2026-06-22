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

use logos::Logos;

#[derive(Logos, Debug, PartialEq, Clone)]
#[logos(skip r"[ \t\n\r]+")]
#[logos(skip r"//[^\n]*")] // Skip // comments entirely
pub enum Token {
    // Sig aliases: sig, sign, signature (lowercase and UPPERCASE)
    #[token("sig")]
    #[token("SIG")]
    #[token("sign")]
    #[token("SIGN")]
    #[token("signature")]
    #[token("SIGNATURE")]
    Sig,

    // Defn aliases: defn, def, definition (lowercase and UPPERCASE)
    #[token("defn")]
    #[token("DEFN")]
    #[token("def")]
    #[token("DEF")]
    #[token("definition")]
    #[token("DEFINITION")]
    Defn,

    #[token("let")]
    #[token("LET")]
    Let,

    // Const aliases: const, constant (lowercase and UPPERCASE)
    #[token("const")]
    #[token("CONST")]
    #[token("constant")]
    #[token("CONSTANT")]
    Const,

    // Txn aliases: txn, transact, transaction (lowercase and UPPERCASE)
    #[token("txn")]
    #[token("TXN")]
    #[token("transact")]
    #[token("TRANSACT")]
    #[token("transaction")]
    #[token("TRANSACTION")]
    Txn,

    #[token("rct")]
    #[token("RCT")]
    Rct,

    #[token("async")]
    #[token("ASYNC")]
    Async,

    #[token("await")]
    #[token("AWAIT")]
    Await,

    #[token("term")]
    #[token("TERM")]
    Term,
    #[token("term!")]
    #[token("TERM!")]
    TermBang,
    #[token("escape")]
    #[token("ESCAPE")]
    Escape,
    #[token("unification")]
    #[token("UNIFICATION")]
    #[token("unify")]
    #[token("UNIFY")]
    #[token("uni")]
    #[token("UNI")]
    Unification,
    #[token("is")]
    #[token("IS")]
    Is,
    #[token("like")]
    #[token("LIKE")]
    Like,
    #[token("import")]
    #[token("IMPORT")]
    Import,
    #[token("from")]
    #[token("FROM")]
    From,
    #[token("as")]
    #[token("AS")]
    As,
    #[token("frgn")]
    #[token("FRGN")]
    Frgn,
    #[token("frgn!")]
    #[token("FRGN!")]
    FrgnBang,
    #[token("inop")]
    #[token("INOP")]
    #[token("inop#")]
    Inop,
    #[token("inop!")]
    #[token("INOP!")]
    #[token("inop#!")]
    InopBang,
    #[token("syscall")]
    #[token("SYSCALL")]
    Syscall,
    #[token("syscall!")]
    #[token("SYSCALL!")]
    SyscallBang,
    #[token("resource")]
    #[token("RESOURCE")]
    Resource,
    #[token("rsrc")]
    #[token("RSRC")]
    Rsrc,
    #[token("reg")]
    #[token("REG")]
    #[token("registry")]
    #[token("REGISTRY")]
    Registry,
    #[token("type")]
    #[token("TYPE")]
    Type,

    #[token("struct")]
    #[token("STRUCT")]
    Struct,
    #[token("rstruct")]
    #[token("RSTRUCT")]
    Rstruct,
    #[token("render")]
    #[token("RENDER")]
    Render,
    #[token("enum")]
    #[token("ENUM")]
    Enum,
    #[token("trg!")]
    #[token("TRG!")]
    #[token("trigger!")]
    #[token("TRIGGER!")]
    TrgBang,
    #[token("trg")]
    #[token("TRG")]
    #[token("trigger")]
    #[token("TRIGGER")]
    Trg,
    #[token("link")]
    #[token("LINK")]
    Link,
    #[token("asm")]
    #[token("ASM")]
    Asm,
    #[token("stage")]
    #[token("STAGE")]
    Stage,
    #[token("on")]
    #[token("ON")]
    On,
    #[token("within")]
    #[token("WITHIN")]
    Within,
    #[token("bank")]
    #[token("BANK")]
    Bank,
    #[token("Ptr!")]
    PtrBang,
    #[token("Ok")]
    #[token("OK")]
    Ok,
    #[token("Err")]
    #[token("ERR")]
    Err,
    #[token("match")]
    #[token("MATCH")]
    Match,

    #[token("template")]
    #[token("TEMPLATE")]
    Template,
    #[token("macro")]
    #[token("MACRO")]
    Macro,
    #[token("quote")]
    #[token("QUOTE")]
    Quote,

    #[token("$")]
    Dollar,
    #[token("$!")]
    DollarBang,

    #[token("foreach")]
    #[token("FOREACH")]
    Foreach,

    // Visibility: pvt / private (struct boundary), sed / sedentary (file boundary)
    #[token("pvt")]
    #[token("PVT")]
    #[token("private")]
    #[token("PRIVATE")]
    Pvt,

    #[token("sed")]
    #[token("SED")]
    #[token("sedentary")]
    #[token("SEDENTARY")]
    Sed,

    #[token("sync")]
    #[token("SYNC")]
    Sync,

    #[token("some")]
    #[token("SOME")]
    #[token("Some")]
    Some,
    #[token("none")]
    #[token("NONE")]
    #[token("None")]
    None,

    #[token("true")]
    #[token("TRUE")]
    BoolTrue,
    #[token("false")]
    #[token("FALSE")]
    BoolFalse,

    // Time units (lowercase and UPPERCASE)
    #[token("cycles")]
    #[token("CYCLES")]
    Cycles,
    #[token("cyc")]
    #[token("CYC")]
    Cyc,
    #[token("ms")]
    #[token("MS")]
    Ms,
    #[token("sec")]
    #[token("SEC")]
    #[token("seconds")]
    #[token("SECONDS")]
    Seconds,
    #[token("minute")]
    #[token("MINUTE")]
    Minute,

    // Operators
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
    #[token("-")]
    Minus,
    #[token("~/")]
    TildeSlash,
    #[token("~")]
    Tilde,
    #[token("++")]
    PlusPlus,
    #[token("+")]
    Plus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("^")]
    BitXor,
    #[token("->")]
    Arrow,
    #[token("<-")]
    ArrowLeft,
    #[token("_")]
    Underscore,

    // Punctuation
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
    #[token("#")]
    Hash,
    #[token("#!")]
    HashBang,
    #[token(";")]
    Semicolon,
    #[token("<:")]
    LtColon,

    #[token(":>")]
    ColonGreaterThan,
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

    // Literals
    #[regex(r"0x[0-9a-fA-F]+", |lex| i64::from_str_radix(&lex.slice()[2..], 16).ok())]
    #[regex(r"[0-9]+", |lex| lex.slice().parse().ok())]
    Integer(i64),
    #[regex(r"[0-9]+\.[0-9]+", |lex| lex.slice().parse().ok())]
    Float(f64),
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
                        // Hex escape: \x03 → char 3
                        let hex_str: String = chars.by_ref().take(2).collect();
                        if let Ok(h) = u8::from_str_radix(&hex_str, 16) {
                            out.push(h as char);
                        }
                    }
                    Some('u') => {
                        // Unicode escape: \u{1F600}
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
        // Handle escape sequences
        if inner.is_empty() {
            return Some(' ');  // Default for empty char literal
        }
        if inner.len() == 1 {
            return Some(inner.chars().next().unwrap());
        }
        if inner == "\\0" {
            return Some('\0');
        }
        if inner == "\\n" {
            return Some('\n');
        }
        if inner == "\\t" {
            return Some('\t');
        }
        if inner == "\\\\" {
            return Some('\\');
        }
        if inner == "\\'" {
            return Some('\'');
        }
        if inner.len() == 4 && inner.starts_with("\\x") {
            // Hex escape: \x03 → char 3
            if let Ok(h) = u8::from_str_radix(&inner[2..], 16) {
                return Some(h as char);
            }
        }
        if inner.starts_with("\\u{") && inner.ends_with('}') {
            // Unicode escape: \u{1F600}
            if let Ok(cp) = u32::from_str_radix(&inner[3..inner.len()-1], 16) {
                return Some(char::from_u32(cp).unwrap_or('?'));
            }
        }
        // Multi-character char literal or invalid - just take first char
        Some(inner.chars().next().unwrap_or(' '))
    })]
    Char(char),

    // Keywords
    #[token("Int")]
    TypeInt,
    #[token("UInt")]
    TypeUInt,
    #[token("Unsigned")]
    TypeUnsigned,
    #[token("USgn")]
    TypeUSgn,
    #[token("Signed")]
    TypeSigned,
    #[token("Sgn")]
    TypeSgn,
    #[token("Float")]
    TypeFloat,
    #[token("String")]
    TypeString,
    #[token("Bool")]
    TypeBool,
    #[token("void")]
    TypeVoid,
    #[token("Data")]
    TypeData,
    #[token("Char")]  // NEW: Char type keyword
    TypeChar,
    // Note: HashMap, HashSet, StringBuilder, Stack, Queue are regular identifiers
    // defined in stdlib, not special type keywords. This keeps the language pure.

    // Shorthand sized integer types (syntactic sugar for Int/UInt @/xN)
    #[token("i8")]
    TypeI8,
    #[token("u8")]
    TypeU8,
    #[token("i16")]
    TypeI16,
    #[token("u16")]
    TypeU16,
    #[token("i32")]
    TypeI32,
    #[token("u32")]
    TypeU32,
    #[token("i64")]
    TypeI64,
    #[token("u64")]
    TypeU64,

    // Identifiers
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Identifier(String),
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Sig => write!(f, "sig"),
            Token::Defn => write!(f, "defn"),
            Token::Let => write!(f, "let"),
            Token::Const => write!(f, "const"),
            Token::Txn => write!(f, "txn"),
            Token::Rct => write!(f, "rct"),
            Token::Async => write!(f, "async"),
            Token::Await => write!(f, "await"),
            Token::Term => write!(f, "term"),
            Token::TermBang => write!(f, "term!"),
            Token::Escape => write!(f, "escape"),
            Token::Unification => write!(f, "unification"),
            Token::Is => write!(f, "is"),
            Token::Like => write!(f, "like"),
            Token::Import => write!(f, "import"),
            Token::From => write!(f, "from"),
            Token::As => write!(f, "as"),
            Token::Frgn => write!(f, "frgn"),
            Token::FrgnBang => write!(f, "frgn!"),
            Token::Inop => write!(f, "inop"),
            Token::InopBang => write!(f, "inop!"),
            Token::Syscall => write!(f, "syscall"),
            Token::SyscallBang => write!(f, "syscall!"),
            Token::Resource => write!(f, "resource"),
            Token::Rsrc => write!(f, "rsrc"),
            Token::Registry => write!(f, "reg"),
            Token::Type => write!(f, "type"),
            Token::Struct => write!(f, "struct"),
            Token::Rstruct => write!(f, "rstruct"),
            Token::Render => write!(f, "render"),
            Token::Enum => write!(f, "enum"),
            Token::TrgBang => write!(f, "trg!"),
            Token::Trg => write!(f, "trg"),
            Token::Link => write!(f, "link"),
            Token::Asm => write!(f, "asm"),
            Token::Stage => write!(f, "stage"),
            Token::On => write!(f, "on"),
            Token::Within => write!(f, "within"),
            Token::Bank => write!(f, "bank"),
            Token::PtrBang => write!(f, "Ptr!"),
            Token::Ok => write!(f, "Ok"),
            Token::Err => write!(f, "Err"),
            Token::Match => write!(f, "match"),
            Token::Template => write!(f, "template"),
            Token::Macro => write!(f, "macro"),
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
            Token::Pipe => write!(f, "|"),
            Token::OrOr => write!(f, "||"),
            Token::AndAnd => write!(f, "&&"),
            Token::Not => write!(f, "!"),
            Token::Question => write!(f, "?"),
            Token::Minus => write!(f, "-"),
            Token::TildeSlash => write!(f, "~/"),
            Token::Tilde => write!(f, "~"),
            Token::PlusPlus => write!(f, "++"),
            Token::Plus => write!(f, "+"),
            Token::Star => write!(f, "*"),
            Token::Slash => write!(f, "/"),
            Token::BitXor => write!(f, "^"),
            Token::Arrow => write!(f, "->"),
            Token::ArrowLeft => write!(f, "<-"),
            Token::Underscore => write!(f, "_"),
            Token::HashBracket => write!(f, "#["),
            Token::HashBangBracket => write!(f, "#!["),
            Token::Pragma => write!(f, "#pragma"),
            Token::PragmaBang => write!(f, "#!pragma"),
            Token::HashQuestion => write!(f, "#?"),
            Token::Hash => write!(f, "#"),
            Token::HashBang => write!(f, "#!"),
            Token::Semicolon => write!(f, ";"),
            Token::LtColon => write!(f, "<:"),
            Token::ColonGreaterThan => write!(f, ":>"),
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
            Token::Float(n) => write!(f, "{}", n),
            Token::String(s) => write!(f, "\"{}\"", s),
            Token::Char(c) => write!(f, "'{}'", c),
            Token::TypeInt => write!(f, "Int"),
            Token::TypeUInt => write!(f, "UInt"),
            Token::TypeUnsigned => write!(f, "Unsigned"),
            Token::TypeUSgn => write!(f, "USgn"),
            Token::TypeSigned => write!(f, "Signed"),
            Token::TypeSgn => write!(f, "Sgn"),
            Token::TypeFloat => write!(f, "Float"),
            Token::TypeString => write!(f, "String"),
            Token::TypeBool => write!(f, "Bool"),
            Token::TypeVoid => write!(f, "void"),
            Token::TypeData => write!(f, "Data"),
            Token::TypeChar => write!(f, "Char"),
            Token::TypeI8 => write!(f, "i8"),
            Token::TypeU8 => write!(f, "u8"),
            Token::TypeI16 => write!(f, "i16"),
            Token::TypeU16 => write!(f, "u16"),
            Token::TypeI32 => write!(f, "i32"),
            Token::TypeU32 => write!(f, "u32"),
            Token::TypeI64 => write!(f, "i64"),
            Token::TypeU64 => write!(f, "u64"),
            Token::Identifier(s) => write!(f, "{}", s),
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
        assert_eq!(lexer.next(), Some(Ok(Token::TypeInt)));
        assert_eq!(lexer.next(), Some(Ok(Token::Arrow)));
        assert_eq!(lexer.next(), Some(Ok(Token::TypeInt)));
        assert_eq!(lexer.next(), Some(Ok(Token::Semicolon)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_char_literals() {
        // Basic char
        let mut lexer = Token::lexer("'a'");
        assert_eq!(lexer.next(), Some(Ok(Token::Char('a'))));
        
        // Newline escape
        let mut lexer = Token::lexer("'\\n'");
        assert_eq!(lexer.next(), Some(Ok(Token::Char('\n'))));
        
        // Tab escape
        let mut lexer = Token::lexer("'\\t'");
        assert_eq!(lexer.next(), Some(Ok(Token::Char('\t'))));
        
        // Backslash escape
        let mut lexer = Token::lexer("'\\\\'");
        assert_eq!(lexer.next(), Some(Ok(Token::Char('\\'))));
        
        // Single quote escape
        let mut lexer = Token::lexer("'\\''");
        assert_eq!(lexer.next(), Some(Ok(Token::Char('\''))));
        
        // Unicode escape
        let mut lexer = Token::lexer("'\\u{1F600}'");
        assert_eq!(lexer.next(), Some(Ok(Token::Char('😀'))));
        
        // Char type keyword
        let mut lexer = Token::lexer("let c: Char = 'x';");
        assert_eq!(lexer.next(), Some(Ok(Token::Let)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("c".to_string()))));
        assert_eq!(lexer.next(), Some(Ok(Token::Colon)));
        assert_eq!(lexer.next(), Some(Ok(Token::TypeChar)));
        assert_eq!(lexer.next(), Some(Ok(Token::Eq)));
        assert_eq!(lexer.next(), Some(Ok(Token::Char('x'))));
        assert_eq!(lexer.next(), Some(Ok(Token::Semicolon)));
    }

    #[test]
    fn test_nested_generic_tokens() {
        // Test that >> is lexed as Shr, not two Gt tokens
        let mut lexer = Token::lexer("List<List<Int>>");
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("List".to_string()))));
        assert_eq!(lexer.next(), Some(Ok(Token::Lt)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("List".to_string()))));
        assert_eq!(lexer.next(), Some(Ok(Token::Lt)));
        assert_eq!(lexer.next(), Some(Ok(Token::TypeInt)));
        assert_eq!(lexer.next(), Some(Ok(Token::Shr)));  // >> is Shr
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_template_macro_keywords() {
        let mut lexer = Token::lexer("template macro quote");
        assert_eq!(lexer.next(), Some(Ok(Token::Template)));
        assert_eq!(lexer.next(), Some(Ok(Token::Macro)));
        assert_eq!(lexer.next(), Some(Ok(Token::Quote)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_dollar_tokens() {
        let mut lexer = Token::lexer("$unless $!circular_buffer");
        assert_eq!(lexer.next(), Some(Ok(Token::Dollar)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("unless".to_string()))));
        assert_eq!(lexer.next(), Some(Ok(Token::DollarBang)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("circular_buffer".to_string()))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_dollar_bang_as_single_token() {
        // $! must be lexed as a single DollarBang token, not Dollar + Not
        let mut lexer = Token::lexer("$!x");
        assert_eq!(lexer.next(), Some(Ok(Token::DollarBang)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("x".to_string()))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_hash_question_as_single_token() {
        // #? must be lexed as a single HashQuestion token, not Hash + Question
        let mut lexer = Token::lexer("#?inline");
        assert_eq!(lexer.next(), Some(Ok(Token::HashQuestion)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("inline".to_string()))));
        assert_eq!(lexer.next(), None);

        // Verify # is still lexed correctly for normal hashtags
        let mut lexer2 = Token::lexer("#volatile");
        assert_eq!(lexer2.next(), Some(Ok(Token::Hash)));
        assert_eq!(lexer2.next(), Some(Ok(Token::Identifier("volatile".to_string()))));
        assert_eq!(lexer2.next(), None);
    }
}
