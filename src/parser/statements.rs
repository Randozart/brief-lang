// ── Statement Parser ───────────────────────────────────────────────────
// 2026-07-12: Phase 1.3 — Parse statement-level constructs.
// Flat code: each function is max 2 levels.
// Handles: let, assign, term, if, guard, foreach, trg, asm, sync, return, escape, metadata.

use super::helpers::Parser;
use crate::ast::{PropertyValue, Statement};
use crate::errors::SyntaxError;
use crate::lexer::Token;

impl<'a> Parser<'a> {
    /// Parse a single statement.
    pub fn parse_statement(&mut self) -> Result<Statement, SyntaxError> {
        match self.peek() {
            Some(Token::Let) => self.parse_let_statement(),
            Some(Token::Term) => self.parse_term_statement(false),
            Some(Token::TermBang) => self.parse_term_statement(true),
            Some(Token::Escape) => self.parse_escape_statement(),
            Some(Token::Foreach) => self.parse_foreach_statement(),
            Some(Token::Trg) => self.parse_trg_binding(),
            Some(Token::Sync) => self.parse_sync_block(),
            Some(Token::LBrace) => self.parse_block_statement(),
            Some(Token::LBracket) => self.parse_guard_statement_bracket(),
            Some(Token::When) => self.parse_guard_statement_when(),
            Some(Token::Semicolon) => {
                self.pos += 1;
                Ok(Statement::Expression(crate::ast::Expr::Decimal(0)))
            }
            _ => {
                // Keywords that lex as identifiers: return, if
                if self.check_identifier("return") {
                    self.parse_return_statement()
                } else if self.check_identifier("if") {
                    self.parse_if_statement()
                } else if self.check(&Token::TildeArrow) {
                    self.parse_metadata_statement()
                } else {
                    self.parse_expression_statement()
                }
            }
        }
    }

    /// let name: Type = expr;
    pub fn parse_let_statement(&mut self) -> Result<Statement, SyntaxError> {
        self.pos += 1;
        let name = self.expect_identifier()?;
        let ty = self.parse_optional_type()?;
        let expr = if self.eat(&Token::Eq) {
            Some(self.parse_expression()?)
        } else {
            None
        };
        self.expect(Token::Semicolon)?;
        Ok(Statement::Let {
            name,
            ty,
            expr,
            modifiers: Vec::new(),
        })
    }

    /// term expr; or term! expr;
    fn parse_term_statement(&mut self, bang: bool) -> Result<Statement, SyntaxError> {
        self.pos += 1;
        // 2026-07-15: Restore swan song: term! -> expr;
        self.eat(&Token::Arrow);
        let val = if !self.check(&Token::Semicolon) && !self.check(&Token::RBrace) {
            Some(self.parse_expression()?)
        } else {
            None
        };
        self.expect(Token::Semicolon)?;
        if bang {
            Ok(Statement::TermBang(val))
        } else {
            Ok(Statement::Term(val))
        }
    }

    /// return expr;
    fn parse_return_statement(&mut self) -> Result<Statement, SyntaxError> {
        self.pos += 1; // consume 'return' identifier
        let val = if !self.check(&Token::Semicolon) {
            Some(self.parse_expression()?)
        } else {
            None
        };
        self.expect(Token::Semicolon)?;
        Ok(Statement::Return(val))
    }

    /// escape expr;
    fn parse_escape_statement(&mut self) -> Result<Statement, SyntaxError> {
        self.pos += 1;
        let val = if !self.check(&Token::Semicolon) {
            Some(self.parse_expression()?)
        } else {
            None
        };
        self.expect(Token::Semicolon)?;
        Ok(Statement::Escape(val))
    }

    /// if expr { ... } else { ... }
    fn parse_if_statement(&mut self) -> Result<Statement, SyntaxError> {
        self.pos += 1; // consume 'if'
        let cond = self.parse_expression()?;
        let then_branch = self.parse_block()?;
        let else_branch = if self.eat_identifier("else") {
            if self.check(&Token::LBrace) {
                self.parse_block()?
            } else if self.check_identifier("if") {
                let else_if = self.parse_if_statement()?;
                vec![else_if]
            } else {
                return self.error_at_current("expected '{' or 'if' after 'else'");
            }
        } else {
            Vec::new()
        };
        Ok(Statement::If(cond, then_branch, else_branch))
    }

