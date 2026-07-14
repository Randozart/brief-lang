// ── Definition/Transaction/Cell Parser ─────────────────────────────────
// 2026-07-12: Phase 1.2 — Parse top-level declarations.
// Flat code: each function is max 2 levels.
// Handles: defn, txn, rct txn, cell, export, import, meld, trg.
// Also handles [#] entry contracts, derivation blocks :=, implicit entry wrapping.

use super::helpers::Parser;
use crate::ast::*;
use crate::errors::{Span, SyntaxError};
use crate::lexer::Token;

impl<'a> Parser<'a> {
    /// Parse a top-level item: defn, txn, cell, import, etc.
    pub fn parse_top_level(&mut self) -> Result<TopLevel, SyntaxError> {
        if self.eat_identifier("export") {
            return self.parse_export();
        }
        match self.peek() {
            Some(Token::Defn) => self.parse_definition().map(TopLevel::Definition),
            Some(Token::Txn) => self
                .parse_transaction(false, false)
                .map(TopLevel::Transaction),
            Some(Token::Rct) => self.parse_reactive_transaction().map(TopLevel::Transaction),
            Some(Token::Cell) => self.parse_cell().map(TopLevel::Cell),
            Some(Token::Import) => self.parse_import().map(TopLevel::Import),
            Some(Token::Meld) => self.parse_meld().map(TopLevel::Meld),
            Some(Token::Trg) => self.parse_top_level_trg().map(TopLevel::Trigger),
            // 2026-07-14: Handle `type Name <: Parent { slots }` definitions
            Some(Token::Type) => self.parse_type_definition().map(TopLevel::TypeDef),
            // 2026-07-14: Top-level let — state variable declaration
            Some(Token::Let) => {
                let stmt = self.parse_let_statement()?;
                Ok(TopLevel::Statement(Box::new(stmt)))
            }
            // 2026-07-14: Top-level const — compile-time constant
            Some(Token::Const) => {
                Ok(TopLevel::Constant(self.parse_const_declaration()?))
            }
            _ => {
                let name = self.expect_identifier()?;
                self.error_at_current(&format!("unexpected top-level item '{}'", name))
            }
        }
    }

    /// parse top-level items until EOF.
    pub fn parse_program(&mut self) -> Result<Vec<TopLevel>, SyntaxError> {
        let mut items = Vec::new();
        while !self.is_at_end() {
            // 2026-07-14: Eat semicolons between top-level items (e.g. `defn foo() {};`)
            while self.eat(&Token::Semicolon) {}
            if self.is_at_end() {
                break;
            }
            let item = self.parse_top_level()?;
            items.push(item);
        }
        // Implicit entry wrapping: if no explicit [#] defns/txns, and we have
        // top-level statements, wrap them in an implicit transaction.
        self.wrap_implicit_entry(&mut items);
        Ok(items)
    }

    /// Parse: defn name<T>(params) -> RetType [pre][post] { body } [:= { ... }]
    // 2026-07-14: Parens are optional (mirrors parse_transaction) so that
    // `defn name -> Int { ... }` works without empty `()`. Test files and
    // the standard library use both forms.
    fn parse_definition(&mut self) -> Result<Definition, SyntaxError> {
        self.pos += 1; // consume 'defn'
        let name = self.expect_identifier()?;
        let type_params = self.parse_type_params()?;
        let parameters = if self.eat(&Token::LParen) {
            let p = self.parse_parameter_list()?;
            self.expect(Token::RParen)?;
            p
        } else {
            Vec::new()
        };
        let output_type = self.parse_output_type()?;
        let contract = self.parse_contract()?;
        let body = self.parse_block()?;
        let derivation = self.parse_derivation_block()?;
        let metadata = self.parse_body_metadata()?;
        Ok(Definition {
            name,
            type_params: type_params
                .into_iter()
                .map(|n| TypeParam {
                    name: n,
                    bound: None,
                })
                .collect(),
            parameters,
            output_type: output_type.clone(),
            outputs: vec![],
            contract,
            body,
            metadata,
            derivation,
            modifiers: vec![],
            annotations: vec![],
            span: None,
        })
    }

    /// Parse: const name: Type = expr;
    // 2026-07-14: Top-level compile-time constant declaration.
    fn parse_const_declaration(&mut self) -> Result<Constant, SyntaxError> {
        self.pos += 1; // consume 'const'
        let name = self.expect_identifier()?;
        let ty = self.parse_optional_type()?.unwrap_or(Type::int());
        self.expect(Token::Eq)?;
        let expr = self.parse_expression()?;
        self.expect(Token::Semicolon)?;
        Ok(Constant {
            name,
            ty,
            expr,
        })
    }

