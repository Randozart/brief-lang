// ── Type Parser ────────────────────────────────────────────────────────
// 2026-07-12: Phase 1.4 — Parse Brief type annotations.
// Flat code: each function is max 2 levels of nesting.

use super::helpers::Parser;
use crate::ast::{Dimension, Type};
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
                    // 2026-08-03: callback/function-pointer type annotation:
                    //   fn(Int) -> Int  /  fn(Int)  (void return)
                    // Crosses an FFI boundary as an opaque function pointer.
                    "fn" => return self.parse_fn_type(),
                    _ if name.starts_with('#') => {
                        // 2026-07-20: Hashword category: #Int, #Float, #String, etc.
                        // Optional protocol variant: #String<UTF8>, #Float<IEEE754>
                        if self.eat(&Token::Lt) {
                            let variant = self.expect_identifier()?;
                            if !self.eat_type_close() {
                                return self.error_at_current("expected '>' or '>>' in hashword variant");
                            }
                            return Ok(Type::HashWordVariant(name, variant));
                        }
                        // 2026-07-20: Bare hashwords resolve to their default variant.
                        // UTF-8 is the universal default for all files.
                        let variant = match name.as_str() {
                            "#String" => "UTF8",
                            "#Float" => "IEEE754",
                            "#Char" => "unicode",
                            _ => "",
                        };
                        if !variant.is_empty() {
                            return Ok(Type::HashWordVariant(name, variant.to_string()));
                        }
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
        // 2026-07-25: Array syntax: Int[1024] → Type::Vector.
        // 2026-07-31: `[` is an array suffix ONLY when the next token is an
        // integer literal (`Int[8]`) or an identifier directly followed by
        // `]` (`T[N]` generic array). A contract bracket (`-> Int [b != 0]`)
        // is left for parse_contract.
        if self.check(&Token::LBracket) {
            if matches!(self.peek_next(), Some(Token::Integer(_))) {
                self.pos += 1; // consume LBracket
                let size = match self.peek() {
                    Some(&Token::Integer(n)) => { self.pos += 1; n as usize }
                    _ => { return self.error_at_current("expected array size (integer)"); }
                };
                self.expect(Token::RBracket)?;
                return Ok(Type::Vector(Box::new(base.1), vec![Dimension::Anonymous(size)]));
            }
            if let Some(Token::Identifier(_)) = self.peek_next() {
                let after_ident = self.tokens.get(self.pos + 2).map(|(t, _)| t);
                if matches!(after_ident, Some(Token::RBracket)) {
                    // 2026-08-01 (Phase 2): `Type[#]` (from `-> Int [#]`) is
                    // the removed entry-point marker, NOT a named array
                    // dimension. Reject it with the same clear error as
                    // parse_contract so `defn main() -> Int [#]` fails loudly
                    // instead of producing `Int[#]` (a named dimension "#").
                    if let Some(Token::Identifier(ident)) = self.peek_next() {
                        if ident == "#" {
                            return Err(crate::errors::SyntaxError::InvalidStatement {
                                reason: "'[#]' entry-point syntax removed — use the entry!/args! \
                                         macros (Phase 3) or write an explicit contract"
                                    .to_string(),
                                span: crate::errors::Span::dummy(),
                            });
                        }
                    }
                    self.pos += 1; // consume LBracket
                    let name = self.expect_identifier()?;
                    self.expect(Token::RBracket)?;
                    return Ok(Type::Vector(Box::new(base.1), vec![Dimension::Named(name, 0)]));
                }
            }
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
        // Bits<N> — numeric bit width, no annotation = flexible
        if name == "Bit" || name == "bits" {
            if self.eat(&Token::Lt) {
                let bits = match self.peek() {
                    Some(&Token::Integer(n)) => {
                        self.pos += 1;
                        n as u64
                    }
                    _ => return self.error_at_current("expected bit count (integer) in Bits<N>"),
                };
                if !self.eat_type_close() {
                    return self.error_at_current("expected '>' or '>>' in Bits<N>");
                }
                return Ok(Type::from_bits(bits));
            }
            return Ok(Type::Bits(0)); // flexible-width Bits
        }

        // Ptr<T> handling
        if name == "Ptr" || name == "Ptr!" {
            if self.eat(&Token::Lt) {
                let inner = self.parse_type()?;
                if !self.eat_type_close() {
                    return self.error_at_current("expected '>' or '>>' in Ptr<T>");
                }
                return Ok(Type::ptr(inner));
            }
            return Ok(Type::ptr(Type::bits(1)));
        }

        // Generic type: Foo<T, U>
        if self.eat(&Token::Lt) {
            let mut args = Vec::new();
            loop {
                // 2026-07-31 (A8): a numeric generic argument is a compile-time
                // SIZE parameter (`Stack<Int, 8>`).
                let next_is_int = matches!(self.peek(), Some(Token::Integer(_)));
                if next_is_int {
                    let n = match self.peek() {
                        Some(Token::Integer(n)) => *n,
                        _ => 0,
                    };
                    self.pos += 1;
                    args.push(Type::Number(n));
                } else {
                    args.push(self.parse_type()?);
                }
                if !self.eat(&Token::Comma) {
                    break;
                }
            }
            if !self.eat_type_close() {
                return self.error_at_current("expected '>' or '>>' to close generic type");
            }
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

        // 2026-07-25: Array syntax for custom types: MyStruct[1024].
        // 2026-07-31: Same non-greedy lookahead as keyword types — an integer
        // size or a `[Name]` generic dimension; otherwise the `[` is a
        // contract bracket.
        if self.check(&Token::LBracket) {
            if matches!(self.peek_next(), Some(Token::Integer(_))) {
                self.pos += 1; // consume LBracket
                let size = match self.peek() {
                    Some(&Token::Integer(n)) => { self.pos += 1; n as usize }
                    _ => { return self.error_at_current("expected array size (integer)"); }
                };
                self.expect(Token::RBracket)?;
                return Ok(Type::Vector(Box::new(Type::Custom(name.to_string())), vec![Dimension::Anonymous(size)]));
            }
            if let Some(Token::Identifier(_)) = self.peek_next() {
                let after_ident = self.tokens.get(self.pos + 2).map(|(t, _)| t);
                if matches!(after_ident, Some(Token::RBracket)) {
                    self.pos += 1;
                    let dim = self.expect_identifier()?;
                    self.expect(Token::RBracket)?;
                    return Ok(Type::Vector(
                        Box::new(Type::Custom(name.to_string())),
                        vec![Dimension::Named(dim, 0)],
                    ));
                }
            }
        }

        Ok(Type::Custom(name.to_string()))
    }

    /// Parse a function/callback type: `fn(Int) -> Int` (or `fn(Int)` → void).
    /// 2026-08-03: crosses an FFI boundary as an opaque function pointer.
    fn parse_fn_type(&mut self) -> Result<Type, SyntaxError> {
        self.expect(Token::LParen)?;
        let mut params = Vec::new();
        if !self.check(&Token::RParen) {
            loop {
                params.push(self.parse_type()?);
                if !self.eat(&Token::Comma) {
                    break;
                }
            }
        }
        self.expect(Token::RParen)?;
        let ret = if self.check(&Token::Arrow) {
            self.pos += 1;
            Box::new(self.parse_type()?)
        } else {
            Box::new(Type::void())
        };
        Ok(Type::Function(params, ret))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Token;
    use logos::Logos;

    fn parse_type_str(src: &str) -> Type {
        let tokens: Vec<(Token, std::ops::Range<usize>)> = Token::lexer(src)
            .map(|r| (r.unwrap(), 0..0))
            .collect();
        let mut p = Parser::new(tokens, src);
        p.parse_type().expect("type should parse")
    }

    #[test]
    fn fn_type_annotation() {
        let t = parse_type_str("fn(Int) -> Int");
        assert!(matches!(t, Type::Function(params, ret) if params.len() == 1 && matches!(params[0], Type::Custom(ref n) if n == "Int") && matches!(*ret, Type::Custom(ref n) if n == "Int")));
    }

    #[test]
    fn fn_type_void_return_defaults() {
        let t = parse_type_str("fn(Int)");
        assert!(matches!(t, Type::Function(params, ret) if params.len() == 1 && matches!(*ret, Type::Void)));
    }

    #[test]
    fn fn_type_multi_param() {
        let t = parse_type_str("fn(Int, Float) -> Bool");
        assert!(matches!(t, Type::Function(params, ret) if params.len() == 2 && matches!(*ret, Type::Custom(ref n) if n == "Bool")));
    }
}
