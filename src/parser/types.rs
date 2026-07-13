// ── Type Parser ────────────────────────────────────────────────────────
// 2026-07-12: Phase 1.4 — Parse Brief type annotations.
// Flat code: each function is max 2 levels of nesting.

use super::helpers::Parser;
use crate::ast::Type;
use crate::errors::SyntaxError;
use crate::lexer::Token;

impl<'a> Parser<'a> {
    /// Parse a type annotation.
    pub fn parse_type(&mut self) -> Result<Type, SyntaxError> {
        match self.peek() {
            Some(Token::TypeInt) => {
                self.pos += 1;
                Ok(Type::int())
            }
            Some(Token::TypeUInt) => {
                self.pos += 1;
                Ok(Type::Custom("UInt".into()))
            }
            Some(Token::TypeFloat) => {
                self.pos += 1;
                Ok(Type::float())
            }
            Some(Token::TypeString) => {
                self.pos += 1;
                Ok(Type::string())
            }
            Some(Token::TypeBool) => {
                self.pos += 1;
                Ok(Type::bool_())
            }
            Some(Token::TypeVoid) => {
                self.pos += 1;
                Ok(Type::void())
            }
            Some(Token::TypeChar) => {
                self.pos += 1;
                Ok(Type::char_())
            }
            Some(Token::TypeData) => {
                self.pos += 1;
                Ok(Type::data())
            }
            Some(Token::Identifier(name)) => self.parse_named_type(name.clone()),
            Some(Token::LParen) => self.parse_tuple_type(),
            _ => self.error_at_current("expected type"),
        }
    }

    /// Parse a named type, possibly with generic parameters or pointer prefix.
    fn parse_named_type(&mut self, name: String) -> Result<Type, SyntaxError> {
        self.pos += 1; // consume identifier

        // Ptr<T> handling
        if name == "Ptr" || name == "Ptr!" {
            if self.eat(&Token::Lt) {
                let inner = self.parse_type()?;
                self.expect(Token::Gt)?;
                return Ok(Type::ptr(inner));
            }
            return Ok(Type::ptr(Type::bits(1)));
        }

        // Generic type: Foo<T, U>
        if self.eat(&Token::Lt) {
            let mut args = Vec::new();
            loop {
                args.push(self.parse_type()?);
                if !self.eat(&Token::Comma) {
                    break;
                }
            }
            self.expect(Token::Gt)?;
            return Ok(Type::Applied(name, args));
        }

        Ok(Type::Custom(name))
    }

    /// Parse a tuple type: (Int, String)
    fn parse_tuple_type(&mut self) -> Result<Type, SyntaxError> {
        self.pos += 1; // consume LParen
        let mut types = Vec::new();
        if !self.check(&Token::RParen) {
            loop {
                types.push(self.parse_type()?);
                if !self.eat(&Token::Comma) {
                    break;
                }
            }
        }
        self.expect(Token::RParen)?;
        if types.len() == 1 {
            return Ok(types.into_iter().next().unwrap());
        }
        Ok(Type::Tuple(types))
    }

    /// Parse an optional type annotation: `: Type` or nothing.
    pub fn parse_optional_type(&mut self) -> Result<Option<Type>, SyntaxError> {
        if self.eat(&Token::Colon) {
            self.parse_type().map(Some)
        } else {
            Ok(None)
        }
    }

    /// Parse function return type: `-> Type`
    pub fn parse_return_type(&mut self) -> Result<Option<Type>, SyntaxError> {
        if self.eat(&Token::Arrow) {
            self.parse_type().map(Some)
        } else {
            Ok(None)
        }
    }

    /// Parse type parameters: `<T, U>` or nothing.
    pub fn parse_type_params(&mut self) -> Result<Vec<String>, SyntaxError> {
        if self.eat(&Token::Lt) {
            let mut params = Vec::new();
            loop {
                let name = self.expect_identifier()?;
                params.push(name);
                if !self.eat(&Token::Comma) {
                    break;
                }
            }
            self.expect(Token::Gt)?;
            Ok(params)
        } else {
            Ok(Vec::new())
        }
    }
}