    /// Parse: txn name [pre][post] { body }
    fn parse_transaction(
        &mut self,
        is_reactive: bool,
        is_async: bool,
    ) -> Result<Transaction, SyntaxError> {
        self.pos += 1; // consume 'txn'
        let name = self.expect_identifier()?;
        let parameters = if self.eat(&Token::LParen) {
            let p = self.parse_parameter_list()?;
            self.expect(Token::RParen)?;
            p
        } else {
            Vec::new()
        };
        let contract = self.parse_contract()?;
        let body = self.parse_block()?;
        let derivation = self.parse_derivation_block()?;
        Ok(Transaction {
            name,
            is_reactive,
            is_async,
            type_params: vec![],
            parameters,
            output_type: None,
            outputs: Vec::new(),
            contract,
            body,
            metadata: std::collections::HashMap::new(),
            derivation,
            modifiers: vec![],
            span: None,
        })
    }

    /// Parse: rct txn name [pre][post] { body }
    fn parse_reactive_transaction(&mut self) -> Result<Transaction, SyntaxError> {
        self.pos += 1; // consume 'rct'
        // 2026-07-14: 'txn' is a keyword token (Token::Txn), not an identifier.
        // Use eat() instead of expect_identifier() to match the keyword token.
        if !self.eat(&Token::Txn) {
            let found = match self.peek() {
                Some(t) => format!("{}", t),
                None => "EOF".to_string(),
            };
            return self.error_at_current(&format!("expected 'txn' after 'rct', found '{}'", found));
        }
        let name = self.expect_identifier()?;
        let parameters = if self.eat(&Token::LParen) {
            let p = self.parse_parameter_list()?;
            self.expect(Token::RParen)?;
            p
        } else {
            Vec::new()
        };
        let contract = self.parse_contract()?;
        let body = self.parse_block()?;
        let derivation = self.parse_derivation_block()?;
        Ok(Transaction {
            name,
            is_reactive: true,
            is_async: false,
            type_params: vec![],
            parameters,
            output_type: None,
            outputs: Vec::new(),
            contract,
            body,
            metadata: std::collections::HashMap::new(),
            derivation,
            modifiers: vec![],
            span: None,
        })
    }

    /// Parse: cell name { ... }
    fn parse_cell(&mut self) -> Result<CellDef, SyntaxError> {
        self.pos += 1;
        let name = self.expect_identifier()?;
        // Cell definition details are complex — for now, parse a minimal skeleton
        self.expect(Token::LBrace)?;
        let mut transactions = Vec::new();
        let mut definitions = Vec::new();
        while !self.check(&Token::RBrace) && !self.is_at_end() {
            // Parse txn inside cell
            if self.check_identifier("txn") || self.check_identifier("rct") {
                // handled in full implementation
            }
            self.parse_toplevel_inside_cell(&mut transactions, &mut definitions)?;
        }
        self.expect(Token::RBrace)?;
        Ok(CellDef {
            name,
            type_params: vec![],
            parameters: vec![],
            output_type: None,
            fields: vec![],
            transactions,
            definitions,
            internal_triggers: vec![],
            is_persistent: false,
            metadata: std::collections::HashMap::new(),
            span: None,
        })
    }

    /// Parse items inside a cell body.
    fn parse_toplevel_inside_cell(
        &mut self,
        _txns: &mut Vec<Transaction>,
        _defns: &mut Vec<Definition>,
    ) -> Result<(), SyntaxError> {
        // Simplified: skip unknown tokens inside cell
        let _ = self.advance();
        Ok(())
    }

    /// Parse: export defn ...
    fn parse_export(&mut self) -> Result<TopLevel, SyntaxError> {
        let inner = self.parse_top_level()?;
        Ok(TopLevel::Export(Export {
            inner: Box::new(inner),
            export_name: None,
        }))
    }

