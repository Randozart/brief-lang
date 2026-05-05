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

use crate::dbrief::ast::*;
use std::str::Chars;

pub struct DbriefParser {
    input: String,
    pos: usize,
}

impl DbriefParser {
    pub fn new(input: String) -> Self {
        DbriefParser { input, pos: 0 }
    }

    pub fn parse(&mut self) -> Result<DbriefProgram, String> {
        let mut program = DbriefProgram {
            imports: Vec::new(),
            registers: Vec::new(),
            aliases: Vec::new(),
            structs: Vec::new(),
            enums: Vec::new(),
            rules: Vec::new(),
            records: Vec::new(),
            checks: Vec::new(),
        };

        while !self.is_eof() {
            self.skip_whitespace();
            if self.is_eof() { break; }

            match self.peek() {
                Some('/') => {
                    if self.peek_next() == Some('/') {
                        self.skip_line_comment();
                    }
                }
                Some('I') if self.starts_with("IMPORT") => {
                    self.parse_import(&mut program)?;
                }
                Some('R') if self.starts_with("REGISTER") => {
                    program.registers.push(self.parse_register()?);
                }
                Some('A') if self.starts_with("ALIAS") => {
                    program.aliases.push(self.parse_alias()?);
                }
                Some('S') if self.starts_with("STRUCT") => {
                    program.structs.push(self.parse_struct()?);
                }
                Some('E') if self.starts_with("ENUM") => {
                    program.enums.push(self.parse_enum()?);
                }
                Some('R') if self.starts_with("RULE") => {
                    program.rules.push(self.parse_rule()?);
                }
                Some('C') if self.starts_with("CHECK") => {
                    program.checks.push(self.parse_check()?);
                }
                Some('@') => {
                    program.records.push(self.parse_record()?);
                }
                _ => {
                    return Err(format!("Unexpected token at position {}", self.pos));
                }
            }
        }

        Ok(program)
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn skip_line_comment(&mut self) {
        while let Some(c) = self.peek() {
            if c == '\n' {
                break;
            }
            self.advance();
        }
    }

    fn peek(&self) -> Option<char> {
        self.input.chars().nth(self.pos)
    }

    fn peek_next(&self) -> Option<char> {
        self.input.chars().nth(self.pos + 1)
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.input.chars().nth(self.pos);
        self.pos += 1;
        c
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn starts_with(&self, s: &str) -> bool {
        self.input[self.pos..].starts_with(s)
    }

    fn parse_import(&mut self, program: &mut DbriefProgram) -> Result<(), String> {
        self.pos += "IMPORT".len();
        self.skip_whitespace();
        
        let path = self.parse_string_literal()?;
        
        let alias = if self.starts_with("AS") {
            self.pos += "AS".len();
            self.skip_whitespace();
            Some(self.parse_identifier()?)
        } else {
            None
        };

        self.consume(';')?;
        
        program.imports.push(ImportStmt { path, alias });
        Ok(())
    }

    fn parse_register(&mut self) -> Result<DbriefRegister, String> {
        self.pos += "REGISTER".len();
        self.skip_whitespace();
        
        let address = self.parse_address()?;
        self.consume(':')?;
        self.skip_whitespace();
        
        let register_type = self.parse_type()?;
        
        let check = if self.starts_with("CHECK") {
            self.pos += "CHECK".len();
            Some(self.parse_check()?)
        } else {
            None
        };

        self.consume(';')?;
        
        Ok(DbriefRegister {
            address,
            name: None,
            register_type,
            check,
        })
    }

    fn parse_alias(&mut self) -> Result<DbriefAlias, String> {
        self.skip_whitespace();
        
        let optional = self.starts_with("ALIAS?");
        if optional {
            self.pos += "ALIAS?".len();
        } else {
            self.pos += "ALIAS".len();
        }
        self.skip_whitespace();
        
        let name = self.parse_identifier()?;
        self.consume(':')?;
        self.skip_whitespace();
        
        let alias_type = self.parse_type()?;
        self.skip_whitespace();
        
        let address = if self.starts_with("=") {
            self.pos += 1;
            self.skip_whitespace();
            Some(self.parse_address()?)
        } else {
            None
        };

        self.skip_whitespace();
        self.consume(';')?;
        
        Ok(DbriefAlias {
            name,
            alias_type,
            address,
            optional,
        })
    }

    fn parse_struct(&mut self) -> Result<DbriefStruct, String> {
        self.pos += "STRUCT".len();
        self.skip_whitespace();
        
        let name = self.parse_identifier()?;
        self.consume('{')?;
        
        let mut fields = Vec::new();
        loop {
            self.skip_whitespace();
            if self.peek() == Some('}') {
                self.advance();
                break;
            }
            
            let field_name = self.parse_identifier()?;
            self.consume(':')?;
            self.skip_whitespace();
            let field_type = self.parse_type()?;
            self.consume(';')?;
            
            fields.push((field_name, field_type));
        }

        Ok(DbriefStruct { name, fields })
    }

    fn parse_enum(&mut self) -> Result<DbriefEnum, String> {
        self.pos += "ENUM".len();
        self.skip_whitespace();
        
        let name = self.parse_identifier()?;
        self.consume('{')?;
        
        let mut variants = Vec::new();
        loop {
            self.skip_whitespace();
            if self.peek() == Some('}') {
                self.advance();
                break;
            }
            
            variants.push(self.parse_identifier()?);
            
            if self.peek() == Some(',') {
                self.advance();
            }
        }

        Ok(DbriefEnum { name, variants })
    }

    fn parse_rule(&mut self) -> Result<DbriefRule, String> {
        self.pos += "RULE".len();
        self.skip_whitespace();
        
        let name = self.parse_identifier()?;
        self.consume('(')?;
        
        let mut params = Vec::new();
        loop {
            self.skip_whitespace();
            if self.peek() == Some(')') {
                self.advance();
                break;
            }
            params.push(self.parse_identifier()?);
            if self.peek() == Some(',') {
                self.advance();
            }
        }
        
        self.consume(':')?;
        self.consume('-')?;
        
        let head = RuleHead { name, params };
        
        let mut body = Vec::new();
        body.push(RuleBody::Fact("todo".to_string(), Vec::new()));
        
        self.consume(';')?;
        
        Ok(DbriefRule { head, body })
    }

    fn parse_check(&mut self) -> Result<DbriefContract, String> {
        self.pos += "CHECK".len();
        self.consume('[')?;
        
        let mut conditions = Vec::new();
        loop {
            self.skip_whitespace();
            if self.peek() == Some(']') {
                self.advance();
                break;
            }
            
            conditions.push(self.parse_expr()?);
            
            if self.peek() == Some(';') {
                self.advance();
            }
        }

        Ok(DbriefContract { conditions })
    }

    fn parse_record(&mut self) -> Result<DbriefRecord, String> {
        self.consume('@')?;
        let address = self.parse_address()?;
        
        self.consume('{')?;
        
        let mut fields = Vec::new();
        loop {
            self.skip_whitespace();
            if self.peek() == Some('}') {
                self.advance();
                break;
            }
            
            let field_name = self.parse_identifier()?;
            self.consume(':')?;
            self.skip_whitespace();
            let value = self.parse_literal()?;
            self.consume(';')?;
            
            fields.push((field_name, value));
        }

        Ok(DbriefRecord { address, fields })
    }

    fn parse_address(&mut self) -> Result<DbriefAddress, String> {
        self.consume('@')?;
        
        if self.starts_with("auto") {
            self.pos += 4;
            return Ok(DbriefAddress::Auto);
        }
        
        if self.peek() == Some('0') && self.peek_next() == Some('x') {
            self.pos += 2;
            let mut hex_str = String::new();
            while let Some(c) = self.peek() {
                if c.is_ascii_hexdigit() {
                    hex_str.push(c);
                    self.advance();
                } else {
                    break;
                }
            }
            let hex_val = u64::from_str_radix(&hex_str, 16).map_err(|_| "Invalid hex")?;
            return Ok(DbriefAddress::Hex(hex_val));
        }
        
        if let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                let mut num_str = String::new();
                while let Some(d) = self.peek() {
                    if d.is_ascii_digit() {
                        num_str.push(d);
                        self.advance();
                    } else {
                        break;
                    }
                }
                let num: u64 = num_str.parse().map_err(|_| "Invalid number")?;
                return Ok(DbriefAddress::Numeric(num));
            }
        }
        
        let name = self.parse_identifier()?;
        Ok(DbriefAddress::Named(name))
    }

fn parse_type(&mut self) -> Result<DbriefType, String> {
        let tok = self.parse_identifier()?;
        
        match tok.as_str() {
            "Bool" => Ok(DbriefType::Bool),
            "Int" => {
                self.consume('[')?;
                let size: usize = self.parse_number::<usize>()?;
                self.consume(']')?;
                Ok(DbriefType::Int(size))
            }
            "UInt" => {
                self.consume('[')?;
                let size: usize = self.parse_number::<usize>()?;
                self.consume(']')?;
                Ok(DbriefType::UInt(size))
            }
            "Float" => Ok(DbriefType::Float),
            "String" => Ok(DbriefType::String),
            "Data" => Ok(DbriefType::Data),
            "Addr" => Ok(DbriefType::Addr),
            "RegOffset" => Ok(DbriefType::RegOffset),
            "Vector" => {
                self.consume('[')?;
                let inner = Box::new(self.parse_type()?);
                self.skip_whitespace();
                let size = if self.peek() == Some(',') {
                    self.advance();
                    let n: usize = self.parse_number::<usize>()?;
                    Some(n)
                } else {
                    None
                };
                self.consume(']')?;
                Ok(DbriefType::Vector(inner, size))
            }
            "Option" => {
                self.consume('[')?;
                let inner = Box::new(self.parse_type()?);
                self.consume(']')?;
                Ok(DbriefType::Option(inner))
            }
            _ => Ok(DbriefType::Named(tok)),
        }
    }

