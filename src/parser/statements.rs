// ── Statement Parser ───────────────────────────────────────────────────
// 2026-07-12: Phase 1.3 — Parse statement-level constructs.
// Flat code: each function is max 2 levels.
// Handles: let, assign, term, if, guard, foreach, trg, asm, sync, return, escape, metadata.

use super::helpers::Parser;
use crate::ast::{Expr, PropertyValue, Statement};
use crate::errors::SyntaxError;
use crate::lexer::Token;

impl<'a> Parser<'a> {
    /// Parse a single statement.
    pub fn parse_statement(&mut self) -> Result<Statement, SyntaxError> {
        match self.peek() {
            Some(Token::Let) => self.parse_let_statement(),
            // 2026-08-01 (Phase E): `vol let x` — memory-visibility modifier
            // (prefix). The let statement records the vol annotation; the
            // backend emits volatile load/store (reusing the mmio machinery).
            Some(Token::Vol) if matches!(self.tokens.get(self.pos + 1).map(|(t, _)| t), Some(Token::Let)) => {
                self.pos += 1; // consume vol
                let mut stmt = self.parse_let_statement()?;
                if let Statement::Let { modifiers, .. } = &mut stmt {
                    modifiers.push(crate::ast::Annotation {
                        name: "vol".to_string(),
                        value: None,
                    });
                }
                Ok(stmt)
            }
            // 2026-08-04 (out-observability plan): `out let x` — the variable's
            // reads/writes are liveness roots (never eliminated). Unlike `vol`,
            // it does NOT force volatile memory semantics. `out vol let x` is
            // legal (vol implies out, but both pins are recorded independently);
            // `out vol` is handled by recursing into the vol arm below.
            Some(Token::Out)
                if matches!(
                    self.tokens.get(self.pos + 1).map(|(t, _)| t),
                    Some(Token::Let) | Some(Token::Vol)
                ) =>
            {
                self.pos += 1; // consume out
                let mut stmt = self.parse_statement()?;
                if let Statement::Let { modifiers, .. } = &mut stmt {
                    modifiers.push(crate::ast::Annotation {
                        name: "out".to_string(),
                        value: None,
                    });
                }
                Ok(stmt)
            }
            Some(Token::Term) => self.parse_term_statement(false),
            Some(Token::TermBang) => self.parse_term_statement(true),
            Some(Token::Escape) => self.parse_escape_statement(),
            Some(Token::Foreach) => self.parse_foreach_statement(),
            Some(Token::Trg) => self.parse_trg_binding(),
            Some(Token::Sync) => self.parse_sync_block(),
            Some(Token::Match) => self.parse_match_statement(),
            Some(Token::LBrace) => self.parse_block_statement(),
            Some(Token::LBracket) => self.parse_guard_statement_bracket(),
            Some(Token::When) => self.parse_guard_statement_when(),
            Some(Token::Semicolon) => {
                self.pos += 1;
                Ok(Statement::Expression(crate::ast::Expr::Decimal(0)))
            }
            // 2026-08-01 (Phase 3): Discard — `<- queue;` (read discard) /
            // `~<- queue;` (destructive discard). The `&` marker is removed.
            Some(Token::ArrowLeft) | Some(Token::TildeArrowLeft) => self.parse_arrow_discard_statement(),
            _ => {
                // 2026-07-24: Skip doc comments inside blocks too
                if let Some(&Token::DocComment(_) | &Token::DocCommentBang(_)) = self.peek() {
                    self.pos += 1;
                    return self.parse_statement();
                }
                // Keywords that lex as identifiers: if, $defn, $txn
                // 2026-08-04 (remove-vestigial-return): Brief has NO `return`
                // statement (never specced, never used). Previously it parsed to
                // Statement::Return whose semantics disagreed across the
                // interpreter (continues) and the LLVM/VM backends (exits) —
                // a latent wrong-codegen hazard. Raise a helpful error instead.
                if self.check_identifier("return") {
                    let span = self
                        .peek_with_span()
                        .map(|(_, r)| self.make_span(r.clone()))
                        .unwrap_or_else(crate::errors::Span::dummy);
                    self.pos += 1; // consume `return`
                    return Err(SyntaxError::InvalidStatement {
                        reason: "Brief has no `return` statement. To return a value \
                                 from a defn use `term <value>`; to mark a convergence \
                                 checkpoint use bare `term;`; `term!` closes the program."
                            .to_string(),
                        span,
                    });
                } else if self.check_identifier("if") {
                    self.parse_if_statement()
                } else if self.check_identifier("$defn") {
                    self.parse_inline_defn()
                } else if self.check_identifier("$txn") {
                    self.parse_inline_txn()
                } else if self.check(&Token::ExclaimArrow) {
                    self.parse_metadata_statement()
                } else if self.check_identifier("free") {
                    self.parse_lifetime_hint(true)
                } else if self.check_identifier("keep") {
                    self.parse_lifetime_hint(false)
                } else {
                    self.parse_expression_statement()
                }
            }
        }
    }

