// ── Definition/Transaction/Cell Parser ─────────────────────────────────
// 2026-07-12: Phase 1.2 — Parse top-level declarations.
// Flat code: each function is max 2 levels.
// Handles: defn, txn, node, cell, export, import, meld, trg.
// Also handles [#] entry contracts, derivation blocks :=, implicit entry wrapping.

use super::helpers::Parser;
use crate::ast::*;
use crate::errors::{Span, SyntaxError};
use crate::lexer::Token;

impl<'a> Parser<'a> {
    /// Parse a top-level item: defn, txn, cell, import, etc.
    pub fn parse_top_level(&mut self) -> Result<TopLevel, SyntaxError> {
        // 2026-07-16: P2 — Drain extension group leftovers first
        if let Some(item) = self.pending_types.next() {
            return Ok(item);
        }
        if self.eat(&Token::Export) {
            return self.parse_export();
        }
        match self.peek() {
            Some(Token::Defn) => self.parse_definition().map(TopLevel::Definition),
            Some(Token::Txn) => self
                .parse_transaction(false, false)
                .map(TopLevel::Transaction),
            Some(Token::Node) => self.parse_node().map(TopLevel::Transaction),
            Some(Token::Cell) => self.parse_cell().map(TopLevel::Cell),
            Some(Token::Import) => self.parse_import().map(TopLevel::Import),
            Some(Token::Meld) => self.parse_meld().map(TopLevel::Meld),
            Some(Token::Trg) => self.parse_top_level_trg().map(TopLevel::Trigger),
            // 2026-07-14: Handle `type Name <: Parent { slots }` definitions
            // 2026-07-16: P2 — Check for extension group Type.[a,b,c] before single type
            Some(Token::Type) => self.parse_type_or_group().map(TopLevel::TypeDef),
            // 2026-07-14: Handle `struct Name { fields }` as TypeDef
            Some(Token::Struct) => self.parse_struct_like().map(TopLevel::TypeDef),
            // 2026-07-14: Handle `enum Name { variants }` as TypeDef (converted by normalizer)
            Some(Token::Enum) => self.parse_enum_like().map(TopLevel::TypeDef),
            // 2026-07-14: Top-level let — state variable declaration
            Some(Token::Let) => {
                let stmt = self.parse_let_statement()?;
                Ok(TopLevel::Statement(Box::new(stmt)))
            }
            // 2026-07-14: Top-level const — compile-time constant
            Some(Token::Const) => {
                Ok(TopLevel::Constant(self.parse_const_declaration()?))
            }
            // 2026-07-15: $(Stage) compile-time metaprogramming block
            Some(Token::Dollar) => {
                // Check if the next token is LParen without consuming
                if self.tokens.get(self.pos + 1).map(|(t, _)| t) == Some(&Token::LParen) {
                    self.parse_stage_block().map(TopLevel::StageBlock)
                } else {
                    let name = self.expect_identifier()?;
                    self.error_at_current(&format!("unexpected top-level item '{}'", name))
                }
            }
            // 2026-07-16: P3 — Parse `frgn` and `frgn!` declarations
            Some(Token::Frgn) | Some(Token::FrgnBang) => {
                self.advance();
                self.parse_frgn_decl().map(TopLevel::ForeignBinding)
            }
            _ => {
                let name = self.expect_identifier()?;
                self.error_at_current(&format!("unexpected top-level item '{}'", name))
            }
        }
    }