    fn parse_literal(&mut self) -> Result<DbriefLiteral, String> {
        match self.peek() {
            Some('t') if self.starts_with("true") => {
                self.pos += 4;
                Ok(DbriefLiteral::Bool(true))
            }
            Some('f') if self.starts_with("false") => {
                self.pos += 5;
                Ok(DbriefLiteral::Bool(false))
            }
            Some('"') => {
                Ok(DbriefLiteral::String(self.parse_string_literal()?))
            }
            Some(c) if c.is_ascii_digit() => {
                let mut num_str = String::new();
                while let Some(d) = self.peek() {
                    if d.is_ascii_digit() || d == '.' {
                        num_str.push(d);
                        self.advance();
                    } else {
                        break;
                    }
                }
                if num_str.contains('.') {
                    let f: f64 = num_str.parse().map_err(|_| "Invalid float")?;
                    Ok(DbriefLiteral::Float(f))
                } else {
                    let n: u64 = num_str.parse().map_err(|_| "Invalid number")?;
                    Ok(DbriefLiteral::UInt(n))
                }
            }
            Some('{') => {
                self.consume('{')?;
                let mut fields = std::collections::HashMap::new();
                loop {
                    self.skip_whitespace();
                    if self.peek() == Some('}') {
                        self.advance();
                        break;
                    }
                    let key = self.parse_identifier()?;
                    self.consume(':')?;
                    self.skip_whitespace();
                    let val = self.parse_literal()?;
                    self.consume(';')?;
                    fields.insert(key, val);
                }
                Ok(DbriefLiteral::Struct(fields))
            }
            _ => Err(format!("Unexpected literal at position {}", self.pos)),
        }
    }