    /// Parse: import "module" or import sym from "module"
    fn parse_import(&mut self) -> Result<Import, SyntaxError> {
        self.pos += 1;
        if self.eat(&Token::LParen) {
            // Import with symbols: import { a, b } from "module"
            let mut symbols = Vec::new();
            loop {
                symbols.push(self.expect_identifier()?);
                if !self.eat(&Token::Comma) {
                    break;
                }
            }
            self.expect(Token::RParen)?;
            self.eat_identifier("from");
            let module = self.expect_string()?;
            self.expect(Token::Semicolon)?;
            return Ok(Import {
                module,
                symbols,
                span: None,
            });
        }
        if matches!(self.peek(), Some(Token::String(_))) {
            // Simple import: import "module"
            let module = self.expect_string()?;
            self.expect(Token::Semicolon)?;
            return Ok(Import {
                module,
                symbols: vec![],
                span: None,
            });
        }
        // Import with symbols: import sym from "module"
        let first = self.expect_identifier()?;
        if self.eat_identifier("from") {
            let module = self.expect_string()?;
            self.expect(Token::Semicolon)?;
            Ok(Import {
                module,
                symbols: vec![first],
                span: None,
            })
        } else {
            let mut symbols = vec![first];
            loop {
                if !self.eat(&Token::Comma) {
                    break;
                }
                symbols.push(self.expect_identifier()?);
            }
            self.eat_identifier("from");
            let module = self.expect_string()?;
            self.expect(Token::Semicolon)?;
            Ok(Import {
                module,
                symbols,
                span: None,
            })
        }
    }

    /// Parse: meld name -> target;
    fn parse_meld(&mut self) -> Result<Meld, SyntaxError> {
        self.pos += 1;
        let name = self.expect_identifier()?;
        self.expect(Token::Arrow)?;
        let target = self.expect_identifier()?;
        self.expect(Token::Semicolon)?;
        Ok(Meld {
            name,
            target,
            bindings: std::collections::HashMap::new(),
            span: None,
        })
    }

    /// Parse top-level trg binding: trg name @ type.port;
    fn parse_top_level_trg(&mut self) -> Result<Trigger, SyntaxError> {
        self.pos += 1;
        let name = self.expect_identifier()?;
        self.expect(Token::At)?;
        let instance = self.parse_expression()?;
        self.expect(Token::Dot)?;
        let port = self.expect_identifier()?;
        self.expect(Token::Semicolon)?;
        Ok(Trigger {
            name,
            instance,
            port,
            span: None,
        })
    }

    // ── Shared parsing helpers ──────────────────────────────────

    /// Parse parameter list: name: Type, name: Type, ...
    fn parse_parameter_list(&mut self) -> Result<Vec<(String, Type)>, SyntaxError> {
        let mut params = Vec::new();
        if !self.check(&Token::RParen) {
            loop {
                let name = self.expect_identifier()?;
                let ty = self.parse_optional_type()?.unwrap_or(Type::int());
                params.push((name, ty));
                if !self.eat(&Token::Comma) {
                    break;
                }
            }
        }
        Ok(params)
    }

    /// Parse optional output type: -> Type
    fn parse_output_type(&mut self) -> Result<Option<OutputType>, SyntaxError> {
        if self.eat(&Token::Arrow) {
            let ty = self.parse_type()?;
            Ok(Some(OutputType::single(ty)))
        } else {
            Ok(None)
        }
    }

    /// Parse contract: [#], [pre][post], [[post], [pre]]
    fn parse_contract(&mut self) -> Result<Contract, SyntaxError> {
        let mut pre = Expr::Bool(true);
        let mut post = Expr::Bool(true);
        let mut is_entry = false;
        // Check for [#]
        if self.check(&Token::LBracket) {
            // Peek inside brackets for #
            // This is tricky: [#] is LBracket, Identifier("#"), RBracket
            let saved = self.pos;
            self.pos += 1; // peek past LBracket
            let is_entry_syntax = self.check_identifier("#");
            self.pos = saved; // restore

            if is_entry_syntax {
                self.pos += 1; // consume LBracket
                self.pos += 1; // consume Identifier("#")
                self.expect(Token::RBracket)?;
                is_entry = true;
                // Optional postcondition: [#][post]
                if self.check(&Token::LBracket) {
                    post = self.parse_single_contract_condition()?;
                }
                return Ok(Contract {
                    pre_condition: pre,
                    post_condition: post,
                    is_entry,
                    watchdog: None,
                    span: None,
                });
            }
        }
        // Parse: [pre] if present
        if self.check(&Token::LBracket) {
            pre = self.parse_single_contract_condition()?;
        }
        // Parse: [post] if present
        if self.check(&Token::LBracket) {
            post = self.parse_single_contract_condition()?;
        }
        Ok(Contract {
            pre_condition: pre,
            post_condition: post,
            is_entry,
            watchdog: None,
            span: None,
        })
    }

    /// Parse a single contract condition: [expr]
    fn parse_single_contract_condition(&mut self) -> Result<Expr, SyntaxError> {
        self.pos += 1; // consume '['
        let expr = self.parse_expression()?;
        self.expect(Token::RBracket)?;
        Ok(expr)
    }

