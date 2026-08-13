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
        if self.eat(&Token::Export) {
            return self.parse_export();
        }
        match self.peek() {
            Some(Token::Defn) => self.parse_definition().map(TopLevel::Definition),
            Some(Token::Txn) => self
                .parse_transaction(false, false)
                .map(TopLevel::Transaction),
            Some(Token::Node) => self.parse_node().map(TopLevel::Transaction),
            // 2026-08-01 (Phase 3c): `sync<group> node name ...` — a reactive
            // node classified into a group barrier. Members that fire hold off
            // finishing until all fired members have (rule #21 classification).
            Some(Token::Sync) => self.parse_sync_group(),
            // 2026-08-01 (Phase E): `seq node name` / `seq txn name` — the seq
            // modifier requests sequential dispatch (no emit_parallel_reactor)
            // and/or non-vectorized array access. Recorded as a modifier
            // annotation; the backend consumes it (never a speed win — a
            // modifier-beaten default is a compiler bug).
            Some(Token::Pack) => self.parse_struct_def().map(TopLevel::StaticStruct),
            Some(Token::Seq) if matches!(self.tokens.get(self.pos + 1).map(|(t, _)| t), Some(Token::Struct) | Some(Token::Pack)) => {
                // 2026-08-05 (Phase 4): `seq struct` — parse_struct_def consumes
                // the seq modifier itself. 2026-08-13: `pack`/`seq` prefixes are
                // order-independent (`pack seq struct`, `seq pack struct`); the
                // struct parser consumes whichever flags precede `struct`.
                self.parse_struct_def().map(TopLevel::StaticStruct)
            }
            Some(Token::Seq) if matches!(self.tokens.get(self.pos + 1).map(|(t, _)| t), Some(Token::Node) | Some(Token::Txn)) => {
                self.pos += 1; // consume seq
                let mut txn = if matches!(self.tokens.get(self.pos).map(|(t, _)| t), Some(Token::Node)) {
                    self.parse_node().map(TopLevel::Transaction)?
                } else {
                    self.parse_transaction(false, false).map(TopLevel::Transaction)?
                };
                if let TopLevel::Transaction(t) = &mut txn {
                    t.modifiers.push(Annotation {
                        name: "seq".to_string(),
                        value: None,
                    });
                }
                Ok(txn)
            }
            // 2026-07-31: `async node` (prefix) — same as `node async`.
            // 2026-08-01 (Phase 3c): the prefix form must preserve the async
            // flag. parse_node reads is_async via eat(Async) AFTER consuming
            // 'node'; with the prefix the async token is already consumed, so
            // we set it on the returned Transaction.
            Some(Token::Async) if matches!(self.tokens.get(self.pos + 1).map(|(t, _)| t), Some(Token::Node) | Some(Token::Accel)) => {
                self.pos += 1; // consume async
                // 2026-08-06 (accel plan): `async accel node name ...` — an
                // accel body whose co-firing is explicitly acknowledged (the
                // phase/counter flags sequence it at runtime).
                let mut txn = if self.check(&Token::Accel) {
                    self.pos += 1; // consume accel
                    let mut t = self.parse_node()?;
                    t.modifiers.push(Annotation {
                        name: "accel".to_string(),
                        value: None,
                    });
                    t
                } else {
                    self.parse_node()?
                };
                txn.is_async = true;
                Ok(TopLevel::Transaction(txn))
            }
            // 2026-08-06 (accel plan): `accel node name` / `accel txn name` —
            // GPU-deferral request. Marks the body as a per-firing parallel
            // map over work-items; the backend defers execution to the GPU
            // only when it verifies a speedup, else silent CPU fallback. See
            // docs/plans/2026-08-06-accel-gpu-offload.md.
            Some(Token::Accel) => {
                self.pos += 1; // consume accel
                let mut txn = if self.check(&Token::Node) {
                    self.parse_node().map(TopLevel::Transaction)?
                } else if self.check(&Token::Txn) {
                    self.parse_transaction(false, false).map(TopLevel::Transaction)?
                } else {
                    return self.error_at_current("expected 'node' or 'txn' after 'accel'");
                };
                if let TopLevel::Transaction(t) = &mut txn {
                    t.modifiers.push(Annotation {
                        name: "accel".to_string(),
                        value: None,
                    });
                }
                Ok(txn)
            }
            // 2026-08-04 (out-observability plan): `out defn` / `out node` /
            // `out txn` / `out let` — the observability pin. Marks the
            // callable's calls (or the variable's reads/writes) as liveness
            // roots. Never an acceleration — a pin the compiler must respect.
            Some(Token::Out)
                if matches!(
                    self.tokens.get(self.pos + 1).map(|(t, _)| t),
                    Some(Token::Defn)
                        | Some(Token::Node)
                        | Some(Token::Txn)
                        | Some(Token::Let)
                        | Some(Token::Vol)
                ) =>
            {
                self.pos += 1; // consume out
                match self.peek() {
                    Some(Token::Defn) => {
                        let mut defn = self.parse_definition()?;
                        defn.modifiers.push(Annotation {
                            name: "out".to_string(),
                            value: None,
                        });
                        Ok(TopLevel::Definition(defn))
                    }
                    Some(Token::Node) => {
                        let mut txn = self.parse_node()?;
                        txn.modifiers.push(Annotation {
                            name: "out".to_string(),
                            value: None,
                        });
                        Ok(TopLevel::Transaction(txn))
                    }
                    Some(Token::Txn) => {
                        let mut txn = self.parse_transaction(false, false)?;
                        txn.modifiers.push(Annotation {
                            name: "out".to_string(),
                            value: None,
                        });
                        Ok(TopLevel::Transaction(txn))
                    }
                    _ => {
                        // `out let` or `out vol let` — recurse through the
                        // statement parser so the let (and any vol) modifiers
                        // are recorded, then push `out` last.
                        let mut stmt = self.parse_statement()?;
                        if let Statement::Let { modifiers, .. } = &mut stmt {
                            modifiers.push(Annotation {
                                name: "out".to_string(),
                                value: None,
                            });
                        }
                        Ok(TopLevel::Statement(Box::new(stmt)))
                    }
                }
            }
            Some(Token::Cell) => self.parse_cell().map(TopLevel::Cell),
            Some(Token::Import) => self.parse_import().map(TopLevel::Import),
            // 2026-08-09 (Phase 12, SPEC §19.6): `meld` is removed — foreign
            // shapes adapt through GLUE/Data Briev descriptors, explicit
            // protocol cast edges, ownership contracts, and effects. Rejected
            // as staged (SPEC §25), not silently accepted.
            Some(Token::Meld) => Err(SyntaxError::StagedFeature {
                feature: "meld declarations are removed — foreign shapes adapt through \
                         GLUE descriptors, explicit protocol cast edges, ownership \
                         contracts, and effects"
                    .into(),
                span: self.peek_with_span().map(|(_, s)| self.make_span(s.clone())).unwrap_or_else(crate::errors::Span::dummy),
            }),
            Some(Token::Trg) => self.parse_top_level_trg().map(TopLevel::Trigger),
            // 2026-08-06 (accel plan): top-level `!> key: value;` module
            // metadata (SPEC §8.9). Multiple consecutive bindings merge into
            // one ModuleMetadata node; last binding wins per key.
            Some(Token::ExclaimArrow) => {
                let mut map = std::collections::HashMap::new();
                while self.check(&Token::ExclaimArrow) {
                    self.pos += 1; // consume '!>'
                    let key = self.parse_metadata_key()?;
                    self.expect(Token::Colon)?;
                    let val = self.parse_metadata_value()?;
                    self.expect(Token::Semicolon)?;
                    map.insert(key, val);
                }
                Ok(TopLevel::ModuleMetadata(map))
            }
            // 2026-07-14: Handle `type Name : Parent { slots }` definitions
            // 2026-07-16: P2 — Check for extension group Type.[a,b,c] before single type
            Some(Token::Type) => self.parse_type_or_group().map(TopLevel::TypeDef),
            // 2026-08-05 (Phase 4): trait / impl declarations
            Some(Token::Trait) => self.parse_trait().map(TopLevel::Trait),
            Some(Token::Impl) => self.parse_impl().map(TopLevel::Impl),
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
            // 2026-08-05 (Phase 3): `frgn` declarations. `optional frgn` is the
            // only modifier; `frgn!`, `frgn?`, `frgn?!` are removed (SPEC §19.3).
            Some(Token::Frgn) => {
                self.advance();
                let mut fb = self.parse_frgn_decl()?;
                fb.is_optional = false;
                fb.is_fire_forget = false;
                fb.is_delivery = false;
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
                // 2026-08-05 (Phase 3): `optional frgn` — optional foreign
                // symbol availability (SPEC §19.3). `frgn?` is removed.
                if self.check_identifier("optional") && self.peek_next_is_frgn() {
                    self.pos += 1; // consume 'optional'
                    self.advance(); // consume 'frgn'
                    let mut fb = self.parse_frgn_decl()?;
                    fb.is_optional = true;
                    fb.is_fire_forget = false;
                    fb.is_delivery = false;
                    return Ok(TopLevel::ForeignBinding(fb));
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
                // 2026-08-09: `init` is a contextual keyword — top-level
                // declaration form only, matching the proto/asm pattern.
                // It stays a legal identifier elsewhere (e.g. `txn init(...)`
                // method + `op Init: init(#Lh,#Rh)` bindings in stdlib).
                if self.check_identifier("init") {
                    self.pos += 1; // consume `init` identifier
                    return Ok(TopLevel::Init(self.parse_init_declaration()?));
                }
                // 2026-08-04 (remove-vestigial-return): Briev has no `return`
                // statement — give the same helpful error at top level as in
                // statement bodies (src/parser/statements.rs parse_statement).
                if self.check_identifier("return") {
                    self.pos += 1; // consume `return`
                    return Err(SyntaxError::InvalidStatement {
                        reason: "Briev has no `return` statement. To return a value \
                                 from a defn use `term <value>`; to mark a convergence \
                                 checkpoint use bare `term;`; `term!` closes the program."
                            .to_string(),
                        span: crate::errors::Span::dummy(),
                    });
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
        // 2026-08-05 (Phase 3): `render Name { ... }` is the sole attachment
        // form (SPEC §21.2). The compiler resolves whether Name is a struct,
        // obj, or cell; `render struct`/`render obj` keywords are removed.
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
    ///   frgn <foreign_symbol>(<params>) [-> <ret>] [as <briev_name>] from <source> [target "c"] [fallback <expr>];
    ///   frgn <foreign_symbol>(<params>) [-> <ret>] [as <briev_name>] from <source> [target "c"] [fallback <fn>(<args>)];
    ///   frgn <foreign_symbol>(<params>) [-> <ret>] [as <briev_name>] from <source> [target "c"] [fallback ;];
    ///
    /// `from` is required (provenance for the foreign module).
    /// `as` is optional and comes before `from` (Briev name, different from the C symbol).
    fn parse_frgn_decl(&mut self) -> Result<ForeignBinding, SyntaxError> {
        // 2026-08-09 (Phase 12, SPEC §19.1): the declaration name is the LOCAL
        // Briev name; a `:` binds a DIFFERENT external (link) symbol. `as` is
        // not an alias operator (removed — the 2026-07-22 inversion).
        let local_name = self.expect_identifier()?;

        // 2026-08-09 (SPEC §19.7): `frgn name @ address` (MMIO) is invalid —
        // memory-mapped I/O uses configured ports or explicit intrinsics.
        // We can't see `@` until after the params; the post-param check
        // rejects it. (Also rejected before `(` by the caller's dispatch.)

        self.expect(Token::LParen)?;
        let mut inputs = Vec::new();
        let mut is_variadic = false;
        while !self.check(&Token::RParen) {
            // 2026-08-09 (SPEC §19.4): `variadic args: ForeignArgs` — an
            // explicit final named variadic parameter. `...` is reserved for
            // slicing, so the marker is the `variadic` keyword.
            if self.check_identifier("variadic") {
                if is_variadic {
                    return self.error_at_current("only one `variadic` parameter is allowed");
                }
                is_variadic = true;
                self.pos += 1; // consume `variadic`
            }
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

        // 2026-08-09 (SPEC §19.1): `: external_symbol` binds the link symbol.
        // Absent → the local name IS the external symbol. The old `as
        // <briev_name>` form is removed (it inverted the names).
        let (foreign_name, briev_name): (String, Option<String>) = if self.eat(&Token::Colon) {
            (self.expect_identifier()?, Some(local_name))
        } else {
            (local_name.clone(), None)
        };

        // 2026-08-09 (SPEC §19.7): reject the MMIO address form `frgn name @
        // address` after the signature (e.g. `frgn x(...) @ 0x...`).
        if self.check(&Token::At) {
            return self.error_at_current(
                "frgn `@ address` (MMIO) is invalid — use configured device/cell \
                 ports or explicit pointer/address intrinsics",
            );
        }

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

        // 2026-08-09 (SPEC §19.3): the declaration-level `fallback` clause is
        // removed — fallback behavior uses ordinary typed control flow (the
        // `optional frgn` + `.^^Available` check). Rejected as staged, not
        // silently accepted.
        if self.check_identifier("fallback") {
            return self.error_at_current(
                "`fallback` clause removed (SPEC 19.3) — use `optional frgn` + \
                 `feature.^^Available` and ordinary typed control flow instead",
            );
        }

        self.expect(Token::Semicolon)?;
        Ok(ForeignBinding {
            foreign_name,
            briev_name,
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
            span: None,
            doc: self.take_doc(),
            is_optional: false,
            is_fire_forget: false,
            is_delivery: false,
            is_variadic,
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
        // 2026-08-01 (Phase 4): implicit entry wrapping is owned by the script
        // plugin (script_plugin.rs) — it synthesizes the one-shot opening node.
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

    /// Parse: `init name: [bound_set] Type (= expr | { body })`.
    /// 2026-08-09: runtime-seeded invariant — the fourth top-level kind.
    /// The bound set is kind-attached (between `:` and the type) so it is not
    /// misread as an array dimension (`Int[16]` is containment, `:[..] Int`
    /// is an expected-value set). Body form seeds once before beginprogram.
    /// Caller has already consumed the `init` identifier token.
    fn parse_init_declaration(&mut self) -> Result<InitDecl, SyntaxError> {
        let name = self.expect_identifier()?;
        self.expect(Token::Colon)?;
        let bound = if self.check(&Token::LBracket) {
            Some(self.parse_bound_set()?)
        } else {
            None
        };
        let ty = self.parse_type()?;
        let mut value = None;
        let mut body = Vec::new();
        if self.eat(&Token::Eq) {
            value = Some(self.parse_expression()?);
            self.expect(Token::Semicolon)?;
        } else if self.check(&Token::LBrace) {
            body = self.parse_block()?;
            self.expect(Token::Semicolon)?;
        } else {
            return self.error_at_current(
                "expected '=' with a seeding expression or '{ ... }' body after the \
                 init declaration's type",
            );
        }
        Ok(InitDecl {
            name,
            bound,
            ty,
            value,
            body,
            span: None,
            doc: self.take_doc(),
        })
    }

    /// Parse a bound set: `[ term | term | ... ]` where each term is a single
    /// literal/ref or a `lo..hi` range. Terms may mix literals and names.
    /// 2026-08-09: expected-value set for a bounded `init` (SPEC §8.1).
    fn parse_bound_set(&mut self) -> Result<BoundSpec, SyntaxError> {
        self.expect(Token::LBracket)?;
        let mut parts = Vec::new();
        loop {
            parts.push(self.parse_bound_part()?);
            if !self.eat(&Token::Pipe) {
                break;
            }
        }
        self.expect(Token::RBracket)?;
        let spec = if parts.len() == 1 {
            parts.pop().unwrap()
        } else {
            BoundSpec::Choice(parts)
        };
        Ok(spec)
    }

    fn parse_bound_part(&mut self) -> Result<BoundSpec, SyntaxError> {
        let lo = self.parse_bound_term()?;
        if self.eat(&Token::DotDot) {
            let hi = self.parse_bound_term()?;
            Ok(BoundSpec::Range(lo, hi))
        } else {
            Ok(BoundSpec::Single(lo))
        }
    }

    fn parse_bound_term(&mut self) -> Result<BoundTerm, SyntaxError> {
        match self.peek() {
            Some(Token::Integer(n)) => {
                let val = *n;
                self.pos += 1;
                Ok(BoundTerm::Lit(val))
            }
            Some(Token::Identifier(_)) => {
                let name = self.expect_identifier()?;
                Ok(BoundTerm::Ref(name))
            }
            _ => self.error_at_current(
                "expected a number or an identifier in an init bound set",
            ),
        }
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

    /// Parse: `sync<group> node name [pre][post] { body }`.
    /// 2026-08-01 (Phase 3c): classifies a reactive node into a group barrier.
    /// Members of the same group that fire hold off finishing until all fired
    /// members have — the concurrency gate accepts a pair when both are in a
    /// shared sync group (rule #21 classification).
    fn parse_sync_group(&mut self) -> Result<TopLevel, SyntaxError> {
        self.pos += 1; // consume 'sync'
        let domains = if self.eat(&Token::Lt) {
            let mut names = Vec::new();
            loop {
                let name = self.expect_identifier()?;
                names.push(name);
                if !self.eat(&Token::Comma) {
                    break;
                }
            }
            self.expect(Token::Gt)?;
            names
        } else {
            vec![]
        };
        // parse_node consumes `node [async] name ...` itself.
        // 2026-08-06 (accel plan): `sync<group> accel node name ...` — an
        // accel kernel classified into a group barrier.
        let node = if self.check(&Token::Accel) {
            self.pos += 1; // consume accel
            let mut txn = self.parse_node()?;
            txn.modifiers.push(Annotation {
                name: "accel".to_string(),
                value: None,
            });
            TopLevel::Transaction(txn)
        } else {
            TopLevel::Transaction(self.parse_node()?)
        };
        Ok(TopLevel::SyncGroup {
            domains,
            item: Box::new(node),
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
        // 2026-08-05 (Phase 2/11): angle paths accept slash-separated
        // components (`import <std/collections>;`) per SPEC §7.1.
        fn parse_import_path(parser: &mut Parser) -> Result<ImportKind, SyntaxError> {
            if parser.eat(&Token::Lt) {
                let first = parser.expect_identifier()?;
                let mut name = first;
                while parser.eat(&Token::Slash) {
                    let part = parser.expect_identifier()?;
                    name = format!("{}/{}", name, part);
                }
                parser.expect(Token::Gt)?;
                Ok(ImportKind::Registry(name))
            } else {
                let path = parser.expect_string()?;
                Ok(ImportKind::Literal(path))
            }
        }

        if self.eat(&Token::LBrace) {
            // Import with symbols: import { a, b: Renamed } from "module".
            // A `:` records a selective rename (local : exported). Globs are
            // invalid (SPEC §7.2).
            let mut symbols: Vec<(String, String)> = Vec::new();
            loop {
                if self.check(&Token::Star) {
                    return Err(SyntaxError::StagedFeature {
                        feature: "glob imports are invalid (SPEC 7.2) — import explicit symbols instead".into(),
                        span: self.make_span(self.peek_with_span().map(|(_, s)| s.clone()).unwrap_or(0..0)),
                    });
                }
                let local = self.expect_identifier()?;
                let exported = if self.eat(&Token::Colon) {
                    self.expect_identifier()?
                } else {
                    local.clone()
                };
                symbols.push((local, exported));
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
                alias: None,
                span: None,
            });
        }

        // Check for < without LBrace: import <name> (slash-separated path)
        if self.eat(&Token::Lt) {
            let mut name = self.expect_identifier()?;
            while self.eat(&Token::Slash) {
                let part = self.expect_identifier()?;
                name = format!("{}/{}", name, part);
            }
            self.expect(Token::Gt)?;
            self.expect(Token::Semicolon)?;
            return Ok(Import {
                kind: ImportKind::Registry(name),
                symbols: vec![],
                alias: None,
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
                alias: None,
                span: None,
            });
        }

        // Import with symbols: import sym from "module" or from <name>
        let first = self.expect_identifier()?;
        // 2026-08-09 (Phase 11, Slice 2): `import alias: <path>` — a `:` module
        // alias. Collision-resolving local TAG only (no qualified access — Briev
        // inlines imports). The path follows the `:`.
        if self.eat(&Token::Colon) {
            let kind = parse_import_path(self)?;
            self.expect(Token::Semicolon)?;
            return Ok(Import {
                kind,
                symbols: vec![],
                alias: Some(first),
                span: None,
            });
        }
        if self.eat_identifier("from") {
            let kind = parse_import_path(self)?;
            self.expect(Token::Semicolon)?;
            Ok(Import {
                kind,
                symbols: vec![(first.clone(), first)],
                alias: None,
                span: None,
            })
        } else {
            let mut symbols = vec![(first.clone(), first)];
            loop {
                if !self.eat(&Token::Comma) {
                    break;
                }
                let name = self.expect_identifier()?;
                symbols.push((name.clone(), name));
            }
            self.eat_identifier("from");
            let kind = parse_import_path(self)?;
            self.expect(Token::Semicolon)?;
            Ok(Import {
                kind,
                symbols,
                alias: None,
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
    /// Parse top-level trg binding: trg name @ instance.#port;
    /// 2026-07-15: The # prefix is required for layout port access.
    fn parse_top_level_trg(&mut self) -> Result<Trigger, SyntaxError> {
        self.pos += 1;
        let name = self.expect_identifier()?;
        self.expect(Token::At)?;
        let instance = self.parse_expression()?;
        // 2026-08-01 (Phase 4): the `.port` is removed — a trigger is the
        // whole-target form `trg name @ instance;`.
        self.expect(Token::Semicolon)?;
        Ok(Trigger {
            name,
            instance,
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
        // Parse: [pre] if present — including the [!/X] / [!/!X] two-in-one
        // invert form (one bracket that expands to both pre and post).
        let mut saw_invert = false;
        if self.check(&Token::LBracket) {
            contract_saw_bracket = true;
            if self.contract_is_invert() {
                (pre, post) = self.parse_contract_invert()?;
                saw_invert = true;
            } else {
                pre = self.parse_single_contract_condition()?;
            }
        }
        // Parse: [post] if present (skipped for the two-in-one invert form)
        if !saw_invert && self.check(&Token::LBracket) {
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
            // 2026-08-05 (Phase 3): canonical units are cyc/ns/ms/s/min
            // (SPEC §16.1); s/ns/min are contextual identifiers.
            if self.lookahead_is_duration_unit() {
                self.pos += 1;
            }
            self.expect(Token::RBracket)?;
            // 2026-08-01 (D2): optional `within N <unit>` deadline — the
            // watchdog must fire within a time/cycle budget even if the
            // liveliness condition never stops holding:
            //   ?[cond] within 10 ms     -> deadline_ns = 10 * 1e6
            //   ?[cond] within 1000 cyc  -> cycles_bound = 1000
            //   ?[cond] within 2 seconds -> deadline_ns = 2 * 1e9
            //   ?[cond] within 1 minute  -> deadline_ns = 60 * 1e9
            let (mut cycles_bound, mut deadline_ns) = (None, None);
            if self.eat(&Token::Within) {
                let bound = match self.peek() {
                    Some(Token::Integer(n)) => {
                        let n = *n;
                        self.pos += 1;
                        n as u64
                    }
                    _ => {
                        return self.error_at_current(
                            "expected a numeric bound after 'within'",
                        );
                    }
                };
                match self.peek() {
                    Some(Token::Cyc) => {
                        self.pos += 1;
                        cycles_bound = Some(bound);
                    }
                    Some(Token::Ms) => {
                        self.pos += 1;
                        deadline_ns = Some(bound.saturating_mul(1_000_000));
                    }
                    Some(Token::Identifier(unit)) => match unit.as_str() {
                        "cyc" => {
                            self.pos += 1;
                            cycles_bound = Some(bound);
                        }
                        "ns" => {
                            self.pos += 1;
                            deadline_ns = Some(bound);
                        }
                        "ms" => {
                            self.pos += 1;
                            deadline_ns = Some(bound.saturating_mul(1_000_000));
                        }
                        "s" => {
                            self.pos += 1;
                            deadline_ns = Some(bound.saturating_mul(1_000_000_000));
                        }
                        "min" => {
                            self.pos += 1;
                            deadline_ns = Some(bound.saturating_mul(60_000_000_000));
                        }
                        _ => {
                            return self.error_at_current(
                                "expected a unit (cyc, ns, ms, s, min) after the 'within' bound",
                            );
                        }
                    },
                    _ => {
                        return self.error_at_current(
                            "expected a unit (cyc, ns, ms, s, min) after the 'within' bound",
                        );
                    }
                }
            }
            // 2026-08-01 (C1): optional `-> handler(val)` on-fire callback.
            // The handler is called with the LAST COMPUTED VALUE on the fire
            // path; the parens are the call marker (`()` or a `(val)` arg-name
            // placeholder — the emitter always passes the last value).
            let on_fire = if self.eat(&Token::Arrow) {
                let handler = self.expect_identifier()?;
                // 2026-08-01 (C2): `-> handler(val)` — the parens optionally
                // name the value passed on the fire path (the last computed
                // value). `()` and `(any_identifier)` both parse; the arg
                // name is captured for the emission (None for `()`).
                let arg = if self.eat(&Token::LParen) {
                    let name = if !self.check(&Token::RParen) {
                        if self.peek_is_identifier() {
                            Some(self.expect_identifier()?)
                        } else {
                            self.parse_expression()?;
                            None
                        }
                    } else {
                        None
                    };
                    self.expect(Token::RParen)?;
                    name
                } else {
                    None
                };
                Some(WatchdogOnFire { handler, arg })
            } else {
                None
            };
            Some(WatchdogSpec {
                condition: cond,
                is_required,
                cycles_bound,
                seconds_bound: None,
                deadline_ns,
                is_proven: false,
                retries: 0,
                fallback: None,
                on_fire,
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

    /// 2026-08-01 (Phase 3): `[!/X]` / `[!/!X]` — a bracket whose content
    /// begins with `!/` (invert the contract). One bracket expands to the
    /// pre/post pair:
    ///   `[!/X]`  → pre `!X`, post `X`
    ///   `[!/!X]` → pre `X`,  post `!X`
    fn contract_is_invert(&self) -> bool {
        if !self.check(&Token::LBracket) {
            return false;
        }
        self.tokens.get(self.pos + 1).map_or(false, |(t, _)| matches!(t, Token::Not))
            && self.tokens.get(self.pos + 2).map_or(false, |(t, _)| matches!(t, Token::Slash))
    }

    fn parse_contract_invert(&mut self) -> Result<(Expr, Expr), SyntaxError> {
        self.pos += 3; // consume '[', '!', '/'
        let inner = self.parse_expression()?;
        self.expect(Token::RBracket)?;
        if let Expr::UnaryOp(UnaryOpKind::Not, y) = &inner {
            Ok(((**y).clone(), Expr::UnaryOp(UnaryOpKind::Not, y.clone())))
        } else {
            Ok((Expr::UnaryOp(UnaryOpKind::Not, Box::new(inner.clone())), inner))
        }
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
        // 2026-08-05 (Phase 6): contracts are mandatory on asm declarations;
        // parse the [pre][post] pair before the body.
        let contract = self.parse_contract()?;
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
        Ok(AsmFn { target, name, params, ret_type, contract, body, span })
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

    /// 2026-07-14: Parse: type Name : Parent { slot; slot; }
    fn parse_type_or_group(&mut self) -> Result<Box<TypeDef>, SyntaxError> {
        self.pos += 1; // consume `type` token
        // 2026-07-16: All type names are Token::Identifier after Type token removal.
        let name = self.expect_identifier()?;
        // 2026-07-20: Parse type parameters: type List<T: #String, V>
        let type_params = self.parse_type_params()?;
        // 2026-08-05 (Phase 3): dotted extension groups (`type Foo.[a,b]`) are
        // removed with the free-form dot-extension mechanism.
        self.parse_type_body(name, type_params)
    }

    /// 2026-08-05 (Phase 4): `trait Name<T> { ... }` — reusable behavioral
    /// requirements and defaults (SPEC §8.6). The body accepts logical field
    /// requirements, required/default function signatures, and op bindings.
    fn parse_trait(&mut self) -> Result<TraitDef, SyntaxError> {
        self.pos += 1; // consume 'trait'
        let name = self.expect_identifier()?;
        let type_params = self.parse_type_params()?;
        let mut functions = Vec::new();
        let mut op_bindings = Vec::new();
        let mut fields = Vec::new();
        if self.eat(&Token::LBrace) {
            while !self.check(&Token::RBrace) && !self.is_at_end() {
                if self.check(&Token::Defn) {
                    functions.push(self.parse_definition()?);
                    self.eat(&Token::Semicolon);
                } else if self.check(&Token::Op) {
                    self.parse_op_definition(&mut op_bindings, None)?;
                } else {
                    let fname = self.expect_identifier()?;
                    self.expect(Token::Colon)?;
                    let fty = self.parse_type()?;
                    self.eat(&Token::Semicolon);
                    fields.push((fname, fty));
                }
            }
            self.expect(Token::RBrace)?;
        }
        self.eat(&Token::Semicolon);
        Ok(TraitDef {
            name,
            type_params,
            functions,
            op_bindings,
            fields,
            span: None,
        })
    }

    /// 2026-08-05 (Phase 4): `impl Name<T> { ... }` — inherent behavior for a
    /// data-only declaration (SPEC §8.8).
    fn parse_impl(&mut self) -> Result<ImplDef, SyntaxError> {
        self.pos += 1; // consume 'impl'
        let target = self.expect_identifier()?;
        let type_params = self.parse_type_params()?;
        let mut functions = Vec::new();
        let mut op_bindings = Vec::new();
        if self.eat(&Token::LBrace) {
            while !self.check(&Token::RBrace) && !self.is_at_end() {
                if self.check(&Token::Defn) {
                    functions.push(self.parse_definition()?);
                    self.eat(&Token::Semicolon);
                } else if self.check(&Token::Op) {
                    self.parse_op_definition(&mut op_bindings, None)?;
                } else {
                    return self.error_at_current("expected 'defn' or 'op' in impl block");
                }
            }
            self.expect(Token::RBrace)?;
        }
        self.eat(&Token::Semicolon);
        Ok(ImplDef {
            target,
            type_params,
            functions,
            op_bindings,
            span: None,
        })
    }

    /// 2026-07-24: Parse `type Name [ : [Parent] [Protocol] ] { body }`.
    fn parse_type_body(&mut self, name: String, type_params: Vec<crate::ast::top::TypeParam>) -> Result<Box<TypeDef>, SyntaxError> {
        let mut parent: Option<Box<Expr>> = None;
        let mut protocol: Option<String> = None;
        let mut traits: Vec<String> = Vec::new();
        if self.eat(&Token::Colon) {
            // 2026-08-05 (Phase 5): comma-separated relationship list:
            //   type X: Parent, Trait1, #Proto { ... }
            // A hashword entry sets the protocol; the first non-hashword is the
            // single refinement parent; the rest are explicitly asserted traits.
            loop {
                match self.peek() {
                    Some(&Token::Identifier(ref s)) if s.starts_with('#') => {
                        let mut proto = s.clone(); self.pos += 1;
                        if self.eat(&Token::Lt) {
                            let variant = self.expect_identifier()?;
                            if !self.eat_type_close() {
                                return self.error_at_current("expected '>' in protocol variant base");
                            }
                            proto = format!("{}<{}>", proto, variant);
                        }
                        protocol = Some(proto);
                    }
                    Some(&Token::Identifier(_)) => {
                        let pname = self.expect_identifier()?;
                        // 2026-08-05 (Phase 5): an entry with generic arguments
                        // (`Comparable<Point>`) is always a trait. A bare name
                        // is the single refinement parent only if none is set;
                        // otherwise it is an explicitly asserted trait.
                        let has_args = self.check(&Token::Lt);
                        if has_args {
                            self.parse_type_params()?;
                            traits.push(pname);
                        } else if parent.is_none() {
                            parent = Some(Box::new(Expr::Identifier(pname)));
                        } else {
                            traits.push(pname);
                        }
                    }
                    _ => return self.error_at_current("expected a parent, trait, or protocol hashword after ':'"),
                }
                if !self.eat(&Token::Comma) {
                    break;
                }
            }
        }
        let mut slots = Vec::new();
        let mut metadata = std::collections::HashMap::new();
        let mut operators: Vec<OperatorDef> = Vec::new();
        let mut op_bindings: Vec<OperatorBinding> = Vec::new();
        let mut members: Vec<crate::ast::TopLevel> = Vec::new();
        if self.eat(&Token::LBrace) {
            while !self.check(&Token::RBrace) && !self.is_at_end() {
                // !> key: value; or spec PascalCase: value; — metadata assignment
                if self.check(&Token::ExclaimArrow) || self.check(&Token::Spec) {
                    self.parse_metadata_clause(&mut metadata)?;
                    continue;
                }
                let slot_name = self.expect_identifier()?;
                if slot_name == "op" {
                    self.parse_op_definition(&mut op_bindings, Some(&mut members))?;
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
            traits,
            bit_range: None,
            body: TypeDefBody {
                slots,
                metadata,
                projections: vec![],
                bindings: vec![],
                operators,
                op_bindings,
                constraints: vec![],
                members,
                span: None,
            },
            span: None,
        }))
    }

    /// 2026-07-20: Parse an op binding within a type body.
    /// Two forms:
    ///   op Add(#Int, #Int);                                     — declarative hashword dispatch
    ///   op Add(Posit32) = Posit32_add(#Lh, #Rh);                  — binding with explicit function

    /// 2026-07-26: Parse prop Name: expr;
    /// Declares a metaproperty with an implementation expression.
    /// `:` replaces the old `=` syntax.
    /// 2026-07-26: Parse op Name(Proto?): expr;
    /// Declares an operator binding. protocol_variant is optional.
    /// Optional discriminator fields: pre:"0x", suf:"f", reg:"[0-9]+"
    /// Examples:
    ///   op InsertAt: push(#Lh, #Rh);
    ///   op Add(#Int): int_add(#Lh, #Rh);
    ///   op Parse(Decimal, pre:"0x"): parse_hex(#Lh);
    ///   op Parse(Decimal, suf:"h"): to_f16(#Lh);
    fn parse_op_definition(&mut self, op_bindings: &mut Vec<OperatorBinding>, members: Option<&mut Vec<crate::ast::TopLevel>>) -> Result<(), SyntaxError> {
        let name = self.expect_identifier()?;
        // 2026-08-12 (Iterable protocol, op-as-member): `op Name(params) -> Ret
        // { body }` — the operator IS a defn-shaped member (SPEC §15.2).
        // Distinguished from the variant/binding forms by a parameter list
        // (`op At(i: Int)`) or an empty paren followed by `->`/`{`.
        if self.check(&Token::LParen) {
            let is_member_form = {
                let saved = self.pos;
                self.pos += 1; // past '('
                let member_form = match self.tokens.get(self.pos).map(|(t, _)| t) {
                    Some(Token::RParen) => {
                        let after = self.tokens.get(self.pos + 1).map(|(t, _)| t);
                        matches!(after, Some(Token::Arrow) | Some(Token::LBrace))
                    }
                    Some(Token::Identifier(_)) => {
                        // `i: Int` parameter list → op-as-member; a bare type or
                        // discriminator (`Decimal, pre:"0x"`) → legacy form.
                        matches!(self.tokens.get(self.pos + 1).map(|(t, _)| t), Some(Token::Colon))
                    }
                    _ => false,
                };
                self.pos = saved;
                member_form
            };
            if is_member_form {
                let Some(members) = members else {
                    return Err(crate::errors::SyntaxError::UnexpectedToken {
                        expected: "a `:` operator binding (operator members are only allowed in obj/type bodies)".into(),
                        found: format!("`op {}(...)`", name),
                        span: crate::errors::Span::new(0, 0, 0, 0),
                    });
                };
                let op_member = self.parse_operator_member(name)?;
                members.push(crate::ast::TopLevel::TypeDefOperator(op_member));
                return Ok(());
            }
        }
        // Optional protocol variant: (#Proto) or (ConcreteType)
        let protocol_variant = if self.eat(&Token::LParen) {
            // 2026-08-01 (B2): parse the variant as a TYPE so hashwords work
            // (`op CastFrom(#Bit) = fn`). Previously expect_identifier rejected
            // the `#` — type-level CastFrom(#Bit) overrides were unparseable.
            // Store the bare category (strip `#`) — compile.rs matches
            // protocol_variant == "#Bit"/"Bit".
            let variant = match self.parse_type()? {
                crate::ast::Type::HashWord(cat) | crate::ast::Type::HashWordVariant(cat, _) => {
                    cat.strip_prefix('#').unwrap_or(&cat).to_string()
                }
                other => format!("{}", other),
            };
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
        // Parse method call with #Lh, #Rh, #T placeholders as a raw expression
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

    /// 2026-08-12 (Iterable protocol, op-as-member): parse the defn-shaped
    /// operator member `op Name(params) -> Ret [contract] { body };`. The
    /// operator name IS the member name; the body is ordinary Briev,
    /// self-parameterized on the enclosing obj/type's slots like a defn
    /// member. The compiler resolves operators by this member (SPEC §15.2).
    fn parse_operator_member(&mut self, name: String) -> Result<Definition, SyntaxError> {
        let parameters = if self.eat(&Token::LParen) {
            let p = self.parse_parameter_list()?;
            self.expect(Token::RParen)?;
            p
        } else {
            Vec::new()
        };
        let (output_type, contract) = self.parse_output_and_contract()?;
        let body = if self.check(&Token::LBrace) {
            self.parse_block()?
        } else {
            Vec::new()
        };
        let derivation = self.parse_derivation_block()?;
        let metadata = self.parse_body_metadata()?;
        self.eat(&Token::Semicolon);
        Ok(Definition {
            name,
            type_params: Vec::new(),
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
        // 2026-08-01 (B2): `op CastFrom(#Bit) = fn` uses `=` (like the proto
        // CastFrom form); other discriminated ops use `:`. Accept either.
        if !self.eat(&Token::Eq) {
            self.expect(Token::Colon)?;
        }
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

    /// 2026-07-26: Parse a hash marker (#Lh, #Rh, #T) or an identifier.
    fn parse_hash_marker(&mut self) -> Result<Expr, SyntaxError> {
        match self.peek() {
            Some(Token::HashL) => { self.pos += 1; Ok(Expr::Identifier("#Lh".to_string())) }
            Some(Token::HashR) => { self.pos += 1; Ok(Expr::Identifier("#Rh".to_string())) }
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

    /// 2026-08-13 (layout-keywords plan): parse one metadata clause in a type/
    /// obj/struct body. Two spellings:
    ///   `!> <lowercase_key>: <value>;`       — annotation form (legacy)
    ///   `spec <PascalCase>: <value>;`         — declared-layout form (modern)
    /// Both write the SAME lowercase metadata keys, so consumers have a single
    /// read path. `!>` keeps the ctd/alu special cases (the `<...>` layout DSL
    /// was removed 2026-08-13 — see the layout-keywords plan); `spec` has the
    /// five physical-layout keys (Alignment/Bits/MaxBits/Bytes/Endian).
    /// Invoked with the cursor AT the `!>` or `spec` token.
    fn parse_metadata_clause(
        &mut self,
        metadata: &mut std::collections::HashMap<String, PropertyValue>,
    ) -> Result<(), SyntaxError> {
        let is_spec = self.check(&Token::Spec);
        self.advance();
        let key = self.expect_identifier()?;
        self.expect(Token::Colon)?;
        if is_spec {
            return self.parse_spec_value(&key, metadata);
        }
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
            _ => {
                let pv = self.parse_metadata_value_standalone()?;
                self.eat(&Token::Semicolon);
                metadata.insert(key, pv);
            }
        }
        Ok(())
    }

    /// 2026-08-13 (layout-keywords plan): parse the value of a `spec` clause.
    /// Key is the PascalCase spelling already lexed. The five physical-layout
    /// keys are recognized; anything else is an error per SPEC §2.1 (no silent
    /// unknown-spec acceptance).
    fn parse_spec_value(
        &mut self,
        name: &str,
        metadata: &mut std::collections::HashMap<String, PropertyValue>,
    ) -> Result<(), SyntaxError> {
        let key = match spec_name_to_key(name) {
            Some(k) => k,
            None => {
                let msg = format!(
                    "unknown spec '{}' — known specs: Alignment, Bits, MaxBits, Bytes, Endian",
                    name
                );
                return self.error_at_current(&msg);
            }
        };
        match key {
            "endian" => {
                let id = self.expect_identifier()?;
                if !matches!(id.as_str(), "Big" | "Little" | "Target") {
                    let msg = format!(
                        "invalid spec Endian value '{}' — expected Big, Little, or Target",
                        id
                    );
                    return self.error_at_current(&msg);
                }
                metadata.insert(key.into(), PropertyValue::Identifier(id));
            }
            _ => {
                let n = self.expect_integer()?;
                if n < 0 {
                    let msg = format!("spec {} must be a non-negative integer, got {}", name, n);
                    return self.error_at_current(&msg);
                }
                metadata.insert(key.into(), PropertyValue::Int(n));
            }
        }
        self.eat(&Token::Semicolon);
        Ok(())
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
        if self.eat(&Token::LBrace) {
            while !self.check(&Token::RBrace) && !self.is_at_end() {
                // !> key: value; or spec PascalCase: value; — metadata.
                if self.check(&Token::ExclaimArrow) || self.check(&Token::Spec) {
                    self.parse_metadata_clause(&mut metadata)?;
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
                    self.parse_op_definition(&mut op_bindings, Some(&mut members))?;
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
            traits: vec![],
            bit_range: None, span: None,
            body: TypeDefBody {
                slots, metadata, projections: vec![], bindings: vec![], operators, op_bindings, constraints: vec![], members, span: None,
            },
        }))
    }

    /// struct Name { field: Type; } — static fixed-layout struct.
    /// Pure data, C-compatible, no methods, no contracts.
    /// 2026-07-24: Fields are space-separated, semicolon-terminated.
    fn parse_struct_def(&mut self) -> Result<StructDef, SyntaxError> {
        // 2026-08-05 (Phase 4): `seq struct` preserves field order/containment.
        // 2026-08-13 (layout-keywords plan): `pack struct` (bit-contiguous) —
        // `pack`/`seq` are order-independent (`pack seq struct`, `seq pack
        // struct`), so consume both flags before `struct` in a loop.
        let mut seq = false;
        let mut pack = false;
        loop {
            match self.tokens.get(self.pos).map(|(t, _)| t) {
                Some(Token::Seq) => { seq = true; self.pos += 1; }
                Some(Token::Pack) => { pack = true; self.pos += 1; }
                _ => break,
            }
        }
        self.expect(Token::Struct)?;
        let name = self.expect_identifier()?;
        // 2026-07-31: Generic struct: struct ListBuffer<T> { ... }.
        let type_params = self.parse_type_params()?;
        let mut fields = Vec::new();
        let mut annotations: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
        // 2026-08-13 (layout-keywords plan): structs gain physical-layout
        // metadata (`spec Bits`, `spec Align`, `spec Bytes`, `spec MaxBits`,
        // `spec Endian`) consumed at StaticStruct registration (llvm/mod.rs).
        let mut metadata: std::collections::HashMap<String, PropertyValue> = std::collections::HashMap::new();
        if self.eat(&Token::LBrace) {
            while !self.check(&Token::RBrace) && !self.is_at_end() {
                // spec PascalCase: value; — declared physical layout.
                if self.check(&Token::Spec) {
                    self.parse_metadata_clause(&mut metadata)?;
                    continue;
                }
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
        if !annotations.is_empty() {
            metadata.insert("annotations".to_string(), crate::ast::PropertyValue::String(format!("{:?}", annotations)));
        }
        // 2026-08-13 (layout-keywords plan): `pack struct` fields must be
        // bit-sliceable — no array fields (bit-contiguous arrays are a
        // contradiction) and no field wider than the 64-bit slice machinery
        // (SPEC §2.5). Other scalar types resolve through the universe at
        // registration; whole-byte widths stay on the aligned native path.
        if pack {
            for (fname, fty) in &fields {
                match fty {
                    Type::Vector(_, _) => {
                        return Err(SyntaxError::InvalidStatement {
                            reason: format!(
                                "packed field '{}': a bit-contiguous struct cannot hold an array field; use an indirection or a sequence (seq)",
                                fname
                            ),
                            span: self
                                .peek_with_span()
                                .map(|(_, s)| self.make_span(s.clone()))
                                .unwrap_or(crate::errors::Span::dummy()),
                        });
                    }
                    Type::Bits(n) if *n > 64 => {
                        return Err(SyntaxError::InvalidStatement {
                            reason: format!(
                                "packed field '{}': width {} exceeds the 64-bit packed slice limit; split it into whole-byte `Bits<64>` fields and combine",
                                fname, n
                            ),
                            span: self
                                .peek_with_span()
                                .map(|(_, s)| self.make_span(s.clone()))
                                .unwrap_or(crate::errors::Span::dummy()),
                        });
                    }
                    // 2026-08-13: `Bits<N>` parses as Applied("Bits", [Number(N)])
                    // (the exact-width alias) — enforce the same slice limit.
                    Type::Applied(name, args) if name == "Bits" => {
                        if let Some(crate::ast::Type::Number(n)) = args.first() {
                            if *n > 64 {
                                return Err(SyntaxError::InvalidStatement {
                                    reason: format!(
                                        "packed field '{}': width {} exceeds the 64-bit packed slice limit; split it into whole-byte `Bits<64>` fields and combine",
                                        fname, n
                                    ),
                                    span: self
                                        .peek_with_span()
                                        .map(|(_, s)| self.make_span(s.clone()))
                                        .unwrap_or(crate::errors::Span::dummy()),
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(StructDef {
            name, type_params, fields,
            metadata,
            span: None,
            seq,
            pack,
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
            traits: vec![],
            bit_range: None, span: None,
            body: TypeDefBody {
                slots, metadata: std::collections::HashMap::new(),
                projections: vec![], bindings: vec![], operators: vec![], op_bindings: vec![], constraints: vec![], members: vec![], span: None,
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
                    // Check for binding: = fn_name(#Lh)
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
                    // Optional binding: = fn(#Lh, #Rh)
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

/// 2026-08-13 (layout-keywords plan): PascalCase spec key → canonical
/// lowercase metadata key. The reverse map (formatter) lives in canonical.rs —
/// keep the two tables in sync.
fn spec_name_to_key(name: &str) -> Option<&'static str> {
    match name {
        "Alignment" => Some("alignment"),
        "Bits" => Some("bits"),
        "MaxBits" => Some("maxbits"),
        "Bytes" => Some("bytes"),
        "Endian" => Some("endian"),
        _ => None,
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
    fn test_parse_dotted_type_extension_rejected() {
        // 2026-08-05 (Phase 3): free-form dotted type extensions (`String.c`)
        // are removed; the keyword type parses and the trailing `.` is left
        // unconsumed (a full declaration therefore fails to parse).
        let ty = parse_type("String.c").unwrap();
        assert_eq!(ty, crate::ast::Type::string());
    }

    #[test]
    fn test_parse_dotted_type_no_extension() {
        // "String" should still parse as Type::string()
        let ty = parse_type("String").unwrap();
        assert_eq!(ty, crate::ast::Type::string());
    }

    #[test]
    fn test_parse_dotted_type_double_extension_rejected() {
        // "Int.c.sso" — the `Int` parses and the dotted suffix is not consumed.
        let ty = parse_type("Int.c.sso").unwrap();
        assert_eq!(ty, crate::ast::Type::int());
    }

    // ── Layout-keywords (Phase 1): spec clause parsing ────────────────

    fn parse_top(src: &str) -> Result<crate::ast::TopLevel, String> {
        let tokens = tokenize(src).map_err(|e| format!("lex: {e}"))?;
        let mut p = Parser::new(tokens, src);
        p.parse_program()
            .map(|mut v| v.remove(0))
            .map_err(|e| format!("parse: {e}"))
    }

    #[test]
    fn test_spec_in_type_body() {
        // 2026-08-13 (layout-keywords plan): `spec Bits: 4` maps to the
        // lowercase metadata key `bits` (same read path as `!> bits`).
        let tl = parse_top("type W4: #Int { spec Bits: 4; };").unwrap();
        let crate::ast::TopLevel::TypeDef(td) = tl else { panic!("expected TypeDef") };
        match td.body.metadata.get("bits") {
            Some(crate::ast::PropertyValue::Int(4)) => {}
            other => panic!("expected bits=4, got {:?}", other),
        }
    }

    #[test]
    fn test_spec_all_layout_keys_in_type_body() {
        let tl = parse_top(
            "type Frame: #Bit {\n  \
             spec Alignment: 2;\n  spec Bits: 12;\n  spec MaxBits: 16;\n  \
             spec Bytes: 4;\n  spec Endian: Big;\n};",
        )
        .unwrap();
        let crate::ast::TopLevel::TypeDef(td) = tl else { panic!("expected TypeDef") };
        assert_eq!(td.body.metadata["alignment"], crate::ast::PropertyValue::Int(2));
        assert_eq!(td.body.metadata["bits"], crate::ast::PropertyValue::Int(12));
        assert_eq!(td.body.metadata["maxbits"], crate::ast::PropertyValue::Int(16));
        assert_eq!(td.body.metadata["bytes"], crate::ast::PropertyValue::Int(4));
        assert_eq!(td.body.metadata["endian"], crate::ast::PropertyValue::Identifier("Big".into()));
    }

    #[test]
    fn test_spec_in_struct_body() {
        let tl = parse_top("struct Header { spec Bytes: 2; tag: Int; };").unwrap();
        let crate::ast::TopLevel::StaticStruct(s) = tl else { panic!("expected StaticStruct") };
        assert_eq!(s.metadata["bytes"], crate::ast::PropertyValue::Int(2));
        assert_eq!(s.fields.len(), 1);
    }

    #[test]
    fn test_pack_struct_flag() {
        let tl = parse_top("pack struct Eth { dst: Bits<48>; src: Bits<48>; etype: Bits<16>; };").unwrap();
        let crate::ast::TopLevel::StaticStruct(s) = tl else { panic!("expected StaticStruct") };
        assert!(s.pack, "pack flag must be set");
        assert!(!s.seq, "pack alone must not set seq");
        assert_eq!(s.fields.len(), 3);
    }

    #[test]
    fn test_pack_seq_and_seq_pack_order_independent() {
        // `pack`/`seq` are order-independent prefix flags (plan Phase 2).
        let tl1 = parse_top("pack seq struct P { a: Bits<4>; };").unwrap();
        let tl2 = parse_top("seq pack struct P { a: Bits<4>; };").unwrap();
        let crate::ast::TopLevel::StaticStruct(s1) = tl1 else { panic!("expected StaticStruct") };
        let crate::ast::TopLevel::StaticStruct(s2) = tl2 else { panic!("expected StaticStruct") };
        assert!(s1.pack && s1.seq, "pack seq struct sets both flags");
        assert!(s2.pack && s2.seq, "seq pack struct sets both flags");
    }

    #[test]
    fn test_pack_rejects_vector_field() {
        let err = parse_top("pack struct V { data: Int[4]; };").unwrap_err();
        assert!(
            err.contains("array") || err.contains("bit-contiguous"),
            "packed array field must be rejected with a layout reason; got: {err}"
        );
    }

    #[test]
    fn test_pack_rejects_overwide_field() {
        let err = parse_top("pack struct W { wide: Bits<128>; };").unwrap_err();
        assert!(
            err.contains("64-bit"),
            "packed field wider than 64 bits must be rejected; got: {err}"
        );
    }

    #[test]
    fn test_trap_statement_parses() {
        // 2026-08-13 (layout-keywords plan Phase 4): `trap;` — hardware abort.
        let src = "defn f(x: Int) -> Int {\n  if x > 0 {\n    trap;\n  } else {\n    term 0;\n  };\n  term 1;\n};\n";
        let tokens = tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        let items = p.parse_program().unwrap();
        let crate::ast::TopLevel::Definition(def) = &items[0] else { panic!("expected defn") };
        let has_trap = def.body.iter().any(|s| match s {
            crate::ast::Statement::If(_, then, _) => {
                then.iter().any(|t| matches!(t, crate::ast::Statement::Trap))
            }
            _ => false,
        });
        assert!(has_trap, "trap; in an if-body must parse to Statement::Trap");
    }

    #[test]
    fn test_trap_in_guard_body_parses() {
        let src = "let g: Int = 0;\nnode n [g < 3][g == 3] {\n  when g == 1 { trap; };\n  g = g + 1;\n  term;\n};\n";
        let tokens = tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        assert!(p.parse_program().is_ok(), "trap; in a when-body must parse");
    }

    #[test]
    fn test_spec_in_obj_body() {
        let tl = parse_top("obj Widget { spec Bits: 8; x: Int; };").unwrap();
        let crate::ast::TopLevel::TypeDef(td) = tl else { panic!("expected TypeDef (obj)") };
        assert_eq!(td.body.metadata["bits"], crate::ast::PropertyValue::Int(8));
    }

    #[test]
    fn test_spec_unknown_name_rejected() {
        // 2026-08-13: unknown spec names are hard errors — never silent.
        let err = parse_top("type W: #Int { spec Flurb: 3; };").unwrap_err();
        assert!(err.contains("unknown spec"), "got: {err}");
    }

    #[test]
    fn test_spec_endian_invalid_value_rejected() {
        let err = parse_top("type W: #Int { spec Endian: Sideways; };").unwrap_err();
        assert!(err.contains("invalid spec Endian value"), "got: {err}");
    }

    #[test]
    fn test_spec_non_integer_width_rejected() {
        let err = parse_top("type W: #Int { spec Bits: many; };").unwrap_err();
        assert!(err.contains("expected integer") || err.contains("integer"), "got: {err}");
    }

    #[test]
    fn test_spec_does_not_break_exclaim_arrow() {
        // `!>` still parses alongside `spec` in the same body.
        let tl = parse_top("type W: #Int { !> ctd: Add; spec Bits: 8; };").unwrap();
        let crate::ast::TopLevel::TypeDef(td) = tl else { panic!("expected TypeDef") };
        assert_eq!(td.body.metadata["ctd"], crate::ast::PropertyValue::Identifier("Add".into()));
        assert_eq!(td.body.metadata["bits"], crate::ast::PropertyValue::Int(8));
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

    // ── 2026-08-09 (Phase 12, SPEC §19.1/19.4/19.7) ──────────────────

    #[test]
    fn test_frgn_colon_binds_external_symbol() {
        // SPEC §19.1: the declaration name is the LOCAL Briev name; `:` binds a
        // different external (link) symbol. `as` is not an alias operator.
        let fb = parse_frgn(
            r#"frgn local_add(a: Int, b: Int) -> Int: external_add from #System;"#,
        )
        .unwrap();
        assert_eq!(fb.foreign_name, "external_add", "the `:` symbol is the link name");
        assert_eq!(fb.briev_name.as_deref(), Some("local_add"), "the declaration name is the local Briev name");
        assert_eq!(fb.effective_briev_name(), "local_add");
    }

    #[test]
    fn test_frgn_variadic_named_param() {
        // SPEC §19.4: `variadic args: ForeignArgs` — an explicit final named
        // variadic parameter; `...` stays reserved for slicing.
        let fb = parse_frgn(
            r#"frgn log(format: String, variadic args: ForeignArgs) -> Void from #System;"#,
        )
        .unwrap();
        assert!(fb.is_variadic, "the `variadic` marker must be recorded");
        assert_eq!(fb.inputs.len(), 2);
        assert_eq!(fb.inputs[1].0, "args");
    }

    #[test]
    fn test_frgn_mmio_address_form_rejected() {
        // SPEC §19.7: `frgn name @ address` is invalid — MMIO uses configured
        // ports or explicit intrinsics. The `@` is rejected regardless of where
        // it appears (before params via the dedicated check, or after `from`
        // via the trailing-token expectation).
        assert!(parse_frgn(r#"frgn reg(a: Int) -> Int from "c" @ 0x40000000;"#).is_err());
        assert!(parse_frgn(r#"frgn reg @ 0x40000000;"#).is_err());
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

    /// 2026-08-12 (Iterable protocol, op-as-member): parse an obj and return
    /// its operator members (`TopLevel::TypeDefOperator`).
    fn parse_op_members(src: &str) -> Vec<crate::ast::Definition> {
        let tokens = crate::lexer::tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        match p.parse_top_level() {
            Ok(crate::ast::TopLevel::TypeDef(td)) => td
                .body
                .members
                .into_iter()
                .filter_map(|m| match m {
                    crate::ast::TopLevel::TypeDefOperator(d) => Some(d),
                    _ => None,
                })
                .collect(),
            other => panic!("expected TypeDef, got {:?}", std::mem::discriminant(&other)),
        }
    }

    #[test]
    fn test_op_as_member_parses() {
        let ops = parse_op_members(
            "obj Counter { count: Int; op Count() -> Int { term count; }; };",
        );
        assert_eq!(ops.len(), 1, "one operator member");
        assert_eq!(ops[0].name, "Count");
        assert_eq!(ops[0].parameters.len(), 0);
        assert!(ops[0].output_type.is_some(), "returns Int");
        assert_eq!(ops[0].body.len(), 1, "body has `term count`");
    }

    #[test]
    fn test_op_as_member_with_params_and_legacy_binding() {
        let tokens = crate::lexer::tokenize(
            "obj T { data: Int[4]; op At(i: Int) -> Int { term data[i]; }; \
             op InsertAt: push(#Lh, #Rh); };",
        )
        .unwrap();
        let mut p = Parser::new(tokens, "test");
        let td = match p.parse_top_level() {
            Ok(crate::ast::TopLevel::TypeDef(td)) => td,
            other => panic!("expected TypeDef, got {:?}", std::mem::discriminant(&other)),
        };
        assert_eq!(td.body.members.len(), 1, "one op-as-member");
        assert_eq!(td.body.op_bindings.len(), 1, "one legacy binding");
        let crate::ast::TopLevel::TypeDefOperator(op) = &td.body.members[0] else {
            panic!("expected TypeDefOperator");
        };
        assert_eq!(op.name, "At");
        assert_eq!(op.parameters, vec![("i".to_string(), crate::ast::Type::int())]);
        assert_eq!(td.body.op_bindings[0].name, "InsertAt");
    }

    #[test]
    fn test_op_declarative_hashword() {
        let ops = parse_op_from_type_def("type T { op Add: int_add(#Lh, #Rh); };");
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].name, "Add");
        assert!(ops[0].protocol_variant.is_none());
    }

    #[test]
    fn test_op_declarative_protocol_variant() {
        // 2026-08-01 (B2): the variant parses as a TYPE, so the stored value
        // is the BARE category ("Int") — hashwords (`op Add(#Int)`) and
        // CastFrom(#Bit) overrides both go through parse_type now.
        let ops = parse_op_from_type_def("type T { op Add(#Int): int_add(#Lh, #Rh); };");
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].name, "Add");
        assert_eq!(ops[0].protocol_variant.as_deref(), Some("Int"));
    }

    #[test]
    fn test_op_binding_with_markers() {
        let ops = parse_op_from_type_def(
            "type T { op InsertAt: push(#Lh, #Rh); };"
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
            "proto ASCII: #String { CastTo(#String<UTF8>); op Add(#String<UTF8>) = add_UTF8_to_ASCII(#Lh, #Rh); };"
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
            "render Foo { <div>Hello</div> };");
        assert_eq!(rb.struct_name, "Foo");
        assert!(rb.view_html.contains("<div>Hello</div>"),
            "HTML content should be preserved: got '{}'", rb.view_html);
    }

    #[test]
    fn test_parse_render_accepts_any_declaration_kind() {
        // 2026-08-05 (Phase 3): `render Name` resolves the kind; struct/obj
        // keywords are gone from the attachment form.
        let rb = parse_render_block(
            "render Bar { <span b-text=\"x\">0</span> };");
        assert_eq!(rb.struct_name, "Bar");
        assert!(rb.view_html.contains("b-text"),
            "HTML should include b-* attribute: got '{}'", rb.view_html);
    }

    #[test]
    fn test_parse_render_struct_with_style_attr() {
        let rb = parse_render_block(
            "render Styled { <div class=\"box\" style=\"color: red;\">Content</div> };");
        assert_eq!(rb.struct_name, "Styled");
        assert!(rb.view_html.contains("class=\"box\""),
            "HTML should preserve attributes: got '{}'", rb.view_html);
    }

    #[test]
    fn test_parse_render_struct_nested_tags() {
        let rb = parse_render_block(
            "render Nest { <ul><li b-each:item=\"list\" b-text=\"item\"></li></ul> };");
        assert_eq!(rb.struct_name, "Nest");
        assert!(rb.view_html.contains("b-each:item"),
            "HTML should preserve b-each: got '{}'", rb.view_html);
    }

    #[test]
    fn test_parse_render_rejects_legacy_kind_keyword() {
        // 2026-08-05 (Phase 3): `render struct` is no longer accepted.
        let src = "render struct foo { <div></div> };";
        let tokens = crate::lexer::tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        let result = p.parse_top_level();
        assert!(result.is_err(), "'render struct' should be rejected");
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
    fn test_tagged_quoted_prefix_rejected() {
        // 2026-08-05 (Phase 3): adjacent prefix-discriminator literals
        // (`sql"SELECT"`) are removed. `sql"..."` parses as the identifier
        // `sql`; the adjacent string is NOT consumed as a tagged literal.
        let src = r#"sql"SELECT * FROM users""#;
        let tokens = tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        let expr = p.parse_expression().unwrap();
        assert_eq!(expr, crate::ast::Expr::Identifier("sql".to_string()));
        assert!(
            matches!(p.peek(), Some(&crate::lexer::Token::String(_))),
            "the adjacent string must remain unconsumed"
        );
    }

    #[test]
    fn test_identifier_then_string_not_consumed() {
        // A string after an identifier is a separate expression — never a
        // prefix-discriminator literal.
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
            "type T { op Parse(Decimal, pre:\"0x\"): parse_hex(#Lh); };"
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
            "type T { op Parse(Decimal, suf:\"km\"): parse_km(#Lh); };"
        );
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].name, "Parse");
        assert_eq!(ops[0].suf.as_deref(), Some("km"));
    }

    #[test]
    fn test_op_parse_with_regex_discriminator() {
        let ops = parse_op_from_type_def(
            "type T { op Parse(Decimal, reg:\"[0-9]+\"): parse_num(#Lh); };"
        );
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].reg.as_deref(), Some("[0-9]+"));
    }

    #[test]
    fn test_op_parse_multiple_discriminators() {
        let ops = parse_op_from_type_def(
            "type T { op Parse(Decimal, pre:\"0x\", suf:\"h\"): parse_hex(#Lh); };"
        );
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].pre.as_deref(), Some("0x"));
        assert_eq!(ops[0].suf.as_deref(), Some("h"));
    }

    #[test]
    fn test_op_parse_quoted_form() {
        let ops = parse_op_from_type_def(
            "type T { op Parse(Quoted): parse_string(#Lh); };"
        );
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].protocol_variant.as_deref(), Some("Quoted"));
    }

    #[test]
    fn test_op_parse_bare_form() {
        let ops = parse_op_from_type_def(
            "type T { op Parse(Bare): parse_bool(#Lh); };"
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
    fn test_watchdog_on_fire_parses() {
        // 2026-08-01 (C1): `-> handler(val)` on-fire callback.
        let t = parse_txn(
            "txn f() [true][done] ?[x < 5] -> report(val) { term; };",
        )
        .unwrap();
        let w = t.contract.watchdog.expect("watchdog must parse");
        let on = w.on_fire.expect("on_fire must parse");
        assert_eq!(on.handler, "report");
        assert_eq!(on.arg.as_deref(), Some("val"));
    }

    #[test]
    fn test_watchdog_on_fire_empty_parens() {
        let t = parse_txn(
            "txn f() [true][done] ?[x < 5] -> report() { term; };",
        )
        .unwrap();
        let w = t.contract.watchdog.expect("watchdog must parse");
        let on = w.on_fire.expect("on_fire must parse");
        assert_eq!(on.handler, "report");
        assert_eq!(on.arg, None);
    }

    #[test]
    fn test_contract_without_watchdog() {
        let t = parse_txn("txn f() [true][done] { term; };").unwrap();
        assert!(t.contract.watchdog.is_none());
    }

    #[test]
    fn test_watchdog_within_ms_deadline_ns() {
        // 2026-08-01 (D2): `?[cond] within 10 ms` → deadline_ns = 10 * 1e6.
        let t = parse_txn(
            "txn f() [true][done] ?[x < 5] within 10 ms { term; };",
        )
        .unwrap();
        let w = t.contract.watchdog.expect("watchdog must parse");
        assert_eq!(w.deadline_ns, Some(10_000_000));
        assert!(w.cycles_bound.is_none());
    }

    #[test]
    fn test_watchdog_within_cyc_cycles_bound() {
        // 2026-08-01 (D2): `?[cond] within 1000 cyc` → cycles_bound = 1000.
        let t = parse_txn(
            "txn f() [true][done] ?[x < 5] within 1000 cyc { term; };",
        )
        .unwrap();
        let w = t.contract.watchdog.expect("watchdog must parse");
        assert_eq!(w.cycles_bound, Some(1000));
        assert!(w.deadline_ns.is_none());
    }

    #[test]
    fn test_watchdog_within_then_handler() {
        // `within` comes before `-> handler`.
        let t = parse_txn(
            "txn f() [true][done] ?[x < 5] within 10 ms -> report(val) { term; };",
        )
        .unwrap();
        let w = t.contract.watchdog.expect("watchdog must parse");
        assert_eq!(w.deadline_ns, Some(10_000_000));
        assert_eq!(w.on_fire.as_ref().map(|f| f.handler.as_str()), Some("report"));
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

    #[test]
    fn test_sync_group_node_parses() {
        // 2026-08-01 (Phase 3c): `sync<group> node name [pre][post] { body }`
        // parses to a TopLevel::SyncGroup wrapping the reactive transaction
        // with the group domains — the concurrency-gate classification.
        let src = "sync<counters> node inc_a [a < 100][a == 100] { a = a + 1; term; };";
        let tokens = tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        let item = p.parse_top_level().unwrap();
        match item {
            crate::ast::TopLevel::SyncGroup { domains, item: inner } => {
                assert_eq!(domains, vec!["counters".to_string()]);
                if let crate::ast::TopLevel::Transaction(t) = inner.as_ref() {
                    assert_eq!(t.name, "inc_a");
                    assert!(t.is_reactive);
                } else {
                    panic!("SyncGroup must wrap a Transaction");
                }
            }
            _ => panic!("expected SyncGroup, got {item:?}"),
        }
    }

    #[test]
    fn test_async_node_prefix_preserves_async() {
        // 2026-08-01 (Phase 3c): `async node name` must set is_async (the
        // prefix form was silently dropping the flag).
        let src = "async node inc_a [a < 100][a == 100] { a = a + 1; term; };";
        let tokens = tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        let item = p.parse_top_level().unwrap();
        if let crate::ast::TopLevel::Transaction(t) = item {
            assert!(t.is_async, "async node prefix must set is_async");
        } else {
            panic!("expected Transaction, got {item:?}");
        }
    }

    #[test]
    fn test_seq_node_records_seq_modifier() {
        // 2026-08-01 (Phase E): `seq node name` records the "seq" modifier.
        let src = "seq node work [true][done] { done = true; term; };";
        let tokens = tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        let item = p.parse_top_level().unwrap();
        if let crate::ast::TopLevel::Transaction(t) = item {
            assert!(t.modifiers.iter().any(|m| m.name == "seq"), "seq modifier must be recorded");
        } else {
            panic!("expected Transaction, got {item:?}");
        }
    }

    #[test]
    fn test_accel_node_records_accel_modifier() {
        // 2026-08-06 (accel plan): `accel node name` records the "accel"
        // modifier on the transaction.
        let src = "accel node force [i < nb][true] { term; };";
        let tokens = tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        let item = p.parse_top_level().unwrap();
        if let crate::ast::TopLevel::Transaction(t) = item {
            assert!(t.modifiers.iter().any(|m| m.name == "accel"), "accel modifier must be recorded");
        } else {
            panic!("expected Transaction, got {item:?}");
        }
    }

    #[test]
    fn test_accel_txn_records_accel_modifier() {
        // 2026-08-06 (accel plan): `accel txn name` records the "accel"
        // modifier on the transaction.
        let src = "accel txn kernel [i < N][true] { term; };";
        let tokens = tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        let item = p.parse_top_level().unwrap();
        if let crate::ast::TopLevel::Transaction(t) = item {
            assert!(t.modifiers.iter().any(|m| m.name == "accel"), "accel modifier must be recorded");
        } else {
            panic!("expected Transaction, got {item:?}");
        }
    }

    #[test]
    fn test_accel_requires_node_or_txn() {
        // 2026-08-06 (accel plan): `accel` must precede a node or txn.
        let src = "accel defn f(x: Int) -> Int { term x; };";
        let tokens = tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        let err = p.parse_top_level().unwrap_err();
        assert!(err.to_string().contains("'node' or 'txn'"),
            "expected helpful diagnostic, got: {err}");
    }

    #[test]
    fn test_top_level_module_metadata_parses() {
        // 2026-08-06 (accel plan): top-level `!> key: value;` becomes
        // TopLevel::ModuleMetadata (SPEC §8.9).
        let src = "!> accel: try_all;";
        let tokens = tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        let item = p.parse_top_level().unwrap();
        match item {
            crate::ast::TopLevel::ModuleMetadata(map) => {
                assert_eq!(map.len(), 1, "expected one metadata key");
                assert!(matches!(map.get("accel"),
                    Some(crate::ast::PropertyValue::Identifier(s)) if s == "try_all"));
            }
            other => panic!("expected ModuleMetadata, got {other:?}"),
        }
    }

    #[test]
    fn test_top_level_module_metadata_merges_last_wins() {
        // 2026-08-06 (accel plan): consecutive top-level `!>` lines merge
        // into one ModuleMetadata node; last binding wins per key.
        let src = "!> accel: try_all;\n!> accel: force;\n!> target: spirv;";
        let tokens = tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        let item = p.parse_top_level().unwrap();
        match item {
            crate::ast::TopLevel::ModuleMetadata(map) => {
                assert_eq!(map.len(), 2, "expected two merged keys, got {map:?}");
                assert!(matches!(map.get("accel"),
                    Some(crate::ast::PropertyValue::Identifier(s)) if s == "force"),
                    "last binding must win, got {map:?}");
                assert!(matches!(map.get("target"),
                    Some(crate::ast::PropertyValue::Identifier(s)) if s == "spirv"));
            }
            other => panic!("expected ModuleMetadata, got {other:?}"),
        }
    }

    #[test]
    fn test_module_metadata_then_node_parses_both() {
        // 2026-08-06 (accel plan): module metadata and following declarations
        // parse as separate top-level items.
        let src = "!> accel: try_all;\nnode work [true][done] { done = true; term; };";
        let tokens = tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        let items = p.parse_program().unwrap();
        assert_eq!(items.len(), 2, "expected ModuleMetadata + node, got {items:?}");
        assert!(matches!(items[0], crate::ast::TopLevel::ModuleMetadata(_)));
        assert!(matches!(items[1], crate::ast::TopLevel::Transaction(_)));
    }

    #[test]
    fn test_module_metadata_value_grammar() {
        // 2026-08-06 (accel plan): module metadata accepts the full value
        // grammar (identifier/int/bool/string/list).
        let src = "!> a: 7;\n!> b: true;\n!> c: \"hi\";\n!> d: [1, two];";
        let tokens = tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        let item = p.parse_top_level().unwrap();
        match item {
            crate::ast::TopLevel::ModuleMetadata(map) => {
                assert!(matches!(map.get("a"), Some(crate::ast::PropertyValue::Int(7))));
                assert!(matches!(map.get("b"), Some(crate::ast::PropertyValue::Bool(true))));
                assert!(matches!(map.get("c"), Some(crate::ast::PropertyValue::String(s)) if s == "hi"));
                assert!(matches!(map.get("d"), Some(crate::ast::PropertyValue::List(_))));
            }
            other => panic!("expected ModuleMetadata, got {other:?}"),
        }
    }

    #[test]
    fn test_vol_let_records_vol_modifier() {
        // 2026-08-01 (Phase E): `vol let x` records the "vol" modifier.
        let src = "node work [true][done] { vol let x: Int = 1; done = true; term; };";
        let tokens = tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        let item = p.parse_top_level().unwrap();
        if let crate::ast::TopLevel::Transaction(t) = item {
            assert!(t.body.iter().any(|s| matches!(s,
                crate::ast::Statement::Let { modifiers, .. } if modifiers.iter().any(|m| m.name == "vol")
            )), "vol modifier must be recorded on the let");
        } else {
            panic!("expected Transaction, got {item:?}");
        }
    }

    #[test]
    fn test_out_defn_records_out_modifier() {
        // 2026-08-04 (out-observability plan): `out defn` records "out".
        let src = "out defn log(msg: String) -> Int { term 1; };";
        let tokens = tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        let item = p.parse_top_level().unwrap();
        if let crate::ast::TopLevel::Definition(d) = item {
            assert!(d.modifiers.iter().any(|m| m.name == "out"), "out modifier must be recorded");
        } else {
            panic!("expected Definition, got {item:?}");
        }
    }

    #[test]
    fn test_out_node_records_out_modifier() {
        // 2026-08-04: `out node` records "out" on the Transaction.
        let src = "out node work [true][done] { done = true; term; };";
        let tokens = tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        let item = p.parse_top_level().unwrap();
        if let crate::ast::TopLevel::Transaction(t) = item {
            assert!(t.modifiers.iter().any(|m| m.name == "out"), "out modifier must be recorded");
        } else {
            panic!("expected Transaction, got {item:?}");
        }
    }

    #[test]
    fn test_out_txn_records_out_modifier() {
        // 2026-08-04: `out txn` records "out" on the Transaction.
        let src = "out txn step(x: Int) -> Int [true][term >= 0] { term x; };";
        let tokens = tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        let item = p.parse_top_level().unwrap();
        if let crate::ast::TopLevel::Transaction(t) = item {
            assert!(t.modifiers.iter().any(|m| m.name == "out"), "out modifier must be recorded");
        } else {
            panic!("expected Transaction, got {item:?}");
        }
    }

    #[test]
    fn test_out_let_records_out_modifier() {
        // 2026-08-04: `out let` inside a node body records "out".
        let src = "node work [true][done] { out let x: Int = 1; done = true; term; };";
        let tokens = tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        let item = p.parse_top_level().unwrap();
        if let crate::ast::TopLevel::Transaction(t) = item {
            assert!(t.body.iter().any(|s| matches!(s,
                crate::ast::Statement::Let { modifiers, .. } if modifiers.iter().any(|m| m.name == "out")
            )), "out modifier must be recorded on the let");
        } else {
            panic!("expected Transaction, got {item:?}");
        }
    }

    #[test]
    fn test_out_vol_let_records_both_modifiers() {
        // 2026-08-04: `out vol let` is legal — both pins recorded independently.
        let src = "node work [true][done] { out vol let x: Int = 1; done = true; term; };";
        let tokens = tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        let item = p.parse_top_level().unwrap();
        if let crate::ast::TopLevel::Transaction(t) = item {
            let lets: Vec<_> = t.body.iter().filter_map(|s| match s {
                crate::ast::Statement::Let { modifiers, .. } => Some(modifiers.clone()),
                _ => None,
            }).collect();
            let mods = lets.first().expect("expected a let");
            assert!(mods.iter().any(|m| m.name == "out"), "out modifier must be recorded");
            assert!(mods.iter().any(|m| m.name == "vol"), "vol modifier must be recorded");
        } else {
            panic!("expected Transaction, got {item:?}");
        }
    }
}


#[cfg(test)]
mod phase3_tests {
    use crate::lexer::tokenize;
    use crate::parser::Parser;

    fn parse_prog(src: &str) -> Vec<crate::ast::TopLevel> {
        let tokens = tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        p.parse_program().expect("parse failed")
    }

    fn body_of(tl: &crate::ast::TopLevel) -> Vec<crate::ast::Statement> {
        match tl {
            crate::ast::TopLevel::Definition(d) => d.body.clone(),
            _ => vec![],
        }
    }

    #[test]
    fn test_consumptive_ops_wrap_rhs_in_consume() {
        // a ~= b; a ~+ b; a ~- b; a ~* b; a ~/ b
        let prog = parse_prog(
            "defn f(a: Int, b: Int) -> Int {\n term a ~+ b;\n };\n\
             defn g(a: Int, b: Int) -> Int {\n a ~= b; term a;\n };\n",
        );
        // g: Statement::Assign(a, Consume(b))
        let defns: Vec<_> = prog
            .iter()
            .filter_map(|t| match t {
                crate::ast::TopLevel::Definition(d) => Some(d.name.clone()),
                _ => None,
            })
            .collect();
        assert!(defns.contains(&"g".to_string()));
        // f: term (a ~+ b) — BinaryOp(Add, a, Consume(b))
        let f = prog
            .iter()
            .find_map(|t| match t {
                crate::ast::TopLevel::Definition(d) if d.name == "f" => Some(d.body.clone()),
                _ => None,
            })
            .unwrap();
        let has_consume = f.iter().any(|s| match s {
            crate::ast::Statement::Term(Some(crate::ast::Expr::BinaryOp(_, _, r))) => {
                matches!(r.as_ref(), crate::ast::Expr::Consume(_))
            }
            _ => false,
        });
        assert!(has_consume, "~+ must wrap the RHS in Expr::Consume");
    }

    #[test]
    fn test_arrow_parses_as_arrow_assign() {
        // dest <- src; dest ~<- src; <- src; ~<- src;
        let prog = parse_prog(
            "defn f(a: Int, b: Int) -> Int {\n a <- b;\n a ~<- b;\n <- b;\n ~<- b;\n term a;\n };\n",
        );
        let body = body_of(&prog[0]);
        let kinds: Vec<String> = body
            .iter()
            .filter_map(|s| match s {
                crate::ast::Statement::ArrowAssign { target, consume, .. } => Some(format!(
                    "arrow:{}-{}",
                    consume,
                    target.is_some()
                )),
                _ => None,
            })
            .collect();
        assert_eq!(kinds, vec!["arrow:false-true", "arrow:true-true", "arrow:false-false", "arrow:true-false"]);
    }

    #[test]
    fn test_invert_contract_expands_pair() {
        // [!/X] → pre !X, post X ; [!/!X] → pre X, post !X
        let prog = parse_prog(
            "node n [!/ a > 0] { term; };\nnode m [!/! (b == 0)] { term; };\n",
        );
        // node n: pre = !(a > 0), post = a > 0
        let n = prog.iter().find_map(|t| match t {
            crate::ast::TopLevel::Transaction(tr) if tr.name == "n" => Some(tr.contract.clone()),
            _ => None,
        });
        let n = n.expect("node n");
        let pre_not = matches!(n.pre_condition, crate::ast::Expr::UnaryOp(crate::ast::UnaryOpKind::Not, _));
        assert!(pre_not, "node n pre must be !(a > 0)");
        assert!(!matches!(n.post_condition, crate::ast::Expr::UnaryOp(crate::ast::UnaryOpKind::Not, _)));
    }

    #[test]
    fn test_parse_seq_struct_preserves_flag() {
        // 2026-08-05 (Phase 4): `seq struct` sets the order/containment flag.
        let seq_items = parse_prog("seq struct Header { tag: Int; len: Int; };");
        let seq = seq_items.iter().find_map(|t| match t {
            crate::ast::TopLevel::StaticStruct(s) => Some(s.seq),
            _ => None,
        });
        assert_eq!(seq, Some(true), "seq struct must set the seq flag");

        let plain_items = parse_prog("struct Point { x: Int; };");
        let plain = plain_items.iter().find_map(|t| match t {
            crate::ast::TopLevel::StaticStruct(s) => Some(s.seq),
            _ => None,
        });
        assert_eq!(plain, Some(false), "plain struct must not set the seq flag");
    }

    #[test]
    fn test_parse_trait_declaration() {
        // 2026-08-05 (Phase 4): trait requirements, fields, and defaults.
        let items = parse_prog(
            "trait Sized {\n  Size: Int;\n  defn compare(left: Self, right: Self) -> Int;\n};\n",
        );
        let t = items.iter().find_map(|t| match t {
            crate::ast::TopLevel::Trait(t) => Some(t),
            _ => None,
        });
        let t = t.expect("trait must parse");
        assert_eq!(t.name, "Sized");
        assert_eq!(t.fields.len(), 1);
        assert_eq!(t.fields[0].0, "Size");
        assert_eq!(t.functions.len(), 1);
        assert_eq!(t.functions[0].name, "compare");
    }

    #[test]
    fn test_parse_impl_declaration() {
        // 2026-08-05 (Phase 4): impl attaches behavior to a data declaration.
        let items = parse_prog(
            "impl Point<Float> {\n  defn add_point(left: Point, right: Point) -> Point { term left; };\n};\n",
        );
        let i = items.iter().find_map(|t| match t {
            crate::ast::TopLevel::Impl(i) => Some(i),
            _ => None,
        });
        let i = i.expect("impl must parse");
        assert_eq!(i.target, "Point");
        assert_eq!(i.functions.len(), 1);
        assert_eq!(i.functions[0].name, "add_point");
    }

    // ── init kind (2026-08-09) ───────────────────────────────────────

    #[test]
    fn test_parse_init_expr_form() {
        // Expr seeding form with an unbounded init.
        let items = parse_prog("init BufSize: Int = get_env_int!(\"BUFSIZE\");\n");
        let init = items.iter().find_map(|t| match t {
            crate::ast::TopLevel::Init(i) => Some(i),
            _ => None,
        });
        let init = init.expect("init must parse");
        assert_eq!(init.name, "BufSize");
        assert_eq!(init.ty, crate::ast::Type::int());
        assert!(init.bound.is_none(), "no bound declared");
        assert!(init.value.is_some(), "expr form has a value");
        assert!(init.body.is_empty(), "expr form has no body");
    }

    #[test]
    fn test_parse_init_bounded_set() {
        // Bounded set: `[16 | 32 | 64]` discrete union and `[64 | lo..hi]`
        // mixed union, both kind-attached between `:` and the type.
        let items = parse_prog(
            "init BitLayout: [16 | 32 | 64] Int = pick(target);\n\
             init Shift: [64 | base..high] Int = 16;\n",
        );
        let inits: Vec<&crate::ast::top::InitDecl> = items
            .iter()
            .filter_map(|t| match t {
                crate::ast::TopLevel::Init(i) => Some(i),
                _ => None,
            })
            .collect();
        assert_eq!(inits.len(), 2);

        let bit = &inits[0];
        assert_eq!(bit.ty, crate::ast::Type::int());
        match &bit.bound {
            Some(crate::ast::BoundSpec::Choice(parts)) => {
                assert_eq!(parts.len(), 3);
                assert_eq!(parts[0], crate::ast::BoundSpec::Single(crate::ast::BoundTerm::Lit(16)));
                assert_eq!(parts[2], crate::ast::BoundSpec::Single(crate::ast::BoundTerm::Lit(64)));
            }
            other => panic!("expected Choice(16|32|64), got {:?}", other),
        }

        let shift = &inits[1];
        match &shift.bound {
            Some(crate::ast::BoundSpec::Choice(parts)) => {
                assert_eq!(parts.len(), 2);
                assert_eq!(parts[0], crate::ast::BoundSpec::Single(crate::ast::BoundTerm::Lit(64)));
                assert_eq!(
                    parts[1],
                    crate::ast::BoundSpec::Range(
                        crate::ast::BoundTerm::Ref("base".into()),
                        crate::ast::BoundTerm::Ref("high".into()),
                    )
                );
            }
            other => panic!("expected Choice(64 | base..high), got {:?}", other),
        }
    }

    #[test]
    fn test_parse_init_body_form() {
        // Body form: seeds once before beginprogram; no `= expr`.
        let items = parse_prog(
            "init Layout: [16 | 32 | 64] Int {\n    term pick(target);\n};\n",
        );
        let init: &crate::ast::top::InitDecl = items
            .iter()
            .find_map(|t| match t {
                crate::ast::TopLevel::Init(i) => Some(i),
                _ => None,
            })
            .expect("init must parse");
        assert!(init.value.is_none(), "body form has no = expr");
        assert_eq!(init.body.len(), 1, "body form has statements");
        assert_eq!(
            init.bound,
            Some(crate::ast::BoundSpec::Choice(vec![
                crate::ast::BoundSpec::Single(crate::ast::BoundTerm::Lit(16)),
                crate::ast::BoundSpec::Single(crate::ast::BoundTerm::Lit(32)),
                crate::ast::BoundSpec::Single(crate::ast::BoundTerm::Lit(64)),
            ]))
        );
    }

    #[test]
    fn test_parse_init_contextual_keyword_preserves_txn_init() {
        // `init` stays a legal identifier in non-declaration positions, so
        // stdlib `txn init(...)` / `op Init: init(#Lh,#Rh)` still parse.
        let items = parse_prog(
            "obj Stack<T, N> {\n\
                     data: T[N];\n\
                     len: Int;\n\
                     op Init: init(#Lh, #Rh);\n\
                     txn init(val: T) [true][len == 1] { data[0] = val; len = 1; };\n\
                 };\n\
                 let b: Stack<Int, 5> = 0;\n",
        );
        let txn_init = items.iter().any(|t| match t {
            crate::ast::TopLevel::TypeDef(td) => td.body.members.iter().any(|member| match member {
                crate::ast::TopLevel::Transaction(tr) => tr.name == "init",
                _ => false,
            }),
            _ => false,
        });
        assert!(txn_init, "txn init must remain legal inside obj");
        let has_top_init = items
            .iter()
            .any(|t| matches!(t, crate::ast::TopLevel::Init(_)));
        assert!(!has_top_init, "no init declaration at top level here");
    }

    // ── 2026-08-09 (Phase 11, Slice 2): `:` module alias ─────────────

    #[test]
    fn import_module_alias_parses() {
        // `import collections: <std/collections>;` — a collision-resolving
        // local tag (SPEC §7.2). The path follows the `:`.
        let src = "import collections: <std/collections>;";
        let tokens = crate::lexer::tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        let items = p.parse_program().unwrap();
        match &items[0] {
            crate::ast::TopLevel::Import(imp) => {
                assert_eq!(imp.alias.as_deref(), Some("collections"));
                assert_eq!(imp.path(), "std/collections");
            }
            other => panic!("expected import, got {other:?}"),
        }
    }

    #[test]
    fn import_alias_with_literal_path_parses() {
        let src = "import x: \"local/path.bv\";";
        let tokens = crate::lexer::tokenize(src).unwrap();
        let mut p = Parser::new(tokens, src);
        let items = p.parse_program().unwrap();
        match &items[0] {
            crate::ast::TopLevel::Import(imp) => {
                assert_eq!(imp.alias.as_deref(), Some("x"));
                assert_eq!(imp.path(), "local/path.bv");
            }
            other => panic!("expected import, got {other:?}"),
        }
    }
}
