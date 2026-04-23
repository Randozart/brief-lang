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
use crate::lexer::Token;
use logos::{Lexer, Logos};
use std::path::Path;

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
    comments: Vec<Comment>,
    current_line: usize,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        let mut lexer = Token::lexer(input);
        let current = lexer.next().map(|token| (token, lexer.span()));
        let peek = lexer.next().map(|token| (token, lexer.span()));
        Parser {
            lexer,
            source: input,
            pos: 0,
            current,
            peek,
            comments: Vec::new(),
            current_line: 1,
        }
    }

    fn advance(&mut self) {
        self.current = self.peek.take();
        self.peek = self.lexer.next().map(|token| (token, self.lexer.span()));

        if let Some((_, span)) = &self.current {
            self.current_line = span.start;
        }
    }

    fn put_back(&mut self, token: Token, span: logos::Span) {
        self.peek = self.current.take();
        self.current = Some((Ok(token), span));
    }

    fn current_token(&self) -> Option<&Result<Token, ()>> {
        self.current.as_ref().map(|(t, _)| t)
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

    fn expect(&mut self, expected: Token) -> Result<(), crate::errors::SyntaxError> {
        let span = self.current_span().unwrap_or_else(Span::dummy);
        match self.current_token() {
            Some(Ok(tok)) if *tok == expected => {
                self.advance();
                Ok(())
            }
            Some(Ok(tok)) => Err(crate::errors::SyntaxError::UnexpectedToken {
                expected: format!("{:?}", expected),
                found: format!("{:?}", tok),
                span,
            }),
            Some(Err(_)) => Err(crate::errors::SyntaxError::InvalidStatement {
                reason: "Lexer error".to_string(),
                span,
            }),
            None => Err(crate::errors::SyntaxError::UnexpectedEOF {
                expected: format!("{:?}", expected),
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
            Some(Ok(Token::TypeData)) => {
                self.advance();
                Ok("Data".to_string())
            }
            Some(Ok(Token::TypeInt)) => {
                self.advance();
                Ok("Int".to_string())
            }
            _ => Err(SyntaxError::UnexpectedToken {
                expected: "identifier".to_string(),
                found: format!("{:?}", self.current_token()),
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
                found: format!("{:?}", self.current_token()),
                span,
            }),
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
                found: format!("{:?}", tok),
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

    pub fn parse(&mut self) -> Result<Program, crate::errors::SyntaxError> {
        let mut reactor_speed: Option<u32> = None;
        let mut items = Vec::new();

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
                            reason: "Reactor speed must be positive (> 0)".to_string(),
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
                        found: format!("{:?}", self.current_token()),
                        span: self.current_span().unwrap_or_else(Span::dummy),
                    });
                }
            }
        }

        while self.current_token().is_some() {
            items.push(self.parse_top_level()?);
        }
        Ok(Program {
            items,
            comments: self.comments.clone(),
            reactor_speed,
        })
    }

    fn parse_top_level(&mut self) -> Result<TopLevel, SyntaxError> {
        let span = self.current_span().unwrap_or_else(Span::dummy);
        if self.current_token().is_none() {
            return Err(SyntaxError::UnexpectedEOF {
                expected: "top level item".to_string(),
                span,
            });
        }

        match self.current_token() {
            Some(Ok(Token::Import)) => {
                let import = self.parse_import()?;
                Ok(TopLevel::Import(import))
            }
            Some(Ok(Token::Sig)) => {
                let sig = self.parse_signature()?;
                Ok(TopLevel::Signature(sig))
            }
            Some(Ok(Token::Let)) => {
                let state = self.parse_state_decl()?;
                Ok(TopLevel::StateDecl(state))
            }
            Some(Ok(Token::Const)) => {
                let constant = self.parse_constant()?;
                Ok(TopLevel::Constant(constant))
            }
            Some(Ok(Token::Txn)) | Some(Ok(Token::Rct)) | Some(Ok(Token::Async)) => {
                let txn = self.parse_transaction()?;
                Ok(TopLevel::Transaction(txn))
            }

            Some(Ok(Token::Defn)) => {
                let defn = self.parse_definition()?;
                Ok(TopLevel::Definition(defn))
            }
            Some(Ok(Token::Trg)) => {
                let trg = self.parse_trigger()?;
                Ok(TopLevel::Trigger(trg))
            }
            Some(Ok(Token::Frgn)) => {
                let frgn_binding = self.parse_frgn_binding(FfiKind::Frgn)?;
                Ok(frgn_binding)
            }
            Some(Ok(Token::FrgnBang)) => {
                let frgn_binding = self.parse_frgn_binding(FfiKind::FrgnBang)?;
                Ok(frgn_binding)
            }
            Some(Ok(Token::Syscall)) => {
                let frgn_binding = self.parse_frgn_binding(FfiKind::Syscall)?;
                Ok(frgn_binding)
            }
            Some(Ok(Token::SyscallBang)) => {
                let frgn_binding = self.parse_frgn_binding(FfiKind::SyscallBang)?;
                Ok(frgn_binding)
            }
            Some(Ok(Token::Resource)) | Some(Ok(Token::Rsrc)) => {
                let resource = self.parse_resource()?;
                Ok(resource)
            }
            Some(Ok(Token::Struct)) => {
                let struct_def = self.parse_struct()?;
                Ok(TopLevel::Struct(struct_def))
            }
            Some(Ok(Token::Rstruct)) => {
                let rstruct_def = self.parse_rstruct()?;
                Ok(TopLevel::RStruct(rstruct_def))
            }
            Some(Ok(Token::Enum)) => {
                let enum_def = self.parse_enum()?;
                Ok(TopLevel::Enum(enum_def))
            }
            Some(Ok(Token::Render)) => {
                let render_block = self.parse_render_block()?;
                Ok(TopLevel::RenderBlock(render_block))
            }
            Some(Ok(tok)) => Err(SyntaxError::UnexpectedToken {
                expected: "top-level declaration".to_string(),
                found: format!("{:?}", tok),
                span,
            }),
            Some(Err(_)) => Err(SyntaxError::InvalidStatement {
                reason: "Lexer error at top level".to_string(),
                span,
            }),
            None => Err(SyntaxError::UnexpectedEOF {
                expected: "top-level declaration".to_string(),
                span,
            }),
        }
    }

    fn parse_import(&mut self) -> Result<Import, SyntaxError> {
        self.expect(Token::Import)?;

        let mut items = if let Some(Ok(Token::LBrace)) = self.current_token() {
            self.advance();
            let mut items = Vec::new();
            while let Some(Ok(Token::Identifier(_))) = self.current_token() {
                let name = self.expect_identifier()?;
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
            self.expect(Token::RBrace)?;
            items
        } else {
            Vec::new()
        };

        let path = if let Some(Ok(Token::From)) = self.current_token() {
            self.advance();
            // Support quoted string paths like "./landing.css" or "./icons/logo.svg"
            if let Some(Ok(Token::String(s))) = self.current_token() {
                let s = s.clone();
                self.advance();
                // Convert "./path/file.css" to ["path", "file.css"]
                let trimmed = s.trim_start_matches("./");
                let parts: Vec<String> = trimmed.split('/').map(String::from).collect();
                parts
            } else {
                let mut path = Vec::new();
                path.push(self.expect_identifier()?);
                while let Some(Ok(Token::Dot)) = self.current_token() {
                    self.advance();
                    path.push(self.expect_identifier()?);
                }
                path
            }
        } else if let Some(Ok(Token::String(s))) = self.current_token() {
            // Support direct quoted path: import "./file.css";
            // Also support: import "./file.svg" as Name;
            let s = s.clone();
            self.advance();
            let trimmed = s.trim_start_matches("./");
            let parts: Vec<String> = trimmed.split('/').map(String::from).collect();

            // Check for 'as Name' after the path
            if let Some(Ok(Token::As)) = self.current_token() {
                self.advance();
                let name = self.expect_identifier()?;
                // For imports like `import "./logo.svg" as Logo;`, create an import item
                items.push(ImportItem { name, alias: None });
            }

            parts
        } else if let Some(Ok(Token::Identifier(_))) = self.current_token() {
            if !items.is_empty() {
                return self.spanned_err(
                    "Cannot have both import items and direct namespace path. Use 'from' keyword."
                        .to_string(),
                );
            }
            let mut path = Vec::new();
            path.push(self.expect_identifier()?);
            while let Some(Ok(Token::Dot)) = self.current_token() {
                self.advance();
                path.push(self.expect_identifier()?);
            }
            path
        } else {
            Vec::new()
        };

        self.expect(Token::Semicolon)?;
        Ok(Import { items, path })
    }

    fn parse_signature(&mut self) -> Result<Signature, SyntaxError> {
        self.expect(Token::Sig)?;
        let name = self.expect_identifier()?;
        self.expect(Token::Colon)?;
        let input_type = self.parse_type()?;
        self.expect(Token::Arrow)?;

        let result_type = self.parse_result_type()?;

        // NEW: Parse optional defn binding: sig name: Input -> Output = defn_name;
        let bound_defn = if let Some(Ok(Token::Eq)) = self.current_token() {
            self.advance();
            let defn_name = self.expect_identifier()?;
            // Optionally parse arguments if present (e.g., = complex(x))
            if let Some(Ok(Token::LParen)) = self.current_token() {
                self.advance();
                let mut depth = 1;
                while depth > 0 {
                    match self.current_token() {
                        Some(Ok(Token::LParen)) => depth += 1,
                        Some(Ok(Token::RParen)) => depth -= 1,
                        _ => {;}
                    }
                    self.advance();
                }
            }
            Some(defn_name)
        } else {
            None
        };

        let source = if let Some(Ok(Token::From)) = self.current_token() {
            self.advance();
            let mut path = Vec::new();
            path.push(self.expect_identifier()?);
            while let Some(Ok(Token::Dot)) = self.current_token() {
                self.advance();
                path.push(self.expect_identifier()?);
            }
            Some(path.join("."))
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
            input_types: vec![input_type],
            result_type,
            source,
            alias,
            bound_defn,
        })
    }

    /// Convert a type name string to a Type
    fn string_to_type(&self, type_name: &str) -> Result<Type, SyntaxError> {
        match type_name {
            "String" => Ok(Type::String),
            "Int" => Ok(Type::Int),
            "Float" => Ok(Type::Float),
            "Bool" => Ok(Type::Bool),
            "void" => Ok(Type::Void),
            "Data" => Ok(Type::Data),
            other => Ok(Type::Custom(other.to_string())),
        }
    }

    /// Parse a foreign function binding declaration
    /// Syntax: frgn name(param: Type, ...) -> Result<T, E> from "binding.toml";
    fn parse_frgn_binding(&mut self, ffi_kind: FfiKind) -> Result<TopLevel, SyntaxError> {
        use crate::ast::{ForeignBinding, ForeignSignature, ForeignTarget, ResultType, FfiKind};

        self.expect(Token::Frgn)?;
        let name = self.expect_identifier()?;

        // Parse parameters
        self.expect(Token::LParen)?;
        let mut inputs = Vec::new();
        while let Some(Ok(Token::Identifier(_))) = self.current_token() {
            let param_name = self.expect_identifier()?;
            self.expect(Token::Colon)?;
            let param_type_name = self.expect_identifier()?;
            let param_type = self.string_to_type(&param_type_name)?;
            inputs.push((param_name, param_type));

            if let Some(Ok(Token::Comma)) = self.current_token() {
                self.advance();
            } else {
                break;
            }
        }
        self.expect(Token::RParen)?;

        // Parse return type: Result<SuccessType, ErrorType>
        self.expect(Token::Arrow)?;

        // Expect "Result<T, E>" pattern
        if let Some(Ok(Token::Identifier(result_id))) = self.current_token() {
            if result_id != "Result" {
                return self.spanned_err(format!("Expected 'Result<T, E>', found {}", result_id));
            }
            self.advance();
        } else {
            return self.spanned_err("Expected Result type for frgn binding".to_string());
        }

        // Parse <SuccessType, E>
        self.expect(Token::Lt)?;

        // Parse success type - could be simple identifier or tuple syntax (field1: T1, field2: T2)
        let mut success_output = Vec::new();

        if let Some(Ok(Token::LParen)) = self.current_token() {
            // Multi-field success output: (field1: T1, field2: T2)
            self.advance();
            loop {
                let field_name = self.expect_identifier()?;
                self.expect(Token::Colon)?;
                let field_type_name = self.expect_identifier()?;
                let field_type = self.string_to_type(&field_type_name)?;
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
            let success_type_name = self.expect_identifier()?;
            let success_type = self.string_to_type(&success_type_name)?;
            success_output.push(("result".to_string(), success_type));
        }

        self.expect(Token::Comma)?;

        // Parse error type (just the name)
        let error_type_name = self.expect_identifier()?;

        self.expect(Token::Gt)?;

        // Parse "from" clause
        self.expect(Token::From)?;

        // Parse TOML path
        let toml_path = if let Some(Ok(Token::String(s))) = self.current_token() {
            let path = s.clone();
            self.advance();
            path
        } else {
            return self.spanned_err("Expected TOML file path as string".to_string());
        };

        self.expect(Token::Semicolon)?;

        // For now, error fields are empty until we load the TOML
        let error_fields = Vec::new();

        let frgn_sig = ForeignSignature {
            name: name.clone(),
            location: String::new(),
            wasm_impl: None,
            wasm_setup: None,
            inputs,
            success_output,
            error_type_name: error_type_name.clone(),
            error_fields,
            input_layout: None,
            output_layout: None,
            precondition: None,
            postcondition: None,
            buffer_mode: None,
            result_type: ResultType::TrueAssertion,
            ffi_kind: Some(crate::ast::FfiKind::Frgn),
            span: None,
        };

        Ok(TopLevel::ForeignBinding {
            name,
            toml_path,
            signature: frgn_sig,
            target: ForeignTarget::Native,
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

        self.expect(Token::LBrace)?;

        let mut fields = Vec::new();
        let mut transactions = Vec::new();

        while let Some(token) = self.current_token() {
            match token {
                Ok(Token::RBrace) => {
                    self.advance();
                    break;
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
                            // No initializer - error
                            return self.spanned_err(format!(
                                "struct field '{}' must have initial value (e.g., let {} = 0;)",
                                field_name, field_name
                            ));
                        };

                        self.expect(Token::Semicolon)?;
                        fields.push(StructField {
                            name: field_name,
                            ty: field_type,
                            default,
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

                    if let Some(Ok(Token::Colon)) = self.peek() {
                        let field_name = self.expect_identifier()?;
                        self.expect(Token::Colon)?;
                        let field_type = self.parse_type()?;

                        // Parse optional initializer - check current token
                        let default = if let Some(Ok(Token::Eq)) = self.current_token() {
                            self.advance(); // consume '='
                            Some(self.parse_expression()?)
                        } else {
                            return self.spanned_err(format!(
                                "struct field '{}' must have initial value (e.g., let {} = 0;)",
                                field_name, field_name
                            ));
                        };

                        self.expect(Token::Semicolon)?;
                        fields.push(StructField {
                            name: field_name,
                            ty: field_type,
                            default,
                        });
                    } else {
                        // Not a field, treat as transaction
                        let txn = self.parse_transaction()?;
                        transactions.push(txn);
                    }
                }
                _ => {
                    return self.spanned_err(format!("Unexpected token in struct: {:?}", token));
                }
            }
        }

        let span = self.current_span();
        self.expect(Token::Semicolon)?;
        Ok(StructDefinition {
            name,
            fields,
            transactions,
            view_html: None,
            span,
        })
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
                            // No initializer - error
                            return self.spanned_err(format!(
                                "rstruct field '{}' must have initial value (e.g., let {} = 0;)",
                                field_name, field_name
                            ));
                        };

                        self.expect(Token::Semicolon)?;
                        fields.push(StructField {
                            name: field_name,
                            ty: field_type,
                            default,
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

                    if let Some(Ok(Token::Colon)) = self.peek() {
                        let field_name = self.expect_identifier()?;
                        self.expect(Token::Colon)?;
                        let field_type = self.parse_type()?;

                        // Parse optional initializer - check current token (not peek)
                        let default = if let Some(Ok(Token::Eq)) = self.current_token() {
                            self.advance(); // consume '='
                            Some(self.parse_expression()?)
                        } else {
                            return self.spanned_err(format!(
                                "rstruct field '{}' must have initial value (e.g., let {} = 0;)",
                                field_name, field_name
                            ));
                        };

                        self.expect(Token::Semicolon)?;
                        fields.push(StructField {
                            name: field_name,
                            ty: field_type,
                            default,
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
                _ => {
                    return self.spanned_err(format!("Unexpected token in rstruct: {:?}", token));
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
        self.expect(Token::Semicolon)?;

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
        if let Some(Ok(Token::Lt)) = self.peek() {
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
                    break;
                }
                Ok(Token::Identifier(variant_name)) => {
                    let variant_name_str = variant_name.to_string();
                    self.advance();

                    // Check for tuple variant: Ok(T) or Err(E)
                    let variant = if let Some(Ok(Token::LParen)) = self.peek() {
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
                _ => return self.spanned_err(format!("Unexpected token in enum: {:?}", token)),
            }
        }

        Ok(EnumDefinition {
            name,
            type_params,
            variants,
            span: self.current_span(),
        })
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
                    .spanned_err(format!("Expected LBrace, found {:?}", self.current_token()));
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

    fn parse_state_decl(&mut self) -> Result<StateDecl, SyntaxError> {
        self.expect(Token::Let)?;
        let name = self.expect_identifier()?;

        let mut address: Option<u64> = None;
        let mut bit_range: Option<BitRange> = None;
        let mut is_override = false;

        // Optional mapping before colon
            loop {
                if let Some(Ok(Token::At)) = self.current_token() {
                    self.advance();
                    match self.current_token() {
                        Some(Ok(Token::Integer(n))) => {
                            address = Some(*n as u64);
                            self.advance();
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
                        _ => return self.spanned_err("Expected address mode after @: raw, stack, or heap".to_string()),
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
                self.advance();
                match self.current_token() {
                    Some(Ok(Token::Integer(n))) => {
                        address = Some(*n as u64);
                        self.advance();
                    }
                    _ => return self.spanned_err("Expected integer address after '@'".to_string()),
                }
                // Handle slash shorthand: @0x1000/x16 or @0x1000/0
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
            is_override,
            os_mode: false,
            span,
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

    fn parse_trigger(&mut self) -> Result<TriggerDeclaration, SyntaxError> {
        self.expect(Token::Trg)?;
        let name = self.expect_identifier()?;
        self.expect(Token::Colon)?;
        let ty = self.parse_type()?;

        let mut address: u64 = 0;
        let mut bit_range: Option<BitRange> = None;

        loop {
            if let Some(Ok(Token::At)) = self.current_token() {
                self.advance();
                match self.current_token() {
                    Some(Ok(Token::Integer(n))) => {
                        address = *n as u64;
                        self.advance();
                    }
                    _ => return self.spanned_err("Expected integer address after '@'".to_string()),
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

        let span = self.current_span();
        self.expect(Token::Semicolon)?;

        Ok(TriggerDeclaration {
            name,
            ty,
            address,
            bit_range,
            stages,
            condition,
            span,
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

        // Parse optional parameters - NOT allowed for rct transactions
        let parameters = if let Some(Ok(Token::LParen)) = self.current_token() {
            self.advance();
            let mut params = Vec::new();
            while let Some(Ok(Token::Identifier(_))) = self.current_token() {
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
            self.expect(Token::RParen)?;
            params
        } else {
            Vec::new()
        };

        // Validate: rct transactions cannot have parameters
        if is_reactive && !parameters.is_empty() {
            return self.spanned_err("rct transactions cannot have parameters".to_string());
        }

        let contract = self.parse_contract()?;

        // Lambda-style: allow ; termination (no body)
        let body = if let Some(Ok(Token::Semicolon)) = self.current_token() {
            // Lambda-style transaction: no body, just contract
            Vec::new()
        } else {
            self.expect(Token::LBrace)?;
            let body = self.parse_body()?;
            self.expect(Token::RBrace)?;
            body
        };

        let is_lambda = body.is_empty();

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

        self.expect(Token::Semicolon)?;

        let dependencies = contract
            .pre_condition
            .extract_dependencies()
            .into_iter()
            .collect();

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
        })
    }

    fn parse_definition(&mut self) -> Result<Definition, SyntaxError> {
        // def/defn/definition all map to Token::Defn via lexer aliases
        self.expect(Token::Defn)?;
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
            while let Some(Ok(Token::Identifier(_))) = self.current_token() {
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
            self.expect(Token::RParen)?;
            params
        } else {
            Vec::new()
        };

        let (outputs, output_names, output_type) =
            if let Some(Ok(Token::Arrow)) = self.current_token() {
                self.advance();
                let (outputs, output_names) = self.parse_output_types_with_names(&parameters)?;

                // Detect if this is a union or tuple
                let output_type = if outputs.is_empty() {
                    None
                } else if outputs.len() == 1 {
                    // Single output - no special OutputType needed
                    None
                } else {
                    // Multiple outputs - need to determine if union or tuple
                    Some(crate::ast::OutputType::Tuple(outputs.clone()))
                };

                (outputs, output_names, output_type)
            } else {
                (Vec::new(), Vec::new(), None)
            };

        // Contract comes AFTER output types for defn
        let contract = self.parse_contract()?;

        // Lambda-style: allow ; termination (no body)
        let body = if let Some(Ok(Token::Semicolon)) = self.current_token() {
            // Lambda-style definition: no body
            Vec::new()
        } else {
            self.expect(Token::LBrace)?;
            let body = self.parse_body()?;
            self.expect(Token::RBrace)?;
            self.expect(Token::Semicolon)?;
            body
        };

        let is_lambda = body.is_empty();

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
        })
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

            // Parse the type
            outputs.push(self.parse_type()?);
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
    fn parse_output_type_structure(&mut self) -> Result<Option<OutputType>, SyntaxError> {
        use crate::ast::OutputType;

        let mut all_types = Vec::new();
        let mut has_pipe = false;
        let mut has_comma = false;

        // Parse first type
        all_types.push(self.parse_type()?);

        // Look for pipes (union) or commas (tuple)
        loop {
            match self.current_token() {
                Some(Ok(Token::Pipe)) => {
                    has_pipe = true;
                    self.advance();
                    all_types.push(self.parse_type()?);
                }
                Some(Ok(Token::Comma)) => {
                    has_comma = true;
                    self.advance();
                    all_types.push(self.parse_type()?);
                }
                _ => break,
            }
        }

        // Determine structure based on what we found
        if all_types.len() == 1 {
            // Single output - no special structure needed
            Ok(None)
        } else if has_pipe && !has_comma {
            // Pure union: A | B | C
            Ok(Some(OutputType::Union(all_types)))
        } else if has_comma && !has_pipe {
            // Pure tuple: A, B, C
            Ok(Some(OutputType::Tuple(all_types)))
        } else if has_pipe && has_comma {
            // Mixed: Handle as tuple, but first element is union
            // For now, simplify to tuple (future: could model as tuple of unions)
            Ok(Some(OutputType::Tuple(all_types)))
        } else {
            Ok(None)
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

    fn parse_contract(&mut self) -> Result<Contract, SyntaxError> {
        let mut pre_condition = Expr::Bool(true);
        let mut post_condition = Expr::Bool(true);
        let mut watchdog: Option<WatchdogSpec> = None;

        let mut count = 0;
        while let Some(Ok(Token::LBracket)) = self.current_token() {
            self.advance(); // consume [

            // Check for ~/ syntax - this is a shorthand for [~identifier][identifier]
            if let Some(Ok(Token::TildeSlash)) = self.current_token() {
                self.advance(); // Consume ~/
                let identifier = self.expect_identifier()?;
                pre_condition = Expr::Not(Box::new(Expr::Identifier(identifier.clone())));
                post_condition = Expr::Identifier(identifier);
                self.expect(Token::RBracket)?;
                break;
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

                if matches!(cond, Expr::Bool(true)) {
                    return self.spanned_err("Watchdog cannot be [true] - must verify something".to_string());
                }

                watchdog = Some(WatchdogSpec {
                    condition: cond,
                    is_required: !is_optional, // default is required
                });
            } else {
                return self.spanned_err("Too many contract brackets (max 3: [pre][post][watchdog])".to_string());
            }

            count += 1;
            self.expect(Token::RBracket)?;
        }

        let span = self.current_span();
        Ok(Contract {
            pre_condition,
            post_condition,
            watchdog,
            span,
        })
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

    fn parse_statement(&mut self) -> Result<Statement, SyntaxError> {
        match self.current_token() {
            Some(Ok(Token::Let)) => {
                self.advance();
                let name = self.expect_identifier()?;

                let mut address: Option<u64> = None;
                let mut bit_range: Option<BitRange> = None;
                let mut is_override = false;

                // Optional mapping before colon
                loop {
                    if let Some(Ok(Token::At)) = self.current_token() {
                        self.advance();
                        if let Expr::Integer(n) = self.parse_expression()? {
                            address = Some(n as u64);
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
                            if let Expr::Integer(n) = self.parse_expression()? {
                                address = Some(n as u64);
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
                    bit_range,
                    is_override,
                })
            }
            Some(Ok(Token::Term)) => {
                self.advance();
                let outputs = self.parse_term_outputs()?;
                self.expect(Token::Semicolon)?;
                Ok(Statement::Term(outputs))
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
                            _ => unreachable!(),
                        };

                        // Expect ( for pattern fields
                        if matches!(self.current_token(), Some(Ok(Token::LParen))) {
                            self.advance(); // consume (
                            let mut fields = Vec::new();
                            while let Some(Ok(Token::Identifier(field_name))) = self.current_token()
                            {
                                fields.push(field_name.clone());
                                self.advance();
                                if let Some(Ok(Token::Comma)) = self.current_token() {
                                    self.advance();
                                } else {
                                    break;
                                }
                            }
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
            _ => {
                // Expression statement or Assignment/Unification
                let expr = self.parse_expression()?;

                if let Some(Ok(Token::Eq)) = self.current_token() {
                    self.advance();
                    let right = self.parse_expression()?;

                    let mut timeout: Option<(Expr, TimeUnit)> = None;
                    if let Some(Ok(Token::Within)) = self.current_token() {
                        self.advance();
                        let expr = self.parse_expression()?;
                        let unit = match self.current_token() {
                            Some(Ok(Token::Cycles)) => {
                                self.advance();
                                TimeUnit::Cycles
                            }
                            Some(Ok(Token::Cyc)) => {
                                self.advance();
                                TimeUnit::Cycles
                            }
                            Some(Ok(Token::Ms)) => {
                                self.advance();
                                TimeUnit::Ms
                            }
                            Some(Ok(Token::Seconds)) => {
                                self.advance();
                                TimeUnit::Seconds
                            }
                            Some(Ok(Token::Minute)) => {
                                self.advance();
                                TimeUnit::Minutes
                            }
                            _ => TimeUnit::Cycles,
                        };
                        timeout = Some((expr, unit));
                    }

                    self.expect(Token::Semicolon)?;

                    match expr {
                        Expr::Call(name, args) => {
                            if args.len() == 1 {
                                if let Expr::Identifier(pattern) = &args[0] {
                                    Ok(Statement::Unification {
                                        name,
                                        pattern: pattern.clone(),
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
                        _ => Ok(Statement::Assignment {
                            lhs: expr,
                            expr: right,
                            timeout,
                        }),
                    }
                } else {
                    self.expect(Token::Semicolon)?;
                    Ok(Statement::Expression(expr))
                }
            }
        }
    }

    fn parse_type(&mut self) -> Result<Type, SyntaxError> {
        let mut ty = match self.current_token() {
            Some(Ok(Token::Identifier(name))) => {
                let name = name.clone();
                self.advance();
                // Create as Custom - type checker will resolve to Sig if needed
                Type::Custom(name)
            }
            Some(Ok(Token::TypeData)) => {
                self.advance();
                Type::Data
            }
            Some(Ok(Token::TypeInt)) => {
                self.advance();
                Type::Int
            }
            Some(Ok(Token::TypeUInt))
            | Some(Ok(Token::TypeUnsigned))
            | Some(Ok(Token::TypeUSgn)) => {
                self.advance();
                Type::UInt
            }
            Some(Ok(Token::TypeSigned)) | Some(Ok(Token::TypeSgn)) => {
                self.advance();
                Type::Int
            }
            Some(Ok(Token::TypeFloat)) => {
                self.advance();
                Type::Float
            }
            Some(Ok(Token::TypeString)) => {
                self.advance();
                Type::String
            }
            Some(Ok(Token::TypeBool)) => {
                self.advance();
                Type::Bool
            }
            Some(Ok(Token::TypeVoid)) => {
                self.advance();
                Type::Void
            }
            Some(Ok(Token::LParen)) => {
                self.advance();
                self.expect(Token::RParen)?;
                Type::Void
            }
            Some(Ok(tok)) => return self.spanned_err(format!("Expected type, found {:?}", tok)),
            Some(Err(_)) => return self.spanned_err("Lexer error".to_string()),
            None => return self.spanned_err("Expected type, found EOF".to_string()),
        };

        // Check for bit-width decorator: Type@/N or Type@/0..7 or Type@/xN
        if let Some(Ok(Token::At)) = self.current_token() {
            if let Some(Ok(Token::Slash)) = self.peek() {
                self.advance(); // consume @
                self.advance(); // consume /
                // Skip constraint parsing for now - use the base type
            }
        }
        if let Some(Ok(Token::Lt)) = self.current_token() {
            self.advance();
            let mut type_args = Vec::new();
            loop {
                type_args.push(self.parse_type()?);
                if let Some(Ok(Token::Comma)) = self.current_token() {
                    self.advance();
                } else {
                    break;
                }
            }
            self.expect(Token::Gt)?;
            ty = Type::Applied(
                match &ty {
                    Type::Custom(name) => name.clone(),
                    _ => return self.spanned_err("Generic type must have a base name".to_string()),
                },
                type_args,
            );
        }

        // Check for vector dimension: Type[N]
        while let Some(Ok(Token::LBracket)) = self.current_token() {
            if !matches!(self.peek(), Some(Ok(Token::Integer(_)))) {
                break;
            }
            self.advance();
            if let Some(Ok(Token::Integer(n))) = self.current_token() {
                let size = *n as usize;
                self.advance();
                self.expect(Token::RBracket)?;
                ty = Type::Vector(Box::new(ty), size);
            } else {
                return self.spanned_err("Expected vector size".to_string());
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
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, SyntaxError> {
        let mut left = self.parse_and()?;
        while let Some(Ok(Token::OrOr)) = self.current_token() {
            self.advance();
            let right = self.parse_and()?;
            left = Expr::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, SyntaxError> {
        let mut left = self.parse_bitwise_or()?;
        while let Some(Ok(Token::AndAnd)) = self.current_token() {
            self.advance();
            let right = self.parse_bitwise_or()?;
            left = Expr::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_bitwise_or(&mut self) -> Result<Expr, SyntaxError> {
        let mut left = self.parse_bitwise_xor()?;
        while let Some(Ok(Token::Pipe)) = self.current_token() {
            self.advance();
            let right = self.parse_bitwise_xor()?;
            left = Expr::BitOr(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_bitwise_xor(&mut self) -> Result<Expr, SyntaxError> {
        let mut left = self.parse_bitwise_and()?;
        while let Some(Ok(Token::BitXor)) = self.current_token() {
            self.advance();
            let right = self.parse_bitwise_and()?;
            left = Expr::BitXor(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_bitwise_and(&mut self) -> Result<Expr, SyntaxError> {
        let mut left = self.parse_equality()?;
        while let Some(Ok(Token::Ampersand)) = self.current_token() {
            self.advance();
            let right = self.parse_equality()?;
            left = Expr::BitAnd(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> Result<Expr, SyntaxError> {
        let mut left = self.parse_comparison()?;
        while let Some(token) = self.current_token() {
            match token {
                Ok(Token::EqEq) => {
                    self.advance();
                    let right = self.parse_comparison()?;
                    left = Expr::Eq(Box::new(left), Box::new(right));
                }
                Ok(Token::Ne) => {
                    self.advance();
                    let right = self.parse_comparison()?;
                    left = Expr::Ne(Box::new(left), Box::new(right));
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expr, SyntaxError> {
        let mut left = self.parse_shift()?;
        while let Some(token) = self.current_token() {
            match token {
                Ok(Token::Lt) => {
                    self.advance();
                    let right = self.parse_shift()?;
                    left = Expr::Lt(Box::new(left), Box::new(right));
                }
                Ok(Token::Le) => {
                    self.advance();
                    let right = self.parse_shift()?;
                    left = Expr::Le(Box::new(left), Box::new(right));
                }
                Ok(Token::Gt) => {
                    self.advance();
                    let right = self.parse_shift()?;
                    left = Expr::Gt(Box::new(left), Box::new(right));
                }
                Ok(Token::Ge) => {
                    self.advance();
                    let right = self.parse_shift()?;
                    left = Expr::Ge(Box::new(left), Box::new(right));
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
                    left = Expr::Shl(Box::new(left), Box::new(right));
                }
                Ok(Token::Shr) => {
                    self.advance();
                    let right = self.parse_additive()?;
                    left = Expr::Shr(Box::new(left), Box::new(right));
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
                    left = Expr::Add(Box::new(left), Box::new(right));
                }
                Ok(Token::Minus) => {
                    self.advance();
                    let right = self.parse_multiplicative()?;
                    left = Expr::Sub(Box::new(left), Box::new(right));
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
                    left = Expr::Mul(Box::new(left), Box::new(right));
                }
                Ok(Token::Slash) => {
                    self.advance();
                    let right = self.parse_unary()?;
                    left = Expr::Div(Box::new(left), Box::new(right));
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
                    Ok(Expr::Not(Box::new(expr)))
                }
                Ok(Token::Minus) => {
                    self.advance();
                    let expr = self.parse_unary()?;
                    Ok(Expr::Neg(Box::new(expr)))
                }
                Ok(Token::Tilde) => {
                    self.advance();
                    let expr = self.parse_unary()?;
                    Ok(Expr::BitNot(Box::new(expr)))
                }
                Ok(Token::Ampersand) => {
                    self.advance();
                    if let Some(Ok(Token::Identifier(name))) = self.current_token() {
                        let name = name.clone();
                        self.advance();
                        self.parse_postfix_expr(Expr::OwnedRef(name))
                    } else {
                        self.spanned_err("Expected identifier after &".to_string())
                    }
                }
                Ok(Token::At) => {
                    self.advance();
                    if let Some(Ok(Token::Identifier(name))) = self.current_token() {
                        let name = name.clone();
                        self.advance();
                        self.parse_postfix_expr(Expr::PriorState(name))
                    } else {
                        self.spanned_err("Expected identifier after @".to_string())
                    }
                }
                _ => self.parse_postfix(),
            }
        } else {
            self.parse_postfix()
        }
    }

    fn parse_postfix_expr(&mut self, expr: Expr) -> Result<Expr, SyntaxError> {
        let mut expr = expr;
        loop {
            if let Some(Ok(Token::LBracket)) = self.current_token() {
                self.advance();
                let index = self.parse_expression()?;
                self.expect(Token::RBracket)?;
                expr = Expr::ListIndex(Box::new(expr), Box::new(index));
            } else if let Some(Ok(Token::Dot)) = self.current_token() {
                self.advance();
                let member_name = self.expect_identifier()?;
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
                    expr = Expr::Call(member_name, vec![expr]);
                } else {
                    expr = Expr::FieldAccess(Box::new(expr), member_name);
                }
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
                let index = self.parse_expression()?;
                self.expect(Token::RBracket)?;
                expr = Expr::ListIndex(Box::new(expr), Box::new(index));
            } else if let Some(Ok(Token::Dot)) = self.current_token() {
                self.advance();
                let member_name = self.expect_identifier()?;
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
                    expr = Expr::Call(member_name, vec![expr]);
                } else {
                    expr = Expr::FieldAccess(Box::new(expr), member_name);
                }
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, SyntaxError> {
        match self.current_token() {
            Some(Ok(Token::Integer(val))) => {
                let val = *val;
                self.advance();
                Ok(Expr::Integer(val))
            }
            Some(Ok(Token::Float(val))) => {
                let val = *val;
                self.advance();
                Ok(Expr::Float(val))
            }
            Some(Ok(Token::String(val))) => {
                let val = val.clone();
                self.advance();
                Ok(Expr::String(val))
            }
            Some(Ok(Token::BoolTrue)) => {
                self.advance();
                Ok(Expr::Bool(true))
            }
            Some(Ok(Token::BoolFalse)) => {
                self.advance();
                Ok(Expr::Bool(false))
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
            Some(Ok(Token::LBrace)) => {
                // Object literal: { field: value, ... }
                self.advance();
                let mut fields = Vec::new();
                if let Some(Ok(Token::RBrace)) = self.current_token() {
                    // Empty object
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
                Ok(Expr::ObjectLiteral(fields))
            }
            Some(Ok(Token::LParen)) => {
                self.advance();
                let expr = self.parse_expression()?;
                self.expect(Token::RParen)?;
                Ok(expr)
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
            Some(Ok(Token::TildeSlash)) => {
                self.advance();
                let identifier = self.expect_identifier()?;
                let path = format!("~/{}", identifier);
                Ok(Expr::String(path))
            }
            Some(tok) => self.spanned_err(format!("Unexpected token in expression: {:?}", tok)),
            None => self.spanned_err("Unexpected EOF in expression".to_string()),
        }
    }
}

#[cfg(test)]
mod parser_tests {
    use super::*;

    #[test]
    fn test_parse_rstruct_with_self_closing_html() {
        let s = r#"rstruct Logo { <svg> <circle /> </svg> };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse_rstruct();
        assert!(result.is_ok());
    }
}