    /// foreach(item in list) { ... }
    fn parse_foreach_statement(&mut self) -> Result<Statement, SyntaxError> {
        self.pos += 1;
        self.expect(Token::LParen)?;
        let item = self.expect_identifier()?;
        self.eat_identifier("in");
        let list = self.parse_expression()?;
        self.expect(Token::RParen)?;
        let body = self.parse_block()?;
        Ok(Statement::Foreach {
            item,
            list: Box::new(list),
            body,
        })
    }

    /// trg name @ instance.port;
    fn parse_trg_binding(&mut self) -> Result<Statement, SyntaxError> {
        self.pos += 1;
        let name = self.expect_identifier()?;
        self.expect(Token::At)?;
        let instance = self.parse_expression()?;
        self.expect(Token::Dot)?;
        let port = self.expect_identifier()?;
        self.expect(Token::Semicolon)?;
        Ok(Statement::TrgBinding {
            name,
            instance,
            port,
        })
    }

    /// asm "instruction" { clobbers };
    fn parse_inline_asm(&mut self) -> Result<Statement, SyntaxError> {
        self.pos += 1;
        let asm_string = self.expect_string()?;
        let clobbers = if self.eat(&Token::LBrace) {
            let mut cl = Vec::new();
            while !self.check(&Token::RBrace) && !self.is_at_end() {
                cl.push(self.expect_string()?);
                self.eat(&Token::Comma);
            }
            self.expect(Token::RBrace)?;
            cl
        } else {
            Vec::new()
        };
        self.expect(Token::Semicolon)?;
        Ok(Statement::InlineAsm {
            asm_string,
            clobbers,
            span: None,
        })
    }

    /// sync { ... }
    fn parse_sync_block(&mut self) -> Result<Statement, SyntaxError> {
        self.pos += 1;
        let body = self.parse_block()?;
        Ok(Statement::SyncBlock(body))
    }

    /// { stmt; stmt; ... }
    fn parse_block_statement(&mut self) -> Result<Statement, SyntaxError> {
        let stmts = self.parse_block()?;
        Ok(Statement::Block(stmts))
    }

    /// [condition] { body }
    fn parse_guard_statement_bracket(&mut self) -> Result<Statement, SyntaxError> {
        self.pos += 1; // consume '['
        let cond = self.parse_expression()?;
        self.expect(Token::RBracket)?;
        let body = self.parse_block()?;
        Ok(Statement::Guarded(cond, body))
    }

    /// when condition { body }
    fn parse_guard_statement_when(&mut self) -> Result<Statement, SyntaxError> {
        self.pos += 1; // consume 'when'
        let cond = self.parse_expression()?;
        let body = self.parse_block()?;
        Ok(Statement::Guarded(cond, body))
    }

    /// key <~ value;
    fn parse_metadata_statement(&mut self) -> Result<Statement, SyntaxError> {
        self.expect(Token::TildeArrow)?;
        let key = self.expect_identifier()?;
        let val = self.parse_metadata_value_standalone()?;
        self.expect(Token::Semicolon)?;
        Ok(Statement::MetadataAssignment(key, val))
    }

    /// Parse a single metadata value (not wrapped in a HashMap loop).
    pub fn parse_metadata_value_standalone(&mut self) -> Result<PropertyValue, SyntaxError> {
        match self.peek() {
            Some(Token::Identifier(s)) => {
                let s = s.clone();
                self.pos += 1;
                Ok(PropertyValue::Identifier(s))
            }
            Some(Token::Integer(n)) => {
                let n = *n;
                self.pos += 1;
                Ok(PropertyValue::Int(n))
            }
            Some(Token::BoolTrue) => {
                self.pos += 1;
                Ok(PropertyValue::Bool(true))
            }
            Some(Token::BoolFalse) => {
                self.pos += 1;
                Ok(PropertyValue::Bool(false))
            }
            Some(Token::String(s)) => {
                let s = s.clone();
                self.pos += 1;
                Ok(PropertyValue::String(s))
            }
            Some(Token::LBracket) => {
                self.pos += 1;
                let mut items = Vec::new();
                if !self.check(&Token::RBracket) {
                    loop {
                        items.push(self.parse_metadata_value_standalone()?);
                        if !self.eat(&Token::Comma) {
                            break;
                        }
                    }
                }
                self.expect(Token::RBracket)?;
                Ok(PropertyValue::List(items))
            }
            _ => self.error_at_current(
                "expected metadata value (identifier, int, bool, string, or list)",
            ),
        }
    }

    /// Fallback: parse as expression statement: expr;
    fn parse_expression_statement(&mut self) -> Result<Statement, SyntaxError> {
        let expr = self.parse_expression()?;
        self.expect(Token::Semicolon)?;
        Ok(Statement::Expression(expr))
    }
}