    fn parse_expr(&mut self) -> Result<DbriefExpr, String> {
        self.skip_whitespace();
        
        let mut left = self.parse_primary_expr()?;
        
        loop {
            self.skip_whitespace();
            if let Some(op) = self.parse_binary_op() {
                self.skip_whitespace();
                let right = self.parse_primary_expr()?;
                left = DbriefExpr::BinaryOp(Box::new(left), op, Box::new(right));
            } else {
                break;
            }
        }
        
        Ok(left)
    }
    
    fn parse_primary_expr(&mut self) -> Result<DbriefExpr, String> {
        self.skip_whitespace();
        
        match self.peek() {
            Some('t') if self.starts_with("true") => {
                self.pos += 4;
                Ok(DbriefExpr::Bool(true))
            }
            Some('f') if self.starts_with("false") => {
                self.pos += 5;
                Ok(DbriefExpr::Bool(false))
            }
            Some(c) if c.is_ascii_digit() => {
                let mut num_str = String::new();
                while let Some(d) = self.peek() {
                    if d.is_ascii_digit() || d == '.' {
                        num_str.push(d);
                        self.advance();
                    } else {
                        break;
                    }
                }
                if num_str.contains('.') {
                    let f: f64 = num_str.parse().map_err(|_| "Invalid float")?;
                    Ok(DbriefExpr::Float(f))
                } else {
                    let n: i64 = num_str.parse().map_err(|_| "Invalid number")?;
                    Ok(DbriefExpr::Int(n))
                }
            }
            Some(c) if c.is_alphabetic() => {
                Ok(DbriefExpr::Ident(self.parse_identifier()?))
            }
            _ => Err(format!("Unexpected expr at position {}", self.pos)),
        }
    }
    