    /// 2026-07-16: P3 — Parse `frgn` declaration.
    /// Syntax: frgn name(params) -> Ret [as sym] from "path" [fallback <expr>];
    ///         frgn name(params) -> Ret [as sym] from <name> [fallback <expr>];
    ///         frgn name(params) -> Ret [as sym] from "path" target "c" [fallback <expr>];
    /// 2026-07-22: Added `as <symbol>` and `fallback <expr>` clauses.
    fn parse_frgn_decl(&mut self) -> Result<ForeignBinding, SyntaxError> {
        let name = self.expect_identifier()?;

        // 2026-07-22: Parse optional `as <foreign_symbol>` — symbol rename, NOT a protocol hint.
        let as_name = if self.eat_identifier("as") {
            Some(self.expect_identifier()?)
        } else {
            None
        };

        self.expect(Token::LParen)?;
        let mut inputs = Vec::new();
        while !self.check(&Token::RParen) {
            let param_name = self.expect_identifier()?;
            self.expect(Token::Colon)?;
            let param_type = self.parse_type()?;
            inputs.push((param_name, param_type));
            if !self.eat(&Token::Comma) {
                break;
            }
        }
        self.expect(Token::RParen)?;
        let success_output = if self.eat(&Token::Arrow) {
            vec![(String::new(), self.parse_type()?)]
        } else {
            vec![]
        };
        let from = if self.eat(&Token::From) {
            self.parse_from_spec()?
        } else {
            FromSpec::default()
        };
        let mut target = ForeignTarget::C;
        if self.eat_identifier("target") {
            let target_str = self.expect_string()?;
            target = match ForeignTarget::from_name(&target_str) {
                Some(t) => t,
                None => {
                    let msg = format!("unknown target: {}", target_str);
                    return self.error_at_current(&msg);
                }
            };
        }

        // 2026-07-22: Parse optional `fallback <expr>` or `fallback <fn>(<args>)`.
        let fallback = if self.eat_identifier("fallback") {
            if self.check(&Token::Semicolon) {
                // fallback; — implicit void, just skip the call
                Fallback::Implicit
            } else if self.peek().map_or(false, |t| matches!(t, Token::Identifier(_))) {
                // Could be FnCall(name, args) or Static(ident)
                // Peek ahead: if next after identifier is LParen, it's a function call
                let saved = self.pos;
                let ident = self.expect_identifier()?;
                if self.eat(&Token::LParen) {
                    let mut args = Vec::new();
                    while !self.check(&Token::RParen) {
                        args.push(self.parse_expression()?);
                        if !self.eat(&Token::Comma) { break; }
                    }
                    self.expect(Token::RParen)?;
                    Fallback::FnCall(ident, args)
                } else {
                    // Single identifier as a static expression
                    // Reconstruct: we've consumed the identifier and need to reparse it as an expr
                    self.pos = saved;
                    let expr = self.parse_expression()?;
                    Fallback::Static(expr)
                }
            } else {
                let expr = self.parse_expression()?;
                Fallback::Static(expr)
            }
        } else {
            Fallback::None
        };

        self.expect(Token::Semicolon)?;
        Ok(ForeignBinding {
            name,
            as_name,
            from,
            target,
            inputs,
            success_output,
            error_type: "Error".to_string(),
            error_fields: vec![],
            input_layout: None,
            output_layout: None,
            precondition: None,
            postcondition: None,
            buffer_mode: None,
            default_watchdog: None,
            wasm_impl: None,
            wasm_setup: None,
            fallback,
            span: None,
        })
    }