    /// let name: Type = expr;
    pub fn parse_let_statement(&mut self) -> Result<Statement, SyntaxError> {
        self.pos += 1;
        // 2026-07-25: Tuple destructuring: let (a, b) = expr;
        // Also: let _ = expr; (discard binding)
        let names = if self.eat(&Token::LParen) {
            let mut names = Vec::new();
            while !self.check(&Token::RParen) {
                if !names.is_empty() {
                    self.expect(Token::Comma)?;
                }
                names.push(self.expect_identifier()?);
            }
            self.expect(Token::RParen)?;
            names
        } else {
            vec![self.expect_identifier()?]
        };
        let ty = self.parse_optional_type()?;
        let expr = if self.eat(&Token::Eq) {
            Some(self.parse_expression()?)
        } else {
            None
        };
        self.expect(Token::Semicolon)?;
        let first = names.first().cloned().unwrap_or_default();
        Ok(Statement::Let {
            name: first,
            names,
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
        // 2026-07-25: term expr? — conditional term (desugars to when-guard).
        // Check for ? AFTER the expression (before semicolon).
        if let Some(ref expr) = val {
            if let Expr::Exists(name) = expr {
                // term fn? → when fn? { term fn; };
                self.expect(Token::Semicolon)?;
                let call = Expr::Call(name.clone(), vec![], None);
                return Ok(Statement::Guarded(
                    expr.clone(),
                    vec![Statement::Term(Some(call))],
                ));
            }
        }
        self.expect(Token::Semicolon)?;
        if bang {
            Ok(Statement::TermBang(val))
        } else {
            Ok(Statement::Term(val))
        }
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
        // 2026-08-01 (Phase 4): the `.port` is removed — whole-target form.
        self.expect(Token::Semicolon)?;
        Ok(Statement::TrgBinding {
            name,
            instance,
        })
    }

    /// `free x;` (free_hint = true) / `keep x;` (free_hint = false) — a
    /// lifetime-hint statement (2026-08-01, Phase 5).
    fn parse_lifetime_hint(&mut self, free_hint: bool) -> Result<Statement, SyntaxError> {
        self.pos += 1; // consume 'free' / 'keep'
        let name = self.expect_identifier()?;
        self.expect(Token::Semicolon)?;
        if free_hint {
            Ok(Statement::FreeHint(name))
        } else {
            Ok(Statement::KeepHint(name))
        }
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

    /// [condition]; — convergence gate (Statement::Gate)
    /// [condition] stmt; — prefix guarded single statement (Statement::Guarded)
    /// [condition] { body } — REJECTED (use `when condition { body }`)
    fn parse_guard_statement_bracket(&mut self) -> Result<Statement, SyntaxError> {
        self.pos += 1; // consume '['
        let cond = self.parse_expression()?;
        self.expect(Token::RBracket)?;
        match self.peek() {
            Some(Token::LBrace) => {
                // 2026-07-26: Hard reject — block bodies require `when` keyword.
                // The bracket prefix [cond] is for gates ([cond];) and prefix
                // guarded single statements ([cond] stmt;) only.
                self.error_at_current(
                    "block bodies require `when` keyword: use `when expr { ... }` instead of `[expr] { ... }`"
                )
            }
            Some(Token::Semicolon) => {
                // [cond]; — standalone convergence gate
                self.pos += 1; // consume ';'
                Ok(Statement::Gate(cond))
            }
            _ => {
                // [cond] stmt; — prefix guarded single statement
                let stmt = self.parse_statement()?;
                Ok(Statement::Guarded(cond, vec![stmt]))
            }
        }
    }

    /// when condition { body }
    fn parse_guard_statement_when(&mut self) -> Result<Statement, SyntaxError> {
        self.pos += 1; // consume 'when'
        let cond = self.parse_expression()?;
        let body = self.parse_block()?;
        // 2026-07-17: Same trailing semicolon fix as bracket guard.
        self.expect(Token::Semicolon)?;
        Ok(Statement::Guarded(cond, body))
    }

    /// !> key: value;
    fn parse_metadata_statement(&mut self) -> Result<Statement, SyntaxError> {
        self.expect(Token::ExclaimArrow)?;
        let key = self.expect_identifier()?;
        self.expect(Token::Colon)?;
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
                // 2026-07-18: Property function call: ring_push(#L, #R).
                // Parse as List([Identifier("ring_push"), HashL, HashR]).
                if self.eat(&Token::LParen) {
                    let mut items = vec![PropertyValue::Identifier(s)];
                    if !self.check(&Token::RParen) {
                        loop {
                            items.push(self.parse_metadata_value_standalone()?);
                            if !self.eat(&Token::Comma) { break; }
                        }
                    }
                    self.expect(Token::RParen)?;
                    return Ok(PropertyValue::List(items));
                }
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
            // 2026-07-18: Hash-prefixed compiler words for strategy op bindings
            Some(Token::HashL) => { self.pos += 1; Ok(PropertyValue::HashL) }
            Some(Token::HashR) => { self.pos += 1; Ok(PropertyValue::HashR) }
            Some(Token::HashT) => { self.pos += 1; Ok(PropertyValue::HashT) }
            _ => self.error_at_current(
                "expected metadata value (identifier, int, bool, string, list, or #L/#R/#T)",
            ),
        }
    }

    /// Discard: `<- &queue;` — pop from collection, discard result.
    /// 2026-08-01 (Phase 3): leading arrow discard — `<- value;` (read
    /// discard) or `~<- value;` (destructive discard). The old `&` fake-pointer
    /// marker is removed: `<- &queue;` is now `<- queue;`.
    fn parse_arrow_discard_statement(&mut self) -> Result<Statement, SyntaxError> {
        let consume = if self.eat(&Token::TildeArrowLeft) {
            true
        } else {
            self.pos += 1; // consume '<-'
            false
        };
        let target = self.parse_expression()?;
        self.expect(Token::Semicolon)?;
        Ok(Statement::ArrowAssign {
            target: None,
            value: Box::new(target),
            consume,
        })
    }

    /// Fallback: parse as expression statement: expr;
    /// Also handles infix `<-` for push/pop: `&queue <- value` or `x <- &queue`.
    fn parse_expression_statement(&mut self) -> Result<Statement, SyntaxError> {
        let lhs = self.parse_expression()?;
            // 2026-08-01 (Phase 3): Arrow — `dest <- value` (copy into lhs),
            // `dest ~<- value` (destructive extract). The dispatch to
            // insert/read/extract is by the op binding on each side (done in
            // the typechecker/codegen); the parser records the ArrowAssign.
            if self.eat(&Token::ArrowLeft) {
                let rhs = self.parse_expression()?;
                self.expect(Token::Semicolon)?;
                return Ok(Statement::ArrowAssign {
                    target: Some(Box::new(lhs)),
                    value: Box::new(rhs),
                    consume: false,
                });
            }
            if self.eat(&Token::TildeArrowLeft) {
                let rhs = self.parse_expression()?;
                self.expect(Token::Semicolon)?;
                return Ok(Statement::ArrowAssign {
                    target: Some(Box::new(lhs)),
                    value: Box::new(rhs),
                    consume: true,
                });
            }
        // 2026-07-24: Compound assignment += and -=.
        // x += 1 → Statement::Assign(id("x"), BinaryOp(Add, id("x"), 1))
        let compound_kind = if self.eat(&Token::PlusEq) {
            Some(crate::ast::BinaryOpKind::Add)
        } else if self.eat(&Token::MinusEq) {
            Some(crate::ast::BinaryOpKind::Sub)
        } else if self.eat(&Token::StarEq) {
            Some(crate::ast::BinaryOpKind::Mul)
        } else if self.eat(&Token::SlashEq) {
            Some(crate::ast::BinaryOpKind::Div)
        } else {
            None
        };
        if let Some(op) = compound_kind {
            let rhs = self.parse_expression()?;
            self.expect(Token::Semicolon)?;
            let value = Expr::BinaryOp(op, Box::new(lhs.clone()), Box::new(rhs));
            return Ok(Statement::Assign(lhs, value));
        }
        self.expect(Token::Semicolon)?;
        // 2026-07-17: Detect assignment at statement level. The expression
        // parser treats both `=` and `==` as BinaryOpKind::Eq (lowest
        // precedence through parse_assignment). At the statement level,
        // `identifier = expr;` must produce Statement::Assign so the backend
        // knows to emit a store for the computed value. A standalone
        // equality check as an expression statement never appears in real
        // programs — the result would be discarded.
        if let Expr::BinaryOp(crate::ast::BinaryOpKind::Eq, lhs, rhs) = &lhs {
            let target: crate::ast::Expr = (**lhs).clone();
            let value: crate::ast::Expr = (**rhs).clone();
            return Ok(Statement::Assign(target, value));
        }
        Ok(Statement::Expression(lhs))
    }

    /// $defn name(params) -> Type { body } — compile-time-only definition.
    /// 2026-07-23: Only valid inside $(Stage) blocks.
    fn parse_inline_defn(&mut self) -> Result<Statement, SyntaxError> {
        self.pos += 1;
        let defn = self.parse_definition()?;
        Ok(Statement::InlineDefn(defn))
    }

    /// $txn name(params) [pre][post] -> Type { body } — compile-time-only tx.
    /// 2026-07-23: Convergent loop with pre/post conditions.
    fn parse_inline_txn(&mut self) -> Result<Statement, SyntaxError> {
        self.pos += 1;
        let txn = self.parse_transaction(false, false)?;
        Ok(Statement::InlineTxn(txn))
    }

    /// match expr { pattern => body; pattern => body; };
    /// 2026-07-24: Compile-time pattern matching for $defn bodies.
    /// Patterns: literal integer, literal string, or wildcard.
    fn parse_match_statement(&mut self) -> Result<Statement, SyntaxError> {
        self.pos += 1; // consume 'match'
        let expr = Box::new(self.parse_expression()?);
        self.expect(Token::LBrace)?;

        let mut arms = Vec::new();
        while !self.check(&Token::RBrace) {
            // 2026-07-30: Parse | -separated patterns: 0x30 | 0x31 => body;
            let mut patterns: Vec<crate::ast::StmtMatchPattern> = Vec::new();
            loop {
                let pat = if self.eat(&Token::Underscore) {
                    crate::ast::StmtMatchPattern::Wildcard
                } else if let Some(&Token::Integer(n)) = self.peek() {
                    self.pos += 1;
                    crate::ast::StmtMatchPattern::Literal(n as i128)
                } else if let Some(&Token::String(ref s)) = self.peek() {
                    let s = s.clone();
                    self.pos += 1;
                    crate::ast::StmtMatchPattern::String(s)
                } else {
                    return self.error_at_current("expected pattern in match arm (string, integer, or _)");
                };
                patterns.push(pat);
                if !self.eat(&Token::Pipe) {
                    break;
                }
            }
            let pattern = if patterns.len() == 1 {
                patterns.into_iter().next().unwrap()
            } else {
                crate::ast::StmtMatchPattern::Multi(patterns)
            };

            self.expect(crate::lexer::Token::FatArrow)?;
            let body = self.parse_block()?;
            self.expect(Token::Semicolon)?;
            arms.push(crate::ast::StmtMatchArm { pattern, body });
        }
        self.pos += 1; // consume '}'
        self.expect(Token::Semicolon)?;
        Ok(Statement::Match { expr, arms })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;

    // 2026-08-04 (remove-vestigial-return): Brief has no `return` statement —
    // using it must fail with a helpful error pointing at `term`, not parse to
    // a Statement::Return whose semantics disagreed across engines.
    #[test]
    fn return_statement_errors_with_helpful_message() {
        for src in ["defn f(x: Int) -> Int { return x; };", "return;", "node n [a==0][a==1] { a = 1; return; };"] {
            let tokens = tokenize(src).unwrap();
            let mut p = Parser::new(tokens, src);
            let err = p.parse_program().unwrap_err();
            assert!(
                err.to_string().contains("has no `return`"),
                "expected the helpful 'has no `return`' message, got: {err}"
            );
            assert!(
                err.to_string().contains("`term <value>`"),
                "message must suggest `term <value>`, got: {err}"
            );
        }
    }

    #[test]
    fn term_statements_still_parse() {
        let src = "defn f(x: Int) -> Int { term x; };\nnode n [a==0][a==1] { a = 1; term; };";
        let tokens = tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        let program = p.parse_program().unwrap();
        assert_eq!(program.len(), 2, "both defn and node must parse");
    }
}