    fn parse_binary_op(&mut self) -> Option<BinaryOp> {
        if self.starts_with(">=") {
            self.pos += 2;
            Some(BinaryOp::Ge)
        } else if self.starts_with("<=") {
            self.pos += 2;
            Some(BinaryOp::Le)
        } else if self.starts_with("==") {
            self.pos += 2;
            Some(BinaryOp::Eq)
        } else if self.starts_with("!=") {
            self.pos += 2;
            Some(BinaryOp::Ne)
        } else if self.peek() == Some('>') {
            self.pos += 1;
            Some(BinaryOp::Gt)
        } else if self.peek() == Some('<') {
            self.pos += 1;
            Some(BinaryOp::Lt)
        } else {
            None
        }
    }

    fn parse_identifier(&mut self) -> Result<String, String> {
        let mut result = String::new();
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                result.push(c);
                self.advance();
            } else {
                break;
            }
        }
        if result.is_empty() {
            Err(format!("Expected identifier at position {}", self.pos))
        } else {
            Ok(result)
        }
    }

    fn parse_string_literal(&mut self) -> Result<String, String> {
        self.consume('"')?;
        let mut result = String::new();
        while let Some(c) = self.peek() {
            if c == '"' {
                self.advance();
                return Ok(result);
            }
            result.push(c);
            self.advance();
        }
        Err("Unterminated string".to_string())
    }

    fn parse_number<T: std::str::FromStr>(&mut self) -> Result<T, String> {
        self.skip_whitespace();
        let mut num_str = String::new();
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                num_str.push(c);
                self.advance();
            } else {
                break;
            }
        }
        num_str.parse().map_err(|_| "Invalid number".to_string())
    }

    fn consume(&mut self, expected: char) -> Result<(), String> {
        self.skip_whitespace();
        if let Some(c) = self.peek() {
            if c == expected {
                self.advance();
                Ok(())
            } else {
                Err(format!("Expected '{}' but found '{}'", expected, c))
            }
        } else {
            Err(format!("Expected '{}' but reached end of input", expected))
        }
    }
}

