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

use crate::ast::*;
use crate::errors::{Span, SyntaxError};
use crate::features::binary_op::{BinaryOpExpr, BinaryOpKind};
use crate::features::literal::LiteralExpr;
use crate::features::unary_op::{UnaryOpExpr, UnaryOpKind};
use crate::lexer::Token;
use logos::{Lexer, Logos};
use std::path::Path;

/// Flatten `#!cfg` guards in a program's items list by evaluating conditions
/// against the given target configuration. Items guarded by false conditions
/// are removed. Nested `#!cfg` guards are recursively flattened.
pub fn flatten_cfg(items: &mut Vec<TopLevel>, target_os: &str, target_arch: &str, board: &str) {
    let mut i = 0;
    while i < items.len() {
        let (is_cfg, active) = match &items[i] {
            TopLevel::Cfg(cfg) => {
                let active = match cfg.condition.evaluate(target_os, target_arch, board) {
                    Ok(v) => v,
                    Err(warn) => {
                        eprintln!("warning: {}", warn);
                        false
                    }
                };
                (true, active)
            }
            _ => (false, false),
        };
        if is_cfg {
            if active {
                let mut replacement = match items.remove(i) {
                    TopLevel::Cfg(cfg) => cfg.items,
                    _ => unreachable!(),
                };
                flatten_cfg(&mut replacement, target_os, target_arch, board);
                items.splice(i..i, replacement);
            } else {
                items.remove(i);
            }
        } else {
            i += 1;
        }
    }
}

pub fn parse_hardware_config(path: &Path) -> Result<HardwareConfig, SyntaxError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read hardware config: {}", e))?;
    toml::from_str(&content).map_err(|e| format!("Failed to parse hardware config: {}", e).into())
}

pub struct Parser<'a> {
    lexer: Lexer<'a, Token>,
    source: &'a str,
    pos: usize,
    current: Option<(Result<Token, ()>, logos::Span)>,
    peek: Option<(Result<Token, ()>, logos::Span)>,
    peek2: Option<(Result<Token, ()>, logos::Span)>,
    comments: Vec<Comment>,
    current_line: usize,
    /// Track if we consumed a >> that should serve as > for parent generic level
    shr_consumed_as_gt: bool,
    strict_mode: StrictMode,
    /// Track whether a top-level executable statement has been seen.
    /// Declarations after the first statement are a compile error.
    seen_top_level_stmt: bool,
    /// When set by parse_trigger_body, the top-level trg handler uses this
    /// to emit a TopLevel::TriggerBinding instead of TopLevel::Trigger.
    pending_cell_binding: Option<(String, String, String, Option<Type>)>, // (trigger_name, cell_name, port, ty)
    /// Names of top-level items marked with `sed` (file-private).
    sed_item_names: Vec<String>,
    /// When true, @ident and @{expr} produce interpolation markers (inside quote { })
    in_quote_block: bool,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        let mut lexer = Token::lexer(input);
        let current = lexer.next().map(|token| (token, lexer.span()));
        let peek = lexer.next().map(|token| (token, lexer.span()));
        let peek2 = lexer.next().map(|token| (token, lexer.span()));
        Parser {
            lexer,
            source: input,
            pos: 0,
            current,
            peek,
            peek2,
            comments: Vec::new(),
            current_line: 1,
            shr_consumed_as_gt: false,
            strict_mode: StrictMode::Off,
            seen_top_level_stmt: false,
            sed_item_names: Vec::new(),
            in_quote_block: false,
            pending_cell_binding: None,
        }
    }

    pub fn with_strict_mode(mut self, mode: bool) -> Self {
        self.strict_mode = if mode { StrictMode::Strict } else { StrictMode::Off };
        self
    }

    pub fn with_gpu_mode(self, _mode: bool) -> Self {
        // GPU mode is handled by the Annotator pass (compiler pass #2).
        // The parser itself is target-agnostic; no changes to parsing needed.
        self
    }

    pub fn take_sed_item_names(&mut self) -> Vec<String> {
        std::mem::take(&mut self.sed_item_names)
    }

    fn parse_field_visibility(&mut self) -> Visibility {
        match self.current_token() {
            Some(Ok(Token::Pvt)) => { self.advance(); Visibility::Private }
            Some(Ok(Token::Sed)) => { self.advance(); Visibility::Sedentary }
            _ => Visibility::Public,
        }
    }

    fn advance(&mut self) {
        self.current = self.peek.take();
        self.peek = self.peek2.take();
        self.peek2 = self.lexer.next().map(|token| (token, self.lexer.span()));

        if let Some((_, span)) = &self.current {
            self.current_line = span.start;
            self.pos = span.start;
        }
    }

    fn put_back(&mut self, token: Token, span: logos::Span) {
        self.peek2 = self.peek.take();
        self.peek = self.current.take();
        self.current = Some((Ok(token), span));
    }

    fn current_token(&self) -> Option<&Result<Token, ()>> {
        self.current.as_ref().map(|(t, _)| t)
    }

    fn peek_token(&self) -> Option<&Result<Token, ()>> {
        self.peek.as_ref().map(|(t, _)| t)
    }

    fn peek_token2(&self) -> Option<&Result<Token, ()>> {
        self.peek2.as_ref().map(|(t, _)| t)
    }

    fn current_span(&self) -> Option<Span> {
        self.current.as_ref().map(|(_, span)| {
            let line = self.source[..span.start].matches('\n').count() + 1;
            let line_start = self.source[..span.start]
                .rfind('\n')
                .map(|p| p + 1)
                .unwrap_or(0);
            let column = span.start - line_start + 1;
            Span::new(span.start, span.end, line, column)
        })
    }

    fn spanned_err<T>(&self, message: String) -> Result<T, SyntaxError> {
        Err(SyntaxError::InvalidStatement {
            reason: message,
            span: self.current_span().unwrap_or_else(Span::dummy),
        })
    }

    fn token_display(token: &Token) -> String {
        match token {
            Token::Identifier(s) => format!("'{}'", s),
            Token::Integer(n) => format!("integer {}", n),
            Token::Float(f) => format!("float {}", f),
            Token::String(s) => format!("\"{}\"", s),
            Token::Char(c) => format!("'{}'", c),
            Token::Eq => "'='".into(),
            Token::EqEq => "'=='".into(),
            Token::Ne => "'!='".into(),
            Token::Lt => "'<'".into(),
            Token::Gt => "'>'".into(),
            Token::Le => "'<='".into(),
            Token::Ge => "'>='".into(),
            Token::LParen => "'('".into(),
            Token::RParen => "')'".into(),
            Token::LBrace => "'{'".into(),
            Token::RBrace => "'}'".into(),
            Token::LBracket => "'['".into(),
            Token::RBracket => "']'".into(),
            Token::Arrow => "'->'".into(),
            Token::ArrowLeft => "'<-'".into(),
            Token::Colon => "':'".into(),
            Token::Semicolon => "';'".into(),
            Token::Comma => "','".into(),
            Token::Dot => "'.'".into(),
            Token::At => "'@'".into(),
            Token::Hash => "'#'".into(),
            Token::Tilde => "'~'".into(),
            Token::Star => "'*'".into(),
            Token::Plus => "'+'".into(),
            Token::Minus => "'-'".into(),
            Token::Slash => "'/'".into(),
            Token::Underscore => "'_'".into(),
            Token::Question => "'?'".into(),
            Token::Not => "'!'".into(),
            Token::Sig => "sig".into(),
            Token::Defn => "defn".into(),
            Token::Let => "let".into(),
            Token::Txn => "txn".into(),
            Token::Rct => "rct".into(),
            Token::Async => "async".into(),
            Token::Await => "await".into(),
            Token::Term => "term".into(),
            Token::Frgn => "frgn".into(),
            Token::Inop => "inop".into(),
            Token::InopBang => "inop!".into(),
            Token::Meld => "meld".into(),
            Token::Import => "import".into(),
            Token::Struct => "struct".into(),
            Token::Enum => "enum".into(),
            Token::Render => "render".into(),
            Token::BoolTrue => "true".into(),
            Token::BoolFalse => "false".into(),
            Token::Match => "match".into(),
            Token::Foreach => "foreach".into(),
            Token::Ok => "Ok".into(),
            Token::Err => "Err".into(),
            Token::Some => "Some".into(),
            Token::None => "None".into(),
            Token::Bank => "bank".into(),
            Token::Trg => "trg".into(),
            Token::Link => "link".into(),
            Token::Asm => "asm".into(),
            Token::Ellipsis => "'...'".into(),
            _ => format!("{}", token),
        }
    }

    fn fmt_current_token(&self) -> String {
        match self.current_token() {
            Some(Ok(tok)) => format!("{}", tok),
            Some(Err(_)) => "<lexer error>".to_string(),
            None => "<end of input>".to_string(),
        }
    }

    fn expect(&mut self, expected: Token) -> Result<(), crate::errors::SyntaxError> {
        let span = self.current_span().unwrap_or_else(Span::dummy);
        match self.current_token() {
            Some(Ok(tok)) if *tok == expected => {
                self.advance();
                Ok(())
            }
            Some(Ok(tok)) => Err(crate::errors::SyntaxError::UnexpectedToken {
                expected: Self::token_display(&expected),
                found: Self::token_display(tok),
                span,
            }),
            Some(Err(_)) => Err(crate::errors::SyntaxError::InvalidStatement {
                reason: "Lexer error".to_string(),
                span,
            }),
            None => Err(crate::errors::SyntaxError::UnexpectedEOF {
                expected: Self::token_display(&expected),
                span,
            }),
        }
    }

    fn expect_identifier(&mut self) -> Result<String, crate::errors::SyntaxError> {
        let span = self.current_span().unwrap_or_else(Span::dummy);
        match self.current_token() {
            Some(Ok(Token::Identifier(name))) => {
                let name = name.clone();
                self.advance();
                Ok(name)
            }
            Some(Ok(Token::TypeData)) => { self.advance(); Ok("Data".to_string()) }
            Some(Ok(Token::TypeInt)) => { self.advance(); Ok("Int".to_string()) }
            Some(Ok(Token::Some)) => { self.advance(); Ok("Some".to_string()) }
            Some(Ok(Token::None)) => { self.advance(); Ok("None".to_string()) }
            Some(Ok(Token::Ok)) => { self.advance(); Ok("Ok".to_string()) }
            Some(Ok(Token::Err)) => { self.advance(); Ok("Err".to_string()) }
            Some(Ok(Token::Sig)) => { self.advance(); Ok("sig".to_string()) }
            Some(Ok(Token::Defn)) => { self.advance(); Ok("defn".to_string()) }
            Some(Ok(Token::Let)) => { self.advance(); Ok("let".to_string()) }
            Some(Ok(Token::Txn)) => { self.advance(); Ok("txn".to_string()) }
            Some(Ok(Token::Rct)) => { self.advance(); Ok("rct".to_string()) }
            Some(Ok(Token::Frgn)) => { self.advance(); Ok("frgn".to_string()) }
            Some(Ok(Token::Inop)) => { self.advance(); Ok("inop".to_string()) }
            Some(Ok(Token::InopBang)) => { self.advance(); Ok("inop!".to_string()) }
            Some(Ok(Token::Meld)) => { self.advance(); Ok("meld".to_string()) }
            Some(Ok(Token::Struct)) => { self.advance(); Ok("struct".to_string()) }
            Some(Ok(Token::Enum)) => { self.advance(); Ok("enum".to_string()) }
            Some(Ok(Token::Import)) => { self.advance(); Ok("import".to_string()) }
            Some(Ok(Token::Term)) => { self.advance(); Ok("term".to_string()) }
            Some(Ok(Token::Const)) => { self.advance(); Ok("const".to_string()) }
            Some(Ok(Token::BoolTrue)) => { self.advance(); Ok("true".to_string()) }
            Some(Ok(Token::BoolFalse)) => { self.advance(); Ok("false".to_string()) }
            Some(Ok(Token::Uni)) => { self.advance(); Ok("uni".to_string()) }
            Some(Ok(Token::Escape)) => { self.advance(); Ok("escape".to_string()) }
            Some(Ok(Token::Async)) => { self.advance(); Ok("async".to_string()) }
            Some(Ok(Token::Await)) => { self.advance(); Ok("await".to_string()) }
            Some(Ok(Token::From)) => { self.advance(); Ok("from".to_string()) }
            Some(Ok(Token::As)) => { self.advance(); Ok("as".to_string()) }
            Some(Ok(Token::Reg)) => { self.advance(); Ok("reg".to_string()) }
            Some(Ok(Token::Is)) => { self.advance(); Ok("is".to_string()) }
            Some(Ok(Token::Like)) => { self.advance(); Ok("like".to_string()) }
            Some(Ok(Token::Cycles)) => { self.advance(); Ok("cycles".to_string()) }
            Some(Ok(Token::Cyc)) => { self.advance(); Ok("cyc".to_string()) }
            Some(Ok(Token::Ms)) => { self.advance(); Ok("ms".to_string()) }
            Some(Ok(Token::Seconds)) => { self.advance(); Ok("seconds".to_string()) }
            Some(Ok(Token::Minute)) => { self.advance(); Ok("minute".to_string()) }
            Some(Ok(Token::TypeUInt)) => { self.advance(); Ok("UInt".to_string()) }
            Some(Ok(Token::TypeUnsigned)) => { self.advance(); Ok("Unsigned".to_string()) }
            Some(Ok(Token::TypeUSgn)) => { self.advance(); Ok("USgn".to_string()) }
            Some(Ok(Token::TypeSigned)) => { self.advance(); Ok("Signed".to_string()) }
            Some(Ok(Token::TypeSgn)) => { self.advance(); Ok("Sgn".to_string()) }
            Some(Ok(Token::TypeChar)) => { self.advance(); Ok("Char".to_string()) }
            Some(Ok(Token::TypeFloat)) => { self.advance(); Ok("Float".to_string()) }
            Some(Ok(Token::TypeString)) => { self.advance(); Ok("String".to_string()) }
            Some(Ok(Token::TypeBool)) => { self.advance(); Ok("Bool".to_string()) }
            Some(Ok(Token::TypeVoid)) => { self.advance(); Ok("Void".to_string()) }
            Some(Ok(Token::TypeI8)) => { self.advance(); Ok("i8".to_string()) }
            Some(Ok(Token::TypeU8)) => { self.advance(); Ok("u8".to_string()) }
            Some(Ok(Token::TypeI16)) => { self.advance(); Ok("i16".to_string()) }
            Some(Ok(Token::TypeU16)) => { self.advance(); Ok("u16".to_string()) }
            Some(Ok(Token::TypeI32)) => { self.advance(); Ok("i32".to_string()) }
            Some(Ok(Token::TypeU32)) => { self.advance(); Ok("u32".to_string()) }
            Some(Ok(Token::TypeI64)) => { self.advance(); Ok("i64".to_string()) }
            Some(Ok(Token::TypeU64)) => { self.advance(); Ok("u64".to_string()) }
            Some(Ok(Token::TypeInt8)) => { self.advance(); Ok("Int8".to_string()) }
            Some(Ok(Token::TypeInt16)) => { self.advance(); Ok("Int16".to_string()) }
            Some(Ok(Token::TypeInt32)) => { self.advance(); Ok("Int32".to_string()) }
            Some(Ok(Token::TypeInt64)) => { self.advance(); Ok("Int64".to_string()) }
            Some(Ok(Token::TypeUInt8)) => { self.advance(); Ok("UInt8".to_string()) }
            Some(Ok(Token::TypeUInt16)) => { self.advance(); Ok("UInt16".to_string()) }
            Some(Ok(Token::TypeUInt32)) => { self.advance(); Ok("UInt32".to_string()) }
            Some(Ok(Token::TypeUInt64)) => { self.advance(); Ok("UInt64".to_string()) }
            Some(Ok(Token::TypeFloat32)) => { self.advance(); Ok("Float32".to_string()) }
            Some(Ok(Token::TypeF32)) => { self.advance(); Ok("F32".to_string()) }
            Some(Ok(Token::TypeFloat64)) => { self.advance(); Ok("Float64".to_string()) }
            Some(Ok(Token::TypeF64)) => { self.advance(); Ok("F64".to_string()) }
            Some(Ok(Token::TypeDouble)) => { self.advance(); Ok("Double".to_string()) }
            _ => Err(SyntaxError::UnexpectedToken {
                expected: "identifier".to_string(),
                found: self.fmt_current_token(),
                span,
            }),
        }
    }

    fn expect_integer(&mut self) -> Result<i64, crate::errors::SyntaxError> {
        let span = self.current_span().unwrap_or_else(Span::dummy);
        match self.current_token() {
            Some(Ok(Token::Integer(n))) => {
                let n = *n;
                self.advance();
                Ok(n)
            }
            _ => Err(SyntaxError::UnexpectedToken {
                expected: "integer".to_string(),
                found: self.fmt_current_token(),
                span,
            }),
        }
    }

    /// Check if the next token is `<:` (subtype projection / type derivation operator)
    fn check_lt_colon(&self) -> bool {
        matches!(self.current_token(), Some(Ok(Token::LtColon)))
    }

    /// Parse a constraint expression inside `[expr]` after `<:`.
    /// `lo..hi` range syntax is desugared to `_ >= lo && _ <= hi`.
    fn parse_constraint_expr(&mut self) -> Result<Box<Expr>, SyntaxError> {
        let first = self.parse_expression()?;
        if let Some(Ok(Token::DotDot)) = self.current_token() {
            self.advance();
            let second = self.parse_expression()?;
            // Desugar: lo..hi → _ >= lo && _ <= hi
            Ok(Box::new(Expr::And(
                Box::new(Expr::Ge(
                    Box::new(Expr::Identifier("_".to_string())),
                    Box::new(first),
                )),
                Box::new(Expr::Le(
                    Box::new(Expr::Identifier("_".to_string())),
                    Box::new(second),
                )),
            )))
        } else {
            // Single expression — the constraint itself references `_`
            Ok(Box::new(first))
        }
    }

    fn parse_hashtag_modifiers(&mut self) -> Result<Vec<Annotation>, SyntaxError> {
        let mut mods = Vec::new();
        loop {
            match self.current_token() {
                Some(Ok(Token::HashQuestion)) => {
                    self.advance();
                    let name = if let Some(Ok(Token::Identifier(n))) = self.current_token() {
                        let n = n.clone();
                        self.advance();
                        n
                    } else {
                        return Ok(mods);
                    };
                    let value = if let Some(Ok(Token::LParen)) = self.current_token() {
                        self.advance();
                        let val = if let Some(Ok(Token::String(s))) = self.current_token() {
                            let s = s.clone();
                            self.advance();
                            s
                        } else if let Some(Ok(Token::Integer(n))) = self.current_token() {
                            let s = n.to_string();
                            self.advance();
                            s
                        } else if let Some(Ok(Token::Identifier(n))) = self.current_token() {
                            let n = n.clone();
                            self.advance();
                            n
                        } else {
                            String::new()
                        };
                        if let Some(Ok(Token::RParen)) = self.current_token() {
                            self.advance();
                        }
                        Some(val)
                    } else {
                        None
                    };
                    let value_expr = match &value {
                        Some(val) => Expr::String(val.clone()),
                        None => Expr::Bool(true),
                    };
                    mods.push(Annotation { name, value: value_expr, mode: AnnotationMode::Speculative });
                }
                Some(Ok(Token::Hash)) => {
                    // If this is #fuzz, stop — caller handles it specially
                    if self.peek_identifier().as_deref() == Some("fuzz") {
                        return Ok(mods);
                    }
                    self.advance();
                    let name = if let Some(Ok(Token::Identifier(n))) = self.current_token() {
                        let n = n.clone();
                        self.advance();
                        n
                    } else {
                        return Ok(mods);
                    };
                    let value = if let Some(Ok(Token::LParen)) = self.current_token() {
                        self.advance();
                        let val = if let Some(Ok(Token::String(s))) = self.current_token() {
                            let s = s.clone();
                            self.advance();
                            s
                        } else if let Some(Ok(Token::Integer(n))) = self.current_token() {
                            let s = n.to_string();
                            self.advance();
                            s
                        } else if let Some(Ok(Token::Identifier(n))) = self.current_token() {
                            let n = n.clone();
                            self.advance();
                            n
                        } else {
                            String::new()
                        };
                        if let Some(Ok(Token::RParen)) = self.current_token() {
                            self.advance();
                        }
                        Some(val)
                    } else {
                        None
                    };
                    let value_expr = match &value {
                        Some(val) => Expr::String(val.clone()),
                        None => Expr::Bool(true),
                    };
                    mods.push(Annotation { name, value: value_expr, mode: AnnotationMode::Advisory });
                }
                Some(Ok(Token::HashBang)) => {
                    self.advance();
                    let name = if let Some(Ok(Token::Identifier(n))) = self.current_token() {
                        let n = n.clone();
                        self.advance();
                        n
                    } else {
                        return Ok(mods);
                    };
                    let mut fallback = Vec::new();
                    while let Some(Ok(Token::Pipe)) = self.current_token() {
                        self.advance();
                        if let Some(Ok(Token::Identifier(n))) = self.current_token() {
                            let n = n.clone();
                            self.advance();
                            fallback.push(n);
                        } else {
                            break;
                        }
                    }
                    let value = if let Some(Ok(Token::LParen)) = self.current_token() {
                        self.advance();
                        let val = if let Some(Ok(Token::String(s))) = self.current_token() {
                            let s = s.clone();
                            self.advance();
                            s
                        } else if let Some(Ok(Token::Integer(n))) = self.current_token() {
                            let s = n.to_string();
                            self.advance();
                            s
                        } else if let Some(Ok(Token::Identifier(n))) = self.current_token() {
                            let n = n.clone();
                            self.advance();
                            n
                        } else {
                            String::new()
                        };
                        if let Some(Ok(Token::RParen)) = self.current_token() {
                            self.advance();
                        }
                        Some(val)
                    } else {
                        None
                    };
                    let value_expr = match &value {
                        Some(val) => Expr::String(val.clone()),
                        None => Expr::Bool(true),
                    };
                    mods.push(Annotation { name, value: value_expr, mode: AnnotationMode::Mandatory });
                }
                Some(Ok(Token::HashBracket)) => {
                    self.advance();
                    let scope = if let Some(Ok(Token::Identifier(n))) = self.current_token() {
                        let n = n.clone();
                        self.advance();
                        n
                    } else {
                        return Ok(mods);
                    };
                    self.expect(Token::RBracket)?;
                    let inner = self.parse_hashtag_modifiers()?;
                    for h in inner {
                        
                        mods.push(h);
                    }
                }
                // 2026-07-11: Phase 1A.0 — <~ (...) removed from hashtag modifiers.
                // TildeArrow is still valid in type bodies and defn/txn bodies.
                _ => return Ok(mods),
            }
        }
    }

    fn expect_type_identifier(&mut self) -> Result<String, crate::errors::SyntaxError> {
        let span = self.current_span().unwrap_or_else(Span::dummy);
        match self.current_token() {
            Some(Ok(Token::TypeFloat)) => {
                self.advance();
                Ok("Float".to_string())
            }
            Some(Ok(Token::TypeString)) => {
                self.advance();
                Ok("String".to_string())
            }
            Some(Ok(Token::TypeBool)) => {
                self.advance();
                Ok("Bool".to_string())
            }
            Some(Ok(Token::TypeVoid)) => {
                self.advance();
                Ok("Void".to_string())
            }
            Some(Ok(tok)) => Err(crate::errors::SyntaxError::UnexpectedToken {
                expected: "identifier".to_string(),
                found: Self::token_display(tok),
                span,
            }),
            Some(Err(_)) => Err(crate::errors::SyntaxError::InvalidStatement {
                reason: "Lexer error".to_string(),
                span,
            }),
            None => Err(crate::errors::SyntaxError::UnexpectedEOF {
                expected: "identifier".to_string(),
                span,
            }),
        }
    }

    /// Parse ops inside a `<:` subtype projection block: `{ FILTER(.x); COUNT; }` or `["pattern"]`
    /// Parse the source expression for a `<:` projection.
    /// Like parse_postfix, but stops before consuming `{` (struct literal) or `[` (match ops).
    fn parse_projection_source(&mut self) -> Result<Expr, SyntaxError> {
        // Start by parsing an identifier (not full parse_primary, which would
        // consume `{...}` as struct literal)
        let name = self.expect_identifier()?;
        let mut expr = Expr::Identifier(name);
        loop {
            match self.current_token() {
                // Leave `[` for parse_subtype_ops to handle as MATCH
                Some(Ok(Token::Dot)) => {
                    self.advance();
                    let field = self.expect_identifier()?;
                    expr = Expr::FieldAccess(Box::new(expr), field);
                }
                Some(Ok(Token::LParen)) => {
                    self.advance();
                    let mut args = Vec::new();
                    if let Some(Ok(Token::RParen)) = self.current_token() {
                        self.advance();
                    } else {
                        loop {
                            args.push(self.parse_expression()?);
                            if let Some(Ok(Token::Comma)) = self.current_token() {
                                self.advance();
                            } else {
                                self.expect(Token::RParen)?;
                                break;
                            }
                        }
                    }
                    let call_name = match &expr {
                        Expr::Identifier(n) => n.clone(),
                        _ => return self.spanned_err("Can only call named targets in projection source".to_string()),
                    };
                    expr = Expr::Call(call_name, args);
                }
                Some(Ok(Token::ColonGreaterThan)) => {
                    self.advance();
                    let target = self.parse_projection_target()?;
                    expr = Expr::Projection {
                        source: Box::new(expr),
                        target,
                    };
                }
                Some(Ok(Token::Lt))|Some(Ok(Token::Gt))|Some(Ok(Token::EqEq))
                | Some(Ok(Token::Ne))|Some(Ok(Token::Le))|Some(Ok(Token::Ge))
                | Some(Ok(Token::Plus))|Some(Ok(Token::Minus))|Some(Ok(Token::Star))
                | Some(Ok(Token::Slash))|Some(Ok(Token::Percent))
                | Some(Ok(Token::AndAnd))|Some(Ok(Token::OrOr))
                | Some(Ok(Token::Pipe))|Some(Ok(Token::BitXor))
                | Some(Ok(Token::Shl))|Some(Ok(Token::Shr))
                | Some(Ok(Token::Arrow))|Some(Ok(Token::ArrowLeft))
                | Some(Ok(Token::Colon))|Some(Ok(Token::Eq))
                | Some(Ok(Token::Semicolon))|Some(Ok(Token::Comma))
                | Some(Ok(Token::RBrace))|Some(Ok(Token::RBracket))
                | Some(Ok(Token::RParen)) => break,
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_subtype_ops(&mut self) -> Result<Vec<crate::ast::SubtypeOp>, SyntaxError> {
        // Check for string projection: `source["pattern"]`
        if let Some(Ok(Token::LBracket)) = self.current_token() {
            self.advance();
            let pattern = self.parse_expression()?;
            self.expect(Token::RBracket)?;
            return Ok(vec![crate::ast::SubtypeOp::Match(Box::new(pattern))]);
        }

        // Otherwise, expect `{` for collection projection
        self.expect(Token::LBrace)?;

        let mut ops = Vec::new();
        loop {
            // Check for closing brace
            if let Some(Ok(Token::RBrace)) = self.current_token() {
                self.advance();
                break;
            }

            let op = self.parse_single_subtype_op()?;
            ops.push(op);

            // Expect semicolon after each op
            if let Some(Ok(Token::Semicolon)) = self.current_token() {
                self.advance();
            } else if let Some(Ok(Token::RBrace)) = self.current_token() {
                // Allow last op without semicolon
                self.advance();
                break;
            } else {
                return self.spanned_err("Expected ';' or '}' after projection operation".to_string());
            }
        }

        Ok(ops)
    }

    /// Parse a single subtype operation keyword + args
    fn parse_single_subtype_op(&mut self) -> Result<crate::ast::SubtypeOp, SyntaxError> {
        let ident = self.expect_identifier()?;
        let upper = ident.to_uppercase();

        match upper.as_str() {
            "FILTER" => {
                self.expect(Token::LParen)?;
                let expr = self.parse_expression()?;
                self.expect(Token::RParen)?;
                Ok(crate::ast::SubtypeOp::Filter(Box::new(expr)))
            }
            "MAP" => {
                self.expect(Token::LParen)?;
                let expr = self.parse_expression()?;
                self.expect(Token::RParen)?;
                Ok(crate::ast::SubtypeOp::Map(Box::new(expr)))
            }
            "SORT" => {
                self.expect(Token::LParen)?;
                let expr = self.parse_expression()?;
                self.expect(Token::RParen)?;
                Ok(crate::ast::SubtypeOp::Sort(Box::new(expr)))
            }
            "LIMIT" => {
                self.expect(Token::LParen)?;
                let n = self.expect_integer()?;
                self.expect(Token::RParen)?;
                Ok(crate::ast::SubtypeOp::Limit(n as usize))
            }
            "SKIP" => {
                self.expect(Token::LParen)?;
                let n = self.expect_integer()?;
                self.expect(Token::RParen)?;
                Ok(crate::ast::SubtypeOp::Skip(n as usize))
            }
            "UNIQUE" => {
                Ok(crate::ast::SubtypeOp::Unique)
            }
            "JOIN" => {
                self.expect(Token::LParen)?;
                let other = self.parse_expression()?;
                self.expect(Token::Comma)?;
                let key = self.parse_expression()?;
                self.expect(Token::RParen)?;
                Ok(crate::ast::SubtypeOp::Join(Box::new(other), Box::new(key)))
            }
            "GROUP" => {
                self.expect(Token::LParen)?;
                let expr = self.parse_expression()?;
                self.expect(Token::RParen)?;
                Ok(crate::ast::SubtypeOp::Group(Box::new(expr)))
            }
            "COUNT" => {
                Ok(crate::ast::SubtypeOp::Count)
            }
            "SUM" => {
                self.expect(Token::LParen)?;
                let expr = self.parse_expression()?;
                self.expect(Token::RParen)?;
                Ok(crate::ast::SubtypeOp::Sum(Box::new(expr)))
            }
            "AVG" => {
                self.expect(Token::LParen)?;
                let expr = self.parse_expression()?;
                self.expect(Token::RParen)?;
                Ok(crate::ast::SubtypeOp::Avg(Box::new(expr)))
            }
            "MIN" => {
                self.expect(Token::LParen)?;
                let expr = self.parse_expression()?;
                self.expect(Token::RParen)?;
                Ok(crate::ast::SubtypeOp::Min(Box::new(expr)))
            }
            "MAX" => {
                self.expect(Token::LParen)?;
                let expr = self.parse_expression()?;
                self.expect(Token::RParen)?;
                Ok(crate::ast::SubtypeOp::Max(Box::new(expr)))
            }
            _ => {
                self.spanned_err(format!("Unknown projection operation '{}'. Expected FILTER, MAP, SORT, LIMIT, SKIP, UNIQUE, JOIN, GROUP, COUNT, SUM, AVG, MIN, or MAX", ident))
            }
        }
    }

    pub fn parse(&mut self) -> Result<Program, crate::errors::SyntaxError> {
        let mut reactor_speed: Option<u32> = None;
        let mut items = Vec::new();
        let mut file_attrs: Vec<crate::ast::Attribute> = Vec::new();

        // NEW: Check for file-level reactor @Hz declaration at start
        if let Some(Ok(Token::Identifier(name))) = self.current_token() {
            if name == "reactor" {
                self.advance(); // consume 'reactor'
                self.expect(Token::At)?;

                // Parse the speed number
                if let Some(Ok(Token::Integer(speed_num))) = self.current_token() {
                    let speed = *speed_num as u32;
                    self.advance();

                    // Optional 'Hz' (as identifier)
                    if let Some(Ok(Token::Identifier(hz))) = self.current_token() {
                        if hz == "Hz" {
                            self.advance();
                        }
                    }

                    // Validate speed
                    if speed == 0 {
                        return Err(SyntaxError::InvalidStatement {
                            reason: "Reactor speed must be positive (>0)".to_string(),
                            span: self.current_span().unwrap_or_else(Span::dummy),
                        });
                    }
                    if speed >= 10000 {
                        // Warn but allow
                        eprintln!("warning: Unusually high reactor speed @{}Hz", speed);
                    }

                    reactor_speed = Some(speed);
                    self.expect(Token::Semicolon)?;
                } else {
                    return Err(SyntaxError::UnexpectedToken {
                        expected: "numeric speed".to_string(),
                        found: self.fmt_current_token(),
                        span: self.current_span().unwrap_or_else(Span::dummy),
                    });
                }
            }
        }

        // Parse file-level directives in a loop: #!exit, #![...], #!pragma, #!...
        // Supports any ordering of these directives before the first top-level item.
        let mut exit_condition = None;
        let mut file_attrs = Vec::new();
        let mut out_pragmas = Vec::new();

        loop {
            match self.current_token() {
                Some(Ok(Token::HashBang)) => {
                    // Could be #!exit, #!out, or #! key(value) (legacy pragma)
                    if let Some(Ok(Token::Identifier(kw))) = self.peek.as_ref().map(|(t, _)| t) {
                        if kw == "exit" {
                            self.advance(); // consume #!
                            self.advance(); // consume "exit"
                            exit_condition = Some(Box::new(self.parse_expression()?));
                            self.expect(Token::Semicolon)?;
                            continue;
                        }
                        if kw == "out" {
                            self.advance(); // consume #!
                            self.advance(); // consume "out"
                            self.expect(Token::LParen)?;
                            let var_name = self.expect_identifier()?;
                            self.expect(Token::RParen)?;
                            self.expect(Token::Semicolon)?;
                            out_pragmas.push(var_name);
                            continue;
                        }
                        if kw == "assert" {
                            self.advance(); // consume #!
                            self.advance(); // consume "assert"
                            let pre = self.parse_expression()?;
                            self.expect(Token::Semicolon)?;
                            // Chain: fn_a -> fn_b -> fn_c
                            let mut chain = Vec::new();
                            loop {
                                let fn_name = self.expect_identifier()?;
                                chain.push(fn_name);
                                match self.current_token() {
                                    Some(Ok(Token::Arrow)) => { self.advance(); }
                                    Some(Ok(Token::Semicolon)) => { self.advance(); break; }
                                    _ => return self.spanned_err(
                                        "Expected '->' or ';' in assertion chain".to_string(),
                                    ),
                                }
                            }
                            items.push(TopLevel::Assertion {
                                pre,
                                chain,
                            });
                            continue;
                        }
                        if kw == "cfg" {
                            let (condition, cfg_items) = self.parse_cfg_guard()?;
                            items.push(TopLevel::Cfg(CfgGuard { condition, items: cfg_items }));
                            continue;
                        }
                    }
                    // Not #!exit, #!out, #!assert, or #!cfg — treat as legacy #! attribues
                    file_attrs.append(&mut self.parse_attributes()?);
                    continue;
                }
                Some(Ok(Token::HashBangBracket))
                | Some(Ok(Token::PragmaBang))
                | Some(Ok(Token::Pragma)) => {
                    file_attrs.append(&mut self.parse_attributes()?);
                    continue;
                }
                _ => break,
            }
        }

        // Process FFI state from file attributes
        let ffi_state = Self::process_ffi_attributes(&file_attrs);
        // Process dispatch mode from file attributes
        let dispatch_mode = Self::process_dispatch_attribute(&file_attrs);

        while self.current_token().is_some() {
            let item = self.parse_top_level()?;
            items.push(item);
        }
        Ok(Program {
            items,
            comments: self.comments.clone(),
            reactor_speed,
            attrs: file_attrs,
            ffi: ffi_state,
            strict_mode: self.strict_mode,
            dispatch_mode,
            exit_condition,
            out_pragmas,
            default_sig_modifier: None,
                watchdog_defaults: (None, None),
        })
    }

    /// Process file-level attributes to extract FFI state
    /// Example: #![ffi.c, bind("./c.toml"), import("./libc.a"), map("uint","uint32_t")]
    fn process_ffi_attributes(attrs: &[crate::ast::Attribute]) -> Option<FfiState> {
        let mut lang = None;
        let mut bind_path = None;
        let mut import_path = None;
        let mut global_maps = Vec::new();

        for attr in attrs {
            match attr.key.as_str() {
                k if k.starts_with("ffi.") => {
                    lang = Some(k[4..].to_string());
                }
                "bind" => {
                    if let Some(v) = &attr.value {
                        bind_path = Some(v.clone());
                    }
                }
                "import" => {
                    if let Some(v) = &attr.value {
                        import_path = Some(v.clone());
                    }
                }
                "map" => {
                    if let Some(v) = &attr.value {
                        // map("uint","uint32_t") -> ("uint", "uint32_t")
                        if let Some((from, to)) = Self::parse_map_pair(v) {
                            global_maps.push((from, to));
                        }
                    }
                }
                _ => {}
            }
        }

        lang.map(|l| FfiState {
            lang: l,
            bind_path,
            import_path,
            global_maps,
        })
    }

    /// Process file-level attributes to extract dispatch mode.
    /// Recognizes: `#pragma dispatch(parallel)` → `DispatchMode::Parallel`
    fn process_dispatch_attribute(attrs: &[crate::ast::Attribute]) -> DispatchMode {
        for attr in attrs {
            if attr.key == "dispatch" {
                if let Some(ref v) = attr.value {
                    if v == "parallel" {
                        return DispatchMode::Parallel;
                    }
                }
            }
        }
        DispatchMode::Sequential
    }

    /// Parse map("from","to") pair from attribute value
    fn parse_map_pair(value: &str) -> Option<(String, String)> {
        let inner = value.trim_matches('"');
        if let Some(comma_pos) = inner.find(',') {
            let from = inner[..comma_pos].trim().to_string();
            let to = inner[comma_pos + 1..].trim().to_string();
            return Some((from, to));
        }
        None
    }

    /// Parse a `#!cfg(condition)` guard. Returns the parsed condition and items.
    fn parse_cfg_guard(&mut self) -> Result<(CfgCondition, Vec<TopLevel>), SyntaxError> {
        // Consume #!
        self.advance();
        // Expect "cfg"
        let kw = self.expect_identifier()?;
        if kw != "cfg" {
            return self.spanned_err(format!("expected 'cfg' after '#!', got '{}'", kw));
        }
        self.expect(Token::LParen)?;
        let condition = self.parse_cfg_condition()?;
        self.expect(Token::RParen)?;

        // Parse the guarded item(s): either a single item or a block { }
        let items = if let Some(Ok(Token::LBrace)) = self.current_token() {
            self.advance();
            let mut parsed = Vec::new();
            while !matches!(self.current_token(), Some(Ok(Token::RBrace))) {
                if self.current_token().is_none() {
                    return self.spanned_err("unexpected EOF in #!cfg block".to_string());
                }
                // Handle nested #!cfg inside blocks
                if matches!(self.current_token(), Some(Ok(Token::HashBang)))
                    && self.peek_identifier().as_deref() == Some("cfg")
                {
                    let (cond, nested) = self.parse_cfg_guard()?;
                    parsed.push(TopLevel::Cfg(CfgGuard {
                        condition: cond,
                        items: nested,
                    }));
                } else {
                    parsed.push(self.parse_top_level()?);
                }
            }
            self.advance(); // consume }
            self.expect(Token::Semicolon)?;
            parsed
        } else {
            vec![self.parse_top_level()?]
        };

        Ok((condition, items))
    }

    /// Parse a cfg condition expression: `key == "val"`, `key != "val"`,
    /// `true`, `false`, `!expr`, `a && b`, `a || b`, `(expr)`.
    fn parse_cfg_condition(&mut self) -> Result<CfgCondition, SyntaxError> {
        self.parse_cfg_or()
    }

    fn parse_cfg_or(&mut self) -> Result<CfgCondition, SyntaxError> {
        let mut left = self.parse_cfg_and()?;
        while let Some(Ok(Token::OrOr)) = self.current_token() {
            self.advance();
            let right = self.parse_cfg_and()?;
            left = CfgCondition::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_cfg_and(&mut self) -> Result<CfgCondition, SyntaxError> {
        let mut left = self.parse_cfg_not()?;
        while let Some(Ok(Token::AndAnd)) = self.current_token() {
            self.advance();
            let right = self.parse_cfg_not()?;
            left = CfgCondition::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_cfg_not(&mut self) -> Result<CfgCondition, SyntaxError> {
        if let Some(Ok(Token::Not)) = self.current_token() {
            self.advance();
            let inner = self.parse_cfg_primary()?;
            Ok(CfgCondition::Not(Box::new(inner)))
        } else {
            self.parse_cfg_primary()
        }
    }

    fn parse_cfg_primary(&mut self) -> Result<CfgCondition, SyntaxError> {
        match self.current_token() {
            Some(Ok(Token::BoolTrue)) => {
                self.advance();
                Ok(CfgCondition::Bool(true))
            }
            Some(Ok(Token::BoolFalse)) => {
                self.advance();
                Ok(CfgCondition::Bool(false))
            }
            Some(Ok(Token::LParen)) => {
                self.advance();
                let inner = self.parse_cfg_condition()?;
                self.expect(Token::RParen)?;
                Ok(inner)
            }
            Some(Ok(Token::Identifier(key))) => {
                let key = key.clone();
                self.advance();
                match self.current_token() {
                    Some(Ok(Token::EqEq)) => {
                        self.advance();
                        let val = self.expect_string()?;
                        Ok(CfgCondition::Eq(key, val))
                    }
                    Some(Ok(Token::Ne)) => {
                        self.advance();
                        let val = self.expect_string()?;
                        Ok(CfgCondition::Ne(key, val))
                    }
                    _ => self.spanned_err(format!(
                        "expected '==' or '!=' in cfg condition after '{}'", key
                    )),
                }
            }
            Some(Ok(tok)) => self.spanned_err(format!(
                "unexpected token in cfg condition: {}",
                Self::token_display(&tok)
            )),
            _ => self.spanned_err("unexpected EOF in cfg condition".to_string()),
        }
    }

    fn expect_string(&mut self) -> Result<String, SyntaxError> {
        if let Some(Ok(Token::String(s))) = self.current_token() {
            let s = s.clone();
            self.advance();
            Ok(s)
        } else {
            self.spanned_err("expected a string literal in cfg condition".to_string())
        }
    }

    fn peek_identifier(&self) -> Option<String> {
        self.peek.as_ref().and_then(|(t, _)| {
            match t {
                Ok(Token::Identifier(s)) => Some(s.clone()),
                _ => None,
            }
        })
    }

    fn parse_top_level(&mut self) -> Result<TopLevel, SyntaxError> {
        // Handle #!cfg guards — must come before all other parsing
        if matches!(self.current_token(), Some(Ok(Token::HashBang)))
            && self.peek_identifier().as_deref() == Some("cfg")
        {
            let (condition, items) = self.parse_cfg_guard()?;
            return Ok(TopLevel::Cfg(CfgGuard { condition, items }));
        }

        let is_sed = matches!(self.current_token(), Some(Ok(Token::Sed)));
        if is_sed {
            self.advance();
        }

        let span = self.current_span().unwrap_or_else(Span::dummy);
        if self.current_token().is_none() {
            return Err(SyntaxError::UnexpectedEOF {
                expected: "top level item".to_string(),
                span,
            });
        }

        // Parse item-level attributes if present
        let attrs = if matches!(self.current_token(), Some(Ok(Token::HashBracket)))
            || matches!(self.current_token(), Some(Ok(Token::Pragma)))
        {
            self.parse_attributes()?
        } else {
            Vec::new()
        };

        // Collect #fuzz cases before any modifier parsing
        let mut fuzz_cases: Vec<crate::ast::FuzzCase> = Vec::new();
        while self.lookahead_is_fuzz() {
            fuzz_cases.push(self.parse_fuzz_case()?);
        }

        // Parse hashtag modifiers (#assume_event, #assume_shape, etc.)
        let modifiers = if matches!(self.current_token(), Some(Ok(Token::Hash))) {
            self.parse_hashtag_modifiers()?
        } else {
            Vec::new()
        };

        // Collect #fuzz cases that followed modifiers
        while self.lookahead_is_fuzz() {
            fuzz_cases.push(self.parse_fuzz_case()?);
        }

        // Check for #test("group") modifiers
        let test_groups: Vec<String> = modifiers.iter()
            .filter(|h| h.name == "test")
            .filter_map(|h| h.string_value())
            .collect();

        // Helper to wrap an item in TopLevel::Test if #test modifiers are present
        let wrap_test = |item: TopLevel, groups: &[String]| -> TopLevel {
            if groups.is_empty() { item }
            else { TopLevel::Test { item: Box::new(item), groups: groups.to_vec() } }
        };

        let cur_tok = self.current_token().cloned();
        let result = match cur_tok {
            // (wasm) import / (circt) import / (javascript) import — try parsing target-prefixed import
            Some(Ok(Token::LParen)) => {
                match self.try_parse_import_target() {
                    Some(item) => Ok(wrap_test(item, &test_groups)),
                    // Not an import target — fall through to exec statement parser
                    None => match self.try_parse_exec_statement() {
                        Some(stmt) => Ok(TopLevel::Statement(Box::new(stmt))),
                        None => Err(SyntaxError::UnexpectedToken {
                            expected: "top-level declaration or statement".to_string(),
                            found: "(".to_string(),
                            span,
                        }),
                    },
                }
            }
            Some(Ok(Token::Import)) => {
                self.parse_import().map(|item| wrap_test(item, &test_groups))
            }
            Some(Ok(Token::Sig)) => {
                let sig = self.parse_signature()?;
                Ok(wrap_test(TopLevel::Signature(sig), &test_groups))
            }
            Some(Ok(Token::Let)) => {
                let mut state = self.parse_state_decl()?;
                state.attrs = attrs;
                Ok(wrap_test(TopLevel::StateDecl(state), &test_groups))
            }
            Some(Ok(Token::Const)) => {
                // Check if this is const trg or a regular constant
                if let Some(Ok(Token::Trg)) = self.peek_token() {
                    self.advance(); // consume const
                    self.advance(); // consume trg
                    let trg = self.parse_trigger_body(true)?;
                    Ok(wrap_test(TopLevel::Trigger(trg), &test_groups))
                } else {
                    let constant = self.parse_constant()?;
                    Ok(wrap_test(TopLevel::Constant(constant), &test_groups))
                }
            }
            Some(Ok(Token::Sync)) => {
                self.advance();
                self.expect(Token::LParen)?;
                let mut domains = Vec::new();
                loop {
                    let domain = self.expect_identifier()?;
                    domains.push(domain);
                    if let Some(Ok(Token::RParen)) = self.current_token() {
                        self.advance();
                        break;
                    }
                    self.expect(Token::Comma)?;
                }
                let item = self.parse_top_level()?;
                Ok(wrap_test(TopLevel::SyncGroup {
                    domains,
                    item: Box::new(item),
                }, &test_groups))
            }
            Some(Ok(Token::Txn)) | Some(Ok(Token::Rct)) | Some(Ok(Token::Async)) => {
                let mut txn = self.parse_transaction()?;
                txn.modifiers = modifiers;
                Ok(wrap_test(TopLevel::Transaction(txn), &test_groups))
            }

            // ── Phase 4: `export` keyword ─────────────────────
            // `export defn` / `export("name") defn` replaces `#export` annotation.
            // `export txn` for callable (non-reactive) transactions.
            Some(Ok(Token::Export)) => {
                self.advance(); // consume export keyword
                // Parse optional ("name") for explicit export symbol
                let export_name = if let Some(Ok(Token::LParen)) = self.current_token() {
                    self.advance();
                    let name = self.expect_string()?;
                    self.expect(Token::RParen)?;
                    name
                } else {
                    String::new()
                };
                let export_annotation = Annotation {
                    name: "export".to_string(),
                    value: if export_name.is_empty() {
                        Expr::Bool(true)
                    } else {
                        Expr::String(export_name)
                    },
                    mode: AnnotationMode::Mandatory,
                };
                // Reject `export rct txn` — reactive txns have no single-entry FFI.
                if matches!(self.current_token(), Some(Ok(Token::Rct))) {
                    return self.spanned_err(
                        "export rct txn is not supported; use `export txn` for callable transactions".to_string()
                    );
                }
                match self.current_token().cloned() {
                    Some(Ok(Token::Defn)) => {
                        let mut defn = self.parse_definition()?;
                        let mut merged = modifiers.clone();
                        merged.push(export_annotation);
                        defn.modifiers = merged;
                        Ok(wrap_test(TopLevel::Definition(defn), &test_groups))
                    }
                    Some(Ok(Token::Txn)) | Some(Ok(Token::Async)) => {
                        let mut txn = self.parse_transaction()?;
                        let mut merged = modifiers.clone();
                        merged.push(export_annotation);
                        txn.modifiers = merged;
                        Ok(wrap_test(TopLevel::Transaction(txn), &test_groups))
                    }
                    Some(Ok(tok)) => {
                        return self.spanned_err(format!(
                            "expected 'defn' or 'txn' after export keyword, got '{}'", tok
                        ));
                    }
                    Some(Err(_)) => {
                        return self.spanned_err(
                            "expected 'defn' or 'txn' after export keyword".to_string()
                        );
                    }
                    None => {
                        return Err(SyntaxError::UnexpectedEOF {
                            expected: "defn or txn after export keyword".to_string(),
                            span,
                        });
                    }
                }
            }

            Some(Ok(Token::Defn)) => {
                let mut defn = self.parse_definition()?;
                defn.modifiers = modifiers;
                Ok(wrap_test(TopLevel::Definition(defn), &test_groups))
            }
            Some(Ok(Token::Trg)) => {
                self.advance();
                let trg = self.parse_trigger_body(false)?;
                // Check if parse_trigger_body detected a cell binding
                if let Some((binding_name, cell_name, port, ty)) = self.pending_cell_binding.take() {
                    Ok(wrap_test(TopLevel::TriggerBinding {
                        name: binding_name,
                        ty,
                        instance: Expr::Identifier(cell_name),
                        port,
                        modifiers: vec![],
                    }, &test_groups))
                } else {
                    // Hardware-addressed triggers (Explicit with non-zero address) require const.
                    if let crate::ast::LinkRef::Explicit(addr) = trg.address {
                        if addr != 0 && !trg.is_const {
                            return self.spanned_err("hardware-addressed triggers must be declared 'const trg'".to_string());
                        }
                    }
                    Ok(wrap_test(TopLevel::Trigger(trg), &test_groups))
                }
            }
            Some(Ok(Token::Frgn)) => {
                let frgn_binding = self.parse_frgn_binding()?;
                Ok(wrap_test(frgn_binding, &test_groups))
            }
            Some(Ok(Token::FrgnBang)) => {
                let frgn_binding = self.parse_frgn_binding()?;
                Ok(wrap_test(frgn_binding, &test_groups))
            }
            Some(Ok(Token::Syscall)) => {
                let frgn_binding = self.parse_frgn_binding()?;
                Ok(wrap_test(frgn_binding, &test_groups))
            }
            Some(Ok(Token::SyscallBang)) => {
                let frgn_binding = self.parse_frgn_binding()?;
                Ok(wrap_test(frgn_binding, &test_groups))
            }
            Some(Ok(Token::Inop)) => {
                let section = Self::extract_section(&modifiers);
                let inop = self.parse_inop_decl(false, section)?;
                Ok(wrap_test(TopLevel::Inop(inop), &test_groups))
            }
            Some(Ok(Token::InopBang)) => {
                let section = Self::extract_section(&modifiers);
                let inop = self.parse_inop_decl(true, section)?;
                Ok(wrap_test(TopLevel::Inop(inop), &test_groups))
            }
            Some(Ok(Token::Meld)) => {
                let meld = self.parse_meld_decl()?;
                Ok(wrap_test(TopLevel::Meld(meld), &test_groups))
            }
            Some(Ok(Token::Reg)) => {
                let resource = self.parse_resource()?;
                Ok(wrap_test(resource, &test_groups))
            }
            Some(Ok(Token::Struct)) => {
                let struct_def = self.parse_struct()?;
                Ok(wrap_test(TopLevel::Struct(struct_def), &test_groups))
            }
            Some(Ok(Token::Rstruct)) => {
                let rstruct_def = self.parse_rstruct()?;
                Ok(wrap_test(TopLevel::RStruct(rstruct_def), &test_groups))
            }
            Some(Ok(Token::Enum)) => {
                let enum_def = self.parse_enum()?;
                Ok(wrap_test(TopLevel::Enum(enum_def), &test_groups))
            }
            Some(Ok(Token::Cell)) => {
                self.advance(); // consume `cell`
                // Check for cell! (persistent) vs cell (auto-terminating)
                let is_persistent = if let Some(Ok(Token::Not)) = self.current_token() {
                    self.advance(); // consume `!`
                    true
                } else {
                    false
                };
                let cell_def = self.parse_cell_definition(is_persistent)?;
                Ok(wrap_test(TopLevel::Cell(Box::new(cell_def)), &test_groups))
            }
            Some(Ok(Token::Template)) => {
                let (name, params, return_type, body) = self.parse_template_def()?;
                Ok(wrap_test(TopLevel::TemplateDef { name, params, return_type, body }, &test_groups))
            }
            Some(Ok(Token::Macro)) => {
                let (name, params, return_type, body) = self.parse_macro_def()?;
                Ok(wrap_test(TopLevel::MacroDef { name, params, return_type, body }, &test_groups))
            }
            Some(Ok(Token::Type)) => {
                let type_def = self.parse_type_def()?;
                Ok(wrap_test(TopLevel::TypeDef(Box::new(type_def)), &test_groups))
            }
            Some(Ok(Token::Render)) => {
                let render_block = self.parse_render_block()?;
                Ok(wrap_test(TopLevel::RenderBlock(render_block), &test_groups))
            }
            Some(Ok(tok)) => {
                // Try to parse as a top-level executable statement
                match self.try_parse_exec_statement() {
                    Some(stmt) => Ok(TopLevel::Statement(Box::new(stmt))),
                    None => Err(SyntaxError::UnexpectedToken {
                        expected: "top-level declaration or statement".to_string(),
                        found: Self::token_display(&tok),
                        span,
                    }),
                }
            }
            Some(Err(_)) => Err(SyntaxError::InvalidStatement {
                reason: "Lexer error at top level".to_string(),
                span,
            }),
            None => Err(SyntaxError::UnexpectedEOF {
                expected: "top-level declaration or statement".to_string(),
                span,
            }),
        };

        // Wrap in TopLevel::Fuzzed if fuzz cases are present.
        // If the item is already wrapped in Test (from wrap_test), insert
        // Fuzzed inside Test so the ordering is Test { Fuzzed { Defn } }.
        let result = result.map(|item| {
            if fuzz_cases.is_empty() {
                item
            } else {
                match item {
                    TopLevel::Test { item: inner, groups } => {
                        TopLevel::Test {
                            item: Box::new(TopLevel::Fuzzed {
                                item: inner,
                                cases: fuzz_cases,
                            }),
                            groups,
                        }
                    }
                    other => TopLevel::Fuzzed {
                        item: Box::new(other),
                        cases: fuzz_cases,
                    },
                }
            }
        });

        if is_sed {
            if let Ok(ref item) = result {
                let name = match item {
                    TopLevel::Definition(d) => Some(d.name.clone()),
                    TopLevel::Transaction(t) => Some(t.name.clone()),
                    TopLevel::Trigger(t) => Some(t.name.clone()),
                    TopLevel::TriggerBinding { name, .. } => Some(name.clone()),
                    TopLevel::StateDecl(s) => Some(s.name.clone()),
                    TopLevel::Struct(s) => Some(s.name.clone()),
                    TopLevel::Enum(e) => Some(e.name.clone()),
                    TopLevel::Constant(c) => Some(c.name.clone()),
                    TopLevel::RStruct(r) => Some(r.name.clone()),
                    TopLevel::Inop(i) => Some(i.name.clone()),
                    TopLevel::Cell(c) => Some(c.name.clone()),
                    _ => None,
                };
                if let Some(name) = name {
                    self.sed_item_names.push(name);
                }
            }
        }

        result
    }

    /// Try to parse a top-level executable statement.
    /// Returns `None` if the current token doesn't look like a statement start.
    fn try_parse_exec_statement(&mut self) -> Option<Statement> {
        let saved_current = self.current.clone();
        let saved_peek = self.peek.clone();
        let saved_pos = self.pos;
        let saved_line = self.current_line;
        match self.parse_statement() {
            Ok(stmt) => Some(stmt),
            Err(_) => {
                self.current = saved_current;
                self.peek = saved_peek;
                self.pos = saved_pos;
                self.current_line = saved_line;
                None
            }
        }
    }

    /// Check if the current token starts a `#fuzz(...)` declaration.
    fn lookahead_is_fuzz(&mut self) -> bool {
        if !matches!(self.current_token(), Some(Ok(Token::Hash))) {
            return false;
        }
        self.peek_identifier().as_deref() == Some("fuzz")
            && matches!(self.peek_token2(), Some(Ok(Token::LParen)))
    }

    /// Parse `#fuzz(param = expr, ...) -> expected_expr ;`
    fn parse_fuzz_case(&mut self) -> Result<crate::ast::FuzzCase, SyntaxError> {
        let span = self.current_span().unwrap_or_else(crate::errors::Span::dummy);
        self.advance(); // consume #
        self.advance(); // consume fuzz identifier
        self.expect(Token::LParen)?;
        let mut bindings = Vec::new();
        loop {
            let ident = self.expect_identifier()?;
            self.expect(Token::Eq)?;
            let expr = self.parse_expression()?;
            bindings.push((ident, expr));
            if let Some(Ok(Token::RParen)) = self.current_token() {
                self.advance();
                break;
            }
            self.expect(Token::Comma)?;
        }
        // parse -> expected
        self.expect(Token::Arrow)?;
        let expected = self.parse_expression()?;
        // expect ; terminator
        self.expect(Token::Semicolon)?;
        Ok(crate::ast::FuzzCase {
            bindings,
            expected,
            span: Some(span),
        })
    }

    /// Try to parse a `(wasm) import` / `(circt) import` / etc. target-prefixed import.
    /// Returns None if the current token is not a valid import target prefix.
    fn try_parse_import_target(&mut self) -> Option<TopLevel> {
        // Current must be `(`, peek must be a target keyword
        if !matches!(self.current_token(), Some(Ok(Token::LParen))) {
            return None;
        }
        let peek_kw = match self.peek_token() {
            Some(Ok(Token::Identifier(kw))) => kw.clone(),
            _ => return None,
        };
        if !matches!(peek_kw.as_str(), "wasm" | "native" | "circt" | "javascript") {
            return None;
        }

        // Create a temporary lexer from source after current token
        let after_open_pos = self.current.as_ref().map(|(_, s)| s.end).unwrap_or(self.pos);
        let sub_source = &self.source[after_open_pos..];
        let mut temp = Parser::new(sub_source);
        if !matches!(temp.current_token(), Some(Ok(Token::Identifier(kw))) if matches!(kw.as_str(), "wasm" | "native" | "circt" | "javascript")) {
            return None;
        }
        temp.advance(); // past keyword
        if !matches!(temp.current_token(), Some(Ok(Token::RParen))) {
            return None;
        }
        temp.advance(); // past )
        if !matches!(temp.current_token(), Some(Ok(Token::Import))) {
            return None;
        }
        // Confirmed: valid import target — let parse_import handle it
        self.parse_import().ok()
    }

    fn parse_import(&mut self) -> Result<TopLevel, SyntaxError> {
        // Parse optional import target: (wasm), (circt), (native), (javascript)
        let mut target = crate::ast::ImportTarget::Native;
        if let Some(Ok(Token::LParen)) = self.current_token() {
            self.advance();
            if let Some(Ok(Token::Identifier(kw))) = self.current_token() {
                match kw.as_str() {
                    "wasm" | "native" | "circt" | "javascript" => {
                        target = match kw.as_str() {
                            "wasm" => crate::ast::ImportTarget::Wasm,
                            "circt" => crate::ast::ImportTarget::Circt,
                            "javascript" => crate::ast::ImportTarget::Javascript,
                            _ => crate::ast::ImportTarget::Native,
                        };
                        self.advance();
                        self.expect(Token::RParen)?;
                    }
                    _ => {
                        return self.spanned_err(
                            format!("Unknown import target: '({})'. Valid: (wasm), (circt), (native), (javascript)", kw)
                        );
                    }
                }
            } else {
                return self.spanned_err("Expected target name after '(' in import target specifier. Use: (wasm) import".to_string());
            }
        }

        self.expect(Token::Import)?;

        // import# — compiler-relative path resolution (resolved against BRIEF_STDLIB_PATH)
        let is_magic = if matches!(self.current_token(), Some(Ok(Token::Hash))) {
            self.advance();
            true
        } else {
            false
        };

        let mut items = if let Some(Ok(Token::LBrace)) = self.current_token() {
            self.advance();
            let mut items = Vec::new();
            loop {
                let name_result = self.expect_identifier();
                match name_result {
                    Ok(name) => {
                        let alias = if let Some(Ok(Token::As)) = self.current_token() {
                            self.advance();
                            Some(self.expect_identifier()?)
                        } else {
                            None
                        };
                        items.push(ImportItem { name, alias });
                        if let Some(Ok(Token::Comma)) = self.current_token() {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            self.expect(Token::RBrace)?;
            items
        } else {
            Vec::new()
        };

        let path = if let Some(Ok(Token::From)) = self.current_token() {
            self.advance();
            if let Some(Ok(Token::String(s))) = self.current_token() {
                let s = s.clone();
                self.advance();
                let trimmed = s.trim_start_matches("./");
                let parts: Vec<String> = trimmed.split('/').map(String::from).collect();
                parts
            } else {
                return self.spanned_err(
                    "Expected quoted string path after 'from'. Use: import { foo } from \"path/to/module\";"
                        .to_string(),
                );
            }
        } else if let Some(Ok(Token::String(s))) = self.current_token() {
            let s = s.clone();
            self.advance();
            let trimmed = s.trim_start_matches("./");
            let parts: Vec<String> = trimmed.split('/').map(String::from).collect();

            if let Some(Ok(Token::As)) = self.current_token() {
                self.advance();
                let name = self.expect_identifier()?;
                items.push(ImportItem { name, alias: None });
            }

            parts
        } else if let Some(Ok(Token::Identifier(_))) = self.current_token() {
            return self.spanned_err(
                "Bare identifier paths are no longer supported. Use quoted string: import \"path/to/module\";"
                    .to_string(),
            );
        } else {
            Vec::new()
        };

        let last = path.last().map(|s| s.as_str()).unwrap_or("");
        let first = path.first().map(|s| s.as_str()).unwrap_or("");

        // Only apply LinkLanguage detection for paths starting with "link/"
        if first == "link" {
            let source_lang = if last.ends_with(".c") {
                crate::ast::LinkLanguage::C
            } else if last.ends_with(".cpp") || last.ends_with(".cc") || last.ends_with(".cxx") {
                crate::ast::LinkLanguage::Cpp
            } else if last.ends_with(".rs") {
                crate::ast::LinkLanguage::Rust
            } else if last.ends_with(".zig") {
                crate::ast::LinkLanguage::Zig
            } else if last.ends_with(".py") {
                crate::ast::LinkLanguage::Python
            } else if last.ends_with(".java") {
                crate::ast::LinkLanguage::Java
            } else if last.ends_with(".ts") || last.ends_with(".as.ts") {
                crate::ast::LinkLanguage::AssemblyScript
            } else if last.ends_with(".bc") {
                crate::ast::LinkLanguage::Bitcode
            } else if last.ends_with(".o") || last.ends_with(".a") {
                crate::ast::LinkLanguage::Object
            } else {
                return self.spanned_err(
                    format!("Unsupported link dependency extension: '{}'. Supported: .c, .cpp, .rs, .zig, .py, .java, .ts, .bc, .o, .a", last)
                );
            };

            if !items.is_empty() {
                return self.spanned_err(
                    "Link dependencies do not support named imports. Use: import \"link/path.ext\";"
                        .to_string(),
                );
            }
            self.expect(Token::Semicolon)?;
            return Ok(TopLevel::LinkDependency(crate::ast::LinkDependency {
                path: path.join("/"),
                source_lang,
            }));
        }

        self.expect(Token::Semicolon)?;
        Ok(TopLevel::Import(Import { items, path, is_magic, target }))
    }

    fn parse_signature(&mut self) -> Result<Signature, SyntaxError> {
        use crate::ast::SigModifier;

        self.expect(Token::Sig)?;

        // Parse optional #out / #inline modifier
        let modifier = if matches!(self.current_token(), Some(Ok(Token::Hash))) {
            self.advance();
            if let Some(Ok(Token::Identifier(kw))) = self.current_token() {
                match kw.as_str() {
                    "out" => {
                        self.advance();
                        Some(SigModifier::Out)
                    }
                    "inline" => {
                        self.advance();
                        Some(SigModifier::Inline)
                    }
                    "export" => {
                        // #export or #export("symbol_name")
                        self.advance();
                        let export_name = if matches!(self.current_token(), Some(Ok(Token::LParen))) {
                            self.advance();
                            let name = match self.current_token() {
                                Some(Ok(Token::String(s))) => {
                                    let s = s.clone();
                                    self.advance();
                                    Some(s)
                                }
                                _ => return self.spanned_err("Expected string literal for export name".to_string()),
                            };
                            self.expect(Token::RParen)?;
                            name
                        } else {
                            None
                        };
                        Some(SigModifier::Export(export_name))
                    }
                    _ => return self.spanned_err("Expected 'out', 'inline', or 'export' after '#' in sig".to_string()),
                }
            } else {
                return self.spanned_err("Expected 'out', 'inline', or 'export' after '#' in sig".to_string());
            }
        } else {
            None
        };

        let name = self.expect_identifier()?;

        // Parse parameter list: (name: Type, ...)
        self.expect(Token::LParen)?;
        let mut params = Vec::new();
        if !matches!(self.current_token(), Some(Ok(Token::RParen))) {
            loop {
                let param_name = self.expect_identifier()?;
                self.expect(Token::Colon)?;
                let param_type = self.parse_type()?;
                params.push((param_name, param_type));

                if let Some(Ok(Token::Comma)) = self.current_token() {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        self.expect(Token::RParen)?;

        self.expect(Token::Arrow)?;

        // Parse output type structure (supports | , [] named slots)
        let output_type = self.parse_output_type_structure()?;
        let result_type = ResultType::Projection(output_type.all_types());

        // Parse optional defn binding: = defn_name
        let bound_defn = if let Some(Ok(Token::Eq)) = self.current_token() {
            self.advance();
            Some(self.expect_identifier()?)
        } else {
            None
        };

        // Parse optional from source clause: from source_name
        let source = if let Some(Ok(Token::From)) = self.current_token() {
            self.advance();
            if let Some(Ok(Token::String(s))) = self.current_token() {
                let loc = s.clone();
                self.advance();
                Some(loc)
            } else {
                let mut path = Vec::new();
                path.push(self.expect_identifier()?);
                while let Some(Ok(Token::Dot)) = self.current_token() {
                    self.advance();
                    path.push(self.expect_identifier()?);
                }
                Some(path.join("."))
            }
        } else {
            None
        };

        let alias = if let Some(Ok(Token::As)) = self.current_token() {
            self.advance();
            Some(self.expect_identifier()?)
        } else {
            None
        };

        self.expect(Token::Semicolon)?;
        Ok(Signature {
            name,
            params,
            result_type,
            source,
            alias,
            bound_defn,
            modifier,
            output_type: Some(output_type),
        })
    }

    /// Convert a type name string to a Type
    fn string_to_type(&self, type_name: &str) -> Result<Type, SyntaxError> {
        match type_name {
            "String" => Ok(Type::Custom("String".to_string())),
            "Int" => Ok(Type::Custom("Int".to_string())),
            "UInt" => Ok(Type::Custom("UInt".to_string())),
            "Float" => Ok(Type::Custom("Float".to_string())),
            "Bool" => Ok(Type::Custom("Bool".to_string())),
            "void" => Ok(Type::Void),
            "Data" => Ok(Type::Custom("Data".to_string())),
            // Shorthand sized types (syntactic sugar for Int/UInt @/xN)
            "u8" => Ok(Type::Custom("UInt8".to_string())),
            "i8" => Ok(Type::Custom("Int8".to_string())),
            "u16" => Ok(Type::Custom("UInt16".to_string())),
            "i16" => Ok(Type::Custom("Int16".to_string())),
            "u32" => Ok(Type::Custom("UInt32".to_string())),
            "i32" => Ok(Type::Custom("Int32".to_string())),
            "u64" => Ok(Type::Custom("UInt".to_string())),
            "i64" => Ok(Type::Custom("Int".to_string())),
            other => Ok(Type::Custom(other.to_string())),
        }
    }

    /// Parse a type name token (handles TypeUInt, Err, Identifier, etc.)
    fn parse_type_name_token(&mut self) -> Result<String, SyntaxError> {
        match self.current_token() {
            Some(Ok(Token::Identifier(s))) => {
                let s = (*s).to_string();
                self.advance();
                Ok(s)
            }
            Some(Ok(Token::TypeInt)) => { self.advance(); Ok("Int".to_string()) }
            Some(Ok(Token::TypeUInt)) => { self.advance(); Ok("UInt".to_string()) }
            Some(Ok(Token::TypeSigned)) => { self.advance(); Ok("Int".to_string()) }
            Some(Ok(Token::TypeUSgn)) => { self.advance(); Ok("UInt".to_string()) }
            Some(Ok(Token::TypeUnsigned)) => { self.advance(); Ok("UInt".to_string()) }
            Some(Ok(Token::TypeFloat)) => { self.advance(); Ok("Float".to_string()) }
            Some(Ok(Token::TypeString)) => { self.advance(); Ok("String".to_string()) }
            Some(Ok(Token::TypeBool)) => { self.advance(); Ok("Bool".to_string()) }
            Some(Ok(Token::TypeVoid)) => { self.advance(); Ok("void".to_string()) }
            Some(Ok(Token::TypeData)) => { self.advance(); Ok("Data".to_string()) }
            // Shorthand sized types
            Some(Ok(Token::TypeU8)) => { self.advance(); Ok("u8".to_string()) }
            Some(Ok(Token::TypeI8)) => { self.advance(); Ok("i8".to_string()) }
            Some(Ok(Token::TypeU16)) => { self.advance(); Ok("u16".to_string()) }
            Some(Ok(Token::TypeI16)) => { self.advance(); Ok("i16".to_string()) }
            Some(Ok(Token::TypeU32)) => { self.advance(); Ok("u32".to_string()) }
            Some(Ok(Token::TypeI32)) => { self.advance(); Ok("i32".to_string()) }
            Some(Ok(Token::TypeU64)) => { self.advance(); Ok("u64".to_string()) }
            Some(Ok(Token::TypeI64)) => { self.advance(); Ok("i64".to_string()) }
            Some(Ok(Token::TypeInt8)) => { self.advance(); Ok("Int8".to_string()) }
            Some(Ok(Token::TypeInt16)) => { self.advance(); Ok("Int16".to_string()) }
            Some(Ok(Token::TypeInt32)) => { self.advance(); Ok("Int32".to_string()) }
            Some(Ok(Token::TypeInt64)) => { self.advance(); Ok("Int64".to_string()) }
            Some(Ok(Token::TypeUInt8)) => { self.advance(); Ok("UInt8".to_string()) }
            Some(Ok(Token::TypeUInt16)) => { self.advance(); Ok("UInt16".to_string()) }
            Some(Ok(Token::TypeUInt32)) => { self.advance(); Ok("UInt32".to_string()) }
            Some(Ok(Token::TypeUInt64)) => { self.advance(); Ok("UInt64".to_string()) }
            Some(Ok(Token::TypeFloat32)) => { self.advance(); Ok("Float32".to_string()) }
            Some(Ok(Token::TypeF32)) => { self.advance(); Ok("F32".to_string()) }
            Some(Ok(Token::TypeFloat64)) => { self.advance(); Ok("Float64".to_string()) }
            Some(Ok(Token::TypeF64)) => { self.advance(); Ok("F64".to_string()) }
            Some(Ok(Token::TypeDouble)) => { self.advance(); Ok("Double".to_string()) }
            Some(Ok(Token::Err)) => { self.advance(); Ok("Err".to_string()) }
            _ => self.spanned_err(format!("Expected type name, found {}", self.fmt_current_token())),
        }
    }

    /// Parse a foreign function binding declaration
    /// New Syntax:
    ///   frgn name @ address (param: Type) -> Result<T, E>;
    ///   frgn name (param: Type) -> Result<T, E>;       // compiler picks address from profile
    ///   frgn! name @ address (param: Type);            // fire-and-forget (Void return)
    ///   frgn! name (param: Type);                       // fire-and-forget, compiler picks address
    fn parse_frgn_binding(&mut self) -> Result<TopLevel, SyntaxError> {
        use crate::ast::{ForeignSignature, ForeignTarget, ResultType, FfiKind};

        // Handle all frgn/frgn!/syscall/syscall! tokens
        let ffi_kind = match self.current_token() {
            Some(Ok(Token::Frgn)) => {
                self.advance();
                FfiKind::Frgn
            }
            Some(Ok(Token::FrgnBang)) => {
                self.advance();
                FfiKind::FrgnBang
            }
            Some(Ok(Token::Syscall)) => {
                self.advance();
                FfiKind::Syscall
            }
            Some(Ok(Token::SyscallBang)) => {
                self.advance();
                FfiKind::SyscallBang
            }
            _ => return self.spanned_err("Expected 'frgn', 'frgn!', 'syscall', or 'syscall!'".to_string()),
        };

        // Parse optional #out modifier — marks function as having observable output
        let mut is_out = matches!(ffi_kind, FfiKind::FrgnBang | FfiKind::SyscallBang);
        if matches!(self.current_token(), Some(Ok(Token::Hash))) {
            self.advance();
            if let Some(Ok(Token::Identifier(kw))) = self.current_token() {
                if kw == "out" {
                    self.advance();
                    is_out = true;
                }
            }
        }

        let name = self.expect_identifier()?;

        // Parse optional @ address
        let address = if matches!(self.current_token(), Some(Ok(Token::At))) {
            self.advance();
            let addr = if let Some(Ok(Token::Integer(n))) = self.current_token() {
                *n as u64
            } else if let Some(Ok(Token::Identifier(name))) = self.current_token() {
                // Named address - resolve from FFI state or .dbv alias map
                // For now, require explicit hex address in .ebv/.cbv mode
                return self.spanned_err("Named address not yet resolved. Use a hex address like @0x40000000.".to_string());
            } else {
                return self.spanned_err("Expected address after @".to_string());
            };
            self.advance();
            Some(addr)
        } else {
            None // Compiler will pick address from profile
        };

        // Parse parameters
        self.expect(Token::LParen)?;
        let mut inputs = Vec::new();
        while !matches!(self.current_token(), Some(Ok(Token::RParen))) {
            // Parameter name can be identifier or 'from' keyword
            let param_name = match self.current_token() {
                Some(Ok(Token::Identifier(s))) => {
                    let name = (*s).to_string();
                    self.advance();
                    name
                }
                Some(Ok(Token::From)) => {
                    self.advance();
                    "from".to_string()
                }
                _ => return self.spanned_err("Expected parameter name".to_string()),
            };
            self.expect(Token::Colon)?;
            let param_type = self.parse_type()?;
            inputs.push((param_name, param_type));

            if let Some(Ok(Token::Comma)) = self.current_token() {
                self.advance();
            } else {
                break;
            }
        }
        self.expect(Token::RParen)?;

        // Parse return type
        let mut success_output = Vec::new();
        let mut is_result = false;
        if ffi_kind != FfiKind::FrgnBang && ffi_kind != FfiKind::SyscallBang {
            self.expect(Token::Arrow)?;

            // Expect "Result<T, E>" or plain type, optionally followed by `| fallback`
            is_result = matches!(self.current_token(), Some(Ok(Token::Identifier(id))) if id == "Result");
            if is_result {
                self.advance();
                // Parse <SuccessType, E>
                self.expect(Token::Lt)?;

                // Parse success type
                if let Some(Ok(Token::LParen)) = self.current_token() {
                    // Multi-field success output: (field1: T1, field2: T2)
                    self.advance();
                    loop {
                        let field_name = self.expect_identifier()?;
                        self.expect(Token::Colon)?;
                        let field_type = self.parse_type()?;
                        success_output.push((field_name, field_type));
                        if let Some(Ok(Token::Comma)) = self.current_token() {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    self.expect(Token::RParen)?;
                } else {
                    // Single-field success output: T -> becomes (result: T)
                    let success_type = self.parse_type()?;
                    success_output.push(("result".to_string(), success_type));
                }

                self.expect(Token::Comma)?;

                // Parse error type
                let _error_type = self.parse_type()?;

                self.expect(Token::Gt)?;
            } else {
                // Plain return type, not Result — e.g. `-> String` or `-> Int`
                let plain_type = self.parse_type()?;
                success_output.push(("result".to_string(), plain_type));
            }
        }

        // Check for pipe syntax: `-> T | fallback_expr`
        // Only valid with plain return type (not Result<T,E>).
        let mut fallback = None;
        let mut is_pipe = false;
        if let Some(Ok(Token::Pipe)) = self.current_token() {
            if is_result {
                return self.spanned_err(
                    "Cannot combine 'Result<T, E>' syntax with pipe '|' fallback. \
                     Use either `-> Result<T, E>` or `-> T | fallback`, not both."
                        .to_string(),
                );
            }
            if ffi_kind == FfiKind::FrgnBang || ffi_kind == FfiKind::SyscallBang {
                return self.spanned_err(
                    "Cannot use pipe '|' fallback with fire-and-forget frgn!/syscall! \
                     (they have no return type)."
                        .to_string(),
                );
            }
            self.advance();
            is_pipe = true;
            fallback = Some(self.parse_expression()?);
        }

        // Parse optional from "location" clause
        let location = if let Some(Ok(Token::From)) = self.current_token() {
            self.advance();
            if let Some(Ok(Token::String(s))) = self.current_token() {
                let loc = s.clone();
                self.advance();
                loc
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        self.expect(Token::Semicolon)?;

        let result_type = if success_output.is_empty() {
            ResultType::TrueAssertion
        } else {
            ResultType::Projection(success_output.iter().map(|(_, t)| t.clone()).collect())
        };
        let frgn_sig = ForeignSignature {
            name: name.clone(),
            inputs,
            success_output,
            error_type_name: String::new(),
            error_fields: Vec::new(),
            location: location.clone(),
            input_layout: None,
            output_layout: None,
            precondition: None,
            postcondition: None,
            buffer_mode: None,
            wasm_impl: None,
            wasm_setup: None,
            result_type,
            ffi_kind: Some(ffi_kind),
            is_out,
            is_pipe,
            fallback,
            default_watchdog: None,
            span: None,
        };

        Ok(TopLevel::ForeignBinding {
            name,
            toml_path: String::new(), // No longer used - profile-based
            signature: frgn_sig,
            target: ForeignTarget::Native,
            span: None,
        })
    }

    /// Extract the `#section("name")` value from hashtag modifiers, if present.
    fn extract_section(modifiers: &[crate::ast::Annotation]) -> Option<String> {
        modifiers.iter()
            .find(|h| h.name == "section")
            .and_then(|h| h.string_value())
    }

    /// Parse an intrinsic operation declaration: `inop[#][!] name(params) -> Ret [pre][post] { llvm_body } fallback { expr }`
    fn parse_inop_decl(&mut self, bang: bool, section: Option<String>) -> Result<InopDeclaration, SyntaxError> {
        // Consume the `inop` / `inop!` / `inop#` / `inop#!` token.
        // bang = true means `inop!` was used (side-effecting).
        if bang {
            self.expect(Token::InopBang)?;
        } else {
            self.expect(Token::Inop)?;
        }
        let name = self.expect_identifier()?;

        // Parse optional type parameters: <T> or <T, U>
        let mut type_params = Vec::new();
        if let Some(Ok(Token::Lt)) = self.current_token() {
            self.advance();
            loop {
                let param = self.expect_identifier()?;
                type_params.push(param);
                if let Some(Ok(Token::Comma)) = self.current_token() {
                    self.advance();
                } else {
                    break;
                }
            }
            self.expect(Token::Gt)?;
        }

        let params = if let Some(Ok(Token::LParen)) = self.current_token() {
            self.advance();
            let mut p = Vec::new();
            loop {
                let param_result = self.expect_identifier();
                match param_result {
                    Ok(param_name) => {
                        self.expect(Token::Colon)?;
                        let param_type = self.parse_type()?;
                        p.push((param_name, param_type));
                        if let Some(Ok(Token::Comma)) = self.current_token() {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            self.expect(Token::RParen)?;
            p
        } else {
            Vec::new()
        };

        // Parse optional [pre][post] contract (before -> return type, to avoid
        // parse_type greedily consuming [ as generic type parameter)
        let mut contract = if let Some(Ok(Token::LBracket)) = self.current_token() {
            self.parse_contract()?
        } else {
            Contract::new(Expr::Bool(true), Expr::Bool(true))
        };

        // Parse optional -> ReturnType(s)
        // Supports single: -> Int, multi: -> Int, Float, or zero: (no arrow)
        // Use parse_type_inner(false) to prevent greedy [ consumption as generic
        // type parameter — [ belongs to the contract, not the return type.
        let outputs = if let Some(Ok(Token::Arrow)) = self.current_token() {
            self.advance();
            let mut tys = vec![self.parse_type_inner(false)?];
            // Support multi-output: -> Int, Float, Bool
            while let Some(Ok(Token::Comma)) = self.current_token() {
                self.advance();
                tys.push(self.parse_type_inner(false)?);
            }
            tys
        } else {
            Vec::new()
        };

        // If contract wasn't before ->, try after
        if matches!(contract.pre_condition, Expr::Bool(true))
            && matches!(contract.post_condition, Expr::Bool(true))
            && matches!(self.current_token(), Some(Ok(Token::LBracket)))
        {
            contract = self.parse_contract()?;
        }

        // Parse optional (%state) access marker — signals that BILD body uses %State*
        let has_state_access = if let Some(Ok(Token::LParen)) = self.current_token() {
            self.advance();
            // Consume % token before "state"
            if matches!(self.current_token(), Some(Ok(Token::Percent))) {
                self.advance();
            }
            let ident = self.expect_identifier()?;
            if ident != "state" {
                return self.spanned_err("expected 'state' in (%state) marker".to_string());
            }
            self.expect(Token::RParen)?;
            true
        } else {
            false
        };

        // Parse LLVM IR body: { ... }
        let (llvm_body, llvm_body_spans) = if let Some(Ok(Token::LBrace)) = self.current_token() {
            self.advance();
            let mut lines = Vec::new();
            let mut spans = Vec::new();
            let mut depth = 1u32;
            let mut current_line = String::new();
            let mut need_space = false;
            while let Some(tok) = self.current_token().cloned() {
                match tok {
                    Ok(Token::LBrace) => {
                        if need_space { current_line.push(' '); }
                        current_line.push('{');
                        depth += 1;
                        need_space = false;
                        self.advance();
                    }
                    Ok(Token::RBrace) => {
                        depth -= 1;
                        if depth == 0 {
                            if !current_line.trim().is_empty() {
                                if let Some(sp) = self.current_span() {
                                    spans.push(sp);
                                }
                                lines.push(current_line.trim().to_string());
                            }
                            self.advance(); // consume }
                            break;
                        }
                        if need_space { current_line.push(' '); }
                        current_line.push('}');
                        need_space = false;
                        self.advance();
                    }
                    Ok(Token::Semicolon) => {
                        current_line.push(';');
                        if !current_line.trim().is_empty() {
                            if let Some(sp) = self.current_span() {
                                spans.push(sp);
                            }
                            lines.push(current_line.trim().to_string());
                        }
                        current_line = String::new();
                        need_space = false;
                        self.advance();
                    }
                    Ok(Token::Identifier(s)) => {
                        if need_space { current_line.push(' '); }
                        current_line.push_str(&s);
                        need_space = true;
                        self.advance();
                    }
                    Ok(Token::Hash) => {
                        if need_space { current_line.push(' '); }
                        current_line.push('#');
                        need_space = false;
                        self.advance();
                    }
                    Ok(Token::Percent) => {
                        if need_space { current_line.push(' '); }
                        current_line.push('%');
                        need_space = false;
                        self.advance();
                    }
                    Ok(Token::At) => {
                        current_line.push('@');
                        need_space = false;
                        self.advance();
                    }
                    Ok(Token::Dot) => {
                        if need_space { current_line.push(' '); }
                        current_line.push('.');
                        need_space = false;
                        self.advance();
                    }
                    Ok(Token::Comma) => {
                        // No space before comma (LLVM convention)
                        current_line.push(',');
                        need_space = true; // space after comma
                        self.advance();
                    }
                    Ok(Token::Colon) => {
                        current_line.push(':');
                        need_space = false;
                        self.advance();
                    }
                    Ok(Token::Eq) => {
                        if need_space { current_line.push(' '); }
                        current_line.push('=');
                        need_space = true;
                        self.advance();
                    }
                    Ok(Token::Not) => {
                        if need_space { current_line.push(' '); }
                        current_line.push('!');
                        need_space = false;
                        self.advance();
                    }
                    Ok(Token::Plus) => {
                        if need_space { current_line.push(' '); }
                        current_line.push('+');
                        need_space = true;
                        self.advance();
                    }
                    Ok(Token::Minus) => {
                        if need_space { current_line.push(' '); }
                        current_line.push('-');
                        need_space = true;
                        self.advance();
                    }
                    Ok(Token::Star) => {
                        if need_space { current_line.push(' '); }
                        current_line.push('*');
                        need_space = true;
                        self.advance();
                    }
                    Ok(Token::Slash) => {
                        if need_space { current_line.push(' '); }
                        current_line.push('/');
                        need_space = true;
                        self.advance();
                    }
                    Ok(Token::LParen) => {
                        if need_space { current_line.push(' '); }
                        current_line.push('(');
                        need_space = false;
                        self.advance();
                    }
                    Ok(Token::RParen) => {
                        // No space before closing paren
                        current_line.push(')');
                        need_space = true; // space after closing paren
                        self.advance();
                    }
                    Ok(Token::Lt) => {
                        if need_space { current_line.push(' '); }
                        current_line.push('<');
                        need_space = true;
                        self.advance();
                    }
                    Ok(Token::Gt) => {
                        if need_space { current_line.push(' '); }
                        current_line.push('>');
                        need_space = true;
                        self.advance();
                    }
                    Ok(Token::Integer(n)) => {
                        if need_space { current_line.push(' '); }
                        current_line.push_str(&n.to_string());
                        need_space = true;
                        self.advance();
                    }
                    Ok(Token::Float(f)) => {
                        if need_space { current_line.push(' '); }
                        current_line.push_str(&f.to_string());
                        need_space = true;
                        self.advance();
                    }
                    Ok(Token::String(s)) => {
                        if need_space { current_line.push(' '); }
                        current_line.push('"');
                        current_line.push_str(&s);
                        current_line.push('"');
                        need_space = true;
                        self.advance();
                    }
                    // LLVM IR type keywords (lexed as Brief type tokens)
                    Ok(Token::TypeVoid) | Ok(Token::TypeBool) | Ok(Token::TypeChar)
                    | Ok(Token::TypeI8) | Ok(Token::TypeU8)
                    | Ok(Token::TypeI16) | Ok(Token::TypeU16)
                    | Ok(Token::TypeI32) | Ok(Token::TypeU32)
                    | Ok(Token::TypeI64) | Ok(Token::TypeU64)
                    | Ok(Token::TypeInt) | Ok(Token::TypeUInt) | Ok(Token::TypeUnsigned) | Ok(Token::TypeUSgn)
                    | Ok(Token::TypeSigned) | Ok(Token::TypeSgn)
                    | Ok(Token::TypeData) | Ok(Token::TypeFloat) | Ok(Token::TypeString) => {
                        if need_space { current_line.push(' '); }
                        let s = Self::token_display(&tok.unwrap());
                        current_line.push_str(&s);
                        need_space = true;
                        self.advance();
                    }
                    // Brief keywords used as LLVM IR tokens: term and term!
                    Ok(Token::Term) => {
                        if need_space { current_line.push(' '); }
                        current_line.push_str("term");
                        need_space = true;
                        self.advance();
                    }
                    Ok(Token::TermBang) => {
                        if need_space { current_line.push(' '); }
                        current_line.push_str("term!");
                        need_space = true;
                        self.advance();
                    }
                    // LLVM IR keywords like i1, i8, i16, i32, i64, half, float, double, ptr
                    // These are lexed as type tokens above. For other LLVM type names like
                    // "float", "double", "ptr", they may be Identifier tokens — handle in Identifier arm.
                    _ => {
                        if need_space { current_line.push(' '); }
                        need_space = false;
                        self.advance();
                    }
                }
            }
            (lines, spans)
        } else {
            return self.spanned_err("expected `{` for inop# LLVM IR body".to_string());
        };

        // Parse optional fallback block: `fallback expr`
        let fallback = if let Some(Ok(Token::Identifier(kw))) = self.current_token() {
            if kw == "fallback" {
                self.advance();
                let expr = self.parse_expression()?;
                Some(expr)
            } else {
                None
            }
        } else {
            None
        };

        // Consume optional trailing semicolon (matches other top-level items)
        if let Some(Ok(Token::Semicolon)) = self.current_token() {
            self.advance();
        }

        Ok(InopDeclaration {
            name,
            type_params,
            params,
            outputs,
            contract,
            llvm_body,
            llvm_body_spans,
            fallback,
            has_side_effects: bang,
            has_state_access,
            section,
            span: None,
        })
    }

    /// Parse a meld declaration:
    ///   `meld A <:> B;` — infer all routes from `@/` bit-range matching
    ///   `meld A <:> B { Ptr -> B.ptr; Size -> B :> Size; };` — explicit routes
    fn parse_meld_decl(&mut self) -> Result<MeldDeclaration, SyntaxError> {
        self.expect(Token::Meld)?;
        let name_a = self.expect_identifier()?;
        self.expect(Token::LtColonGt)?;
        let name_b = self.expect_identifier()?;

        let routes = if let Some(Ok(Token::LBrace)) = self.current_token() {
            self.advance();
            let mut r = Vec::new();
            while !matches!(self.current_token(), Some(Ok(Token::RBrace)) | None) {
                let accessor = self.expect_identifier()?;
                self.expect(Token::Arrow)?;
                let dest_expr = self.parse_expression()?;
                r.push(MeldRouteDef { accessor, dest_expr });
                if let Some(Ok(Token::Semicolon)) = self.current_token() {
                    self.advance();
                }
            }
            self.expect(Token::RBrace)?;
            r
        } else {
            Vec::new()
        };

        if let Some(Ok(Token::Semicolon)) = self.current_token() {
            self.advance();
        }

        Ok(MeldDeclaration {
            name_a,
            name_b,
            routes,
            span: None,
        })
    }

    /// Parse a resource declaration: rsrc name: Type(args);
    fn parse_resource(&mut self) -> Result<TopLevel, SyntaxError> {
        use crate::ast::ResourceDeclaration;

        let name = self.expect_identifier()?;
        self.expect(Token::Colon)?;

        let type_name = self.expect_identifier()?;

        let mut args = Vec::new();
        if let Some(Ok(Token::LParen)) = self.current_token() {
            self.advance();
            while let Some(Ok(Token::Integer(n))) = self.current_token() {
                let val = *n as i64;
                self.advance();
                args.push(val);
                if let Some(Ok(Token::Comma)) = self.current_token() {
                    self.advance();
                } else {
                    break;
                }
            }
            self.expect(Token::RParen)?;
        }

        self.expect(Token::Semicolon)?;

        Ok(TopLevel::ResourceDecl(ResourceDeclaration {
            name,
            resource_type: type_name,
            args,
            span: None,
        }))
    }

    fn parse_struct(&mut self) -> Result<StructDefinition, SyntaxError> {
        self.expect(Token::Struct)?;
        let name = self.expect_identifier()?;

        // Parse optional type parameters: <K, V>
        let mut type_params = Vec::new();
        if let Some(Ok(Token::Lt)) = self.current_token() {
            self.advance();
            loop {
                let param = self.expect_identifier()?;
                type_params.push(param);
                if let Some(Ok(Token::Comma)) = self.current_token() {
                    self.advance();
                } else {
                    break;
                }
            }
            self.expect(Token::Gt)?;
        }

        // Parse optional parent type: struct Name <: ParentType {
        let parent = if let Some(Ok(Token::LtColon)) = self.current_token() {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };

        self.expect(Token::LBrace)?;

        let mut fields = Vec::new();
        let mut transactions = Vec::new();

        while let Some(token) = self.current_token() {
            match token {
                Ok(Token::RBrace) => {
                    self.advance();
                    break;
                }
                Ok(Token::Pvt) | Ok(Token::Sed) => {
                    let vis = self.parse_field_visibility();
                    let field_name = self.expect_identifier()?;
                    self.expect(Token::Colon)?;
                    let field_type = self.parse_type()?;

                    let default = if let Some(Ok(Token::Eq)) = self.peek() {
                        self.expect(Token::Eq)?;
                        Some(self.parse_expression()?)
                    } else {
                        None
                    };

                    if let Some(Ok(Token::Semicolon)) = self.current_token() {
                        self.advance();
                    } else if let Some(Ok(Token::Comma)) = self.current_token() {
                        self.advance();
                    }
                    fields.push(StructField {
                        name: field_name,
                        ty: field_type,
                        default,
                        visibility: vis,
                    });
                }
                Ok(Token::Identifier(_)) => {
                    if let Some(Ok(Token::Colon)) = self.peek() {
                        let field_name = self.expect_identifier()?;
                        self.expect(Token::Colon)?;
                        let field_type = self.parse_type()?;

                        // Parse optional initializer
                        let default = if let Some(Ok(Token::Eq)) = self.peek() {
                            self.expect(Token::Eq)?;
                            Some(self.parse_expression()?)
                        } else {
                            // No initializer - field will be uninitialized
                            None
                        };

                        // Accept both semicolon and comma as field separator
                        if let Some(Ok(Token::Semicolon)) = self.current_token() {
                            self.advance();
                        } else if let Some(Ok(Token::Comma)) = self.current_token() {
                            self.advance();
                        }
                        fields.push(StructField {
                            name: field_name,
                            ty: field_type,
                            default,
                            visibility: Visibility::Public,
                        });
                    } else {
                        let txn = self.parse_transaction()?;
                        transactions.push(txn);
                    }
                }
                Ok(Token::Txn) | Ok(Token::Rct) | Ok(Token::Async) => {
                    let txn = self.parse_transaction()?;
                    transactions.push(txn);
                }
                Ok(Token::Let) => {
                    // Handle "let field: Type;" syntax explicitly
                    self.advance(); // Consume 'let' keyword

                    let vis = self.parse_field_visibility();

                    if let Some(Ok(Token::Colon)) = self.peek() {
                        let field_name = self.expect_identifier()?;
                        self.expect(Token::Colon)?;
                        let field_type = self.parse_type()?;

                        // Parse optional initializer - check current token
                        let default = if let Some(Ok(Token::Eq)) = self.current_token() {
                            self.advance(); // consume '='
                            Some(self.parse_expression()?)
                        } else {
                            // No initializer - field will be uninitialized
                            None
                        };

                        // Accept both semicolon and comma as field separator
                        if let Some(Ok(Token::Semicolon)) = self.current_token() {
                            self.advance();
                        } else if let Some(Ok(Token::Comma)) = self.current_token() {
                            self.advance();
                        }
                        fields.push(StructField {
                            name: field_name,
                            ty: field_type,
                            default,
                            visibility: vis,
                        });
                    } else {
                        // Not a field, treat as transaction
                        let txn = self.parse_transaction()?;
                        transactions.push(txn);
                    }
                }
                _ => {
                    return self.spanned_err(format!("Unexpected token in struct: {}", match token { Ok(t) => Self::token_display(t), _ => "<lexer error>".into() }));
                }
            }
        }

        let span = self.current_span();

        // Struct variants: [discriminant] { +field, -field, field }
        let variants = self.parse_struct_variants()?;

        // Semicolon after struct is optional
        if let Some(Ok(Token::Semicolon)) = self.current_token() {
            self.advance();
        }

        Ok(StructDefinition {
            name,
            type_params,
            parent,
            fields,
            transactions,
            view_html: None,
            span,
            modifiers: Vec::new(),
            variants,
        })
    }

    fn parse_struct_variants(&mut self) -> Result<Vec<StructVariant>, SyntaxError> {
        let mut variants = Vec::new();
        loop {
            match self.current_token() {
                Some(Ok(Token::LBracket)) => {
                    self.advance();
                    let discriminant = self.parse_expression()?;
                    self.expect(Token::RBracket)?;
                    let contract = Some(Contract {
                        pre_condition: discriminant,
                        post_condition: Expr::Bool(true),
                        watchdog: None,
                        span: None,
                    });
                    self.expect(Token::LBrace)?;
                    let (fields, additions, removals) = self.parse_struct_variant_fields()?;
                    self.expect(Token::RBrace)?;
                    variants.push(StructVariant { contract, fields, additions, removals });
                }
                _ => return Ok(variants),
            }
        }
    }

    fn parse_struct_variant_fields(&mut self) -> Result<(Vec<StructField>, Vec<StructField>, Vec<String>), SyntaxError> {
        let mut fields = Vec::new();
        let mut additions = Vec::new();
        let mut removals = Vec::new();
        loop {
            match self.current_token() {
                Some(Ok(Token::RBrace)) => break,
                Some(Ok(Token::Plus)) => {
                    self.advance();
                    let name = self.expect_identifier()?;
                    self.expect(Token::Colon)?;
                    let ty = self.parse_type()?;
                    let default = if let Some(Ok(Token::Eq)) = self.current_token() {
                        self.advance();
                        Some(self.parse_expression()?)
                    } else {
                        None
                    };
                    if let Some(Ok(Token::Semicolon)) = self.current_token() {
                        self.advance();
                    } else if let Some(Ok(Token::Comma)) = self.current_token() {
                        self.advance();
                    }
                    additions.push(StructField { name, ty, default, visibility: Visibility::Public });
                }
                Some(Ok(Token::Minus)) => {
                    self.advance();
                    let name = self.expect_identifier()?;
                    if let Some(Ok(Token::Semicolon)) = self.current_token() {
                        self.advance();
                    } else if let Some(Ok(Token::Comma)) = self.current_token() {
                        self.advance();
                    }
                    removals.push(name);
                }
                Some(Ok(Token::Identifier(_))) | Some(Ok(Token::Pvt)) | Some(Ok(Token::Sed)) => {
                    let vis = self.parse_field_visibility();
                    let name = self.expect_identifier()?;
                    self.expect(Token::Colon)?;
                    let ty = self.parse_type()?;
                    let default = if let Some(Ok(Token::Eq)) = self.current_token() {
                        self.advance();
                        Some(self.parse_expression()?)
                    } else {
                        None
                    };
                    if let Some(Ok(Token::Semicolon)) = self.current_token() {
                        self.advance();
                    } else if let Some(Ok(Token::Comma)) = self.current_token() {
                        self.advance();
                    }
                    fields.push(StructField { name, ty, default, visibility: vis });
                }
                _ => {
                    return self.spanned_err(
                        "Expected field declaration, +addition, or -removal in struct variant".to_string()
                    );
                }
            }
        }
        Ok((fields, additions, removals))
    }

    fn parse_rstruct(&mut self) -> Result<RStructDefinition, SyntaxError> {
        self.expect(Token::Rstruct)?;
        let name = self.expect_identifier()?;

        self.expect(Token::LBrace)?;

        let mut fields = Vec::new();
        let mut transactions = Vec::new();
        let mut view_html = String::new();

        let mut iterations = 0;
        const MAX_ITERATIONS: usize = 10000;

        while let Some(token) = self.current_token() {
            iterations += 1;
            if iterations > MAX_ITERATIONS {
                return self.spanned_err(
                    "rstruct parsing exceeded iteration limit - possible infinite loop".to_string(),
                );
            }

            // rstruct closing brace handling
            match token {
                Ok(Token::RBrace) => {
                    self.advance();
                    break;
                }
                Ok(Token::Pvt) | Ok(Token::Sed) => {
                    let vis = self.parse_field_visibility();
                    let field_name = self.expect_identifier()?;
                    self.expect(Token::Colon)?;
                    let field_type = self.parse_type()?;

                    let default = if let Some(Ok(Token::Eq)) = self.current_token() {
                        self.advance(); // consume '='
                        Some(self.parse_expression()?)
                    } else {
                        None
                    };

                    if let Some(Ok(Token::Semicolon)) = self.current_token() {
                        self.advance();
                    } else if let Some(Ok(Token::Comma)) = self.current_token() {
                        self.advance();
                    }
                    fields.push(StructField {
                        name: field_name,
                        ty: field_type,
                        default,
                        visibility: vis,
                    });
                }
                Ok(Token::Identifier(_)) => {
                    // Check if it's a field (name: Type) or transaction
                    if let Some(Ok(Token::Colon)) = self.peek() {
                        let field_name = self.expect_identifier()?;
                        self.expect(Token::Colon)?;
                        let field_type = self.parse_type()?;

                        // Parse optional initializer - check current token (not peek)
                        let default = if let Some(Ok(Token::Eq)) = self.current_token() {
                            self.advance(); // consume '='
                            Some(self.parse_expression()?)
                        } else {
                            // No initializer - field will be uninitialized
                            None
                        };

                        // Accept both semicolon and comma as field separator
                        if let Some(Ok(Token::Semicolon)) = self.current_token() {
                            self.advance();
                        } else if let Some(Ok(Token::Comma)) = self.current_token() {
                            self.advance();
                        }
                        fields.push(StructField {
                            name: field_name,
                            ty: field_type,
                            default,
                            visibility: Visibility::Public,
                        });
                    } else {
                        // This is a transaction - parse it and expand name if no dot
                        let txn = self.parse_transaction()?;
                        // If txn name doesn't contain '.', prepend rstruct name
                        let expanded_txn = if !txn.name.contains('.') {
                            Transaction {
                                name: format!("{}.{}", name, txn.name),
                                ..txn
                            }
                        } else {
                            txn
                        };
                        transactions.push(expanded_txn);
                    }
                }
                Ok(Token::Let) => {
                    // Handle "let field: Type;" syntax explicitly
                    self.advance(); // Consume 'let' keyword

                    let vis = self.parse_field_visibility();

                    if let Some(Ok(Token::Colon)) = self.peek() {
                        let field_name = self.expect_identifier()?;
                        self.expect(Token::Colon)?;
                        let field_type = self.parse_type()?;

                        // Parse optional initializer - check current token (not peek)
                        let default = if let Some(Ok(Token::Eq)) = self.current_token() {
                            self.advance(); // consume '='
                            Some(self.parse_expression()?)
                        } else {
                            // No initializer - field will be uninitialized
                            None
                        };

                        // Accept both semicolon and comma as field separator
                        if let Some(Ok(Token::Semicolon)) = self.current_token() {
                            self.advance();
                        } else if let Some(Ok(Token::Comma)) = self.current_token() {
                            self.advance();
                        }
                        fields.push(StructField {
                            name: field_name,
                            ty: field_type,
                            default,
                            visibility: vis,
                        });
                    } else {
                        // Not a field, treat as transaction - parse and expand name
                        let txn = self.parse_transaction()?;
                        let expanded_txn = if !txn.name.contains('.') {
                            Transaction {
                                name: format!("{}.{}", name, txn.name),
                                ..txn
                            }
                        } else {
                            txn
                        };
                        transactions.push(expanded_txn);
                    }
                }
                Ok(Token::Txn) | Ok(Token::Rct) | Ok(Token::Async) => {
                    // Parse transaction and expand name if no dot
                    let txn = self.parse_transaction()?;
                    let expanded_txn = if !txn.name.contains('.') {
                            Transaction {
                                name: format!("{}.{}", name, txn.name),
                                ..txn
                            }
                    } else {
                        txn
                    };
                    transactions.push(expanded_txn);
                }
                Ok(Token::Lt) => {
                    let start = if let Some((_, span)) = &self.current {
                        span.start
                    } else {
                        return self.spanned_err("Unexpected EOF in rstruct".to_string());
                    };
                    let (html, end_pos) = self.scan_html_block(start)?;
                    view_html.push_str(&html);
                    self.advance_past_position(end_pos);
                    self.advance();
                }
                Ok(Token::Lt) => {
                    let start = if let Some((_, span)) = &self.current {
                        span.start
                    } else {
                        return self.spanned_err("Unexpected EOF in rstruct".to_string());
                    };
                    let (html, end_pos) = self.scan_html_block(start)?;
                    view_html.push_str(&html);
                    self.advance_past_position(end_pos);
                    self.advance();
                }
                _ => {
                    return self.spanned_err(format!("Unexpected token in rstruct: {}", match token { Ok(t) => Self::token_display(t), _ => "<lexer error>".into() }));
                }
            }
        }

        let span = self.current_span();

        if view_html.is_empty() {
            return self.spanned_err(
                "rstruct requires a view body (HTML). Add <div>...</div> inside the rstruct."
                    .to_string(),
            );
        }

        let span = self.current_span();
        // Semicolon after rstruct is optional
        if let Some(Ok(Token::Semicolon)) = self.current_token() {
            self.advance();
        }

        Ok(RStructDefinition {
            name,
            fields,
            transactions,
            view_html,
            span,
     })
    }

    fn parse_enum(&mut self) -> Result<EnumDefinition, SyntaxError> {
        self.expect(Token::Enum)?;
        let name = self.expect_identifier()?;

        // Parse optional type parameters: <T, E>
        let mut type_params = Vec::new();
        if let Some(Ok(Token::Lt)) = self.current_token() {
            self.expect(Token::Lt)?;
            loop {
                let param_name = self.expect_identifier()?;
                type_params.push(TypeParam {
                    name: param_name,
                    bounds: vec![],
                });
                match self.current_token() {
                    Some(Ok(Token::Comma)) => {
                        self.advance(); // consume comma
                    }
                    Some(Ok(Token::Gt)) => {
                        self.advance(); // consume >
                        break;
                    }
                    _ => {
                        return self
                            .spanned_err("Expected ',' or '>' in enum type parameters".to_string())
                    }
                }
            }
        }

        self.expect(Token::LBrace)?;

        let mut variants = Vec::new();

        while let Some(token) = self.current_token() {
            match token {
                Ok(Token::RBrace) => {
                    self.advance();
                    if let Some(Ok(Token::Semicolon)) = self.current_token() {
                        self.advance();
                    }
                    break;
                }
                Ok(Token::Some) => {
                    let variant_name_str = "Some".to_string();
                    self.advance();

                    // Check for tuple variant: Ok(T) or Err(E)
                    let variant = if let Some(Ok(Token::LParen)) = self.current_token() {
                        self.expect(Token::LParen)?;
                        let mut inner_types = Vec::new();
                        loop {
                            let inner_type = self.parse_type()?;
                            inner_types.push(inner_type);
                            match self.current_token() {
                                Some(Ok(Token::Comma)) => {
                                    self.advance();
                                }
                                Some(Ok(Token::RParen)) => {
                                    self.advance();
                                    break;
                                }
                                _ => {
                                    return self.spanned_err(
                                        "Expected ',' or ')' in enum variant".to_string(),
                                    )
                                }
                            }
                        }
                        EnumVariant::Tuple(variant_name_str, inner_types)
                    } else {
                        EnumVariant::Unit(variant_name_str)
                    };

                    variants.push(variant);

                    // Consume optional comma
                    if let Some(Ok(Token::Comma)) = self.current_token() {
                        self.advance();
                    }
                }
                Ok(Token::None) => {
                    let variant_name_str = "None".to_string();
                    self.advance();
                    variants.push(EnumVariant::Unit(variant_name_str));
                    if let Some(Ok(Token::Comma)) = self.current_token() {
                        self.advance();
                    }
                }
                Ok(Token::Ok) => {
                    let variant_name_str = "Ok".to_string();
                    self.advance();

                    let variant = if let Some(Ok(Token::LParen)) = self.current_token() {
                        self.expect(Token::LParen)?;
                        let mut inner_types = Vec::new();
                        loop {
                            let inner_type = self.parse_type()?;
                            inner_types.push(inner_type);
                            match self.current_token() {
                                Some(Ok(Token::Comma)) => {
                                    self.advance();
                                }
                                Some(Ok(Token::RParen)) => {
                                    self.advance();
                                    break;
                                }
                                _ => {
                                    return self.spanned_err(
                                        "Expected ',' or ')' in enum variant".to_string(),
                                    )
                                }
                            }
                        }
                        EnumVariant::Tuple(variant_name_str, inner_types)
                    } else {
                        EnumVariant::Unit(variant_name_str)
                    };

                    variants.push(variant);

                    if let Some(Ok(Token::Comma)) = self.current_token() {
                        self.advance();
                    }
                }
                Ok(Token::Err) => {
                    let variant_name_str = "Err".to_string();
                    self.advance();

                    let variant = if let Some(Ok(Token::LParen)) = self.current_token() {
                        self.expect(Token::LParen)?;
                        let mut inner_types = Vec::new();
                        loop {
                            let inner_type = self.parse_type()?;
                            inner_types.push(inner_type);
                            match self.current_token() {
                                Some(Ok(Token::Comma)) => {
                                    self.advance();
                                }
                                Some(Ok(Token::RParen)) => {
                                    self.advance();
                                    break;
                                }
                                _ => {
                                    return self.spanned_err(
                                        "Expected ',' or ')' in enum variant".to_string(),
                                    )
                                }
                            }
                        }
                        EnumVariant::Tuple(variant_name_str, inner_types)
                    } else {
                        EnumVariant::Unit(variant_name_str)
                    };

                    variants.push(variant);

                    if let Some(Ok(Token::Comma)) = self.current_token() {
                        self.advance();
                    }
                }
                Ok(Token::Identifier(variant_name)) => {
                    let variant_name_str = variant_name.to_string();
                    self.advance();

                    // Check for tuple variant: Ok(T) or Err(E)
                    let variant = if let Some(Ok(Token::LParen)) = self.current_token() {
                        self.expect(Token::LParen)?;
                        let mut inner_types = Vec::new();
                        loop {
                            let inner_type = self.parse_type()?;
                            inner_types.push(inner_type);
                            match self.current_token() {
                                Some(Ok(Token::Comma)) => {
                                    self.advance();
                                }
                                Some(Ok(Token::RParen)) => {
                                    self.advance();
                                    break;
                                }
                                _ => {
                                    return self.spanned_err(
                                        "Expected ',' or ')' in enum variant".to_string(),
                                    )
                                }
                            }
                        }
                        EnumVariant::Tuple(variant_name_str, inner_types)
                    } else {
                        EnumVariant::Unit(variant_name_str)
                    };

                    variants.push(variant);

                    // Consume optional comma
                    if let Some(Ok(Token::Comma)) = self.current_token() {
                        self.advance();
                    }
                }
                Ok(Token::None) => {
                    let variant_name_str = "None".to_string();
                    self.advance();
                    variants.push(EnumVariant::Unit(variant_name_str));
                    if let Some(Ok(Token::Comma)) = self.current_token() {
                        self.advance();
                    }
                }
                Ok(Token::Ok) => {
                    let variant_name_str = "Ok".to_string();
                    self.advance();

                    let variant = if let Some(Ok(Token::LParen)) = self.current_token() {
                        self.expect(Token::LParen)?;
                        let mut inner_types = Vec::new();
                        loop {
                            let inner_type = self.parse_type()?;
                            inner_types.push(inner_type);
                            match self.current_token() {
                                Some(Ok(Token::Comma)) => {
                                    self.advance();
                                }
                                Some(Ok(Token::RParen)) => {
                                    self.advance();
                                    break;
                                }
                                _ => {
                                    return self.spanned_err(
                                        "Expected ',' or ')' in enum variant".to_string(),
                                    )
                                }
                            }
                        }
                        EnumVariant::Tuple(variant_name_str, inner_types)
                    } else {
                        EnumVariant::Unit(variant_name_str)
                    };

                    variants.push(variant);

                    if let Some(Ok(Token::Comma)) = self.current_token() {
                        self.advance();
                    }
                }
                Ok(Token::Err) => {
                    let variant_name_str = "Err".to_string();
                    self.advance();

                    let variant = if let Some(Ok(Token::LParen)) = self.current_token() {
                        self.expect(Token::LParen)?;
                        let mut inner_types = Vec::new();
                        loop {
                            let inner_type = self.parse_type()?;
                            inner_types.push(inner_type);
                            match self.current_token() {
                                Some(Ok(Token::Comma)) => {
                                    self.advance();
                                }
                                Some(Ok(Token::RParen)) => {
                                    self.advance();
                                    break;
                                }
                                _ => {
                                    return self.spanned_err(
                                        "Expected ',' or ')' in enum variant".to_string(),
                                    )
                                }
                            }
                        }
                        EnumVariant::Tuple(variant_name_str, inner_types)
                    } else {
                        EnumVariant::Unit(variant_name_str)
                    };

                    variants.push(variant);

                    if let Some(Ok(Token::Comma)) = self.current_token() {
                        self.advance();
                    }
                }
                Ok(Token::Identifier(variant_name)) => {
                    let variant_name_str = variant_name.to_string();
                    self.advance();

                    // Check for tuple variant: Ok(T) or Err(E)
                    let variant = if let Some(Ok(Token::LParen)) = self.current_token() {
                        self.expect(Token::LParen)?;
                        let mut inner_types = Vec::new();
                        loop {
                            let inner_type = self.parse_type()?;
                            inner_types.push(inner_type);
                            match self.current_token() {
                                Some(Ok(Token::Comma)) => {
                                    self.advance();
                                }
                                Some(Ok(Token::RParen)) => {
                                    self.advance();
                                    break;
                                }
                                _ => {
                                    return self.spanned_err(
                                        "Expected ',' or ')' in enum variant".to_string(),
                                    )
                                }
                            }
                        }
                        EnumVariant::Tuple(variant_name_str, inner_types)
                    } else {
                        EnumVariant::Unit(variant_name_str)
                    };

                    variants.push(variant);

                    // Consume optional comma
                    if let Some(Ok(Token::Comma)) = self.current_token() {
                        self.advance();
                    }
                }
                _ => return self.spanned_err(format!("Unexpected token in enum: {}", match token { Ok(t) => Self::token_display(t), _ => "<lexer error>".into() })),
            }
        }

        Ok(EnumDefinition {
            name,
            type_params,
            variants,
            span: self.current_span(),
        })
    }

    /// Parse a `Type Name <: Base { ... }` declaration.
    ///
    /// Grammar:
    ///   type_decl  ::= "type" ident ("<" type_params ">")? "<:" type_expr "{" property* constraint* "}" ";"
    ///   property   ::= ident "=" expr ";"
    ///   constraint ::= "[" expr "]"
    ///
    /// Supported properties: Bytes, Alignment, Endian, Volatile, Atomic,
    /// ElementType, FixedSize, InsertAt, ExtractFrom, AllowIndex, AllowSlice, AllowArrow, Codec.
    fn parse_type_def(&mut self) -> Result<TypeDef, SyntaxError> {
        self.expect(Token::Type)?;
        let name = self.expect_identifier()?;

        // Parse optional type parameters: <T, K>
        let mut type_params = Vec::new();
        if let Some(Ok(Token::Lt)) = self.current_token() {
            self.expect(Token::Lt)?;
            loop {
                let param_name = self.expect_identifier()?;
                type_params.push(param_name);
                match self.current_token() {
                    Some(Ok(Token::Comma)) => { self.advance(); }
                    Some(Ok(Token::Gt)) => { self.advance(); break; }
                    _ => return self.spanned_err(
                        "Expected ',' or '>' in type parameters".to_string(),
                    ),
                }
            }
        }

        // Parse `<:` operator
        self.expect(Token::LtColon)?;

        // Parse base type expression
        let base = self.parse_type_expr_for_typedef()?;

        // Parse optional bit-range suffix: `Bits @/0..7`
        let bit_range = if let Some(Ok(Token::At)) = self.current_token() {
            if let Some(Ok(Token::Slash)) = self.peek_token() {
                self.advance(); // consume @
                self.advance(); // consume /
                Some(self.parse_bit_range()?)
            } else {
                None
            }
        } else {
            None
        };

        // Parse body `{ ... }`
        self.expect(Token::LBrace)?;

        let mut slots = Vec::new();
        let mut bindings = Vec::new();
        let mut constraints = Vec::new();
        let mut operators = Vec::new();

        // Parse properties and constraints until `}`
        loop {
            // Early exit for `}`
            if let Some(Ok(Token::RBrace)) = self.current_token() {
                self.advance();
                break;
            }

            // Check for constraint: `[ expr ]`
            if let Some(Ok(Token::LBracket)) = self.current_token() {
                self.advance(); // consume `[`
                let constraint = self.parse_expression()?;
                self.expect(Token::RBracket)?;
                constraints.push(constraint);
                continue;
            }

            // 2026-06-29: Phase 7B — operator declaration: op Rune(Param) -> Ret = intrinsic;
            if let Some(Ok(Token::Op)) = self.current_token() {
                self.advance(); // consume `op`
                let op_decl = self.parse_operator_declaration()?;
                operators.push(op_decl);
                continue;
            }

            // Check for pragma shorthand: #ident  →  Name = true;
            // Also accepts #!ident (mandatory) and ?#ident (speculative).
            let is_pragma = matches!(self.current_token(), Some(Ok(Token::Hash | Token::HashBang | Token::HashQuestion)));
            if is_pragma {
                self.advance();
                let pragma_name = self.expect_identifier()?;
                // Normalize to lowercase for apply_binding() case-insensitive matching
                let pragma_lower = pragma_name.to_lowercase();
                bindings.push(TypeBinding {
                    name: pragma_lower,
                    params: vec![],
                    value: Box::new(Expr::Bool(true)),
                    span: self.current_span(),
                });
                if let Some(Ok(Token::Semicolon)) = self.current_token() {
                    self.advance();
                } else {
                    return self.spanned_err("Expected ';' after pragma in type body".to_string());
                }
                continue;
            }

            // Parse a slot or binding: ident : Type ;  OR  ident [ ( params ) ]? = expr ;
            let item_name = self.expect_identifier()?;

            // Check for optional params: Name(param1, param2)
            let params = if matches!(self.current_token(), Some(Ok(Token::LParen))) {
                self.advance();
                let mut ps = Vec::new();
                loop {
                    ps.push(self.expect_identifier()?);
                    if matches!(self.current_token(), Some(Ok(Token::Comma))) {
                        self.advance();
                    } else {
                        break;
                    }
                }
                self.expect(Token::RParen)?;
                ps
            } else {
                Vec::new()
            };

            // 2026-07-11: Type slot syntax — `name : Type ;` declares a structural field.
            // Distinguish from bindings by checking for `:` vs `=` / `<~`.
            if matches!(self.current_token(), Some(Ok(Token::Colon))) {
                // Slot declaration: ident : Type ;
                if !params.is_empty() {
                    return self.spanned_err("Slot declarations do not support parameters".to_string());
                }
                self.advance(); // consume `:`
                let ty = self.parse_type()?;
                // Expect semicolon after slot
                if let Some(Ok(Token::Semicolon)) = self.current_token() {
                    self.advance();
                } else {
                    return self.spanned_err("Expected ';' after type slot".to_string());
                }
                slots.push(TypeSlot {
                    name: item_name,
                    ty,
                    span: self.current_span(),
                });
                continue;
            }

            // Otherwise it's a binding: ident [ ( params ) ]? <~ expr ; or ident [ ( params ) ]? = expr ;
            // 2026-07-11: Phase 0.2 — <~ for metadata, = only valid with params (projections)
            if matches!(self.current_token(), Some(Ok(Token::TildeArrow))) {
                self.advance(); // consume <~
            } else if matches!(self.current_token(), Some(Ok(Token::Eq))) {
                if params.is_empty() {
                    return self.spanned_err("use '<~' for metadata in type bodies".to_string());
                }
                self.advance(); // consume =
            } else {
                return self.spanned_err("expected '<~' or '=' in type body binding".to_string());
            }
            let value = self.parse_expression()?;

            // Create binding
            bindings.push(TypeBinding {
                name: item_name,
                params,
                value: Box::new(value),
                span: self.current_span(),
            });

            // Expect semicolon after binding
            if let Some(Ok(Token::Semicolon)) = self.current_token() {
                self.advance();
            } else {
                return self.spanned_err("Expected ';' after type binding".to_string());
            }
        }

        // Expect optional semicolon after `}`
        if let Some(Ok(Token::Semicolon)) = self.current_token() {
            self.advance();
        }

        Ok(TypeDef {
            name,
            type_params,
            base,
            bit_range,
            body: TypeDefBody {
                slots,
                bindings,
                operators,
                constraints,
                span: self.current_span(),
            },
            span: self.current_span(),
        })
    }

    /// Parse an operator declaration inside a type body:
    /// `op Rune(ParamType) -> ReturnType = intrinsic;`
    /// 2026-06-29: Phase 7B.
    fn parse_operator_declaration(&mut self) -> Result<OpDeclaration, SyntaxError> {
        let rune_name = self.expect_identifier()?;
        let rune = match rune_name.as_str() {
            "Add" => OpRune::Add, "Sub" => OpRune::Sub,
            "Mul" => OpRune::Mul, "Div" => OpRune::Div,
            "Mod" => OpRune::Mod, "Neg" => OpRune::Neg,
            "Eq" => OpRune::Eq, "Ne" => OpRune::Ne,
            "Lt" => OpRune::Lt, "Le" => OpRune::Le,
            "Gt" => OpRune::Gt, "Ge" => OpRune::Ge,
            "And" => OpRune::And, "Or" => OpRune::Or, "Not" => OpRune::Not,
            "Shl" => OpRune::Shl, "Shr" => OpRune::Shr,
            "BitAnd" => OpRune::BitAnd, "BitOr" => OpRune::BitOr,
            "BitXor" => OpRune::BitXor, "BitNot" => OpRune::BitNot,
            "Index" => OpRune::Index, "Slice" => OpRune::Slice,
            "Cast" => OpRune::Cast,
            "Box" => OpRune::Box, "Unbox" => OpRune::Unbox,
            "ArrowPush" => OpRune::ArrowPush, "ArrowPop" => OpRune::ArrowPop,
            _ => return self.spanned_err(format!(
                "Unknown operator rune '{}'", rune_name)),
        };

        let param_type = if matches!(self.current_token(), Some(Ok(Token::LParen))) {
            self.advance();
            // Allow empty parens for unary operators: op Neg() -> Ret = impl;
            if matches!(self.current_token(), Some(Ok(Token::RParen))) {
                self.advance();
                None
            } else {
                let pt = self.parse_type_expr_for_typedef()?;
                self.expect(Token::RParen)?;
                Some(pt)
            }
        } else {
            None
        };

        self.expect(Token::Arrow)?;
        let return_type = self.parse_type_expr_for_typedef()?;
        self.expect(Token::Eq)?;
        let implementation = self.parse_expression()?;

        if let Some(Ok(Token::Semicolon)) = self.current_token() {
            self.advance();
        } else {
            return self.spanned_err("Expected ';' after operator declaration".to_string());
        }

        Ok(OpDeclaration { rune, param_type, return_type, implementation: Box::new(implementation), span: self.current_span() })
    }

    /// Parse a type expression used as the base in `Type Name <: Base { ... }`.
    /// This is a restricted subset of parse_expression — currently handles:
    ///   - TypeRef identifiers (e.g. `Bits`, `List`)
    ///   - Generic applications (e.g. `List<T>`, `KeyedQueue<T, K>`)
    /// DEFERRED (D-1): Full type expression support for complex base types.
    fn parse_type_expr_for_typedef(&mut self) -> Result<Box<Expr>, SyntaxError> {
        let name = self.expect_identifier()?;

        // Check for generic application: Name<T, K>
        if let Some(Ok(Token::Lt)) = self.current_token() {
            // We can't fully parse a generic type expression here without adding
            // type info to Expr. For now, store as TypeRef and handle in Pass 1.
            // DEFERRED (D-1): Proper generic type expression parsing.
            self.advance(); // consume `<`
            // Consume everything until `>`
            let mut depth = 1;
            while depth > 0 {
                match self.current_token() {
                    Some(Ok(Token::Gt)) => { self.advance(); depth -= 1; }
                    Some(Ok(Token::Lt)) => { self.advance(); depth += 1; }
                    Some(Ok(Token::LtColon)) => { self.advance(); }
                    _ => { self.advance(); }
                }
            }
        }

        Ok(Box::new(Expr::TypeRef(name)))
    }

    fn scan_html_block(&mut self, start: usize) -> Result<(String, usize), SyntaxError> {
        // Find the opening tag's closing >
        let mut byte_pos = start;
        let source_bytes = self.source.as_bytes();

        // Scan to find the '>' that closes the opening tag
        while byte_pos < source_bytes.len() && source_bytes[byte_pos] != b'>' {
            byte_pos += 1;
        }

        if byte_pos >= source_bytes.len() {
            return self.spanned_err("Unclosed HTML tag in rstruct (no closing >)".to_string());
        }

        byte_pos += 1; // Move past the '>'

        let tag_content = &self.source[start..byte_pos];

        // Handle self-closing tags: <tag /> or <tag> (if it ends with />)
        if tag_content.trim_end().ends_with("/>") {
            return Ok((tag_content.to_string(), byte_pos));
        }

        // Extract tag name from opening tag
        let mut tag_name = String::new();
        let after_lt = if tag_content.starts_with("<") {
            &tag_content[1..]
        } else {
            tag_content
        };
        if !after_lt.starts_with('/') && !after_lt.starts_with('!') {
            for c in after_lt.chars() {
                if c.is_alphanumeric() || c == '-' {
                    tag_name.push(c);
                } else {
                    break;
                }
            }
        }

        if tag_name.is_empty() {
            return self
                .spanned_err("Could not parse HTML tag in rstruct (no tag name)".to_string());
        }

        let close_tag = format!("</{}>", tag_name);
        let open_tag = format!("<{}", tag_name);

        // Now scan for matching closing tag with depth tracking
        // to handle nested tags with the same name
        let mut depth = 1;

        while byte_pos < source_bytes.len() {
            // Check if we found the close tag
            if self.source[byte_pos..].starts_with(&close_tag) {
                depth -= 1;
                if depth == 0 {
                    byte_pos += close_tag.len();
                    return Ok((self.source[start..byte_pos].to_string(), byte_pos));
                }
                // Skip past this close tag
                byte_pos += close_tag.len();
            }
            // Check if we found an open tag (for depth tracking)
            else if self.source[byte_pos..].starts_with(&open_tag) {
                // Make sure this is actually an opening tag (not closing or self-closing)
                let after_tag_name = &self.source[byte_pos + open_tag.len()..];
                if !after_tag_name.is_empty() {
                    let next_char = after_tag_name.chars().next().unwrap_or('\0');
                    // If next char is '>', space, or attribute marker, it's an open tag
                    if next_char == '>'
                        || next_char == ' '
                        || next_char == '\t'
                        || next_char == '\n'
                    {
                        depth += 1;
                    }
                }
                byte_pos += open_tag.len();
            } else {
                // Safely advance by one character
                if byte_pos < source_bytes.len() {
                    let ch = self.source[byte_pos..].chars().next().unwrap_or('\0');
                    byte_pos += ch.len_utf8();
                } else {
                    byte_pos += 1;
                }
            }
        }

        self.spanned_err(format!(
            "Unclosed HTML tag in rstruct (missing </{}>)",
            tag_name
        ))
    }

    fn advance_past_position(&mut self, target_pos: usize) {
        while let Some((_, span)) = &self.current {
            if span.end >= target_pos {
                break;
            }
            self.advance();
        }
    }

    fn parse_render_block(&mut self) -> Result<RenderBlock, SyntaxError> {
        self.expect(Token::Render)?;
        let struct_name = self.expect_identifier()?;

        let lbrace_pos = if let Some((_, span)) = &self.current {
            if let Some(Ok(Token::LBrace)) = self.current_token() {
                span.start
            } else {
                return self
                    .spanned_err(format!("Expected LBrace, found {}", self.fmt_current_token()));
            }
        } else {
            return self.spanned_err("Unexpected EOF".to_string());
        };
        self.advance();

        let mut brace_depth = 1;
        let mut end_pos = lbrace_pos;
        while let Some((_, span)) = &self.current {
            if let Some(Ok(Token::LBrace)) = self.current_token() {
                brace_depth += 1;
            } else if let Some(Ok(Token::RBrace)) = self.current_token() {
                brace_depth -= 1;
                if brace_depth == 0 {
                    end_pos = span.start;
                    self.advance();
                    break;
                }
            }
            self.advance();
        }

        let view_html = self.source[lbrace_pos + 1..end_pos].trim().to_string();
        let span = self.current_span();
        Ok(RenderBlock {
            struct_name,
            view_html,
            span,
        })
    }

    fn peek(&self) -> Option<&Result<Token, ()>> {
        self.peek.as_ref().map(|(t, _)| t)
    }

    fn parse_variant_bodies(&mut self) -> Result<Vec<(Option<Contract>, Vec<Statement>)>, SyntaxError> {
        let mut variants = Vec::new();
        loop {
            match self.current_token() {
                Some(Ok(Token::LBracket)) => {
                    self.advance();
                    let pre = self.parse_expression()?;
                    self.expect(Token::RBracket)?;
                    self.expect(Token::LBrace)?;
                    let body = self.parse_body()?;
                    self.expect(Token::RBrace)?;
                    variants.push((Some(Contract { pre_condition: pre, post_condition: Expr::Bool(true), watchdog: None, span: None }), body));
                }
                Some(Ok(Token::LBrace)) => {
                    self.advance();
                    let body = self.parse_body()?;
                    self.expect(Token::RBrace)?;
                    variants.push((None, body));
                }
                _ => return Ok(variants),
            }
        }
    }

    // PERMANENTLY ABANDONED: alka/on_exit — code left as historical artifact. No revisit planned.
    // fn parse_alka_block(&mut self) -> Result<Statement, SyntaxError> {
    //     self.advance();
    //     let dangerous = if let Some(Ok(Token::Not)) = self.current_token() {
    //         self.advance();
    //         true
    //     } else {
    //         false
    //     };
    //     let lbrace_pos = if let Some((_, span)) = &self.current {
    //         if let Some(Ok(Token::LBrace)) = self.current_token() {
    //             span.start
    //         } else {
    //             return self.spanned_err("Expected { after alka".to_string());
    //         }
    //     } else {
    //         return self.spanned_err("Unexpected EOF".to_string());
    //     };
    //     self.advance();
    //     let mut brace_depth = 1;
    //     let mut end_pos = lbrace_pos;
    //     while let Some((_, span)) = &self.current {
    //         if let Some(Ok(Token::LBrace)) = self.current_token() {
    //             brace_depth += 1;
    //         } else if let Some(Ok(Token::RBrace)) = self.current_token() {
    //             brace_depth -= 1;
    //             if brace_depth == 0 {
    //                 end_pos = span.start;
    //                 self.advance();
    //                 break;
    //             }
    //         }
    //         self.advance();
    //     }
    //     if brace_depth != 0 {
    //         return self.spanned_err("Unterminated alka block".to_string());
    //     }
    //     let content = self.source[lbrace_pos + 1..end_pos].trim().to_string();
    //     self.expect(Token::Semicolon)?;
    //     let span = self.current_span();
    //     Ok(Statement::Alka(AlkaBlock { dangerous, content, span }))
    // }

    // PERMANENTLY ABANDONED: alka/on_exit — code left as historical artifact. No revisit planned.
    // fn parse_block_pragma(&mut self) -> Result<Statement, SyntaxError> {
    //     // #identifier { body };
    //     self.advance();
    //     let name = self.expect_identifier()?;
    //     self.expect(Token::LBrace)?;
    //     let body = self.parse_body()?;
    //     self.expect(Token::RBrace)?;
    //     self.expect(Token::Semicolon)?;
    //     let span = self.current_span();
    //     Ok(Statement::OnExit { body, span })
    // }

    fn parse_state_decl(&mut self) -> Result<StateDecl, SyntaxError> {
        self.expect(Token::Let)?;
        let name = self.expect_identifier()?;

        let mut address: Option<u64> = None;
        let mut bit_range: Option<BitRange> = None;
        let mut is_override = false;

        // Optional mapping before colon
        // Supports: @ address / bit-spec, @ / bit-spec, @ stack:offset, @ heap:offset, [bit-spec]
        // Also: @"..." (regex literal) — let it fall through to expression parser
            loop {
                if let Some(Ok(Token::At)) = self.current_token() {
                    // @"..." is a regex literal, not an address — let expression parser handle it
                    if let Some(Ok(Token::String(_))) = self.peek_token() {
                        break;
                    }
                    self.advance();
                    // Check for / immediately after @ (auto-allocate with bit-spec)
                    if let Some(Ok(Token::Slash)) = self.current_token() {
                        self.advance();
                        bit_range = Some(self.parse_bit_range()?);
                    } else {
                        match self.current_token() {
                            Some(Ok(Token::Integer(n))) => {
                                address = Some(*n as u64);
                                self.advance();
                                // Handle slash shorthand: @0x1000/x16 or @0x1000/0
                                if let Some(Ok(Token::Slash)) = self.current_token() {
                                    self.advance();
                                    bit_range = Some(self.parse_bit_range()?);
                                }
                            }
                            Some(Ok(Token::Identifier(id))) if id == "stack" => {
                                self.advance();
                                self.expect(Token::Colon)?;
                                let offset = self.expect_integer()?;
                                address = Some(offset as u64);
                            }
                            Some(Ok(Token::Identifier(id))) if id == "heap" => {
                                self.advance();
                                self.expect(Token::Colon)?;
                                let offset = self.expect_integer()?;
                                address = Some(offset as u64);
                            }
                            Some(Ok(Token::Identifier(id))) => {
                                return self.spanned_err(format!("Named address '{}' not yet resolved. Use a hex address like @0x40000000.", id));
                            }
                            _ => return self.spanned_err("Expected address mode after @: raw, stack, heap, or /".to_string()),
                        }
                    }
                } else if let Some(Ok(Token::LBracket)) = self.current_token() {
                self.advance();
                bit_range = Some(self.parse_bit_range()?);
                self.expect(Token::RBracket)?;
            } else {
                break;
            }
        }

        self.expect(Token::Colon)?;
        let ty = self.parse_type()?;

        // Hardware mapping after type (Spec 2.2 / 3.0)
        loop {
            if let Some(Ok(Token::At)) = self.current_token() {
                // @"..." is a regex literal, not an address — let expression parser handle it
                if let Some(Ok(Token::String(_))) = self.peek_token() {
                    break;
                }
                self.advance();
                // Check for / immediately after @ (auto-allocate with bit-spec)
                if let Some(Ok(Token::Slash)) = self.current_token() {
                    self.advance();
                    bit_range = Some(self.parse_bit_range()?);
                } else {
                    match self.current_token() {
                        Some(Ok(Token::Integer(n))) => {
                            address = Some(*n as u64);
                            self.advance();
                            // Handle slash shorthand: @0x1000/x16 or @0x1000/0
                            if let Some(Ok(Token::Slash)) = self.current_token() {
                                self.advance();
                                bit_range = Some(self.parse_bit_range()?);
                            }
                        }
                        Some(Ok(Token::Identifier(id))) => {
                            return self.spanned_err(format!("Named address '{}' not yet resolved. Use a hex address like @0x40000000.", id));
                        }
                        _ => return self.spanned_err("Expected integer address or / after '@'".to_string()),
                    }
                }
            } else if let Some(Ok(Token::LBracket)) = self.current_token() {
                self.advance();
                bit_range = Some(self.parse_bit_range()?);
                self.expect(Token::RBracket)?;
            } else {
                break;
            }
        }

        // `<: [lo..hi]` or `<: [expr]` constraint after type
        let mut constraint: Option<Box<Expr>> = None;
        if self.check_lt_colon() {
            self.advance();
            if let Some(Ok(Token::LBracket)) = self.current_token() {
                self.advance();
                constraint = Some(self.parse_constraint_expr()?);
                self.expect(Token::RBracket)?;
            }
        }

        let expr = if let Some(Ok(Token::Eq)) = self.current_token() {
            self.advance();
            Some(self.parse_expression()?)
        } else {
            None
        };
let span = self.current_span();
        self.expect(Token::Semicolon)?;
        Ok(StateDecl {
            name,
            ty,
            expr,
            address,
            bit_range,
            constraint,
            is_override,
            os_mode: false,
            span,
            attrs: Vec::new(),  // Initialize attrs
        })
    }

    fn parse_bit_range(&mut self) -> Result<BitRange, SyntaxError> {
        let result = match self.current_token() {
            Some(Ok(Token::Identifier(name))) => {
                let name = name.clone();
                if name == "x" || name == "*" {
                    self.advance();
                    if let Some(Ok(Token::Integer(n))) = self.current_token() {
                        let n = *n as usize;
                        self.advance();
                        BitRange::Any(n)
                    } else {
                        BitRange::Any(1)
                    }
                } else if name.starts_with('x') {
                    if let Ok(n) = name[1..].parse::<usize>() {
                        self.advance();
                        BitRange::Any(n)
                    } else {
                        return self.spanned_err(format!("Invalid bit-width shorthand: {}", name));
                    }
                } else if let Ok(bit) = name.parse::<usize>() {
                    self.advance();
                    if let Some(Ok(token)) = self.current_token() {
                        match token {
                            Token::Colon | Token::DotDot => {
                                self.advance();
                                let end = self.expect_identifier()?;
                                if let Ok(end_bit) = end.parse::<usize>() {
                                    BitRange::Range(bit, end_bit)
                                } else {
                                    return self
                                        .spanned_err(format!("Expected bit number, got {}", end));
                                }
                            }
                            _ => BitRange::Single(bit),
                        }
                    } else {
                        BitRange::Single(bit)
                    }
                } else {
                    return self.spanned_err(format!("Expected bit number or 'x', got {}", name));
                }
            }
            Some(Ok(Token::Integer(n))) => {
                let n = *n as usize;
                self.advance();
                if let Some(Ok(token)) = self.current_token() {
                    match token {
                        Token::Colon | Token::DotDot => {
                            self.advance();
                            if let Some(Ok(Token::Integer(end))) = self.current_token() {
                                let end = *end as usize;
                                self.advance();
                                BitRange::Range(n, end)
                            } else {
                                return self.spanned_err("Expected end bit number".to_string());
                            }
                        }
                        _ => BitRange::Single(n),
                    }
                } else {
                    BitRange::Single(n)
                }
            }
            _ => return self.spanned_err("Expected bit number or 'x'".to_string()),
        };
        Ok(result)
    }

    /// Parse a single #pragma directive: #pragma.c, #pragma bind(...), etc.
    fn parse_pragma_item(&mut self, target_from_dot: Option<String>) -> Result<crate::ast::Attribute, SyntaxError> {
        let mut key: String;
        let mut target: Option<String> = target_from_dot;

        // If we already have a target from #pragma.c, the attribute key is the target name
        if let Some(ref tgt) = target {
            key = tgt.clone();
        } else {
            // Otherwise, parse an identifier as the key
            key = if let Some(Ok(Token::Identifier(name))) = self.current_token() {
                let name = name.clone();
                self.advance();
                name
            } else {
                return self.spanned_err("Expected identifier after #pragma".to_string());
            };

            // Handle ffi.c, ffi.rust dot syntax
            if key == "ffi" && matches!(self.current_token(), Some(Ok(Token::Dot))) {
                self.advance(); // consume Dot
                if let Some(Ok(Token::Identifier(lang))) = self.current_token() {
                    let full_key = format!("ffi.{}", lang);
                    self.advance();
                    target = Some(full_key.clone());
                    key = full_key;
                } else {
                    return self.spanned_err("Expected language after ffi.".to_string());
                }
            }
        }

        // Check for key(value)
        let value = if matches!(self.current_token(), Some(Ok(Token::LParen))) {
            self.advance(); // consume (
            let val = if let Some(Ok(Token::String(s))) = self.current_token() {
                let s = s.clone();
                self.advance();
                s
            } else if let Some(Ok(Token::Integer(n))) = self.current_token() {
                let s = n.to_string();
                self.advance();
                s
            } else if let Some(Ok(Token::Identifier(name))) = self.current_token() {
                let name = name.clone();
                self.advance();
                name
            } else {
                return self.spanned_err("Expected value in pragma key(value)".to_string());
            };
            self.expect(Token::RParen)?;
            Some(val)
        } else {
            None
        };

        Ok(crate::ast::Attribute { target, key, value })
    }

    /// Parse #[...], #![...], #pragma, or #!pragma attribute syntax
    /// For #[...] / #![...]: returns parsed items from inside brackets
    /// For #pragma.c: returns a single attribute with target specifier
    /// For #!pragma ... ]: returns parsed items from inside brackets
    fn parse_attributes(&mut self) -> Result<Vec<crate::ast::Attribute>, SyntaxError> {
        let mut attrs = Vec::new();
        let mut is_pragma = false;
        let mut is_file_level = false;
        
        // Detect which syntax is being used
        match self.current_token() {
            Some(Ok(Token::HashBracket)) => {
                // Deprecated #[...] syntax
                eprintln!("warning: #[...] syntax is deprecated, use #pragma instead");
                self.advance(); // consume #[
            }
            Some(Ok(Token::HashBangBracket)) => {
                // Deprecated #![...] syntax
                eprintln!("warning: #![...] syntax is deprecated, use #!pragma instead");
                self.advance(); // consume #![
                is_file_level = true;
            }
            Some(Ok(Token::Pragma)) => {
                self.advance(); // consume #pragma
                is_pragma = true;
                // Check for #pragma.c dot syntax
                if matches!(self.current_token(), Some(Ok(Token::Dot))) {
                    self.advance(); // consume .
                    if let Some(Ok(Token::Identifier(target_name))) = self.current_token() {
                        let name = target_name.clone();
                        self.advance();
                        let attr = crate::ast::Attribute {
                            target: Some(name.clone()),
                            key: name,
                            value: None,
                        };
                        attrs.push(attr);
                        return Ok(attrs);
                    } else {
                        return self.spanned_err("Expected target name after #pragma.".to_string());
                    }
                }
                // Parse pragma key
                let attr = self.parse_pragma_item(None)?;
                attrs.push(attr);
                return Ok(attrs);
            }
            Some(Ok(Token::PragmaBang)) => {
                self.advance(); // consume #!pragma
                is_pragma = true;
                is_file_level = true;
            }
            Some(Ok(Token::HashBang)) => {
                self.advance(); // consume #!
                is_pragma = true;
                is_file_level = true;
            }
            _ => {
                return self.spanned_err("Expected #[, #![, #pragma, #!pragma, or #! for attribute".to_string());
            }
        }
        
        // Parse items
        // For #[...] / #![...]: items are inside brackets, terminated by ]
        // For #pragma.c: single item, no brackets (already handled above with return)
        // For #!/#!pragma: comma-separated items, no brackets needed (optional trailing ])
        if is_pragma {
            // Pragma form: parse comma-separated items without bracket enclosure
            loop {
                attrs.push(self.parse_pragma_item(None)?);
                if matches!(self.current_token(), Some(Ok(Token::Comma))) {
                    self.advance();
                } else {
                    break;
                }
            }
            if matches!(self.current_token(), Some(Ok(Token::RBracket))) {
                self.advance(); // consume optional ]
            }
        } else {
            // #[...] / #![...] form: items inside brackets
            while !matches!(self.current_token(), Some(Ok(Token::RBracket))) {
                let key = if let Some(Ok(Token::Identifier(name))) = self.current_token() {
                    let name = name.clone();
                    self.advance();
                    if name == "ffi" && matches!(self.current_token(), Some(Ok(Token::Dot))) {
                        self.advance(); // consume Dot
                        if let Some(Ok(Token::Identifier(lang))) = self.current_token() {
                            let full_key = format!("ffi.{}", lang);
                            self.advance();
                            full_key
                        } else {
                            return self.spanned_err("Expected language after ffi.".to_string());
                        }
                    } else {
                        name
                    }
                } else {
                    return self.spanned_err("Expected identifier in attribute".to_string());
                };

                let value = if matches!(self.current_token(), Some(Ok(Token::LParen))) {
                    self.advance(); // consume (
                    let val = if let Some(Ok(Token::String(s))) = self.current_token() {
                        let s = s.clone();
                        self.advance();
                        s
                    } else if let Some(Ok(Token::Integer(n))) = self.current_token() {
                        let s = n.to_string();
                        self.advance();
                        s
                    } else if let Some(Ok(Token::Identifier(name))) = self.current_token() {
                        let name = name.clone();
                        self.advance();
                        name
                    } else {
                        return self.spanned_err("Expected value in attribute key(value)".to_string());
                    };
                    self.expect(Token::RParen)?;
                    Some(val)
                } else {
                    None
                };

                let target = if attrs.is_empty() && value.is_none() {
                    match key.as_str() {
                        "c" | "sv" | "rust" | "wasm" | "kernel" => Some(key.clone()),
                        _ if key.starts_with("ffi.") => Some(key.clone()),
                        _ => None,
                    }
                } else {
                    None
                };

                attrs.push(crate::ast::Attribute { target, key, value });

                if matches!(self.current_token(), Some(Ok(Token::Comma))) {
                    self.advance();
                } else {
                    break;
                }
            }
            self.expect(Token::RBracket)?;
        }
        Ok(attrs)
    }

    fn parse_trigger(&mut self) -> Result<TriggerDeclaration, SyntaxError> {
        self.expect(Token::Trg)?;
        self.parse_trigger_body(false)
    }

    fn parse_trigger_body(&mut self, is_const: bool) -> Result<TriggerDeclaration, SyntaxError> {
        let name = self.expect_identifier()?;
        self.expect(Token::Colon)?;
        let ty = self.parse_type()?;
        let _trg_name = name.clone(); // saved for cell binding shorthand
        let _trg_ty = ty.clone();     // saved for cell binding shorthand
        let trg_annotations = self.parse_annotations()?;

        let mut address: crate::ast::LinkRef = crate::ast::LinkRef::Explicit(0);
        let mut bit_range: Option<BitRange> = None;

        loop {
            if let Some(Ok(Token::At)) = self.current_token() {
                self.advance();
                // Check for / immediately after @ (auto-allocate with bit-spec)
                if let Some(Ok(Token::Slash)) = self.current_token() {
                    self.advance();
                    bit_range = Some(self.parse_bit_range()?);
                } else {
                    match self.current_token() {
                        Some(Ok(Token::Integer(n))) => {
                            address = crate::ast::LinkRef::Explicit(*n as u64);
                            self.advance();
                        }
                        Some(Ok(Token::Link)) => {
                            self.advance();
                            // @ link <name> - use identifier after link keyword
                            let link_name = self.expect_identifier()?;
                            address = crate::ast::LinkRef::Linked(link_name);
                        }
                        Some(Ok(Token::Identifier(name))) => {
                            match name.as_str() {
                                "stdin" => {
                                    self.advance();
                                    self.expect(Token::Hash)?;
                                    address = crate::ast::LinkRef::Stdin;
                                }
                                "timer" => {
                                    self.advance();
                                    self.expect(Token::Hash)?;
                                    self.expect(Token::LParen)?;
                                    let hz = match self.current_token() {
                                        Some(Ok(Token::Integer(n))) => *n as u64,
                                        _ => return self.spanned_err("Expected integer Hz for @ timer#(Hz)".to_string()),
                                    };
                                    self.advance();
                                    self.expect(Token::RParen)?;
                                    address = crate::ast::LinkRef::Timer(hz);
                                }
                                "signal" => {
                                    self.advance();
                                    self.expect(Token::Hash)?;
                                    self.expect(Token::LParen)?;
                                    let sig_name = self.expect_identifier()?;
                                    self.expect(Token::RParen)?;
                                    address = crate::ast::LinkRef::Signal(sig_name);
                                }
                                _ => {
                                    let cell_ident = name.clone();
                                    self.advance(); // consume the identifier
                                    // Check for @ CellName! or @ CellName!.port shorthand
                                    if let Some(Ok(Token::Not)) = self.current_token() {
                                        self.advance(); // consume !
                                        let port = if let Some(Ok(Token::Dot)) = self.current_token() {
                                            self.advance();
                                            self.expect_identifier().unwrap_or_default()
                                        } else { String::new() };
                                        self.pending_cell_binding = Some((
                                            _trg_name.clone(), cell_ident, port, Some(_trg_ty.clone())
                                        ));
                                        address = crate::ast::LinkRef::Explicit(0);
                                    } else if let Some(Ok(Token::Dot)) = self.current_token() {
                                        // @ instance.port (no !) — cell binding with explicit port
                                        self.advance(); // consume .
                                        let port = self.expect_identifier().unwrap_or_default();
                                        self.pending_cell_binding = Some((
                                            _trg_name.clone(), cell_ident, port, Some(_trg_ty.clone())
                                        ));
                                        address = crate::ast::LinkRef::Explicit(0);
                                    } else {
                                        // Backward compat: @ identifier as link reference
                                        address = crate::ast::LinkRef::Linked(cell_ident);
                                    }
                                }
                            }
                        }
                        _ => return self.spanned_err("Expected integer address, 'link <name>', or / after '@'".to_string()),
                    }
                    if let Some(Ok(Token::Slash)) = self.current_token() {
                        self.advance();
                        bit_range = Some(self.parse_bit_range()?);
                    }
                }
            } else if let Some(Ok(Token::LBracket)) = self.current_token() {
                self.advance();
                bit_range = Some(self.parse_bit_range()?);
                self.expect(Token::RBracket)?;
            } else {
                break;
            }
        }

        let mut stages = Vec::new();
        if let Some(Ok(Token::On)) = self.current_token() {
            self.advance();
            self.expect(Token::Stage)?;
            stages.push(self.expect_identifier()?);
            while let Some(Ok(Token::Comma)) = self.current_token() {
                self.advance();
                stages.push(self.expect_identifier()?);
            }
        }

        let mut condition = None;
        if let Some(Ok(Token::LBracket)) = self.current_token() {
            self.advance();
            condition = Some(self.parse_expression()?);
            self.expect(Token::RBracket)?;
        }

        // Wake is now the default for all trigger types — the reactor re-evaluates on
        // every tick when any trigger is `is_wake`. Use `#nowake` for passive MMIO reads.
        let mut is_wake = true;
        if let Some(Ok(Token::Hash)) = self.current_token() {
            self.advance();
            if let Some(Ok(Token::Identifier(n))) = self.current_token() {
                if n == "nowake" {
                    self.advance();
                    is_wake = false;
                } else {
                    return self.spanned_err("Expected 'nowake' after '#' modifier".to_string());
                }
            }
        }

        let span = self.current_span();
        self.expect(Token::Semicolon)?;

        Ok(TriggerDeclaration {
            name,
            ty,
            address,
            bit_range,
            stages,
            condition,
            is_wake,
            is_const,
            span,
            annotations: trg_annotations,
            modifiers: vec![],
        })
    }

    fn parse_constant(&mut self) -> Result<Constant, SyntaxError> {
        self.expect(Token::Const)?;
        let name = self.expect_identifier()?;
        self.expect(Token::Colon)?;
        let ty = self.parse_type()?;
        self.expect(Token::Eq)?;
        let expr = self.parse_expression()?;
        self.expect(Token::Semicolon)?;
        Ok(Constant { name, ty, expr })
    }

    fn parse_transaction(&mut self) -> Result<Transaction, SyntaxError> {
        let mut is_async = false;
        let mut is_reactive = false;

        if let Some(Ok(Token::Async)) = self.current_token() {
            is_async = true;
            self.advance();
        }
        if let Some(Ok(Token::Rct)) = self.current_token() {
            is_reactive = true;
            self.advance();
            if let Some(Ok(Token::Async)) = self.current_token() {
                is_async = true;
                self.advance();
            }
        }

        self.expect(Token::Txn)?;
        let name = self.expect_identifier()?;
        let name = if let Some(Ok(Token::Dot)) = self.current_token() {
            self.advance();
            let method = self.expect_identifier()?;
            format!("{}.{}", name, method)
        } else {
            name
        };
        let txn_annotations = self.parse_annotations()?;

        // Parse optional parameters - NOT allowed for rct transactions
        let parameters = if let Some(Ok(Token::LParen)) = self.current_token() {
            self.advance();
            let mut params = Vec::new();
            loop {
                let param_result = self.expect_identifier();
                match param_result {
                    Ok(param_name) => {
                        self.expect(Token::Colon)?;
                        let param_type = self.parse_type()?;
                        params.push((param_name, param_type));
                        if let Some(Ok(Token::Comma)) = self.current_token() {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            self.expect(Token::RParen)?;
            params
        } else {
            Vec::new()
        };

        // Validate: rct transactions cannot have parameters
        if is_reactive && !parameters.is_empty() {
            return self.spanned_err("rct transactions cannot have parameters".to_string());
        }

        // Parse contracts — check before -> Type (canonical) and after (fallback)
        let mut contract = if let Some(Ok(Token::LBracket)) = self.current_token() {
            self.parse_contract()?
        } else {
            Contract::new(Expr::Bool(true), Expr::Bool(true))
        };

        // Parse optional return type for regular (non-reactive) txns
        let (txn_outputs, txn_output_type) = if !is_reactive && matches!(self.current_token(), Some(Ok(Token::Arrow))) {
            self.advance();
            let (outputs, _output_names) = self.parse_output_types_with_names(&parameters)?;
            let output_type = if outputs.len() > 1 {
                Some(crate::ast::OutputType::Tuple(outputs.iter().map(|t| crate::ast::OutputType::Single(t.clone())).collect()))
            } else {
                None
            };
            (outputs, output_type)
        } else {
            (Vec::new(), None)
        };

        // If contracts weren't before -> Type, check after (with soft hint)
        if !is_reactive && matches!(self.current_token(), Some(Ok(Token::LBracket))) {
            let c = self.parse_contract()?;
            if c.pre_condition != Expr::Bool(true) || c.post_condition != Expr::Bool(true) {
            }
            contract = c;
        }

        // Capture closing-brace span for better error messages on missing ';'
        let closing_brace_span;
        // Lambda-style: allow ; termination (no body)
        let body = if let Some(Ok(Token::Semicolon)) = self.current_token() {
            // Lambda-style transaction: no body, just contract
            closing_brace_span = Span::dummy();
            Vec::new()
        } else {
            self.expect(Token::LBrace)?;
            let body = self.parse_body()?;
            closing_brace_span = self.current_span().unwrap_or_else(Span::dummy);
            self.expect(Token::RBrace)?;
            body
        };

        let is_lambda = body.is_empty();

        // Multi-body dispatch: parse additional [pre]{body} variants
        let variant_bodies = if is_lambda {
            Vec::new()
        } else {
            self.parse_variant_bodies()?
        };

        let span = self.current_span();

        // NEW: Check for @Hz speed declaration after closing brace (for rct blocks)
        let reactor_speed = if is_reactive && matches!(self.current_token(), Some(Ok(Token::At))) {
            self.advance(); // consume @

            if let Some(Ok(Token::Integer(speed_num))) = self.current_token() {
                let speed = *speed_num as u32;
                self.advance();

                // Optional 'Hz'
                if let Some(Ok(Token::Identifier(hz))) = self.current_token() {
                    if hz == "Hz" {
                        self.advance();
                    }
                }

                if speed == 0 {
                    return self.spanned_err("Reactor speed must be positive".to_string());
                }
                if speed >= 10000 {
                    eprintln!("warning: Unusually high reactor speed @{}Hz", speed);
                }
                Some(speed)
            } else {
                return self.spanned_err("Expected numeric speed after '@'".to_string());
            }
        } else {
            None
        };

        match self.current_token() {
            Some(Ok(Token::Semicolon)) => { self.advance(); }
            Some(Ok(tok)) => {
                return Err(SyntaxError::UnexpectedToken {
                    expected: format!(
                        "';' after {} block — all {} declarations must end with '}};'",
                        if is_reactive { "rct txn" } else { "transaction" },
                        if is_reactive { "rct txn" } else { "txn" },
                    ),
                    found: Self::token_display(tok),
                    span: self.current_span().unwrap_or_else(Span::dummy),
                });
            }
            Some(Err(_)) => {
                return Err(SyntaxError::InvalidStatement {
                    reason: "Lexer error".to_string(),
                    span: self.current_span().unwrap_or_else(Span::dummy),
                });
            }
            None => {
                return Err(SyntaxError::UnexpectedEOF {
                    expected: format!(
                        "';' after {} block — all {} declarations must end with '}};'",
                        if is_reactive { "rct txn" } else { "transaction" },
                        if is_reactive { "rct txn" } else { "txn" },
                    ),
                    span: closing_brace_span,
                });
            }
        }

        let dependencies = contract
            .pre_condition
            .extract_dependencies()
            .into_iter()
            .collect();

        if is_reactive && !is_lambda && !Self::has_term_or_escape_in_tree(&body)
            && !Self::is_convergent_contract_pair(&contract.pre_condition, &contract.post_condition)
        {
            return Err(SyntaxError::InvalidStatement {
                reason: "reactive transaction has no valid termination — add term;, escape;, or a convergent contract like [count < N][count == N]".to_string(),
                span: closing_brace_span,
            });
        }

        Ok(Transaction {
            is_async,
            is_reactive,
            name,
            parameters,
            contract,
            body,
            reactor_speed,
            span,
            is_lambda,
            dependencies,
            annotations: txn_annotations,
            modifiers: Vec::new(),
            variant_bodies,
            outputs: txn_outputs,
            output_type: txn_output_type,
        })
    }

    fn parse_definition(&mut self) -> Result<Definition, SyntaxError> {
        // def/defn/definition all map to Token::Defn via lexer aliases
        self.expect(Token::Defn)?;
        let name = self.expect_identifier()?;
        let defn_annotations = self.parse_annotations()?;

        let type_params = if let Some(Ok(Token::Lt)) = self.current_token() {
            self.advance();
            let mut params = Vec::new();
            loop {
                let param_name = self.expect_identifier()?;
                let mut bounds = Vec::new();
                if let Some(Ok(Token::Colon)) = self.current_token() {
                    self.advance();
                    loop {
                        let bound_name = self.expect_identifier()?;
                        bounds.push(TypeBound::HasTrait(bound_name));
                        if let Some(Ok(Token::Plus)) = self.current_token() {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                params.push(TypeParam {
                    name: param_name,
                    bounds,
                });
                if let Some(Ok(Token::Comma)) = self.current_token() {
                    self.advance();
                } else {
                    break;
                }
            }
            self.expect(Token::Gt)?;
            params
        } else {
            Vec::new()
        };

        let parameters = if let Some(Ok(Token::LParen)) = self.current_token() {
            self.advance();
            let mut params = Vec::new();
            loop {
                let param_result = self.expect_identifier();
                match param_result {
                    Ok(param_name) => {
                        self.expect(Token::Colon)?;
                        let param_type = self.parse_type()?;
                        params.push((param_name, param_type));
                        if let Some(Ok(Token::Comma)) = self.current_token() {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            self.expect(Token::RParen)?;
            params
        } else {
            Vec::new()
        };

        let (outputs, output_names, output_type, contract) = if let Some(Ok(Token::LBracket)) = self.current_token() {
            // Contract before arrow: defn name(params) [pre][post] -> Type
            let contract = self.parse_contract()?;
            if let Some(Ok(Token::Arrow)) = self.current_token() {
                self.advance();
                let (outputs, output_names) = self.parse_output_types_with_names(&parameters)?;
                let output_type = if outputs.len() > 1 {
                    Some(crate::ast::OutputType::Tuple(outputs.iter().map(|t| crate::ast::OutputType::Single(t.clone())).collect()))
                } else {
                    None
                };
                (outputs, output_names, output_type, contract)
            } else {
                (Vec::new(), Vec::new(), None, contract)
            }
        } else if let Some(Ok(Token::Arrow)) = self.current_token() {
            self.advance();
            let (outputs, output_names) = self.parse_output_types_with_names(&parameters)?;
            let output_type = if outputs.len() > 1 {
                Some(crate::ast::OutputType::Tuple(outputs.iter().map(|t| crate::ast::OutputType::Single(t.clone())).collect()))
            } else {
                None
            };
            let contract = if let Some(Ok(Token::LBracket)) = self.current_token() {
                // Path B: contract after return type — works but [pre][post] -> Type
                // is the canonical ordering and avoids confusion with Type[expr] bounds.
                // We parse it but emit a soft hint so users learn the canonical form.
                let c = self.parse_contract()?;
                if c.pre_condition != Expr::Bool(true) || c.post_condition != Expr::Bool(true) {
                }
                c
            } else {
                Contract::new(Expr::Bool(true), Expr::Bool(true))
            };
            (outputs, output_names, output_type, contract)
        } else {
            (Vec::new(), Vec::new(), None, Contract::new(Expr::Bool(true), Expr::Bool(true)))
        };

        // Lambda-style: allow ; termination (no body)
        let body = if let Some(Ok(Token::Semicolon)) = self.current_token() {
            Vec::new()
        } else {
            self.expect(Token::LBrace)?;
            let body = self.parse_body()?;
            self.expect(Token::RBrace)?;
            body
        };

        let is_lambda = body.is_empty();

        // Multi-body dispatch: parse additional [pre]{body} variants
        let variant_bodies = if is_lambda {
            Vec::new()
        } else {
            self.parse_variant_bodies()?
        };

        // Semicolon after body or last variant body
        if let Some(Ok(Token::Semicolon)) = self.current_token() {
            self.advance();
        }

        Ok(Definition {
            name,
            type_params,
            parameters,
            outputs,
            output_type,
            output_names,
            contract,
            body,
            is_lambda,
            annotations: defn_annotations,
            modifiers: Vec::new(),
            variant_bodies,
        })
    }

    fn parse_cell_definition(&mut self, is_persistent: bool) -> Result<CellDef, SyntaxError> {
        let name = self.expect_identifier()?;

        let type_params = if let Some(Ok(Token::Lt)) = self.current_token() {
            self.advance();
            let mut params = Vec::new();
            loop {
                let param_name = self.expect_identifier()?;
                let mut bounds = Vec::new();
                if let Some(Ok(Token::Colon)) = self.current_token() {
                    self.advance();
                    loop {
                        let bound_name = self.expect_identifier()?;
                        bounds.push(TypeBound::HasTrait(bound_name));
                        if let Some(Ok(Token::Plus)) = self.current_token() {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                params.push(TypeParam {
                    name: param_name,
                    bounds,
                });
                if let Some(Ok(Token::Comma)) = self.current_token() {
                    self.advance();
                } else {
                    break;
                }
            }
            self.expect(Token::Gt)?;
            params
        } else {
            Vec::new()
        };

        let parameters = if let Some(Ok(Token::LParen)) = self.current_token() {
            self.advance();
            let mut params = Vec::new();
            loop {
                let param_result = self.expect_identifier();
                match param_result {
                    Ok(param_name) => {
                        self.expect(Token::Colon)?;
                        let param_type = self.parse_type()?;
                        params.push((param_name, param_type));
                        if let Some(Ok(Token::Comma)) = self.current_token() {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            self.expect(Token::RParen)?;
            params
        } else {
            Vec::new()
        };

        let (outputs, output_names, output_type) = if let Some(Ok(Token::Arrow)) = self.current_token() {
            self.advance();
            let (outs, names) = self.parse_output_types_with_names(&parameters)?;
            let out_ty = if outs.len() > 1 {
                Some(OutputType::Tuple(outs.iter().enumerate().map(|(i, t)| {
                    if let Some(Some(name)) = names.get(i) {
                        OutputType::Named(name.clone(), Box::new(OutputType::Single(t.clone())))
                    } else {
                        OutputType::Single(t.clone())
                    }
                }).collect()))
            } else if outs.len() == 1 {
                if let Some(Some(name)) = names.first() {
                    Some(OutputType::Named(name.clone(), Box::new(OutputType::Single(outs[0].clone()))))
                } else {
                    Some(OutputType::Single(outs[0].clone()))
                }
            } else {
                None
            };
            (outs, names, out_ty)
        } else {
            (Vec::new(), Vec::new(), None)
        };

        // Parse body: { fields; txns; defns; trgs; }
        self.expect(Token::LBrace)?;

        let mut fields = Vec::new();
        let mut transactions = Vec::new();
        let mut definitions = Vec::new();
        let mut internal_triggers = Vec::new();

        while let Some(token) = self.current_token() {
            if let Ok(Token::RBrace) = token {
                break;
            }

            match token {
                Ok(Token::Rct) | Ok(Token::Txn) => {
                    let mut txn = self.parse_transaction()?;
                    txn.modifiers = self.parse_hashtag_modifiers()?;
                    transactions.push(txn);
                }
                Ok(Token::Defn) => {
                    let mut defn = self.parse_definition()?;
                    defn.modifiers = self.parse_hashtag_modifiers()?;
                    definitions.push(defn);
                }
                Ok(Token::Trg) => {
                    self.advance();
                    let trg = self.parse_trigger_body(false)?;
                    internal_triggers.push(trg);
                }
                _ => {
                    // Try to parse as a state field: name: Type = init;
                    let field_name = self.expect_identifier()?;
                    self.expect(Token::Colon)?;
                    let field_type = self.parse_type()?;
                    let default = if let Some(Ok(Token::Eq)) = self.current_token() {
                        self.advance();
                        Some(self.parse_expression()?)
                    } else {
                        None
                    };
                    self.expect(Token::Semicolon)?;
                    fields.push(StructField {
                        name: field_name,
                        ty: field_type,
                        default,
                        visibility: Visibility::Private,
                    });
                }
            }
        }

        self.expect(Token::RBrace)?;

        if let Some(Ok(Token::Semicolon)) = self.current_token() {
            self.advance();
        }

        Ok(CellDef {
            is_persistent,
            name,
            type_params,
            parameters,
            output_type,
            fields,
            transactions,
            definitions,
            internal_triggers,
            span: None,
            modifiers: Vec::new(),
        })
    }

    fn parse_template_def(&mut self) -> Result<(String, Vec<(String, MacroArgType)>, Option<MacroArgType>, Vec<Statement>), SyntaxError> {
        self.expect(Token::Template)?;
        let name = self.expect_identifier()?;
        self.expect(Token::LParen)?;
        let mut params = Vec::new();
        if !matches!(self.current_token(), Some(Ok(Token::RParen))) {
            loop {
                let param_name = self.expect_identifier()?;
                self.expect(Token::Colon)?;
                let arg_type = self.parse_macro_arg_type()?;
                params.push((param_name, arg_type));
                if let Some(Ok(Token::Comma)) = self.current_token() {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        self.expect(Token::RParen)?;

        let return_type = if let Some(Ok(Token::Arrow)) = self.current_token() {
            self.advance();
            Some(self.parse_macro_arg_type()?)
        } else {
            None
        };

        self.expect(Token::LBrace)?;
        let body = self.parse_body()?;
        self.expect(Token::RBrace)?;

        if let Some(Ok(Token::Semicolon)) = self.current_token() {
            self.advance();
        }

        Ok((name, params, return_type, body))
    }

    fn parse_macro_def(&mut self) -> Result<(String, Vec<(String, MacroArgType)>, Option<MacroArgType>, Vec<Statement>), SyntaxError> {
        self.expect(Token::Macro)?;
        let name = self.expect_identifier()?;
        self.expect(Token::LParen)?;
        let mut params = Vec::new();
        if !matches!(self.current_token(), Some(Ok(Token::RParen))) {
            loop {
                let param_name = self.expect_identifier()?;
                self.expect(Token::Colon)?;
                let arg_type = self.parse_macro_arg_type()?;
                params.push((param_name, arg_type));
                if let Some(Ok(Token::Comma)) = self.current_token() {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        self.expect(Token::RParen)?;

        let return_type = if let Some(Ok(Token::Arrow)) = self.current_token() {
            self.advance();
            Some(self.parse_macro_arg_type()?)
        } else {
            None
        };

        self.expect(Token::LBrace)?;
        let body = self.parse_body()?;
        self.expect(Token::RBrace)?;

        if let Some(Ok(Token::Semicolon)) = self.current_token() {
            self.advance();
        }

        Ok((name, params, return_type, body))
    }

    fn parse_macro_arg_type(&mut self) -> Result<MacroArgType, SyntaxError> {
        let ident = self.expect_identifier()?;
        match ident.as_str() {
            "Expr" => Ok(MacroArgType::Expr),
            "Stmt" => Ok(MacroArgType::Stmt),
            "Block" => Ok(MacroArgType::Block),
            "Type" => Ok(MacroArgType::Type),
            "Int" => Ok(MacroArgType::Int),
            "String" => Ok(MacroArgType::String),
            "Bool" => Ok(MacroArgType::Bool),
            _ => self.spanned_err(format!("Invalid macro argument type: '{}'. Expected Expr, Stmt, Block, Type, Int, String, or Bool", ident)),
        }
    }

    fn parse_output_types(&mut self) -> Result<Vec<Type>, SyntaxError> {
        let mut outputs = Vec::new();
        outputs.push(self.parse_type()?);
        while let Some(Ok(Token::Comma)) = self.current_token() {
            self.advance();
            outputs.push(self.parse_type()?);
        }
        Ok(outputs)
    }

    /// Parse output types with optional names: `Bool`, `result: Bool`, or mixed
    /// Returns (output_types, output_names) where output_names is parallel to output_types
    fn parse_output_types_with_names(
        &mut self,
        parameters: &[(String, Type)],
    ) -> Result<(Vec<Type>, Vec<Option<String>>), SyntaxError> {
        let mut outputs = Vec::new();
        let mut names = Vec::new();
        let param_names: std::collections::HashSet<String> =
            parameters.iter().map(|(n, _)| n.clone()).collect();
        let mut seen_names = std::collections::HashSet::new();

        loop {
            // Check if we're at the contract section (next token is [)
            // If so, we're done parsing output types
            if let Some(Ok(Token::LBracket)) = self.current_token() {
                break;
            }

            // Check if next token is an identifier followed by colon (indicates a name)
            let name = if let Some(Ok(Token::Identifier(id))) = self.current_token() {
                let id = id.clone();

                // Check if next token is colon (peek token)
                if let Some(Ok(Token::Colon)) = self.peek() {
                    // This is a named output
                    self.advance(); // consume identifier
                    self.advance(); // consume colon

                    // Check for duplicate names
                    if seen_names.contains(&id) {
                        return self.spanned_err(format!("Duplicate output name: '{}'", id));
                    }

                    // Check for shadowing parameters
                    if param_names.contains(&id) {
                        return self.spanned_err(format!("Output name '{}' shadows parameter", id));
                    }

                    seen_names.insert(id.clone());
                    Some(id)
                } else {
                    // Not a named output
                    None
                }
            } else {
                None
            };

            // Parse the type (no ContractBound — brackets belong to contract, not type)
            outputs.push(self.parse_type_inner(false)?);
            names.push(name);

            // Check for comma (tuple separator) or pipe (union)
            match self.current_token() {
                Some(Ok(Token::Comma)) => {
                    self.advance();
                }
                Some(Ok(Token::Pipe)) => {
                    // Union detected - continue parsing union members
                    self.advance();
                }
                _ => {
                    break;
                }
            }
        }

        Ok((outputs, names))
    }

    /// Detect and parse output type structure: Single | Union | Tuple
    /// Returns OutputType for Feature A multi-output support
    /// Syntax:
    ///   -> Bool                    (Single)
    ///   -> Bool | Error            (Union)
    ///   -> Bool, String            (Tuple)
    ///   -> Bool | Error, String    (Mixed: Union then Tuple element)
    fn parse_output_type_structure(&mut self) -> Result<OutputType, SyntaxError> {
        use crate::ast::OutputType;

        let result = self.parse_union()?;
        Ok(result)
    }

    /// Parse union: product ("|" product)*
    fn parse_union(&mut self) -> Result<OutputType, SyntaxError> {
        let mut alternatives = vec![self.parse_product()?];
        while let Some(Ok(Token::Pipe)) = self.current_token() {
            self.advance();
            alternatives.push(self.parse_product()?);
        }
        if alternatives.len() == 1 {
            Ok(alternatives.into_iter().next().unwrap())
        } else {
            Ok(OutputType::Union(alternatives))
        }
    }

    /// Parse product: slot ("," slot)*
    fn parse_product(&mut self) -> Result<OutputType, SyntaxError> {
        let mut slots = vec![self.parse_slot()?];
        while let Some(Ok(Token::Comma)) = self.current_token() {
            self.advance();
            slots.push(self.parse_slot()?);
        }
        if slots.len() == 1 {
            Ok(slots.into_iter().next().unwrap())
        } else {
            Ok(OutputType::Tuple(slots))
        }
    }

    /// Parse slot: [name ":"] array
    fn parse_slot(&mut self) -> Result<OutputType, SyntaxError> {
        // Check for named slot pattern: Identifier ':' Type[]
        if let Some(Ok(Token::Identifier(_))) = self.current_token() {
            if let Some(Ok(Token::Colon)) = self.peek() {
                let name = self.expect_identifier()?;
                self.advance(); // consume ':'
                let inner = self.parse_type()?;
                // Check for [] suffix on named type
                if let Some(Ok(Token::LBracket)) = self.current_token() {
                    self.advance();
                    self.expect(Token::RBracket)?;
                    return Ok(OutputType::Named(name, Box::new(OutputType::Array(Box::new(inner)))));
                }
                return Ok(OutputType::Named(name, Box::new(OutputType::Single(inner))));
            }
        }
        // Plain type (maybe with [] suffix)
        let ty = self.parse_type()?;
        if let Some(Ok(Token::LBracket)) = self.current_token() {
            self.advance();
            self.expect(Token::RBracket)?;
            Ok(OutputType::Array(Box::new(ty)))
        } else {
            Ok(OutputType::Single(ty))
        }
    }

    fn parse_result_type(&mut self) -> Result<ResultType, SyntaxError> {
        if let Some(Ok(Token::BoolTrue)) = self.current_token() {
            self.advance();
            return Ok(ResultType::TrueAssertion);
        }

        let mut outputs = Vec::new();
        outputs.push(self.parse_type()?);
        while let Some(Ok(Token::Comma)) = self.current_token() {
            self.advance();
            outputs.push(self.parse_type()?);
        }

        Ok(ResultType::Projection(outputs))
    }

    fn parse_term_outputs(&mut self) -> Result<Vec<Option<Expr>>, SyntaxError> {
        let mut outputs = Vec::new();

        if let Some(Ok(Token::Semicolon)) = self.current_token() {
            return Ok(outputs);
        }

        // Stop at hashtag tokens so caller can parse modifiers
        if matches!(self.current_token(), Some(Ok(Token::Hash | Token::HashBang))) {
            return Ok(outputs);
        }

        // Stop at arrow token — swan song follows
        if let Some(Ok(Token::Arrow)) = self.current_token() {
            return Ok(outputs);
        }

        outputs.push(Some(self.parse_expression()?));

        while let Some(Ok(Token::Comma)) = self.current_token() {
            self.advance();
            if let Some(Ok(Token::Comma)) = self.current_token() {
                outputs.push(None);
            } else if let Some(Ok(Token::Semicolon)) = self.current_token() {
                outputs.push(None);
            } else {
                outputs.push(Some(self.parse_expression()?));
            }
        }

        Ok(outputs)
    }

    /// Parse optional annotations after a declaration name.
    /// Syntax: `<~ name: expr, #shorthand, ...`
    /// Returns empty vec if no `<~` token is present.
    fn parse_annotations(&mut self) -> Result<Vec<TypeBinding>, SyntaxError> {
        if !matches!(self.current_token(), Some(Ok(Token::TildeArrow))) {
            return Ok(Vec::new());
        }
        self.advance(); // consume <~
        let mut annotations = Vec::new();
        loop {
            // #shorthand desugars to name: true
            if matches!(self.current_token(), Some(Ok(Token::Hash))) {
                self.advance();
                let name = self.expect_identifier()?;
                annotations.push(TypeBinding {
                    name,
                    params: vec![],
                    value: Box::new(Expr::Bool(true)),
                    span: self.current_span(),
                });
            } else {
                let name = self.expect_identifier()?;
                self.expect(Token::Colon)?;
                let value = self.parse_expression()?;
                annotations.push(TypeBinding {
                    name,
                    params: vec![],
                    value: Box::new(value),
                    span: self.current_span(),
                });
            }
            if matches!(self.current_token(), Some(Ok(Token::Comma))) {
                self.advance();
            } else {
                break;
            }
        }
        Ok(annotations)
    }

    fn parse_contract(&mut self) -> Result<Contract, SyntaxError> {
        let mut pre_condition = Expr::Bool(true);
        let mut post_condition = Expr::Bool(true);
        let mut watchdog: Option<WatchdogSpec> = None;

        let mut count = 0;
        while let Some(Ok(Token::LBracket)) = self.current_token() {
            self.advance(); // consume [

            // Check for ~/ syntax - [~/expr] is shorthand for [!expr][expr]
            // [~/!var] → pre=var, post=!var (inverted toggle)
            if let Some(Ok(Token::TildeSlash)) = self.current_token() {
                self.advance(); // Consume ~/
                let expr = self.parse_expression()?;
                pre_condition = Expr::UnaryOp(Box::new(UnaryOpExpr::new(UnaryOpKind::Not, expr.clone())));
                post_condition = expr;
                self.expect(Token::RBracket)?;
                count = 2; // ~/ provides both pre and post
                break;
            }

            // [[post] shorthand: second [ means pre is omitted
            if count == 0 && matches!(self.current_token(), Some(Ok(Token::LBracket))) {
                self.advance(); // consume inner [
                pre_condition = Expr::Bool(true);
                post_condition = self.parse_expression()?;
                self.expect(Token::RBracket)?;
                count = 2;
                continue;
            }

            // [pre]] shorthand: empty brackets mean [true] for post
            if count == 0 && matches!(self.current_token(), Some(Ok(Token::RBracket))) {
                self.advance(); // consume ]
                pre_condition = Expr::Bool(true);
                count = 1;
                continue; // continue to parse post normally
            }

            if count == 0 {
                pre_condition = self.parse_expression()?;
            } else if count == 1 {
                post_condition = self.parse_expression()?;
            } else {
                // count >= 2 — no more contract brackets allowed
                return self.spanned_err("Too many contract brackets (max 3: [pre][post][watchdog])".to_string());
            }

            count += 1;
            self.expect(Token::RBracket)?;

            // [pre]] shorthand: after pre's ], if next token is ], set post = true
            if count == 1 && matches!(self.current_token(), Some(Ok(Token::RBracket))) {
                self.advance(); // consume extra ]
                post_condition = Expr::Bool(true);
                count = 2;
            }
        }

        // Single-bracket contracts are ambiguous: is it a precondition or postcondition?
        // Use [pre]] to omit postcondition or [[post] to omit precondition.
        if count == 1 {
            return self.spanned_err(
                "single-bracket contract is ambiguous — use [pre]] to omit postcondition or [[post] to omit precondition".to_string()
            );
        }

        // External watchdog: ?[cond], ?![cond], or ?#[cond] (after all bracket pairs)
        if let Some(Ok(Token::Question)) = self.current_token() {
            self.advance();
            let is_required = if let Some(Ok(Token::Not)) = self.current_token() {
                self.advance();
                true // ?!
            } else {
                false // ?
            };
            let is_proven = if let Some(Ok(Token::Hash)) = self.current_token() {
                self.advance();
                true
            } else {
                false
            };
            self.expect(Token::LBracket)?;
            let cond = self.parse_expression()?;
            self.expect(Token::RBracket)?;
            if cond.as_bool() == Some(true) {
                return self.spanned_err("Watchdog cannot be [true] - must verify something".to_string());
            }
            let (cycles_bound, seconds_bound) = Self::extract_timing_bound(&cond);
            watchdog = Some(WatchdogSpec {
                cycles_bound,
                seconds_bound,
                condition: cond,
                is_required,
                is_proven,
                retries: 0,
                fallback: None,
            });
        }

        // [true][true] is always an error when the user wrote brackets — defeats contract-first
        // Bracketless (count == 0) is deliberate omission, not a contract claim.
        if count > 0 && pre_condition.as_bool() == Some(true) && post_condition.as_bool() == Some(true) {
            return self.spanned_err(
                "both precondition and postcondition are [true] — at least one side must specify meaningful constraints".to_string()
            );
        }

        // In strict mode, both pre and post conditions are required and must be non-trivial
        if self.strict_mode.is_strict() {
            if count < 2 {
                return self.spanned_err(
                    "Strict mode requires both [precondition] and [postcondition]".to_string()
                );
            }

            if count == 0 {
                pre_condition = self.parse_expression()?;
            } else if count == 1 {
                post_condition = self.parse_expression()?;
            } else if count == 2 {
                // Watchdog specification - third bracket
                //
                // Syntax: [watchdog]       -> required (default)
                // Syntax: [?watchdog]      -> optional

                let is_optional = match self.current_token() {
                    Some(Ok(Token::Question)) => {
                        self.advance(); // consume ?
                        true
                    }
                    _ => false,
                };

                let cond = self.parse_expression()?;

                if cond.as_bool() == Some(true) {
                    return self.spanned_err("Watchdog cannot be [true] - must verify something".to_string());
                }

                let (cycles_bound, seconds_bound) = Self::extract_timing_bound(&cond);
                watchdog = Some(WatchdogSpec {
                    cycles_bound,
                    seconds_bound,
                    condition: cond,
                    is_required: !is_optional, // default is required
                    is_proven: false,
                    retries: 0,
                    fallback: None,
                });
            } else {
                return self.spanned_err("Too many contract brackets (max 3: [pre][post][watchdog])".to_string());
            }

            count += 1;
            self.expect(Token::RBracket)?;
        }

        // [true][n >= 0] is always an error — defeats contract-first programming
        if count > 0 && pre_condition.as_bool() == Some(true) && post_condition.as_bool() == Some(true) {
            return self.spanned_err(
                "both precondition and postcondition are [true] — at least one side must specify meaningful constraints".to_string()
            );
        }

        // In strict mode, both pre and post conditions are required and must be non-trivial
        if self.strict_mode.is_strict() {
            if count < 2 {
                return self.spanned_err(
                    "Strict mode requires both [precondition] and [postcondition]".to_string()
                );
            }
            if pre_condition.as_bool() == Some(true) {
                return self.spanned_err(
                    "Strict mode: precondition [true] is not allowed - specify actual state requirements".to_string()
                );
            }
            if post_condition.as_bool() == Some(true) {
                return self.spanned_err(
                    "Strict mode: postcondition [true] is not allowed - specify actual state guarantees".to_string()
                );
            }
        }

        let span = self.current_span();
        Ok(Contract {
            pre_condition,
            post_condition,
            watchdog,
            span,
        })
    }

    fn extract_timing_bound(expr: &Expr) -> (Option<u64>, Option<u64>) {
        match expr {
            Expr::Lt(left, right) => {
                if let (Expr::Identifier(name), Expr::Integer(n)) = (left.as_ref(), right.as_ref()) {
                    if *n >= 0 {
                        if name == "cycles" {
                            return (Some(*n as u64), None);
                        }
                        if name == "seconds" {
                            return (None, Some(*n as u64));
                        }
                    }
                }
                (None, None)
            }
            _ => (None, None),
        }
    }

    fn parse_body(&mut self) -> Result<Vec<Statement>, SyntaxError> {
        let mut statements = Vec::new();
        while let Some(token) = self.current_token() {
            if let Ok(Token::RBrace) = token {
                break;
            }
            let stmt = self.parse_statement()?;
            statements.push(stmt);
        }
        Ok(statements)
    }

    /// Parse a time unit keyword. Returns an error if no time unit is found.
    fn parse_time_unit(&mut self) -> Result<TimeUnit, SyntaxError> {
        match self.current_token() {
            Some(Ok(Token::Cycles)) => { self.advance(); Ok(TimeUnit::Cycles) }
            Some(Ok(Token::Cyc)) => { self.advance(); Ok(TimeUnit::Cycles) }
            Some(Ok(Token::Ms)) => { self.advance(); Ok(TimeUnit::Ms) }
            Some(Ok(Token::Seconds)) => { self.advance(); Ok(TimeUnit::Seconds) }
            Some(Ok(Token::Minute)) => { self.advance(); Ok(TimeUnit::Minutes) }
            Some(Ok(Token::Minutes)) => { self.advance(); Ok(TimeUnit::Minutes) }
            Some(Ok(Token::Nanoseconds)) => { self.advance(); Ok(TimeUnit::Nanoseconds) }
            _ => {
                // Also accept identifiers: cyc, s, ms, min, ns
                if let Some(Ok(Token::Identifier(s))) = self.current_token() {
                    match s.as_str() {
                        "s" | "sec" | "seconds" | "secs" => { self.advance(); Ok(TimeUnit::Seconds) }
                        "ms" => { self.advance(); Ok(TimeUnit::Ms) }
                        "min" | "mins" | "minutes" => { self.advance(); Ok(TimeUnit::Minutes) }
                        "ns" | "nanos" | "nanoseconds" => { self.advance(); Ok(TimeUnit::Nanoseconds) }
                        "cyc" | "cycles" => { self.advance(); Ok(TimeUnit::Cycles) }
                        _ => self.spanned_err("Expected time unit: cyc, s, ms, min, ns, or full name".to_string())
                    }
                } else {
                    self.spanned_err("Expected time unit: cyc, s, ms, min, ns, or full name".to_string())
                }
            }
        }
    }

    /// Parse a time unit keyword, defaulting to TimeUnit::Cycles if absent.
    fn parse_time_unit_or_default(&mut self) -> Result<TimeUnit, SyntaxError> {
        match self.current_token() {
            Some(Ok(Token::Cycles)) | Some(Ok(Token::Cyc))
            | Some(Ok(Token::Ms)) | Some(Ok(Token::Seconds))
            | Some(Ok(Token::Minute)) | Some(Ok(Token::Minutes))
            | Some(Ok(Token::Nanoseconds)) => self.parse_time_unit(),
            Some(Ok(Token::Identifier(s))) => match s.as_str() {
                "cyc" | "cycles" | "s" | "sec" | "secs" | "seconds"
                | "ms" | "min" | "mins" | "minutes" | "ns" | "nanos" | "nanoseconds" => self.parse_time_unit(),
                _ => Ok(TimeUnit::Cycles),
            },
            _ => Ok(TimeUnit::Cycles),
        }
    }

    /// Extract the base identifier from a potentially nested access expression.
    /// For `program.items[i]` → returns "program".
    fn get_base_identifier(&self, expr: &Expr) -> String {
        match expr {
            Expr::Identifier(n) => n.clone(),
            Expr::ListIndex(inner, _) | Expr::FieldAccess(inner, _) => self.get_base_identifier(inner),
            _ => String::new(),
        }
    }

    /// Supports block style: { stmts } or expression.
    fn parse_unification_rhs(&mut self) -> Result<Expr, SyntaxError> {
        if let Some(Ok(Token::LBrace)) = self.current_token() {
            self.advance();
            let mut stmts = Vec::new();
            loop {
                if let Some(Ok(Token::RBrace)) = self.current_token() {
                    self.advance();
                    break;
                }
                stmts.push(self.parse_statement()?);
            }
            Ok(Expr::Block(stmts, Box::new(Expr::Bool(true))))
        } else {
            self.parse_expression()
        }
    }

    /// Parse a sequence of pattern fields inside parentheses, handling
    /// nested groups, string literals, identifiers, and wildcards.
    fn parse_pattern_fields(&mut self) -> Result<Vec<Pattern>, SyntaxError> {
        let mut fields = Vec::new();
        loop {
            match self.current_token() {
                Some(Ok(Token::Underscore)) => {
                    fields.push(Pattern::Wildcard);
                    self.advance();
                }
                Some(Ok(Token::String(s))) => {
                    let val = s.clone();
                    fields.push(Pattern::LitString(val));
                    self.advance();
                }
                Some(Ok(Token::LParen)) => {
                    self.advance();
                    let inner = self.parse_pattern_fields()?;
                    self.expect(Token::RParen)?;
                    fields.push(Pattern::Tuple(inner));
                }
                Some(Ok(Token::Integer(val))) => {
                    fields.push(Pattern::LitInt(*val));
                    self.advance();
                }
                Some(Ok(Token::Float(val))) => {
                    fields.push(Pattern::LitFloat(*val));
                    self.advance();
                }
                Some(Ok(Token::BoolTrue)) => {
                    fields.push(Pattern::LitBool(true));
                    self.advance();
                }
                Some(Ok(Token::BoolFalse)) => {
                    fields.push(Pattern::LitBool(false));
                    self.advance();
                }
                Some(Ok(Token::Char(c))) => {
                    fields.push(Pattern::LitChar(*c));
                    self.advance();
                }
                _ => {
                    match self.expect_identifier() {
                        Ok(name) => fields.push(Pattern::Var(name)),
                        Err(_) => break,
                    }
                }
            }
            if let Some(Ok(Token::Comma)) = self.current_token() {
                self.advance();
            } else {
                break;
            }
        }
        Ok(fields)
    }
    /// or .field access, but stopping before (Pattern) which belongs to unification.
    fn parse_uni_target(&mut self) -> Result<Expr, SyntaxError> {
        let ident = self.expect_identifier()?;
        let mut expr = Expr::Identifier(ident);
        loop {
            match self.current_token() {
                Some(Ok(Token::Dot)) => {
                    self.advance();
                    let field = self.expect_identifier()?;
                    if let Some(Ok(Token::LParen)) = self.current_token() {
                        return Ok(Expr::FieldAccess(Box::new(expr), field));
                    }
                    expr = Expr::FieldAccess(Box::new(expr), field);
                }
                Some(Ok(Token::LBracket)) => {
                    self.advance();
                    let index = self.parse_expression()?;
                    self.expect(Token::RBracket)?;
                    expr = Expr::ListIndex(Box::new(expr), Box::new(index));
                }
                // Stop at LParen — it belongs to the unification pattern
                Some(Ok(Token::LParen)) => break,
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_statement(&mut self) -> Result<Statement, SyntaxError> {
        match self.current_token() {
            Some(Ok(Token::Let)) => {
                self.advance();
                
                // Check for tuple destructuring: let (a, b) = expr;
                if let Some(Ok(Token::LParen)) = self.current_token() {
                    self.advance();
                    let mut names = Vec::new();
                    loop {
                        if matches!(self.current_token(), Some(Ok(Token::Underscore))) {
                            names.push("_".to_string());
                            self.advance();
                        } else {
                            names.push(self.expect_identifier()?);
                        }
                        if let Some(Ok(Token::Comma)) = self.current_token() {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    self.expect(Token::RParen)?;
                    
                    let ty = if let Some(Ok(Token::Colon)) = self.current_token() {
                        self.advance();
                        Some(self.parse_type()?)
                    } else {
                        None
                    };

                    // Check for `<:` subtype projection (string match destructuring)
                    if self.check_lt_colon() {
                        self.advance(); // consume `<:`
                        let source = self.parse_projection_source()?;
                        let ops = self.parse_subtype_ops()?;
                        self.expect(Token::Semicolon)?;
                        return Ok(Statement::Let {
                            name: names.join(","),
                            ty,
                            expr: Some(Expr::SubtypeProjection {
                                source: Box::new(source),
                                ops,
                            }),
                            address: None,
                            address_expr: None,
                            bit_range: None,
                            is_override: false,
                            modifiers: Vec::new(),
                            constraint: None,
                        });
                    }
                    
                    self.expect(Token::Eq)?;
                    let expr = self.parse_expression()?;
                    self.expect(Token::Semicolon)?;
                    
                    // Create nested let statements for each tuple element
                    // For now, create a single Let with the first name and use tuple indexing
                    // A more complete solution would create multiple let statements
                    Ok(Statement::Let {
                        name: names.join(","),
                        ty,
                        expr: Some(Expr::TupleDestructure(names, Box::new(expr))),
                        address: None,
                        address_expr: None,
                        bit_range: None,
                        is_override: false,
                        modifiers: Vec::new(),
                        constraint: None,
                    })
                } else {
                    let name = if let Some(Ok(Token::Underscore)) = self.current_token() {
                        self.advance();
                        "_".to_string()
                    } else {
                        self.expect_identifier()?
                    };

                    // Check for `<:` — constraint or subtype projection
                    if self.check_lt_colon() {
                        self.advance(); // consume `<:`
                        if let Some(Ok(Token::LBracket)) = self.current_token() {
                            // Constraint syntax: let name <: [expr];
                            self.advance();
                            let constraint = Some(self.parse_constraint_expr()?);
                            self.expect(Token::RBracket)?;
                            let expr = if let Some(Ok(Token::Eq)) = self.current_token() {
                                self.advance();
                                Some(self.parse_expression()?)
                            } else {
                                None
                            };
                            self.expect(Token::Semicolon)?;
                            return Ok(Statement::Let {
                                name,
                                ty: None,
                                expr,
                                address: None,
                                address_expr: None,
                                bit_range: None,
                                is_override: false,
                                modifiers: Vec::new(),
                                constraint,
                            });
                        } else {
                            // Subtype projection: let name <: SOURCE . OPS
                            let source = self.parse_projection_source()?;
                            let ops = self.parse_subtype_ops()?;
                            self.expect(Token::Semicolon)?;
                            return Ok(Statement::Let {
                                name,
                                ty: None,
                                expr: Some(Expr::SubtypeProjection {
                                    source: Box::new(source),
                                    ops,
                                }),
                                address: None,
                                address_expr: None,
                                bit_range: None,
                                is_override: false,
                                modifiers: Vec::new(),
                                constraint: None,
                            });
                        }
                    }

                let mut modifiers = self.parse_hashtag_modifiers()?;
                let mut address: Option<u64> = None;
                let mut address_expr: Option<Box<Expr>> = None;
                let mut bit_range: Option<BitRange> = None;
                let mut is_override = false;

                // Optional mapping before colon
                loop {
                    if let Some(Ok(Token::At)) = self.current_token() {
                        self.advance();
                        let addr_expr = self.parse_expression()?;
                        match addr_expr.as_integer() {
                            Some(n) => { address = Some(n as u64); }
                            None => { address_expr = Some(Box::new(addr_expr)); }
                        }
                    } else if let Some(Ok(Token::LBracket)) = self.current_token() {
                        self.advance();
                        bit_range = Some(self.parse_bit_range()?);
                        self.expect(Token::RBracket)?;
                    } else {
                        break;
                    }
                }

                let ty = if let Some(Ok(Token::Colon)) = self.current_token() {
                    self.advance();
                    let t = self.parse_type()?;

                    // Hardware mapping after type
                    loop {
                        if let Some(Ok(Token::At)) = self.current_token() {
                            self.advance();
                            let addr_expr = self.parse_expression()?;
                            match addr_expr.as_integer() {
                                Some(n) => { address = Some(n as u64); }
                                None => { address_expr = Some(Box::new(addr_expr)); }
                            }
                            if let Some(Ok(Token::Slash)) = self.current_token() {
                                self.advance();
                                bit_range = Some(self.parse_bit_range()?);
                            }
                        } else if let Some(Ok(Token::LBracket)) = self.current_token() {
                            self.advance();
                            bit_range = Some(self.parse_bit_range()?);
                            self.expect(Token::RBracket)?;
                        } else {
                            break;
                        }
                    }
                    Some(t)
                } else {
                    None
                };

                // `<: [lo..hi]` or `<: [expr]` constraint after type
                let mut constraint: Option<Box<Expr>> = None;
                if self.check_lt_colon() {
                    self.advance();
                    if let Some(Ok(Token::LBracket)) = self.current_token() {
                        self.advance();
                        constraint = Some(self.parse_constraint_expr()?);
                        self.expect(Token::RBracket)?;
                    }
                }

                // Modifiers after type before =
                let mods_after = self.parse_hashtag_modifiers()?;
                modifiers.extend(mods_after);

                let expr = if let Some(Ok(Token::Eq)) = self.current_token() {
                    self.advance();
                    Some(self.parse_expression()?)
                } else {
                    None
                };
                self.expect(Token::Semicolon)?;
                Ok(Statement::Let {
                    name,
                    ty,
                    expr,
                    address,
                    address_expr,
                    bit_range,
                    constraint,
                    is_override,
                    modifiers,
                })
                }
            }
            Some(Ok(Token::Sync)) => {
                self.advance();
                self.expect(Token::LBrace)?;
                let body = self.parse_body()?;
                self.expect(Token::RBrace)?;
                self.expect(Token::Semicolon)?;
                Ok(Statement::SyncBlock { body })
            }
            Some(Ok(Token::Term)) => {
                self.advance();
                let outputs = self.parse_term_outputs()?;
                let mut swan_song = None;
                if let Some(Ok(Token::Arrow)) = self.current_token() {
                    self.advance();
                    let swan_expr = self.parse_expression()?;
                    swan_song = Some(Box::new(Statement::Expression(swan_expr)));
                }
                let modifiers = self.parse_hashtag_modifiers()?;
                self.expect(Token::Semicolon)?;
                Ok(Statement::Term { values: outputs, swan_song, modifiers })
            }
            Some(Ok(Token::TermBang)) => {
                self.advance();
                let outputs = self.parse_term_outputs()?;
                let mut swan_song = None;
                if let Some(Ok(Token::Arrow)) = self.current_token() {
                    self.advance();
                    let swan_expr = self.parse_expression()?;
                    swan_song = Some(Box::new(Statement::Expression(swan_expr)));
                }
                let modifiers = self.parse_hashtag_modifiers()?;
                self.expect(Token::Semicolon)?;
                Ok(Statement::TermBang { values: outputs, swan_song, modifiers })
            }
            Some(Ok(Token::Escape)) => {
                self.advance();
                let expr = if let Some(Ok(Token::Semicolon)) = self.current_token() {
                    None
                } else {
                    Some(self.parse_expression()?)
                };
                self.expect(Token::Semicolon)?;
                Ok(Statement::Escape(expr))
            }
            Some(Ok(Token::Foreach)) => {
                self.advance();
                self.expect(Token::LParen)?;
                let item = self.expect_identifier()?;
                // expect the `in` keyword
                if !matches!(self.current_token(), Some(Ok(Token::Identifier(s))) if s == "in") {
                    return Err(SyntaxError::UnexpectedToken {
                        expected: "'in'".to_string(),
                        found: self.fmt_current_token(),
                        span: self.current_span().unwrap_or_else(Span::dummy),
                    });
                }
                self.advance();
                let list = self.parse_expression()?;
                self.expect(Token::RParen)?;
                self.expect(Token::LBrace)?;
                let body = self.parse_body()?;
                self.expect(Token::RBrace)?;
                let modifiers = self.parse_hashtag_modifiers()?;
                self.expect(Token::Semicolon)?;
                Ok(Statement::Foreach {
                    item,
                    list: Box::new(list),
                    body,
                    modifiers,
                })
            }
            Some(Ok(Token::Question)) => {
                // ?#[handler] { body } — proof oracle
                self.advance();
                if !matches!(self.current_token(), Some(Ok(Token::HashBracket))) {
                    return Err(SyntaxError::UnexpectedToken {
                        expected: "'#['".to_string(),
                        found: self.fmt_current_token(),
                        span: self.current_span().unwrap_or_else(Span::dummy),
                    });
                }
                self.advance();
                let handler = self.parse_body()?;
                self.expect(Token::RBracket)?;
                self.expect(Token::LBrace)?;
                let body = self.parse_body()?;
                self.expect(Token::RBrace)?;
                self.expect(Token::Semicolon)?;
                Ok(Statement::Oracle {
                    handler,
                    body,
                    span: None,
                })
            }
            Some(Ok(Token::Uni)) => {
                // Three syntaxes supported:
                // 1. uni variable(Pattern) = result; (library pattern match)
                // 2. uni expr[Index](Pattern) = result; (indexed pattern match)
                // 3. uni pattern = expr; (simple pattern)
                self.advance();
                
                // Read target: an identifier, possibly with [index] access, but NOT with
                // postfix (Pattern) — those belong to the unification pattern, not a call.
                let target = self.parse_uni_target()?;
                
                // If followed by (, this is library-style pattern matching
                if let Some(Ok(Token::LParen)) = self.current_token() {
                    // Extract the base variable name from nested access expressions
                    let var_name = self.get_base_identifier(&target);
                    self.advance(); // consume (
                    
                    // Parse pattern - could be Variant or Variant(data) or just _
                    let pattern_name = match self.current_token() {
                        Some(Ok(Token::Underscore)) => {
                            self.advance();
                            self.expect(Token::RParen)?;
                            self.expect(Token::Arrow)?;
                            let expr = self.parse_unification_rhs()?;
                            self.expect(Token::Semicolon)?;
                            return Ok(Statement::Unification {
                                name: var_name,
                                variant: "_".to_string(),
                                fields: vec![],
                                expr,
                            });
                        }
                        Some(Ok(Token::Identifier(name))) => name.clone(),
                        Some(Ok(Token::TypeData)) => "Data".to_string(),
                        Some(Ok(Token::Ok)) => "Ok".to_string(),
                        Some(Ok(Token::Err)) => "Err".to_string(),
                        Some(Ok(Token::Some)) => "Some".to_string(),
                        Some(Ok(Token::None)) => "None".to_string(),
                        Some(Ok(Token::TypeInt)) => "Int".to_string(),
                        Some(Ok(Token::TypeString)) => "String".to_string(),
                        Some(Ok(Token::TypeBool)) => "Bool".to_string(),
                        Some(Ok(Token::TypeChar)) => "Char".to_string(),
                        Some(Ok(Token::TypeFloat)) => "Float".to_string(),
                        Some(Ok(Token::TypeVoid)) => "Void".to_string(),
                        Some(Ok(Token::TypeUInt)) => "UInt".to_string(),
                        Some(Ok(Token::BoolTrue)) => "KeywordTrue".to_string(),
                        Some(Ok(Token::BoolFalse)) => "KeywordFalse".to_string(),
                        Some(Ok(Token::Let)) => "KeywordLet".to_string(),
                        Some(Ok(Token::Txn)) => "KeywordTxn".to_string(),
                        Some(Ok(Token::Defn)) => "KeywordDefn".to_string(),
                        Some(Ok(Token::Sig)) => "KeywordSig".to_string(),
                        Some(Ok(Token::Enum)) => "KeywordEnum".to_string(),
                        Some(Ok(Token::Struct)) => "KeywordStruct".to_string(),
                        Some(Ok(Token::Frgn)) => "KeywordFrgn".to_string(),
                        Some(Ok(Token::Import)) => "KeywordImport".to_string(),
                        Some(Ok(Token::Term)) => "KeywordTerm".to_string(),
                        Some(Ok(Token::Rct)) => "KeywordRct".to_string(),
                        Some(Ok(Token::Async)) => "KeywordAsync".to_string(),
                        Some(Ok(Token::Escape)) => "KeywordEscape".to_string(),
                        Some(Ok(Token::Uni)) => "KeywordUni".to_string(),
                        Some(Ok(Token::Render)) => "KeywordRender".to_string(),
                        Some(Ok(Token::Rstruct)) => "KeywordRstruct".to_string(),
                        Some(Ok(Token::Reg)) => "KeywordReg".to_string(),
                        Some(Ok(Token::Trg)) => "KeywordTrg".to_string(),
                        Some(Ok(Token::Link)) => "KeywordLink".to_string(),
                        Some(Ok(Token::Asm)) => "KeywordAsm".to_string(),
                        Some(Ok(Token::Bank)) => "KeywordBank".to_string(),
                        Some(Ok(Token::Match)) => "KeywordMatch".to_string(),
                        _ => return self.spanned_err(format!("Expected pattern variant, found {}", self.fmt_current_token()).to_string()),
                    };
                    self.advance();
                    
                    // Check for pattern data: Variant(field1, ...) or just Variant
                    let fields = if let Some(Ok(Token::LParen)) = self.current_token() {
                        self.advance();
                        let f = self.parse_pattern_fields()?;
                        self.expect(Token::RParen)?;
                        f
                    } else {
                        vec![]
                    };

                    self.expect(Token::RParen)?;
                    self.expect(Token::Arrow)?;
                    let expr = self.parse_unification_rhs()?;
                    self.expect(Token::Semicolon)?;
                    Ok(Statement::Unification {
                        name: var_name,
                        variant: pattern_name,
                        fields,
                        expr,
                    })
                } else {
                    // Simple pattern: uni pattern = expr;
                    let pattern_name = match &target {
                        Expr::Identifier(n) => n.clone(),
                        _ => return self.spanned_err("Expected pattern name after uni".to_string()),
                    };
                    self.expect(Token::Arrow)?;
                    let expr = self.parse_expression()?;
                    self.expect(Token::Semicolon)?;
                    Ok(Statement::Unification {
                        name: "uni".to_string(),
                        variant: pattern_name,
                        fields: vec![],
                        expr,
                    })
                }
            }
            Some(Ok(Token::Asm)) => {
                self.advance();
                // Parse: asm "instruction" { "clobber1", "clobber2" };
                let asm_string = match self.current_token() {
                    Some(Ok(Token::String(s))) => {
                        let s = s.clone();
                        self.advance();
                        s
                    }
                    _ => return self.spanned_err("Expected string literal after asm".to_string()),
                };

                let clobbers = if let Some(Ok(Token::LBrace)) = self.current_token() {
                    self.advance();
                    let mut clobbers = Vec::new();
                    loop {
                        match self.current_token() {
                            Some(Ok(Token::String(s))) => {
                                clobbers.push(s.clone());
                                self.advance();
                            }
                            Some(Ok(Token::Comma)) => {
                                self.advance();
                            }
                            Some(Ok(Token::RBrace)) => {
                                self.advance();
                                break;
                            }
                            _ => return self.spanned_err("Expected clobber list".to_string()),
                        }
                    }
                    clobbers
                } else {
                    Vec::new()
                };

                self.expect(Token::Semicolon)?;
                let span = self.current_span();
                Ok(Statement::InlineAsm { asm_string, clobbers, span })
            }
            Some(Ok(Token::LBracket)) => {
                // Guarded statement: [condition] statement or [condition] { statements }
                // Also supports pattern matching: [value Pattern(field)] { statements };
                self.advance(); // consume [

                // Check for pattern match structure before consuming tokens:
                // Pattern: identifier Variant(fields) where Variant starts with uppercase
                let is_pattern = matches!(self.current_token(), Some(Ok(Token::Identifier(_))))
                    && matches!(self.peek(), Some(Ok(Token::Identifier(v))) if v.chars().next().map_or(false, |c| c.is_uppercase()))
                    || matches!(self.peek(), Some(Ok(Token::Ok | Token::Err)));

                let condition = if is_pattern {
                    // Parse as pattern match: variable Variant(field1, field2)
                    if let Some(Ok(Token::Identifier(var_name))) = self.current_token() {
                        let var_name_clone = var_name.clone();
                        self.advance(); // consume variable name

                        let variant_name = match self.current_token() {
                            Some(Ok(Token::Identifier(v))) => {
                                let name = v.clone();
                                self.advance();
                                name
                            }
                            Some(Ok(Token::Ok)) => {
                                self.advance();
                                "Ok".to_string()
                            }
                            Some(Ok(Token::Err)) => {
                                self.advance();
                                "Err".to_string()
                            }
                            _ => {
                                match self.expect_identifier() {
                                    Ok(n) => n,
                                    Err(e) => return Err(e),
                                }
                            }
                        };

                        // Expect ( for pattern fields
                        if matches!(self.current_token(), Some(Ok(Token::LParen))) {
                            self.advance(); // consume (
                            let fields = self.parse_pattern_fields()?;
                            self.expect(Token::RParen)?;
                            Expr::PatternMatch {
                                value: Box::new(Expr::Identifier(var_name_clone)),
                                variant: variant_name,
                                fields,
                            }
                        } else {
                            // Variant without parens - still a pattern match
                            Expr::PatternMatch {
                                value: Box::new(Expr::Identifier(var_name_clone)),
                                variant: variant_name,
                                fields: vec![],
                            }
                        }
                    } else {
                        self.parse_expression()?
                    }
                } else {
                    // Not a pattern - parse as regular expression
                    self.parse_expression()?
                };

                self.expect(Token::RBracket)?;

                // Check for block syntax
                if let Some(Ok(Token::LBrace)) = self.current_token() {
                    // Block guard: [condition] { statements };
                    self.advance(); // consume {
                    let mut statements = Vec::new();

                    // Parse statements until we hit }
                    while !matches!(self.current_token(), Some(Ok(Token::RBrace))) {
                        statements.push(self.parse_statement()?);
                    }

                    if statements.is_empty() {
                        return self.spanned_err("Empty guarded block".to_string());
                    }

                    self.expect(Token::RBrace)?;
                    self.expect(Token::Semicolon)?; // Block must be terminated with ;

                    Ok(Statement::Guarded {
                        condition,
                        statements,
                    })
                } else {
                    // Flat guard: [condition] statement
                    let statement = self.parse_statement()?;
                    Ok(Statement::Guarded {
                        condition,
                        statements: vec![statement],
                    })
                }
            }
            Some(Ok(Token::Trg)) => {
                // Phase 3: trg name: Type @ cell!(args).port  — trigger binding
                // Phase 3: trg name: Type @ cell! .port      — shorthand (single output port)
                self.advance();
                let name = self.expect_identifier()?;
                let ty = if let Some(Ok(Token::Colon)) = self.current_token() {
                    self.advance();
                    Some(self.parse_type()?)
                } else {
                    None
                };
                if let Some(Ok(Token::At)) = self.current_token() {
                    self.advance();
                    // Parse instance expression — cell_name(args).port or cell_name(args)
                    // The expression parser will eagerly consume .port as FieldAccess, so
                    // parse the full expression then extract port from FieldAccess if present.
                    let instance = self.parse_expression()?;
                    let (instance, port) = if let Expr::FieldAccess(obj, name) = &instance {
                        (obj.as_ref().clone(), name.clone())
                    } else {
                        (instance, String::new())
                    };
                    // Parse optional @Hz suffix (e.g. @1kHz, @10MHz) for tick rate
                    let mut modifiers: Vec<Annotation> = vec![];
                    if let Some(Ok(Token::At)) = self.current_token() {
                        self.advance();
                        let hz_raw = self.expect_integer()?;
                        let unit = if let Some(Ok(Token::Identifier(u))) = self.current_token() {
                            let u = u.clone();
                            self.advance();
                            u
                        } else { "Hz".to_string() };
                        let multiplier: u64 = match unit.as_str() {
                            "Hz" | "hz" => 1,
                            "kHz" | "khz" | "KHz" => 1000,
                            "MHz" | "mhz" | "MHz" => 1_000_000,
                            _ => 1,
                        };
                        let hz_val = (hz_raw as u64) * multiplier;
                        modifiers.push(Annotation {
                            name: "hz".to_string(),
                            value: Expr::Bool(true),
                            mode: AnnotationMode::Advisory,
                        });
                    }
                    self.expect(Token::Semicolon)?;
                    Ok(Statement::TrgBinding { name, ty, instance, port, modifiers })
                } else {
                    // No @ — fall through to the old error (deprecated local trigger)
                    self.spanned_err(
                        "Local triggers introduce asynchronous rollback risks. \
                         You must use 'trg!' or 'trigger!' to explicitly acknowledge this boundary. \
                         (Trigger bindings use 'trg name: Type @ expr.port')".to_string(),
                    )
                }
            }
            // DISABLED: alka/on_exit — not ready for use; keep parser disabled until revisited.
            // Some(Ok(Token::Hash)) => {
            //     // Block pragma: #on_exit { ... };
            //     return self.parse_block_pragma();
            // }
            _ => {
                // Check for discard form: <- &list or <- &list[i]
                if let Some(Ok(Token::ArrowLeft)) = self.current_token() {
                    self.advance();
                    let target_expr = self.parse_expression()?;
                    let (target, index) = match self.extract_arrow_target(target_expr) {
                        Some((t, i)) => (t, i),
                        None => return self.spanned_err::<Statement>(
                            "Expected a list expression after '<-'".to_string()
                        ),
                    };
                    self.expect(Token::Semicolon)?;
                    return Ok(Statement::Expression(Expr::ArrowDiscard {
                        target: Box::new(target),
                        index: Box::new(index),
                    }));
                }
                // DISABLED: alka/on_exit — not ready for use; keep parser disabled until revisited.
                // if let Some(Ok(Token::Identifier(name))) = self.current_token() {
                //     if name == "alka" || name == "ALKA" {
                //         return self.parse_alka_block();
                //     }
                // }
                // Expression statement or Assignment/Unification/Arrow
                // 2026-07-11: Phase 1A.0 — prefix annotation handler removed.
                let expr = self.parse_expression()?;

                if let Some(Ok(Token::Eq)) = self.current_token() {
                    self.advance();
                    let right = self.parse_expression()?;

                    let mut timeout: Option<(Expr, TimeUnit)> = None;
                    if let Some(Ok(Token::Within)) = self.current_token() {
                        self.advance();
                        let expr = self.parse_expression()?;
                        let unit = self.parse_time_unit_or_default()?;
                        timeout = Some((expr, unit));
                    }

                    match expr {
                        Expr::Call(name, args) => {
                            if args.len() == 1 {
                                if let Expr::Identifier(pattern) = &args[0] {
                                    self.expect(Token::Semicolon)?;
                                    Ok(Statement::Unification {
                                        name,
                                        variant: pattern.clone(),
                                        fields: vec![],
                                        expr: right,
                                    })
                                } else {
                                    self.spanned_err(
                                        "Unification pattern must be an identifier".to_string(),
                                    )
                                }
                            } else {
                                self.spanned_err(
                                    "Unification expects one pattern argument".to_string(),
                                )
                            }
                        }
                        _ => {
                            let modifiers = self.parse_hashtag_modifiers()?;
                            self.expect(Token::Semicolon)?;
                            Ok(Statement::Assignment {
                                lhs: expr,
                                expr: right,
                                timeout,
                                modifiers,
                            })
                        }
                    }
                } else if let Some(Ok(Token::ArrowLeft)) = self.current_token() {
                    self.advance();
                    let right = self.parse_expression()?;

                    // Check arrow targets: has_& indicates consumption.
                    // 2026-07-10: Both bare identifiers are accepted as targets.
                    let left_has_amp = matches!(&expr, Expr::AddrOf(_));
                    let right_has_amp = matches!(&right, Expr::AddrOf(_));
                    let left_target = self.extract_arrow_target(expr.clone());
                    let right_target = self.extract_arrow_target(right.clone());

                    if left_has_amp && right_has_amp {
                        // &dest <- &source (both have &) → transfer, consume: true
                        let (dest, _) = left_target.unwrap();
                        let (source, _) = right_target.unwrap();
                        let filter = self.extract_arrow_filter(&right);
                        let modifiers = self.parse_hashtag_modifiers()?;
                        self.expect(Token::Semicolon)?;
                        Ok(Statement::Expression(Expr::ArrowTransfer { consume: true,
                            dest: Box::new(dest),
                            source: Box::new(source),
                            filter,
                        }))
                    } else if left_has_amp {
                        // &list <- x or &list[i] <- x → Push
                        let (target, index) = left_target.unwrap();
                        let modifiers = self.parse_hashtag_modifiers()?;
                        self.expect(Token::Semicolon)?;
                        Ok(Statement::Expression(Expr::ArrowMut {
                            dir: ArrowDir::Push,
                            consume: false, target: Box::new(target),
                            index: Box::new(index),
                            value: Some(Box::new(right)),
                        }))
                    } else if right_has_amp {
                        // dest <- &source → transfer, OR value <- &list → pop
                        // Both have & on the RHS (consumption). Codegen decides
                        // transfer vs pop based on types.
                        let (source, index) = right_target.unwrap();
                        if left_target.is_some() {
                            // dest <- &source → transfer (both sides are collections)
                            let (dest, _) = left_target.unwrap();
                            let filter = self.extract_arrow_filter(&right);
                            let modifiers = self.parse_hashtag_modifiers()?;
                            self.expect(Token::Semicolon)?;
                            Ok(Statement::Expression(Expr::ArrowTransfer { consume: true,
                                dest: Box::new(dest),
                                source: Box::new(source),
                                filter,
                            }))
                        } else {
                            // value <- &list → pop, bind to expr
                            let modifiers = self.parse_hashtag_modifiers()?;
                            self.expect(Token::Semicolon)?;
                            Ok(Statement::Assignment {
                                lhs: expr,
                                expr: Expr::ArrowMut {
                                    dir: ArrowDir::Pop,
                                    consume: true, target: Box::new(source),
                                    index: Box::new(index),
                                    value: None,
                                },
                                timeout: None,
                                modifiers,
                            })
                        }
                    } else if let Some((target, index)) = left_target {
                        // list <- x or list[i] <- x → Push (no & on either side)
                        // The LHS looks like a collection → push
                        let modifiers = self.parse_hashtag_modifiers()?;
                        self.expect(Token::Semicolon)?;
                        Ok(Statement::Expression(Expr::ArrowMut {
                            dir: ArrowDir::Push,
                            consume: false, target: Box::new(target),
                            index: Box::new(index),
                            value: Some(Box::new(right)),
                        }))
                    } else if let Some((target, index)) = right_target {
                        // value <- list → Peek/Pop without consumption, bind to expr
                        let modifiers = self.parse_hashtag_modifiers()?;
                        self.expect(Token::Semicolon)?;
                        Ok(Statement::Assignment {
                            lhs: expr,
                            expr: Expr::ArrowMut {
                                dir: ArrowDir::Pop,
                                consume: false, target: Box::new(target),
                                index: Box::new(index),
                                value: None,
                            },
                            timeout: None,
                            modifiers,
                        })
                    } else {
                        self.spanned_err(
                            "Either side of '<-' must be a list or &list".to_string()
                        )
                    }
                } else {
                    self.expect(Token::Semicolon)?;
                    Ok(Statement::Expression(expr))
                }
            }
            Some(Ok(Token::Await)) => {
                self.advance();
                let expr = self.parse_expression()?;
                self.expect(Token::Semicolon)?;
                let modifiers = self.parse_hashtag_modifiers()?;
                Ok(Statement::Await { expr, modifiers })
            }
            Some(Ok(Token::Async)) => {
                self.advance();
                // Check if followed by await -> async await
                if let Some(Ok(Token::Await)) = self.current_token() {
                    return self.parse_async_await();
                }
                // Check if followed by rct/txn -> error (top-level only)
                if let Some(Ok(Token::Rct)) | Some(Ok(Token::Txn)) = self.current_token() {
                    return Err(SyntaxError::UnexpectedToken {
                        expected: "statement or block".to_string(),
                        found: "'rct' or 'txn'".to_string(),
                        span: self.current_span().unwrap_or_else(Span::dummy),
                    });
                }
                let body = Box::new(self.parse_statement()?);
                let modifiers = self.parse_hashtag_modifiers()?;
                Ok(Statement::Async { body, modifiers })
            }
        }
    }

    fn parse_async_await(&mut self) -> Result<Statement, SyntaxError> {
        // Already consumed Token::Async, now on Token::Await
        self.advance(); // consume await

        // Optional: "let x = "
        let lhs = if let Some(Ok(Token::Let)) = self.current_token() {
            self.advance();
            let name = self.expect_identifier()?;
            self.expect(Token::Eq)?;
            Some(name)
        } else {
            None
        };

        let body = Box::new(self.parse_statement()?);
        let modifiers = self.parse_hashtag_modifiers()?;
        Ok(Statement::AsyncAwait { body, lhs, modifiers })
    }

    /// Extract (target, index) from an arrow mutation expression.
    /// Returns None if the expression is not a collection reference.
    /// - `&list` → (OwnedRef("list"), Term)
    /// - `&list[5]` → (OwnedRef("list"), Integer(5))
    /// Check if an expression is a valid inner target for arrow mutation.
    /// Accepts `&name`, `name`, `&name.field`, `name.field`, and deeper field chains.
    fn is_valid_arrow_inner(&self, expr: &Expr) -> bool {
        match expr {
            // 2026-07-10: Accept bare Identifier as a collection target.
            // Previously only AddrOf was accepted (the & marker was required).
            Expr::AddrOf(_) | Expr::Identifier(_) => true,
            Expr::FieldAccess(target, _) => self.is_valid_arrow_inner(target),
            _ => false,
        }
    }

    fn extract_arrow_target(&self, expr: Expr) -> Option<(Expr, Expr)> {
        match expr {
            // 2026-07-10: Accept bare Identifier as a collection target.
            Expr::AddrOf(_) | Expr::Identifier(_) => Some((expr, Expr::Term)),
            Expr::ListIndex(target, index) => {
                if self.is_valid_arrow_inner(&*target) {
                    Some((*target, *index))
                } else {
                    None
                }
            }
            Expr::FieldAccess(target, field) => {
                // Only accept if the inner target is an OwnedRef
                if self.is_valid_arrow_inner(&*target) {
                    Some((Expr::FieldAccess(target, field), Expr::Term))
                } else {
                    None
                }
            }
            Expr::MultiSlice { value, ops, .. } => {
                // Accept `&list[; cond]` and `&list[0; cond]` as arrow targets.
                // Coordinates beyond the first are invalid for arrow targets.
                if !self.is_valid_arrow_inner(&*value) {
                    return None;
                }
                // Find the first Coord op to extract the index (if any)
                let coord_idx = ops.iter().position(|op| matches!(op, crate::ast::BracketOp::Coord(_)));
                match coord_idx {
                    Some(i) => {
                        if let crate::ast::BracketOp::Coord(crate::ast::SliceCoordinate::Index(idx)) = &ops[i] {
                            Some((*value, *idx.clone()))
                        } else {
                            // Range/named coordinates not valid for arrow targets
                            None
                        }
                    }
                    None => Some((*value, Expr::Term)),
                }
            }
            _ => None,
        }
    }

    /// Extract the optional filter from a `MultiSlice` expression.
    /// Returns `Some(filter)` for `&list[; cond]`, `None` otherwise.
    fn extract_arrow_filter(&self, expr: &Expr) -> Option<Box<Expr>> {
        match expr {
            Expr::MultiSlice { ops, .. } => {
                ops.iter().find_map(|op| {
                    if let crate::ast::BracketOp::Mask(m) = op {
                        Some(m.clone())
                    } else {
                        None
                    }
                })
            }
            _ => None,
        }
    }

    fn parse_type(&mut self) -> Result<Type, SyntaxError> {
        self.parse_type_inner(true)
    }

    fn parse_type_inner(&mut self, allow_contract_bound: bool) -> Result<Type, SyntaxError> {
        let mut ty = match self.current_token() {
            Some(Ok(Token::Identifier(name))) => {
                let name = name.clone();
                self.advance();
                // Create as Custom - type checker will resolve to Sig if needed
                Type::Custom(name)
            }
            Some(Ok(Token::TypeData)) => {
                self.advance();
                Type::Custom("Data".to_string())
            }
            Some(Ok(Token::TypeInt)) => {
                self.advance();
                Type::Custom("Int".to_string())
            }
            Some(Ok(Token::TypeUInt))
            | Some(Ok(Token::TypeUnsigned))
            | Some(Ok(Token::TypeUSgn)) => {
                self.advance();
                Type::Custom("UInt".to_string())
            }
            Some(Ok(Token::TypeSigned)) | Some(Ok(Token::TypeSgn)) => {
                self.advance();
                Type::Custom("Int".to_string())
            }
            Some(Ok(Token::TypeFloat)) => {
                self.advance();
                Type::Custom("Float".to_string())
            }
            Some(Ok(Token::TypeString)) => {
                self.advance();
                Type::Custom("String".to_string())
            }
            Some(Ok(Token::TypeBool)) => {
                self.advance();
                Type::Custom("Bool".to_string())
            }
            Some(Ok(Token::TypeChar)) => {
                self.advance();
                Type::Custom("Char".to_string())
            }
            // Shorthand sized integer types (syntactic sugar for Int/UInt @/xN)
            Some(Ok(Token::TypeU8)) => {
                self.advance();
                Type::Custom("UInt8".to_string())
            }
            Some(Ok(Token::TypeI8)) => {
                self.advance();
                Type::Custom("Int8".to_string())
            }
            Some(Ok(Token::TypeU16)) => {
                self.advance();
                Type::Custom("UInt16".to_string())
            }
            Some(Ok(Token::TypeI16)) => {
                self.advance();
                Type::Custom("Int16".to_string())
            }
            Some(Ok(Token::TypeU32)) => {
                self.advance();
                Type::Custom("UInt32".to_string())
            }
            Some(Ok(Token::TypeI32)) => {
                self.advance();
                Type::Custom("Int32".to_string())
            }
            Some(Ok(Token::TypeU64)) => {
                self.advance();
                Type::Custom("UInt".to_string())
            }
            Some(Ok(Token::TypeI64)) => {
                self.advance();
                Type::Custom("Int".to_string())
            }
            Some(Ok(Token::TypeInt8)) | Some(Ok(Token::TypeI8)) => {
                self.advance();
                Type::Custom("Int8".to_string())
            }
            Some(Ok(Token::TypeUInt8)) | Some(Ok(Token::TypeU8)) => {
                self.advance();
                Type::Custom("UInt8".to_string())
            }
            Some(Ok(Token::TypeInt16)) | Some(Ok(Token::TypeI16)) => {
                self.advance();
                Type::Custom("Int16".to_string())
            }
            Some(Ok(Token::TypeUInt16)) | Some(Ok(Token::TypeU16)) => {
                self.advance();
                Type::Custom("UInt16".to_string())
            }
            Some(Ok(Token::TypeInt32)) | Some(Ok(Token::TypeI32)) => {
                self.advance();
                Type::Custom("Int32".to_string())
            }
            Some(Ok(Token::TypeUInt32)) | Some(Ok(Token::TypeU32)) => {
                self.advance();
                Type::Custom("UInt32".to_string())
            }
            Some(Ok(Token::TypeInt64)) | Some(Ok(Token::TypeI64)) => {
                self.advance();
                Type::Custom("Int".to_string())
            }
            Some(Ok(Token::TypeUInt64)) | Some(Ok(Token::TypeU64)) => {
                self.advance();
                Type::Custom("UInt".to_string())
            }
            Some(Ok(Token::TypeFloat32)) | Some(Ok(Token::TypeF32)) => {
                self.advance();
                Type::Custom("Float".to_string())
            }
            Some(Ok(Token::TypeFloat64)) | Some(Ok(Token::TypeF64)) | Some(Ok(Token::TypeDouble)) => {
                self.advance();
                Type::Custom("Float64".to_string())
            }
            // Note: HashMap, HashSet, StringBuilder, Stack, Queue are parsed as
            // regular identifiers (Custom/Applied types) defined in stdlib.
            // No special AST variants - keeps the language philosophically pure.
            Some(Ok(Token::TypeVoid)) => {
                self.advance();
                Type::Void
            }
            Some(Ok(Token::LParen)) => {
                self.advance();
                // Check if it's a tuple type or empty () or function type () -> T
                if let Some(Ok(Token::RParen)) = self.current_token() {
                    self.advance();
                    // Check for function type: () -> T
                    if let Some(Ok(Token::Arrow)) = self.current_token() {
                        self.advance();
                        let return_type = self.parse_type()?;
                        Type::Applied("Fn".to_string(), vec![Type::Tuple(vec![]), return_type])
                    } else {
                        Type::Void
                    }
                } else {
                    let mut tuple_types = Vec::new();
                    tuple_types.push(self.parse_type()?);
                    while let Some(Ok(Token::Comma)) = self.current_token() {
                        self.advance();
                        if let Some(Ok(Token::RParen)) = self.current_token() {
                            break;
                        }
                        tuple_types.push(self.parse_type()?);
                    }
                    self.expect(Token::RParen)?;
                    // Check for function type: (A, B) -> R
                    if let Some(Ok(Token::Arrow)) = self.current_token() {
                        self.advance();
                        let return_type = self.parse_type()?;
                        Type::Applied("Fn".to_string(), vec![Type::Tuple(tuple_types), return_type])
                    } else if tuple_types.len() == 1 {
                        tuple_types.remove(0)
                    } else {
                        Type::Tuple(tuple_types)
                    }
                }
            }
            Some(Ok(tok)) => return self.spanned_err(format!("Expected type, found {}", Self::token_display(tok))),
            Some(Err(_)) => return self.spanned_err("Lexer error".to_string()),
            None => return self.spanned_err("Expected type, found EOF".to_string()),
        };

        // Check for bit-width decorator: Type@/N or Type@/0..7 or Type@/xN
        // Only consume @ if followed by / (bit-range constraint)
        // If @ is followed by something else (address), leave it for the caller
        if let Some(Ok(Token::At)) = self.current_token() {
            if let Some(Ok(Token::Slash)) = self.peek() {
                self.advance(); // consume @
                self.advance(); // consume /
                let br = self.parse_bit_range()?;
                // Wrap the type in a Constrained type with the bit range
                ty = Type::Constrained(Box::new(ty), br);
            }
            // If @ is not followed by /, don't consume it - let the caller handle it
        }

        // Type[expr] bracket syntax: generic application like `Option[Int]`.
        // The old `Type[expr]` contract-bound syntax is removed — use `<: [expr]` instead.
        if allow_contract_bound {
            if let Some(Ok(Token::LBracket)) = self.current_token() {
                self.advance();
                let inner = self.parse_expression()?;
                self.expect(Token::RBracket)?;
                let arg_type = match inner {
                    Expr::Identifier(name) => match name.as_str() {
                        "Int" => Type::Custom("Int".to_string()),
                        "Float" => Type::Custom("Float".to_string()),
                        "Bool" => Type::Custom("Bool".to_string()),
                        "String" => Type::Custom("String".to_string()),
                        "Char" => Type::Custom("Char".to_string()),
                        "Void" => Type::Void,
                        _ => Type::Custom(name),
                    },
                    // 2026-07-08: Phase 2b — use Width(n) instead of Custom("Literal(n)")
                    Expr::Integer(n) => Type::Width(n as u64),
                    Expr::Literal(lit) => match lit.as_ref() {
                        crate::features::literal::LiteralExpr::Integer(n) => {
                            Type::Width(*n as u64)
                        }
                        _ => return self.spanned_err(
                            "Invalid generic type argument. Use a type name (e.g. `Option[Int]`) or integer (e.g. `Byte[4096]`). \
                             For constraints use `<: [expr]` syntax instead (e.g. `let x: Int <: [product > 0]`)."
                                .to_string(),
                        ),
                    },
                    _ => return self.spanned_err(
                        "Invalid generic type argument. Use a type name (e.g. `Option[Int]`) or integer (e.g. `Byte[4096]`). \
                         For constraints use `<: [expr]` syntax instead (e.g. `let x: Int <: [product > 0]`)."
                            .to_string(),
                    ),
                };
                // 2026-07-08: Phase 2b — use type_to_base_name for keyword types too
                let base_name = match Self::type_to_base_name(&ty) {
                    Some(n) => n,
                    None => return self.spanned_err("Generic type must have a base name".to_string()),
                };
                // 2026-07-08: Phase 2b — try Bits resolution for known keyword + Width
                if let Some(bits) = Self::resolve_bits_type(&base_name, &[arg_type.clone()]) {
                    ty = bits;
                } else {
                    ty = Type::Applied(base_name, vec![arg_type]);
                }
            }
        }

        if let Some(Ok(Token::Lt)) = self.current_token() {
            self.advance();
            
            // Special handling for Vector<T, dim1, dim2, ...> 
            // Parse element type first, then dimensions as integers
            if let Type::Custom(name) = &ty {
                if name == "Vector" {
                    // First: parse the element type
                    let inner = Box::new(self.parse_type()?);
                    
                    // Parse dimensions as integers
                    let mut dimensions = Vec::new();
                    while let Some(Ok(Token::Comma)) = self.current_token() {
                        self.advance(); // consume comma
                        
                        // Try to parse as integer first
                        if let Some(Ok(Token::Integer(n))) = self.current_token() {
                            dimensions.push(crate::ast::Dimension::Anonymous(*n as usize));
                            self.advance();
                        } else {
                            return self.spanned_err("Vector dimension must be an integer".to_string());
                        }
                    }
                    
                    // Expect closing >
                    if let Some(Ok(Token::Gt)) = self.current_token() {
                        self.advance();
                    } else if let Some(Ok(Token::Shr)) = self.current_token() {
                        self.shr_consumed_as_gt = true;
                        self.advance();
                    } else {
                        return self.spanned_err("Expected '>' to close Vector type".to_string());
                    }
                    
                    return Ok(Type::Vector(inner, dimensions));
                }
            }
            
            // 2026-07-08: Phase 2b — use parse_type_arg for integer token → Width(n)
            // Standard generic type parsing
            let mut type_args = Vec::new();
            loop {
                type_args.push(self.parse_type_arg()?);
                // Check if child level consumed Shr as Gt
                if self.shr_consumed_as_gt {
                    // Child consumed >> which serves as our closing > too
                    self.shr_consumed_as_gt = false;
                    let base_name = match Self::type_to_base_name(&ty) {
                        Some(n) => n,
                        None => return self.spanned_err("Generic type must have a base name".to_string()),
                    };
                    // 2026-07-08: Phase 2b — try Bits resolution for known keyword + Width
                    if let Some(bits) = Self::resolve_bits_type(&base_name, &type_args) {
                        return Ok(bits);
                    }
                    ty = Type::Applied(base_name, type_args);
                    return Ok(ty);
                }
                if let Some(Ok(Token::Comma)) = self.current_token() {
                    self.advance();
                } else {
                    break;
                }
            }
            // Handle >> as two > tokens in generic context
            if let Some(Ok(Token::Gt)) = self.current_token() {
                self.advance();
            } else if let Some(Ok(Token::Shr)) = self.current_token() {
                // >> in generic context means two > tokens
                // Mark that we consumed Shr as Gt for parent level
                self.shr_consumed_as_gt = true;
                self.advance();
            } else {
                return self.spanned_err("Expected '>' to close generic type arguments".to_string());
            }
            
            // 2026-07-08: Phase 2b — extract base name from keyword types too
            let base_name = match Self::type_to_base_name(&ty) {
                Some(n) => n,
                None => return self.spanned_err("Generic type must have a base name".to_string()),
            };
            
            // 2026-07-08: Phase 2b — try Bits resolution for known keyword + Width
            if let Some(bits) = Self::resolve_bits_type(&base_name, &type_args) {
                ty = bits;
            // Special handling for Vector<T, dim1, dim2, ...> syntax
            } else if base_name == "Vector" && type_args.len() >= 2 {
                let inner = Box::new(type_args[0].clone());
                let dimensions = match Self::parse_vector_dimensions(&type_args) {
                    Ok(d) => d,
                    Err(e) => return self.spanned_err(e),
                };
                ty = Type::Vector(inner, dimensions);
            } else {
                ty = Type::Applied(base_name, type_args);
            }
        }

        // Check for vector dimension: Type[N] (backward compatible)
        while let Some(Ok(Token::LBracket)) = self.current_token() {
            if !matches!(self.peek(), Some(Ok(Token::Integer(_)))) {
                break;
            }
            self.advance();
            if let Some(Ok(Token::Integer(n))) = self.current_token() {
                let size = *n as usize;
                self.advance();
                self.expect(Token::RBracket)?;
                ty = Type::Vector(Box::new(ty), vec![crate::ast::Dimension::Anonymous(size)]);
            } else {
                return self.spanned_err("Expected vector size".to_string());
            }
        }

        // Check for function type: Type -> Type (e.g., T -> U or (A, B) -> R)
        if let Some(Ok(Token::Arrow)) = self.current_token() {
            self.advance();
            let return_type = self.parse_type()?;
            ty = Type::Applied("Fn".to_string(), vec![ty, return_type]);
        }

        // Check for function type: Type -> Type (e.g., T -> U or (A, B) -> R)
        if let Some(Ok(Token::Arrow)) = self.current_token() {
            self.advance();
            let return_type = self.parse_type()?;
            ty = Type::Applied("Fn".to_string(), vec![ty, return_type]);
        }

        // 2026-07-03: Desugar bare Ptr/PtrN to layout-constrained pointer.
        // This runs AFTER generic arg parsing, so Ptr<Int> stays as Applied("Ptr", ...)
        // while bare Ptr becomes LayoutPtr { bytes: 8 }. The "<:>" and "Ptr!->" variants
        // remain as Custom names (not LayoutPtr) since they have different semantics.
        if let Type::Custom(name) = &ty {
            match name.as_str() {
                "Ptr" => ty = Type::LayoutPtr(LayoutConstraint { bytes: 8, alignment: 8 }),
                "Ptr8" => ty = Type::LayoutPtr(LayoutConstraint { bytes: 1, alignment: 1 }),
                "Ptr16" => ty = Type::LayoutPtr(LayoutConstraint { bytes: 2, alignment: 2 }),
                "Ptr32" => ty = Type::LayoutPtr(LayoutConstraint { bytes: 4, alignment: 4 }),
                "Ptr64" => ty = Type::LayoutPtr(LayoutConstraint { bytes: 8, alignment: 8 }),
                "Ptr128" => ty = Type::LayoutPtr(LayoutConstraint { bytes: 16, alignment: 16 }),
                "Ptr256" => ty = Type::LayoutPtr(LayoutConstraint { bytes: 32, alignment: 32 }),
                _ => {}
            }
        }

        // Check for union: Type | Type
        let mut union_types = Vec::new();
        union_types.push(ty);

        while let Some(Ok(Token::Pipe)) = self.current_token() {
            self.advance();
            let next_ty = self.parse_type()?;
            union_types.push(next_ty);
        }

        if union_types.len() > 1 {
            Ok(Type::Union(union_types))
        } else {
            Ok(union_types.remove(0))
        }
    }

    fn parse_expression(&mut self) -> Result<Expr, SyntaxError> {
        self.parse_pipe_chain()
    }

    /// Parse pipe chain: `initial |> step |> step .|> step ..|> step .N|> step`.
    /// Pipe has the lowest precedence, wrapping `parse_or`.
    /// Supports:
    ///   `|>`    — skip=0 (adjacent)
    ///   `.|>`   — skip=1 (one back)
    ///   `..|>`  — skip=2 (two back)
    ///   `.N|>`  — skip=N (N back, e.g. `.2|>`, `.5|>`)
    fn parse_pipe_chain(&mut self) -> Result<Expr, SyntaxError> {
        let initial = self.parse_or()?;
        let mut steps = Vec::new();

        loop {
            let skip = match self.current_token() {
                Some(Ok(Token::PipeGreater)) => {
                    self.advance();
                    0usize
                }
                Some(Ok(Token::Dot)) => {
                    // .|> (skip=1) or .N|> (skip=N)
                    // Safe to consume speculatively: `.N` at top-level expression
                    // has no other valid meaning (field access `.N` is consumed
                    // by parse_postfix much earlier in the precedence chain).
                    if matches!(self.peek_token(), Some(Ok(Token::PipeGreater))) {
                        self.advance(); // '.'
                        self.advance(); // '|>'
                        1usize
                    } else if let Some(Ok(Token::Integer(n))) = self.peek_token() {
                        if *n > 0 {
                            let n = *n as usize;
                            self.advance(); // '.'
                            self.advance(); // Integer(n)
                            if matches!(self.current_token(), Some(Ok(Token::PipeGreater))) {
                                self.advance(); // '|>'
                                n
                            } else {
                                return Err(SyntaxError::InvalidStatement {
                                    reason: format!(
                                        "expected `|>` after `.{}`, found unexpected token", n
                                    ),
                                    span: self.current_span().unwrap_or_else(Span::dummy),
                                });
                            }
                        } else {
                            break; // `.0` not valid (skip=0 is just `|>`)
                        }
                    } else {
                        break;
                    }
                }
                Some(Ok(Token::DotDot)) => {
                    // ..|> — check peek without consuming
                    match self.peek_token() {
                        Some(Ok(Token::PipeGreater)) => {
                            self.advance(); // consume '..'
                            self.advance(); // consume '|>'
                            2usize
                        }
                        _ => break,
                    }
                }
                _ => break,
            };

            // Parse the target expression at or-level (full expression)
            let target = self.parse_or()?;
            // Validate target is callable or an identifier (auto-wrapped in desugaring)
            match &target {
                Expr::Identifier(_) | Expr::Call(_, _) => {}
                _ => {
                    return Err(SyntaxError::InvalidStatement {
                        reason: "pipe target must be a function call".to_string(),
                        span: self.current_span().unwrap_or_else(Span::dummy),
                    });
                }
            }
            steps.push(PipeStep {
                target: Box::new(target),
                skip,
            });
        }

        if steps.is_empty() {
            Ok(initial)
        } else {
            Ok(Expr::PipeChain(crate::ast::PipeChain {
                initial: Box::new(initial),
                steps,
            }))
        }
    }

    fn parse_or(&mut self) -> Result<Expr, SyntaxError> {
        let mut left = self.parse_and()?;
        while let Some(Ok(Token::OrOr)) = self.current_token() {
            self.advance();
            let right = self.parse_and()?;
                    left = Expr::BinaryOp(Box::new(BinaryOpExpr::new(BinaryOpKind::Or, left, right)));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, SyntaxError> {
        let mut left = self.parse_bitwise_or()?;
        while let Some(Ok(Token::AndAnd)) = self.current_token() {
            self.advance();
            let right = self.parse_bitwise_or()?;
            left = Expr::BinaryOp(Box::new(BinaryOpExpr::new(BinaryOpKind::And, left, right)));
        }
        Ok(left)
    }

    fn parse_bitwise_or(&mut self) -> Result<Expr, SyntaxError> {
        let mut left = self.parse_bitwise_xor()?;
        while let Some(Ok(Token::Pipe)) = self.current_token() {
            self.advance();
            let right = self.parse_bitwise_xor()?;
                    left = Expr::BinaryOp(Box::new(BinaryOpExpr::new(BinaryOpKind::BitOr, left, right)));
        }
        Ok(left)
    }

    fn parse_bitwise_xor(&mut self) -> Result<Expr, SyntaxError> {
        let mut left = self.parse_bitwise_and()?;
        while let Some(Ok(Token::BitXor)) = self.current_token() {
            self.advance();
            let right = self.parse_bitwise_and()?;
                    left = Expr::BinaryOp(Box::new(BinaryOpExpr::new(BinaryOpKind::BitXor, left, right)));
        }
        Ok(left)
    }

    fn parse_bitwise_and(&mut self) -> Result<Expr, SyntaxError> {
        let mut left = self.parse_equality()?;
        while let Some(Ok(Token::Ampersand)) = self.current_token() {
            self.advance();
            let right = self.parse_equality()?;
                    left = Expr::BinaryOp(Box::new(BinaryOpExpr::new(BinaryOpKind::BitAnd, left, right)));
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> Result<Expr, SyntaxError> {
        let mut left = self.parse_check()?;
        while let Some(token) = self.current_token() {
            match token {
                Ok(Token::EqEq) => {
                    self.advance();
                    let right = self.parse_check()?;
                    left = Expr::BinaryOp(Box::new(BinaryOpExpr::new(BinaryOpKind::Eq, left, right)));
                }
                Ok(Token::Ne) => {
                    self.advance();
                    let right = self.parse_check()?;
                    left = Expr::BinaryOp(Box::new(BinaryOpExpr::new(BinaryOpKind::Ne, left, right)));
                }
                _ => break,
            }
        }
        Ok(left)
    }

    /// Parse `is`/`from`/`like` check expressions.
    /// Precedence: tighter than `==`/`!=`, looser than `<`/`>`/`<=`/`>=`.
    fn parse_check(&mut self) -> Result<Expr, SyntaxError> {
        let mut left = self.parse_comparison()?;
        while let Some(token) = self.current_token() {
            match token {
                Ok(Token::Is) => {
                    self.advance();
                    let target = self.parse_is_target()?;
                    left = Expr::IsType(Box::new(left), target);
                }
                Ok(Token::From) => {
                    self.advance();
                    let ty = self.parse_type()?;
                    left = Expr::FromCheck(Box::new(left), ty);
                }
                Ok(Token::Like) => {
                    self.advance();
                    let right = self.parse_comparison()?;
                    left = Expr::Like(Box::new(left), Box::new(right));
                }
                _ => break,
            }
        }
        Ok(left)
    }

    /// Parse the RHS of an `is` expression: either a Type or a Variant name.
    /// `Some`, `None`, `Ok`, `Err` tokens → Variant; everything else → Type.
    fn parse_is_target(&mut self) -> Result<IsTarget, SyntaxError> {
        let target = match self.current_token() {
            Some(Ok(Token::Some)) => {
                self.advance();
                IsTarget::Variant("Some".to_string())
            }
            Some(Ok(Token::None)) => {
                self.advance();
                IsTarget::Variant("None".to_string())
            }
            Some(Ok(Token::Ok)) => {
                self.advance();
                IsTarget::Variant("Ok".to_string())
            }
            Some(Ok(Token::Err)) => {
                self.advance();
                IsTarget::Variant("Err".to_string())
            }
            _ => {
                let ty = self.parse_type()?;
                IsTarget::Type(ty)
            }
        };
        Ok(target)
    }

    fn parse_comparison(&mut self) -> Result<Expr, SyntaxError> {
        let mut left = self.parse_shift()?;
        while let Some(token) = self.current_token() {
            match token {
                Ok(Token::Lt) => {
                    self.advance();
                    let right = self.parse_shift()?;
                    left = Expr::BinaryOp(Box::new(BinaryOpExpr::new(BinaryOpKind::Lt, left, right)));
                }
                Ok(Token::Le) => {
                    self.advance();
                    let right = self.parse_shift()?;
                    left = Expr::BinaryOp(Box::new(BinaryOpExpr::new(BinaryOpKind::Le, left, right)));
                }
                Ok(Token::Gt) => {
                    self.advance();
                    let right = self.parse_shift()?;
                    left = Expr::BinaryOp(Box::new(BinaryOpExpr::new(BinaryOpKind::Gt, left, right)));
                }
                Ok(Token::Ge) => {
                    self.advance();
                    let right = self.parse_shift()?;
                    left = Expr::BinaryOp(Box::new(BinaryOpExpr::new(BinaryOpKind::Ge, left, right)));
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_shift(&mut self) -> Result<Expr, SyntaxError> {
        let mut left = self.parse_additive()?;
        while let Some(token) = self.current_token() {
            match token {
                Ok(Token::Shl) => {
                    self.advance();
                    let right = self.parse_additive()?;
                    left = Expr::BinaryOp(Box::new(BinaryOpExpr::new(BinaryOpKind::Shl, left, right)));
                }
                Ok(Token::Shr) => {
                    self.advance();
                    let right = self.parse_additive()?;
                    left = Expr::BinaryOp(Box::new(BinaryOpExpr::new(BinaryOpKind::Shr, left, right)));
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<Expr, SyntaxError> {
        let mut left = self.parse_multiplicative()?;
        while let Some(token) = self.current_token() {
            match token {
                Ok(Token::Plus) => {
                    self.advance();
                    let right = self.parse_multiplicative()?;
                    left = Expr::BinaryOp(Box::new(BinaryOpExpr::new(BinaryOpKind::Add, left, right)));
                }
                Ok(Token::PlusPlus) => {
                    self.advance();
                    let right = self.parse_multiplicative()?;
                    left = Expr::Concat(Box::new(left), Box::new(right));
                }
                Ok(Token::Minus) => {
                    self.advance();
                    let right = self.parse_multiplicative()?;
                    left = Expr::BinaryOp(Box::new(BinaryOpExpr::new(BinaryOpKind::Sub, left, right)));
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, SyntaxError> {
        let mut left = self.parse_unary()?;
        while let Some(token) = self.current_token() {
            match token {
                Ok(Token::Star) => {
                    self.advance();
                    let right = self.parse_unary()?;
                    left = Expr::BinaryOp(Box::new(BinaryOpExpr::new(BinaryOpKind::Mul, left, right)));
                }
                Ok(Token::Slash) => {
                    self.advance();
                    let right = self.parse_unary()?;
                    left = Expr::BinaryOp(Box::new(BinaryOpExpr::new(BinaryOpKind::Div, left, right)));
                }
                Ok(Token::Percent) => {
                    self.advance();
                    let right = self.parse_unary()?;
                    left = Expr::BinaryOp(Box::new(BinaryOpExpr::new(BinaryOpKind::Mod, left, right)));
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, SyntaxError> {
        if let Some(token) = self.current_token() {
            match token {
                Ok(Token::Not) => {
                    self.advance();
                    let expr = self.parse_unary()?;
                    Ok(Expr::UnaryOp(Box::new(UnaryOpExpr::new(UnaryOpKind::Not, expr))))
                }
                Ok(Token::Minus) => {
                    self.advance();
                    let expr = self.parse_unary()?;
                    Ok(Expr::UnaryOp(Box::new(UnaryOpExpr::new(UnaryOpKind::Neg, expr))))
                }
                Ok(Token::Tilde) => {
                    self.advance();
                    let expr = self.parse_unary()?;
                    Ok(Expr::UnaryOp(Box::new(UnaryOpExpr::new(UnaryOpKind::BitNot, expr))))
                }
                Ok(Token::Star) => {
                    self.advance();
                    let expr = self.parse_unary()?;
                    self.parse_postfix_expr(Expr::Deref(Box::new(expr)))
                }
                Ok(Token::Ampersand) => {
                    self.advance();
                    match self.current_token() {
                        Some(Ok(Token::LParen)) => {
                            // &(a, b, ...) — tuple destructuring LHS
                            self.advance();
                            let mut names = Vec::new();
                            loop {
                                if matches!(self.current_token(), Some(Ok(Token::Underscore))) {
                                    names.push("_".to_string());
                                    self.advance();
                                } else {
                                    names.push(self.expect_identifier()?);
                                }
                                if let Some(Ok(Token::Comma)) = self.current_token() {
                                    self.advance();
                                } else {
                                    break;
                                }
                            }
                            self.expect(Token::RParen)?;
                            // No postfix ops on &(a, b) — return directly
                            Ok(Expr::TupleDestructure(names, Box::new(Expr::Term)))
                        }
                        _ => {
                            let expr = self.parse_unary()?;
                            Ok(Expr::AddrOf(Box::new(expr)))
                        }
                    }
                }
                Ok(Token::At) if self.in_quote_block => {
                    self.advance();
                    if let Some(Ok(Token::LBrace)) = self.current_token() {
                        // @{expr} — computed interpolation inside quote { }
                        self.advance();
                        let expr = self.parse_expression()?;
                        self.expect(Token::RBrace)?;
                        return Ok(Expr::InterpolateExpr(Box::new(expr)));
                    }
                    // @ident — variable interpolation inside quote { }
                    let name = self.expect_identifier()?;
                    return Ok(Expr::Interpolate(name));
                }
                Ok(Token::At) => {
                    self.advance();
                    // @"..." — regex literal; @ident — prior state
                    if let Some(Ok(Token::String(s))) = self.current_token() {
                        let pattern = s.clone();
                        self.advance();
                        return Ok(Expr::RegexLiteral(pattern));
                    }
                    match self.expect_identifier() {
                        Ok(name) => self.parse_postfix_expr(Expr::PriorState(name)),
                        Err(e) => return Err(e),
                    }
                }
                _ => self.parse_postfix(),
            }
        } else {
            self.parse_postfix()
        }
    }

    fn parse_projection_target(&mut self) -> Result<ProjectionTarget, SyntaxError> {
        let name = self.expect_identifier()?;
        match name.as_str() {
            "Size" => Ok(ProjectionTarget::Size),
            "Bytes" => Ok(ProjectionTarget::Bytes),
            "Ptr" => Ok(ProjectionTarget::Ptr),
            "Alignment" => Ok(ProjectionTarget::Alignment),
            "Range" => Ok(ProjectionTarget::Range),
            "Popcount" => Ok(ProjectionTarget::Popcount),
            "LeadingZeros" => Ok(ProjectionTarget::LeadingZeros),
            "TrailingZeros" => Ok(ProjectionTarget::TrailingZeros),
            "Absolute" => Ok(ProjectionTarget::Absolute),
            "BitReverse" => Ok(ProjectionTarget::BitReverse),
            "Type" => Ok(ProjectionTarget::Type),
            "Ptr!" => Ok(ProjectionTarget::PtrBang),
            "Match" => {
                return self.spanned_err("Match projection is no longer supported; use the `<:` operator instead".to_string());
            }
            "Keys" => Ok(ProjectionTarget::Keys),
            "Values" => Ok(ProjectionTarget::Values),
            "IsEmpty" => Ok(ProjectionTarget::IsEmpty),
            "Contains" => {
                self.expect(Token::LParen)?;
                let expr = self.parse_expression()?;
                self.expect(Token::RParen)?;
                Ok(ProjectionTarget::Contains(Box::new(expr)))
            }
            "Get" => {
                // Get(key) — non-mutating HashMap read → Option<V>
                self.expect(Token::LParen)?;
                let key_expr = self.parse_expression()?;
                self.expect(Token::RParen)?;
                Ok(ProjectionTarget::Get(Box::new(key_expr)))
            }
            "Top" => Ok(ProjectionTarget::Top),
            "Front" => Ok(ProjectionTarget::Front),
            "Elements" => Ok(ProjectionTarget::Elements),
            "AsStack" => Ok(ProjectionTarget::AsStack),
            "AsQueue" => Ok(ProjectionTarget::AsQueue),
            // Phase 2F: Metadata projections
            "Width" => Ok(ProjectionTarget::Width),
            "Endian" => Ok(ProjectionTarget::Endian),
            "Codec" => Ok(ProjectionTarget::Codec),
            "Ops" => Ok(ProjectionTarget::Ops),
            "Address" => Ok(ProjectionTarget::Address),
            "Name" => Ok(ProjectionTarget::Name),
            "Params" => Ok(ProjectionTarget::Params),
            "Returns" => Ok(ProjectionTarget::Returns),
            "Arity" => Ok(ProjectionTarget::Arity),
            "Loc" => Ok(ProjectionTarget::Loc),
            "Doc" => Ok(ProjectionTarget::Doc),
            "Hash" => Ok(ProjectionTarget::Hash),
            "Contracts" => Ok(ProjectionTarget::Contracts),
            "Module" => Ok(ProjectionTarget::Module),
            "IsPure" => Ok(ProjectionTarget::IsPure),
            "FnSpan" => Ok(ProjectionTarget::FnSpan),
            _ => {
                // User-defined projection — check for parameterized form
                // Handle intrinsic call targets: `fadd#(rhs)` → name# with arg
                if matches!(self.current_token(), Some(Ok(Token::Hash)))
                    && matches!(self.peek_token(), Some(Ok(Token::LParen)))
                {
                    self.advance(); // consume #
                    self.advance(); // consume (
                    let expr = self.parse_expression()?;
                    self.expect(Token::RParen)?;
                    Ok(ProjectionTarget::UserDefinedWithArg(
                        format!("{}#", name),
                        Box::new(expr),
                    ))
                } else if matches!(self.current_token(), Some(Ok(Token::Hash))) {
                    // Bare intrinsic reference: `fneg#` or `ptrtoint#`
                    self.advance(); // consume #
                    Ok(ProjectionTarget::UserDefined(format!("{}#", name)))
                } else if matches!(self.current_token(), Some(Ok(Token::LParen))) {
                    self.advance();
                    let expr = self.parse_expression()?;
                    self.expect(Token::RParen)?;
                    Ok(ProjectionTarget::UserDefinedWithArg(
                        name,
                        Box::new(expr),
                    ))
                } else {
                    Ok(ProjectionTarget::UserDefined(name))
                }
            }
        }
    }

    fn parse_postfix_expr(&mut self, expr: Expr) -> Result<Expr, SyntaxError> {
        let mut expr = expr;
        loop {
            if let Some(Ok(Token::LBracket)) = self.current_token() {
                self.advance();
                // Check if this is a multidimensional slice by looking ahead for commas
                // before any `;`, `..`, `::`, or `]`
                let is_multi = self.peek_multidimensional_slice();
                if is_multi {
                    let result = self.parse_multi_slice()?;
                    expr = Expr::MultiSlice {
                        value: Box::new(expr),
                        ops: result.ops,
                    };
                } else {
                    let result = self.parse_bracket_contents()?;
                    expr = self.bracket_contents_to_expr(expr, result);
                }
            } else if let Some(Ok(Token::Dot)) = self.current_token() {
                self.advance();
                let member_name = if let Some(Ok(Token::Integer(n))) = self.current_token() {
                    let s = n.to_string();
                    self.advance();
                    s
                } else {
                    self.expect_identifier()?
                };
                if let Some(Ok(Token::LParen)) = self.current_token() {
                    self.advance();
                    let mut args = Vec::new();
                    if let Some(Ok(Token::RParen)) = self.current_token() {
                    } else {
                        loop {
                            args.push(self.parse_expression()?);
                            if let Some(Ok(Token::Comma)) = self.current_token() {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect(Token::RParen)?;
                    let mut call_args = vec![expr];
                    call_args.extend(args);
                    expr = Expr::Call(member_name, call_args);
                } else {
                    expr = Expr::FieldAccess(Box::new(expr), member_name);
                }
            } else if let Some(Ok(Token::ColonGreaterThan)) = self.current_token() {
                self.advance();
                let target = self.parse_projection_target()?;
                expr = Expr::Projection {
                    source: Box::new(expr),
                    target,
                };
            } else if let Some(Ok(Token::At)) = self.current_token() {
                // @/ — bit-range extraction in expression context: word @/0..3
                if let Some(Ok(Token::Slash)) = self.peek_token() {
                    self.advance(); // consume @
                    self.advance(); // consume /
                    let br = self.parse_bit_range()?;
                    expr = Expr::Projection {
                        source: Box::new(expr),
                        target: ProjectionTarget::BitRange(br),
                    };
                } else {
                    break;
                }
            } else if let Some(Ok(Token::Hash)) = self.current_token() {
                // Only treat `#` as intrinsic call if followed by `(`
                if matches!(self.peek_token(), Some(Ok(Token::LParen))) {
                    self.advance(); // consume #
                    self.advance(); // consume (
                    let mut args = Vec::new();
                    if !matches!(self.current_token(), Some(Ok(Token::RParen))) {
                        loop {
                            args.push(self.parse_expression()?);
                            if matches!(self.current_token(), Some(Ok(Token::Comma))) {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect(Token::RParen)?;
                    if let Expr::Identifier(name) = &expr {
                        if let Some(intrinsic) = Intrinsic::from_name(name) {
                            expr = Expr::IntrinsicCall { intrinsic, args };
                        } else {
                            // Could be a user-defined `inop#` — defer validation to typechecker
                            expr = Expr::IntrinsicCall { intrinsic: Intrinsic::UserDefined(name.clone()), args };
                        }
                    } else {
                        return self.spanned_err("intrinsic call requires an identifier".to_string());
                    }
                } else {
                    break; // leave # for outer parser (modifiers)
                }
            } else if let Some(Ok(Token::Within)) = self.current_token() {
                // expr within N cycles (M) ~? fallback
                self.advance();
                let bound = self.expect_integer()?;
                if bound < 0 {
                    return self.spanned_err("Bound must be non-negative".to_string());
                }
                let unit = self.parse_time_unit()?;
                let retries = if let Some(Ok(Token::LParen)) = self.current_token() {
                    self.advance();
                    let n = self.expect_integer()?;
                    if n < 0 { return self.spanned_err("Retry must be non-negative".to_string()); }
                    self.expect(Token::RParen)?;
                    n as u64
                } else if let Some(Ok(Token::Identifier(s))) = self.current_token() {
                    if s == "retry" || s == "RETRY" {
                        self.advance();
                        let n = self.expect_integer()?;
                        if n < 0 { return self.spanned_err("Retry must be non-negative".to_string()); }
                        n as u64
                    } else { 0 }
                } else { 0 };
                if !matches!(self.current_token(), Some(Ok(Token::TildeQuestion))) {
                    return self.spanned_err("Expected '~?' after timeout bound".to_string());
                }
                self.advance();
                let fallback = Box::new(self.parse_expression()?);
                expr = Expr::Within {
                    body: Box::new(expr),
                    bound: bound as u64,
                    unit,
                    retries,
                    fallback,
                };
            } else if let Some(Ok(Token::LParen)) = self.current_token() {
                self.advance();
                let mut args = Vec::new();
                if !matches!(self.current_token(), Some(Ok(Token::RParen))) {
                    loop {
                        args.push(self.parse_expression()?);
                        if let Some(Ok(Token::Comma)) = self.current_token() {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                self.expect(Token::RParen)?;
                let fn_name = match &expr {
                    Expr::Identifier(n) => n.clone(),
                    Expr::AddrOf(inner) => inner.as_var_name().map(|s| s.to_string()).unwrap_or("".to_string()),
                    _ => "".to_string(),
                };
                expr = Expr::Call(fn_name, args);
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_postfix(&mut self) -> Result<Expr, SyntaxError> {
        let mut expr = self.parse_primary()?;
        loop {
            if let Some(Ok(Token::LBracket)) = self.current_token() {
                self.advance();
                // Check for multidimensional slice: `s[..., 0]`, `s[@3:0..10]`
                if self.peek_multidimensional_slice() {
                    let result = self.parse_multi_slice()?;
                    expr = Expr::MultiSlice {
                        value: Box::new(expr),
                        ops: result.ops,
                    };
                    continue;
                }
                // Check for slice: start..end or start..end..stride
                let first = self.parse_expression()?;
                if let Some(Ok(Token::DotDot)) = self.current_token() {
                    self.advance();
                    let second = self.parse_expression()?;
                    // Check for stride: start..end..stride
                    if let Some(Ok(Token::DotDot)) = self.current_token() {
                        self.advance();
                        let stride = self.parse_expression()?;
                        self.expect(Token::RBracket)?;
                        expr = Expr::Slice {
                            value: Box::new(expr),
                            start: Some(Box::new(first)),
                            end: Some(Box::new(second)),
                            stride: Some(Box::new(stride)),
                            mask: None,
                        };
                    } else {
                        self.expect(Token::RBracket)?;
                        expr = Expr::Slice {
                            value: Box::new(expr),
                            start: Some(Box::new(first)),
                            end: Some(Box::new(second)),
                            stride: None,
                            mask: None,
                        };
                    }
                } else {
                    self.expect(Token::RBracket)?;
                    expr = Expr::ListIndex(Box::new(expr), Box::new(first));
                }
            } else if let Some(Ok(Token::Dot)) = self.current_token() {
                // Don't consume '.' if it's a dot-skip pipe (.|> or .N|>)
                if matches!(self.peek_token(), Some(Ok(Token::PipeGreater))) {
                    break; // .|>
                }
                if matches!(self.peek_token(), Some(Ok(Token::Integer(_))))
                    && matches!(self.peek_token2(), Some(Ok(Token::PipeGreater)))
                {
                    break; // .N|>
                }
                self.advance();
                let member_name = if let Some(Ok(Token::Integer(n))) = self.current_token() {
                    let s = n.to_string();
                    self.advance();
                    s
                } else {
                    self.expect_identifier()?
                };
                if let Some(Ok(Token::LParen)) = self.current_token() {
                    self.advance();
                    let mut args = Vec::new();
                    if let Some(Ok(Token::RParen)) = self.current_token() {
                    } else {
                        loop {
                            args.push(self.parse_expression()?);
                            if let Some(Ok(Token::Comma)) = self.current_token() {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect(Token::RParen)?;
                    let mut call_args = vec![expr];
                    call_args.extend(args);
                    expr = Expr::Call(member_name, call_args);
                } else {
                    expr = Expr::FieldAccess(Box::new(expr), member_name);
                }
            } else if let Some(Ok(Token::As)) = self.current_token() {
                self.advance();
                let cast_type = self.parse_type()?;
                expr = Expr::Cast(Box::new(expr), cast_type);
            } else if let Some(Ok(Token::ColonGreaterThan)) = self.current_token() {
                self.advance();
                let target = self.parse_projection_target()?;
                expr = Expr::Projection {
                    source: Box::new(expr),
                    target,
                };
            } else if let Some(Ok(Token::At)) = self.current_token() {
                // @/ — bit-range extraction in expression context: x @/0..7
                if let Some(Ok(Token::Slash)) = self.peek_token() {
                    self.advance(); // consume @
                    self.advance(); // consume /
                    let br = self.parse_bit_range()?;
                    expr = Expr::Projection {
                        source: Box::new(expr),
                        target: ProjectionTarget::BitRange(br),
                    };
                } else {
                    break;
                }
            } else if let Some(Ok(Token::Hash)) = self.current_token() {
                // Only treat `#` as intrinsic call if followed by `(`
                if matches!(self.peek_token(), Some(Ok(Token::LParen))) {
                    self.advance(); // consume #
                    self.advance(); // consume (
                    let mut args = Vec::new();
                    if !matches!(self.current_token(), Some(Ok(Token::RParen))) {
                        loop {
                            args.push(self.parse_expression()?);
                            if matches!(self.current_token(), Some(Ok(Token::Comma))) {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect(Token::RParen)?;
                    if let Expr::Identifier(name) = &expr {
                        if let Some(intrinsic) = Intrinsic::from_name(name) {
                            expr = Expr::IntrinsicCall { intrinsic, args };
                        } else {
                            // Could be a user-defined `inop#` — defer validation to typechecker
                            expr = Expr::IntrinsicCall { intrinsic: Intrinsic::UserDefined(name.clone()), args };
                        }
                    } else {
                        return self.spanned_err("intrinsic call requires an identifier".to_string());
                    }
                } else {
                    break; // leave # for outer parser (modifiers)
                }
            } else if let Some(Ok(Token::Within)) = self.current_token() {
                // foo() within N cycles (M) ~? bar()
                self.advance();
                let bound = self.expect_integer()?;
                if bound < 0 {
                    return self.spanned_err("Bound must be non-negative".to_string());
                }
                let unit = self.parse_time_unit()?;
                // Optional retry count: (N) or retry N
                let retries = if let Some(Ok(Token::LParen)) = self.current_token() {
                    self.advance();
                    let n = self.expect_integer()?;
                    if n < 0 {
                        return self.spanned_err("Retry count must be non-negative".to_string());
                    }
                    self.expect(Token::RParen)?;
                    n as u64
                } else if let Some(Ok(Token::Identifier(s))) = self.current_token() {
                    if s == "retry" || s == "RETRY" {
                        self.advance();
                        let n = self.expect_integer()?;
                        if n < 0 {
                            return self.spanned_err("Retry count must be non-negative".to_string());
                        }
                        n as u64
                    } else {
                        0
                    }
                } else {
                    0
                };
                // Expect ~? and parse fallback
                if let Some(Ok(Token::TildeQuestion)) = self.current_token() {
                    self.advance();
                } else {
                    return self.spanned_err("Expected '~?' after timeout bound".to_string());
                }
                let fallback = Box::new(self.parse_expression()?);
                expr = Expr::Within {
                    body: Box::new(expr),
                    bound: bound as u64,
                    unit,
                    retries,
                    fallback,
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, SyntaxError> {
        // Handle $name(args) — template call
        if let Some(Ok(Token::Dollar)) = self.current_token() {
            self.advance();
            return self.parse_template_call();
        }

        // Handle $!name(args) — macro call
        if let Some(Ok(Token::DollarBang)) = self.current_token() {
            self.advance();
            return self.parse_macro_call();
        }

        // Handle quote { ... } — quasiquoting
        if let Some(Ok(Token::Quote)) = self.current_token() {
            self.advance();
            return self.parse_quote_block();
        }

        match self.current_token() {
            Some(Ok(Token::Integer(val))) => {
                let val = *val;
                self.advance();
                Ok(Expr::Literal(Box::new(LiteralExpr::Integer(val))))
            }
            Some(Ok(Token::Float(val))) => {
                let val = *val;
                self.advance();
                Ok(Expr::Literal(Box::new(LiteralExpr::Float(val))))
            }
            Some(Ok(Token::IntegerI8(n))) => { let n = *n; self.advance(); Ok(Expr::IntegerSuffixed(n, Type::Custom("Int8".to_string()))) }
            Some(Ok(Token::IntegerI16(n))) => { let n = *n; self.advance(); Ok(Expr::IntegerSuffixed(n, Type::Custom("Int16".to_string()))) }
            Some(Ok(Token::IntegerI32(n))) => { let n = *n; self.advance(); Ok(Expr::IntegerSuffixed(n, Type::Custom("Int32".to_string()))) }
            Some(Ok(Token::IntegerI64(n))) => { let n = *n; self.advance(); Ok(Expr::IntegerSuffixed(n, Type::Custom("Int".to_string()))) }
            Some(Ok(Token::IntegerU8(n))) => { let n = *n; self.advance(); Ok(Expr::IntegerSuffixed(n, Type::Custom("UInt8".to_string()))) }
            Some(Ok(Token::IntegerU16(n))) => { let n = *n; self.advance(); Ok(Expr::IntegerSuffixed(n, Type::Custom("UInt16".to_string()))) }
            Some(Ok(Token::IntegerU32(n))) => { let n = *n; self.advance(); Ok(Expr::IntegerSuffixed(n, Type::Custom("UInt32".to_string()))) }
            Some(Ok(Token::IntegerU64(n))) => { let n = *n; self.advance(); Ok(Expr::IntegerSuffixed(n, Type::Custom("UInt".to_string()))) }
            Some(Ok(Token::Float32(f))) => { let f = *f; self.advance(); Ok(Expr::Literal(Box::new(LiteralExpr::Float(f)))) }
            Some(Ok(Token::Float64(f))) => { let f = *f; self.advance(); Ok(Expr::Float64(f)) }
            Some(Ok(Token::String(val))) => {
                let val = val.clone();
                self.advance();
                Ok(Expr::Literal(Box::new(LiteralExpr::String(val))))
            }
            Some(Ok(Token::Char(val))) => {
                let val = *val;
                self.advance();
                Ok(Expr::Literal(Box::new(LiteralExpr::Char(val))))
            }
            Some(Ok(Token::BoolTrue)) => {
                self.advance();
                Ok(Expr::Literal(Box::new(LiteralExpr::Bool(true))))
            }
            Some(Ok(Token::BoolFalse)) => {
                self.advance();
                Ok(Expr::Literal(Box::new(LiteralExpr::Bool(false))))
            }
            Some(Ok(Token::Term)) => {
                self.advance();
                Ok(Expr::Term)
            }
            Some(Ok(Token::Cell)) => {
                self.advance();
                // cell name(args) — explicit synchronous cell creation
                let cell_name = self.expect_identifier()?;
                let args = if let Some(Ok(Token::LParen)) = self.current_token() {
                    self.advance();
                    let mut a = Vec::new();
                    loop {
                        if let Some(Ok(Token::RParen)) = self.current_token() { break; }
                        a.push(self.parse_expression()?);
                        if let Some(Ok(Token::Comma)) = self.current_token() {
                            self.advance();
                        } else { break; }
                    }
                    self.expect(Token::RParen)?;
                    a
                } else { vec![] };
                Ok(Expr::CellCall(Box::new(Expr::Identifier(cell_name)), args))
            }
            Some(Ok(Token::Match)) => {
                self.advance();
                self.parse_match_expr()
            }
            Some(Ok(Token::Identifier(name))) => {
                let name = name.clone();
                self.advance();
                // Check if it's a struct literal: TypeName { field: value, ... }
                if let Some(Ok(Token::LBrace)) = self.current_token() {
                    self.advance();
                    let mut fields = Vec::new();
                    if let Some(Ok(Token::RBrace)) = self.current_token() {
                        // Empty struct
                    } else {
                        loop {
                            let field_name = self.expect_identifier()?;
                            self.expect(Token::Colon)?;
                            let field_value = self.parse_expression()?;
                            fields.push((field_name, field_value));
                            if let Some(Ok(Token::Comma)) = self.current_token() {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect(Token::RBrace)?;
                    Ok(Expr::StructInstance(name, fields))
                // Check if it's a function call
                } else if let Some(Ok(Token::LParen)) = self.current_token() {
                    self.advance();
                    let mut args = Vec::new();
                    if let Some(Ok(Token::RParen)) = self.current_token() {
                        // Empty args
                    } else {
                        loop {
                            args.push(self.parse_expression()?);
                            if let Some(Ok(Token::Comma)) = self.current_token() {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect(Token::RParen)?;
                    Ok(Expr::Call(name, args))
                } else {
                    Ok(Expr::Identifier(name))
                }
            }
            Some(Ok(token)) if Self::keyword_token_to_name(token).is_some() => {
                let name = Self::keyword_token_to_name(token).unwrap().to_string();
                self.parse_keyword_as_expr(&name)
            }
            Some(Ok(Token::TypeData)) => {
                self.advance();
                if let Some(Ok(Token::LParen)) = self.current_token() {
                    self.advance();
                    let mut args = Vec::new();
                    if let Some(Ok(Token::RParen)) = self.current_token() {
                        // Empty args
                    } else {
                        loop {
                            args.push(self.parse_expression()?);
                            if let Some(Ok(Token::Comma)) = self.current_token() {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect(Token::RParen)?;
                    Ok(Expr::Call("Data".to_string(), args))
                } else {
                    Ok(Expr::Identifier("Data".to_string()))
                }
            }
            Some(Ok(Token::TypeInt)) => {
                self.advance();
                if let Some(Ok(Token::LParen)) = self.current_token() {
                    self.advance();
                    let mut args = Vec::new();
                    if let Some(Ok(Token::RParen)) = self.current_token() {
                        // Empty args
                    } else {
                        loop {
                            args.push(self.parse_expression()?);
                            if let Some(Ok(Token::Comma)) = self.current_token() {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect(Token::RParen)?;
                    Ok(Expr::Call("Int".to_string(), args))
                } else {
                    Ok(Expr::Identifier("Int".to_string()))
                }
            }
            Some(Ok(Token::TypeFloat)) => {
                self.advance();
                if let Some(Ok(Token::LParen)) = self.current_token() {
                    self.advance();
                    let mut args = Vec::new();
                    if let Some(Ok(Token::RParen)) = self.current_token() {
                        // Empty args
                    } else {
                        loop {
                            args.push(self.parse_expression()?);
                            if let Some(Ok(Token::Comma)) = self.current_token() {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect(Token::RParen)?;
                    Ok(Expr::Call("Float".to_string(), args))
                } else {
                    Ok(Expr::Identifier("Float".to_string()))
                }
            }
            Some(Ok(Token::TypeString)) => {
                self.advance();
                if let Some(Ok(Token::LParen)) = self.current_token() {
                    self.advance();
                    let mut args = Vec::new();
                    if let Some(Ok(Token::RParen)) = self.current_token() {
                        // Empty args
                    } else {
                        loop {
                            args.push(self.parse_expression()?);
                            if let Some(Ok(Token::Comma)) = self.current_token() {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect(Token::RParen)?;
                    Ok(Expr::Call("String".to_string(), args))
                } else {
                    Ok(Expr::Identifier("String".to_string()))
                }
            }
            Some(Ok(Token::TypeBool)) => {
                self.advance();
                if let Some(Ok(Token::LParen)) = self.current_token() {
                    self.advance();
                    let mut args = Vec::new();
                    if let Some(Ok(Token::RParen)) = self.current_token() {
                        // Empty args
                    } else {
                        loop {
                            args.push(self.parse_expression()?);
                            if let Some(Ok(Token::Comma)) = self.current_token() {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect(Token::RParen)?;
                    Ok(Expr::Call("Bool".to_string(), args))
                } else {
                    Ok(Expr::Identifier("Bool".to_string()))
                }
            }
            Some(Ok(Token::TypeVoid)) => {
                self.advance();
                if let Some(Ok(Token::LParen)) = self.current_token() {
                    self.advance();
                    let mut args = Vec::new();
                    if let Some(Ok(Token::RParen)) = self.current_token() {
                        // Empty args
                    } else {
                        loop {
                            args.push(self.parse_expression()?);
                            if let Some(Ok(Token::Comma)) = self.current_token() {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect(Token::RParen)?;
                    Ok(Expr::Call("Void".to_string(), args))
                } else {
                    Ok(Expr::Identifier("Void".to_string()))
                }
            }
            Some(Ok(Token::Ok)) | Some(Ok(Token::Err)) | Some(Ok(Token::Some)) | Some(Ok(Token::None)) => {
                let name = match self.current_token() {
                    Some(Ok(Token::Ok)) => "Ok".to_string(),
                    Some(Ok(Token::Err)) => "Err".to_string(),
                    Some(Ok(Token::Some)) => "Some".to_string(),
                    Some(Ok(Token::None)) => "None".to_string(),
                    _ => unreachable!(),
                };
                self.advance();
                if let Some(Ok(Token::LParen)) = self.current_token() {
                    self.advance();
                    let mut args = Vec::new();
                    if let Some(Ok(Token::RParen)) = self.current_token() {
                    } else {
                        loop {
                            args.push(self.parse_expression()?);
                            if let Some(Ok(Token::Comma)) = self.current_token() {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect(Token::RParen)?;
                    Ok(Expr::Call(name, args))
                } else {
                    Ok(Expr::Identifier(name))
                }
            }
            Some(Ok(Token::LBrace)) => {
                self.advance();
                // Empty braces: try ObjectLiteral, default to SetLiteral
                if let Some(Ok(Token::RBrace)) = self.current_token() {
                    self.advance();
                    return Ok(Expr::ObjectLiteral(vec![]));
                }
                // Peek to check if first element starts with a string/int (map literal)
                // or an identifier followed by `:` (object literal) or just `,` (set literal)
                let is_map_like = matches!(self.current_token(), Some(Ok(Token::String(_))))
                    || matches!(self.current_token(), Some(Ok(Token::Integer(_))))
                    || matches!(self.current_token(), Some(Ok(Token::TypeString)));
                if is_map_like {
                    // MapLiteral: {"key": value, ...} or {0: value, ...}
                    let mut entries = Vec::new();
                    loop {
                        let key = self.parse_expression()?;
                        self.expect(Token::Colon)?;
                        let val = self.parse_expression()?;
                        entries.push((key, val));
                        if let Some(Ok(Token::Comma)) = self.current_token() {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    self.expect(Token::RBrace)?;
                    return Ok(Expr::MapLiteral(entries));
                }
                // Try ObjectLiteral first: { ident: value, ... }
                match self.current_token() {
                    Some(Ok(Token::Identifier(_))) => {
                        let field_name = self.expect_identifier()?;
                        if let Some(Ok(Token::Colon)) = self.current_token() {
                            // ObjectLiteral
                            self.advance();
                            let field_value = self.parse_expression()?;
                            let mut fields = vec![(field_name, field_value)];
                            while let Some(Ok(Token::Comma)) = self.current_token() {
                                self.advance();
                                let fn2 = self.expect_identifier()?;
                                self.expect(Token::Colon)?;
                                let fv2 = self.parse_expression()?;
                                fields.push((fn2, fv2));
                            }
                            self.expect(Token::RBrace)?;
                            Ok(Expr::ObjectLiteral(fields))
                        } else {
                            // Single identifier without colon → SetLiteral element
                            let mut elements = vec![Expr::Identifier(field_name)];
                            while let Some(Ok(Token::Comma)) = self.current_token() {
                                self.advance();
                                elements.push(self.parse_expression()?);
                            }
                            self.expect(Token::RBrace)?;
                            Ok(Expr::SetLiteral(elements))
                        }
                    }
                    _ => {
                        // SetLiteral: { expr, expr, ... }
                        let first = self.parse_expression()?;
                        let mut elements = vec![first];
                        while let Some(Ok(Token::Comma)) = self.current_token() {
                            self.advance();
                            elements.push(self.parse_expression()?);
                        }
                        self.expect(Token::RBrace)?;
                        Ok(Expr::SetLiteral(elements))
                    }
                }
            }
            Some(Ok(Token::LParen)) => {
                // Check for C-style prefix cast: (Type)expr
                // Built-in types have dedicated tokens (TypeInt, TypeString, etc.),
                // so this is unambiguous with a parenthesized expression.
                let is_cast = matches!(self.peek_token(), 
                    Some(Ok(Token::TypeInt | Token::TypeFloat | Token::TypeString |
                           Token::TypeChar | Token::TypeBool | Token::TypeUInt |
                           Token::TypeData | Token::TypeVoid)));
                if is_cast {
                    self.advance(); // consume (
                    let cast_ty = self.parse_type()?;
                    self.expect(Token::RParen)?; // consume )
                    let inner = self.parse_expression()?;
                    return Ok(Expr::Cast(Box::new(inner), cast_ty));
                }
                self.advance();
                // Check if it's a tuple or just a parenthesized expression
                let expr = self.parse_expression()?;
                if let Some(Ok(Token::Comma)) = self.current_token() {
                    // It's a tuple
                    self.advance();
                    let mut elements = vec![expr];
                    loop {
                        elements.push(self.parse_expression()?);
                        if let Some(Ok(Token::Comma)) = self.current_token() {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    self.expect(Token::RParen)?;
                    Ok(Expr::Tuple(elements))
                } else {
                    self.expect(Token::RParen)?;
                    Ok(expr)
                }
            }
            Some(Ok(Token::LBracket)) => {
                self.advance();
                let mut elements = Vec::new();
                if let Some(Ok(Token::RBracket)) = self.current_token() {
                } else {
                    loop {
                        elements.push(self.parse_expression()?);
                        if let Some(Ok(Token::Comma)) = self.current_token() {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                self.expect(Token::RBracket)?;
                Ok(Expr::ListLiteral(elements))
            }
            Some(Ok(Token::Underscore)) => {
                self.advance();
                Ok(Expr::Identifier("_".to_string()))
            }
            Some(Ok(Token::TildeSlash)) => {
                self.advance();
                let identifier = self.expect_identifier()?;
                let path = format!("~/{}", identifier);
                Ok(Expr::String(path))
            }
            _ => {
                // Fallback: try expect_identifier (handles keywords + identifiers)
                match self.expect_identifier() {
                    Ok(name) => {
                        if let Some(Ok(Token::LParen)) = self.current_token() {
                            self.advance();
                            let mut args = Vec::new();
                            if let Some(Ok(Token::RParen)) = self.current_token() {
                            } else {
                                loop {
                                    args.push(self.parse_expression()?);
                                    if let Some(Ok(Token::Comma)) = self.current_token() {
                                        self.advance();
                                    } else {
                                        break;
        }
    }

    #[test]
    fn test_parse_template_def() {
        let src = "template unless(cond: Expr, body: Block) -> Stmt { return quote { [@cond] { @body } }; };";
        let mut parser = Parser::new(src);
        let prog = parser.parse().unwrap();
        match &prog.items[0] {
            TopLevel::TemplateDef { name, params, return_type, body } => {
                assert_eq!(name, "unless");
                assert_eq!(params.len(), 2);
                assert_eq!(params[0].0, "cond");
                assert_eq!(params[0].1, MacroArgType::Expr);
                assert_eq!(params[1].0, "body");
                assert_eq!(params[1].1, MacroArgType::Block);
                assert!(return_type == &Some(MacroArgType::Stmt));
                assert!(!body.is_empty());
            }
            other => panic!("Expected TemplateDef, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_macro_def() {
        let src = "macro circular_buffer(name: String, size: Int) -> Block { [size <= 0] { $error(\"bad size\"); }; return compile#(\"state @{name}_head: Int = 0;\"); };";
        let mut parser = Parser::new(src);
        let prog = parser.parse().unwrap();
        match &prog.items[0] {
            TopLevel::MacroDef { name, params, return_type, body } => {
                assert_eq!(name, "circular_buffer");
                assert_eq!(params.len(), 2);
                assert_eq!(params[0].0, "name");
                assert_eq!(params[0].1, MacroArgType::String);
                assert_eq!(params[1].0, "size");
                assert_eq!(params[1].1, MacroArgType::Int);
                assert!(return_type == &Some(MacroArgType::Block));
                assert!(!body.is_empty());
            }
            other => panic!("Expected MacroDef, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_template_call() {
        let src = "$unless(sensor_tripped) { keep_moving(); };";
        let mut parser = Parser::new(src);
        let prog = parser.parse().unwrap();
        match &prog.items[0] {
            TopLevel::Statement(stmt) => {
                if let Statement::Expression(Expr::TemplateCall { name, args, block, .. }) = stmt.as_ref() {
                    assert_eq!(name, "unless");
                    assert_eq!(args.len(), 1);
                    assert!(block.is_some());
                    let b = block.as_ref().unwrap();
                    assert_eq!(b.statements.len(), 1);
                } else {
                    panic!("Expected TemplateCall expression, got {:?}", stmt);
                }
            }
            other => panic!("Expected Statement, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_macro_call() {
        let src = "$!circular_buffer(\"rx\", 256);";
        let mut parser = Parser::new(src);
        let prog = parser.parse().unwrap();
        match &prog.items[0] {
            TopLevel::Statement(stmt) => {
                if let Statement::Expression(Expr::MacroCall { name, args, block, .. }) = stmt.as_ref() {
                    assert_eq!(name, "circular_buffer");
                    assert_eq!(args.len(), 2);
                    assert!(block.is_none());
                } else {
                    panic!("Expected MacroCall expression, got {:?}", stmt);
                }
            }
            other => panic!("Expected Statement, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_quote_block() {
        let src = "let x = quote { state @name: Int = 0; };";
        let mut parser = Parser::new(src);
        let prog = parser.parse().unwrap();
        match &prog.items[0] {
            TopLevel::Statement(stmt) => {
                if let Statement::Let { expr: Some(Expr::QuoteBlock { statements, .. }), .. } = stmt.as_ref() {
                    assert!(!statements.is_empty(), "QuoteBlock should contain statements");
                } else {
                    panic!("Expected Let with QuoteBlock, got {:?}", stmt);
                }
            }
            other => panic!("Expected Statement, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_template_def_no_return_type() {
        let src = "template foo(x: Int) { return $bar(x); };";
        let mut parser = Parser::new(src);
        let prog = parser.parse().unwrap();
        match &prog.items[0] {
            TopLevel::TemplateDef { name, return_type, .. } => {
                assert_eq!(name, "foo");
                assert!(return_type.is_none());
            }
            other => panic!("Expected TemplateDef, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_template_call_no_block() {
        let src = "$double(5);";
        let mut parser = Parser::new(src);
        let prog = parser.parse().unwrap();
        match &prog.items[0] {
            TopLevel::Statement(stmt) => {
                if let Statement::Expression(Expr::TemplateCall { name, args, block, .. }) = stmt.as_ref() {
                    assert_eq!(name, "double");
                    assert_eq!(args.len(), 1);
                    assert!(block.is_none());
                } else {
                    panic!("Expected TemplateCall, got {:?}", stmt);
                }
            }
            other => panic!("Expected Statement, got {:?}", other),
        }
    }
}
                            self.expect(Token::RParen)?;
                            Ok(Expr::Call(name, args))
                        } else {
                            Ok(Expr::Identifier(name))
                        }
                    }
                    Err(e) => {
                        if self.current_token().is_none() {
                            self.spanned_err("Unexpected EOF in expression".to_string())
                        } else {
                            Err(e)
                        }
                    }
                }
            }
        }
    }

    fn parse_template_call(&mut self) -> Result<Expr, SyntaxError> {
        let name = self.expect_identifier()?;
        self.expect(Token::LParen)?;
        let mut args = Vec::new();
        if !matches!(self.current_token(), Some(Ok(Token::RParen))) {
            loop {
                args.push(self.parse_expression()?);
                if let Some(Ok(Token::Comma)) = self.current_token() {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        self.expect(Token::RParen)?;

        // Optional trailing block: $name(args) { ... }
        let block = if let Some(Ok(Token::LBrace)) = self.current_token() {
            self.advance();
            let saved = self.in_quote_block;
            self.in_quote_block = false;
            let stmts = self.parse_body()?;
            self.in_quote_block = saved;
            self.expect(Token::RBrace)?;
            Some(crate::ast::Block { statements: stmts, trailing_expr: None })
        } else {
            None
        };

        let span = self.current_span();
        Ok(Expr::TemplateCall { name, args, block, span })
    }

    fn parse_macro_call(&mut self) -> Result<Expr, SyntaxError> {
        let name = self.expect_identifier()?;
        let span = self.current_span();
        self.expect(Token::LParen)?;
        let mut args = Vec::new();
        if !matches!(self.current_token(), Some(Ok(Token::RParen))) {
            loop {
                args.push(self.parse_expression()?);
                if let Some(Ok(Token::Comma)) = self.current_token() {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        self.expect(Token::RParen)?;

        // Optional trailing block: $!name(args) { ... }
        let block = if let Some(Ok(Token::LBrace)) = self.current_token() {
            self.advance();
            let saved = self.in_quote_block;
            self.in_quote_block = false;
            let stmts = self.parse_body()?;
            self.in_quote_block = saved;
            self.expect(Token::RBrace)?;
            Some(crate::ast::Block { statements: stmts, trailing_expr: None })
        } else {
            None
        };

        Ok(Expr::MacroCall { name, args, block, span })
    }

    fn parse_quote_block(&mut self) -> Result<Expr, SyntaxError> {
        // Set in_quote_block so @ident / @{expr} produce interpolation markers
        let saved = self.in_quote_block;
        self.in_quote_block = true;

        self.expect(Token::LBrace)?;
        let statements = self.parse_body()?;
        // Check for trailing expression (the last statement might be an expression)
        let trailing_expr: Option<Box<Expr>> = if self.in_quote_block {
            // Inside quote block, we only parse statements
            // The body parsing already handles everything
            None
        } else {
            None
        };
        self.expect(Token::RBrace)?;

        self.in_quote_block = saved;

        Ok(Expr::QuoteBlock {
            statements,
            trailing_expr: None,
        })
    }

    fn parse_match_expr(&mut self) -> Result<Expr, SyntaxError> {
        // Parse the scrutinee — explicitly without the struct-literal lookahead
        // that parse_primary() does for `Ident { ... }`.
        let mut value = match self.current_token() {
            Some(Ok(Token::Integer(n))) => { let n = *n; self.advance(); Expr::Literal(Box::new(LiteralExpr::Integer(n))) }
            Some(Ok(Token::IntegerI8(n))) => { let n = *n; self.advance(); Expr::IntegerSuffixed(n, Type::Custom("Int8".to_string())) }
            Some(Ok(Token::IntegerI16(n))) => { let n = *n; self.advance(); Expr::IntegerSuffixed(n, Type::Custom("Int16".to_string())) }
            Some(Ok(Token::IntegerI32(n))) => { let n = *n; self.advance(); Expr::IntegerSuffixed(n, Type::Custom("Int32".to_string())) }
            Some(Ok(Token::IntegerI64(n))) => { let n = *n; self.advance(); Expr::IntegerSuffixed(n, Type::Custom("Int".to_string())) }
            Some(Ok(Token::IntegerU8(n))) => { let n = *n; self.advance(); Expr::IntegerSuffixed(n, Type::Custom("UInt8".to_string())) }
            Some(Ok(Token::IntegerU16(n))) => { let n = *n; self.advance(); Expr::IntegerSuffixed(n, Type::Custom("UInt16".to_string())) }
            Some(Ok(Token::IntegerU32(n))) => { let n = *n; self.advance(); Expr::IntegerSuffixed(n, Type::Custom("UInt32".to_string())) }
            Some(Ok(Token::IntegerU64(n))) => { let n = *n; self.advance(); Expr::IntegerSuffixed(n, Type::Custom("UInt".to_string())) }
            Some(Ok(Token::Float(f))) => { let f = *f; self.advance(); Expr::Literal(Box::new(LiteralExpr::Float(f))) }
            Some(Ok(Token::Float32(f))) => { let f = *f; self.advance(); Expr::Literal(Box::new(LiteralExpr::Float(f))) }
            Some(Ok(Token::Float64(f))) => { let f = *f; self.advance(); Expr::Float64(f) }
            Some(Ok(Token::String(s))) => { let s = s.clone(); self.advance(); Expr::Literal(Box::new(LiteralExpr::String(s))) }
            Some(Ok(Token::Char(c))) => { let c = *c; self.advance(); Expr::Literal(Box::new(LiteralExpr::Char(c))) }
            Some(Ok(Token::BoolTrue)) => { self.advance(); Expr::Literal(Box::new(LiteralExpr::Bool(true))) }
            Some(Ok(Token::BoolFalse)) => { self.advance(); Expr::Literal(Box::new(LiteralExpr::Bool(false))) }
            Some(Ok(Token::Term)) => { self.advance(); Expr::Literal(Box::new(LiteralExpr::Term)) }
            Some(Ok(Token::Underscore)) => { self.advance(); Expr::Identifier("_".to_string()) }
            Some(Ok(Token::Match)) => {
                self.advance();
                return self.parse_match_expr();
            }
            Some(Ok(Token::LParen)) => {
                self.advance();
                let mut elements = Vec::new();
                if let Some(Ok(Token::RParen)) = self.current_token() {
                    self.advance();
                    return Err(SyntaxError::UnexpectedToken {
                        expected: "expression".to_string(),
                        found: "empty tuple in match scrutinee".to_string(),
                        span: self.current_span().unwrap_or_else(Span::dummy),
                    });
                }
                loop {
                    elements.push(self.parse_expression()?);
                    if let Some(Ok(Token::Comma)) = self.current_token() {
                        self.advance();
                    } else {
                        self.expect(Token::RParen)?;
                        break;
                    }
                }
                if elements.len() == 1 {
                    elements.remove(0)
                } else {
                    Expr::Tuple(elements)
                }
            }
            _ => {
                // Must be an identifier or known keyword-as-identifier
                let name = self.expect_identifier()?;
                Expr::Identifier(name)
            }
        };
        // Allow field access on the scrutinee: match obj.field { ... }
        while let Some(Ok(Token::Dot)) = self.current_token() {
            self.advance();
            let member = self.expect_identifier()?;
            value = Expr::FieldAccess(Box::new(value), member);
        }
        self.expect(Token::LBrace)?;

        let mut arms = Vec::new();
        loop {
            // Check for closing brace
            if let Some(Ok(Token::RBrace)) = self.current_token() {
                self.advance();
                break;
            }
            // Parse pattern
            let pattern = if let Some(Ok(Token::Underscore)) = self.current_token() {
                self.advance();
                MatchPattern::Wildcard
            } else if matches!(self.current_token(), Some(Ok(Token::String(_)))) {
                let s = match self.current_token() { Some(Ok(Token::String(s))) => s.clone(), _ => unreachable!() };
                self.advance();
                MatchPattern::Literal(Pattern::LitString(s))
            } else if matches!(self.current_token(), Some(Ok(Token::Integer(_)))) {
                let val = match self.current_token() { Some(Ok(Token::Integer(n))) => *n, _ => unreachable!() };
                self.advance();
                MatchPattern::Literal(Pattern::LitInt(val))
            } else if matches!(self.current_token(), Some(Ok(Token::Float(_)))) {
                let val = match self.current_token() { Some(Ok(Token::Float(f))) => *f, _ => unreachable!() };
                self.advance();
                MatchPattern::Literal(Pattern::LitFloat(val))
            } else if let Some(Ok(Token::BoolTrue)) = self.current_token() {
                self.advance();
                MatchPattern::Literal(Pattern::LitBool(true))
            } else if let Some(Ok(Token::BoolFalse)) = self.current_token() {
                self.advance();
                MatchPattern::Literal(Pattern::LitBool(false))
            } else if matches!(self.current_token(), Some(Ok(Token::Char(_)))) {
                let val = match self.current_token() { Some(Ok(Token::Char(c))) => *c, _ => unreachable!() };
                self.advance();
                MatchPattern::Literal(Pattern::LitChar(val))
            } else {
                let pattern_name = self.expect_identifier()?;
                // Check for variant fields: Variant(f1, f2, ...)
                let fields = if let Some(Ok(Token::LParen)) = self.current_token() {
                    self.advance();
                    let f = self.parse_pattern_fields()?;
                    self.expect(Token::RParen)?;
                    f
                } else {
                    Vec::new()
                };
                MatchPattern::Variant { name: pattern_name, fields }
            };

            // Parse -> (consistent with uni pattern -> expr syntax)
            self.expect(Token::Arrow)?;

            // Parse body: expression or block
            let body = if let Some(Ok(Token::LBrace)) = self.current_token() {
                self.advance();
                let mut stmts = Vec::new();
                while let Some(Ok(token)) = self.current_token() {
                    if matches!(token, Token::RBrace) { break; }
                    stmts.push(self.parse_statement()?);
                }
                self.expect(Token::RBrace)?;
                Box::new(Expr::Block(stmts, Box::new(Expr::Term)))
            } else {
                Box::new(self.parse_expression()?)
            };

            // Parse optional comma after arm
            if let Some(Ok(Token::Comma)) = self.current_token() {
                self.advance();
            }

            arms.push(MatchArm { pattern, guard: None, body });
        }

        Ok(Expr::Match {
            value: Box::new(value),
            arms,
        })
    }

    fn parse_bracket_contents(&mut self) -> Result<BracketContents, SyntaxError> {
        if let Some(Ok(Token::RBracket)) = self.current_token() {
            return Ok(BracketContents::Empty);
        }

        let mut start: Option<Box<Expr>> = None;
        let mut end: Option<Box<Expr>> = None;
        let mut stride: Option<Box<Expr>> = None;
        let mut mask: Option<Box<Expr>> = None;

        let mut current_element = BracketElement::Start;

        loop {
            match current_element {
                BracketElement::Start => {
                    if let Some(Ok(Token::DotDot)) = self.current_token() {
                        self.advance();
                        current_element = BracketElement::End;
                    } else if let Some(Ok(Token::ColonColon)) = self.current_token() {
                        self.advance();
                        current_element = BracketElement::Stride;
                    } else if let Some(Ok(Token::Semicolon)) = self.current_token() {
                        self.advance();
                        current_element = BracketElement::Mask;
                    } else if let Some(Ok(Token::RBracket)) = self.current_token() {
                        break;
                    } else {
                        start = Some(Box::new(self.parse_expression()?));
                        current_element = BracketElement::AfterStart;
                    }
                }
                BracketElement::AfterStart => {
                    if let Some(Ok(Token::DotDot)) = self.current_token() {
                        self.advance();
                        current_element = BracketElement::End;
                    } else if let Some(Ok(Token::ColonColon)) = self.current_token() {
                        self.advance();
                        current_element = BracketElement::Stride;
                    } else if let Some(Ok(Token::Semicolon)) = self.current_token() {
                        self.advance();
                        current_element = BracketElement::Mask;
                    } else if let Some(Ok(Token::RBracket)) = self.current_token() {
                        break;
                    } else {
                        return Err(SyntaxError::UnexpectedToken { expected: ".., ::, ;, or ]".to_string(), found: "".to_string(), span: self.current_span().unwrap_or_else(Span::dummy) });
                    }
                }
                BracketElement::End => {
                    if let Some(Ok(Token::RBracket)) = self.current_token() {
                        current_element = BracketElement::AfterEnd;
                    } else if let Some(Ok(Token::ColonColon)) = self.current_token() {
                        self.advance();
                        current_element = BracketElement::Stride;
                    } else if let Some(Ok(Token::Semicolon)) = self.current_token() {
                        self.advance();
                        current_element = BracketElement::Mask;
                    } else {
                        end = Some(Box::new(self.parse_expression()?));
                        current_element = BracketElement::AfterEnd;
                    }
                }
                BracketElement::AfterEnd => {
                    if let Some(Ok(Token::ColonColon)) = self.current_token() {
                        self.advance();
                        current_element = BracketElement::Stride;
                    } else if let Some(Ok(Token::Semicolon)) = self.current_token() {
                        self.advance();
                        current_element = BracketElement::Mask;
                    } else if let Some(Ok(Token::RBracket)) = self.current_token() {
                        break;
                    } else {
                        return Err(SyntaxError::UnexpectedToken { expected: "::, ;, or ]".to_string(), found: "".to_string(), span: self.current_span().unwrap_or_else(Span::dummy) });
                    }
                }
                BracketElement::Stride => {
                    stride = Some(Box::new(self.parse_expression()?));
                    current_element = BracketElement::AfterStride;
                }
                BracketElement::AfterStride => {
                    if let Some(Ok(Token::Semicolon)) = self.current_token() {
                        self.advance();
                        current_element = BracketElement::Mask;
                    } else if let Some(Ok(Token::RBracket)) = self.current_token() {
                        break;
                    } else {
                        return Err(SyntaxError::UnexpectedToken { expected: "; or ]".to_string(), found: "".to_string(), span: self.current_span().unwrap_or_else(Span::dummy) });
                    }
                }
                BracketElement::Mask => {
                    mask = Some(Box::new(self.parse_expression()?));
                    current_element = BracketElement::AfterMask;
                }
                BracketElement::AfterMask => {
                    if let Some(Ok(Token::RBracket)) = self.current_token() {
                        break;
                    } else {
                        return Err(SyntaxError::UnexpectedToken { expected: "]".to_string(), found: "".to_string(), span: self.current_span().unwrap_or_else(Span::dummy) });
                    }
                }
            }
        }

        self.expect(Token::RBracket)?;

        if start.is_none() && end.is_none() && stride.is_none() && mask.is_none() {
            Ok(BracketContents::Empty)
        } else if stride.is_none() && mask.is_none() && end.is_none() && start.is_some() {
            Ok(BracketContents::SimpleIndex(start.unwrap()))
        } else {
            Ok(BracketContents::Slice {
                start,
                end,
                stride,
                mask,
            })
        }
    }

    fn bracket_contents_to_expr(&self, base: Expr, contents: BracketContents) -> Expr {
        match contents {
            BracketContents::Empty => Expr::ListIndex(Box::new(base), Box::new(Expr::Integer(0))),
            BracketContents::SimpleIndex(idx) => Expr::ListIndex(Box::new(base), idx),
            BracketContents::Slice { start, end, stride, mask } => Expr::Slice {
                value: Box::new(base),
                start,
                end,
                stride,
                mask,
            },
        }
    }

    /// Extract dimension size from a Type for Vector parsing
    // 2026-07-08: Phase 2b — extract base name from both Custom and keyword types
    // Returns the canonical string name of a type for generic application resolution.
    // Handles keyword types (Int, UInt, Float, Bool, etc.) in addition to Custom types.
    // This enables `Int<8>` parses as Applied("Int", [Width(8)]) → Bits.
    fn type_to_base_name(ty: &Type) -> Option<String> {
        let name = match ty {
            Type::Custom(name) => name.clone(),
            Type::Custom(__t) if __t == "Int" => "Int".into(),
            Type::Custom(__t) if __t == "UInt" => "UInt".into(),
            Type::Custom(__t) if __t == "Float" => "Float".into(),
            Type::Custom(__t) if __t == "Bool" => "Bool".into(),
            Type::Custom(__t) if __t == "String" => "String".into(),
            Type::Custom(__t) if __t == "Char" => "Char".into(),
            Type::Custom(__t) if __t == "Data" => "Data".into(),
            Type::Void => "void".into(),
            Type::Custom(__t) if __t == "Int8" => "Int8".into(),
            Type::Custom(__t) if __t == "Int16" => "Int16".into(),
            Type::Custom(__t) if __t == "Int32" => "Int32".into(),
            Type::Custom(__t) if __t == "UInt8" => "UInt8".into(),
            Type::Custom(__t) if __t == "UInt16" => "UInt16".into(),
            Type::Custom(__t) if __t == "UInt32" => "UInt32".into(),
            Type::Custom(__t) if __t == "Float64" => "Float64".into(),
            _ => return None,
        };
        Some(name)
    }

    // 2026-07-08: Phase 2b — resolve well-known type + Width(N) to Bits
    // Enables `Int<8>`, `UInt<16>`, `Float<32>`, `Bits<64>` etc.
    // 2026-07-08: Phase 2A — resolve_bits_type returns Type::Bits(width).
    // Interpretation is removed — semantics live in the TypeUniverse.
    // Returns Some(Type::Bits) if base_name + type_args forms a known Bits type.
    // Returns None for non-Bits types (HashMap, List, Ptr, etc.) or invalid widths.
    // Only handles single Width(N) arguments — generic types like HashMap<Int, String>
    // fall through to the standard Applied type path.
    fn resolve_bits_type(base_name: &str, type_args: &[Type]) -> Option<Type> {
        let width = match type_args {
            [Type::Width(n)] => *n,
            _ => return None,
        };
        match base_name {
            "Int" | "i8" | "i16" | "i32" | "i64"
            | "UInt" | "u8" | "u16" | "u32" | "u64"
            | "Float" | "f32" | "f64" | "Float64" | "Double"
            | "Bool" | "Char" | "Bits" => Some(Type::Bits(width)),
            _ => None,
        }
    }

    // 2026-07-08: Phase 2b — parse integer token as Type::Width(n) inside generic type arguments
    // This is called from the generic <...> argument loop when the current token is an integer.
    // Converts `Int<8>` to use Width(8) as the type argument, which resolve_bits_type then
    // converts to Type::Bits.
    fn parse_type_arg(&mut self) -> Result<Type, SyntaxError> {
        if let Some(Ok(Token::Integer(n))) = self.current_token() {
            let n = *n as u64;
            self.advance();
            return Ok(Type::Width(n));
        }
        self.parse_type()
    }

    // 2026-07-08: Phase 2b — extract vector dimensions from type args after the element type
    // Converts Width(n) and integer-parsable Custom types into Dimension::Anonymous.
    // Returns an error if any arg after the first is not a valid dimension.
    fn parse_vector_dimensions(type_args: &[Type]) -> Result<Vec<crate::ast::Dimension>, String> {
        let mut dimensions = Vec::new();
        for arg in &type_args[1..] {
            let size = Self::parse_dimension_size(arg)?;
            dimensions.push(crate::ast::Dimension::Anonymous(size));
        }
        Ok(dimensions)
    }

    // 2026-07-08: Phase 2b — extract usize from a dimension type arg
    fn parse_dimension_size(ty: &Type) -> Result<usize, String> {
        match ty {
            Type::Width(n) => Ok(*n as usize),
            Type::Custom(s) => s.parse::<usize>().map_err(|_| format!("Invalid vector dimension: {}", s)),
            _ => Err("Vector dimension must be an integer".to_string()),
        }
    }

    // 2026-07-08: Phase 2b — handle Width(n) from integer token type args
    fn extract_dimension_size(ty: &Type) -> Option<usize> {
        match ty {
            Type::Custom(s) => s.parse::<usize>().ok(),
            Type::Width(n) => Some(*n as usize),
            _ => None,
        }
    }

    /// Peek ahead to check if this is a multidimensional slice (has commas before `;`, `..`, `::`, or `]`)
    fn peek_multidimensional_slice(&self) -> bool {
        // Save current position and scan ahead
        let mut pos = self.pos;
        let input = &self.source;
        let bytes = input.as_bytes();
        
        // Skip whitespace
        while pos < bytes.len() && (bytes[pos] == b' ' || bytes[pos] == b'\t' || bytes[pos] == b'\n' || bytes[pos] == b'\r') {
            pos += 1;
        }
        
        if pos >= bytes.len() {
            return false;
        }
        
        // If starts with `]`, it's not multidimensional
        if bytes[pos] == b']' {
            return false;
        }
        if pos + 1 < bytes.len() && bytes[pos] == b'.' && bytes[pos + 1] == b'.' {
            // `..` is a range inside a single slice; `...` (ellipsis) is multi
            if pos + 2 < bytes.len() && bytes[pos + 2] == b'.' {
                return true; // `...` ellipsis
            }
            return false; // `..` range
        }
        if pos + 1 < bytes.len() && bytes[pos] == b':' && bytes[pos + 1] == b':' {
            return true; // `::` stride is a bracket op
        }
        
        // Scan until we find a comma, semicolon, or closing bracket
        let mut found_colon = false;
        while pos < bytes.len() {
            match bytes[pos] {
                b',' => return true,  // Found comma = multidimensional
                b';' => return true,  // Found semicolon = mask bracket op
                b']' => return false,  // Closing bracket = single dimension
                b'.' => {
                    // `...` (ellipsis) — treat as multidimensional
                    if pos + 2 < bytes.len() && bytes[pos] == b'.' && bytes[pos + 1] == b'.' && bytes[pos + 2] == b'.' {
                        return true;
                    }
                    if pos + 1 < bytes.len() && bytes[pos + 1] == b'.' {
                        return false;  // Found ..
                    }
                }
                b'@' => {
                    // `@N:` — @ dimension specifier — treat as multidimensional
                    if pos + 1 < bytes.len() && bytes[pos + 1].is_ascii_digit() {
                        return true;
                    }
                }
                b':' => {
                    if found_colon {
                        // :: means stride = bracket op
                        return true;
                    }
                    found_colon = true;
                }
                _ => {}
            }
            pos += 1;
        }
        false
    }

    /// Parse a multidimensional slice: vec[coord1, coord2, ... ; mask :: stride]
    fn parse_multi_slice(&mut self) -> Result<MultiSliceResult, SyntaxError> {
        let mut ops: Vec<crate::ast::BracketOp> = Vec::new();

        loop {
            if let Some(Ok(Token::RBracket)) = self.current_token() {
                break;
            }
            if let Some(Ok(Token::Semicolon)) = self.current_token() {
                // Filter/mask: ; cond
                self.advance();
                let expr = self.parse_expression()?;
                ops.push(crate::ast::BracketOp::Mask(Box::new(expr)));
            } else if let Some(Ok(Token::ColonColon)) = self.current_token() {
                // Stride: ::N
                self.advance();
                let expr = self.parse_expression()?;
                ops.push(crate::ast::BracketOp::Stride(Box::new(expr)));
            } else {
                // Coordinate: index, range, named, @dim, ...
                // Comma before next coordinate is handled inside each iteration:
                // only coordinates are comma-separated; mask/stride have their own prefixes.
                let coord = self.parse_slice_coordinate()?;
                ops.push(crate::ast::BracketOp::Coord(coord));
                if let Some(Ok(Token::Comma)) = self.current_token() {
                    self.advance();
                    // more coordinates follow
                }
                // Continue loop — next may be another coord, ;, ::, or ]
            }
        }

        self.expect(Token::RBracket)?;

        Ok(MultiSliceResult { ops })
    }

    /// Parse a single slice coordinate: index, range, named, ellipsis, or @dim
    fn parse_slice_coordinate(&mut self) -> Result<crate::ast::SliceCoordinate, SyntaxError> {
        // Ellipsis: `...`
        if let Some(Ok(Token::Ellipsis)) = self.current_token() {
            self.advance();
            return Ok(crate::ast::SliceCoordinate::Ellipsis);
        }
        // @dim: coordinate — `@3:0..10`
        if let Some(Ok(Token::At)) = self.current_token() {
            // peek at the next token (self.peek) for an integer
            if let Some(Ok(Token::Integer(n))) = self.peek.as_ref().map(|(t, _)| t) {
                let dim = *n as usize;
                self.advance(); // consume @
                self.advance(); // consume integer
                // expect colon
                if matches!(self.current_token(), Some(Ok(Token::Colon))) {
                    self.advance(); // consume colon
                    let coord = self.parse_slice_coordinate_inner()?;
                    return Ok(crate::ast::SliceCoordinate::AtDimension {
                        dimension: dim,
                        coord: Box::new(coord),
                    });
                }
                return self.spanned_err("Expected ':' after @N in dimension specifier".to_string());
            }
            // Not @dim — fall through to expression parsing below
        }
        // Check for named dimension: identifier:coord
        if let Some(Ok(Token::Identifier(_))) = self.current_token() {
            // Save the identifier name before advancing
            let name = match self.current_token() {
                Some(Ok(Token::Identifier(n))) => n.clone(),
                _ => unreachable!(),
            };
            self.advance();
            
            if let Some(Ok(Token::Colon)) = self.current_token() {
                self.advance(); // consume colon
                // Parse the coordinate part after the name
                let coord = self.parse_slice_coordinate_inner()?;
                return Ok(crate::ast::SliceCoordinate::Named {
                    name,
                    coord: Box::new(coord),
                });
            } else {
                // Not a named dimension, treat as identifier expression
                let expr = Expr::Identifier(name);
                return Ok(crate::ast::SliceCoordinate::Index(Box::new(expr)));
            }
        }
        
        self.parse_slice_coordinate_inner()
    }

    /// Parse the inner part of a coordinate (index or range)
    fn parse_slice_coordinate_inner(&mut self) -> Result<crate::ast::SliceCoordinate, SyntaxError> {
        if let Some(Ok(Token::DotDot)) = self.current_token() {
            // ..end range
            self.advance();
            let end = Some(Box::new(self.parse_expression()?));
            Ok(crate::ast::SliceCoordinate::Range { start: None, end })
        } else {
            let start = self.parse_expression()?;
            if let Some(Ok(Token::DotDot)) = self.current_token() {
                // start.. or start..end range
                self.advance();
                let end = if let Some(Ok(Token::Comma)) | Some(Ok(Token::Semicolon)) | Some(Ok(Token::RBracket)) = self.current_token() {
                    None // start.. (open-ended)
                } else {
                    Some(Box::new(self.parse_expression()?))
                };
                Ok(crate::ast::SliceCoordinate::Range { start: Some(Box::new(start)), end })
            } else {
                // Single index
                Ok(crate::ast::SliceCoordinate::Index(Box::new(start)))
            }
        }
    }

    fn keyword_token_to_name(token: &Token) -> Option<&'static str> {
        match token {
            Token::Sig => Some("sig"),
            Token::Defn => Some("defn"),
            Token::Let => Some("let"),
            Token::Txn => Some("txn"),
            Token::Rct => Some("rct"),
            Token::Frgn => Some("frgn"),
            Token::Struct => Some("struct"),
            Token::Enum => Some("enum"),
            Token::Import => Some("import"),
            Token::Term => Some("term"),
            Token::Const => Some("const"),
            Token::BoolTrue => Some("true"),
            Token::BoolFalse => Some("false"),
            Token::Uni => Some("uni"),
            Token::Escape => Some("escape"),
            Token::Async => Some("async"),
            Token::Await => Some("await"),
            Token::Is => Some("is"),
            Token::Like => Some("like"),
            Token::Some => Some("Some"),
            Token::None => Some("None"),
            Token::Ok => Some("Ok"),
            Token::Err => Some("Err"),
            _ => None,
        }
    }

    /// Check if a statement tree contains `term;` or `escape;` anywhere (including guarded blocks).
    fn has_term_or_escape_in_tree(stmts: &[Statement]) -> bool {
        for s in stmts {
            match s {
                Statement::Term { .. } | Statement::TermBang { .. } | Statement::Escape(_) => return true,
                Statement::Guarded { statements, .. } => {
                    if Self::has_term_or_escape_in_tree(statements) { return true; }
                }
                _ => {}
            }
        }
        false
    }

    /// Check if a contract pair is structurally convergent (post ⇒ ¬pre).
    /// True when pre = `var < bound` and post = `var == bound`, or similar pairings.
    /// Simplified version of proof_engine::check_convergence — the full proof runs
    /// later during verification; this is just a parse-time structural check.
    fn is_convergent_contract_pair(pre: &Expr, post: &Expr) -> bool {
        use crate::features::binary_op::BinaryOpKind::*;
        // Pattern: post is a comparison wrapped in BinaryOp, pre is also a comparison.
        let is_comparison = |e: &Expr| -> bool {
            matches!(e, Expr::BinaryOp(bop) if matches!(bop.kind, Eq | Ne | Lt | Le | Gt | Ge | And))
        };
        is_comparison(post) && is_comparison(pre)
    }

    fn parse_keyword_as_expr(&mut self, name: &str) -> Result<Expr, SyntaxError> {
        self.advance();
        if let Some(Ok(Token::LParen)) = self.current_token() {
            self.advance();
            let mut args = Vec::new();
            if let Some(Ok(Token::RParen)) = self.current_token() {
                // Empty args
            } else {
                loop {
                    args.push(self.parse_expression()?);
                    if let Some(Ok(Token::Comma)) = self.current_token() {
                        self.advance();
                    } else {
                        break;
                    }
                }
            }
            self.expect(Token::RParen)?;
            Ok(Expr::Call(name.to_string(), args))
        } else {
            Ok(Expr::Identifier(name.to_string()))
        }
    }
}

struct MultiSliceResult {
    ops: Vec<crate::ast::BracketOp>,
}

#[cfg(test)]
mod parser_tests {
    use super::*;
    use crate::ast::LayoutConstraint;
    use crate::ast::TypeSlot;

    // ── Ptr/PtrN LayoutPtr parsing tests ──────────────────────────

    #[test]
    fn test_parse_ptr_bare_as_layout_ptr() {
        let src = "let x: Ptr = 0;";
        let mut parser = Parser::new(src);
        let prog = parser.parse().unwrap();
        if let TopLevel::StateDecl(s) = &prog.items[0] {
            assert_eq!(s.ty, Type::LayoutPtr(LayoutConstraint { bytes: 8, alignment: 8 }));
        } else {
            panic!("Expected StateDecl, got {:?}", prog.items[0]);
        }
    }

    #[test]
    fn test_parse_ptr64_as_layout_ptr() {
        let src = "let x: Ptr64 = 0;";
        let mut parser = Parser::new(src);
        let prog = parser.parse().unwrap();
        if let TopLevel::StateDecl(s) = &prog.items[0] {
            assert_eq!(s.ty, Type::LayoutPtr(LayoutConstraint { bytes: 8, alignment: 8 }));
        } else {
            panic!("Expected StateDecl, got {:?}", prog.items[0]);
        }
    }

    #[test]
    fn test_parse_ptr32_as_layout_ptr() {
        let src = "let x: Ptr32 = 0;";
        let mut parser = Parser::new(src);
        let prog = parser.parse().unwrap();
        if let TopLevel::StateDecl(s) = &prog.items[0] {
            assert_eq!(s.ty, Type::LayoutPtr(LayoutConstraint { bytes: 4, alignment: 4 }));
        } else {
            panic!("Expected StateDecl, got {:?}", prog.items[0]);
        }
    }

    #[test]
    fn test_parse_ptr8_as_layout_ptr() {
        let src = "let x: Ptr8 = 0;";
        let mut parser = Parser::new(src);
        let prog = parser.parse().unwrap();
        if let TopLevel::StateDecl(s) = &prog.items[0] {
            assert_eq!(s.ty, Type::LayoutPtr(LayoutConstraint { bytes: 1, alignment: 1 }));
        } else {
            panic!("Expected StateDecl, got {:?}", prog.items[0]);
        }
    }

    #[test]
    fn test_parse_ptr_typed_stays_applied() {
        let src = "let x: Ptr<Int> = 0;";
        let mut parser = Parser::new(src);
        let prog = parser.parse().unwrap();
        if let TopLevel::StateDecl(s) = &prog.items[0] {
            assert!(matches!(&s.ty, Type::Applied(n, _) if n == "Ptr"),
                "Expected Ptr<Int>, got {:?}", s.ty);
        } else {
            panic!("Expected StateDecl, got {:?}", prog.items[0]);
        }
    }

    #[test]
    fn test_parse_ptr128_as_layout_ptr() {
        let src = "let x: Ptr128 = 0;";
        let mut parser = Parser::new(src);
        let prog = parser.parse().unwrap();
        if let TopLevel::StateDecl(s) = &prog.items[0] {
            assert_eq!(s.ty, Type::LayoutPtr(LayoutConstraint { bytes: 16, alignment: 16 }));
        } else {
            panic!("Expected StateDecl, got {:?}", prog.items[0]);
        }
    }

    // ── Existing tests ─────────────────────────────────────────

    #[test]
    fn test_parse_struct_public_field_default() {
        let src = "struct S { x: Int; };";
        let mut parser = Parser::new(src);
        let prog = parser.parse().unwrap();
        if let TopLevel::Struct(s) = &prog.items[0] {
            assert!(!s.fields.is_empty());
            assert_eq!(s.fields[0].visibility, Visibility::Public);
        } else {
            panic!("Expected Struct");
        }
    }

    #[test]
    fn test_parse_struct_pvt_field() {
        let src = "struct S { pvt x: Int; };";
        let mut parser = Parser::new(src);
        let prog = parser.parse().unwrap();
        if let TopLevel::Struct(s) = &prog.items[0] {
            assert!(!s.fields.is_empty());
            assert_eq!(s.fields[0].visibility, Visibility::Private);
        } else {
            panic!("Expected Struct");
        }
    }

    #[test]
    fn test_parse_struct_sed_field() {
        let src = "struct S { sed x: Int; };";
        let mut parser = Parser::new(src);
        let prog = parser.parse().unwrap();
        if let TopLevel::Struct(s) = &prog.items[0] {
            assert!(!s.fields.is_empty());
            assert_eq!(s.fields[0].visibility, Visibility::Sedentary);
        } else {
            panic!("Expected Struct");
        }
    }

    #[test]
    fn test_parse_trg_binding_simple() {
        let src = "defn main() -> Int {
            trg X: Int @ add_one(41);
            term 0;
        };";
        let mut parser = Parser::new(src);
        let prog = parser.parse().unwrap();
        match &prog.items[0] {
            TopLevel::Definition(d) => {
                assert_eq!(d.body.len(), 2);
                match &d.body[0] {
                    Statement::TrgBinding { name, ty, instance, port, .. } => {
                        assert_eq!(name, "X");
                        assert!(ty.is_some());
                        assert_eq!(port, "");
                        if let Expr::Call(callee, args) = instance {
                            assert_eq!(callee, "add_one");
                            assert_eq!(args.len(), 1);
                        } else {
                            panic!("Expected Call, got {:?}", instance);
                        }
                    }
                    other => panic!("Expected TrgBinding, got {:?}", other),
                }
            }
            other => panic!("Expected Definition, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_trg_binding_with_port() {
        let src = "defn main() -> Int {
            trg elapsed: Int @ timer(1000).elapsed;
            term 0;
        };";
        let mut parser = Parser::new(src);
        let prog = parser.parse().unwrap();
        match &prog.items[0] {
            TopLevel::Definition(d) => {
                assert_eq!(d.body.len(), 2);
                match &d.body[0] {
                    Statement::TrgBinding { name, ty, instance, port, .. } => {
                        assert_eq!(name, "elapsed");
                        assert!(ty.is_some());
                        assert_eq!(port, "elapsed");
                        if let Expr::Call(callee, args) = instance {
                            assert_eq!(callee, "timer");
                            assert_eq!(args.len(), 1);
                        } else {
                            panic!("Expected Call, got {:?}", instance);
                        }
                    }
                    other => panic!("Expected TrgBinding, got {:?}", other),
                }
            }
            other => panic!("Expected Definition, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_cell_auto_terminating() {
        let src = "cell timer(duration: Int) -> elapsed: Int, done: Bool {
            elapsed: Int = 0;
            done: Bool = false;

            rct txn tick [elapsed < duration]] {
                &elapsed = elapsed + 1;
                term;
            };

            rct txn finish [elapsed >= duration && !done]] {
                &done = true;
                term!;
            };
        };";
        let mut parser = Parser::new(src);
        let prog = parser.parse().unwrap();
        match &prog.items[0] {
            TopLevel::Cell(cell) => {
                assert!(!cell.is_persistent);
                assert_eq!(cell.name, "timer");
                assert_eq!(cell.parameters.len(), 1);
                assert_eq!(cell.parameters[0].0, "duration");
                assert_eq!(cell.fields.len(), 2);
                assert_eq!(cell.fields[0].name, "elapsed");
                assert_eq!(cell.fields[1].name, "done");
                assert_eq!(cell.transactions.len(), 2);
                assert_eq!(cell.transactions[0].name, "tick");
                assert_eq!(cell.transactions[1].name, "finish");
            }
            other => panic!("Expected TopLevel::Cell, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_cell_persistent() {
        let src = "cell! console(path: String) -> buffer: String {
            accumulated: String = \"\";

            rct txn read [path != \"\"]] {
                &accumulated = accumulated + path;
                term;
            };
        };";
        let mut parser = Parser::new(src);
        let prog = parser.parse().unwrap();
        match &prog.items[0] {
            TopLevel::Cell(cell) => {
                assert!(cell.is_persistent);
                assert_eq!(cell.name, "console");
                assert_eq!(cell.parameters.len(), 1);
                assert_eq!(cell.parameters[0].0, "path");
                assert_eq!(cell.fields.len(), 1);
                assert_eq!(cell.fields[0].name, "accumulated");
                assert_eq!(cell.transactions.len(), 1);
                assert_eq!(cell.transactions[0].name, "read");
            }
            other => panic!("Expected TopLevel::Cell, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_cell_no_outputs() {
        let src = "cell sink(data: Int) {
            buffer: Int = 0;
            rct txn absorb [data != 0]] { &buffer = buffer + data; term; };
        };";
        let mut parser = Parser::new(src);
        let prog = parser.parse().unwrap();
        match &prog.items[0] {
            TopLevel::Cell(cell) => {
                assert!(!cell.is_persistent);
                assert_eq!(cell.name, "sink");
                assert!(cell.output_type.is_none());
            }
            other => panic!("Expected TopLevel::Cell, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_struct_derivation_basic() {
        let src = "struct B <: A { z: Int; };";
        let mut parser = Parser::new(src);
        let prog = parser.parse().unwrap();
        if let TopLevel::Struct(s) = &prog.items[0] {
            assert_eq!(s.name, "B");
            assert!(s.parent.is_some());
            if let Some(Type::Custom(parent)) = &s.parent {
                assert_eq!(parent, "A");
            } else {
                panic!("Expected Custom parent type, got {:?}", s.parent);
            }
            assert_eq!(s.fields.len(), 1);
            assert_eq!(s.fields[0].name, "z");
        } else {
            panic!("Expected Struct");
        }
    }

    #[test]
    fn test_parse_struct_derivation_with_type_params() {
        let src = "struct B <: Container<Int> { z: Float; };";
        let mut parser = Parser::new(src);
        let prog = parser.parse().unwrap();
        if let TopLevel::Struct(s) = &prog.items[0] {
            assert!(s.parent.is_some());
            if let Some(Type::Applied(parent, args)) = &s.parent {
                assert_eq!(parent, "Container");
                assert_eq!(args.len(), 1);
                assert_eq!(args[0], Type::Custom("Int".to_string()));
            } else {
                panic!("Expected Applied parent type, got {:?}", s.parent);
            }
        } else {
            panic!("Expected Struct");
        }
    }

    #[test]
    fn test_parse_struct_no_derivation() {
        let src = "struct A { x: Int; };";
        let mut parser = Parser::new(src);
        let prog = parser.parse().unwrap();
        if let TopLevel::Struct(s) = &prog.items[0] {
            assert_eq!(s.name, "A");
            assert!(s.parent.is_none(), "Struct without <: should have no parent");
        } else {
            panic!("Expected Struct");
        }
    }

    #[test]
    fn test_parse_list_index_after_import() {
        // Regression: peek_multidimensional_slice used self.pos which was
        // always 0, causing ';' from earlier statements to trigger a false
        // positive for "mask bracket op", making items[0] parse as MultiSlice.
        let src = "import \"empty\";\n\ndefn get_first(items: List<String>) -> String {\n    term items[0];\n};";
        let mut parser = Parser::new(src);
        let prog = parser.parse().unwrap();
        assert_eq!(prog.items.len(), 2);
        if let TopLevel::Definition(defn) = &prog.items[1] {
            if let Statement::Term { values, .. } = &defn.body[0] {
                assert!(matches!(&values[0], Some(Expr::ListIndex(..))),
                    "Expected ListIndex for items[0], got {:?}", values[0]);
            } else {
                panic!("Expected Term statement");
            }
        } else {
            panic!("Expected Definition");
        }
    }

    #[test]
    fn test_parse_rstruct_with_self_closing_html() {
        let s = r#"rstruct Logo { <svg> <circle /> </svg> };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse_rstruct();
        assert!(result.is_ok(), "Should parse successfully");
        
        let rstruct_def = result.unwrap();
        assert_eq!(rstruct_def.name, "Logo", "Struct should be named Logo");
        assert!(!rstruct_def.view_html.is_empty(), "Should have SVG content");
    }

    #[test]
    fn test_parse_trigger_with_link() {
        let s = r#"trg signal: Bool @ link my_signal;"#;
        let mut parser = Parser::new(s);
        let result = parser.parse_trigger();
        assert!(result.is_ok(), "Should parse trigger with link");
        
        let trg = result.unwrap();
        assert_eq!(trg.name, "signal", "Trigger should be named signal");
    }

    #[test]
    fn test_parse_trigger_with_explicit_address() {
        let s = r#"trg control: UInt @ 0x8000A000 /0..7;"#;
        let mut parser = Parser::new(s);
        let result = parser.parse_trigger();
        assert!(result.is_ok(), "Should parse trigger with explicit address");
        
        let trg = result.unwrap();
        assert_eq!(trg.name, "control", "Trigger should be named control");
    }

    #[test]
    fn test_parse_minimal_program() {
        let s = r#"let x: Int = 0;"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse minimal program");
        assert_eq!(result.unwrap().items.len(), 1, "Should have one item");
    }

    #[test]
    fn test_parse_inline_asm() {
        let s = r#"txn Foo [true][n >= 0] { asm "mov x0, #0" { "x0" }; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse inline asm");
        let program = result.unwrap();
        assert!(!program.items.is_empty(), "Should have items");
        let item = &program.items[0];
        if let TopLevel::Transaction(txn) = item {
            assert_eq!(txn.body.len(), 1);
            match &txn.body[0] {
                Statement::InlineAsm { asm_string, clobbers, .. } => {
                    assert_eq!(asm_string, "mov x0, #0");
                    assert_eq!(clobbers.len(), 1);
                    assert_eq!(clobbers[0], "x0");
                }
                _ => panic!("Expected InlineAsm statement"),
            }
        } else {
            panic!("Expected Transaction item");
        }
    }

    #[test]
    fn test_parse_inline_asm_no_clobbers() {
        let s = r#"txn Bar [true][n >= 0] { asm "wfi"; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse inline asm without clobbers");
        let program = result.unwrap();
        assert!(!program.items.is_empty(), "Should have items");
        let item = &program.items[0];
        if let TopLevel::Transaction(txn) = item {
            assert_eq!(txn.body.len(), 1);
            match &txn.body[0] {
                Statement::InlineAsm { asm_string, clobbers, .. } => {
                    assert_eq!(asm_string, "wfi");
                    assert!(clobbers.is_empty());
                }
                _ => panic!("Expected InlineAsm statement"),
            }
        } else {
            panic!("Expected Transaction item");
        }
    }

    #[test]
    fn test_parse_shorthand_types() {
        // Test u8 shorthand
        let s = r#"let x: u8 = 0;"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse u8 type");
        if let TopLevel::StateDecl(decl) = &result.unwrap().items[0] {
            assert!(matches!(&decl.ty, Type::Custom(__t) if __t == "UInt8"), "Expected UInt8, got {:?}", decl.ty);
        }

        // Test i16 shorthand
        let s = r#"let y: i16 = 0;"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse i16 type");
        if let TopLevel::StateDecl(decl) = &result.unwrap().items[0] {
            assert!(matches!(&decl.ty, Type::Custom(__t) if __t == "Int16"), "Expected Int16, got {:?}", decl.ty);
        }

        // Test u32 shorthand
        let s = r#"let z: u32 = 0;"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse u32 type");
        if let TopLevel::StateDecl(decl) = &result.unwrap().items[0] {
            assert!(matches!(&decl.ty, Type::Custom(__t) if __t == "UInt32"), "Expected UInt32, got {:?}", decl.ty);
        }

        // Test i64 shorthand
        let s = r#"let w: i64 = 0;"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse i64 type");
        if let TopLevel::StateDecl(decl) = &result.unwrap().items[0] {
            assert!(matches!(&decl.ty, Type::Custom(__t) if __t == "Int"), "Expected Int, got {:?}", decl.ty);
        }
    }

    #[test]
    fn test_parse_angle_bracket_bits_types() {
        // Test Int<8> → Type::Bits(8)
        let s = r#"let x: Int<8> = 0;"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse Int<8>");
        if let TopLevel::StateDecl(decl) = &result.unwrap().items[0] {
            assert!(matches!(&decl.ty, Type::Bits(8)),
                "Expected Bits(8), got {:?}", decl.ty);
        }

        // Test UInt<16> → Type::Bits(16)
        let s = r#"let y: UInt<16> = 0;"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse UInt<16>");
        if let TopLevel::StateDecl(decl) = &result.unwrap().items[0] {
            assert!(matches!(&decl.ty, Type::Bits(16)),
                "Expected Bits(16), got {:?}", decl.ty);
        }

        // Test Float<32> → Type::Bits(32)
        let s = r#"let z: Float<32> = 0.0;"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse Float<32>");
        if let TopLevel::StateDecl(decl) = &result.unwrap().items[0] {
            assert!(matches!(&decl.ty, Type::Bits(32)),
                "Expected Bits(32), got {:?}", decl.ty);
        }

        // Test Float<64> → Type::Bits(64)
        let s = r#"let w: Float<64> = 0.0;"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse Float<64>");
        if let TopLevel::StateDecl(decl) = &result.unwrap().items[0] {
            assert!(matches!(&decl.ty, Type::Bits(64)),
                "Expected Bits(64), got {:?}", decl.ty);
        }

        // Test Bits<64> → Type::Bits(64)
        let s = r#"let d: Bits<64> = 0;"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse Bits<64>");
        if let TopLevel::StateDecl(decl) = &result.unwrap().items[0] {
            assert!(matches!(&decl.ty, Type::Bits(64)),
                "Expected Bits(64), got {:?}", decl.ty);
        }

        // Test HashMap<String, Int> still works unchanged
        let s = r#"let m: HashMap<String, Int> = HashMap.new();"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse HashMap<String, Int>");
        if let TopLevel::StateDecl(decl) = &result.unwrap().items[0] {
            assert!(matches!(&decl.ty, Type::Applied(n, _) if n == "HashMap"),
                "Expected Applied(HashMap, ..), got {:?}", decl.ty);
        }

        // Test List<Int<8>> — nested Bits resolution
        let s = r#"let v: List<Int<8>> = [1, 2, 3];"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse List<Int<8>>");
        if let TopLevel::StateDecl(decl) = &result.unwrap().items[0] {
            assert!(matches!(&decl.ty, Type::Applied(n, args) if n == "List" && args.len() == 1),
                "Expected Applied(List, [Bits<8>]), got {:?}", decl.ty);
        }
    }

    #[test]
    fn test_parse_at_slash_bit_spec() {
        // Test @ /x16 (auto-allocate with bit count)
        let s = r#"let x: Int @ /x16 = 0;"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse @ /x16 syntax: {:?}", result.err());
        if let TopLevel::StateDecl(decl) = &result.unwrap().items[0] {
            // Bit range is now part of the type (Constrained)
            assert!(matches!(&decl.ty, Type::Constrained(_, BitRange::Any(16))));
        }

        // Test @ /3..5 (auto-allocate with bit range)
        let s = r#"let y: UInt @ /3..5 = 0;"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse @ /3..5 syntax: {:?}", result.err());
        if let TopLevel::StateDecl(decl) = &result.unwrap().items[0] {
            assert!(matches!(&decl.ty, Type::Constrained(_, BitRange::Range(3, 5))));
        }

        // Test @ 0x1000/x8 (fixed address with bit count)
        let s = r#"let z: u8 @ 0x1000/x8 = 0;"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse @ 0x1000/x8 syntax: {:?}", result.err());
        if let TopLevel::StateDecl(decl) = &result.unwrap().items[0] {
            assert_eq!(decl.address, Some(0x1000));
        }

        // Test @ /x1 (single bit auto-allocate)
        let s = r#"let flag: Bool @ /x1 = false;"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse @ /x1 syntax: {:?}", result.err());
        if let TopLevel::StateDecl(decl) = &result.unwrap().items[0] {
            assert!(matches!(&decl.ty, Type::Constrained(_, BitRange::Any(1))));
        }
    }


    #[test]
    fn test_parse_top_level_trigger_without_bang() {
        // Test that top-level trg (without !) still works
        let s = r#"const trg button: Bool @ 0x1000;"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Top-level trg without ! should parse: {:?}", result.err());
        if let TopLevel::Trigger(trg) = &result.unwrap().items[0] {
            assert_eq!(trg.name, "button");
            assert!(trg.is_const, "const trg should have is_const = true");
        } else {
            panic!("Expected Trigger item");
        }
    }

    #[test]
    fn test_parse_const_trg_with_stdin() {
        // const trg is optional for software triggers
        let s = r#"const trg keypress: Char @stdin#;"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "const trg with @stdin# should parse: {:?}", result.err());
        if let TopLevel::Trigger(trg) = &result.unwrap().items[0] {
            assert!(trg.is_const, "const trg should have is_const = true");
            assert!(matches!(trg.address, crate::ast::LinkRef::Stdin));
        } else {
            panic!("Expected Trigger item");
        }
    }

    #[test]
    fn test_parse_trg_explicit_address_requires_const() {
        // Explicit address without const must error
        let s = r#"trg led: Bool @ 0x1000;"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_err(), "trg @address without const should error");
        let err = result.err().unwrap();
        let err_msg = format!("{}", err);
        assert!(err_msg.contains("must be declared 'const trg'"),
            "Error should mention const trg, got: {}", err_msg);
    }

    #[test]
    fn test_parse_trg_unbound_no_address() {
        // Unbound trigger (no @) should work without const
        let s = r#"trg flag: Bool;"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Unbound trg should parse without const: {:?}", result.err());
        if let TopLevel::Trigger(trg) = &result.unwrap().items[0] {
            assert!(!trg.is_const, "Unbound trg should have is_const = false");
        } else {
            panic!("Expected Trigger item");
        }
    }

    // DISABLED: alka/on_exit — not ready for use.
    // #[test]
    // fn test_parse_alka_block_safe() {
    //     let s = r#"txn Foo [true][n >= 0] { alka { FENCE GPU_MAIN.METAPAGE == 1; }; term; };"#;
    //     let mut parser = Parser::new(s);
    //     let result = parser.parse();
    //     assert!(result.is_ok(), "Should parse alka block: {:?}", result.err());
    //     if let TopLevel::Transaction(txn) = &result.unwrap().items[0] {
    //         match &txn.body[0] {
    //             Statement::Alka(block) => {
    //                 assert!(!block.dangerous, "Safe alka");
    //                 assert!(block.content.contains("FENCE GPU_MAIN"));
    //             }
    //             _ => panic!("Expected Alka statement"),
    //         }
    //     }
    // }
    // #[test]
    // fn test_parse_alka_block_dangerous() {
    //     let s = r#"txn Foo [true][n >= 0] { alka! { PULSE DOORBELL @ 0x90; }; term; };"#;
    //     let mut parser = Parser::new(s);
    //     let result = parser.parse();
    //     assert!(result.is_ok(), "Should parse alka! block: {:?}", result.err());
    //     if let TopLevel::Transaction(txn) = &result.unwrap().items[0] {
    //         match &txn.body[0] {
    //             Statement::Alka(block) => {
    //                 assert!(block.dangerous, "Dangerous alka");
    //                 assert!(block.content.contains("PULSE DOORBELL"));
    //             }
    //             _ => panic!("Expected Alka statement"),
    //         }
    //     }
    // }
    // #[test]
    // fn test_parse_alka_multi_line() {
    //     let s = "txn Foo [true][n >= 0] { alka {\n  FENCE GPU_MAIN.METAPAGE == 1;\n  SIGNAL EXPERT_READY;\n}; term; };";
    //     let mut parser = Parser::new(s);
    //     let result = parser.parse();
    //     assert!(result.is_ok(), "Should parse multi-line alka: {:?}", result.err());
    // }

    #[test]
    fn test_parse_hashtag_on_let() {
        let s = r#"txn Foo [true][n >= 0] { let x: Int #volatile; term; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse hashtag on let: {:?}", result.err());
    }

    #[test]
    fn test_parse_hashtag_on_assignment() {
        let s = r#"txn Foo [true][n >= 0] { &x = 1 #!sfence; term; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse mandatory hashtag on assignment: {:?}", result.err());
    }

    #[test]
    fn test_parse_hashtag_on_term() {
        let s = r#"txn Foo [true][n >= 0] { &x = 1; term #retry; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse hashtag on term: {:?}", result.err());
    }

    #[test]
    fn test_parse_fallback_chain() {
        let s = r#"txn Foo [true][n >= 0] { &x = 1 #!sfence|lfence|mfence; term; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse fallback chain: {:?}", result.err());
        if let TopLevel::Transaction(txn) = &result.unwrap().items[0] {
            match &txn.body[0] {
                Statement::Assignment { modifiers, .. } => {
                    assert_eq!(modifiers.len(), 1);
                    assert!(modifiers[0].mandatory());
                }
                _ => panic!("Expected Assignment"),
            }
        }
    }

    #[test]
    fn test_parse_scoped_hashtag() {
        let s = r#"txn Foo [true][n >= 0] { let x: Int #[cpp]#volatile; term; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse scoped hashtag: {:?}", result.err());
    }

    #[test]
    fn test_parse_dynamic_address() {
        let s = r#"txn Foo [true][n >= 0] { let x: Int @ some_ptr #volatile = 0; term; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse dynamic @ address: {:?}", result.err());
        if let TopLevel::Transaction(txn) = &result.unwrap().items[0] {
            match &txn.body[0] {
                Statement::Let { address_expr, address, .. } => {
                    assert!(address_expr.is_some(), "Should have dynamic address");
                    assert!(address.is_none(), "Should NOT have static address");
                }
                _ => panic!("Expected Let statement"),
            }
        }
    }

    #[test]
    fn test_parse_hashtag_with_value() {
        let s = r#"txn Foo [true][n >= 0] { let buf: Byte[4096] #!aligned(4096); term; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse hashtag with value: {:?}", result.err());
        if let TopLevel::Transaction(txn) = &result.unwrap().items[0] {
            match &txn.body[0] {
                Statement::Let { modifiers, .. } => {
                    assert_eq!(modifiers.len(), 1);
                    assert_eq!(modifiers[0].name, "aligned");
                    assert_eq!(modifiers[0].string_value(), Some("4096".to_string()));
                }
                _ => panic!("Expected Let"),
            }
        }
    }

    #[test]
    fn test_parse_speculative_hashtag() {
        let s = r#"txn Foo [true][n >= 0] { &x = 1 #?inline; term; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse speculative hashtag: {:?}", result.err());
        if let TopLevel::Transaction(txn) = &result.unwrap().items[0] {
            match &txn.body[0] {
                Statement::Assignment { modifiers, .. } => {
                    assert_eq!(modifiers.len(), 1);
                    assert!(modifiers[0].speculative(), "Annotation should be speculative");
                    assert!(!modifiers[0].mandatory(), "Speculative annotation should not be mandatory");
                    assert_eq!(modifiers[0].name, "inline");
                }
                _ => panic!("Expected Assignment"),
            }
        }
    }

    #[test]
    fn test_parse_speculative_hashtag_on_let() {
        let s = r#"txn Foo [true][n >= 0] { let x: Int #?volatile; term; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse speculative hashtag on let: {:?}", result.err());
        if let TopLevel::Transaction(txn) = &result.unwrap().items[0] {
            match &txn.body[0] {
                Statement::Let { modifiers, .. } => {
                    assert_eq!(modifiers.len(), 1);
                    assert!(modifiers[0].speculative());
                    assert_eq!(modifiers[0].name, "volatile");
                }
                _ => panic!("Expected Let"),
            }
        }
    }

    #[test]
    fn test_parse_speculative_hashtag_with_value() {
        let s = r#"txn Foo [true][n >= 0] { &x = 1 #?gpu(1024); term; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse speculative hashtag with value: {:?}", result.err());
        if let TopLevel::Transaction(txn) = &result.unwrap().items[0] {
            match &txn.body[0] {
                Statement::Assignment { modifiers, .. } => {
                    assert_eq!(modifiers.len(), 1);
                    assert!(modifiers[0].speculative());
                    assert_eq!(modifiers[0].name, "gpu");
                    assert_eq!(modifiers[0].string_value(), Some("1024".to_string()));
                }
                _ => panic!("Expected Assignment"),
            }
        }
    }

    #[test]
    fn test_parse_speculative_hashtag_with_negative_value() {
        // Negative values in hashtags not supported — skip this test
    }
    #[test]
    fn test_parse_multi_body_transaction() {
        let s = r#"txn Foo [x > 0][ready] {
                &x = 1;
            }
            [x == 0] {
                &x = 2;
            }
            {
                term;
            };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse multi-body transaction: {:?}", result.err());
        if let Ok(program) = result {
            if let TopLevel::Transaction(txn) = &program.items[0] {
                assert_eq!(txn.variant_bodies.len(), 2, "Should have 2 variant bodies");
                assert!(txn.variant_bodies[1].0.is_none(), "Catch-all variant should have no precondition");
                assert!(!txn.variant_bodies[1].1.is_empty(), "Catch-all body should have statements");
            } else {
                panic!("Expected Transaction");
            }
        }
    }

    #[test]
    fn test_parse_multi_body_definition() {
        let s = r#"defn process [x > 0][result > 0] -> Int {
                &result = x * 2;
            }
            [x == 0] {
                &result = 1;
            }
            {
                &result = 0;
            };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse multi-body definition: {:?}", result.err());
        if let Ok(program) = result {
            if let TopLevel::Definition(defn) = &program.items[0] {
                assert_eq!(defn.variant_bodies.len(), 2, "Should have 2 variant bodies");
            } else {
                panic!("Expected Definition");
            }
        }
    }

    #[test]
    fn test_parse_single_body_still_works() {
        let s = r#"txn Foo [true][n >= 0] { &x = 1; term; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Single body should still parse: {:?}", result.err());
        if let Ok(program) = result {
            if let TopLevel::Transaction(txn) = &program.items[0] {
                assert!(txn.variant_bodies.is_empty(), "Single body should have no variants");
                assert!(!txn.body.is_empty(), "Should have body content");
            } else {
                panic!("Expected Transaction");
            }
        }
    }

    // DISABLED: alka/on_exit — not ready for use.
    // #[test]
    // fn test_on_exit_block_pragma() {
    //     let s = r#"txn Foo [true][n >= 0] {
    //         &CLAIMED = true;
    //         #on_exit {
    //             &CLAIMED = false;
    //         };
    //         dma_work();
    //     };"#;
    //     let mut parser = Parser::new(s);
    //     let result = parser.parse();
    //     assert!(result.is_ok(), "Should parse #on_exit block: {:?}", result.err());
    //     if let Ok(program) = result {
    //         if let TopLevel::Transaction(txn) = &program.items[0] {
    //             let has_on_exit = txn.body.iter().any(|s| matches!(s, Statement::OnExit { .. }));
    //             assert!(has_on_exit, "Should contain OnExit statement");
    //         } else {
    //             panic!("Expected Transaction");
    //         }
    //     }
    // }
    // #[test]
    // fn test_on_exit_no_precondition() {
    //     let s = r#"txn Foo [true][n >= 0] {
    //         #on_exit { &x = 0; };
    //         term;
    //     };"#;
    //     let mut parser = Parser::new(s);
    //     let result = parser.parse();
    //     assert!(result.is_ok(), "Should parse on_exit without pre: {:?}", result.err());
    // }

    #[test]
    fn test_parse_struct_variant_with_add() {
        let s = r#"struct GPU {
            vendor: UInt16;
            bar0: Ptr;
        }
        [has_ce] {
            has_ce: Bool = true;
            + ce_engine: UInt16;
        };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse struct variant with +: {:?}", result.err());
        if let Ok(program) = result {
            if let TopLevel::Struct(sdef) = &program.items[0] {
                assert_eq!(sdef.variants.len(), 1, "Should have 1 variant");
                let v = &sdef.variants[0];
                assert_eq!(v.additions.len(), 1, "Should have 1 addition");
                assert_eq!(v.additions[0].name, "ce_engine");
                assert!(v.removals.is_empty(), "Should have no removals");
            } else {
                panic!("Expected Struct");
            }
        }
    }

    #[test]
    fn test_parse_struct_variant_with_remove() {
        let s = r#"struct GPU {
            vendor: UInt16;
            bar0: Ptr;
        }
        [no_bar0] {
            - bar0;
        };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse struct variant with -: {:?}", result.err());
        if let Ok(program) = result {
            if let TopLevel::Struct(sdef) = &program.items[0] {
                assert_eq!(sdef.variants.len(), 1, "Should have 1 variant");
                assert!(sdef.variants[0].additions.is_empty(), "Should have no additions");
                assert_eq!(sdef.variants[0].removals.len(), 1, "Should have 1 removal");
                assert_eq!(sdef.variants[0].removals[0], "bar0");
            } else {
                panic!("Expected Struct");
            }
        }
    }

    // ── Phase 3/4: Pragma and IO tests ────────────────────────────

    #[test]
    fn test_hashbang_dispatch_parallel() {
        let s = "#!dispatch(parallel)\ntrg x: Bool @ link __x;\nrct txn t [x]] { term; };";
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "#!dispatch(parallel) should parse: {:?}", result.err());
        let program = result.unwrap();
        assert_eq!(program.dispatch_mode, crate::ast::DispatchMode::Parallel,
            "dispatch_mode should be Parallel");
    }

    #[test]
    fn test_nowake_modifier() {
        let s = "trg x: Bool @ link __x #nowake;";
        let mut parser = Parser::new(s);
        let result = parser.parse_trigger();
        assert!(result.is_ok(), "#nowake modifier should parse: {:?}", result.err());
        let trg = result.unwrap();
        assert!(!trg.is_wake, "is_wake should be false with #nowake");
    }

    #[test]
    fn test_wake_default_mmio() {
        let s = "trg x: Bool @ 0x4000;";
        let mut parser = Parser::new(s);
        let result = parser.parse_trigger();
        assert!(result.is_ok(), "MMIO trigger without modifier should parse: {:?}", result.err());
        let trg = result.unwrap();
        assert!(trg.is_wake, "MMIO trigger should default to is_wake=true");
    }

    #[test]
    fn test_nowake_on_mmio() {
        let s = "trg x: Bool @ 0x4000 #nowake;";
        let mut parser = Parser::new(s);
        let result = parser.parse_trigger();
        assert!(result.is_ok(), "MMIO trigger with #nowake should parse: {:?}", result.err());
        let trg = result.unwrap();
        assert!(!trg.is_wake, "is_wake should be false with #nowake on MMIO");
    }

    #[test]
    fn test_link_dependency_parsed() {
        let s = "import \"link/brief_rt.c\";";
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "import link dep should parse: {:?}", result.err());
        let program = result.unwrap();
        assert_eq!(program.items.len(), 1);
        if let TopLevel::LinkDependency(dep) = &program.items[0] {
            assert_eq!(dep.path, "link/brief_rt.c");
            assert_eq!(dep.source_lang, crate::ast::LinkLanguage::C);
        } else {
            panic!("Expected LinkDependency, got {:?}", program.items[0]);
        }
    }

    #[test]
    fn test_link_dependency_user_o() {
        let s = "import \"link/foo.o\";";
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "import link/foo.o should parse: {:?}", result.err());
        let program = result.unwrap();
        if let TopLevel::LinkDependency(dep) = &program.items[0] {
            assert_eq!(dep.path, "link/foo.o");
            assert_eq!(dep.source_lang, crate::ast::LinkLanguage::Object);
        } else {
            panic!("Expected LinkDependency, got {:?}", program.items[0]);
        }
    }

    #[test]
    fn test_link_dependency_user_a() {
        let s = "import \"link/custom_hw.a\";";
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "import .a file should parse: {:?}", result.err());
        let program = result.unwrap();
        if let TopLevel::LinkDependency(dep) = &program.items[0] {
            assert_eq!(dep.path, "link/custom_hw.a");
            assert_eq!(dep.source_lang, crate::ast::LinkLanguage::Object);
        } else {
            panic!("Expected LinkDependency, got {:?}", program.items[0]);
        }
    }

    #[test]
    fn test_link_dependency_java() {
        let s = "import \"link/Main.java\";";
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "import link java should parse: {:?}", result.err());
        let program = result.unwrap();
        if let TopLevel::LinkDependency(dep) = &program.items[0] {
            assert_eq!(dep.path, "link/Main.java");
            assert_eq!(dep.source_lang, crate::ast::LinkLanguage::Java);
        } else {
            panic!("Expected LinkDependency, got {:?}", program.items[0]);
        }
    }

    #[test]
    fn test_link_dependency_typescript() {
        let s = "import \"link/math.ts\";";
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "import link typescript should parse: {:?}", result.err());
        let program = result.unwrap();
        if let TopLevel::LinkDependency(dep) = &program.items[0] {
            assert_eq!(dep.path, "link/math.ts");
            assert_eq!(dep.source_lang, crate::ast::LinkLanguage::AssemblyScript);
        } else {
            panic!("Expected LinkDependency, got {:?}", program.items[0]);
        }
    }

    #[test]
    fn test_sync_block_in_txn_body() {
        let s = "let x: Int; let y: Int; txn test [x==0][x>0] { sync { &x = 1; &y = 2; }; term; };";
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "sync block in txn should parse: {:?}", result.err());
        let program = result.unwrap();
        assert_eq!(program.items.len(), 3);
        if let TopLevel::Transaction(txn) = &program.items[2] {
            assert_eq!(txn.name, "test");
            assert_eq!(txn.body.len(), 2);
            assert!(matches!(txn.body[0], Statement::SyncBlock { .. }));
            assert!(matches!(txn.body[1], Statement::Term { .. }));
        } else {
            panic!("Expected Transaction");
        }
    }

    #[test]
    fn test_import_not_link_dep() {
        let s = "import { X } from \"std/system.bv\";";
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "import module should still parse: {:?}", result.err());
        let program = result.unwrap();
        assert!(matches!(&program.items[0], TopLevel::Import(_)),
            "Expected Import, got {:?}", program.items[0]);
    }

    #[test]
    fn test_link_dep_rejects_named_imports() {
        let s = "import { foo } from \"link/brief_rt.c\";";
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_err(), "Named imports on link deps should error");
    }

    #[test]
    fn test_pragmabang_without_bracket() {
        let s = "#!pragma dispatch(parallel)\ntrg x: Bool @ link __x;\nrct txn t [x]] { term; };";
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "#!pragma without ] should parse: {:?}", result.err());
        let program = result.unwrap();
        assert_eq!(program.dispatch_mode, crate::ast::DispatchMode::Parallel,
            "dispatch_mode should be Parallel");
    }

    #[test]
    fn test_parse_multi_slice_ellipsis() {
        let mut parser = Parser::new("s[...]");
        let result = parser.parse_expression();
        assert!(result.is_ok(), "s[...] should parse: {:?}", result.err());
    }

    #[test]
    fn test_parse_multi_slice_ellipsis_trailing() {
        let mut parser = Parser::new("s[..., 0]");
        let result = parser.parse_expression();
        assert!(result.is_ok(), "s[..., 0] should parse: {:?}", result.err());
    }

    #[test]
    fn test_parse_multi_slice_at_dim() {
        let mut parser = Parser::new("s[@3:0..10]");
        let result = parser.parse_expression();
        assert!(result.is_ok(), "s[@3:0..10] should parse: {:?}", result.err());
    }

    #[test]
    fn test_parse_multi_slice_ast_ellipsis() {
        let mut parser = Parser::new("s[...]");
        let expr = parser.parse_expression().unwrap();
        match &expr {
            crate::ast::Expr::MultiSlice { ops, .. } => {
                assert_eq!(ops.len(), 1);
                assert!(matches!(ops[0], crate::ast::BracketOp::Coord(crate::ast::SliceCoordinate::Ellipsis)));
            }
            _ => panic!("Expected MultiSlice, got {:?}", expr),
        }
    }

    #[test]
    fn test_parse_multi_slice_ast_at_dim() {
        let mut parser = Parser::new("s[@2:5..10]");
        let expr = parser.parse_expression().unwrap();
        match &expr {
            crate::ast::Expr::MultiSlice { ops, .. } => {
                assert_eq!(ops.len(), 1);
                if let crate::ast::BracketOp::Coord(crate::ast::SliceCoordinate::AtDimension { dimension, coord }) = &ops[0] {
                    assert_eq!(*dimension, 2);
                    match coord.as_ref() {
                        crate::ast::SliceCoordinate::Range { start, end } => {
                            assert!(start.is_some());
                            assert!(end.is_some());
                        }
                        _ => panic!("Expected Range inner coordinate"),
                    }
                } else {
                    panic!("Expected Coord(AtDimension), got {:?}", ops[0]);
                }
            }
            _ => panic!("Expected MultiSlice, got {:?}", expr),
        }
    }

    #[test]
    fn test_parse_multi_slice_ast_ellipsis_trailing() {
        let mut parser = Parser::new("s[..., 0]");
        let expr = parser.parse_expression().unwrap();
        match &expr {
            crate::ast::Expr::MultiSlice { ops, .. } => {
                assert_eq!(ops.len(), 2);
                assert!(matches!(ops[0], crate::ast::BracketOp::Coord(crate::ast::SliceCoordinate::Ellipsis)));
                assert!(matches!(ops[1], crate::ast::BracketOp::Coord(crate::ast::SliceCoordinate::Index(_))));
            }
            _ => panic!("Expected MultiSlice, got {:?}", expr),
        }
    }

    #[test]
    fn test_parse_multi_slice_with_mask() {
        let mut parser = Parser::new("list[; age >= 18]");
        let expr = parser.parse_expression().unwrap();
        match &expr {
            crate::ast::Expr::MultiSlice { ops, .. } => {
                assert_eq!(ops.len(), 1);
                assert!(matches!(ops[0], crate::ast::BracketOp::Mask(_)));
            }
            _ => panic!("Expected MultiSlice, got {:?}", expr),
        }
    }

    #[test]
    fn test_parse_multi_slice_with_stride_and_mask() {
        let mut parser = Parser::new("list[::3 ; age >= 18 ::2]");
        let expr = parser.parse_expression().unwrap();
        match &expr {
            crate::ast::Expr::MultiSlice { ops, .. } => {
                assert_eq!(ops.len(), 3);
                assert!(matches!(ops[0], crate::ast::BracketOp::Stride(_)));
                assert!(matches!(ops[1], crate::ast::BracketOp::Mask(_)));
                assert!(matches!(ops[2], crate::ast::BracketOp::Stride(_)));
            }
            _ => panic!("Expected MultiSlice, got {:?}", expr),
        }
    }

    #[test]
    fn test_parse_projection_size() {
        let mut parser = Parser::new("x :> Size");
        let expr = parser.parse_expression().unwrap();
        match &expr {
            crate::ast::Expr::Projection { source, target } => {
                assert!(matches!(source.as_ref(), crate::ast::Expr::Identifier(name) if name == "x"));
                assert!(matches!(target, crate::ast::ProjectionTarget::Size));
            }
            _ => panic!("Expected Projection, got {:?}", expr),
        }
    }

    #[test]
    fn test_parse_projection_ptr() {
        let mut parser = Parser::new("my_list :> Ptr");
        let expr = parser.parse_expression().unwrap();
        match &expr {
            crate::ast::Expr::Projection { source, target } => {
                assert!(matches!(source.as_ref(), crate::ast::Expr::Identifier(name) if name == "my_list"));
                assert!(matches!(target, crate::ast::ProjectionTarget::Ptr));
            }
            _ => panic!("Expected Projection, got {:?}", expr),
        }
    }

    #[test]
    fn test_parse_projection_user_defined() {
        let mut parser = Parser::new("x :> Invalid");
        let expr = parser.parse_expression().unwrap();
        match expr {
            Expr::Projection { source: _, target: ProjectionTarget::UserDefined(name) } => {
                assert_eq!(name, "Invalid");
            }
            _ => panic!("Expected UserDefined projection, got {:?}", expr),
        }
    }

    #[test]
    fn test_parse_projection_user_defined_with_arg() {
        let mut parser = Parser::new("x :> MyField(42)");
        let expr = parser.parse_expression().unwrap();
        match expr {
            Expr::Projection { source: _, target: ProjectionTarget::UserDefinedWithArg(name, arg) } => {
                assert_eq!(name, "MyField");
                assert_eq!(arg.as_integer(), Some(42));
            }
            _ => panic!("Expected UserDefinedWithArg projection, got {:?}", expr),
        }
    }

    #[test]
    fn test_parse_fn_projection_ptr() {
        let mut parser = Parser::new("f :> Address");
        let expr = parser.parse_expression().unwrap();
        match expr {
            Expr::Projection { source: _, target: ProjectionTarget::Address } => {}
            _ => panic!("Expected Address projection, got {:?}", expr),
        }
    }

    #[test]
    fn test_parse_fn_projection_name() {
        let mut parser = Parser::new("add :> Name");
        let expr = parser.parse_expression().unwrap();
        match expr {
            Expr::Projection { source: _, target: ProjectionTarget::Name } => {}
            _ => panic!("Expected Name projection, got {:?}", expr),
        }
    }

    #[test]
    fn test_parse_fn_projection_arity() {
        let mut parser = Parser::new("handler :> Arity");
        let expr = parser.parse_expression().unwrap();
        match expr {
            Expr::Projection { source: _, target: ProjectionTarget::Arity } => {}
            _ => panic!("Expected Arity projection, got {:?}", expr),
        }
    }

    #[test]
    fn test_parse_fn_projection_is_pure() {
        let mut parser = Parser::new("compute :> IsPure");
        let expr = parser.parse_expression().unwrap();
        match expr {
            Expr::Projection { source: _, target: ProjectionTarget::IsPure } => {}
            _ => panic!("Expected IsPure projection, got {:?}", expr),
        }
    }

    #[test]
    fn test_parse_fn_projection_span() {
        let mut parser = Parser::new("txn :> FnSpan");
        let expr = parser.parse_expression().unwrap();
        match expr {
            Expr::Projection { source: _, target: ProjectionTarget::FnSpan } => {}
            _ => panic!("Expected FnSpan projection, got {:?}", expr),
        }
    }

    #[test]
    fn test_parse_uni_simple_pattern() {
        let mut parser = Parser::new("uni x -> 42;");
        let stmt = parser.parse_statement().unwrap();
        match stmt {
            Statement::Unification { name, variant, fields, expr } => {
                assert_eq!(name, "uni");
                assert_eq!(variant, "x");
                assert!(fields.is_empty());
                assert_eq!(expr.as_integer(), Some(42));
            }
            _ => panic!("Expected Unification, got {:?}", stmt),
        }
    }

    #[test]
    fn test_parse_uni_variant_with_field() {
        let src = "uni val(Some(v)) -> 42;";
        let mut parser = Parser::new(src);
        let stmt = parser.parse_statement().unwrap();
        match stmt {
            Statement::Unification { name, variant, fields, expr } => {
                assert_eq!(name, "val");
                assert_eq!(variant, "Some");
                assert_eq!(fields.len(), 1);
                assert!(matches!(&fields[0], Pattern::Var(f) if f == "v"));
                assert_eq!(expr.as_integer(), Some(42));
            }
            _ => panic!("Expected Unification, got {:?}", stmt),
        }
    }

    #[test]
    fn test_parse_uni_wildcard() {
        let src = "uni val(_) -> 99;";
        let mut parser = Parser::new(src);
        let stmt = parser.parse_statement().unwrap();
        match stmt {
            Statement::Unification { name, variant, fields, expr } => {
                assert_eq!(name, "val");
                assert_eq!(variant, "_");
                assert!(fields.is_empty());
                assert_eq!(expr.as_integer(), Some(99));
            }
            _ => panic!("Expected Unification, got {:?}", stmt),
        }
    }

    #[test]
    fn test_parse_uni_variant_with_tuple_field() {
        let src = "uni val(Some((a, b))) -> 0;";
        let mut parser = Parser::new(src);
        let stmt = parser.parse_statement().unwrap();
        match stmt {
            Statement::Unification { name, variant, fields, expr } => {
                assert_eq!(name, "val");
                assert_eq!(variant, "Some");
                assert_eq!(fields.len(), 1);
                match &fields[0] {
                    Pattern::Tuple(elems) => {
                        assert_eq!(elems.len(), 2);
                        assert!(matches!(&elems[0], Pattern::Var(e) if e == "a"));
                        assert!(matches!(&elems[1], Pattern::Var(e) if e == "b"));
                    }
                    other => panic!("Expected Tuple pattern, got {:?}", other),
                }
                assert_eq!(expr.as_integer(), Some(0));
            }
            _ => panic!("Expected Unification, got {:?}", stmt),
        }
    }

    #[test]
    fn test_parse_uni_variant_with_literal_int() {
        let src = "uni val(Some(42)) -> 0;";
        let mut parser = Parser::new(src);
        let stmt = parser.parse_statement().unwrap();
        match stmt {
            Statement::Unification { name, variant, fields, expr } => {
                assert_eq!(name, "val");
                assert_eq!(variant, "Some");
                assert_eq!(fields.len(), 1);
                assert!(matches!(&fields[0], Pattern::LitInt(42)));
                assert_eq!(expr.as_integer(), Some(0));
            }
            _ => panic!("Expected Unification, got {:?}", stmt),
        }
    }

    #[test]
    fn test_parse_uni_variant_with_literal_string() {
        let src = r#"uni val(Msg("hello")) -> 0;"#;
        let mut parser = Parser::new(src);
        let stmt = parser.parse_statement().unwrap();
        match stmt {
            Statement::Unification { name, variant, fields, expr } => {
                assert_eq!(name, "val");
                assert_eq!(variant, "Msg");
                assert_eq!(fields.len(), 1);
                assert!(matches!(&fields[0], Pattern::LitString(s) if s == "hello"));
                assert_eq!(expr.as_integer(), Some(0));
            }
            _ => panic!("Expected Unification, got {:?}", stmt),
        }
    }

    #[test]
    fn test_parse_uni_variant_with_multiple_fields() {
        let src = "uni pair(Pair(a, b)) -> 1;";
        let mut parser = Parser::new(src);
        let stmt = parser.parse_statement().unwrap();
        match stmt {
            Statement::Unification { name, variant, fields, expr } => {
                assert_eq!(name, "pair");
                assert_eq!(variant, "Pair");
                assert_eq!(fields.len(), 2);
                assert!(matches!(&fields[0], Pattern::Var(f) if f == "a"));
                assert!(matches!(&fields[1], Pattern::Var(f) if f == "b"));
                assert_eq!(expr.as_integer(), Some(1));
            }
            _ => panic!("Expected Unification, got {:?}", stmt),
        }
    }

    #[test]
    fn test_parse_uni_block_rhs() {
        let src = "uni val(Some(v)) -> { term; };";
        let mut parser = Parser::new(src);
        let stmt = parser.parse_statement().unwrap();
        match stmt {
            Statement::Unification { name, variant, fields, expr } => {
                assert_eq!(name, "val");
                assert_eq!(variant, "Some");
                assert_eq!(fields.len(), 1);
                assert!(matches!(&fields[0], Pattern::Var(f) if f == "v"));
                assert!(matches!(expr, Expr::Block(_, _)));
            }
            _ => panic!("Expected Unification, got {:?}", stmt),
        }
    }

    #[test]
    fn test_parse_uni_with_wildcard_in_block_rhs() {
        let src = "uni val(_) -> { term; };";
        let mut parser = Parser::new(src);
        let stmt = parser.parse_statement().unwrap();
        match stmt {
            Statement::Unification { name, variant, fields, expr } => {
                assert_eq!(name, "val");
                assert_eq!(variant, "_");
                assert!(fields.is_empty());
                assert!(matches!(expr, Expr::Block(_, _)));
            }
            _ => panic!("Expected Unification, got {:?}", stmt),
        }
    }

    #[test]
    fn test_parse_match_wildcard_only() {
        let src = "match x { _ -> 0 }";
        let mut parser = Parser::new(src);
        let expr = parser.parse_expression().unwrap();
        match expr {
            Expr::Match { value, arms } => {
                assert!(matches!(*value, Expr::Identifier(n) if n == "x"));
                assert_eq!(arms.len(), 1);
                assert!(matches!(arms[0].pattern, MatchPattern::Wildcard));
                assert_eq!(arms[0].body.as_integer(), Some(0));
            }
            _ => panic!("Expected Match, got {:?}", expr),
        }
    }

    #[test]
    fn test_parse_match_variant_with_field() {
        let src = "match x { Some(v) -> v, _ -> 0 }";
        let mut parser = Parser::new(src);
        let expr = parser.parse_expression().unwrap();
        match expr {
            Expr::Match { value, arms } => {
                assert!(matches!(*value, Expr::Identifier(n) if n == "x"));
                assert_eq!(arms.len(), 2);
                match &arms[0].pattern {
                    MatchPattern::Variant { name, fields } => {
                        assert_eq!(name, "Some");
                        assert_eq!(fields.len(), 1);
                        assert!(matches!(&fields[0], Pattern::Var(f) if f == "v"));
                    }
                    _ => panic!("Expected Variant pattern"),
                }
                assert!(matches!(arms[1].pattern, MatchPattern::Wildcard));
            }
            _ => panic!("Expected Match, got {:?}", expr),
        }
    }

    #[test]
    fn test_parse_match_variant_with_literal_int() {
        let src = "match x { N(42) -> 1, _ -> 0 }";
        let mut parser = Parser::new(src);
        let expr = parser.parse_expression().unwrap();
        match expr {
            Expr::Match { value, arms } => {
                assert!(matches!(*value, Expr::Identifier(n) if n == "x"));
                assert_eq!(arms.len(), 2);
                match &arms[0].pattern {
                    MatchPattern::Variant { name, fields } => {
                        assert_eq!(name, "N");
                        assert_eq!(fields.len(), 1);
                        assert!(matches!(&fields[0], Pattern::LitInt(42)));
                    }
                    _ => panic!("Expected Variant pattern"),
                }
            }
            _ => panic!("Expected Match, got {:?}", expr),
        }
    }

    #[test]
    fn test_parse_match_variant_with_tuple() {
        let src = "match x { P((a, b)) -> a, _ -> 0 }";
        let mut parser = Parser::new(src);
        let expr = parser.parse_expression().unwrap();
        match expr {
            Expr::Match { value, arms } => {
                assert!(matches!(*value, Expr::Identifier(n) if n == "x"));
                assert_eq!(arms.len(), 2);
                match &arms[0].pattern {
                    MatchPattern::Variant { name, fields } => {
                        assert_eq!(name, "P");
                        assert_eq!(fields.len(), 1);
                        match &fields[0] {
                            Pattern::Tuple(elems) => {
                                assert_eq!(elems.len(), 2);
                                assert!(matches!(&elems[0], Pattern::Var(e) if e == "a"));
                                assert!(matches!(&elems[1], Pattern::Var(e) if e == "b"));
                            }
                            _ => panic!("Expected Tuple pattern"),
                        }
                    }
                    _ => panic!("Expected Variant pattern"),
                }
            }
            _ => panic!("Expected Match, got {:?}", expr),
        }
    }

    #[test]
    fn test_parse_match_variant_with_literal_string() {
        let src = r#"match x { Msg("ok") -> 1, _ -> 0 }"#;
        let mut parser = Parser::new(src);
        let expr = parser.parse_expression().unwrap();
        match expr {
            Expr::Match { value, arms } => {
                assert!(matches!(*value, Expr::Identifier(n) if n == "x"));
                assert_eq!(arms.len(), 2);
                match &arms[0].pattern {
                    MatchPattern::Variant { name, fields } => {
                        assert_eq!(name, "Msg");
                        assert_eq!(fields.len(), 1);
                        assert!(matches!(&fields[0], Pattern::LitString(s) if s == "ok"));
                    }
                    _ => panic!("Expected Variant pattern"),
                }
            }
            _ => panic!("Expected Match, got {:?}", expr),
        }
    }

    #[test]
    fn test_parse_match_multiple_fields() {
        let src = "match x { Pair(a, b) -> 0, _ -> 1 }";
        let mut parser = Parser::new(src);
        let expr = parser.parse_expression().unwrap();
        match expr {
            Expr::Match { value, arms } => {
                assert!(matches!(*value, Expr::Identifier(n) if n == "x"));
                assert_eq!(arms.len(), 2);
                match &arms[0].pattern {
                    MatchPattern::Variant { name, fields } => {
                        assert_eq!(name, "Pair");
                        assert_eq!(fields.len(), 2);
                        assert!(matches!(&fields[0], Pattern::Var(f) if f == "a"));
                        assert!(matches!(&fields[1], Pattern::Var(f) if f == "b"));
                    }
                    _ => panic!("Expected Variant pattern"),
                }
            }
            _ => panic!("Expected Match, got {:?}", expr),
        }
    }

    // ---- Subtype projection (<:) tests ----

    fn parse_subtype_expr(src: &str) -> Expr {
        let full = format!("defn f [x][x] {{\n{}\nterm 0;\n}};", src);
        let mut p = Parser::new(&full);
        let prog = p.parse().unwrap();
        match &prog.items[0] {
            TopLevel::Definition(d) => match &d.body[0] {
                Statement::Let { expr: Some(e), .. } => e.clone(),
                _ => panic!("Expected Let, got {:?}", &d.body[0]),
            },
            _ => panic!("Expected Definition, got {:?}", &prog.items[0]),
        }
    }

    fn check_subtype_ops(src: &str, expected_ops: &[crate::ast::SubtypeOp]) {
        let expr = parse_subtype_expr(src);
        match expr {
            Expr::SubtypeProjection { ops, .. } => {
                assert_eq!(ops.len(), expected_ops.len());
                for (got, want) in ops.iter().zip(expected_ops.iter()) {
                    assert_eq!(got, want);
                }
            }
            _ => panic!("Expected SubtypeProjection, got {:?}", expr),
        }
    }

    #[test]
    fn test_parse_subtype_filter() {
        check_subtype_ops(
            "let result <: items { FILTER(active); };",
            &[crate::ast::SubtypeOp::Filter(Box::new(Expr::Identifier("active".into())))],
        );
    }

    #[test]
    fn test_parse_subtype_map() {
        check_subtype_ops(
            "let result <: items { MAP(x * 2); };",
            &[crate::ast::SubtypeOp::Map(Box::new(Expr::BinaryOp(Box::new(
                crate::features::binary_op::BinaryOpExpr::new(
                    crate::features::binary_op::BinaryOpKind::Mul,
                    Expr::Identifier("x".into()),
                    Expr::Literal(Box::new(LiteralExpr::Integer(2))),
                ),
            ))))],
        );
    }

    #[test]
    fn test_parse_subtype_sort() {
        check_subtype_ops(
            "let result <: items { SORT(name); };",
            &[crate::ast::SubtypeOp::Sort(Box::new(Expr::Identifier("name".into())))],
        );
    }

    #[test]
    fn test_parse_subtype_limit() {
        check_subtype_ops(
            "let result <: items { LIMIT(10); };",
            &[crate::ast::SubtypeOp::Limit(10)],
        );
    }

    #[test]
    fn test_parse_subtype_skip() {
        check_subtype_ops(
            "let result <: items { SKIP(5); };",
            &[crate::ast::SubtypeOp::Skip(5)],
        );
    }

    #[test]
    fn test_parse_subtype_unique() {
        check_subtype_ops(
            "let result <: items { UNIQUE; };",
            &[crate::ast::SubtypeOp::Unique],
        );
    }

    #[test]
    fn test_parse_subtype_join() {
        check_subtype_ops(
            "let result <: items { JOIN(other, key); };",
            &[crate::ast::SubtypeOp::Join(
                Box::new(Expr::Identifier("other".into())),
                Box::new(Expr::Identifier("key".into())),
            )],
        );
    }

    #[test]
    fn test_parse_subtype_group() {
        check_subtype_ops(
            "let result <: items { GROUP(category); };",
            &[crate::ast::SubtypeOp::Group(Box::new(Expr::Identifier("category".into())))],
        );
    }

    #[test]
    fn test_parse_subtype_count() {
        check_subtype_ops(
            "let result <: items { COUNT; };",
            &[crate::ast::SubtypeOp::Count],
        );
    }

    #[test]
    fn test_parse_subtype_sum() {
        check_subtype_ops(
            "let result <: items { SUM(price); };",
            &[crate::ast::SubtypeOp::Sum(Box::new(Expr::Identifier("price".into())))],
        );
    }

    #[test]
    fn test_parse_subtype_avg() {
        check_subtype_ops(
            "let result <: items { AVG(score); };",
            &[crate::ast::SubtypeOp::Avg(Box::new(Expr::Identifier("score".into())))],
        );
    }

    #[test]
    fn test_parse_subtype_min() {
        check_subtype_ops(
            "let result <: items { MIN(age); };",
            &[crate::ast::SubtypeOp::Min(Box::new(Expr::Identifier("age".into())))],
        );
    }

    #[test]
    fn test_parse_subtype_max() {
        check_subtype_ops(
            "let result <: items { MAX(height); };",
            &[crate::ast::SubtypeOp::Max(Box::new(Expr::Identifier("height".into())))],
        );
    }

    #[test]
    fn test_parse_subtype_match() {
        check_subtype_ops(
            r#"let result <: email["^(.+)@(.+)$"];"#,
            &[crate::ast::SubtypeOp::Match(Box::new(Expr::Literal(Box::new(LiteralExpr::String("^(.+)@(.+)$".into())))))],
        );
    }

    #[test]
    fn test_parse_subtype_composite_chain() {
        check_subtype_ops(
            "let result <: items { FILTER(active); MAP(x * 2); LIMIT(5); };",
            &[
                crate::ast::SubtypeOp::Filter(Box::new(Expr::Identifier("active".into()))),
                crate::ast::SubtypeOp::Map(Box::new(Expr::BinaryOp(Box::new(
                    crate::features::binary_op::BinaryOpExpr::new(
                        crate::features::binary_op::BinaryOpKind::Mul,
                        Expr::Identifier("x".into()),
                        Expr::Literal(Box::new(LiteralExpr::Integer(2))),
                    ),
                )))),
                crate::ast::SubtypeOp::Limit(5),
            ],
        );
    }

    #[test]
    fn test_parse_is_type() {
        let s = r#"defn f() -> Bool { term 42 is Int; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "is type check should parse: {:?}", result.err());
        if let TopLevel::Definition(defn) = &result.unwrap().items[0] {
            match &defn.body[0] {
                Statement::Term { values, .. } => {
                    let expr = values[0].as_ref().unwrap();
                    if let Expr::IsType(_, crate::ast::IsTarget::Type(Type::Custom(name))) = expr {
                        assert_eq!(name, "Int", "Expected IsType(Int), got {:?}", expr);
                    } else {
                        panic!("Expected IsType(Int), got {:?}", expr);
                    }
                }
                _ => panic!("Expected Term"),
            }
        }
    }

    #[test]
    fn test_parse_is_variant() {
        let s = r#"defn f(x: Option[Int]) -> Bool { term x is some; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "is variant check should parse: {:?}", result.err());
        if let TopLevel::Definition(defn) = &result.unwrap().items[0] {
            match &defn.body[0] {
                Statement::Term { values, .. } => {
                    let expr = values[0].as_ref().unwrap();
                    assert!(matches!(expr, Expr::IsType(_, crate::ast::IsTarget::Variant(v)) if v == "Some"),
                        "Expected IsType(Some), got {:?}", expr);
                }
                _ => panic!("Expected Term"),
            }
        }
    }

    #[test]
    fn test_parse_from_check() {
        let s = r#"defn f(x: Int) -> Bool { term x from Int; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "from check should parse: {:?}", result.err());
        if let TopLevel::Definition(defn) = &result.unwrap().items[0] {
            match &defn.body[0] {
                Statement::Term { values, .. } => {
                    let expr = values[0].as_ref().unwrap();
                    if let Expr::FromCheck(_, Type::Custom(name)) = expr {
                        assert_eq!(name, "Int", "Expected FromCheck(Int), got {:?}", expr);
                    } else {
                        panic!("Expected FromCheck(Int), got {:?}", expr);
                    }
                }
                _ => panic!("Expected Term"),
            }
        }
    }

    #[test]
    fn test_parse_like() {
        let s = r#"defn f(x: Int, y: Int) -> Bool { term x like y; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "like should parse: {:?}", result.err());
        if let TopLevel::Definition(defn) = &result.unwrap().items[0] {
            match &defn.body[0] {
                Statement::Term { values, .. } => {
                    let expr = values[0].as_ref().unwrap();
                    assert!(matches!(expr, Expr::Like(_, _)),
                        "Expected Like expr, got {:?}", expr);
                }
                _ => panic!("Expected Term"),
            }
        }
    }

    #[test]
    fn test_parse_is_precedence() {
        let s = r#"defn f(x: Int, y: Int) -> Bool { term x is Int && y is Int; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "is with && should parse: {:?}", result.err());
        if let TopLevel::Definition(defn) = &result.unwrap().items[0] {
            match &defn.body[0] {
                Statement::Term { values, .. } => {
                    let expr = values[0].as_ref().unwrap();
                    assert!(matches!(expr, Expr::BinaryOp(bop) if bop.kind == crate::features::binary_op::BinaryOpKind::And),
                        "is should bind tighter than &&, got: {:?}", expr);
                }
                _ => panic!("Expected Term"),
            }
        }
    }

    #[test]
    fn test_parse_cast_as() {
        let s = r#"defn f(x: Int) -> String { term x as String; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "as cast should parse: {:?}", result.err());
        if let TopLevel::Definition(defn) = &result.unwrap().items[0] {
            match &defn.body[0] {
                Statement::Term { values, .. } => {
                    let expr = values[0].as_ref().unwrap();
                    if let Expr::Cast(_, Type::Custom(name)) = expr {
                        assert_eq!(name, "String", "Expected Cast(Int, String), got {:?}", expr);
                    } else {
                        panic!("Expected Cast(Int, String), got {:?}", expr);
                    }
                }
                _ => panic!("Expected Term"),
            }
        }
    }

    #[test]
    fn test_parse_cast_paren() {
        let s = r#"defn f(x: Int) -> String { term (String)x; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "(Type) cast should parse: {:?}", result.err());
        if let TopLevel::Definition(defn) = &result.unwrap().items[0] {
            match &defn.body[0] {
                Statement::Term { values, .. } => {
                    let expr = values[0].as_ref().unwrap();
                    if let Expr::Cast(_, Type::Custom(name)) = expr {
                        assert_eq!(name, "String", "Expected Cast(Int, String), got {:?}", expr);
                    } else {
                        panic!("Expected Cast(Int, String), got {:?}", expr);
                    }
                }
                _ => panic!("Expected Term"),
            }
        }
    }

    #[test]
    fn test_parse_cast_int_paren() {
        let s = r#"defn f(x: String) -> Int { term (Int)x; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "(Int) cast should parse: {:?}", result.err());
        if let TopLevel::Definition(defn) = &result.unwrap().items[0] {
            match &defn.body[0] {
                Statement::Term { values, .. } => {
                    let expr = values[0].as_ref().unwrap();
                    if let Expr::Cast(_, Type::Custom(name)) = expr {
                        assert_eq!(name, "Int", "Expected Cast(String, Int), got {:?}", expr);
                    } else {
                        panic!("Expected Cast(String, Int), got {:?}", expr);
                    }
                }
                _ => panic!("Expected Term"),
            }
        }
    }

    #[test]
    fn test_parse_import_wasm_target() {
        let s = r#"(wasm) import "physics.bv" as physics; defn main -> Int { term 42; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "(wasm) import should parse: {:?}", result.err());
        if let TopLevel::Import(imp) = &result.unwrap().items[0] {
            assert_eq!(imp.target, crate::ast::ImportTarget::Wasm);
            assert_eq!(imp.path.join("/"), "physics.bv");
        } else {
            panic!("Expected Import");
        }
    }

    #[test]
    fn test_parse_typedef_bits_bitrange() {
        let s = "type MyInt <: Bits @/0..63 { Bytes <~ 8; };";
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse Bits @/0..63 base: {:?}", result.err());
        if let TopLevel::TypeDef(td) = &result.unwrap().items[0] {
            assert_eq!(td.bit_range, Some(crate::ast::BitRange::Range(0, 63)));
        } else {
            panic!("Expected TypeDef");
        }
    }

    #[test]
    fn test_cfg_guard_single_item() {
        let s = r##"#!cfg(target_os == "linux")
defn foo() -> Int { term 1; };
"##;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse cfg guard: {:?}", result.err());
        let program = result.unwrap();
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            TopLevel::Cfg(cfg) => {
                assert_eq!(cfg.condition, CfgCondition::Eq("target_os".into(), "linux".into()));
                assert_eq!(cfg.items.len(), 1);
                match &cfg.items[0] {
                    TopLevel::Definition(d) => assert_eq!(d.name, "foo"),
                    _ => panic!("Expected Definition inside cfg guard"),
                }
            }
            other => panic!("Expected Cfg guard, got {:?}", other),
        }
    }

    #[test]
    fn test_cfg_guard_block() {
        let s = r##"#!cfg(target_os == "freestanding") {
    defn a() -> Int { term 1; };
    defn b() -> Int { term 2; };
};
"##;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse cfg block: {:?}", result.err());
        let program = result.unwrap();
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            TopLevel::Cfg(cfg) => {
                assert_eq!(cfg.condition, CfgCondition::Eq("target_os".into(), "freestanding".into()));
                assert_eq!(cfg.items.len(), 2);
            }
            other => panic!("Expected Cfg guard, got {:?}", other),
        }
    }

    #[test]
    fn test_cfg_guard_not_equal() {
        let s = r##"#!cfg(target_arch != "x86_64")
defn fallback() -> Int { term 0; };
"##;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse cfg != guard: {:?}", result.err());
        let program = result.unwrap();
        match &program.items[0] {
            TopLevel::Cfg(cfg) => {
                assert_eq!(cfg.condition, CfgCondition::Ne("target_arch".into(), "x86_64".into()));
            }
            other => panic!("Expected Cfg guard, got {:?}", other),
        }
    }

    fn make_defn(name: &str) -> Definition {
        Definition {
            name: name.to_string(),
            type_params: vec![],
            parameters: vec![],
            outputs: vec![],
            output_type: None,
            output_names: vec![],
            contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
            body: vec![],
            is_lambda: false,
            annotations: vec![],
            modifiers: vec![],
            variant_bodies: vec![],
        }
    }

    #[test]
    fn test_flatten_cfg_includes_true() {
        let mut items = vec![
            TopLevel::Cfg(CfgGuard {
                condition: CfgCondition::Eq("target_os".into(), "linux".into()),
                items: vec![
                    TopLevel::Definition(make_defn("only_linux")),
                ],
            }),
        ];
        crate::parser::flatten_cfg(&mut items, "linux", "x86_64", "");
        assert_eq!(items.len(), 1);
        match &items[0] {
            TopLevel::Definition(d) => assert_eq!(d.name, "only_linux"),
            other => panic!("Expected Definition after flatten, got {:?}", other),
        }
    }

    #[test]
    fn test_flatten_cfg_excludes_false() {
        let mut items = vec![
            TopLevel::Cfg(CfgGuard {
                condition: CfgCondition::Eq("target_os".into(), "freestanding".into()),
                items: vec![
                    TopLevel::Definition(make_defn("only_freestanding")),
                ],
            }),
        ];
        crate::parser::flatten_cfg(&mut items, "linux", "x86_64", "");
        assert_eq!(items.len(), 0, "freestanding-only items should be removed on linux");
    }

    #[test]
    fn test_cfg_condition_evaluate() {
        let cond = CfgCondition::And(
            Box::new(CfgCondition::Eq("target_os".into(), "linux".into())),
            Box::new(CfgCondition::Eq("target_arch".into(), "x86_64".into())),
        );
        assert_eq!(cond.evaluate("linux", "x86_64", ""), Ok(true));
        assert_eq!(cond.evaluate("linux", "aarch64", ""), Ok(false));
        assert_eq!(cond.evaluate("freestanding", "x86_64", ""), Ok(false));
    }

    #[test]
    fn test_cfg_condition_unknown_key_warning() {
        // Unknown keys should produce Err, not silent false
        let cond = CfgCondition::Eq("target_os".into(), "linux".into());
        assert_eq!(cond.evaluate("linux", "x86_64", ""), Ok(true));
        let cond_typo = CfgCondition::Eq("targt_os".into(), "linux".into());
        let result = cond_typo.evaluate("linux", "x86_64", "");
        assert!(result.is_err(), "typo in cfg key should produce warning");
        assert!(result.unwrap_err().contains("unknown cfg key"), "warning should mention unknown key");
    }

    // ── Phase 7B: Operator Declaration Tests ─────────────────
    #[test]
    fn test_parse_operator_add_declaration() {
        let s = "type MyFloat <: Bits { Bytes <~ 4; op Add(MyFloat) -> MyFloat = my_add; };";
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse op declaration: {:?}", result.err());
        if let TopLevel::TypeDef(td) = &result.unwrap().items[0] {
            assert_eq!(td.body.operators.len(), 1);
            assert_eq!(td.body.operators[0].rune, OpRune::Add);
        } else { panic!("Expected TypeDef"); }
    }

    #[test]
    fn test_parse_operator_unary_neg() {
        let s = "type Fixed <: Bits { Bytes <~ 4; op Neg -> Fixed = my_neg; };";
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse unary op: {:?}", result.err());
        if let TopLevel::TypeDef(td) = &result.unwrap().items[0] {
            assert_eq!(td.body.operators[0].rune, OpRune::Neg);
            assert!(td.body.operators[0].param_type.is_none());
        } else { panic!("Expected TypeDef"); }
    }

    #[test]
    fn test_parse_multiple_operators() {
        let s = "type Float4 <: Bits { Bytes <~ 16; op Add(Float4) -> Float4 = my_add; op Sub(Float4) -> Float4 = my_sub; };";
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse multiple ops: {:?}", result.err());
        if let TopLevel::TypeDef(td) = &result.unwrap().items[0] {
            assert_eq!(td.body.operators.len(), 2);
            assert_eq!(td.body.operators[0].rune, OpRune::Add);
            assert_eq!(td.body.operators[1].rune, OpRune::Sub);
        } else { panic!("Expected TypeDef"); }
    }

    #[test]
    fn test_parse_operator_unknown_rune_fails() {
        let s = "type Bad <: Bits { Bytes <~ 4; op Unknown() -> Int = identity; };";
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_err(), "Unknown rune should fail");
    }

    #[test]
    fn test_parse_example_float4() {
        let source = include_str!("../examples/inop-float4.bv");
        let mut parser = Parser::new(source);
        let result = parser.parse();
        assert!(result.is_ok(), "Float4 example should parse: {:?}", result.err());
    }

    #[test]
    fn test_parse_example_custom_types() {
        let source = include_str!("../examples/inop-custom-types.bv");
        let mut parser = Parser::new(source);
        let result = parser.parse();
        assert!(result.is_ok(), "Custom types example should parse: {:?}", result.err());
    }

    #[test]
    fn test_parse_from_bits_dot_bv() {
        let source = include_str!("../lib/std/from-bits.bv");
        let mut parser = Parser::new(source);
        let result = parser.parse();
        // from-bits.bv is an educational file with conceptual syntax.
        // It should parse up to the `$ slot(n) { ... }` template macro
        // which is not supported as a top-level construct.
        match &result {
            Ok(prog) => assert!(prog.items.len() > 5, "should have parsed many type defs"),
            Err(e) => {
                let msg = format!("{}", e);
                assert!(msg.contains("$"), "expected failure on $ template syntax, got: {}", msg);
            }
        }
    }

    // ── Phase D: Annotation Arrow Tests ───────────────────────
    #[test]
    fn test_parse_definition_with_annotations() {
        let src = "defn compute <~ priority: 2, #cached (x: Int) -> Int { term x; };";
        let mut parser = Parser::new(src);
        let prog = parser.parse().unwrap();
        match &prog.items[0] {
            TopLevel::Definition(d) => {
                assert_eq!(d.name, "compute");
                assert_eq!(d.annotations.len(), 2);
                assert_eq!(d.annotations[0].name, "priority");
                assert_eq!(d.annotations[1].name, "cached");
                assert_eq!(d.annotations[1].value.as_ref(), &Expr::Bool(true));
            }
            other => panic!("Expected Definition, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_transaction_with_annotations() {
        let src = "txn process <~ retry: 3, #atomic (x: Int) [x > 0][x == 0] { term; };";
        let mut parser = Parser::new(src);
        let prog = parser.parse().unwrap();
        match &prog.items[0] {
            TopLevel::Transaction(t) => {
                assert_eq!(t.name, "process");
                assert_eq!(t.annotations.len(), 2);
                assert_eq!(t.annotations[0].name, "retry");
                assert_eq!(t.annotations[1].name, "atomic");
                assert_eq!(t.annotations[1].value.as_ref(), &Expr::Bool(true));
            }
            other => panic!("Expected Transaction, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_trigger_with_annotations() {
        let src = "trg tick: Int <~ period: 100, #critical @timer#(1000);";
        let mut parser = Parser::new(src);
        let prog = parser.parse().unwrap();
        match &prog.items[0] {
            TopLevel::Trigger(t) => {
                assert_eq!(t.name, "tick");
                assert_eq!(t.annotations.len(), 2);
                assert_eq!(t.annotations[0].name, "period");
                assert_eq!(t.annotations[1].name, "critical");
                assert_eq!(t.annotations[1].value.as_ref(), &Expr::Bool(true));
            }
            other => panic!("Expected Trigger, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_definition_without_annotations() {
        let src = "defn add(x: Int, y: Int) -> Int { term x + y; };";
        let mut parser = Parser::new(src);
        let prog = parser.parse().unwrap();
        match &prog.items[0] {
            TopLevel::Definition(d) => {
                assert_eq!(d.name, "add");
                assert!(d.annotations.is_empty());
            }
            other => panic!("Expected Definition, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_typedef_bindings_with_tilde_arrow() {
        let src = "type Foo <: Bits { bytes <~ 8; alignment <~ 4; };";
        let mut parser = Parser::new(src);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse <~ bindings in type def: {:?}", result.err());
        if let Ok(prog) = result {
            match &prog.items[0] {
                TopLevel::TypeDef(td) => {
                    assert_eq!(td.name, "Foo");
                    assert_eq!(td.body.bindings.len(), 2);
                    assert_eq!(td.body.bindings[0].name, "bytes");
                    assert_eq!(td.body.bindings[1].name, "alignment");
                }
                other => panic!("Expected TypeDef, got {:?}", other),
            }
        }
    }

    #[test]
    fn test_parse_typedef_pragma_shorthand() {
        // #volatile inside a type body should produce binding name "volatile" (lowercase)
        let src = "type Bar <: Bits { #volatile; };";
        let mut parser = Parser::new(src);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse #volatile in type def: {:?}", result.err());
        if let Ok(prog) = result {
            match &prog.items[0] {
                TopLevel::TypeDef(td) => {
                    assert_eq!(td.body.bindings.len(), 1);
                    assert_eq!(td.body.bindings[0].name, "volatile");
                    assert_eq!(td.body.bindings[0].value.as_ref(), &Expr::Bool(true));
                }
                other => panic!("Expected TypeDef, got {:?}", other),
            }
        }
    }

    // ── Phase 4: `export` keyword tests ────────────────────────────

    #[test]
    fn test_parse_export_defn_basic() {
        // export defn without explicit name — bare export
        let src = "export defn add(x: Int) -> Int { term x + 1; };";
        let mut parser = Parser::new(src);
        let prog = parser.parse().unwrap();
        match &prog.items[0] {
            TopLevel::Definition(d) => {
                assert_eq!(d.name, "add");
                assert_eq!(d.modifiers.len(), 1);
                assert_eq!(d.modifiers[0].name, "export");
                assert_eq!(d.modifiers[0].value, Expr::Bool(true));
            }
            other => panic!("Expected Definition, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_export_defn_with_name() {
        // export("my_add") defn with explicit export symbol
        let src = r#"export("my_add_api") defn add(x: Int) -> Int { term x + 1; };"#;
        let mut parser = Parser::new(src);
        let prog = parser.parse().unwrap();
        match &prog.items[0] {
            TopLevel::Definition(d) => {
                assert_eq!(d.name, "add");
                assert_eq!(d.modifiers.len(), 1);
                assert_eq!(d.modifiers[0].name, "export");
                assert_eq!(d.modifiers[0].value, Expr::String("my_add_api".to_string()));
            }
            other => panic!("Expected Definition, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_export_txn_callable() {
        // export txn — callable transaction with convergence loop
        let src = "export txn count(n: Int, i: Int) [i < n][i == n] -> Int { term i; };";
        let mut parser = Parser::new(src);
        let prog = parser.parse().unwrap();
        match &prog.items[0] {
            TopLevel::Transaction(t) => {
                assert_eq!(t.name, "count");
                assert!(!t.is_reactive, "export txn must be non-reactive");
                assert_eq!(t.modifiers.len(), 1);
                assert_eq!(t.modifiers[0].name, "export");
            }
            other => panic!("Expected Transaction, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_hash_export_still_works() {
        // Backward compat: #export defn still parses (Phase 4 is additive)
        let src = "#export defn add(x: Int) -> Int { term x + 1; };";
        let mut parser = Parser::new(src);
        let prog = parser.parse().unwrap();
        match &prog.items[0] {
            TopLevel::Definition(d) => {
                assert_eq!(d.name, "add");
                assert!(d.modifiers.iter().any(|m| m.name == "export"),
                    "#export should produce export annotation");
            }
            other => panic!("Expected Definition, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_export_rct_txn_rejected() {
        // export rct txn is not supported — must produce error
        let src = "export rct txn tick [x < 100][x == 100] { };";
        let mut parser = Parser::new(src);
        let result = parser.parse();
        assert!(result.is_err(), "export rct txn should be rejected");
    }

    // ── #fuzz pragma tests ─────────────────────────────────────────

    #[test]
    fn test_parse_fuzz_single_case() {
        let s = "#fuzz(x = 5) -> 25; defn sq(x: Int) -> Int { term x * x; };";
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "fuzz + defn should parse: {:?}", result.err());
        let program = result.unwrap();
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            TopLevel::Fuzzed { item: inner, cases } => {
                assert_eq!(cases.len(), 1);
                assert_eq!(cases[0].bindings.len(), 1);
                assert_eq!(cases[0].bindings[0].0, "x");
                match &inner.as_ref() {
                    TopLevel::Definition(defn) => assert_eq!(defn.name, "sq"),
                    _ => panic!("Expected Definition inside Fuzzed"),
                }
            }
            other => panic!("Expected Fuzzed, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_fuzz_multiple_cases() {
        let s = "#fuzz(x = 0) -> 0; #fuzz(x = 1) -> 1; #fuzz(x = 2) -> 4; defn sq(x: Int) -> Int { term x * x; };";
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "multiple fuzz should parse: {:?}", result.err());
        let program = result.unwrap();
        match &program.items[0] {
            TopLevel::Fuzzed { cases, .. } => {
                assert_eq!(cases.len(), 3);
                assert_eq!(cases[0].bindings[0].0, "x");
                assert_eq!(cases[1].bindings[0].0, "x");
                assert_eq!(cases[2].bindings[0].0, "x");
            }
            _ => panic!("Expected Fuzzed with 3 cases"),
        }
    }

    #[test]
    fn test_parse_fuzz_named_params_out_of_order() {
        let s = "#fuzz(b = 2, a = 1) -> 3; defn add(a: Int, b: Int) -> Int { term a + b; };";
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "fuzz with out-of-order params should parse: {:?}", result.err());
        let program = result.unwrap();
        match &program.items[0] {
            TopLevel::Fuzzed { cases, .. } => {
                assert_eq!(cases[0].bindings.len(), 2);
                assert_eq!(cases[0].bindings[0].0, "b");
                assert_eq!(cases[0].bindings[1].0, "a");
            }
            _ => panic!("Expected Fuzzed"),
        }
    }

    #[test]
    fn test_parse_fuzz_on_txn() {
        let s = "#fuzz(v = 10) -> 20; txn add(v: Int) -> Int { &result = result + v; term result; };";
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "fuzz + txn should parse: {:?}", result.err());
        let program = result.unwrap();
        match &program.items[0] {
            TopLevel::Fuzzed { item: inner, .. } => {
                match inner.as_ref() {
                    TopLevel::Transaction(txn) => assert_eq!(txn.name, "add"),
                    _ => panic!("Expected Transaction inside Fuzzed"),
                }
            }
            _ => panic!("Expected Fuzzed"),
        }
    }

    #[test]
    fn test_parse_fuzz_on_inop() {
        let s = "#fuzz(a = 10, b = 2) -> 5; inop! div(a: Int, b: Int) -> Int [b != 0][true] (%state) { %r = sdiv i64 %a, %b; term %r; };";
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "fuzz + inop should parse: {:?}", result.err());
        let program = result.unwrap();
        match &program.items[0] {
            TopLevel::Fuzzed { item: inner, .. } => {
                match inner.as_ref() {
                    TopLevel::Inop(inop) => assert_eq!(inop.name, "div"),
                    _ => panic!("Expected Inop inside Fuzzed"),
                }
            }
            _ => panic!("Expected Fuzzed"),
        }
    }

    #[test]
    fn test_parse_fuzz_with_test_modifier() {
        let s = "#test(\"group1\") #fuzz(x = 5) -> 25; defn sq(x: Int) -> Int { term x * x; };";
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "test + fuzz + defn should parse: {:?}", result.err());
        let program = result.unwrap();
        match &program.items[0] {
            TopLevel::Test { item: inner, groups } => {
                assert_eq!(groups, &vec!["group1".to_string()]);
                match inner.as_ref() {
                    TopLevel::Fuzzed { cases, .. } => {
                        assert_eq!(cases.len(), 1);
                    }
                    other => panic!("Expected Fuzzed inside Test, got {:?}", std::mem::discriminant(other)),
                }
            }
            other => panic!("Expected Test wrapping Fuzzed, got {:?}", std::mem::discriminant(other)),
        }
    }
} // end parser_tests

// ── Type slot syntax tests ──────────────────────────────────────────

#[test]
fn test_parse_typedef_slot_syntax_proper() {
    let s = "type MyStruct <: Bits { x: Int; y: Int; };";
    let mut parser = Parser::new(s);
    let result = parser.parse();
    assert!(result.is_ok(), "Should parse slot syntax: {:?}", result.err());
    if let TopLevel::TypeDef(td) = &result.unwrap().items[0] {
        assert_eq!(td.name, "MyStruct");
        assert_eq!(td.body.slots.len(), 2);
        assert_eq!(td.body.slots[0].name, "x");
        assert_eq!(td.body.slots[1].name, "y");
    } else {
        panic!("Expected TypeDef");
    }
}

#[test]
fn test_parse_typedef_slot_syntax_with_param_type_proper() {
    let s = "type MyStruct <: Bits { ptr: Ptr<UInt8>; len: Int; };";
    let mut parser = Parser::new(s);
    let result = parser.parse();
    assert!(result.is_ok(), "Should parse slot with Ptr<UInt8>: {:?}", result.err());
    if let TopLevel::TypeDef(td) = &result.unwrap().items[0] {
        assert_eq!(td.body.slots.len(), 2);
        assert_eq!(td.body.slots[0].name, "ptr");
        assert_eq!(td.body.slots[1].name, "len");
        assert!(matches!(&td.body.slots[0].ty, crate::ast::Type::Applied(name, _) if name == "Ptr"));
    } else {
        panic!("Expected TypeDef");
    }
}

#[test]
fn test_parse_typedef_mixed_slots_and_bindings_proper() {
    let s = "type MyStruct <: Bits { x: Int; y: Int; bytes <~ 16; };";
    let mut parser = Parser::new(s);
    let result = parser.parse();
    assert!(result.is_ok(), "Should parse mixed slots+bindings: {:?}", result.err());
    if let TopLevel::TypeDef(td) = &result.unwrap().items[0] {
        assert_eq!(td.body.slots.len(), 2);
        assert_eq!(td.body.bindings.len(), 1);
        assert_eq!(td.body.bindings[0].name, "bytes");
    } else {
        panic!("Expected TypeDef");
    }
}

#[test]
fn test_parse_typedef_slot_error_with_params_proper() {
    let s = "type MyStruct <: Bits { x(a): Int; };";
    let mut parser = Parser::new(s);
    let result = parser.parse();
    assert!(result.is_err(), "Slot with params should fail to parse");
}

#[test]
fn test_parse_typedef_slot_between_bindings_and_constraints_proper() {
    let s = "type MyStruct <: Bits { bytes <~ 16; x: Int; [x > 0] }";
    let mut parser = Parser::new(s);
    let result = parser.parse();
    assert!(result.is_ok(), "Should parse slot between binding and constraint: {:?}", result.err());
    if let TopLevel::TypeDef(td) = &result.unwrap().items[0] {
        assert_eq!(td.body.slots.len(), 1);
        assert_eq!(td.body.bindings.len(), 1);
        assert_eq!(td.body.constraints.len(), 1);
    } else {
        panic!("Expected TypeDef");
    }
}

#[test]
fn test_parse_typedef_bits_bitrange_again() {
    let s = "type MyInt <: Bits @/0..63 { Bytes <~ 8; };";
    let mut parser = Parser::new(s);
    let result = parser.parse();
    assert!(result.is_ok(), "Should parse Bits @/0..63 base: {:?}", result.err());
    if let TopLevel::TypeDef(td) = &result.unwrap().items[0] {
        assert_eq!(td.bit_range, Some(crate::ast::BitRange::Range(0, 63)));
    } else {
        panic!("Expected TypeDef");
    }
}

enum BracketElement {
    Start,
    AfterStart,
    End,
    AfterEnd,
    Stride,
    AfterStride,
    Mask,
    AfterMask,
}

enum BracketContents {
    Empty,
    SimpleIndex(Box<Expr>),
    Slice {
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
        stride: Option<Box<Expr>>,
        mask: Option<Box<Expr>>,
    },
}

#[cfg(test)]
mod within_tests {
    use super::*;

    #[test]
    fn test_within_expression() {
        let s = "let x: Int = foo() within 10 cycles (3) ~? 0;";
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse within: {:?}", result.err());
    }

    #[test]
    fn test_within_chained() {
        let s = "let x: Int = a() within 10 cyc (2) ~? b() within 5 cyc (1) ~? 0;";
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse chained within: {:?}", result.err());
    }

    #[test]
    fn test_within_retry_keyword() {
        let s = "let x: Int = a() within 10 seconds retry 3 ~? 0;";
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse retry kw: {:?}", result.err());
    }

    #[test]
    fn test_within_ms_unit() {
        let s = "let x: Int = a() within 500 ms (5) ~? -1;";
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse ms: {:?}", result.err());
    }
}

#[cfg(all(feature = "kani", feature = "kani_full"))]
mod kani_full_tests {
    use super::*;
    use crate::features::literal::LiteralExpr;

    #[kani::proof]
    fn verify_parse_literal_integer() {
        let mut parser = Parser::new("42");
        let result = parser.parse_expression();
        assert!(result.is_ok());
        let expr = result.unwrap();
        assert!(matches!(expr, Expr::Literal(_)));
        assert_eq!(expr.as_integer(), Some(42));
    }

    #[kani::proof]
    fn verify_parse_literal_bool_true() {
        let mut parser = Parser::new("true");
        let result = parser.parse_expression();
        assert!(result.is_ok());
        let expr = result.unwrap();
        assert_eq!(expr.as_bool(), Some(true));
    }

    #[kani::proof]
    fn verify_parse_literal_bool_false() {
        let mut parser = Parser::new("false");
        let result = parser.parse_expression();
        assert!(result.is_ok());
        let expr = result.unwrap();
        assert_eq!(expr.as_bool(), Some(false));
    }

    #[kani::proof]
    fn verify_parse_literal_float() {
        let mut parser = Parser::new("3.14");
        let result = parser.parse_expression();
        assert!(result.is_ok());
        let expr = result.unwrap();
        assert!(matches!(expr, Expr::Literal(_)));
    }

    #[kani::proof]
    fn verify_parse_literal_string() {
        let mut parser = Parser::new("\"hello\"");
        let result = parser.parse_expression();
        assert!(result.is_ok());
        let expr = result.unwrap();
        assert!(matches!(expr, Expr::Literal(_)));
        assert_eq!(expr.as_string(), Some(&"hello".to_string()));
    }

    #[kani::proof]
    fn verify_parse_contract_validation_uses_as_bool() {
        // Contract syntax: [true][true] should fail at parse time
        // because as_bool() == Some(true) detects trivial contracts
        let mut parser = Parser::new("defn foo()[true][true] { } ;");
        let result = parser.parse_definition();
        assert!(result.is_err());
    }

    // ── Intrinsic call parsing tests ────────────────────────────

    #[test]
    fn test_parse_sqrt_intrinsic() {
        let s = r#"defn f(x: Float) -> Float { term sqrt#(x); };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse sqrt#(x): {:?}", result.err());
        if let TopLevel::Definition(defn) = &result.unwrap().items[0] {
            match &defn.body[0] {
                Statement::Term { values, .. } => {
                    let expr = values[0].as_ref().unwrap();
                    assert!(matches!(expr, Expr::IntrinsicCall { intrinsic: Intrinsic::Sqrt, .. }));
                }
                _ => panic!("Expected Term"),
            }
        } else {
            panic!("Expected Definition");
        }
    }

    #[test]
    fn test_parse_abs_intrinsic() {
        let s = r#"defn f(x: Int) -> Int { term abs#(x); };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse abs#(x): {:?}", result.err());
        if let TopLevel::Definition(defn) = &result.unwrap().items[0] {
            match &defn.body[0] {
                Statement::Term { values, .. } => {
                    let expr = values[0].as_ref().unwrap();
                    assert!(matches!(expr, Expr::IntrinsicCall { intrinsic: Intrinsic::Abs, .. }));
                }
                _ => panic!("Expected Term"),
            }
        } else {
            panic!("Expected Definition");
        }
    }

    #[test]
    fn test_parse_intrinsic_multiple_args() {
        let s = r#"defn f(xs: List<Int>, x: Int) -> Bool { term contains#(xs, x); };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse contains#(xs, x): {:?}", result.err());
        if let TopLevel::Definition(defn) = &result.unwrap().items[0] {
            match &defn.body[0] {
                Statement::Term { values, .. } => {
                    let expr = values[0].as_ref().unwrap();
                    if let Expr::IntrinsicCall { intrinsic, args } = expr {
                        assert_eq!(intrinsic, &Intrinsic::Contains);
                        assert_eq!(args.len(), 2);
                    } else {
                        panic!("Expected IntrinsicCall");
                    }
                }
                _ => panic!("Expected Term"),
            }
        } else {
            panic!("Expected Definition");
        }
    }

    #[test]
    fn test_parse_unknown_intrinsic_errors() {
        let s = r#"defn f(x: Int) -> Int { term foobar#(x); };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_err(), "foobar# should produce a parse error");
    }

    #[test]
    fn test_parse_hash_not_intrinsic_without_paren() {
        // `#volatile` after an expression should NOT be consumed as intrinsic
        let s = r#"txn Foo [true][n >= 0] { let x: Int @ some_ptr #volatile = 0; term; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse #volatile modifier: {:?}", result.err());
    }

    #[test]
    fn test_parse_intrinsic_in_txn_body() {
        let s = r#"txn Foo [true][true] { let x: Float = sqrt#(9.0); term; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse sqrt# in txn body: {:?}", result.err());
        if let TopLevel::Transaction(txn) = &result.unwrap().items[0] {
            match &txn.body[0] {
                Statement::Let { expr: Some(e), .. } => {
                    assert!(matches!(e, Expr::IntrinsicCall { intrinsic: Intrinsic::Sqrt, .. }));
                }
                _ => panic!("Expected Let with intrinsic"),
            }
        } else {
            panic!("Expected Transaction");
        }
    }

    #[test]
    fn test_parse_import_magic() {
        let s = r#"import# "std/core/ptr.bv"; defn main -> Int { term 42; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "import# should parse: {:?}", result.err());
        if let TopLevel::Import(imp) = &result.unwrap().items[0] {
            assert!(imp.is_magic, "import# should set is_magic = true");
            assert_eq!(imp.path.join("/"), "std/core/ptr.bv");
        } else {
            panic!("Expected Import");
        }
    }

    #[test]
    fn test_parse_import_normal() {
        let s = r#"import "test_module"; defn main -> Int { term 42; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "import (without #) should parse: {:?}", result.err());
        if let TopLevel::Import(imp) = &result.unwrap().items[0] {
            assert!(!imp.is_magic, "import without # should set is_magic = false");
            assert_eq!(imp.path.join("/"), "test_module");
        } else {
            panic!("Expected Import");
        }
    }

    #[test]
    fn test_parse_import_magic_with_items() {
        let s = r#"import# { greet } from "std/core/test.bv"; defn main -> Int { term 42; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "import# with items should parse: {:?}", result.err());
        if let TopLevel::Import(imp) = &result.unwrap().items[0] {
            assert!(imp.is_magic, "import# should set is_magic = true");
            assert!(!imp.items.is_empty(), "should have import items");
            assert_eq!(imp.items[0].name, "greet");
        } else {
            panic!("Expected Import");
        }
    }

    #[test]
    fn test_parse_import_magic_glob() {
        let s = r#"import# "std/core/*"; defn main -> Int { term 42; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "import# with glob should parse: {:?}", result.err());
        if let TopLevel::Import(imp) = &result.unwrap().items[0] {
            assert!(imp.is_magic, "import# should set is_magic = true");
            assert_eq!(imp.path.join("/"), "std/core/*");
        } else {
            panic!("Expected Import");
        }
    }

    #[test]
    fn test_parse_frgn_pipe_literal() {
        let s = "frgn read_file(path: String) -> String | \"\" ;";
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "frgn with pipe literal should parse: {:?}", result.err());
        if let TopLevel::ForeignBinding { signature, .. } = &result.unwrap().items[0] {
            assert!(signature.is_pipe, "should be marked as pipe");
            assert!(signature.fallback.is_some(), "should have fallback expression");
            assert_eq!(signature.success_output.len(), 1);
            assert_eq!(signature.success_output[0].1, Type::Custom("String".to_string()));
        } else {
            panic!("Expected ForeignBinding");
        }
    }

    #[test]
    fn test_parse_frgn_pipe_constructor() {
        let s = "frgn get_value() -> String | Error(\"not found\") from \"libtest.so\";";
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "frgn with pipe constructor should parse: {:?}", result.err());
        if let TopLevel::ForeignBinding { signature, .. } = &result.unwrap().items[0] {
            assert!(signature.is_pipe, "should be marked as pipe");
            assert!(signature.fallback.is_some(), "should have fallback expression");
            assert_eq!(signature.success_output[0].1, Type::Custom("String".to_string()));
            assert_eq!(signature.location, "libtest.so");
        } else {
            panic!("Expected ForeignBinding");
        }
    }

    #[test]
    fn test_parse_frgn_pipe_with_from() {
        let s = "frgn get_int(x: Int) -> Int | 0 from \"libc.so\";";
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "frgn with pipe + from should parse: {:?}", result.err());
        if let TopLevel::ForeignBinding { signature, .. } = &result.unwrap().items[0] {
            assert!(signature.is_pipe, "should be marked as pipe");
            assert!(signature.fallback.is_some(), "should have fallback expression");
            assert_eq!(signature.location, "libc.so");
        } else {
            panic!("Expected ForeignBinding");
        }
    }

    #[test]
    fn test_parse_frgn_pipe_does_not_break_plain() {
        let s = "frgn plain_fn(x: Int) -> Int;";
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "plain frgn should still parse: {:?}", result.err());
        if let TopLevel::ForeignBinding { signature, .. } = &result.unwrap().items[0] {
            assert!(!signature.is_pipe, "plain frgn should not be pipe");
            assert!(signature.fallback.is_none(), "plain frgn should have no fallback");
        } else {
            panic!("Expected ForeignBinding");
        }
    }

    #[test]
    fn test_parse_frgn_pipe_does_not_break_result() {
        let s = "frgn result_fn(x: Int) -> Result<String, IoError> from \"std::test\";";
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Result<T,E> frgn should still parse: {:?}", result.err());
        if let TopLevel::ForeignBinding { signature, .. } = &result.unwrap().items[0] {
            assert!(!signature.is_pipe, "Result<T,E> frgn should not be pipe");
        } else {
            panic!("Expected ForeignBinding");
        }
    }

    #[test]
    fn test_parse_intrinsic_all_names() {
        for (name, intrinsic) in [
            ("sqrt", Intrinsic::Sqrt),
            ("fabs", Intrinsic::Fabs),
            ("ceil", Intrinsic::Ceil),
            ("floor", Intrinsic::Floor),
            ("ctpop", Intrinsic::Ctpop),
            ("ctlz", Intrinsic::Ctlz),
            ("cttz", Intrinsic::Cttz),
            ("abs", Intrinsic::Abs),
            ("bitreverse", Intrinsic::Bitreverse),
            ("byte_count", Intrinsic::ByteCount),
            ("str_bytes", Intrinsic::StrBytes),
            ("size", Intrinsic::Size),
            ("pop", Intrinsic::Pop),
            ("contains", Intrinsic::Contains),
            ("keys", Intrinsic::Keys),
            ("values", Intrinsic::Values),
            ("strlen", Intrinsic::Strlen),
        ] {
            let s = format!("defn f() -> Int {{ term {name}#(0); }};");
            let mut parser = Parser::new(&s);
            let result = parser.parse();
            assert!(result.is_ok(), "Should parse {name}#(): {:?}", result.err());
            if let TopLevel::Definition(defn) = &result.unwrap().items[0] {
                match &defn.body[0] {
                    Statement::Term { values, .. } => {
                        let expr = values[0].as_ref().unwrap();
                        assert!(matches!(expr, Expr::IntrinsicCall { intrinsic: i, .. } if *i == intrinsic),
                            "Expected {name} intrinsic, got {:?}", expr);
                    }
                    _ => panic!("Expected Term for {name}"),
                }
            }
        }
    }

    #[test]
    fn test_parse_await_expr() {
        let s = r#"txn Test [true][true] { await compute(x); };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse await");
        if let TopLevel::Transaction(txn) = &result.unwrap().items[0] {
            assert!(matches!(&txn.body[0], Statement::Await { .. }));
        } else {
            panic!("Expected Transaction");
        }
    }

    #[test]
    fn test_parse_async_expr() {
        let s = r#"txn Test [true][true] { async compute(x); };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse async");
        if let TopLevel::Transaction(txn) = &result.unwrap().items[0] {
            assert!(matches!(&txn.body[0], Statement::Async { .. }));
        } else {
            panic!("Expected Transaction");
        }
    }

    #[test]
    fn test_parse_async_await_expr() {
        let s = r#"txn Test [true][true] { async await compute(x); };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse async await");
        if let TopLevel::Transaction(txn) = &result.unwrap().items[0] {
            match &txn.body[0] {
                Statement::AsyncAwait { lhs, .. } => assert!(lhs.is_none()),
                _ => panic!("Expected AsyncAwait with no lhs"),
            }
        } else {
            panic!("Expected Transaction");
        }
    }

    #[test]
    fn test_parse_async_await_let() {
        let s = r#"txn Test [true][true] { async await let r = compute(x); };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse async await let");
        if let TopLevel::Transaction(txn) = &result.unwrap().items[0] {
            match &txn.body[0] {
                Statement::AsyncAwait { lhs, .. } => assert_eq!(lhs.as_deref(), Some("r")),
                _ => panic!("Expected AsyncAwait with lhs=r"),
            }
        } else {
            panic!("Expected Transaction");
        }
    }

    #[test]
    fn test_parse_typedef_underscore_bitrange() {
        let s = "type MyType <: Bits { Field <~ _ @/0..7; };";
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse _ @/ in TypeDef: {:?}", result.err());
        if let TopLevel::TypeDef(td) = &result.unwrap().items[0] {
            assert_eq!(td.name, "MyType");
            assert_eq!(td.body.bindings.len(), 1);
            assert_eq!(td.body.bindings[0].name, "Field");
            let val = &td.body.bindings[0].value;
            assert!(matches!(val.as_ref(),
                Expr::Projection { source, target: ProjectionTarget::BitRange(_) }
                if matches!(source.as_ref(), Expr::Identifier(name) if name == "_")
            ), "Expected Projection(Identifier(_), BitRange), got {:?}", val);
        } else {
            panic!("Expected TypeDef");
        }
    }

    #[test]
    fn test_parse_expr_bitrange_after_identifier() {
        let s = "defn f() -> Int { term x @/0..7; };";
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse x @/0..7: {:?}", result.err());
    }

} // end parser_tests
