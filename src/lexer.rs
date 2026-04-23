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
    // Sig aliases: sig, sign, signature
    #[token("sig")]
    #[token("sign")]
    #[token("signature")]
    Sig,

    // Defn aliases: defn, def, definition
    #[token("defn")]
    #[token("def")]
    #[token("definition")]
    Defn,

    #[token("let")]
    Let,

    // Const aliases: const, constant
    #[token("const")]
    #[token("constant")]
    Const,

    // Txn aliases: txn, transact, transaction
    #[token("txn")]
    #[token("transact")]
    #[token("transaction")]
    Txn,

    #[token("rct")]
    Rct,

    #[token("async")]
    Async,

    #[token("term")]
    Term,
    #[token("escape")]
    Escape,
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
    #[token("syscall")]
    Syscall,
    #[token("syscall!")]
    SyscallBang,
    #[token("resource")]
    Resource,
    #[token("rsrc")]
    Rsrc,
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
    #[token("stage")]
    Stage,
    #[token("on")]
    On,
    #[token("forall")]
    Forall,
    #[token("exists")]
    Exists,
    #[token("within")]
    Within,
    #[token("bank")]
    Bank,
    #[token("Ok")]
    Ok,
    #[token("Err")]
    Err,
    #[token("match")]
    Match,

    #[token("some")]
    Some,
    #[token("none")]
    None,

    #[token("true")]
    BoolTrue,
    #[token("false")]
    BoolFalse,

    // Time units
    #[token("cycles")]
    Cycles,
    #[token("cyc")]
    Cyc,
    #[token("ms")]
    Ms,
    #[token("s")]
    #[token("sec")]
    #[token("seconds")]
    Seconds,
    #[token("min")]
    #[token("minute")]
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
    #[token(";")]
    Semicolon,
    #[token(":")]
    Colon,
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
}