    /// Parse optional derivation block: := { ... }
    fn parse_derivation_block(&mut self) -> Result<Option<DerivationBlock>, SyntaxError> {
        if !self.eat(&Token::ColonEq) {
            return Ok(None);
        }
        self.expect(Token::LBrace)?;
        let mut examples = Vec::new();
        while !self.check(&Token::RBrace) && !self.is_at_end() {
            let example = self.parse_derivation_example()?;
            examples.push(example);
            self.eat(&Token::Semicolon);
        }
        self.expect(Token::RBrace)?;
        let span = Span::dummy();
        Ok(Some(DerivationBlock {
            examples,
            synthesized: None,
            span,
        }))
    }

    /// Parse a single derivation example: inputs -> output
    fn parse_derivation_example(&mut self) -> Result<DerivationExample, SyntaxError> {
        let mut inputs = Vec::new();
        loop {
            inputs.push(self.parse_expression()?);
            if self.eat(&Token::Arrow) {
                break;
            }
            self.expect(Token::Comma); // must be followed by comma or arrow
        }
        let output = Box::new(self.parse_expression()?);
        Ok(DerivationExample {
            inputs,
            output,
            span: Span::dummy(),
        })
    }

    /// Wrap top-level statements in an implicit [#] transaction if needed.
    fn wrap_implicit_entry(&self, _items: &mut Vec<TopLevel>) {
        // Placeholder: full implementation in Phase 16E
    }

    /// 2026-07-14: Parse: type Name <: Parent { slot; slot; }
    fn parse_type_definition(&mut self) -> Result<Box<TypeDef>, SyntaxError> {
        self.pos += 1;
        // 2026-07-14: Type name may be Int, Float, etc. which lex as dedicated
        // tokens not Identifier. Try expect_identifier first, then check for
        // built-in type name tokens (TypeInt, TypeFloat, etc).
        let name = match self.peek() {
            Some(Token::TypeInt) => { self.advance(); "Int".to_string() }
            Some(Token::TypeFloat) => { self.advance(); "Float".to_string() }
            Some(Token::TypeUInt) => { self.advance(); "UInt".to_string() }
            Some(Token::TypeString) => { self.advance(); "String".to_string() }
            Some(Token::TypeBool) => { self.advance(); "Bool".to_string() }
            Some(Token::TypeChar) => { self.advance(); "Char".to_string() }
            _ => self.expect_identifier()?,
        };
        let base = if self.eat(&Token::LtColon) {
            self.parse_expression()?
        } else {
            Expr::Identifier("Bits".to_string())
        };
        let mut slots = Vec::new();
        let mut metadata = std::collections::HashMap::new();
        if self.eat(&Token::LBrace) {
            while !self.check(&Token::RBrace) && !self.is_at_end() {
                let slot_name = self.expect_identifier()?;
                // 2026-07-14: Handle `primitive <~ Name` as type metadata, not a slot
                if slot_name == "primitive" && self.check(&Token::TildeArrow) {
                    self.advance();
                    let prim_name = self.expect_identifier()?;
                    self.eat(&Token::Semicolon);
                    metadata.insert("primitive".into(), PropertyValue::Identifier(prim_name));
                    continue;
                }
                // 2026-07-14: Handle `op Add <~ custom_add(#L, #R)` — type-level operation binding
                if slot_name == "op" {
                    let op_name = self.expect_identifier()?;
                    self.expect(Token::TildeArrow)?;
                    let fn_name = self.expect_identifier()?;
                    self.expect(Token::LParen)?;
                    let mut params = Vec::new();
                    while !self.check(&Token::RParen) {
                        params.push(self.expect_identifier()?);
                        if !self.check(&Token::RParen) { self.expect(Token::Comma)?; }
                    }
                    self.advance(); // consume RParen
                    self.eat(&Token::Semicolon);
                    metadata.insert(format!("op.{}", op_name), PropertyValue::Identifier(fn_name));
                    continue;
                }
                // 2026-07-14: Handle `bytes <~ 8` and `name: Type` slot syntax
                let slot_ty = if self.eat(&Token::TildeArrow) {
                    self.parse_expression()?;
                    Type::int()
                } else {
                    self.expect(Token::Colon)?;
                    self.parse_type()?
                };
                self.eat(&Token::Semicolon);
                slots.push(TypeDefSlot { name: slot_name, ty: slot_ty, bit_range: None });
            }
            self.expect(Token::RBrace)?;
        }
        Ok(Box::new(TypeDef {
            name,
            type_params: vec![],
            base: Box::new(base),
            bit_range: None,
            body: TypeDefBody {
                slots,
                metadata,
                projections: vec![],
                bindings: vec![],
                operators: vec![],
                constraints: vec![],
                span: None,
            },
            span: None,
        }))
    }
}