pub fn parse_dbrief(input: &str) -> Result<DbriefProgram, String> {
    DbriefParser::new(input.to_string()).parse()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_register() {
        let input = r#"
            REGISTER @1: Vector[Person];
            
            STRUCT Person {
                name: String;
                age: UInt[8];
            }
        "#;
        let mut parser = DbriefParser::new(input.to_string());
        let result = parser.parse();
        assert!(result.is_ok());
        let program = result.unwrap();
        assert_eq!(program.registers.len(), 1);
        assert_eq!(program.structs.len(), 1);
    }

    #[test]
    fn test_parse_alias_with_address() {
        let input = "ALIAS led: Bool = @0xFF5E0000;";
        let mut parser = DbriefParser::new(input.to_string());
        let result = parser.parse();
        assert!(result.is_ok(), "Failed to parse: {:?}", result);
        let program = result.unwrap();
        assert_eq!(program.aliases.len(), 1);
        
        let alias = &program.aliases[0];
        assert_eq!(alias.name, "led");
        assert!(matches!(alias.alias_type, DbriefType::Bool));
        assert!(!alias.optional);
        assert!(alias.address.is_some());
        
        if let Some(addr) = &alias.address {
            assert!(matches!(addr, DbriefAddress::Hex(0xFF5E0000)));
        }
    }

    #[test]
    fn test_parse_alias_optional() {
        let input = "ALIAS? debug: Bool;";
        let mut parser = DbriefParser::new(input.to_string());
        let result = parser.parse();
        assert!(result.is_ok(), "Failed to parse: {:?}", result);
        let program = result.unwrap();
        assert_eq!(program.aliases.len(), 1);
        
        let alias = &program.aliases[0];
        assert_eq!(alias.name, "debug");
        assert!(alias.optional);
        assert!(alias.address.is_none());
    }

    #[test]
    fn test_parse_alias_multiple() {
        let input = r#"
            ALIAS led: Bool = @0xFF5E0000;
            ALIAS? debug: Bool;
        "#;
        let mut parser = DbriefParser::new(input.to_string());
        let result = parser.parse();
        assert!(result.is_ok(), "Failed to parse: {:?}", result);
        let program = result.unwrap();
        assert_eq!(program.aliases.len(), 2);
        
        // First alias should have address
        assert!(program.aliases[0].address.is_some());
        
        // Second alias should be optional without address
        assert!(program.aliases[1].optional);
        assert!(program.aliases[1].address.is_none());
    }

    #[test]
    fn test_parse_check_with_conditions() {
        let input = r#"
            CHECK [
                age > 18;
                age < 150;
            ]
        "#;
        let mut parser = DbriefParser::new(input.to_string());
        let result = parser.parse();
        assert!(result.is_ok(), "Failed to parse: {:?}", result);
        let program = result.unwrap();
        assert_eq!(program.checks.len(), 1);
        
        let check = &program.checks[0];
        assert_eq!(check.conditions.len(), 2);
        
        // First condition should be age > 18 (BinaryOp)
        if let DbriefExpr::BinaryOp(left, op, right) = &check.conditions[0] {
            assert!(matches!(**left, DbriefExpr::Ident(ref n) if n == "age"));
            assert!(matches!(op, BinaryOp::Gt));
            assert!(matches!(**right, DbriefExpr::Int(18)));
        } else {
            panic!("Expected binary expression for age > 18");
        }
        
        // Second condition should be age < 150
        if let DbriefExpr::BinaryOp(left, op, right) = &check.conditions[1] {
            assert!(matches!(**left, DbriefExpr::Ident(ref n) if n == "age"));
            assert!(matches!(op, BinaryOp::Lt));
            assert!(matches!(**right, DbriefExpr::Int(150)));
        } else {
            panic!("Expected binary expression for age < 150");
        }
    }

    #[test]
    fn test_parse_check_single_condition() {
        let input = "CHECK [ age >= 21 ]";
        let mut parser = DbriefParser::new(input.to_string());
        let result = parser.parse();
        assert!(result.is_ok(), "Failed to parse: {:?}", result);
        let program = result.unwrap();
        assert_eq!(program.checks.len(), 1);
        
        let check = &program.checks[0];
        assert_eq!(check.conditions.len(), 1);
        
        if let DbriefExpr::BinaryOp(_, op, _) = &check.conditions[0] {
            assert!(matches!(op, BinaryOp::Ge));
        } else {
            panic!("Expected binary expression");
        }
    }

    #[test]
    fn test_parse_address_hex() {
        let input = "@0xFF5E0000";
        let mut parser = DbriefParser::new(input.to_string());
        let result = parser.parse_address();
        assert!(result.is_ok(), "Failed to parse address: {:?}", result);
        assert_eq!(result.unwrap(), DbriefAddress::Hex(0xFF5E0000));
    }

    #[test]
    fn test_parse_address_numeric() {
        let input = "@123";
        let mut parser = DbriefParser::new(input.to_string());
        let result = parser.parse_address();
        assert!(result.is_ok(), "Failed to parse address: {:?}", result);
        assert_eq!(result.unwrap(), DbriefAddress::Numeric(123));
    }

    #[test]
    fn test_parse_address_auto() {
        let input = "@auto";
        let mut parser = DbriefParser::new(input.to_string());
        let result = parser.parse_address();
        assert!(result.is_ok(), "Failed to parse address: {:?}", result);
        assert_eq!(result.unwrap(), DbriefAddress::Auto);
    }

    #[test]
    fn test_parse_address_named() {
        let input = "@led_register";
        let mut parser = DbriefParser::new(input.to_string());
        let result = parser.parse_address();
        assert!(result.is_ok(), "Failed to parse address: {:?}", result);
        assert_eq!(result.unwrap(), DbriefAddress::Named("led_register".to_string()));
    }

    #[test]
    fn test_parse_type_uint() {
        let input = "UInt[32]";
        let mut parser = DbriefParser::new(input.to_string());
        let result = parser.parse_type();
        assert!(result.is_ok(), "Failed to parse type: {:?}", result);
        assert_eq!(result.unwrap(), DbriefType::UInt(32));
    }

    #[test]
    fn test_parse_type_int() {
        let input = "Int[16]";
        let mut parser = DbriefParser::new(input.to_string());
        let result = parser.parse_type();
        assert!(result.is_ok(), "Failed to parse type: {:?}", result);
        assert_eq!(result.unwrap(), DbriefType::Int(16));
    }

    #[test]
    fn test_parse_type_vector() {
        let input = "Vector[UInt[8], 1024]";
        let mut parser = DbriefParser::new(input.to_string());
        let result = parser.parse_type();
        assert!(result.is_ok(), "Failed to parse type: {:?}", result);
        
        if let DbriefType::Vector(inner, size) = result.unwrap() {
            assert!(matches!(*inner, DbriefType::UInt(8)));
            assert_eq!(size, Some(1024));
        } else {
            panic!("Expected Vector type");
        }
    }
}