    /// 2026-07-16: P3 — Parse `from "path"` or `from <name>` after `from` token is consumed.
    /// 2026-07-16: P3 — Parse `from "path"` or `from <name>` after `from` token is consumed.
    fn parse_from_spec(&mut self) -> Result<FromSpec, SyntaxError> {
        if self.eat(&Token::Lt) {
            // Consume all tokens until `>`, building the name string.
            // Supports: <xxhash.c>, <std/io.c>, <a.b.c>
            let mut name = String::new();
            loop {
                match self.peek() {
                    Some(Token::Gt) => {
                        self.advance();
                        break;
                    }
                    Some(Token::Identifier(seg)) => {
                        name.push_str(seg);
                        self.advance();
                    }
                    Some(Token::Dot) => {
                        name.push('.');
                        self.advance();
                    }
                    Some(Token::Slash) => {
                        name.push('/');
                        self.advance();
                    }
                    other => {
                        let msg = format!("expected '>' to close compiler-relative path, found {:?}", other);
                        return self.error_at_current(&msg);
                    }
                }
            }
            Ok(FromSpec::CompilerRegistry(name))
        } else {
            let path_str = self.expect_string()?;
            Ok(FromSpec::Literal(std::path::PathBuf::from(path_str)))
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
            type_params,
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
        // 2026-07-18: Optional return type for txn: txn name(params) [pre][post] -> Type { body }
        let output_type: Option<crate::ast::OutputType> = if self.eat(&Token::Arrow) {
            Some(crate::ast::OutputType::Single(self.parse_type()?))
        } else {
            None
        };
        let body = self.parse_block()?;
        let derivation = self.parse_derivation_block()?;
        Ok(Transaction {
            name,
            is_reactive,
            is_async,
            type_params: vec![],
            parameters,
            output_type,
            outputs: Vec::new(),
            contract,
            body,
            metadata: std::collections::HashMap::new(),
            derivation,
            modifiers: vec![],
            span: None,
        })
    }

    /// Parse: node [async] name [pre][post] { body }
    /// A node is a reactive state machine — no parameters, no return value.
    /// It fires automatically when its precondition is true.
    fn parse_node(&mut self) -> Result<Transaction, SyntaxError> {
        self.pos += 1; // consume 'node'
        // 2026-07-21: Optional 'async' modifier after node keyword.
        // node async signals that the compiler should dispatch this
        // transaction in parallel when write sets are disjoint.
        let is_async = self.eat(&Token::Async);
        let name = self.expect_identifier()?;
        // node has no parameters and no return value (purely reactive)
        let contract = self.parse_contract()?;
        let body = self.parse_block()?;
        let derivation = self.parse_derivation_block()?;
        Ok(Transaction {
            name,
            is_reactive: true,
            is_async,
            type_params: vec![],
            parameters: vec![],
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
            if self.check_identifier("txn") || self.check_identifier("node") {
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
    /// 2026-07-15: Added import <name> (registry lookup) support.
    fn parse_import(&mut self) -> Result<Import, SyntaxError> {
        self.pos += 1;

        // Helper: parse a string path or angle-bracketed registry name.
        // Must be a local fn to avoid borrow conflicts with &mut self.
        fn parse_import_path(parser: &mut Parser) -> Result<ImportKind, SyntaxError> {
            if parser.eat(&Token::Lt) {
                let name = parser.expect_identifier()?;
                parser.expect(Token::Gt)?;
                Ok(ImportKind::Registry(name))
            } else {
                let path = parser.expect_string()?;
                Ok(ImportKind::Literal(path))
            }
        }

        if self.eat(&Token::LBrace) {
            // Import with symbols: import { a, b } from "module" or from <name>
            let mut symbols = Vec::new();
            loop {
                symbols.push(self.expect_identifier()?);
                if !self.eat(&Token::Comma) {
                    break;
                }
            }
            self.expect(Token::RBrace)?;
            if !self.eat(&Token::From) {
                let tok = self.advance().unwrap();
                return Err(SyntaxError::UnexpectedToken {
                    expected: "from".into(),
                    found: format!("{}", tok.0),
                    span: self.make_span(tok.1),
                });
            }
            let kind = parse_import_path(self)?;
            self.expect(Token::Semicolon)?;
            return Ok(Import {
                kind,
                symbols,
                span: None,
            });
        }

        // Check for < without LBrace: import <name>
        if self.eat(&Token::Lt) {
            let name = self.expect_identifier()?;
            self.expect(Token::Gt)?;
            self.expect(Token::Semicolon)?;
            return Ok(Import {
                kind: ImportKind::Registry(name),
                symbols: vec![],
                span: None,
            });
        }

        // Check for string: import "path"
        if matches!(self.peek(), Some(Token::String(_))) {
            let module = self.expect_string()?;
            self.expect(Token::Semicolon)?;
            return Ok(Import {
                kind: ImportKind::Literal(module),
                symbols: vec![],
                span: None,
            });
        }

        // Import with symbols: import sym from "module" or from <name>
        let first = self.expect_identifier()?;
        if self.eat_identifier("from") {
            let kind = parse_import_path(self)?;
            self.expect(Token::Semicolon)?;
            Ok(Import {
                kind,
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
            let kind = parse_import_path(self)?;
            self.expect(Token::Semicolon)?;
            Ok(Import {
                kind,
                symbols,
                span: None,
            })
        }
    }

    /// Parse: $(Stage @ priority) { body }
    /// 2026-07-15: Compile-time metaprogramming block.
    /// Stage is one of: PreLex, Parsed, Resolved, Typed, Normalized, Verified,
    /// Allocated, Provenanced, Generated, Optimized, Linked.
    /// Priority is optional, defaults to 500 (normal).
    /// Old names (Front, Mid, Post, Back) produce a clear migration error.
    fn parse_stage_block(&mut self) -> Result<StageBlock, SyntaxError> {
        self.pos += 1; // consume $
        let old_stages = &["Front", "Mid", "Post", "Back"];
        self.expect(Token::LParen)?;
        let stage_str = self.expect_identifier()?;
        if old_stages.contains(&stage_str.as_str()) {
            let hint = match stage_str.as_str() {
                "Front" => "Use $(PreLex) for source-text plugins or $(Parsed) for AST plugins",
                "Mid" => "Use $(Typed) for post-typecheck plugins",
                "Post" => "Use $(Generated) for post-codegen IR plugins",
                "Back" => "Use $(Optimized) for post-optimization plugins",
                _ => unreachable!(),
            };
            return Err(SyntaxError::InvalidExpression {
                reason: format!("stage '{}' was removed in the 2026-07-21 pipeline redesign. {}", stage_str, hint),
                span: crate::errors::Span::dummy(),
            });
        }
        let stage = match stage_str.as_str() {
            "PreLex" => StageKind::PreLex,
            "Parsed" => StageKind::Parsed,
            "Resolved" => StageKind::Resolved,
            "Typed" => StageKind::Typed,
            "Normalized" => StageKind::Normalized,
            "Verified" => StageKind::Verified,
            "Allocated" => StageKind::Allocated,
            "Provenanced" => StageKind::Provenanced,
            "Generated" => StageKind::Generated,
            "Optimized" => StageKind::Optimized,
            "Linked" => StageKind::Linked,
            _ => {
                return Err(SyntaxError::InvalidExpression {
                    reason: format!(
                        "unknown stage '{}'. Expected one of: PreLex, Parsed, Resolved, Typed, \
                         Normalized, Verified, Allocated, Provenanced, Generated, Optimized, Linked",
                        stage_str
                    ),
                    span: crate::errors::Span::dummy(),
                });
            }
        };

        // Optional priority: @ N or @ name
        let priority = if self.eat(&Token::At) {
            if let Some(Token::Integer(n)) = self.peek() {
                let p = *n as u32;
                self.pos += 1;
                p
            } else {
                let name = self.expect_identifier()?;
                match name.as_str() {
                    "highest" => 1000,
                    "high" => 750,
                    "normal" => 500,
                    "low" => 250,
                    "lowest" => 0,
                    _ => {
                        return Err(SyntaxError::InvalidExpression {
                            reason: format!(
                                "unknown priority '{}'. Expected integer or one of: \
                                 highest, high, normal, low, lowest",
                                name
                            ),
                            span: crate::errors::Span::dummy(),
                        });
                    }
                }
            }
        } else {
            500
        };

        self.expect(Token::RParen)?;
        let body = self.parse_block()?;

        Ok(StageBlock {
            stage,
            priority,
            body,
            span: None,
        })
    }

    /// Parse: meld name -> target;
    fn parse_meld(&mut self) -> Result<Meld, SyntaxError> {
        self.pos += 1;
        let name = self.expect_identifier()?;
        // 2026-07-16: -> separates the two meld types
        self.expect(Token::Arrow)?;
        let target = self.expect_identifier()?;
        // 2026-07-14: Optional body with layout { field -> field; } mappings
        let mut bindings = std::collections::HashMap::new();
        if self.eat(&Token::LBrace) {
            while !self.check(&Token::RBrace) && !self.is_at_end() {
                // Expect "layout" as the block keyword
                let keyword = self.expect_identifier()?;
                if keyword == "layout" {
                    self.expect(Token::LBrace)?;
                    while !self.check(&Token::RBrace) && !self.is_at_end() {
                        let lhs = self.expect_identifier()?;
                        // 2026-07-16: -> maps source field to target field
                        self.expect(Token::Arrow)?;
                        let rhs = self.expect_identifier()?;
                        self.eat(&Token::Semicolon);
                        bindings.insert(format!("layout.{}", lhs), rhs);
                    }
                    self.expect(Token::RBrace)?;
                    // 2026-07-16: layout { ... }; — eat the statement terminator
                    self.eat(&Token::Semicolon);
                } else {
                    return Err(SyntaxError::InvalidExpression {
                        reason: format!("expected 'layout' in meld body, got '{}'", keyword),
                        span: crate::errors::Span::new(0, 0, 0, 0),
                    });
                }
            }
            self.expect(Token::RBrace)?;
        }
        Ok(Meld {
            name,
            target,
            bindings,
            span: None,
        })
    }

    /// Parse top-level trg binding: trg name @ instance.#port;
    /// 2026-07-15: The # prefix is required for layout port access.
    fn parse_top_level_trg(&mut self) -> Result<Trigger, SyntaxError> {
        self.pos += 1;
        let name = self.expect_identifier()?;
        self.expect(Token::At)?;
        let instance = self.parse_expression()?;
        self.expect(Token::Dot)?;
        // Require # prefix for layout port access
        let port = self.expect_identifier()?;
        if !port.starts_with('#') {
            return Err(SyntaxError::InvalidExpression {
                reason: format!(
                    "trigger port '{}' must use # prefix for layout access: .#{}",
                    port, port
                ),
                span: crate::errors::Span::dummy(),
            });
        }
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
    /// 2026-07-16: P2 — Parse `type Name` or `type Name.[a,b,c]` (extension group).
    /// For groups, stores extra TypeDefs in self.pending_types for subsequent drain.
    fn parse_type_or_group(&mut self) -> Result<Box<TypeDef>, SyntaxError> {
        self.pos += 1; // consume `type` token
        // 2026-07-16: All type names are Token::Identifier after Type token removal.
        let name = self.expect_identifier()?;
        // 2026-07-20: Parse type parameters: type List<T: #String, V>
        let type_params = self.parse_type_params()?;
        // 2026-07-16: P2 — Check for .[ext, ...] extension group syntax
        if self.eat(&Token::Dot) && self.eat(&Token::LBracket) {
            return self.parse_extension_group_body(&name);
        }
        self.parse_type_body(name, type_params)
    }

    /// 2026-07-16: P2 — Parse extension group body after `Name.[` has been consumed.
    /// Expands into one TypeDef per extension, stores extras in pending_types.
    fn parse_extension_group_body(&mut self, base_name: &str) -> Result<Box<TypeDef>, SyntaxError> {
        let mut exts = Vec::new();
        loop {
            let ext = self.expect_identifier()?;
            exts.push(ext);
            if !self.eat(&Token::Comma) {
                break;
            }
        }
        self.expect(Token::RBracket)?;
        let base = if self.eat(&Token::LtColon) {
            self.parse_expression()?
        } else {
            Expr::Identifier(base_name.to_string())
        };
        let mut slots = Vec::new();
        let mut metadata = std::collections::HashMap::new();
        let mut operators: Vec<OperatorDef> = Vec::new();
        if self.eat(&Token::LBrace) {
            while !self.check(&Token::RBrace) && !self.is_at_end() {
                let slot_name = self.expect_identifier()?;
                // 2026-07-17: CTD replaces primitive; ALU is a new slot
                if slot_name == "ctd" && self.check(&Token::TildeArrow) {
                    self.advance();
                    let ctd_name = self.expect_identifier()?;
                    self.eat(&Token::Semicolon);
                    metadata.insert("ctd".into(), PropertyValue::Identifier(ctd_name));
                    continue;
                }
                if slot_name == "alu" && self.check(&Token::TildeArrow) {
                    self.advance();
                    // PascalCase identifier → known built-in ALU
                    // Lowercase quoted string → backend/plugin-specific
                    match self.peek() {
                        Some(Token::Identifier(_)) => {
                            let alu_name = self.expect_identifier()?;
                            self.eat(&Token::Semicolon);
                            metadata.insert("alu".into(), PropertyValue::Identifier(alu_name));
                        }
                        _ => {
                            let alu_str = self.expect_string()?;
                            self.eat(&Token::Semicolon);
                            metadata.insert("alu".into(), PropertyValue::String(alu_str));
                        }
                    }
                    continue;
                }
                if slot_name == "layout" && self.check(&Token::TildeArrow) {
                    self.advance();
                    // 2026-07-16: Two syntaxes: { field: Type } (struct) or <...> (angle explicit)
                    if self.check(&Token::LBrace) {
                        let fields = self.parse_layout_struct_body()?;
                        metadata.insert("layout_struct".into(), fields);
                    } else {
                        let raw = self.read_layout_body()?;
                        metadata.insert("layout".into(), PropertyValue::String(raw));
                    }
                    self.eat(&Token::Semicolon);
                    continue;
                }
                if slot_name == "op" {
                    let op_name = self.expect_identifier()?;
                    self.expect(Token::TildeArrow)?;
                    // 2026-07-18: Accept string ("int.add") or identifier (int_add)
                    // for the generic op identifier. Parenthesized param list is
                    // optional syntactic sugar (currently discarded — the generic
                    // ID uniquely identifies the operation).
                    let impl_val = if self.check(&Token::String(String::new())) {
                        self.expect_string()?
                    } else {
                        self.expect_identifier()?
                    };
                    if self.eat(&Token::LParen) {
                        // Skip parameter types (documentation only)
                        while !self.check(&Token::RParen) {
                            if self.check(&Token::Identifier(String::new())) || self.check(&Token::String(String::new())) {
                                self.advance();
                            } else { break; }
                            if !self.check(&Token::RParen) { self.eat(&Token::Comma); }
                        }
                        self.expect(Token::RParen)?;
                    }
                    self.eat(&Token::Semicolon);
                    metadata.insert(format!("op.{}", op_name), PropertyValue::String(impl_val));
                    continue;
                }
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
        if exts.is_empty() {
            return self.error_at_current("extension group cannot be empty");
        }
        let first_ext = exts.remove(0);
        let mut extra_types = Vec::new();
        for ext in exts {
            let full_name = format!("{}.{}", base_name, ext);
            extra_types.push(TopLevel::TypeDef(Box::new(TypeDef {
                name: full_name,
                type_params: vec![],
                base: Box::new(base.clone()),
                bit_range: None,
                body: TypeDefBody {
                    slots: slots.clone(),
                    metadata: metadata.clone(),
                    projections: vec![],
                    bindings: vec![],
                    operators: operators.clone(),
                    constraints: vec![],
                    span: None,
                },
                span: None,
            })));
        }
        self.pending_types = extra_types.into_iter();
        let first_name = format!("{}.{}", base_name, first_ext);
        Ok(Box::new(TypeDef {
            name: first_name,
            type_params: vec![],
            base: Box::new(base),
            bit_range: None,
            body: TypeDefBody {
                slots,
                metadata,
                projections: vec![],
                bindings: vec![],
                operators,
                constraints: vec![],
                span: None,
            },
            span: None,
        }))
    }

    /// 2026-07-16: P2 — Parse `type Name <: Parent { body }` (single type, not a group).
    /// Extracted from the original parse_type_definition body block.
    fn parse_type_body(&mut self, name: String, type_params: Vec<crate::ast::top::TypeParam>) -> Result<Box<TypeDef>, SyntaxError> {
        let base = if self.eat(&Token::LtColon) {
            self.parse_expression()?
        } else {
            Expr::Identifier("Bits".to_string())
        };
        let mut slots = Vec::new();
        let mut metadata = std::collections::HashMap::new();
        let mut operators: Vec<OperatorDef> = Vec::new();
        if self.eat(&Token::LBrace) {
            while !self.check(&Token::RBrace) && !self.is_at_end() {
                let slot_name = self.expect_identifier()?;
                // 2026-07-17: CTD replaces primitive; ALU is a new slot
                if slot_name == "ctd" && self.check(&Token::TildeArrow) {
                    self.advance();
                    let ctd_name = self.expect_identifier()?;
                    self.eat(&Token::Semicolon);
                    metadata.insert("ctd".into(), PropertyValue::Identifier(ctd_name));
                    continue;
                }
                if slot_name == "alu" && self.check(&Token::TildeArrow) {
                    self.advance();
                    // PascalCase identifier → known built-in ALU
                    // Lowercase quoted string → backend/plugin-specific
                    match self.peek() {
                        Some(Token::Identifier(_)) => {
                            let alu_name = self.expect_identifier()?;
                            self.eat(&Token::Semicolon);
                            metadata.insert("alu".into(), PropertyValue::Identifier(alu_name));
                        }
                        _ => {
                            let alu_str = self.expect_string()?;
                            self.eat(&Token::Semicolon);
                            metadata.insert("alu".into(), PropertyValue::String(alu_str));
                        }
                    }
                    continue;
                }
                if slot_name == "layout" && self.check(&Token::TildeArrow) {
                    self.advance();
                    // 2026-07-16: Two syntaxes: { field: Type } (struct) or <...> (angle explicit)
                    if self.check(&Token::LBrace) {
                        let fields = self.parse_layout_struct_body()?;
                        metadata.insert("layout_struct".into(), fields);
                    } else {
                        let raw = self.read_layout_body()?;
                        metadata.insert("layout".into(), PropertyValue::String(raw));
                    }
                    self.eat(&Token::Semicolon);
                    continue;
                }
                if slot_name == "op" {
                    self.parse_op_binding(&mut operators)?;
                    continue;
                }
                // 2026-07-20: Slot-name property binding via <~ (general metadata).
                // InsertAt/ExtractFrom are now parsed via `op InsertAt(T) = fn(#L,#R)`
                // in the op handler above. They no longer get special "op." prefix treatment.
                if self.eat(&Token::TildeArrow) {
                    let pv = self.parse_metadata_value_standalone()?;
                    self.eat(&Token::Semicolon);
                    metadata.insert(slot_name, pv);
                    continue;
                }
                self.expect(Token::Colon)?;
                let slot_ty = self.parse_type()?;
                self.eat(&Token::Semicolon);
                slots.push(TypeDefSlot { name: slot_name, ty: slot_ty, bit_range: None });
            }
            self.expect(Token::RBrace)?;
        }
        Ok(Box::new(TypeDef {
            name,
            type_params,
            base: Box::new(base),
            bit_range: None,
            body: TypeDefBody {
                slots,
                metadata,
                projections: vec![],
                bindings: vec![],
                operators,
                constraints: vec![],
                span: None,
            },
            span: None,
        }))
    }

    /// 2026-07-20: Parse an op binding within a type body.
    /// Two forms:
    ///   op Add(#Int, #Int);                                     — declarative hashword dispatch
    ///   op Add(Posit32) = posit32_add(#L, #R);                  — binding with explicit function
    fn parse_op_binding(&mut self, operators: &mut Vec<OperatorDef>) -> Result<(), SyntaxError> {
        let op_name = self.expect_identifier()?;
        self.expect(Token::LParen)?;
        self.parse_op_with_params(op_name, operators)
    }

    /// 2026-07-20: Parse op Add(#Int, #Int) or op Add(Posit32) = fn(#L, #R).
    /// Also parses optional discriminator qualifiers:
    ///   op Parse(Decimal, pre: "0x")
    ///   op Parse(Decimal, suf: "h")
    fn parse_op_with_params(&mut self, op_name: String,
                            operators: &mut Vec<OperatorDef>) -> Result<(), SyntaxError> {
        let mut params = Vec::new();
        let mut pre: Option<String> = None;
        let mut suf: Option<String> = None;
        if !self.check(&Token::RParen) {
            loop {
                let pty = self.parse_type()?;
                params.push(pty);
                // 2026-07-20: Check for discriminator qualifiers after the type
                if self.eat_identifier("pre") {
                    self.eat(&Token::Colon);
                    let val = self.expect_string()?;
                    self.validate_discriminator(&val)?;
                    pre = Some(val);
                }
                if self.eat_identifier("suf") {
                    self.eat(&Token::Colon);
                    let val = self.expect_string()?;
                    self.validate_discriminator(&val)?;
                    suf = Some(val);
                }
                if !self.eat(&Token::Comma) { break; }
            }
        }
        self.expect(Token::RParen)?;
        // Declarative: op Add(#Int, #Int);
        if self.eat(&Token::Semicolon) {
            operators.push(OperatorDef {
                op: op_name, params, pre, suf,
                impl_args: None, impl_name: String::new(), span: None,
            });
            return Ok(());
        }
        // Binding: op InsertAt(#RingBuffer, #T) = ring_push(#L, #R);
        self.expect(Token::Eq)?;
        let impl_args = self.parse_metadata_value_standalone()?;
        self.expect(Token::Semicolon)?;
        operators.push(OperatorDef {
            op: op_name, params, pre, suf,
            impl_args: Some(impl_args),
            impl_name: String::new(), span: None,
        });
        Ok(())
    }

    /// 2026-07-20: Validate a pre:/suf: discriminator string.
    /// Rejects symbols that conflict with language operators or syntax.
    fn validate_discriminator(&self, val: &str) -> Result<(), crate::errors::SyntaxError> {
        const FORBIDDEN: &[&str] = &[
            "#", "!", "@", "&", "$", "(", ")", "[", "]", "<", ">",
            "*", ",", ";", ":", "=", "~", "%", "{", "}", "\"", "'",
            "|", "\\",
        ];
        for sym in FORBIDDEN {
            if val.contains(sym) {
                return Err(crate::errors::SyntaxError::InvalidExpression {
                    reason: format!("invalid discriminator '{}': symbol '{}' is reserved by the language", val, sym),
                    span: crate::errors::Span::new(0, 0, 0, 0),
                });
            }
        }
        Ok(())
    }

    /// 2026-07-16: Parse struct-format layout body: { field: Type, ... }.
    /// Returns PropertyValue::List of [name_string, type_name_identifier] pairs.
    fn parse_layout_struct_body(&mut self) -> Result<PropertyValue, SyntaxError> {
        self.expect(Token::LBrace)?;
        let mut fields = Vec::new();
        while !self.check(&Token::RBrace) && !self.is_at_end() {
            let name = self.expect_identifier()?;
            self.expect(Token::Colon)?;
            let ty = self.parse_type()?;
            self.eat(&Token::Comma);
            self.eat(&Token::Semicolon);
            fields.push(PropertyValue::List(vec![
                PropertyValue::String(name),
                PropertyValue::Identifier(ty.to_string()),
            ]));
        }
        self.expect(Token::RBrace)?;
        Ok(PropertyValue::List(fields))
    }

    /// 2026-07-14: Parse a `struct Name { fields }` declaration as a TypeDef.
    /// Consumes the `struct` keyword, then delegates to parse_type_definition
    /// with the modified position already past the keyword.
    fn parse_struct_like(&mut self) -> Result<Box<TypeDef>, SyntaxError> {
        // struct Name { slot: Type; ... }
        self.pos += 1; // consume struct
        let name = self.expect_identifier()?;
        let mut slots = Vec::new();
        if self.eat(&Token::LBrace) {
            while !self.check(&Token::RBrace) && !self.is_at_end() {
                let slot_name = self.expect_identifier()?;
                self.expect(Token::Colon)?;
                let slot_ty = self.parse_type()?;
                self.eat(&Token::Semicolon);
                slots.push(TypeDefSlot { name: slot_name, ty: slot_ty, bit_range: None });
            }
            self.expect(Token::RBrace)?;
        }
        self.eat(&Token::Semicolon);
        Ok(Box::new(TypeDef {
            name, type_params: vec![], base: Box::new(Expr::Identifier("Bits".into())),
            bit_range: None, span: None,
            body: TypeDefBody {
                slots, metadata: std::collections::HashMap::new(),
                projections: vec![], bindings: vec![], operators: vec![], constraints: vec![], span: None,
            },
        }))
    }

    /// 2026-07-14: Parse an `enum Name { Variant, Variant(Type) }` declaration.
    /// Handles the basic form and stores as a TypeDef with variant metadata.
    fn parse_enum_like(&mut self) -> Result<Box<TypeDef>, SyntaxError> {
        // enum Name { A, B, C(Int) }
        self.pos += 1;
        let name = self.expect_identifier()?;
        let mut slots = Vec::new();
        if self.eat(&Token::LBrace) {
            while !self.check(&Token::RBrace) && !self.is_at_end() {
                let variant_name = self.expect_identifier()?;
                let variant_ty = if self.eat(&Token::LParen) {
                    let inner = self.parse_type()?;
                    self.expect(Token::RParen)?;
                    inner
                } else {
                    Type::int()
                };
                self.eat(&Token::Comma);
                slots.push(TypeDefSlot { name: format!("__variant_{}", variant_name), ty: variant_ty, bit_range: None });
            }
            self.expect(Token::RBrace)?;
        }
        self.eat(&Token::Semicolon);
        Ok(Box::new(TypeDef {
            name, type_params: vec![], base: Box::new(Expr::Identifier("Bits".into())),
            bit_range: None, span: None,
            body: TypeDefBody {
                slots, metadata: std::collections::HashMap::new(),
                projections: vec![], bindings: vec![], operators: vec![], constraints: vec![], span: None,
            },
        }))
    }
}

#[cfg(test)]
mod tests {
    use crate::lexer::tokenize;
    use crate::parser::Parser;

    fn parse_type(src: &str) -> Result<crate::ast::Type, crate::errors::SyntaxError> {
        let tokens = tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        p.parse_type()
    }

    #[test]
    fn test_parse_dotted_type_extension() {
        // "String.c" should parse as Type::Custom("String.c")
        let ty = parse_type("String.c").unwrap();
        assert_eq!(ty, crate::ast::Type::Custom("String.c".into()));
    }

    #[test]
    fn test_parse_dotted_type_no_extension() {
        // "String" should still parse as Type::string()
        let ty = parse_type("String").unwrap();
        assert_eq!(ty, crate::ast::Type::string());
    }

    #[test]
    fn test_parse_dotted_type_double_extension() {
        // "Int.c.sso" should parse as Type::Custom("Int.c.sso")
        let ty = parse_type("Int.c.sso").unwrap();
        assert_eq!(ty, crate::ast::Type::Custom("Int.c.sso".into()));
    }

    // ── P3: frgn declaration parsing ─────────────────────────────────

    fn parse_frgn(src: &str) -> Result<crate::ast::ForeignBinding, crate::errors::SyntaxError> {
        let tokens = tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        match p.parse_top_level()? {
            crate::ast::TopLevel::ForeignBinding(fb) => Ok(fb),
            _ => panic!("expected ForeignBinding"),
        }
    }

    #[test]
    fn test_parse_frgn_literal_path() {
        let fb = parse_frgn(r#"frgn strlen(s: String) -> Int from "libc.so.6";"#).unwrap();
        assert_eq!(fb.name, "strlen");
        assert_eq!(fb.inputs.len(), 1);
        assert_eq!(fb.inputs[0].0, "s");
        assert_eq!(fb.inputs[0].1, crate::ast::Type::string());
        assert_eq!(fb.success_output.len(), 1);
        match &fb.from {
            crate::ast::FromSpec::Literal(p) => {
                assert_eq!(p.to_string_lossy(), "libc.so.6");
            }
            _ => panic!("expected Literal"),
        }
    }

    #[test]
    fn test_parse_frgn_compiler_path() {
        let fb = parse_frgn(r#"frgn hash(data: Data) -> Int from <xxhash.c>;"#).unwrap();
        assert_eq!(fb.name, "hash");
        assert_eq!(fb.inputs.len(), 1);
        match &fb.from {
            crate::ast::FromSpec::CompilerRegistry(name) => {
                assert_eq!(name, "xxhash.c");
            }
            _ => panic!("expected CompilerRegistry"),
        }
    }

    #[test]
    fn test_parse_frgn_no_return() {
        let fb = parse_frgn(r#"frgn print(s: String) from "libio.so";"#).unwrap();
        assert_eq!(fb.name, "print");
        assert!(fb.success_output.is_empty());
    }

    #[test]
    fn test_from_spec_extension() {
        use crate::ast::FromSpec;
        use std::path::PathBuf;
        let lit = FromSpec::Literal(PathBuf::from("libc.so.6"));
        // PathBuf::extension() returns only the segment after the LAST dot
        assert_eq!(lit.extension(), Some("6".into()));
        let reg = FromSpec::CompilerRegistry("xxhash.c".into());
        assert_eq!(reg.extension(), Some("c".into()));
        let no_ext = FromSpec::Literal(PathBuf::from("Makefile"));
        assert_eq!(no_ext.extension(), None);
    }

    #[test]
    fn test_from_spec_as_str() {
        use crate::ast::FromSpec;
        use std::path::PathBuf;
        let lit = FromSpec::Literal(PathBuf::from("libc.so.6"));
        assert_eq!(lit.as_str(), "libc.so.6");
        let reg = FromSpec::CompilerRegistry("xxhash.c".into());
        assert_eq!(reg.as_str(), "xxhash.c");
    }

    // ── Hashword type parsing ────────────────────────────────────────

    #[test]
    fn test_hashword_int_no_variant() {
        let ty = parse_type("#Int").unwrap();
        assert_eq!(ty, crate::ast::Type::HashWord("#Int".into()));
    }

    #[test]
    fn test_hashword_bits_no_variant() {
        let ty = parse_type("#Bits").unwrap();
        assert_eq!(ty, crate::ast::Type::HashWord("#Bits".into()));
    }

    #[test]
    fn test_hashword_string_with_default_variant() {
        // Bare #String resolves to utf8 (universal default)
        let ty = parse_type("#String").unwrap();
        assert_eq!(ty, crate::ast::Type::HashWordVariant("#String".into(), "utf8".into()));
    }

    #[test]
    fn test_hashword_string_with_explicit_variant() {
        let ty = parse_type("#String<utf8>").unwrap();
        assert_eq!(ty, crate::ast::Type::HashWordVariant("#String".into(), "utf8".into()));
    }

    #[test]
    fn test_hashword_string_with_explicit_ascii_variant() {
        let ty = parse_type("#String<ascii>").unwrap();
        assert_eq!(ty, crate::ast::Type::HashWordVariant("#String".into(), "ascii".into()));
    }

    #[test]
    fn test_hashword_float_with_explicit_variant() {
        let ty = parse_type("#Float<ieee754>").unwrap();
        assert_eq!(ty, crate::ast::Type::HashWordVariant("#Float".into(), "ieee754".into()));
    }

    // ── Op declaration parsing ───────────────────────────────────────

    fn parse_op_from_type_def(src: &str) -> Vec<crate::ast::top::OperatorDef> {
        let tokens = crate::lexer::tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        match p.parse_top_level() {
            Ok(crate::ast::TopLevel::TypeDef(td)) => td.body.operators,
            _ => panic!("expected TypeDef"),
        }
    }

    #[test]
    fn test_op_declarative_hashword() {
        let ops = parse_op_from_type_def("type T { op Add(#Int, #Int); };");
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].op, "Add");
        assert_eq!(ops[0].params.len(), 2);
        assert_eq!(ops[0].params[0], crate::ast::Type::HashWord("#Int".into()));
        assert_eq!(ops[0].params[1], crate::ast::Type::HashWord("#Int".into()));
        assert!(ops[0].impl_args.is_none());
    }

    #[test]
    fn test_op_declarative_multiple_params() {
        let ops = parse_op_from_type_def("type T { op Add(#Float, #Float); };");
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].params[0], crate::ast::Type::HashWordVariant("#Float".into(), "ieee754".into()));
    }

    #[test]
    fn test_op_binding_with_markers() {
        let ops = parse_op_from_type_def(
            "type T { op InsertAt(T) = ring_push(#L, #R); };"
        );
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].op, "InsertAt");
        assert_eq!(ops[0].params.len(), 1);
        assert!(ops[0].impl_args.is_some());
    }
}
