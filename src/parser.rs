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
    /// Track if we consumed a >> that should serve as > for parent generic level
    shr_consumed_as_gt: bool,
    strict_mode: StrictMode,
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
            shr_consumed_as_gt: false,
            strict_mode: StrictMode::Off,
        }
    }

    pub fn with_strict_mode(mut self, strict: bool) -> Self {
        self.strict_mode = if strict { StrictMode::Strict } else { StrictMode::Off };
        self
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
            Some(Ok(Token::Struct)) => { self.advance(); Ok("struct".to_string()) }
            Some(Ok(Token::Enum)) => { self.advance(); Ok("enum".to_string()) }
            Some(Ok(Token::Import)) => { self.advance(); Ok("import".to_string()) }
            Some(Ok(Token::Term)) => { self.advance(); Ok("term".to_string()) }
            Some(Ok(Token::Const)) => { self.advance(); Ok("const".to_string()) }
            Some(Ok(Token::BoolTrue)) => { self.advance(); Ok("true".to_string()) }
            Some(Ok(Token::BoolFalse)) => { self.advance(); Ok("false".to_string()) }
            Some(Ok(Token::Unification)) => { self.advance(); Ok("uni".to_string()) }
            Some(Ok(Token::Escape)) => { self.advance(); Ok("escape".to_string()) }
            Some(Ok(Token::Async)) => { self.advance(); Ok("async".to_string()) }
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

    fn parse_hashtag_modifiers(&mut self) -> Result<Vec<Hashtag>, SyntaxError> {
        let mut mods = Vec::new();
        loop {
            match self.current_token() {
                Some(Ok(Token::Hash)) => {
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
                    mods.push(Hashtag { name, value, mandatory: false, fallback: Vec::new(), scoped: None });
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
                    mods.push(Hashtag { name, value, mandatory: true, fallback, scoped: None });
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
                    for mut h in inner {
                        h.scoped = Some(scope.clone());
                        mods.push(h);
                    }
                }
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
                        found: format!("{:?}", self.current_token()),
                        span: self.current_span().unwrap_or_else(Span::dummy),
                    });
                }
            }
        }

        // Parse file-level attributes #![...] or #!pragma ... ]
        if matches!(self.current_token(), Some(Ok(Token::HashBangBracket)))
            || matches!(self.current_token(), Some(Ok(Token::PragmaBang)))
        {
            file_attrs = self.parse_attributes()?;
        }

        // Process FFI state from file attributes
        let ffi_state = Self::process_ffi_attributes(&file_attrs);

        while self.current_token().is_some() {
            items.push(self.parse_top_level()?);
        }
        Ok(Program {
            items,
            comments: self.comments.clone(),
            reactor_speed,
            attrs: file_attrs,
            ffi: ffi_state,
            strict_mode: self.strict_mode,
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

    fn parse_top_level(&mut self) -> Result<TopLevel, SyntaxError> {
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
                let mut state = self.parse_state_decl()?;
                state.attrs = attrs;
                Ok(TopLevel::StateDecl(state))
            }
            Some(Ok(Token::Const)) => {
                let constant = self.parse_constant()?;
                Ok(TopLevel::Constant(constant))
            }
            Some(Ok(Token::Txn)) | Some(Ok(Token::Rct)) | Some(Ok(Token::Async)) => {
                let mut txn = self.parse_transaction()?;
                txn.attrs = attrs;
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
                let frgn_binding = self.parse_frgn_binding()?;
                Ok(frgn_binding)
            }
            Some(Ok(Token::FrgnBang)) => {
                let frgn_binding = self.parse_frgn_binding()?;
                Ok(frgn_binding)
            }
            Some(Ok(Token::Syscall)) => {
                let frgn_binding = self.parse_frgn_binding()?;
                Ok(frgn_binding)
            }
            Some(Ok(Token::SyscallBang)) => {
                let frgn_binding = self.parse_frgn_binding()?;
                Ok(frgn_binding)
            }
            Some(Ok(Token::Resource)) | Some(Ok(Token::Rsrc)) | Some(Ok(Token::Registry)) => {
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
            "UInt" => Ok(Type::UInt),
            "Float" => Ok(Type::Float),
            "Bool" => Ok(Type::Bool),
            "void" => Ok(Type::Void),
            "Data" => Ok(Type::Data),
            // Shorthand sized types (syntactic sugar for Int/UInt @/xN)
            "u8" => Ok(Type::Constrained(Box::new(Type::UInt), BitRange::Any(8))),
            "i8" => Ok(Type::Constrained(Box::new(Type::Int), BitRange::Any(8))),
            "u16" => Ok(Type::Constrained(Box::new(Type::UInt), BitRange::Any(16))),
            "i16" => Ok(Type::Constrained(Box::new(Type::Int), BitRange::Any(16))),
            "u32" => Ok(Type::Constrained(Box::new(Type::UInt), BitRange::Any(32))),
            "i32" => Ok(Type::Constrained(Box::new(Type::Int), BitRange::Any(32))),
            "u64" => Ok(Type::Constrained(Box::new(Type::UInt), BitRange::Any(64))),
            "i64" => Ok(Type::Constrained(Box::new(Type::Int), BitRange::Any(64))),
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
            Some(Ok(Token::Err)) => { self.advance(); Ok("Err".to_string()) }
            other => self.spanned_err(format!("Expected type name, found {:?}", other)),
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

        // Handle both frgn and frgn! tokens
        let ffi_kind = match self.current_token() {
            Some(Ok(Token::Frgn)) => {
                self.advance();
                FfiKind::Frgn
            }
            Some(Ok(Token::FrgnBang)) => {
                self.advance();
                FfiKind::FrgnBang
            }
            _ => return self.spanned_err("Expected 'frgn' or 'frgn!'".to_string()),
        };

        let name = self.expect_identifier()?;

        // Parse optional @ address
        let address = if matches!(self.current_token(), Some(Ok(Token::At))) {
            self.advance();
            let addr = if let Some(Ok(Token::Integer(n))) = self.current_token() {
                *n as u64
            } else if let Some(Ok(Token::Identifier(name))) = self.current_token() {
                // Named address - resolve to actual address from FFI state
                // For now, store the name; resolution happens in type checking
                0 // TODO: resolve named address
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
        let success_output = if ffi_kind == FfiKind::FrgnBang {
            // Fire-and-forget: no return type expected
            Vec::new()
        } else {
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

            // Parse success type
            let mut success_output = Vec::new();
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
            success_output
        };

        self.expect(Token::Semicolon)?;

        let frgn_sig = ForeignSignature {
            name: name.clone(),
            location: String::new(),
            wasm_impl: None,
            wasm_setup: None,
            inputs,
            success_output,
            error_type_name: "Err".to_string(),
            error_fields: Vec::new(),
            input_layout: None,
            output_layout: None,
            precondition: None,
            postcondition: None,
            buffer_mode: None,
            result_type: ResultType::TrueAssertion,
            ffi_kind: Some(ffi_kind),
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

        // Struct variants: [discriminant] { +field, -field, field }
        let variants = self.parse_struct_variants()?;

        // Semicolon after struct is optional
        if let Some(Ok(Token::Semicolon)) = self.current_token() {
            self.advance();
        }
        Ok(StructDefinition {
            name,
            type_params,
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
                    additions.push(StructField { name, ty, default });
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
                Some(Ok(Token::Identifier(_))) => {
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
                    fields.push(StructField { name, ty, default });
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
                                attrs: Vec::new(),
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

    fn parse_alka_block(&mut self) -> Result<Statement, SyntaxError> {
        self.advance();
        let dangerous = if let Some(Ok(Token::Not)) = self.current_token() {
            self.advance();
            true
        } else {
            false
        };
        let lbrace_pos = if let Some((_, span)) = &self.current {
            if let Some(Ok(Token::LBrace)) = self.current_token() {
                span.start
            } else {
                return self.spanned_err("Expected { after alka".to_string());
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
        if brace_depth != 0 {
            return self.spanned_err("Unterminated alka block".to_string());
        }
        let content = self.source[lbrace_pos + 1..end_pos].trim().to_string();
        self.expect(Token::Semicolon)?;
        let span = self.current_span();
        Ok(Statement::Alka(AlkaBlock { dangerous, content, span }))
    }

    fn parse_block_pragma(&mut self) -> Result<Statement, SyntaxError> {
        // #identifier { body };
        self.advance();
        let name = self.expect_identifier()?;
        self.expect(Token::LBrace)?;
        let body = self.parse_body()?;
        self.expect(Token::RBrace)?;
        self.expect(Token::Semicolon)?;
        let span = self.current_span();
        Ok(Statement::OnExit { body, span })
    }

    fn parse_state_decl(&mut self) -> Result<StateDecl, SyntaxError> {
        self.expect(Token::Let)?;
        let name = self.expect_identifier()?;

        let mut address: Option<u64> = None;
        let mut bit_range: Option<BitRange> = None;
        let mut is_override = false;

        // Optional mapping before colon
        // Supports: @ address / bit-spec, @ / bit-spec, @ stack:offset, @ heap:offset, [bit-spec]
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
                                // Named address variable
                                address = Some(0); // TODO: resolve named address
                                self.advance();
                                if let Some(Ok(Token::Slash)) = self.current_token() {
                                    self.advance();
                                    bit_range = Some(self.parse_bit_range()?);
                                }
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
                            // Named address variable
                            address = Some(0); // TODO: resolve named address
                            self.advance();
                            if let Some(Ok(Token::Slash)) = self.current_token() {
                                self.advance();
                                bit_range = Some(self.parse_bit_range()?);
                            }
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
            _ => {
                return self.spanned_err("Expected #[, #![, #pragma, or #!pragma for attribute".to_string());
            }
        }
        
        // Parse comma-separated items
        // For #[...] / #![...]: items are inside brackets, terminated by ]
        // For #pragma.c: single item, no brackets (already handled above)
        // For #!pragma: items comma-separated, no brackets needed
        while !matches!(self.current_token(), Some(Ok(Token::RBracket))) {
            let attr = if is_pragma {
                self.parse_pragma_item(None)?
            } else {
                // #[...]: parse old-style item
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

                crate::ast::Attribute { target, key, value }
            };

            attrs.push(attr);

            // Expect comma or ]
            if matches!(self.current_token(), Some(Ok(Token::Comma))) {
                self.advance();
            } else {
                break;
            }
        }

        // For #[...] and #![...] syntax, expect closing bracket
        // For #pragma variants, consume optional closing bracket if present
        if !is_pragma {
            self.expect(Token::RBracket)?;
        } else if matches!(self.current_token(), Some(Ok(Token::RBracket))) {
            self.advance(); // consume optional ]
        }
        Ok(attrs)
    }

    fn parse_trigger(&mut self) -> Result<TriggerDeclaration, SyntaxError> {
        self.expect(Token::Trg)?;
        let name = self.expect_identifier()?;
        self.expect(Token::Colon)?;
        let ty = self.parse_type()?;

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
                            // Backward compat: @ identifier as link reference
                            address = crate::ast::LinkRef::Linked(name.clone());
                            self.advance();
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
            attrs: Vec::new(),
            modifiers: Vec::new(),
            variant_bodies,
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

        let (outputs, output_names, output_type, contract) =
            if let Some(Ok(Token::LBracket)) = self.current_token() {
                // Contract before arrow: defn name(params) [pre][post] -> Type
                let contract = self.parse_contract()?;
                if let Some(Ok(Token::Arrow)) = self.current_token() {
                    self.advance();
                    let (outputs, output_names) = self.parse_output_types_with_names(&parameters)?;
                    let output_type = if outputs.len() > 1 {
                        Some(crate::ast::OutputType::Tuple(outputs.clone()))
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
                    Some(crate::ast::OutputType::Tuple(outputs.clone()))
                } else {
                    None
                };
                let contract = if let Some(Ok(Token::LBracket)) = self.current_token() {
                    self.parse_contract()?
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
            modifiers: Vec::new(),
            variant_bodies,
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

        // Stop at hashtag tokens so caller can parse modifiers
        if matches!(self.current_token(), Some(Ok(Token::Hash | Token::HashBang))) {
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
                count = 2; // ~/ provides both pre and post
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

        // [true][n >= 0] is always an error — defeats contract-first programming
        if matches!(&pre_condition, Expr::Bool(true)) && matches!(&post_condition, Expr::Bool(true)) {
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
            if matches!(&pre_condition, Expr::Bool(true)) {
                return self.spanned_err(
                    "Strict mode: precondition [true] is not allowed - specify actual state requirements".to_string()
                );
            }
            if matches!(&post_condition, Expr::Bool(true)) {
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
                
                // Check for tuple destructuring: let (a, b) = expr;
                if let Some(Ok(Token::LParen)) = self.current_token() {
                    self.advance();
                    let mut names = Vec::new();
                    loop {
                        names.push(self.expect_identifier()?);
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
                    })
                } else {
                    let name = self.expect_identifier()?;

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
                        match &addr_expr {
                            Expr::Integer(n) => { address = Some(*n as u64); }
                            _ => { address_expr = Some(Box::new(addr_expr)); }
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
                            match &addr_expr {
                                Expr::Integer(n) => { address = Some(*n as u64); }
                                _ => { address_expr = Some(Box::new(addr_expr)); }
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
                    is_override,
                    modifiers,
                })
                }
            }
            Some(Ok(Token::Term)) => {
                self.advance();
                let outputs = self.parse_term_outputs()?;
                let modifiers = self.parse_hashtag_modifiers()?;
                self.expect(Token::Semicolon)?;
                Ok(Statement::Term { values: outputs, modifiers })
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
            Some(Ok(Token::Unification)) => {
                // Two syntaxes supported:
                // 1. uni pattern = expr; (current Brief style)
                // 2. uni variable(Pattern) = result; (Brief compiler library style)
                self.advance();
                
                // Get the first token - could be variable name or pattern
                let first = match self.current_token() {
                    Some(Ok(Token::Identifier(name))) => name.clone(),
                    Some(Ok(Token::TypeData)) => "Data".to_string(),
                    Some(Ok(Token::Ok)) => "Ok".to_string(),
                    Some(Ok(Token::Err)) => "Err".to_string(),
                    Some(Ok(Token::Some)) => "Some".to_string(),
                    Some(Ok(Token::None)) => "None".to_string(),
                    _ => return self.spanned_err("Expected pattern after uni".to_string()),
                };
                self.advance();
                
                // Check what follows: ( for library style or = for current style
                if let Some(Ok(Token::LParen)) = self.current_token() {
                    // Library style: uni variable(Pattern) = result;
                    // First token was the variable name, now parse the pattern
                    let var_name = first;
                    self.advance(); // consume (
                    
                    // Parse pattern - could be Variant or Variant(data) or just _
                    let pattern_name = match self.current_token() {
                        Some(Ok(Token::Underscore)) => {
                            self.advance();
                            // Simple wildcard pattern
                            let pattern = "_".to_string();
                            self.expect(Token::RParen)?;
                            self.expect(Token::Eq)?;
                            let expr = if let Some(Ok(Token::LBrace)) = self.current_token() {
                                self.advance();
                                let mut stmts = Vec::new();
                                loop {
                                    if let Some(Ok(Token::RBrace)) = self.current_token() {
                                        self.advance();
                                        break;
                                    }
                                    stmts.push(self.parse_statement()?);
                                }
                                Expr::Block(stmts, Box::new(Expr::Bool(true)))
                            } else {
                                self.parse_expression()?
                            };
                            self.expect(Token::Semicolon)?;
                            return Ok(Statement::Unification {
                                name: var_name,
                                pattern,
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
                        Some(Ok(Token::Let)) => "KeywordLet".to_string(),
                        Some(Ok(Token::Const)) => "KeywordConst".to_string(),
                        Some(Ok(Token::Txn)) => "KeywordTxn".to_string(),
                        Some(Ok(Token::Rct)) => "KeywordRct".to_string(),
                        Some(Ok(Token::Async)) => "KeywordAsync".to_string(),
                        Some(Ok(Token::Term)) => "KeywordTerm".to_string(),
                        Some(Ok(Token::Escape)) => "KeywordEscape".to_string(),
                        Some(Ok(Token::Defn)) => "KeywordDefn".to_string(),
                        Some(Ok(Token::Sig)) => "KeywordSig".to_string(),
                        Some(Ok(Token::Frgn)) => "KeywordFrgn".to_string(),
                        Some(Ok(Token::Struct)) => "KeywordStruct".to_string(),
                        Some(Ok(Token::Enum)) => "KeywordEnum".to_string(),
                        Some(Ok(Token::Import)) => "KeywordImport".to_string(),
                        Some(Ok(Token::From)) => "KeywordFrom".to_string(),
                        Some(Ok(Token::As)) => "KeywordAs".to_string(),
                        Some(Ok(Token::BoolTrue)) => "KeywordTrue".to_string(),
                        Some(Ok(Token::BoolFalse)) => "KeywordFalse".to_string(),
                        Some(Ok(Token::TypeFloat)) => "Float".to_string(),
                        Some(Ok(Token::TypeVoid)) => "Void".to_string(),
                        Some(Ok(Token::TypeUInt)) => "UInt".to_string(),
                        Some(Ok(Token::Rstruct)) => "KeywordRstruct".to_string(),
                        Some(Ok(Token::Registry)) => "KeywordRegistry".to_string(),
                        Some(Ok(Token::Trg)) => "KeywordTrg".to_string(),
                        Some(Ok(Token::TrgBang)) => "KeywordTrgBang".to_string(),
                        Some(Ok(Token::Syscall)) => "KeywordSyscall".to_string(),
                        Some(Ok(Token::Resource)) => "KeywordResource".to_string(),
                        Some(Ok(Token::Rsrc)) => "KeywordRsrc".to_string(),
                        Some(Ok(Token::Link)) => "KeywordLink".to_string(),
                        Some(Ok(Token::Asm)) => "KeywordAsm".to_string(),
                        Some(Ok(Token::Stage)) => "KeywordStage".to_string(),
                        Some(Ok(Token::On)) => "KeywordOn".to_string(),
                        Some(Ok(Token::Forall)) => "KeywordForall".to_string(),
                        Some(Ok(Token::Exists)) => "KeywordExists".to_string(),
                        Some(Ok(Token::Within)) => "KeywordWithin".to_string(),
                        Some(Ok(Token::Bank)) => "KeywordBank".to_string(),
                        Some(Ok(Token::Match)) => "KeywordMatch".to_string(),
                        Some(Ok(Token::Unification)) => "KeywordUnification".to_string(),
                        Some(Ok(Token::Render)) => "KeywordRender".to_string(),
                        _ => return self.spanned_err(format!("Expected pattern variant, found {:?}", self.current_token()).to_string()),
                    };
                    self.advance();
                    
                    // Check for pattern data: Variant(field1, field2, ...) or Variant(_) or just Variant
                    let pattern = if let Some(Ok(Token::LParen)) = self.current_token() {
                        self.advance();
                        let mut fields = Vec::new();
                        loop {
                            if let Some(Ok(Token::Identifier(name))) = self.current_token() {
                                fields.push(name.clone());
                                self.advance();
                            } else if let Some(Ok(Token::Underscore)) = self.current_token() {
                                fields.push("_".to_string());
                                self.advance();
                            } else {
                                break;
                            }
                            if let Some(Ok(Token::Comma)) = self.current_token() {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                        self.expect(Token::RParen)?;
                        format!("{}({})", pattern_name, fields.join(","))
                    } else {
                        pattern_name.clone()
                    };
                    
                    self.expect(Token::RParen)?;
                    self.expect(Token::Eq)?;
                    // Check for block-style result: { stmts... }
                    let expr = if let Some(Ok(Token::LBrace)) = self.current_token() {
                        self.advance();
                        let mut stmts = Vec::new();
                        loop {
                            if let Some(Ok(Token::RBrace)) = self.current_token() {
                                self.advance();
                                break;
                            }
                            stmts.push(self.parse_statement()?);
                        }
                        // Block needs a final expression - use a placeholder for now
                        Expr::Block(stmts, Box::new(Expr::Bool(true)))
                    } else {
                        self.parse_expression()?
                    };
                    self.expect(Token::Semicolon)?;
                    Ok(Statement::Unification {
                        name: var_name,
                        pattern,
                        expr,
                    })
                } else {
                    // Current Brief style: uni pattern = expr;
                    let pattern = if let Some(Ok(Token::LParen)) = self.current_token() {
                        self.advance();
                        let field = self.expect_identifier()?;
                        self.expect(Token::RParen)?;
                        format!("{}:{}", first, field)
                    } else {
                        first
                    };
                    
                    self.expect(Token::Eq)?;
                    let expr = self.parse_expression()?;
                    self.expect(Token::Semicolon)?;
                    Ok(Statement::Unification {
                        name: "uni".to_string(),
                        pattern,
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
            Some(Ok(Token::TrgBang)) => {
                self.advance();
                let name = self.expect_identifier()?;
                self.expect(Token::Colon)?;
                let ty = self.parse_type()?;
                let expr = if let Some(Ok(Token::Eq)) = self.current_token() {
                    self.advance();
                    Some(self.parse_expression()?)
                } else {
                    None
                };
                self.expect(Token::Semicolon)?;
                let span = self.current_span();
                Ok(Statement::LocalTrigger { name, ty, expr, span })
            }
            Some(Ok(Token::Trg)) => {
                self.spanned_err(
                    "Local triggers introduce asynchronous rollback risks. \
                     You must use 'trg!' or 'trigger!' to explicitly acknowledge this boundary. \
                     (Top-level trigger declarations use 'trg' without '!')".to_string(),
                )
            }
            Some(Ok(Token::Hash)) => {
                // Block pragma: #on_exit { ... };
                return self.parse_block_pragma();
            }
            _ => {
                // Check for alka block before parsing as expression
                if let Some(Ok(Token::Identifier(name))) = self.current_token() {
                    if name == "alka" || name == "ALKA" {
                        return self.parse_alka_block();
                    }
                }
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

                    match expr {
                        Expr::Call(name, args) => {
                            if args.len() == 1 {
                                if let Expr::Identifier(pattern) = &args[0] {
                                    self.expect(Token::Semicolon)?;
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
            Some(Ok(Token::TypeChar)) => {
                self.advance();
                Type::Char
            }
            // Shorthand sized integer types (syntactic sugar for Int/UInt @/xN)
            Some(Ok(Token::TypeU8)) => {
                self.advance();
                Type::Constrained(Box::new(Type::UInt), BitRange::Any(8))
            }
            Some(Ok(Token::TypeI8)) => {
                self.advance();
                Type::Constrained(Box::new(Type::Int), BitRange::Any(8))
            }
            Some(Ok(Token::TypeU16)) => {
                self.advance();
                Type::Constrained(Box::new(Type::UInt), BitRange::Any(16))
            }
            Some(Ok(Token::TypeI16)) => {
                self.advance();
                Type::Constrained(Box::new(Type::Int), BitRange::Any(16))
            }
            Some(Ok(Token::TypeU32)) => {
                self.advance();
                Type::Constrained(Box::new(Type::UInt), BitRange::Any(32))
            }
            Some(Ok(Token::TypeI32)) => {
                self.advance();
                Type::Constrained(Box::new(Type::Int), BitRange::Any(32))
            }
            Some(Ok(Token::TypeU64)) => {
                self.advance();
                Type::Constrained(Box::new(Type::UInt), BitRange::Any(64))
            }
            Some(Ok(Token::TypeI64)) => {
                self.advance();
                Type::Constrained(Box::new(Type::Int), BitRange::Any(64))
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
            Some(Ok(tok)) => return self.spanned_err(format!("Expected type, found {:?}", tok)),
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
            
            // Standard generic type parsing
            let mut type_args = Vec::new();
            loop {
                type_args.push(self.parse_type()?);
                // Check if child level consumed Shr as Gt
                if self.shr_consumed_as_gt {
                    // Child consumed >> which serves as our closing > too
                    self.shr_consumed_as_gt = false;
                    ty = Type::Applied(
                        match &ty {
                            Type::Custom(name) => name.clone(),
                            _ => return self.spanned_err("Generic type must have a base name".to_string()),
                        },
                        type_args,
                    );
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
            
            // Special handling for Vector<T, dim1, dim2, ...> syntax
            if let Type::Custom(name) = &ty {
                if name == "Vector" && type_args.len() >= 2 {
                    // First arg is element type, rest are dimensions
                    let inner = Box::new(type_args[0].clone());
                    let mut dimensions = Vec::new();
                    for arg in &type_args[1..] {
                        match arg {
                            Type::Custom(dim_name) => {
                                // Named dimension: name:size - but we need to parse this differently
                                // For now, treat as anonymous with size from a constant
                                return self.spanned_err("Named dimensions must be in 'name:size' format".to_string());
                            }
                            _ => {
                                // Extract size from type - should be an integer literal type
                                // For simplicity, we'll handle this in a helper
                                if let Some(size) = Self::extract_dimension_size(arg) {
                                    dimensions.push(crate::ast::Dimension::Anonymous(size));
                                } else {
                                    return self.spanned_err("Vector dimension must be an integer".to_string());
                                }
                            }
                        }
                    }
                    ty = Type::Vector(inner, dimensions);
                } else {
                    ty = Type::Applied(
                        match &ty {
                            Type::Custom(name) => name.clone(),
                            _ => return self.spanned_err("Generic type must have a base name".to_string()),
                        },
                        type_args,
                    );
                }
            } else {
                ty = Type::Applied(
                    match &ty {
                        Type::Custom(name) => name.clone(),
                        _ => return self.spanned_err("Generic type must have a base name".to_string()),
                    },
                    type_args,
                );
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
                Ok(Token::Percent) => {
                    self.advance();
                    let right = self.parse_unary()?;
                    left = Expr::Mod(Box::new(left), Box::new(right));
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
                // Check if this is a multidimensional slice by looking ahead for commas
                // before any `;`, `..`, `::`, or `]`
                let is_multi = self.peek_multidimensional_slice();
                if is_multi {
                    let result = self.parse_multi_slice()?;
                    expr = Expr::MultiSlice {
                        value: Box::new(expr),
                        coordinates: result.coordinates,
                        mask: result.mask,
                    };
                } else {
                    let result = self.parse_bracket_contents()?;
                    expr = self.bracket_contents_to_expr(expr, result);
                }
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
            Some(Ok(Token::Char(val))) => {
                let val = *val;
                self.advance();
                Ok(Expr::Char(val))
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
            Some(Ok(Token::Sig)) | Some(Ok(Token::Defn)) | Some(Ok(Token::Let)) | Some(Ok(Token::Txn)) | Some(Ok(Token::Rct)) | Some(Ok(Token::Frgn)) | Some(Ok(Token::Struct)) | Some(Ok(Token::Enum)) | Some(Ok(Token::Import)) | Some(Ok(Token::Term)) | Some(Ok(Token::Const)) | Some(Ok(Token::BoolTrue)) | Some(Ok(Token::BoolFalse)) | Some(Ok(Token::Unification)) | Some(Ok(Token::Escape)) | Some(Ok(Token::Async)) | Some(Ok(Token::Some)) | Some(Ok(Token::None)) | Some(Ok(Token::Ok)) | Some(Ok(Token::Err)) => {
                let name = match self.current_token() {
                    Some(Ok(Token::Sig)) => "sig".to_string(),
                    Some(Ok(Token::Defn)) => "defn".to_string(),
                    Some(Ok(Token::Let)) => "let".to_string(),
                    Some(Ok(Token::Txn)) => "txn".to_string(),
                    Some(Ok(Token::Rct)) => "rct".to_string(),
                    Some(Ok(Token::Frgn)) => "frgn".to_string(),
                    Some(Ok(Token::Struct)) => "struct".to_string(),
                    Some(Ok(Token::Enum)) => "enum".to_string(),
                    Some(Ok(Token::Import)) => "import".to_string(),
                    Some(Ok(Token::Term)) => "term".to_string(),
                    Some(Ok(Token::Const)) => "const".to_string(),
                    Some(Ok(Token::BoolTrue)) => "true".to_string(),
                    Some(Ok(Token::BoolFalse)) => "false".to_string(),
                    Some(Ok(Token::Unification)) => "uni".to_string(),
                    Some(Ok(Token::Escape)) => "escape".to_string(),
                    Some(Ok(Token::Async)) => "async".to_string(),
                    Some(Ok(Token::Some)) => "Some".to_string(),
                    Some(Ok(Token::None)) => "None".to_string(),
                    Some(Ok(Token::Ok)) => "Ok".to_string(),
                    Some(Ok(Token::Err)) => "Err".to_string(),
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
            Some(Ok(Token::Sig)) | Some(Ok(Token::Defn)) | Some(Ok(Token::Let)) | Some(Ok(Token::Txn)) | Some(Ok(Token::Rct)) | Some(Ok(Token::Frgn)) | Some(Ok(Token::Struct)) | Some(Ok(Token::Enum)) | Some(Ok(Token::Import)) | Some(Ok(Token::Term)) | Some(Ok(Token::Const)) | Some(Ok(Token::BoolTrue)) | Some(Ok(Token::BoolFalse)) | Some(Ok(Token::Unification)) | Some(Ok(Token::Escape)) | Some(Ok(Token::Async)) | Some(Ok(Token::Some)) | Some(Ok(Token::None)) | Some(Ok(Token::Ok)) | Some(Ok(Token::Err)) => {
                let name = match self.current_token() {
                    Some(Ok(Token::Sig)) => "sig".to_string(),
                    Some(Ok(Token::Defn)) => "defn".to_string(),
                    Some(Ok(Token::Let)) => "let".to_string(),
                    Some(Ok(Token::Txn)) => "txn".to_string(),
                    Some(Ok(Token::Rct)) => "rct".to_string(),
                    Some(Ok(Token::Frgn)) => "frgn".to_string(),
                    Some(Ok(Token::Struct)) => "struct".to_string(),
                    Some(Ok(Token::Enum)) => "enum".to_string(),
                    Some(Ok(Token::Import)) => "import".to_string(),
                    Some(Ok(Token::Term)) => "term".to_string(),
                    Some(Ok(Token::Const)) => "const".to_string(),
                    Some(Ok(Token::BoolTrue)) => "true".to_string(),
                    Some(Ok(Token::BoolFalse)) => "false".to_string(),
                    Some(Ok(Token::Unification)) => "uni".to_string(),
                    Some(Ok(Token::Escape)) => "escape".to_string(),
                    Some(Ok(Token::Async)) => "async".to_string(),
                    Some(Ok(Token::Some)) => "Some".to_string(),
                    Some(Ok(Token::None)) => "None".to_string(),
                    Some(Ok(Token::Ok)) => "Ok".to_string(),
                    Some(Ok(Token::Err)) => "Err".to_string(),
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
            Some(Ok(Token::Some)) | Some(Ok(Token::None)) | Some(Ok(Token::Ok)) | Some(Ok(Token::Err)) => {
                let name = match self.current_token() {
                    Some(Ok(Token::Some)) => "Some".to_string(),
                    Some(Ok(Token::None)) => "None".to_string(),
                    Some(Ok(Token::Ok)) => "Ok".to_string(),
                    Some(Ok(Token::Err)) => "Err".to_string(),
                    _ => unreachable!(),
                };
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
                    Ok(Expr::Call(name, args))
                } else {
                    Ok(Expr::Identifier(name))
                }
            }
            Some(Ok(Token::Sig)) | Some(Ok(Token::Defn)) | Some(Ok(Token::Let)) | Some(Ok(Token::Txn)) | Some(Ok(Token::Rct)) | Some(Ok(Token::Frgn)) | Some(Ok(Token::Struct)) | Some(Ok(Token::Enum)) | Some(Ok(Token::Import)) | Some(Ok(Token::Term)) | Some(Ok(Token::Const)) | Some(Ok(Token::BoolTrue)) | Some(Ok(Token::BoolFalse)) | Some(Ok(Token::Unification)) | Some(Ok(Token::Escape)) | Some(Ok(Token::Async)) | Some(Ok(Token::Some)) | Some(Ok(Token::None)) | Some(Ok(Token::Ok)) | Some(Ok(Token::Err)) => {
                let name = match self.current_token() {
                    Some(Ok(Token::Sig)) => "sig".to_string(),
                    Some(Ok(Token::Defn)) => "defn".to_string(),
                    Some(Ok(Token::Let)) => "let".to_string(),
                    Some(Ok(Token::Txn)) => "txn".to_string(),
                    Some(Ok(Token::Rct)) => "rct".to_string(),
                    Some(Ok(Token::Frgn)) => "frgn".to_string(),
                    Some(Ok(Token::Struct)) => "struct".to_string(),
                    Some(Ok(Token::Enum)) => "enum".to_string(),
                    Some(Ok(Token::Import)) => "import".to_string(),
                    Some(Ok(Token::Term)) => "term".to_string(),
                    Some(Ok(Token::Const)) => "const".to_string(),
                    Some(Ok(Token::BoolTrue)) => "true".to_string(),
                    Some(Ok(Token::BoolFalse)) => "false".to_string(),
                    Some(Ok(Token::Unification)) => "uni".to_string(),
                    Some(Ok(Token::Escape)) => "escape".to_string(),
                    Some(Ok(Token::Async)) => "async".to_string(),
                    Some(Ok(Token::Some)) => "Some".to_string(),
                    Some(Ok(Token::None)) => "None".to_string(),
                    Some(Ok(Token::Ok)) => "Ok".to_string(),
                    Some(Ok(Token::Err)) => "Err".to_string(),
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
            Some(Ok(Token::Sig)) | Some(Ok(Token::Defn)) | Some(Ok(Token::Let)) | Some(Ok(Token::Txn)) | Some(Ok(Token::Rct)) | Some(Ok(Token::Frgn)) | Some(Ok(Token::Struct)) | Some(Ok(Token::Enum)) | Some(Ok(Token::Import)) | Some(Ok(Token::Term)) | Some(Ok(Token::Const)) | Some(Ok(Token::BoolTrue)) | Some(Ok(Token::BoolFalse)) | Some(Ok(Token::Unification)) | Some(Ok(Token::Escape)) | Some(Ok(Token::Async)) | Some(Ok(Token::Some)) | Some(Ok(Token::None)) | Some(Ok(Token::Ok)) | Some(Ok(Token::Err)) => {
                let name = match self.current_token() {
                    Some(Ok(Token::Sig)) => "sig".to_string(),
                    Some(Ok(Token::Defn)) => "defn".to_string(),
                    Some(Ok(Token::Let)) => "let".to_string(),
                    Some(Ok(Token::Txn)) => "txn".to_string(),
                    Some(Ok(Token::Rct)) => "rct".to_string(),
                    Some(Ok(Token::Frgn)) => "frgn".to_string(),
                    Some(Ok(Token::Struct)) => "struct".to_string(),
                    Some(Ok(Token::Enum)) => "enum".to_string(),
                    Some(Ok(Token::Import)) => "import".to_string(),
                    Some(Ok(Token::Term)) => "term".to_string(),
                    Some(Ok(Token::Const)) => "const".to_string(),
                    Some(Ok(Token::BoolTrue)) => "true".to_string(),
                    Some(Ok(Token::BoolFalse)) => "false".to_string(),
                    Some(Ok(Token::Unification)) => "uni".to_string(),
                    Some(Ok(Token::Escape)) => "escape".to_string(),
                    Some(Ok(Token::Async)) => "async".to_string(),
                    Some(Ok(Token::Some)) => "Some".to_string(),
                    Some(Ok(Token::None)) => "None".to_string(),
                    Some(Ok(Token::Ok)) => "Ok".to_string(),
                    Some(Ok(Token::Err)) => "Err".to_string(),
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
            Some(Ok(Token::Some)) | Some(Ok(Token::None)) | Some(Ok(Token::Ok)) | Some(Ok(Token::Err)) => {
                let name = match self.current_token() {
                    Some(Ok(Token::Some)) => "Some".to_string(),
                    Some(Ok(Token::None)) => "None".to_string(),
                    Some(Ok(Token::Ok)) => "Ok".to_string(),
                    Some(Ok(Token::Err)) => "Err".to_string(),
                    _ => unreachable!(),
                };
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
    fn extract_dimension_size(ty: &Type) -> Option<usize> {
        match ty {
            Type::Custom(s) => s.parse::<usize>().ok(),
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
        
        // If starts with `]`, `;`, `..`, `::`, it's not multidimensional
        if bytes[pos] == b']' || bytes[pos] == b';' {
            return false;
        }
        if pos + 1 < bytes.len() && bytes[pos] == b'.' && bytes[pos + 1] == b'.' {
            return false;
        }
        if pos + 1 < bytes.len() && bytes[pos] == b':' && bytes[pos + 1] == b':' {
            return false;
        }
        
        // Scan until we find a comma, semicolon, or closing bracket
        let mut found_colon = false;
        while pos < bytes.len() {
            match bytes[pos] {
                b',' => return true,  // Found comma = multidimensional
                b';' | b']' => return false,  // Found semicolon or bracket = single dimension
                b'.' => {
                    if pos + 1 < bytes.len() && bytes[pos + 1] == b'.' {
                        return false;  // Found ..
                    }
                }
                b':' => {
                    if found_colon {
                        // :: means stride, not multidimensional
                        return false;
                    }
                    found_colon = true;
                }
                _ => {}
            }
            pos += 1;
        }
        false
    }

    /// Parse a multidimensional slice: vec[coord1, coord2, ...; mask]
    fn parse_multi_slice(&mut self) -> Result<MultiSliceResult, SyntaxError> {
        let mut coordinates = Vec::new();
        let mut mask: Option<Box<Expr>> = None;
        
        loop {
            // Parse a single coordinate
            let coord = self.parse_slice_coordinate()?;
            coordinates.push(coord);
            
            // Check what comes next
            if let Some(Ok(Token::Comma)) = self.current_token() {
                self.advance(); // consume comma, continue to next coordinate
            } else if let Some(Ok(Token::Semicolon)) = self.current_token() {
                self.advance(); // consume semicolon
                mask = Some(Box::new(self.parse_expression()?));
                break;
            } else if let Some(Ok(Token::RBracket)) = self.current_token() {
                break;
            } else {
                return self.spanned_err("Expected ',', ';', or ']' in multidimensional slice".to_string());
            }
        }
        
        self.expect(Token::RBracket)?;
        
        Ok(MultiSliceResult { coordinates, mask })
    }

    /// Parse a single slice coordinate: index, range, or named
    fn parse_slice_coordinate(&mut self) -> Result<crate::ast::SliceCoordinate, SyntaxError> {
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
}

struct MultiSliceResult {
    coordinates: Vec<crate::ast::SliceCoordinate>,
    mask: Option<Box<Expr>>,
}

#[cfg(test)]
mod parser_tests {
    use super::*;

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
            assert!(matches!(&decl.ty, Type::Constrained(inner, BitRange::Any(8)) if matches!(**inner, Type::UInt)));
        }

        // Test i16 shorthand
        let s = r#"let y: i16 = 0;"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse i16 type");
        if let TopLevel::StateDecl(decl) = &result.unwrap().items[0] {
            assert!(matches!(&decl.ty, Type::Constrained(inner, BitRange::Any(16)) if matches!(**inner, Type::Int)));
        }

        // Test u32 shorthand
        let s = r#"let z: u32 = 0;"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse u32 type");
        if let TopLevel::StateDecl(decl) = &result.unwrap().items[0] {
            assert!(matches!(&decl.ty, Type::Constrained(inner, BitRange::Any(32)) if matches!(**inner, Type::UInt)));
        }

        // Test i64 shorthand
        let s = r#"let w: i64 = 0;"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse i64 type");
        if let TopLevel::StateDecl(decl) = &result.unwrap().items[0] {
            assert!(matches!(&decl.ty, Type::Constrained(inner, BitRange::Any(64)) if matches!(**inner, Type::Int)));
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
    fn test_parse_local_trigger_bang() {
        // Test trg! inside transaction
        let s = r#"txn Foo [true][n >= 0] { trg! resp: Int = fetch(); term; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse trg! inside transaction: {:?}", result.err());
        if let TopLevel::Transaction(txn) = &result.unwrap().items[0] {
            assert_eq!(txn.body.len(), 2);
            match &txn.body[0] {
                Statement::LocalTrigger { name, ty, expr, .. } => {
                    assert_eq!(name, "resp");
                    assert!(matches!(ty, Type::Int));
                    assert!(expr.is_some());
                }
                _ => panic!("Expected LocalTrigger statement"),
            }
        } else {
            panic!("Expected Transaction item");
        }
    }

    #[test]
    fn test_parse_local_trigger_bang_aliases() {
        // Test trigger! alias
        let s = r#"txn Foo [true][n >= 0] { trigger! resp: Bool = check(); term; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse trigger! inside transaction: {:?}", result.err());

        // Test TRG! uppercase
        let s = r#"txn Foo [true][n >= 0] { TRG! resp: UInt = read(); term; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse TRG! inside transaction: {:?}", result.err());

        // Test TRIGGER! uppercase
        let s = r#"txn Foo [true][n >= 0] { TRIGGER! resp: String = get(); term; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse TRIGGER! inside transaction: {:?}", result.err());
    }

    #[test]
    fn test_parse_local_trigger_without_bang_errors() {
        // Test that plain trg inside transaction gives helpful error
        let s = r#"txn Foo [true][n >= 0] { trg resp: Int = fetch(); term; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_err(), "Should error on plain trg inside transaction");
        if let Err(e) = result {
            let msg = format!("{}", e);
            assert!(msg.contains("trg!"), "Error should mention trg!: {}", msg);
            assert!(msg.contains("rollback"), "Error should mention rollback risk: {}", msg);
        }
    }

    #[test]
    fn test_parse_top_level_trigger_without_bang() {
        // Test that top-level trg (without !) still works
        let s = r#"trg button: Bool @ 0x1000;"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Top-level trg without ! should parse: {:?}", result.err());
        if let TopLevel::Trigger(trg) = &result.unwrap().items[0] {
            assert_eq!(trg.name, "button");
        } else {
            panic!("Expected Trigger item");
        }
    }

    #[test]
    fn test_parse_alka_block_safe() {
        let s = r#"txn Foo [true][n >= 0] { alka { FENCE GPU_MAIN.METAPAGE == 1; }; term; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse alka block: {:?}", result.err());
        if let TopLevel::Transaction(txn) = &result.unwrap().items[0] {
            match &txn.body[0] {
                Statement::Alka(block) => {
                    assert!(!block.dangerous, "Safe alka");
                    assert!(block.content.contains("FENCE GPU_MAIN"));
                }
                _ => panic!("Expected Alka statement"),
            }
        }
    }

    #[test]
    fn test_parse_alka_block_dangerous() {
        let s = r#"txn Foo [true][n >= 0] { alka! { PULSE DOORBELL @ 0x90; }; term; };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse alka! block: {:?}", result.err());
        if let TopLevel::Transaction(txn) = &result.unwrap().items[0] {
            match &txn.body[0] {
                Statement::Alka(block) => {
                    assert!(block.dangerous, "Dangerous alka");
                    assert!(block.content.contains("PULSE DOORBELL"));
                }
                _ => panic!("Expected Alka statement"),
            }
        }
    }

    #[test]
    fn test_parse_alka_multi_line() {
        let s = "txn Foo [true][n >= 0] { alka {\n  FENCE GPU_MAIN.METAPAGE == 1;\n  SIGNAL EXPERT_READY;\n}; term; };";
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse multi-line alka: {:?}", result.err());
    }

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
                    assert!(modifiers[0].mandatory);
                    assert_eq!(modifiers[0].fallback.len(), 2);
                    assert_eq!(modifiers[0].fallback[0], "lfence");
                    assert_eq!(modifiers[0].fallback[1], "mfence");
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
                    assert_eq!(modifiers[0].value.as_deref(), Some("4096"));
                }
                _ => panic!("Expected Let"),
            }
        }
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

    #[test]
    fn test_on_exit_block_pragma() {
        let s = r#"txn Foo [true][n >= 0] {
            &CLAIMED = true;
            #on_exit {
                &CLAIMED = false;
            };
            dma_work();
        };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse #on_exit block: {:?}", result.err());
        if let Ok(program) = result {
            if let TopLevel::Transaction(txn) = &program.items[0] {
                let has_on_exit = txn.body.iter().any(|s| matches!(s, Statement::OnExit { .. }));
                assert!(has_on_exit, "Should contain OnExit statement");
            } else {
                panic!("Expected Transaction");
            }
        }
    }

    #[test]
    fn test_on_exit_no_precondition() {
        let s = r#"txn Foo [true][n >= 0] {
            #on_exit { &x = 0; };
            term;
        };"#;
        let mut parser = Parser::new(s);
        let result = parser.parse();
        assert!(result.is_ok(), "Should parse on_exit without pre: {:?}", result.err());
    }

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
