use logos::Logos;

#[derive(Logos, Debug, PartialEq, Clone)]
#[logos(skip r"[ \t\n\r]+")]
pub enum Token {
    #[token("sig")]
    Sig,
    #[token("defn")]
    Defn,
    #[token("let")]
    Let,
    #[token("const")]
    Const,
    #[token("txn")]
    Txn,
    #[token("rct")]
    Rct,
    #[token("txc")]
    Txc,
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
    #[token("struct")]
    Struct,
    #[token("rstruct")]
    Rstruct,
    #[token("render")]
    Render,

    #[token("true")]
    BoolTrue,
    #[token("false")]
    BoolFalse,

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
    #[token("|")]
    Pipe,
    #[token("||")]
    OrOr,
    #[token("&&")]
    AndAnd,
    #[token("!")]
    Not,
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
    #[token(".")]
    Dot,

    // Literals
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

    // Comments (preserved with content)
    #[regex(r"#[^\n]*", |lex| lex.slice().to_string())]
    Comment(String),
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
