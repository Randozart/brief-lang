// ── Parser Shared Utilities ────────────────────────────────────────────
// 2026-07-12: Phase 1.6 — expect, advance, peek, error reporting, span tracking.
// Flat code: each function is max 2 levels of nesting.

use crate::errors::{Span, SyntaxError};
use crate::lexer::Token;

pub struct Parser<'a> {
    pub tokens: Vec<(Token, std::ops::Range<usize>)>,
    pub pos: usize,
    pub source: &'a str,
    pub strict_mode: bool,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: Vec<(Token, std::ops::Range<usize>)>, source: &'a str) -> Self {
        Parser {
            tokens,
            pos: 0,
            source,
            strict_mode: false,
        }
    }

    pub fn with_strict_mode(mut self, mode: bool) -> Self {
        self.strict_mode = mode;
        self
    }

    /// Peek at the current token without consuming it.
    pub fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|(t, _)| t)
    }

    /// Peek at the current token and its span.
    pub fn peek_with_span(&self) -> Option<(&Token, &std::ops::Range<usize>)> {
        self.tokens.get(self.pos).map(|(t, s)| (t, s))
    }

    /// Check if the current token matches a specific kind.
    pub fn check(&self, kind: &Token) -> bool {
        self.peek().map_or(false, |t| {
            std::mem::discriminant(t) == std::mem::discriminant(kind)
        })
    }

    /// Expect a specific token, consume it, or return an error.
    pub fn expect(&mut self, kind: Token) -> Result<(), SyntaxError> {
        let (cur, span) = self
            .peek_with_span()
            .ok_or_else(|| SyntaxError::UnexpectedEOF {
                expected: format!("{:?}", kind),
                span: Span::dummy(),
            })?;
        if std::mem::discriminant(cur) == std::mem::discriminant(&kind) {
            self.pos += 1;
            Ok(())
        } else {
            Err(SyntaxError::UnexpectedToken {
                expected: format!("{:?}", kind),
                found: format!("{}", cur),
                span: self.make_span(span.clone()),
            })
        }
    }

    /// Advance past the current token and return it.
    pub fn advance(&mut self) -> Option<(Token, std::ops::Range<usize>)> {
        let tok = self.tokens.get(self.pos).cloned();
        self.pos += 1;
        tok
    }

    /// Get the current token as an identifier string, or error.
    /// 2026-07-14: Also accepts keyword tokens that are commonly used as
    /// identifiers (reg, op, bank, asm, stage, cell, etc.).
    pub fn expect_identifier(&mut self) -> Result<String, SyntaxError> {
        match self.advance() {
            Some((Token::Identifier(name), _)) => Ok(name),
            Some((tok, span)) => {
                // 2026-07-14: Many keywords are also used as identifiers
                // in benchmarks and user code. Accept them gracefully.
                if let Some(name) = self.keyword_as_identifier(&tok) {
                    Ok(name)
                } else {
                    Err(SyntaxError::UnexpectedToken {
                        expected: "identifier".into(),
                        found: format!("{}", tok),
                        span: self.make_span(span),
                    })
                }
            }
            None => Err(SyntaxError::UnexpectedEOF {
                expected: "identifier".into(),
                span: Span::dummy(),
            }),
        }
    }

    /// Get the current token as an integer, or error.
    pub fn expect_integer(&mut self) -> Result<i64, SyntaxError> {
        match self.advance() {
            Some((Token::Integer(n), _)) => Ok(n),
            Some((tok, span)) => Err(SyntaxError::UnexpectedToken {
                expected: "integer".into(),
                found: format!("{}", tok),
                span: self.make_span(span),
            }),
            None => Err(SyntaxError::UnexpectedEOF {
                expected: "integer".into(),
                span: Span::dummy(),
            }),
        }
    }

    /// Get the current token as a string literal, or error.
    pub fn expect_string(&mut self) -> Result<String, SyntaxError> {
        match self.advance() {
            Some((Token::String(s), _)) => Ok(s),
            Some((tok, span)) => Err(SyntaxError::UnexpectedToken {
                expected: "string literal".into(),
                found: format!("{}", tok),
                span: self.make_span(span),
            }),
            None => Err(SyntaxError::UnexpectedEOF {
                expected: "string literal".into(),
                span: Span::dummy(),
            }),
        }
    }

    /// Check if the current token is an identifier with a specific name.
    pub fn check_identifier(&self, name: &str) -> bool {
        self.peek()
            .map_or(false, |t| matches!(t, Token::Identifier(s) if s == name))
    }

    /// Consume a specific identifier if present.
    pub fn eat_identifier(&mut self, name: &str) -> bool {
        if self.check_identifier(name) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// Consume a token if it matches, without error.
    pub fn eat(&mut self, kind: &Token) -> bool {
        if self.check(kind) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// Create a Span from a logos byte range.
    pub fn make_span(&self, range: std::ops::Range<usize>) -> Span {
        let start = range.start;
        let end = range.end;
        let line = self.source[..start].lines().count();
        let column = start - self.source[..start].rfind('\n').map_or(0, |i| i + 1);
        Span::new(start, end, line, column + 1)
    }

    /// Report an error at the current position.
    pub fn error_at_current<T>(&self, msg: &str) -> Result<T, SyntaxError> {
        let span = self
            .peek_with_span()
            .map(|(_, s)| self.make_span(s.clone()))
            .unwrap_or(Span::dummy());
        Err(SyntaxError::InvalidExpression {
            reason: msg.to_string(),
            span,
        })
    }

    /// Report an error at a specific span.
    pub fn error_at<T>(&self, msg: &str, span: Span) -> Result<T, SyntaxError> {
        Err(SyntaxError::InvalidExpression {
            reason: msg.to_string(),
            span,
        })
    }

    /// 2026-07-14: Read the body of a `layout <~ <...>` pattern.
    /// Consumes tokens from opening `<` to matching `>`, tracking nested
    /// brackets `[]`, parens `()`, and braces `{}`. Returns the raw text
    /// sliced from the source for accuracy.
    pub fn read_layout_body(&mut self) -> Result<String, SyntaxError> {
        let start = self.peek_with_span().map(|(_, s)| s.start).unwrap_or(self.pos);
        self.expect(Token::Lt)?;
        let mut depth: u64 = 1;
        loop {
            let Some((tok, span)) = self.tokens.get(self.pos).cloned() else {
                return Err(SyntaxError::InvalidExpression {
                    reason: "unexpected end of file in layout body (missing >)".into(),
                    span: crate::errors::Span::new(0, 0, 0, 0),
                });
            };
            match tok {
                Token::Lt => { depth += 1; self.pos += 1; }
                Token::Gt => {
                    depth -= 1;
                    self.pos += 1;
                    if depth == 0 {
                        let raw = &self.source[start..span.start];
                        return Ok(raw.trim().to_string());
                    }
                }
                Token::LBrace | Token::LParen | Token::LBracket => { depth += 1; self.pos += 1; }
                Token::RBrace | Token::RParen | Token::RBracket => { depth -= 1; self.pos += 1; }
                _ => { self.pos += 1; }
            }
        }
    }

    /// 2026-07-14: Bridge between lexer keyword tokens and parser identifier matching.
    /// Maps keyword tokens (Frgn, Struct, Enum, Ok, etc.) to their string representations.
    pub fn keyword_as_identifier(&self, tok: &Token) -> Option<String> {
        Some(match tok {
            Token::Sig => "sig".into(), Token::Export => "export".into(),
            Token::Defn => "defn".into(), Token::Let => "let".into(),
            Token::Const => "const".into(), Token::Txn => "txn".into(),
            Token::Rct => "rct".into(), Token::Async => "async".into(),
            Token::Await => "await".into(), Token::Term => "term".into(),
            Token::TermBang => "term!".into(), Token::Escape => "escape".into(),
            Token::Uni => "uni".into(), Token::Is => "is".into(),
            Token::Like => "like".into(), Token::Import => "import".into(),
            Token::From => "from".into(), Token::As => "as".into(),
            Token::Frgn => "frgn".into(), Token::FrgnBang => "frgn!".into(),
            Token::Meld => "meld".into(), Token::Reg => "reg".into(),
            Token::Op => "op".into(), Token::Type => "type".into(),
            Token::Cell => "cell".into(), Token::Struct => "struct".into(),
            Token::Rstruct => "rstruct".into(), Token::Render => "render".into(),
            Token::Enum => "enum".into(), Token::Trg => "trg".into(),
            Token::Within => "within".into(), Token::PtrBang => "Ptr!".into(),
            Token::Ok => "Ok".into(), Token::Err => "Err".into(),
            Token::Match => "match".into(),
            // 2026-07-15: Template/Macro tokens removed
            Token::Quote => "quote".into(),
            Token::Dollar => "$".into(), Token::DollarBang => "$!".into(),
            Token::Foreach => "foreach".into(), Token::Pvt => "pvt".into(),
            Token::Sed => "sed".into(), Token::Sync => "sync".into(),
            Token::Some => "some".into(), Token::None => "none".into(),
            Token::When => "when".into(), Token::Cycles => "cycles".into(),
            Token::Cyc => "cyc".into(), Token::Ms => "ms".into(),
            Token::Seconds => "seconds".into(), Token::Minute => "minute".into(),
            Token::Minutes => "minutes".into(), Token::Nanoseconds => "nanoseconds".into(),
            Token::Input => "input".into(), Token::Output => "output".into(),
            Token::BoolTrue => "true".into(), Token::BoolFalse => "false".into(),
            _ => return None,
        })
    }

    /// Check if we're at end of file.
    pub fn is_at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }
}
