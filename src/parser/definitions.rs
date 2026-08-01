// ── Definition/Transaction/Cell Parser ─────────────────────────────────
// 2026-07-12: Phase 1.2 — Parse top-level declarations.
// Flat code: each function is max 2 levels.
// Handles: defn, txn, node, cell, export, import, meld, trg.
// Also handles derivation blocks :=, implicit entry wrapping.
// 2026-08-01 (Phase 2): `[#]` entry contracts removed — entry!/args! (Phase 3)
// replace the marker with explicit macros.

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
            // 2026-07-31: `async node` (prefix) — same as `node async`.
            Some(Token::Async) if matches!(self.tokens.get(self.pos + 1).map(|(t, _)| t), Some(Token::Node)) => {
                self.pos += 1; // consume async
                self.parse_node().map(TopLevel::Transaction)
            }
            Some(Token::Cell) => self.parse_cell().map(TopLevel::Cell),
            Some(Token::Import) => self.parse_import().map(TopLevel::Import),
            Some(Token::Meld) => self.parse_meld().map(TopLevel::Meld),
            Some(Token::Trg) => self.parse_top_level_trg().map(TopLevel::Trigger),
            // 2026-07-14: Handle `type Name : Parent { slots }` definitions
            // 2026-07-16: P2 — Check for extension group Type.[a,b,c] before single type
            Some(Token::Type) => self.parse_type_or_group().map(TopLevel::TypeDef),
            // 2026-07-14: Handle `struct Name { fields }` as TypeDef
            Some(Token::Obj) => self.parse_obj_like().map(TopLevel::TypeDef),
            Some(Token::Struct) => self.parse_struct_def().map(TopLevel::StaticStruct),
            // 2026-07-26: Handle `render struct Name { <html> }` and `render obj Name { <html> }`
            Some(Token::Render) => self.parse_render_block(),
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
                let is_bang = matches!(self.peek(), Some(Token::FrgnBang));
                self.advance();
                // 2026-07-25: frgn? / frgn! / frgn?! modifiers
                // frgn? → optional. frgn! → fire-and-forget (via FrgnBang token).
                // frgn?! → frgn! + immediate question check for delivery.
                let is_optional = !is_bang && self.eat(&Token::Question);
                let is_fire_forget = is_bang;
                let mut is_delivery = false;
                if is_fire_forget && self.eat(&Token::Question) {
                    is_delivery = true;
                }
                let mut fb = self.parse_frgn_decl()?;
                fb.is_optional = is_optional;
                fb.is_fire_forget = is_fire_forget;
                fb.is_delivery = is_delivery;
                Ok(TopLevel::ForeignBinding(fb))
            }
            _ => {
                // 2026-07-24: Capture doc comments (/// and //!) and attach
                // to the next definition/transaction/cell/frgn.
                if let Some(&crate::lexer::Token::DocComment(ref text)) = self.peek() {
                    self.set_doc(text.clone());
                    self.pos += 1;
                    return self.parse_top_level();
                }
                if let Some(&crate::lexer::Token::DocCommentBang(ref text)) = self.peek() {
                    self.set_doc(text.clone());
                    self.pos += 1;
                    return self.parse_top_level();
                }
                // 2026-07-23: $defn and $txn at top level (lexed as identifiers)
                if self.check_identifier("$defn") {
                    return self.parse_compile_time_defn();
                }
                if self.check_identifier("$txn") {
                    return self.parse_compile_time_txn();
                }
                // 2026-07-25: $let and $const — compile-time variables
                if self.check_identifier("$let") {
                    return self.parse_compile_time_let(false);
                }
                if self.check_identifier("$const") {
                    return self.parse_compile_time_let(true);
                }
                // 2026-07-23: proto variant: #Category { ... } — protocol declaration
                if self.check_identifier("proto") {
                    return self.parse_protocol_def().map(TopLevel::ProtocolDef);
                }
                // 2026-07-29: asm<target> name(args) -> T { "instr"; };
                if self.check_identifier("asm") {
                    return self.parse_asm_fn().map(TopLevel::AsmFn);
                }
                let name = self.expect_identifier()?;
                self.error_at_current(&format!("unexpected top-level item '{}'", name))
            }
        }
    }

    /// 2026-07-26: Parse `render struct <name> { <html> }` or `render obj <name> { <html> }`.
    /// Consumes the `render` keyword. Checks for `struct` or `obj` identifier,
    /// then the name, then reads the raw HTML body between braces.
    pub fn parse_render_block(&mut self) -> Result<TopLevel, SyntaxError> {
        // Capture span from the 'render' keyword
        let start_span = self.peek_with_span()
            .map(|(_, s)| self.make_span(s.clone()))
            .unwrap_or(Span::dummy());
        // Consume 'render' keyword
        self.advance();
        // Check for 'struct' or 'obj' keyword
        if !self.eat(&Token::Struct) && !self.eat(&Token::Obj) {
            return self.error_at_current("expected 'struct' or 'obj' after 'render'");
        }
        let struct_name = self.expect_identifier()?;
        self.expect(Token::LBrace)?;
        let view_html = self.read_html_body()?;
        // Optional trailing semicolon after '}'
        self.eat(&Token::Semicolon);
        Ok(TopLevel::RenderBlock(RenderBlock {
            struct_name,
            view_html,
            span: Some(start_span),
        }))
    }

    /// 2026-07-22: Parse `frgn` declaration (import model).
    /// Syntax:
    ///   frgn <foreign_symbol>(<params>) [-> <ret>] [as <brief_name>] from <source> [target "c"] [fallback <expr>];
    ///   frgn <foreign_symbol>(<params>) [-> <ret>] [as <brief_name>] from <source> [target "c"] [fallback <fn>(<args>)];
    ///   frgn <foreign_symbol>(<params>) [-> <ret>] [as <brief_name>] from <source> [target "c"] [fallback ;];
    ///
    /// `from` is required (provenance for the foreign module).
    /// `as` is optional and comes before `from` (Brief name, different from the C symbol).
    fn parse_frgn_decl(&mut self) -> Result<ForeignBinding, SyntaxError> {
        // 2026-07-22: First name after `frgn` is the C/foreign symbol name.
        let foreign_name = self.expect_identifier()?;

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

        // 2026-07-22: Parse optional `as <brief_name>` — Brief-side name, before `from`.
        let brief_name = if self.eat(&Token::As) {
            Some(self.expect_identifier()?)
        } else {
            None
        };

        // 2026-07-22: `from` is REQUIRED — every frgn must declare provenance.
        if !self.eat(&Token::From) {
            let msg = format!(
                "frgn '{}' requires `from <source>` — specify which foreign module provides this symbol",
                foreign_name
            );
            return self.error_at_current(&msg);
        }
        let from = self.parse_from_spec()?;

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
            foreign_name,
            brief_name,
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
            doc: self.take_doc(),
            is_optional: false,
            is_fire_forget: false,
            is_delivery: false,
        })
    }

    /// 2026-07-16: P3 — Parse `from "path"` or `from <name>` after `from` token is consumed.
    fn parse_from_spec(&mut self) -> Result<FromSpec, SyntaxError> {
        // 2026-07-26: from #System — protocol-based linking.
        // from #Link<name> — direct linker directive (-l<name>).
        if let Some(Token::Identifier(name)) = self.peek() {
            if name.starts_with('#') {
                let hashword = name.clone();
                self.advance();
                // 2026-07-26: #Link<name> — parse <name> part
                if hashword == "#Link" {
                    self.expect(Token::Lt)?;
                    let mut link_name = String::new();
                    loop {
                        match self.peek() {
                            Some(Token::Gt) => {
                                self.advance();
                                break;
                            }
                            Some(Token::Identifier(seg)) => {
                                link_name.push_str(seg);
                                self.advance();
                            }
                            Some(Token::Dot) => {
                                link_name.push('.');
                                self.advance();
                            }
                            Some(Token::Integer(n)) => {
                                link_name.push_str(&n.to_string());
                                self.advance();
                            }
                            other => {
                                return self.error_at_current(&format!(
                                    "expected '>' to close #Link<...>, got {:?}", other
                                ));
                            }
                        }
                    }
                    if link_name.is_empty() {
                        return self.error_at_current("expected library name in #Link<...>");
                    }
                    return Ok(FromSpec::Linked(link_name));
                }
                return Ok(FromSpec::Protocol(hashword));
            }
        }
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
    pub(crate) fn parse_definition(&mut self) -> Result<Definition, SyntaxError> {
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
        // 2026-07-31: Contract may precede or follow the `-> Type` return
        // type (see parse_output_and_contract).
        let (output_type, contract) = self.parse_output_and_contract()?;
        // 2026-07-28: Body is optional — `defn f(x) -> T := { ... }` has no { body }.
        let body = if self.check(&Token::LBrace) {
            self.parse_block()?
        } else {
            Vec::new()
        };
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
            doc: self.take_doc(),
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
    pub(crate) fn parse_transaction(
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
        // 2026-07-31: Contract may precede or follow the `-> Type` return
        // type (see parse_output_and_contract).
        let (output_type, contract) = self.parse_output_and_contract()?;
        // 2026-07-28: Body is optional — `txn f -> T := { ... }` has no { body }.
        let body = if self.check(&Token::LBrace) {
            self.parse_block()?
        } else {
            Vec::new()
        };
        let derivation = self.parse_derivation_block()?;
        let doc = self.take_doc();
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
            doc,
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
        // 2026-07-28: Body is optional for consistency with defn/txn.
        let body = if self.check(&Token::LBrace) {
            self.parse_block()?
        } else {
            Vec::new()
        };
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
            doc: self.take_doc(),
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
            doc: self.take_doc(),
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

    /// 2026-07-31: Parse the return type and contract in EITHER order, so a
    /// contract may come before OR after the `-> Type` return type:
    ///
    ///   defn f(a, b) -> Int [pre][post] { ... }   // return type first
    ///   defn f(a, b) [pre][post] -> Int { ... }   // contract first
    ///   txn  f(a, b) [pre][post] -> Int { ... }   // txn form
    ///   txn  f(a, b) -> Int [pre][post] { ... }
    ///
    /// Both are optional: a missing return type is inferred from `term`; a
    /// missing contract defaults to `[true][true]`. Returns (output_type,
    /// contract). The `parse_type` array-size lookahead (types.rs) leaves a
    /// non-integer `[` for the contract parser.
    fn parse_output_and_contract(&mut self) -> Result<(Option<OutputType>, Contract), SyntaxError> {
        let contract = if self.check(&Token::LBracket) {
            Some(self.parse_contract()?)
        } else {
            None
        };
        let output_type = self.parse_output_type()?;
        let contract = match contract {
            Some(c) => c,
            None => self.parse_contract()?,
        };
        Ok((output_type, contract))
    }

    /// Parse contract: [pre][post], [[post], [pre]]
    fn parse_contract(&mut self) -> Result<Contract, SyntaxError> {
        let mut pre = Expr::Bool(true);
        let mut post = Expr::Bool(true);
        // 2026-07-31: true once any `[` was consumed — distinguishes an
        // explicit contract from the no-contract default `[true][true]`.
        let mut contract_saw_bracket = false;
        // 2026-08-01 (Phase 2): `[#]` entry-point marker removed. Peek for it
        // and raise a clear error — the entry!/args! plugin (Phase 3) replaces
        // the marker with explicit macros, so `[#]` must not silently parse as
        // a precondition referencing the identifier `#`.
        if self.check(&Token::LBracket) {
            let saved = self.pos;
            self.pos += 1; // peek past LBracket
            let is_entry_syntax = self.check_identifier("#");
            self.pos = saved; // restore
            if is_entry_syntax {
                return Err(SyntaxError::InvalidStatement {
                    reason: "'[#]' entry-point syntax removed — use the entry!/args! \
                             macros (Phase 3) or write an explicit contract"
                        .to_string(),
                    span: Span::dummy(),
                });
            }
        }
        // Parse: [pre] if present
        if self.check(&Token::LBracket) {
            contract_saw_bracket = true;
            pre = self.parse_single_contract_condition()?;
        }
        // Parse: [post] if present
        if self.check(&Token::LBracket) {
            contract_saw_bracket = true;
            post = self.parse_single_contract_condition()?;
        }
        // 2026-07-31 (Phase 3): Watchdog — optional `?[cond]` or required
        // `![cond]` after the postcondition. Populated into Contract.watchdog.
        let watchdog = if self.check(&Token::Question) || self.check(&Token::Not) {
            let is_required = matches!(self.peek(), Some(Token::Not));
            self.pos += 1; // consume '?' or '!'
            contract_saw_bracket = true;
            self.expect(Token::LBracket)?;
            let cond = self.parse_expression()?;
            // 2026-07-31: Optional duration unit: `?[5000 ms]` / `?[5000ms]`.
            // The condition carries the numeric bound; the unit token is
            // consumed so the documented `ms`/`cyc`/`seconds` forms parse.
            match self.peek() {
                Some(Token::Ms) | Some(Token::Cyc)
                | Some(Token::Seconds) | Some(Token::Minute) => {
                    self.pos += 1;
                }
                _ => {}
            }
            self.expect(Token::RBracket)?;
            Some(WatchdogSpec {
                condition: cond,
                is_required,
                cycles_bound: None,
                seconds_bound: None,
                is_proven: false,
                retries: 0,
                fallback: None,
            })
        } else {
            None
        };
        let explicit = contract_saw_bracket;
        Ok(Contract {
            pre_condition: pre,
            post_condition: post,
            watchdog,
            span: None,
            explicit,
        })
    }

    /// Parse a single contract condition: [expr]
    fn parse_single_contract_condition(&mut self) -> Result<Expr, SyntaxError> {
        self.pos += 1; // consume '['
        let expr = self.parse_expression()?;
        self.expect(Token::RBracket)?;
        Ok(expr)
    }

    /// Parse optional derivation block: := { ... } := ref_fn
    /// 2026-07-29: Uses two `:=` — one for examples, one for reference (order-free).
    ///   := { 0 -> 0; }           — examples only (existing)
    ///   := popcount_ref          — reference only (synthesis skipped, use ref body)
    ///   := { 0 -> 0; } := ref_fn — both (verify against reference)
    ///   := ref_fn := { 0 -> 0; } — both (reversed order)
    /// 2026-07-29: Parse a single segment after :=: either { examples } or identifier.
    /// Returns (examples, ref_name, ref_tolerance) for a derivation segment,
    /// or Ok(None) for the last segment when the next token isn't := or {.
    fn parse_derivation_segment(&mut self) -> Result<Option<(Vec<DerivationExample>, Option<String>, Option<f64>)>, SyntaxError> {
        if self.check(&Token::LBrace) {
            self.expect(Token::LBrace)?;
            let mut examples = Vec::new();
            while !self.check(&Token::RBrace) && !self.is_at_end() {
                let example = self.parse_derivation_example()?;
                examples.push(example);
                self.eat(&Token::Semicolon);
            }
            self.expect(Token::RBrace)?;
            self.eat(&Token::Semicolon);
            Ok(Some((examples, None, None)))
        } else if let Some(Token::Identifier(n)) = self.peek().cloned() {
            self.advance();
            let mut ref_tolerance: Option<f64> = None;
            if self.eat(&Token::LBracket) {
                self.expect(Token::Identifier("tol".into())).ok();
                self.eat(&Token::Colon);
                ref_tolerance = self.parse_expression().ok().and_then(|e| {
                    match e { Expr::Float(f) => Some(f), Expr::Decimal(n) => Some(n as f64), _ => None }
                });
                self.eat(&Token::RBracket);
            }
            Ok(Some((vec![], Some(n), ref_tolerance)))
        } else {
            Ok(None)
        }
    }

    /// 2026-07-29: Parse asm<target> name(args) -> T { "instr"; "instr"; };
    fn parse_asm_fn(&mut self) -> Result<AsmFn, SyntaxError> {
        let start = self.pos;
        self.advance(); // consume 'asm'
        // expect '<'
        self.expect(Token::Lt)?;
        // expect target identifier
        let target = self.expect_identifier()?;
        // expect '>'
        self.expect(Token::Gt)?;
        // expect function name
        let name = self.expect_identifier()?;
        // expect '('
        self.expect(Token::LParen)?;
        // parse params
        let params = self.parse_parameter_list()?;
        // expect ')'
        self.expect(Token::RParen)?;
        // expect '->'
        self.expect(Token::Arrow)?;
        // parse return type
        let ret_type = self.parse_type()?;
        // expect '{'
        self.expect(Token::LBrace)?;
        // parse asm body (string literals separated by semicolons)
        let body = self.parse_asm_body()?;
        // expect '}'
        self.expect(Token::RBrace)?;
        // expect ';'
        self.eat(&Token::Semicolon);
        let span = self.tokens.get(start)
            .and_then(|(_, s1)| self.tokens.get(self.pos - 1).map(|(_, s2)| (s1, s2)))
            .map(|(s1, s2)| Span::new(s1.start, s2.end, 0, 0))
            .unwrap_or(Span::new(0, 0, 0, 0));
        Ok(AsmFn { target, name, params, ret_type, body, span })
    }

    /// 2026-07-29: Parse the body of an asm block: string literals separated by semicolons.
    fn parse_asm_body(&mut self) -> Result<Vec<String>, SyntaxError> {
        let mut strings = Vec::new();
        loop {
            if self.check(&Token::RBrace) {
                break;
            }
            let s = self.expect_string()?;
            strings.push(s);
            self.eat(&Token::Semicolon);
        }
        Ok(strings)
    }

    fn parse_derivation_block(&mut self) -> Result<Option<DerivationBlock>, SyntaxError> {
        let colon_eq_span = self.tokens.get(self.pos)
            .map(|(_, s)| s.clone())
            .unwrap_or(0..0);
        if !self.eat(&Token::ColonEq) {
            return Ok(None);
        }

        // 2026-07-29: Multi-segment chain: := a := b := c
        // Parse segments in a loop, each segment is either { examples }
        // or an identifier (asm/defn ref).
        let mut chain: Vec<ChainSegment> = Vec::new();
        let mut examples: Vec<DerivationExample> = Vec::new();
        let mut ref_name: Option<String> = None;
        let mut ref_tolerance: Option<f64> = None;

        // Parse the first segment
        if let Some((ex, rn, rt)) = self.parse_derivation_segment()? {
            if !ex.is_empty() {
                // First segment has examples (backward compat)
                examples = ex;
                ref_name = rn;
                ref_tolerance = rt;
            } else if let Some(name) = rn {
                chain.push(ChainSegment::Ref(name));
            }
        } else {
            return self.error_at_current("expected '{' for examples or identifier for reference function after ':='");
        }

        // Parse additional segments
        while self.eat(&Token::ColonEq) {
            if let Some((ex, rn, rt)) = self.parse_derivation_segment()? {
                if !ex.is_empty() {
                    // Standalone examples block (no ref): := { ex }
                    chain.push(ChainSegment::Derivation(Box::new(DerivationBlock {
                        examples: ex, synthesized: None,
                        postcondition: None, precondition: None,
                        ref_name: None, ref_tolerance: None,
                        chain: vec![], span: crate::errors::Span::dummy(),
                    })));
                } else if let Some(name) = rn {
                    chain.push(ChainSegment::Ref(name));
                }
            } else {
                break;
            }
        }

        // Contract parsing: [[post], [pre][post], [pre]]
        let (precondition, postcondition) = if self.check(&Token::LBracket) {
            let next_is_bracket = self.tokens.get(self.pos + 1)
                .map(|(t, _)| matches!(t, Token::LBracket))
                .unwrap_or(false);
            if next_is_bracket {
                // [[ — postcondition only
                self.advance();
                self.advance();
                let post = Some(self.parse_expression()?);
                self.expect(Token::RBracket)?;
                (None, post)
            } else {
                self.advance();
                let expr = Some(self.parse_expression()?);
                let closed = self.eat(&Token::RBracket);
                if self.check(&Token::LBracket) {
                    self.advance();
                    let post = Some(self.parse_expression()?);
                    self.expect(Token::RBracket)?;
                    (expr, post)
                } else if !closed {
                    (None, None)
                } else {
                    if self.check(&Token::RBracket) {
                        self.advance();
                        (expr, None)
                    } else {
                        (None, expr)
                    }
                }
            }
        } else {
            (None, None)
        };

        let end = self.tokens.get(self.pos).map(|(_, s)| s.start).unwrap_or(colon_eq_span.start + 2);
        let span = Span::new(colon_eq_span.start, end, 0, 0);
        Ok(Some(DerivationBlock {
            examples,
            synthesized: None,
            postcondition,
            precondition,
            ref_name,
            ref_tolerance,
            chain,
            span,
        }))
    }

    /// Parse a single derivation example: inputs -> [tol] output
    /// 2026-07-28: Optional [expr] tolerance bracket after -> for FP relaxed equivalence.
    /// Syntax: `input -> [0.001] output;`
    fn parse_derivation_example(&mut self) -> Result<DerivationExample, SyntaxError> {
        let mut inputs = Vec::new();
        loop {
            inputs.push(self.parse_expression()?);
            if self.eat(&Token::Arrow) {
                break;
            }
            self.expect(Token::Comma); // must be followed by comma or arrow
        }
        // 2026-07-28: Optional tolerance bracket: -> [tol] output
        let tolerance = if self.eat(&Token::LBracket) {
            let tol_expr = self.parse_expression()?;
            self.expect(Token::RBracket)?;
            Some(self.expr_to_f64_constant(&tol_expr)?)
        } else {
            None
        };
        let output = Box::new(self.parse_expression()?);
        Ok(DerivationExample {
            inputs,
            output,
            tolerance,
            span: Span::dummy(),
        })
    }

    /// 2026-07-28: Evaluate a compile-time constant expression to f64.
    /// Used for tolerance parsing and other early-parse constant folding.
    /// Handles Expr::Float, Expr::Decimal, and Expr::UnaryOp(Neg, ...).
    fn expr_to_f64_constant(&self, expr: &Expr) -> Result<f64, SyntaxError> {
        match expr {
            Expr::Float(f) => Ok(*f),
            Expr::Decimal(n) => Ok(*n as f64),
            Expr::UnaryOp(UnaryOpKind::Neg, inner) => {
                let val = self.expr_to_f64_constant(inner)?;
                Ok(-val)
            }
            _ => {
                let msg = format!(
                    "expected a numeric constant (float or integer) in tolerance bracket, got '{}'",
                    expr
                );
                Err(SyntaxError::InvalidExpression {
                    reason: msg,
                    span: Span::dummy(),
                })
            }
        }
    }

    /// Wrap top-level statements in an implicit entry transaction if needed.
    /// 2026-08-01 (Phase 2): the `[#]` marker is gone — the entry!/args!
    /// plugin (Phase 3) owns one-shot opening-node synthesis.
    fn wrap_implicit_entry(&self, _items: &mut Vec<TopLevel>) {
        // Placeholder: full implementation in Phase 16E
    }

    /// 2026-07-14: Parse: type Name : Parent { slot; slot; }
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
        let base = if self.eat(&Token::Colon) {
            self.parse_expression()?
        } else {
            Expr::Identifier(base_name.to_string())
        };
        let mut slots = Vec::new();
        let mut metadata = std::collections::HashMap::new();
        let mut operators: Vec<OperatorDef> = Vec::new();
        let mut op_bindings: Vec<OperatorBinding> = Vec::new();
        let mut props: Vec<PropDef> = Vec::new();
        if self.eat(&Token::LBrace) {
            while !self.check(&Token::RBrace) && !self.is_at_end() {
                // !> key: value; — metadata assignment (new syntax)
                if self.check(&Token::ExclaimArrow) {
                    self.advance();
                    let key = self.expect_identifier()?;
                    self.expect(Token::Colon)?;
                    match key.as_str() {
                        "ctd" => {
                            let ctd_name = self.expect_identifier()?;
                            self.eat(&Token::Semicolon);
                            metadata.insert("ctd".into(), PropertyValue::Identifier(ctd_name));
                        }
                        "alu" => {
                            match self.peek() {
                                Some(Token::Identifier(_)) => {
                                    let alu_name = self.expect_identifier()?;
                                    metadata.insert("alu".into(), PropertyValue::Identifier(alu_name));
                                }
                                _ => {
                                    let alu_str = self.expect_string()?;
                                    metadata.insert("alu".into(), PropertyValue::String(alu_str));
                                }
                            }
                            self.eat(&Token::Semicolon);
                        }
                        "layout" => {
                            if self.check(&Token::LBrace) {
                                let fields = self.parse_layout_struct_body()?;
                                metadata.insert("layout_struct".into(), fields);
                            } else {
                                let raw = self.read_layout_body()?;
                                metadata.insert("layout".into(), PropertyValue::String(raw));
                            }
                            self.eat(&Token::Semicolon);
                        }
                        _ => {
                            let pv = self.parse_metadata_value_standalone()?;
                            self.eat(&Token::Semicolon);
                            metadata.insert(key, pv);
                        }
                    }
                    continue;
                }
                let slot_name = self.expect_identifier()?;
                if slot_name == "op" {
                    self.parse_op_definition(&mut op_bindings)?;
                    continue;
                }
                if slot_name == "prop" {
                    self.parse_prop_definition(&mut props)?;
                    continue;
                }
                self.expect(Token::Colon)?;
                let slot_ty = self.parse_type()?;
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
                parent: Some(Box::new(base.clone())),
            protocol: None,
                bit_range: None,
                body: TypeDefBody {
                    slots: slots.clone(),
                    metadata: metadata.clone(),
                    projections: vec![],
                    bindings: vec![],
                    operators: operators.clone(), op_bindings: op_bindings.clone(),
                    props: props.clone(),
                    constraints: vec![],
                    members: vec![],
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
            parent: Some(Box::new(base)),
            protocol: None,
            bit_range: None,
            body: TypeDefBody {
                slots,
                metadata,
                projections: vec![],
                bindings: vec![],
                operators,
                op_bindings,
                props,
                constraints: vec![],
                members: vec![],
                span: None,
            },
            span: None,
        }))
    }

    /// 2026-07-24: Parse `type Name [ : [Parent] [Protocol] ] { body }`.
    fn parse_type_body(&mut self, name: String, type_params: Vec<crate::ast::top::TypeParam>) -> Result<Box<TypeDef>, SyntaxError> {
        let mut parent: Option<Box<Expr>> = None;
        let mut protocol: Option<String> = None;
        if self.eat(&Token::Colon) {
            // 2026-07-26: Parse type X: #Proto Parent or type X: Parent or type X: #Proto
            // Hashwords always sit left of what they attach to.
            match self.peek() {
                Some(&Token::Identifier(ref s)) if s.starts_with('#') => {
                    let proto = s.clone(); self.pos += 1;
                    protocol = Some(proto);
                    // Optional parent type after protocol hashword
                    match self.peek() {
                        Some(&Token::Identifier(ref s)) if !s.starts_with('#') => {
                            let pname = s.clone(); self.pos += 1;
                            parent = Some(Box::new(Expr::Identifier(pname)));
                        }
                        _ => {}
                    }
                }
                Some(&Token::Identifier(_)) => {
                    let pname = self.expect_identifier()?;
                    parent = Some(Box::new(Expr::Identifier(pname)));
                }
                _ => {}
            }
        }
        let mut slots = Vec::new();
        let mut metadata = std::collections::HashMap::new();
        let mut operators: Vec<OperatorDef> = Vec::new();
        let mut op_bindings: Vec<OperatorBinding> = Vec::new();
        let mut props: Vec<PropDef> = Vec::new();
        if self.eat(&Token::LBrace) {
            while !self.check(&Token::RBrace) && !self.is_at_end() {
                // !> key: value; — metadata assignment (new syntax)
                if self.check(&Token::ExclaimArrow) {
                    self.advance();
                    let key = self.expect_identifier()?;
                    self.expect(Token::Colon)?;
                    match key.as_str() {
                        "ctd" => {
                            let ctd_name = self.expect_identifier()?;
                            self.eat(&Token::Semicolon);
                            metadata.insert("ctd".into(), PropertyValue::Identifier(ctd_name));
                        }
                        "alu" => {
                            match self.peek() {
                                Some(Token::Identifier(_)) => {
                                    let alu_name = self.expect_identifier()?;
                                    metadata.insert("alu".into(), PropertyValue::Identifier(alu_name));
                                }
                                _ => {
                                    let alu_str = self.expect_string()?;
                                    metadata.insert("alu".into(), PropertyValue::String(alu_str));
                                }
                            }
                            self.eat(&Token::Semicolon);
                        }
                        "layout" => {
                            if self.check(&Token::LBrace) {
                                let fields = self.parse_layout_struct_body()?;
                                metadata.insert("layout_struct".into(), fields);
                            } else {
                                let raw = self.read_layout_body()?;
                                metadata.insert("layout".into(), PropertyValue::String(raw));
                            }
                            self.eat(&Token::Semicolon);
                        }
                        _ => {
                            let pv = self.parse_metadata_value_standalone()?;
                            self.eat(&Token::Semicolon);
                            metadata.insert(key, pv);
                        }
                    }
                    continue;
                }
                let slot_name = self.expect_identifier()?;
                if slot_name == "op" {
                    self.parse_op_definition(&mut op_bindings)?;
                    continue;
                }
                if slot_name == "prop" {
                    self.parse_prop_definition(&mut props)?;
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
            parent,
            protocol,
            bit_range: None,
            body: TypeDefBody {
                slots,
                metadata,
                projections: vec![],
                bindings: vec![],
                operators,
                op_bindings,
                props,
                constraints: vec![],
                members: vec![],
                span: None,
            },
            span: None,
        }))
    }

    /// 2026-07-20: Parse an op binding within a type body.
    /// Two forms:
    ///   op Add(#Int, #Int);                                     — declarative hashword dispatch
    ///   op Add(Posit32) = Posit32_add(#L, #R);                  — binding with explicit function

    /// 2026-07-26: Parse prop Name: expr;
    /// Declares a metaproperty with an implementation expression.
    /// `:` replaces the old `=` syntax.
    fn parse_prop_definition(&mut self, props: &mut Vec<PropDef>) -> Result<(), SyntaxError> {
        let name = self.expect_identifier()?;
        self.expect(Token::Colon)?;
        // Parse method call with #L placeholder
        let fn_name = self.expect_identifier()?;
        self.expect(Token::LParen)?;
        let mut args = Vec::new();
        while !self.check(&Token::RParen) && !self.is_at_end() {
            args.push(self.parse_hash_marker()?);
            if !self.check(&Token::RParen) {
                self.eat(&Token::Comma);
            }
        }
        self.expect(Token::RParen)?;
        self.expect(Token::Semicolon)?;
        let expr = Expr::Call(fn_name, args, None);
        props.push(PropDef { name, expr, span: None });
        Ok(())
    }

    /// 2026-07-26: Parse op Name(Proto?): expr;
    /// Declares an operator binding. protocol_variant is optional.
    /// Optional discriminator fields: pre:"0x", suf:"f", reg:"[0-9]+"
    /// Examples:
    ///   op InsertAt: push(#L, #R);
    ///   op Add(#Int): int_add(#L, #R);
    ///   op Parse(Decimal, pre:"0x"): parse_hex(#L);
    ///   op Parse(Decimal, suf:"h"): to_f16(#L);
    fn parse_op_definition(&mut self, op_bindings: &mut Vec<OperatorBinding>) -> Result<(), SyntaxError> {
        let name = self.expect_identifier()?;
        // Optional protocol variant: (#Proto) or (ConcreteType)
        let protocol_variant = if self.eat(&Token::LParen) {
            // Parse the protocol variant or concrete type
            let variant = self.expect_identifier()?;
            // Check for discriminator key-value pairs: pre:"0x", suf:"f", reg:"..."
            let mut pre: Option<String> = None;
            let mut suf: Option<String> = None;
            let mut reg: Option<String> = None;
            while self.eat(&Token::Comma) {
                let key = self.expect_identifier()?;
                self.expect(Token::Colon)?;
                let val = self.expect_string()?;
                match key.as_str() {
                    "pre" => { pre = Some(val); }
                    "suf" => { suf = Some(val); }
                    "reg" => { reg = Some(val); }
                    _ => {
                        let msg = format!(
                            "unknown discriminator '{}', expected 'pre', 'suf', or 'reg'", key);
                        return self.error_at_current(&msg);
                    }
                }
            }
            self.expect(Token::RParen)?;
            // Store discriminator fields on the OperatorBinding
            self.parse_discriminated_op(name, Some(variant), pre, suf, reg, op_bindings)?;
            return Ok(());
        } else {
            None
        };
        self.expect(Token::Colon)?;
        // Parse method call with #L, #R, #T placeholders as a raw expression
        let fn_name = self.expect_identifier()?;
        self.expect(Token::LParen)?;
        let mut args = Vec::new();
        while !self.check(&Token::RParen) && !self.is_at_end() {
            args.push(self.parse_hash_marker()?);
            if !self.check(&Token::RParen) {
                self.eat(&Token::Comma);
            }
        }
        self.expect(Token::RParen)?;
        self.expect(Token::Semicolon)?;
        let expr = Expr::Call(fn_name, args, None);
        op_bindings.push(OperatorBinding {
            name,
            protocol_variant,
            pre: None,
            suf: None,
            reg: None,
            expr,
            span: None,
        });
        Ok(())
    }

    /// 2026-07-27: Parse the expression part of a discriminated op binding
    /// (the part after the `:`) and push to op_bindings with discriminator fields.
    fn parse_discriminated_op(
        &mut self,
        name: String,
        protocol_variant: Option<String>,
        pre: Option<String>,
        suf: Option<String>,
        reg: Option<String>,
        op_bindings: &mut Vec<OperatorBinding>,
    ) -> Result<(), SyntaxError> {
        self.expect(Token::Colon)?;
        let fn_name = self.expect_identifier()?;
        self.expect(Token::LParen)?;
        let mut args = Vec::new();
        while !self.check(&Token::RParen) && !self.is_at_end() {
            args.push(self.parse_hash_marker()?);
            if !self.check(&Token::RParen) {
                self.eat(&Token::Comma);
            }
        }
        self.expect(Token::RParen)?;
        self.expect(Token::Semicolon)?;
        let expr = Expr::Call(fn_name, args, None);
        op_bindings.push(OperatorBinding { name, protocol_variant, pre, suf, reg, expr, span: None });
        Ok(())
    }

    /// 2026-07-26: Parse a hash marker (#L, #R, #T) or an identifier.
    fn parse_hash_marker(&mut self) -> Result<Expr, SyntaxError> {
        match self.peek() {
            Some(Token::HashL) => { self.pos += 1; Ok(Expr::Identifier("#L".to_string())) }
            Some(Token::HashR) => { self.pos += 1; Ok(Expr::Identifier("#R".to_string())) }
            Some(Token::HashT) => { self.pos += 1; Ok(Expr::Identifier("#T".to_string())) }
            _ => {
                let ident = self.expect_identifier()?;
                Ok(Expr::Identifier(ident))
            }
        }
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
    /// obj name { fields } — dynamic object definition.
    fn parse_obj_like(&mut self) -> Result<Box<TypeDef>, SyntaxError> {
        // 2026-07-31: obj Name<Params> { slot: Type; op …; txn member(…); defn member(…) }
        // Type params, operator bindings, and self-parameterized members are
        // collected into the TypeDef body.
        self.pos += 1; // consume obj
        let name = self.expect_identifier()?;
        let type_params = self.parse_type_params()?;
        let mut slots = Vec::new();
        let mut members: Vec<crate::ast::TopLevel> = Vec::new();
        let mut metadata = std::collections::HashMap::new();
        let mut operators: Vec<OperatorDef> = Vec::new();
        let mut op_bindings: Vec<OperatorBinding> = Vec::new();
        let mut props: Vec<PropDef> = Vec::new();
        if self.eat(&Token::LBrace) {
            while !self.check(&Token::RBrace) && !self.is_at_end() {
                if self.check(&Token::ExclaimArrow) {
                    // !> key: value; metadata (same handling as type bodies).
                    self.advance();
                    let key = self.expect_identifier()?;
                    self.expect(Token::Colon)?;
                    let pv = self.parse_metadata_value_standalone()?;
                    self.eat(&Token::Semicolon);
                    metadata.insert(key, pv);
                    continue;
                }
                if self.check(&Token::Txn) {
                    let txn = self.parse_transaction(false, false)?;
                    members.push(crate::ast::TopLevel::Transaction(txn));
                    self.eat(&Token::Semicolon);
                    continue;
                }
                if self.check(&Token::Defn) {
                    let defn = self.parse_definition()?;
                    members.push(crate::ast::TopLevel::Definition(defn));
                    self.eat(&Token::Semicolon);
                    continue;
                }
                if self.check(&Token::Node) {
                    // 2026-07-31 (A3): Reactive per-instance node member.
                    let node = self.parse_node()?;
                    members.push(crate::ast::TopLevel::Transaction(node));
                    self.eat(&Token::Semicolon);
                    continue;
                }
                let slot_name = self.expect_identifier()?;
                if slot_name == "op" {
                    self.parse_op_definition(&mut op_bindings)?;
                    continue;
                }
                if slot_name == "prop" {
                    self.parse_prop_definition(&mut props)?;
                    continue;
                }
                self.expect(Token::Colon)?;
                let slot_ty = self.parse_type()?;
                self.eat(&Token::Semicolon);
                slots.push(TypeDefSlot { name: slot_name, ty: slot_ty, bit_range: None });
            }
            self.expect(Token::RBrace)?;
        }
        self.eat(&Token::Semicolon);
        Ok(Box::new(TypeDef {
            name, type_params, parent: None,
            protocol: None,
            bit_range: None, span: None,
            body: TypeDefBody {
                slots, metadata, projections: vec![], bindings: vec![], operators, op_bindings, props, constraints: vec![], members, span: None,
            },
        }))
    }

    /// struct Name { field: Type; } — static fixed-layout struct.
    /// Pure data, C-compatible, no methods, no contracts.
    /// 2026-07-24: Fields are space-separated, semicolon-terminated.
    fn parse_struct_def(&mut self) -> Result<StructDef, SyntaxError> {
        self.pos += 1; // consume struct
        let name = self.expect_identifier()?;
        // 2026-07-31: Generic struct: struct ListBuffer<T> { ... }.
        let type_params = self.parse_type_params()?;
        let mut fields = Vec::new();
        let mut annotations: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
        if self.eat(&Token::LBrace) {
            while !self.check(&Token::RBrace) && !self.is_at_end() {
                // 2026-07-26: Parse optional hashword annotations (#Stack, #Heap, #Scalar)
                let mut field_annotations = Vec::new();
                while let Some(&Token::Identifier(ref s)) = self.peek() {
                    if s.starts_with('#') {
                        field_annotations.push(s.clone());
                        self.pos += 1;
                    } else {
                        break;
                    }
                }
                let field_name = self.expect_identifier()?;
                self.expect(Token::Colon)?;
                let field_type = self.parse_type()?;
                self.eat(&Token::Semicolon);
                fields.push((field_name.clone(), field_type));
                if !field_annotations.is_empty() {
                    annotations.insert(field_name, field_annotations);
                }
            }
            self.expect(Token::RBrace)?;
        }
        self.eat(&Token::Semicolon);
        let mut metadata = std::collections::HashMap::new();
        if !annotations.is_empty() {
            metadata.insert("annotations".to_string(), crate::ast::PropertyValue::String(format!("{:?}", annotations)));
        }
        Ok(StructDef {
            name, type_params, fields,
            metadata,
            span: None,
        })
    }

    /// 2026-07-14: Parse an `enum Name { Variant, Variant(Type) }` declaration.
    /// Handles the basic form and stores as a TypeDef with variant metadata.
    fn parse_enum_like(&mut self) -> Result<Box<TypeDef>, SyntaxError> {
        // enum Name { A, B, C(Int) }
        self.pos += 1;
        let name = self.expect_identifier()?;
        // 2026-07-31: Generic enum: enum Option<T> { Some(T), None }.
        let type_params = self.parse_type_params()?;
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
            name, type_params, parent: None,
            protocol: None,
            bit_range: None, span: None,
            body: TypeDefBody {
                slots, metadata: std::collections::HashMap::new(),
                projections: vec![], bindings: vec![], operators: vec![], op_bindings: vec![], props: vec![], constraints: vec![], members: vec![], span: None,
            },
        }))
    }

    /// $defn name(params) -> Type { body } — compile-time-only definition.
    /// 2026-07-23: Top-level item, extracted before codegen.
    fn parse_compile_time_defn(&mut self) -> Result<TopLevel, SyntaxError> {
        self.pos += 1; // consume $defn identifier
        let name = self.expect_identifier()?;
        let type_params = self.parse_type_params()?;
        let parameters = if self.eat(&Token::LParen) {
            let p = self.parse_parameter_list()?;
            self.expect(Token::RParen)?;
            p
        } else {
            vec![]
        };
        let output_type = self.parse_output_type()?;
        let contract = self.parse_contract()?;
        // 2026-07-28: Body is optional for consistency with defn/txn.
        let body = if self.check(&Token::LBrace) {
            self.parse_block()?
        } else {
            Vec::new()
        };
        let derivation = self.parse_derivation_block()?;
        let metadata = self.parse_body_metadata()?;
        Ok(TopLevel::CompileTimeDefn(Definition {
            name, type_params, parameters,
            output_type: output_type.clone(),
            outputs: vec![],
            contract, body, metadata,
            derivation, modifiers: vec![], annotations: vec![], span: None, doc: self.take_doc(),
        }))
    }

    /// $txn name(params) [pre][post] -> Type { body } — compile-time-only tx.
    /// 2026-07-23: Convergent loop with pre/post, top-level before codegen.
    fn parse_compile_time_txn(&mut self) -> Result<TopLevel, SyntaxError> {
        self.pos += 1; // consume $txn identifier
        let name = self.expect_identifier()?;
        let type_params = self.parse_type_params()?;
        let parameters = if self.eat(&Token::LParen) {
            let p = self.parse_parameter_list()?;
            self.expect(Token::RParen)?;
            p
        } else {
            vec![]
        };
        let output_type = self.parse_output_type()?;
        let contract = self.parse_contract()?;
        // 2026-07-28: Body is optional for consistency with defn/txn.
        let body = if self.check(&Token::LBrace) {
            self.parse_block()?
        } else {
            Vec::new()
        };
        let derivation = self.parse_derivation_block()?;
        let metadata = self.parse_body_metadata()?;
        Ok(TopLevel::CompileTimeTxn(Transaction {
            name, type_params, parameters,
            output_type: output_type.clone(),
            outputs: vec![],
            contract, body, metadata,
            is_reactive: true, is_async: false,
            derivation, modifiers: vec![], span: None, doc: self.take_doc(),
        }))
    }

    /// $let name = expr; / $const name = expr; — compile-time variable.
    /// 2026-07-25: Mutable ($let) or immutable ($const). Removed before codegen.
    fn parse_compile_time_let(&mut self, is_const: bool) -> Result<TopLevel, SyntaxError> {
        self.pos += 1; // consume $let or $const identifier
        let name = self.expect_identifier()?;
        self.expect(Token::Eq)?;
        let expr = self.parse_expression()?;
        self.expect(Token::Semicolon)?;
        if is_const {
            Ok(TopLevel::CompileTimeConst(name, expr))
        } else {
            Ok(TopLevel::CompileTimeLet(name, expr))
        }
    }

    // ── Protocol Declaration: proto name: #Category [contract] { ... } ──
    // 2026-07-23: Declares a protocol variant with CastTo/CastFrom edges
    // and optional cross-variant op overrides.
    fn parse_protocol_def(&mut self) -> Result<ProtocolDef, SyntaxError> {
        self.pos += 1; // consume "proto" identifier
        let name = self.expect_identifier()?;
        self.expect(Token::Colon)?;

        // Parse the category hashword: #String, #Float, etc.
        let category_type = self.parse_type()?;
        let category = match &category_type {
            Type::HashWord(cat) => cat.strip_prefix('#').unwrap_or(cat).to_string(),
            Type::HashWordVariant(cat, _) => cat.strip_prefix('#').unwrap_or(cat).to_string(),
            _ => return self.error_at_current(&format!(
                "expected protocol category hashword like '#String', got '{}'", category_type
            )),
        };

        // Parse optional contract [expr]
        let contract = self.parse_optional_protocol_contract();

        // Parse body: { CastTo(...); CastFrom(...); op ...; }
        let mut cast_edges = Vec::new();
        let mut cross_ops = Vec::new();

        if self.eat(&Token::LBrace) {
            while !self.check(&Token::RBrace) && !self.is_at_end() {
                let item_name = self.expect_identifier()?;
                if item_name == "CastTo" || item_name == "CastFrom" {
                    let direction = if item_name == "CastTo" {
                        CastDirection::CastTo
                    } else {
                        CastDirection::CastFrom
                    };
                    self.expect(Token::LParen)?;
                    let target_type = self.parse_type()?;
                    let (target_category, target_variant) = match &target_type {
                        Type::HashWordVariant(cat, var) => (
                            cat.strip_prefix('#').unwrap_or(cat).to_string(),
                            var.clone(),
                        ),
                        Type::HashWord(cat) => (
                            cat.strip_prefix('#').unwrap_or(cat).to_string(),
                            String::new(),
                        ),
                        _ => return self.error_at_current(&format!(
                            "expected protocol variant like '#String<UTF8>', got '{}'", target_type
                        )),
                    };
                    self.expect(Token::RParen)?;
                    // Check for binding: = fn_name(#L)
                    let binding = if self.eat(&Token::Eq) {
                        let impl_args = self.parse_metadata_value_standalone()?;
                        let fn_name = match &impl_args {
                            PropertyValue::List(items) => {
                                if let Some(PropertyValue::Identifier(name)) = items.first() {
                                    name.clone()
                                } else { format!("{:?}", impl_args) }
                            }
                            PropertyValue::Identifier(name) => name.clone(),
                            _ => format!("{:?}", impl_args),
                        };
                        Some(CastBinding { fn_name, param: "L".to_string() })
                    } else {
                        None
                    };
                    self.eat(&Token::Semicolon);
                    cast_edges.push(CastEdge { direction, target_category, target_variant, binding });
                } else if item_name == "op" {
                    let op_name = self.expect_identifier()?;
                    self.expect(Token::LParen)?;
                    let params = if !self.check(&Token::RParen) {
                        let mut p = Vec::new();
                        loop {
                            p.push(self.parse_type()?);
                            if !self.eat(&Token::Comma) { break; }
                        }
                        p
                    } else {
                        vec![]
                    };
                    self.expect(Token::RParen)?;
                    // Optional return type: -> Type
                    if self.eat(&Token::Arrow) {
                        let _ret = self.parse_type()?;
                    }
                    // Optional binding: = fn(#L, #R)
                    let impl_args = if self.eat(&Token::Eq) {
                        Some(self.parse_metadata_value_standalone()?)
                    } else {
                        None
                    };
                    self.eat(&Token::Semicolon);
                    cross_ops.push(OperatorDef {
                        op: op_name,
                        params,
                        pre: None,
                        suf: None,
                        impl_args,
                        impl_name: String::new(),
                        span: None,
                    });
                } else {
                    return self.error_at_current(&format!(
                        "expected 'CastTo', 'CastFrom', or 'op' in protocol body, got '{}'", item_name
                    ));
                }
            }
            self.expect(Token::RBrace)?;
        }

        Ok(ProtocolDef {
            name,
            category,
            contract,
            cast_edges,
            cross_ops,
            span: None,
        })
    }

    /// Parse optional contract in a protocol declaration.
    /// Returns None if no contract is present.
    fn parse_optional_protocol_contract(&mut self) -> Option<Contract> {
        if self.check(&Token::LBracket) {
            let saved = self.pos;
            // Check if this looks like a contract bracket, not something else
            // parse_contract handles [pre][post] pairs. For protocol, we want
            // just [pre] — a single invariant.
            let contract = self.parse_contract().ok()?;
            Some(contract)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lexer::tokenize;
    use crate::parser::Parser;
    use crate::ast::top::{CastDirection, ProtocolDef};

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
        assert_eq!(fb.foreign_name, "strlen");
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
        assert_eq!(fb.foreign_name, "hash");
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
        assert_eq!(fb.foreign_name, "print");
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
        // Bare #String resolves to UTF8 (universal default)
        let ty = parse_type("#String").unwrap();
        assert_eq!(ty, crate::ast::Type::HashWordVariant("#String".into(), "UTF8".into()));
    }

    #[test]
    fn test_hashword_string_with_explicit_variant() {
        let ty = parse_type("#String<UTF8>").unwrap();
        assert_eq!(ty, crate::ast::Type::HashWordVariant("#String".into(), "UTF8".into()));
    }

    #[test]
    fn test_hashword_string_with_explicit_ASCII_variant() {
        let ty = parse_type("#String<ASCII>").unwrap();
        assert_eq!(ty, crate::ast::Type::HashWordVariant("#String".into(), "ASCII".into()));
    }

    #[test]
    fn test_hashword_float_with_explicit_variant() {
        let ty = parse_type("#Float<IEEE754>").unwrap();
        assert_eq!(ty, crate::ast::Type::HashWordVariant("#Float".into(), "IEEE754".into()));
    }

    // ── Op declaration parsing ───────────────────────────────────────

    fn parse_op_from_type_def(src: &str) -> Vec<crate::ast::top::OperatorBinding> {
        let tokens = crate::lexer::tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        match p.parse_top_level() {
            Ok(crate::ast::TopLevel::TypeDef(td)) => td.body.op_bindings,
            _ => panic!("expected TypeDef"),
        }
    }

    #[test]
    fn test_op_declarative_hashword() {
        let ops = parse_op_from_type_def("type T { op Add: int_add(#L, #R); };");
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].name, "Add");
        assert!(ops[0].protocol_variant.is_none());
    }

    #[test]
    fn test_op_declarative_protocol_variant() {
        let ops = parse_op_from_type_def("type T { op Add(#Int): int_add(#L, #R); };");
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].name, "Add");
        assert_eq!(ops[0].protocol_variant.as_deref(), Some("#Int"));
    }

    #[test]
    fn test_op_binding_with_markers() {
        let ops = parse_op_from_type_def(
            "type T { op InsertAt: push(#L, #R); };"
        );
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].name, "InsertAt");
        assert!(ops[0].protocol_variant.is_none());
    }

    // ── Protocol declaration parsing ──────────────────────────────

    fn parse_protocol(src: &str) -> ProtocolDef {
        let tokens = crate::lexer::tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        match p.parse_top_level() {
            Ok(crate::ast::TopLevel::ProtocolDef(pd)) => pd,
            Ok(other) => panic!("expected ProtocolDef, got {:?}", other),
            Err(e) => panic!("parse error: {}", e),
        }
    }

    #[test]
    fn test_protocol_def_edges_only() {
        let pd = parse_protocol("proto ASCII: #String { CastTo(#String<UTF8>); };");
        assert_eq!(pd.name, "ASCII");
        assert_eq!(pd.category, "String");
        assert_eq!(pd.cast_edges.len(), 1);
        assert_eq!(pd.cast_edges[0].direction, CastDirection::CastTo);
        assert_eq!(pd.cast_edges[0].target_category, "String");
        assert_eq!(pd.cast_edges[0].target_variant, "UTF8");
        assert!(pd.cross_ops.is_empty());
        assert!(pd.contract.is_none());
    }

    #[test]
    fn test_protocol_def_cross_op() {
        let pd = parse_protocol(
            "proto ASCII: #String { CastTo(#String<UTF8>); op Add(#String<UTF8>) = add_UTF8_to_ASCII(#L, #R); };"
        );
        assert_eq!(pd.name, "ASCII");
        assert_eq!(pd.cast_edges.len(), 1);
        assert_eq!(pd.cross_ops.len(), 1);
        assert_eq!(pd.cross_ops[0].op, "Add");
        assert!(pd.cross_ops[0].impl_args.is_some());
    }

    #[test]
    fn test_protocol_def_with_contract() {
        let pd = parse_protocol(
            "proto ASCII: #String [#Self < 128] { CastTo(#String<UTF8>); };"
        );
        assert_eq!(pd.name, "ASCII");
        assert!(pd.contract.is_some(), "contract should be parsed");
        assert_eq!(pd.cast_edges.len(), 1);
    }

    #[test]
    fn test_protocol_def_empty_body() {
        let pd = parse_protocol("proto ASCII: #String {};");
        assert_eq!(pd.name, "ASCII");
        assert_eq!(pd.cast_edges.len(), 0);
        assert_eq!(pd.cross_ops.len(), 0);
    }

    #[test]
    fn test_protocol_def_both_edges() {
        let pd = parse_protocol(
            "proto ASCII: #String { CastTo(#String<UTF8>); CastFrom(#String<UTF8>); };"
        );
        assert_eq!(pd.cast_edges.len(), 2);
        assert_eq!(pd.cast_edges[0].direction, CastDirection::CastTo);
        assert_eq!(pd.cast_edges[1].direction, CastDirection::CastFrom);
    }

    #[test]
    fn test_protocol_def_multiple_edges() {
        let pd = parse_protocol(
            "proto multi: #String { CastTo(#String<UTF8>); CastTo(#String<UTF16>); };"
        );
        assert_eq!(pd.cast_edges.len(), 2);
        assert_eq!(pd.cast_edges[0].target_variant, "UTF8");
        assert_eq!(pd.cast_edges[1].target_variant, "UTF16");
    }

    // ── Slice expression parsing ──────────────────────────────────

    #[test]
    fn test_parse_slice_contiguous() {
        let src = "arr[2:8:1]";
        let tokens = crate::lexer::tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        let expr = p.parse_expression().unwrap();
        match expr {
            crate::ast::Expr::Slice { array, start, end, stride } => {
                assert_eq!(format!("{}", array), "arr");
                assert!(start.is_some()); assert_eq!(format!("{}", start.unwrap()), "2");
                assert!(end.is_some()); assert_eq!(format!("{}", end.unwrap()), "8");
                assert!(stride.is_some()); assert_eq!(format!("{}", stride.unwrap()), "1");
            }
            _ => panic!("expected Expr::Slice"),
        }
    }

    #[test]
    fn test_parse_slice_implicit_start() {
        let src = "arr[:10]";
        let tokens = crate::lexer::tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        let expr = p.parse_expression().unwrap();
        match expr {
            crate::ast::Expr::Slice { array, start, end, stride } => {
                assert!(start.is_none());
                assert!(end.is_some()); assert_eq!(format!("{}", end.unwrap()), "10");
                assert!(stride.is_none());
            }
            _ => panic!("expected Expr::Slice"),
        }
    }

    #[test]
    fn test_parse_slice_implicit_stride() {
        let src = "arr[2:8]";
        let tokens = crate::lexer::tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        let expr = p.parse_expression().unwrap();
        match expr {
            crate::ast::Expr::Slice { array, start, end, stride } => {
                assert!(start.is_some()); assert_eq!(format!("{}", start.unwrap()), "2");
                assert!(end.is_some()); assert_eq!(format!("{}", end.unwrap()), "8");
                assert!(stride.is_none());
            }
            _ => panic!("expected Expr::Slice"),
        }
    }

    #[test]
    fn test_parse_slice_dynamic_bounds() {
        let src = "arr[i:j:k]";
        let tokens = crate::lexer::tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        let expr = p.parse_expression().unwrap();
        match expr {
            crate::ast::Expr::Slice { array, start, end, stride } => {
                assert_eq!(format!("{}", array), "arr");
                assert!(start.is_some()); assert_eq!(format!("{}", start.unwrap()), "i");
                assert!(end.is_some()); assert_eq!(format!("{}", end.unwrap()), "j");
                assert!(stride.is_some()); assert_eq!(format!("{}", stride.unwrap()), "k");
            }
            _ => panic!("expected Expr::Slice"),
        }
    }

    #[test]
    fn test_parse_slice_full_view() {
        let src = "arr[:]";
        let tokens = crate::lexer::tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        let expr = p.parse_expression().unwrap();
        match expr {
            crate::ast::Expr::Slice { array, start, end, stride } => {
                assert!(start.is_none());
                assert!(end.is_none());
                assert!(stride.is_none());
            }
            _ => panic!("expected Expr::Slice"),
        }
    }

    // ── Prop declaration parsing ──────────────────────────────────

    fn parse_prop_from_type_def(src: &str) -> Vec<crate::ast::top::PropDef> {
        let tokens = crate::lexer::tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        match p.parse_top_level() {
            Ok(crate::ast::TopLevel::TypeDef(td)) => td.body.props,
            Ok(other) => panic!("expected TypeDef, got {:?}", other),
            Err(e) => panic!("parse error: {:?}", e),
        }
    }

    #[test]
    fn test_parse_prop_declaration() {
        let props = parse_prop_from_type_def("type T { prop Size: chars(#L); };");
        assert_eq!(props.len(), 1);
        assert_eq!(props[0].name, "Size");
    }

    #[test]
    fn test_parse_multiple_props() {
        let props = parse_prop_from_type_def(
            "type T { prop Size: chars(#L); prop Bytes: len(#L); };");
        assert_eq!(props.len(), 2);
        assert_eq!(props[0].name, "Size");
        assert_eq!(props[1].name, "Bytes");
    }

    // ── Render struct/obj parsing ─────────────────────────────────

    fn parse_render_block(src: &str) -> crate::ast::RenderBlock {
        let tokens = crate::lexer::tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        match p.parse_top_level() {
            Ok(crate::ast::TopLevel::RenderBlock(rb)) => rb,
            Ok(other) => panic!("expected RenderBlock, got {:?}", other),
            Err(e) => panic!("parse error: {:?}", e),
        }
    }

    #[test]
    fn test_parse_render_struct() {
        let rb = parse_render_block(
            "render struct Foo { <div>Hello</div> };");
        assert_eq!(rb.struct_name, "Foo");
        assert!(rb.view_html.contains("<div>Hello</div>"),
            "HTML content should be preserved: got '{}'", rb.view_html);
    }

    #[test]
    fn test_parse_render_obj() {
        let rb = parse_render_block(
            "render obj Bar { <span b-text=\"x\">0</span> };");
        assert_eq!(rb.struct_name, "Bar");
        assert!(rb.view_html.contains("b-text"),
            "HTML should include b-* attribute: got '{}'", rb.view_html);
    }

    #[test]
    fn test_parse_render_struct_with_style_attr() {
        let rb = parse_render_block(
            "render struct Styled { <div class=\"box\" style=\"color: red;\">Content</div> };");
        assert_eq!(rb.struct_name, "Styled");
        assert!(rb.view_html.contains("class=\"box\""),
            "HTML should preserve attributes: got '{}'", rb.view_html);
    }

    #[test]
    fn test_parse_render_struct_nested_tags() {
        let rb = parse_render_block(
            "render struct Nest { <ul><li b-each:item=\"list\" b-text=\"item\"></li></ul> };");
        assert_eq!(rb.struct_name, "Nest");
        assert!(rb.view_html.contains("b-each:item"),
            "HTML should preserve b-each: got '{}'", rb.view_html);
    }

    #[test]
    fn test_parse_render_rejects_bare_identifier() {
        let src = "render foo { <div></div> };";
        let tokens = crate::lexer::tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        let result = p.parse_top_level();
        assert!(result.is_err(), "bare 'render foo' should be rejected");
    }

    // ── Tagged literal + Parse op discriminator tests ────────────────────

    #[test]
    fn test_tagged_literal_suffix() {
        let src = "42km";
        let tokens = tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        let expr = p.parse_expression().unwrap();
        match expr {
            crate::ast::Expr::TaggedLiteral(n, ref tag) => {
                assert_eq!(n, 42);
                assert_eq!(tag, "km");
            }
            _ => panic!("expected TaggedLiteral(42, \"km\")"),
        }
    }

    #[test]
    fn test_tagged_literal_hex_suffix() {
        let src = "0xFFh";
        let tokens = tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        let expr = p.parse_expression().unwrap();
        match expr {
            crate::ast::Expr::TaggedLiteral(n, ref tag) => {
                assert_eq!(n, 0xFF);
                assert_eq!(tag, "h");
            }
            _ => panic!("expected TaggedLiteral(255, \"h\")"),
        }
    }

    #[test]
    fn test_tagged_literal_no_suffix_with_space() {
        // Space between literal and identifier: not a suffix
        let src = "42 km";
        let tokens = tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        let expr = p.parse_expression().unwrap();
        match expr {
            crate::ast::Expr::Decimal(n) => assert_eq!(n, 42),
            _ => panic!("expected Decimal(42) with space separator"),
        }
    }

    #[test]
    fn test_tagged_quoted_prefix() {
        let src = r#"sql"SELECT * FROM users""#;
        let tokens = tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        let expr = p.parse_expression().unwrap();
        match expr {
            crate::ast::Expr::TaggedQuotedLiteral(ref bytes, ref prefix) => {
                assert_eq!(bytes, b"SELECT * FROM users");
                assert_eq!(prefix, "sql");
            }
            _ => panic!("expected TaggedQuotedLiteral, got {:?}", expr),
        }
    }

    #[test]
    fn test_tagged_quoted_prefix_no_false_positive_with_space() {
        // Space between identifier and string: not a prefix
        let src = r#"my "hello""#;
        let tokens = tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        let expr = p.parse_expression().unwrap();
        match expr {
            crate::ast::Expr::Identifier(ref name) => {
                assert_eq!(name, "my");
                // The string "hello" is a separate expression — not consumed
            }
            _ => panic!("expected Identifier(\"my\") with space separator, got {:?}", expr),
        }
    }

    #[test]
    fn test_op_parse_with_pre_discriminator() {
        let ops = parse_op_from_type_def(
            "type T { op Parse(Decimal, pre:\"0x\"): parse_hex(#L); };"
        );
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].name, "Parse");
        assert_eq!(ops[0].protocol_variant.as_deref(), Some("Decimal"));
        assert_eq!(ops[0].pre.as_deref(), Some("0x"));
        assert!(ops[0].suf.is_none());
        assert!(ops[0].reg.is_none());
    }

    #[test]
    fn test_op_parse_with_suf_discriminator() {
        let ops = parse_op_from_type_def(
            "type T { op Parse(Decimal, suf:\"km\"): parse_km(#L); };"
        );
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].name, "Parse");
        assert_eq!(ops[0].suf.as_deref(), Some("km"));
    }

    #[test]
    fn test_op_parse_with_regex_discriminator() {
        let ops = parse_op_from_type_def(
            "type T { op Parse(Decimal, reg:\"[0-9]+\"): parse_num(#L); };"
        );
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].reg.as_deref(), Some("[0-9]+"));
    }

    #[test]
    fn test_op_parse_multiple_discriminators() {
        let ops = parse_op_from_type_def(
            "type T { op Parse(Decimal, pre:\"0x\", suf:\"h\"): parse_hex(#L); };"
        );
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].pre.as_deref(), Some("0x"));
        assert_eq!(ops[0].suf.as_deref(), Some("h"));
    }

    #[test]
    fn test_op_parse_quoted_form() {
        let ops = parse_op_from_type_def(
            "type T { op Parse(Quoted): parse_string(#L); };"
        );
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].protocol_variant.as_deref(), Some("Quoted"));
    }

    #[test]
    fn test_op_parse_bare_form() {
        let ops = parse_op_from_type_def(
            "type T { op Parse(Bare): parse_bool(#L); };"
        );
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].protocol_variant.as_deref(), Some("Bare"));
    }

    // ── 2026-07-31: Contracts in either position (pre/post return type) ──

    fn parse_defn(src: &str) -> Result<crate::ast::Definition, crate::errors::SyntaxError> {
        let tokens = tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        match p.parse_top_level()? {
            crate::ast::TopLevel::Definition(d) => Ok(d),
            other => panic!("expected Definition, got {:?}", std::mem::discriminant(&other)),
        }
    }

    fn parse_txn(src: &str) -> Result<crate::ast::Transaction, crate::errors::SyntaxError> {
        let tokens = tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        match p.parse_top_level()? {
            crate::ast::TopLevel::Transaction(t) => Ok(t),
            other => panic!("expected Transaction, got {:?}", std::mem::discriminant(&other)),
        }
    }

    fn is_single(out: &Option<crate::ast::OutputType>) -> bool {
        matches!(out, Some(crate::ast::OutputType::Single(_)))
    }

    fn has_pre(c: &crate::ast::Contract) -> bool {
        !matches!(c.pre_condition, crate::ast::Expr::Bool(true))
    }

    fn has_post(c: &crate::ast::Contract) -> bool {
        !matches!(c.post_condition, crate::ast::Expr::Bool(true))
    }

    #[test]
    fn test_defn_contract_after_return_type() {
        let d = parse_defn(
            "defn f(a: Int, b: Int) -> Int [b != 0][result == a / b] { term a / b; };",
        )
        .unwrap();
        assert!(is_single(&d.output_type));
        assert!(has_pre(&d.contract));
        assert!(has_post(&d.contract));
    }

    #[test]
    fn test_defn_contract_before_return_type() {
        let d = parse_defn(
            "defn f(a: Int, b: Int) [b != 0][result == a / b] -> Int { term a / b; };",
        )
        .unwrap();
        assert!(is_single(&d.output_type));
        assert!(has_pre(&d.contract));
        assert!(has_post(&d.contract));
    }

    #[test]
    fn test_defn_implicit_return_type_no_contract() {
        let d = parse_defn("defn f(a: Int, b: Int) { term a + b; };").unwrap();
        assert!(d.output_type.is_none());
        assert!(!has_pre(&d.contract));
        assert!(!has_post(&d.contract));
    }

    #[test]
    fn test_txn_contract_after_return_type() {
        let t = parse_txn(
            "txn f(a: Int) -> Bool [a > 0][a >= 0] { term a > 0; };",
        )
        .unwrap();
        assert!(is_single(&t.output_type));
        assert!(has_pre(&t.contract));
        assert!(has_post(&t.contract));
    }

    #[test]
    fn test_array_type_still_parses_with_contract_after() {
        // Int[8] is a vector; the following [pre] is the contract, not an
        // array size. Regression: parse_type must only consume `[` as an
        // array suffix when the next token is an integer literal.
        let d = parse_defn(
            "defn f(v: Int[8]) -> Int[8] [v[0] == 0][result == v[0]] { term v[0]; };",
        )
        .unwrap();
        assert!(is_single(&d.output_type));
        assert!(has_pre(&d.contract));
    }

    #[test]
    fn test_non_integer_bracket_left_for_contract() {
        // parse_type on `Int [b != 0]` must stop at `Int`, leaving the
        // bracket for the contract parser (not "expected array size").
        let ty = parse_type("Int [b != 0]").unwrap();
        assert_eq!(ty, crate::ast::Type::int());
    }

    // ── 2026-07-31 (Phase 3): Watchdog parsing ───────────────────

    #[test]
    fn test_watchdog_optional_parses() {
        let t = parse_txn(
            "txn f() [true][done] ?[5000ms] { term; };",
        )
        .unwrap();
        let w = t.contract.watchdog.expect("watchdog must parse");
        assert!(!w.is_required);
    }

    #[test]
    fn test_watchdog_required_parses() {
        let t = parse_txn(
            "txn f() [true][done] ![1000ms] { term; };",
        )
        .unwrap();
        let w = t.contract.watchdog.expect("watchdog must parse");
        assert!(w.is_required);
    }

    #[test]
    fn test_contract_without_watchdog() {
        let t = parse_txn("txn f() [true][done] { term; };").unwrap();
        assert!(t.contract.watchdog.is_none());
    }

    // ── 2026-08-01 (Phase 2): `[#]` entry marker removal ──────────────

    #[test]
    fn test_entry_hash_bracket_is_syntax_error() {
        // 2026-08-01 (Phase 2): `[#]` is no longer an entry-point marker — it
        // must be rejected with a clear error, NOT silently parsed as a
        // precondition referencing `#` or a `Type[#]` array dimension.
        // Contract position: `txn f() [#]`.
        let err = parse_txn("txn f() [#] { term; };").unwrap_err();
        let msg = format!("{}", err);
        assert!(
            msg.contains("entry-point syntax removed"),
            "expected a clear '[#] removed' error, got: {msg}"
        );
        // After the return type: `defn main() -> Int [#]` (the classic form).
        let err = parse_defn("defn main() -> Int [#] { term 0; };").unwrap_err();
        assert!(
            format!("{}", err).contains("entry-point syntax removed"),
            "expected '[#] removed' error for '-> Int [#]', got: {}",
            err
        );
        // `[#][post]` form is rejected too.
        let err = parse_txn("txn f() [#][r == 0] { term; };").unwrap_err();
        assert!(format!("{}", err).contains("entry-point syntax removed"));
    }

    #[test]
    fn test_plain_contract_still_parses() {
        // 2026-08-01 (Phase 2): removing `[#]` must not disturb ordinary
        // contracts — pre/post still parse.
        let t = parse_txn("txn f() [x > 0][done] { term; };").unwrap();
        assert!(t.contract.watchdog.is_none());
        assert!(t.contract.explicit);
    }
}
