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

    #[token("term")]
    #[token("TERM")]
    Term,
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
    #[token("trg")]
    #[token("TRG")]
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
    #[token("forall")]
    #[token("FORALL")]
    Forall,
    #[token("exists")]
    #[token("EXISTS")]
    Exists,
    #[token("within")]
    #[token("WITHIN")]
    Within,
    #[token("bank")]
    #[token("BANK")]
    Bank,
    #[token("Ok")]
    #[token("OK")]
    Ok,
    #[token("Err")]
    #[token("ERR")]
    Err,
    #[token("match")]
    #[token("MATCH")]
    Match,

    #[token("some")]
    #[token("SOME")]
    Some,
    #[token("none")]
    #[token("NONE")]
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
    #[token("s")]
    #[token("S")]
    #[token("sec")]
    #[token("SEC")]
    #[token("seconds")]
    #[token("SECONDS")]
    Seconds,
    #[token("min")]
    #[token("MIN")]
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

    // Punctuation
    #[token("#[")]
    HashBracket,
    #[token("#![")]
    HashBangBracket,
    #[token(";")]
    Semicolon,
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
        // Remove surrounding quotes and handle escapes
        let inner = &s[1..s.len()-1];
        // For simplicity, just return the string slice without unescaping for now
        // A full implementation would handle escape sequences properly
        Some(inner.to_string())
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
        if inner.starts_with("\\u{") && inner.ends_with('}') {
            // Unicode escape: \u{1F600}
            let hex = &inner[3..inner.len()-1];
            return u32::from_str_radix(hex, 16).ok().and_then(|cp| char::from_u32(cp));
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
    #[token("Char")]  // NEW: Char type keyword
    TypeChar,
    #[token("HashMap")]  // HashMap type keyword
    TypeHashMap,
    #[token("HashSet")]  // HashSet type keyword
    TypeHashSet,
    #[token("StringBuilder")]  // StringBuilder type keyword
    TypeStringBuilder,
    #[token("Stack")]  // Stack type keyword
    TypeStack,
    #[token("Queue")]  // Queue type keyword
    TypeQueue,
    #[token("Data")]
    TypeData,
    #[token("Void")]
    TypeVoid,

    // Identifiers
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Identifier(String),
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
}
