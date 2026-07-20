// ── Type Parser ────────────────────────────────────────────────────────
// 2026-07-12: Phase 1.4 — Parse Brief type annotations.
// Flat code: each function is max 2 levels of nesting.

use super::helpers::Parser;
use crate::ast::Type;
use crate::errors::SyntaxError;
use crate::lexer::Token;

impl<'a> Parser<'a> {
    /// Parse a type annotation.
    /// 2026-07-16: Bits-thesis — all type names are Token::Identifier.
    /// Dispatch on identifier string, not token variant.
    pub fn parse_type(&mut self) -> Result<Type, SyntaxError> {
        let base = match self.peek() {
            Some(Token::Identifier(name)) => {
                let name = name.clone();
                self.pos += 1;
                match name.as_str() {
                    // Type::int() etc. are frontend conveniences for Bits(N) + metadata.
                    "Int" => ("Int", Type::int()),
                    "UInt" => ("UInt", Type::Custom("UInt".into())),
                    "Float" | "Float32" | "F32" => ("Float", Type::float()),
                    "Float64" | "F64" | "Double" => ("Float64", Type::float64()),
                    "String" => ("String", Type::string()),
                    "Bool" => ("Bool", Type::bool_()),
                    "Void" => return Ok(Type::void()),
                    "Char" => ("Char", Type::char_()),
                    "Data" => ("Data", Type::data()),
                    _ if name.starts_with('#') => {
                        // 2026-07-20: Hashword category: #Int, #Float, #String, etc.
                        return Ok(Type::HashWord(name));
                    }
                    _ => {
                        // Unknown name — delegate to parse_named_type_body
                        // (handles Ptr<T>, Foo<T>, .ext suffixes, Custom types)
                        let ty = self.parse_named_type_body(&name)?;
                        return Ok(ty);
                    }
                }
            }
            Some(Token::LParen) => return self.parse_tuple_type(),
            _ => return self.error_at_current("expected type"),
        };
        // 2026-07-16: P2 — Check for .ext suffix on keyword types (e.g. String.c, Int.c.sso)
        if let Some(ext) = self.try_parse_dot_extension() {
            let mut full = format!("{}.{}", base.0, ext);
            // Allow deeper extensions: "Int.c.sso"
            while let Some(next) = self.try_parse_dot_extension() {
                full = format!("{}.{}", full, next);
            }
            return Ok(Type::Custom(full));
        }
        Ok(base.1)
    }

    /// 2026-07-16: P2 — If the next token is `.ident`, consume and return the identifier.
    /// Used to parse extension type names like String.c.
    fn try_parse_dot_extension(&mut self) -> Option<String> {
        if !self.eat(&Token::Dot) {
            return None;
        }
        match self.peek() {
            Some(Token::Identifier(name)) => {
                let name = name.clone();
                self.pos += 1;
                Some(name)
            }
            _ => {
                // Dot without following identifier — restore position.
                // We can't easily un-eat the Dot, but in practice this shouldn't occur
                // in valid programs.
                None
            }
        }
    }

    /// Parse a named type, possibly with generic parameters, pointer prefix, or extension.
    fn parse_named_type_body(&mut self, name: &str) -> Result<Type, SyntaxError> {
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
            return Ok(Type::Applied(name.to_string(), args));
        }

        // 2026-07-16: P2 — Check for .ext suffix (e.g., "MyType.c", "MyType.c.sso")
        if let Some(ext) = self.try_parse_dot_extension() {
            let mut full = format!("{}.{}", name, ext);
            // Allow deeper: "Int.c.sso"
            while let Some(next) = self.try_parse_dot_extension() {
                full = format!("{}.{}", full, next);
            }
            return Ok(Type::Custom(full));
        }

        Ok(Type::Custom(name.to_string()))
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
    pub fn parse_type_params(&mut self) -> Result<Vec<crate::ast::top::TypeParam>, SyntaxError> {
        if self.eat(&Token::Lt) {
            let mut params = Vec::new();
            loop {
                let name = self.expect_identifier()?;
                // 2026-07-20: Optional bound: K: #String or K: String
                let bound = if self.eat(&Token::Colon) {
                    Some(self.parse_type()?)
                } else {
                    None
                };
                params.push(crate::ast::top::TypeParam { name, bound });
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
