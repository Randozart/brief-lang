use crate::ast::*;
use crate::lexer::Token;
use logos::{Lexer, Logos, Span};

pub struct Parser<'a> {
    lexer: Lexer<'a, Token>,
    current: Option<(Result<Token, ()>, Span)>,
    peek: Option<(Result<Token, ()>, Span)>,
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
            current,
            peek,
            comments: Vec::new(),
            current_line: 1,
        }
    }

    fn advance(&mut self) {
        // Check for comments before advancing
        if let Some((Ok(Token::Comment(text)), span)) = &self.current {
            self.comments.push(Comment {
                line: span.start,
                text: text.clone(),
            });
        }
        
        self.current = self.peek.take();
        self.peek = self.lexer.next().map(|token| (token, self.lexer.span()));
        
        // Update line number
        if let Some((_, span)) = &self.current {
            self.current_line = span.start;
        }
    }

    fn current_token(&self) -> Option<&Result<Token, ()>> {
        self.current.as_ref().map(|(t, _)| t)
    }

    fn expect(&mut self, expected: Token) -> Result<(), String> {
        match self.current_token() {
            Some(Ok(tok)) if *tok == expected => {
                self.advance();
                Ok(())
            }
            Some(Ok(tok)) => Err(format!("Expected {:?}, found {:?}", expected, tok)),
            Some(Err(_)) => Err("Lexer error".to_string()),
            None => Err(format!("Expected {:?}, found EOF", expected)),
        }
    }

    fn expect_identifier(&mut self) -> Result<String, String> {
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
            Some(Ok(tok)) => Err(format!("Expected identifier, found {:?}", tok)),
            Some(Err(_)) => Err("Lexer error".to_string()),
            None => Err("Expected identifier, found EOF".to_string()),
        }
    }

    pub fn parse(&mut self) -> Result<Program, String> {
        let mut items = Vec::new();
        while self.current_token().is_some() {
            items.push(self.parse_top_level()?);
        }
        // Collect any trailing comments
        while let Some((Ok(Token::Comment(text)), span)) = &self.current {
            self.comments.push(Comment {
                line: span.start,
                text: text.clone(),
            });
            self.advance();
        }
        Ok(Program { items, comments: self.comments.clone() })
    }

    fn parse_top_level(&mut self) -> Result<TopLevel, String> {
        // Skip standalone comments at top level
        while let Some(Ok(Token::Comment(_))) = self.current_token() {
            self.advance();
        }
        
        if self.current_token().is_none() {
            return Err("Unexpected EOF".to_string());
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
            Some(Ok(Token::Frgn)) => {
                let frgn_sig = self.parse_frgn_sig()?;
                Ok(TopLevel::ForeignSig(frgn_sig))
            }
            Some(Ok(Token::Comment(_))) => {
                self.advance();
                self.parse_top_level()
            }
            Some(Ok(tok)) => Err(format!("Unexpected token at top level: {:?}", tok)),
            Some(Err(_)) => Err("Lexer error at top level".to_string()),
            None => Err("Unexpected EOF".to_string()),
        }
    }

    fn parse_import(&mut self) -> Result<Import, String> {
        self.expect(Token::Import)?;
        
        let items = if let Some(Ok(Token::LBrace)) = self.current_token() {
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

    fn parse_signature(&mut self) -> Result<Signature, String> {
        self.expect(Token::Sig)?;
        let name = self.expect_identifier()?;
        self.expect(Token::Colon)?;
        let input_type = self.parse_type()?;
        self.expect(Token::Arrow)?;
        
        let result_type = self.parse_result_type()?;
        
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
        })
    }

    fn parse_frgn_sig(&mut self) -> Result<ForeignSig, String> {
        self.expect(Token::Frgn)?;
        self.expect(Token::Sig)?;
        let name = self.expect_identifier()?;
        
        let parameters = if let Some(Ok(Token::LParen)) = self.current_token() {
            self.advance();
            let mut params = Vec::new();
            while let Some(Ok(Token::Identifier(_))) = self.current_token() {
                let _param_name = self.expect_identifier()?;
                self.expect(Token::Colon)?;
                let param_type = self.parse_type()?;
                params.push(param_type);
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
        
        self.expect(Token::Arrow)?;
        let outputs = self.parse_output_types()?;
        self.expect(Token::Semicolon)?;
        
        Ok(ForeignSig {
            name,
            input_types: parameters,
            outputs,
        })
    }

    fn parse_state_decl(&mut self) -> Result<StateDecl, String> {
        self.expect(Token::Let)?;
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
        Ok(StateDecl { name, ty, expr })
    }

    fn parse_constant(&mut self) -> Result<Constant, String> {
        self.expect(Token::Const)?;
        let name = self.expect_identifier()?;
        self.expect(Token::Colon)?;
        let ty = self.parse_type()?;
        self.expect(Token::Eq)?;
        let expr = self.parse_expression()?;
        self.expect(Token::Semicolon)?;
        Ok(Constant { name, ty, expr })
    }

    fn parse_transaction(&mut self) -> Result<Transaction, String> {
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
        let contract = self.parse_contract()?;
        self.expect(Token::LBrace)?;
        let body = self.parse_body()?;
        self.expect(Token::RBrace)?;
        self.expect(Token::Semicolon)?;

        Ok(Transaction {
            is_async,
            is_reactive,
            name,
            contract,
            body,
        })
    }

    fn parse_definition(&mut self) -> Result<Definition, String> {
        self.expect(Token::Defn)?;
        let name = self.expect_identifier()?;

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

        let contract = self.parse_contract()?;
        
        let outputs = if let Some(Ok(Token::Arrow)) = self.current_token() {
            self.advance();
            self.parse_output_types()?
        } else {
            Vec::new()
        };

        self.expect(Token::LBrace)?;
        let body = self.parse_body()?;
        self.expect(Token::RBrace)?;
        self.expect(Token::Semicolon)?;

        Ok(Definition {
            name,
            parameters,
            outputs,
            contract,
            body,
        })
    }
    
    fn parse_output_types(&mut self) -> Result<Vec<Type>, String> {
        let mut outputs = Vec::new();
        outputs.push(self.parse_type()?);
        while let Some(Ok(Token::Comma)) = self.current_token() {
            self.advance();
            outputs.push(self.parse_type()?);
        }
        Ok(outputs)
    }
    
    fn parse_result_type(&mut self) -> Result<ResultType, String> {
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
    
    fn parse_term_outputs(&mut self) -> Result<Vec<Option<Expr>>, String> {
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

    fn parse_contract(&mut self) -> Result<Contract, String> {
        // Contract is enclosed in [].
        // Sample: [~/data] -> expands to [~data][data]
        // Sample: [data != null][count == @count + 1]
        // ast::Contract has pre_condition and post_condition.

        let mut pre_condition = Expr::Bool(true);
        let mut post_condition = Expr::Bool(true);

        let mut count = 0;
        while let Some(Ok(Token::LBracket)) = self.current_token() {
            self.advance();

            // Check for ~/ syntax - this is a shorthand for [~identifier][identifier]
            if let Some(Ok(Token::TildeSlash)) = self.current_token() {
                self.advance(); // Consume ~/
                let identifier = self.expect_identifier()?;

                // For ~/identifier, we need to generate two conditions:
                // pre_condition = ~identifier (logical NOT)
                // post_condition = identifier
                // Note: We use Expr::Not for logical NOT, even though ~ is bitwise NOT in expressions
                // This is because the spec uses ~/ as a shorthand for logical NOT in contracts
                if count == 0 {
                    // This is the first bracket, and it's ~/identifier
                    // So we set pre_condition = ~identifier and post_condition = identifier
                    pre_condition = Expr::Not(Box::new(Expr::Identifier(identifier.clone())));
                    post_condition = Expr::Identifier(identifier);
                    count = 2; // Mark that we've processed both conditions
                } else {
                    return Err("Unexpected ~/ in non-first contract bracket".to_string());
                }
            } else {
                let cond = self.parse_expression()?;
                if count == 0 {
                    pre_condition = cond;
                } else if count == 1 {
                    post_condition = cond;
                }
                count += 1;
            }

            self.expect(Token::RBracket)?;
        }

        // After processing brackets, ensure we have both conditions
        // If count == 1, it means we only saw one bracket pair without ~/ shorthand
        // This is invalid for a full contract (needs both pre and post)
        // But for type bounds (like Int[expr]), this is handled in parse_type
        // So we just return what we have

        Ok(Contract {
            pre_condition,
            post_condition,
        })
    }

    fn parse_body(&mut self) -> Result<Vec<Statement>, String> {
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

    fn parse_statement(&mut self) -> Result<Statement, String> {
        match self.current_token() {
            Some(Ok(Token::Let)) => {
                self.advance();
                let name = self.expect_identifier()?;
                let ty = if let Some(Ok(Token::Colon)) = self.current_token() {
                    self.advance();
                    Some(self.parse_type()?)
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
                Ok(Statement::Let { name, ty, expr })
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
                // Guarded statement: [condition] statement
                self.advance(); // consume [
                let condition = self.parse_expression()?;
                self.expect(Token::RBracket)?;
                let statement = self.parse_statement()?;
                Ok(Statement::Guarded {
                    condition,
                    statement: Box::new(statement),
                })
            }
            _ => {
                // Expression statement or Assignment/Unification
                let expr = self.parse_expression()?;

                if let Some(Ok(Token::Eq)) = self.current_token() {
                    self.advance();
                    let right = self.parse_expression()?;
                    self.expect(Token::Semicolon)?;

                    match expr {
                        Expr::Identifier(name) => Ok(Statement::Assignment {
                            is_owned: false,
                            name,
                            expr: right,
                        }),
                        Expr::OwnedRef(name) => Ok(Statement::Assignment {
                            is_owned: true,
                            name,
                            expr: right,
                        }),
                        Expr::Call(name, args) => {
                            if args.len() == 1 {
                                if let Expr::Identifier(pattern) = &args[0] {
                                    Ok(Statement::Unification {
                                        name,
                                        pattern: pattern.clone(),
                                        expr: right,
                                    })
                                } else {
                                    Err("Unification pattern must be an identifier".to_string())
                                }
                            } else {
                                Err("Unification expects one pattern argument".to_string())
                            }
                        }
                        _ => Err("Invalid left-hand side in assignment".to_string()),
                    }
                } else {
                    self.expect(Token::Semicolon)?;
                    Ok(Statement::Expression(expr))
                }
            }
        }
    }

    fn parse_type(&mut self) -> Result<Type, String> {
        let mut ty = match self.current_token() {
            Some(Ok(Token::Identifier(name))) => {
                let name = name.clone();
                self.advance();
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
            Some(Ok(tok)) => return Err(format!("Expected type, found {:?}", tok)),
            Some(Err(_)) => return Err("Lexer error".to_string()),
            None => return Err("Expected type, found EOF".to_string()),
        };

        // Check for contract bound: Type[Expr]
        if let Some(Ok(Token::LBracket)) = self.current_token() {
            self.advance();
            let contract = self.parse_expression()?;
            self.expect(Token::RBracket)?;
            ty = Type::ContractBound(Box::new(ty), Box::new(contract));
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

    fn parse_expression(&mut self) -> Result<Expr, String> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_and()?;
        while let Some(Ok(Token::OrOr)) = self.current_token() {
            self.advance();
            let right = self.parse_and()?;
            left = Expr::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_equality()?;
        while let Some(Ok(Token::AndAnd)) = self.current_token() {
            self.advance();
            let right = self.parse_equality()?;
            left = Expr::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> Result<Expr, String> {
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

    fn parse_comparison(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_additive()?;
        while let Some(token) = self.current_token() {
            match token {
                Ok(Token::Lt) => {
                    self.advance();
                    let right = self.parse_additive()?;
                    left = Expr::Lt(Box::new(left), Box::new(right));
                }
                Ok(Token::Le) => {
                    self.advance();
                    let right = self.parse_additive()?;
                    left = Expr::Le(Box::new(left), Box::new(right));
                }
                Ok(Token::Gt) => {
                    self.advance();
                    let right = self.parse_additive()?;
                    left = Expr::Gt(Box::new(left), Box::new(right));
                }
                Ok(Token::Ge) => {
                    self.advance();
                    let right = self.parse_additive()?;
                    left = Expr::Ge(Box::new(left), Box::new(right));
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<Expr, String> {
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

    fn parse_multiplicative(&mut self) -> Result<Expr, String> {
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

    fn parse_unary(&mut self) -> Result<Expr, String> {
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
                    // OwnedRef
                    if let Some(Ok(Token::Identifier(name))) = self.current_token() {
                        let name = name.clone();
                        self.advance();
                        Ok(Expr::OwnedRef(name))
                    } else {
                        Err("Expected identifier after &".to_string())
                    }
                }
                Ok(Token::At) => {
                    self.advance();
                    // PriorState
                    if let Some(Ok(Token::Identifier(name))) = self.current_token() {
                        let name = name.clone();
                        self.advance();
                        Ok(Expr::PriorState(name))
                    } else {
                        Err("Expected identifier after @".to_string())
                    }
                }
                _ => self.parse_primary(),
            }
        } else {
            self.parse_primary()
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
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
                // Check if it's a function call
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
            Some(Ok(Token::LParen)) => {
                self.advance();
                let expr = self.parse_expression()?;
                self.expect(Token::RParen)?;
                Ok(expr)
            }
            Some(Ok(Token::TildeSlash)) => {
                // This should be handled in parse_contract or type context.
                // If we see it here, it's likely an error in expression context.
                // But maybe `~/path` is an expression?
                // Lexer tokenizes `~/` and `path`.
                // If we are here, `TildeSlash` is consumed.
                // We need to consume the next identifier.
                self.advance();
                let identifier = self.expect_identifier()?;
                let path = format!("~/{}", identifier);
                Ok(Expr::String(path))
            }
            Some(tok) => Err(format!("Unexpected token in expression: {:?}", tok)),
            None => Err("Unexpected EOF in expression".to_string()),
        }
    }
}
