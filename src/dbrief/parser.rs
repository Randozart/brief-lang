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
            services: Vec::new(),
            rules: Vec::new(),
            records: Vec::new(),
            checks: Vec::new(),
            depends: Vec::new(),
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
                Some('i') if self.starts_with("import") => {
                    self.parse_import(&mut program)?;
                }
                // DEPENDS - new keyword (abbreviations: dep, deps)
                Some('D') if self.starts_with("DEPENDS") || self.starts_with("DEPS") || self.starts_with("DEP") => {
                    self.parse_depends(&mut program)?;
                }
                Some('d') if self.starts_with("depends") || self.starts_with("deps") || self.starts_with("dep") => {
                    self.parse_depends(&mut program)?;
                }
                // REGISTER (abbreviations: reg, regs)
                Some('R') if self.starts_with("REGISTER") || self.starts_with("REGS") || self.starts_with("REG") => {
                    program.registers.push(self.parse_register()?);
                }
                Some('r') if self.starts_with("register") || self.starts_with("regs") || self.starts_with("reg") => {
                    program.registers.push(self.parse_register()?);
                }
                // ALIAS (abbreviation: ali)
                Some('A') if self.starts_with("ALIAS") || self.starts_with("ALIAS?") || self.starts_with("ALI") => {
                    program.aliases.push(self.parse_alias()?);
                }
                Some('a') if self.starts_with("alias") || self.starts_with("alias?") || self.starts_with("ali") => {
                    program.aliases.push(self.parse_alias()?);
                }
                // STRUCT (abbreviations: stru, str)
                Some('S') if self.starts_with("STRUCT") || self.starts_with("STRU") || self.starts_with("STR") => {
                    program.structs.push(self.parse_struct()?);
                }
                Some('s') if self.starts_with("struct") || self.starts_with("stru") || self.starts_with("str") => {
                    program.structs.push(self.parse_struct()?);
                }
                // SERVICE (abbreviations: serv, svc)
                Some('S') if self.starts_with("SERVICE") || self.starts_with("SERV") || self.starts_with("SVC") => {
                    program.services.push(self.parse_service()?);
                }
                Some('s') if self.starts_with("service") || self.starts_with("serv") || self.starts_with("svc") => {
                    program.services.push(self.parse_service()?);
                }
                // ENUM (abbreviations: en, e)
                Some('E') if self.starts_with("ENUM") || self.starts_with("EN") => {
                    program.enums.push(self.parse_enum()?);
                }
                Some('e') if self.starts_with("enum") || self.starts_with("en") => {
                    program.enums.push(self.parse_enum()?);
                }
                // RULE (abbreviations: rl, rul)
                Some('R') if self.starts_with("RULE") || self.starts_with("RL") || self.starts_with("RUL") => {
                    program.rules.push(self.parse_rule()?);
                }
                Some('r') if self.starts_with("rule") || self.starts_with("rl") || self.starts_with("rul") => {
                    program.rules.push(self.parse_rule()?);
                }
                // CHECK (abbreviation: chk)
                Some('C') if self.starts_with("CHECK") || self.starts_with("CHK") => {
                    program.checks.push(self.parse_check()?);
                }
                Some('c') if self.starts_with("check") || self.starts_with("chk") => {
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

    fn consume_keyword(&mut self, kw: &str) -> Result<(), String> {
        if self.starts_with(&kw.to_uppercase()) {
            self.pos += kw.len();
            Ok(())
        } else if self.starts_with(&kw.to_lowercase()) {
            self.pos += kw.len();
            Ok(())
        } else {
            Err(format!("Expected keyword '{}'", kw))
        }
    }

    fn parse_import(&mut self, program: &mut DbriefProgram) -> Result<(), String> {
        self.consume_keyword("IMPORT")?;
        self.skip_whitespace();
        
        let path = self.parse_string_literal()?;
        
        let alias = if self.starts_with("AS") || self.starts_with("as") {
            if self.starts_with("AS") {
                self.pos += 2;
            } else {
                self.pos += 2;
            }
            self.skip_whitespace();
            Some(self.parse_identifier()?)
        } else {
            None
        };

        self.consume(';')?;
        
        program.imports.push(ImportStmt { path, alias });
        Ok(())
    }

    fn parse_depends(&mut self, program: &mut DbriefProgram) -> Result<(), String> {
        // Consume DEPENDS (or dep/deps)
        if self.starts_with("DEPENDS") {
            self.pos += 7;
        } else if self.starts_with("DEPS") {
            self.pos += 4;
        } else if self.starts_with("DEP") {
            self.pos += 3;
        } else if self.starts_with("depends") {
            self.pos += 7;
        } else if self.starts_with("deps") {
            self.pos += 4;
        } else if self.starts_with("dep") {
            self.pos += 3;
        }
        
        self.skip_whitespace();
        
        // Parse package name (string)
        let name = self.parse_string_literal()?;
        self.skip_whitespace();
        
        let mut version_constraint: Option<String> = None;
        let mut platform: Vec<String> = Vec::new();
        let mut features: Vec<String> = Vec::new();
        let mut source: Option<String> = None;
        
        // Parse remaining clauses
        while !self.starts_with(";") && !self.is_eof() {
            let kw = self.parse_identifier()?;
            self.skip_whitespace();
            
            match kw.to_uppercase().as_str() {
                "VERSION" | "VER" | "V" => {
                    version_constraint = Some(self.parse_string_literal()?);
                }
                "PLATFORM" | "PLAT" | "P" => {
                    if self.peek() == Some('[') {
                        self.consume('[')?;
                        while !self.starts_with("]") && !self.is_eof() {
                            let p = self.parse_identifier()?;
                            platform.push(p);
                            if self.peek() == Some(',') {
                                self.consume(',')?;
                                self.skip_whitespace();
                            }
                        }
                        self.consume(']')?;
                    } else {
                        platform.push(self.parse_identifier()?);
                    }
                }
                "FEATURES" | "FEAT" | "F" => {
                    if self.peek() == Some('[') {
                        self.consume('[')?;
                        while !self.starts_with("]") && !self.is_eof() {
                            let f = self.parse_identifier()?;
                            features.push(f);
                            if self.peek() == Some(',') {
                                self.consume(',')?;
                                self.skip_whitespace();
                            }
                        }
                        self.consume(']')?;
                    } else {
                        features.push(self.parse_identifier()?);
                    }
                }
                "SOURCE" | "SRC" | "S" => {
                    source = Some(self.parse_string_literal()?);
                }
                _ => {
                    return Err(format!("Unknown DEPENDS clause: {}", kw));
                }
            }
            self.skip_whitespace();
        }
        
        self.consume(';')?;
        
        program.depends.push(DbriefDependency {
            name,
            version_constraint,
            platform,
            features,
            source,
        });
        
        Ok(())
    }

    fn parse_register(&mut self) -> Result<DbriefRegister, String> {
        // Handle both full keyword and abbreviations
        if self.starts_with("REGISTER") || self.starts_with("register") {
            self.consume_keyword("REGISTER")?;
        } else if self.starts_with("REGS") {
            self.pos += 4;
        } else if self.starts_with("regs") {
            self.pos += 4;
        } else if self.starts_with("REG") {
            self.pos += 3;
        } else if self.starts_with("reg") {
            self.pos += 3;
        } else {
            return Err("Expected REGISTER".to_string());
        }
        self.skip_whitespace();
        
        let address = self.parse_address()?;
        self.skip_whitespace();
        
        // Parse optional "as" name
        let name = if self.starts_with("as") || self.starts_with("AS") {
            self.consume_keyword("as")?;
            self.skip_whitespace();
            Some(self.parse_string()?)
        } else {
None
        };
        
        self.skip_whitespace();
        
        // Check for block syntax { ... } or colon syntax : Type
        let (register_type, check, location, target, description, input_params, output_type, error_type) = 
            if self.peek() == Some('{') {
                self.consume('{')?;
                let mut reg_type: Option<DbriefType> = None;
                let mut chk: Option<DbriefContract> = None;
                let mut loc: Option<String> = None;
                let mut tgt: Option<String> = None;
                let mut desc: Option<String> = None;
                let mut inputs: Vec<(String, DbriefType)> = Vec::new();
                let mut out_type: Option<DbriefType> = None;
                let mut err_type: Option<DbriefType> = None;
                
                loop {
                    self.skip_whitespace();
                    if self.peek() == Some('}') {
                        self.advance();
                        break;
                    }
                    
                    let field = self.parse_identifier()?;
                    self.consume(':')?;
                    self.skip_whitespace();
                    
                    match field.to_lowercase().as_str() {
                        "type" => {
                            reg_type = Some(self.parse_type()?);
                        }
                        "check" => {
                            chk = Some(self.parse_check()?);
                        }
                        "location" => {
                            loc = Some(self.parse_string()?);
                        }
                        "target" => {
                            tgt = Some(self.parse_string()?);
                        }
                        "description" => {
                            desc = Some(self.parse_string()?);
                        }
                        "input" => {
                            self.consume('(')?;
                            loop {
                                self.skip_whitespace();
                                if self.peek() == Some(')') {
                                    self.advance();
                                    break;
                                }
                                let param_name = self.parse_identifier()?;
                                self.consume(':')?;
                                let param_type = self.parse_type()?;
                                inputs.push((param_name, param_type));
                                self.skip_whitespace();
                                if self.peek() == Some(',') {
                                    self.advance();
                                }
                            }
                        }
                        "output" => {
                            out_type = Some(self.parse_type()?);
                        }
                        "error" => {
                            err_type = Some(self.parse_type()?);
                        }
                        _ => {
                            // Skip unknown field value
                            self.skip_to_semicolon()?;
                        }
                    }
                    
                    if self.peek() == Some(';') {
                        self.advance();
                    }
                }
                
                (reg_type.unwrap_or(DbriefType::Data), chk, loc, tgt, desc, inputs, out_type, err_type)
            } else {
                self.consume(':')?;
                self.skip_whitespace();
                let reg_type = self.parse_type()?;
                
                let chk = if self.starts_with("CHECK") || self.starts_with("check") {
                    self.consume_keyword("CHECK")?;
                    Some(self.parse_check()?)
                } else {
                    None
                };
                
                self.consume(';')?;
                (reg_type, chk, None, None, None, Vec::new(), None, None)
            };
        
        Ok(DbriefRegister {
            address,
            name,
            register_type,
            check,
            location,
            target,
            description,
            input_params,
            output_type,
            error_type,
        })
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.skip_whitespace();
        if self.peek() == Some('"') {
            self.advance(); // consume opening quote
            let mut s = String::new();
            while let Some(c) = self.peek() {
                if c == '"' {
                    self.advance(); // consume closing quote
                    return Ok(s);
                }
                s.push(c);
                self.advance();
            }
            Err("Unterminated string".to_string())
        } else {
            // Parse as identifier
            self.parse_identifier()
        }
    }

    fn skip_to_semicolon(&mut self) -> Result<(), String> {
        while let Some(c) = self.peek() {
            if c == ';' {
                return Ok(());
            }
            self.advance();
        }
        Err("Expected semicolon".to_string())
    }

    fn parse_alias(&mut self) -> Result<DbriefAlias, String> {
        self.skip_whitespace();
        
        let optional = self.starts_with("ALIAS?") || self.starts_with("alias?");
        if optional {
            self.pos += "ALIAS?".len();
        } else if self.starts_with("ALIAS") || self.starts_with("alias") {
            self.pos += 5;
        } else if self.starts_with("ALI") || self.starts_with("ali") {
            self.pos += 3;
        } else {
            return Err("Expected ALIAS or ALI".to_string());
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
        // Accept STRUCT, STR, STRU (case insensitive)
        if self.starts_with("STRUCT") || self.starts_with("struct") {
            self.pos += 6;
        } else if self.starts_with("STRU") || self.starts_with("stru") {
            self.pos += 4;
        } else if self.starts_with("STR") || self.starts_with("str") {
            self.pos += 3;
        } else {
            return Err("Expected STRUCT, STR, or STRU".to_string());
        }
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
        self.consume_keyword("ENUM")?;
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

    fn parse_service(&mut self) -> Result<DbriefService, String> {
        // Accept SERVICE, SERV, SVC (case insensitive)
        if self.starts_with("SERVICE") || self.starts_with("service") {
            self.pos += 7;
        } else if self.starts_with("SERV") || self.starts_with("serv") {
            self.pos += 4;
        } else if self.starts_with("SVC") || self.starts_with("svc") {
            self.pos += 3;
        } else {
            return Err("Expected SERVICE, SERV, or SVC".to_string());
        }
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

            let direction = self.parse_identifier()?;
            let direction_upper = direction.to_uppercase();
            if direction_upper != "INPUT" && direction_upper != "OUTPUT" {
                return Err(format!(
                    "Service field direction must be INPUT or OUTPUT, got '{}'",
                    direction
                ));
            }

            self.skip_whitespace();
            let field_name = self.parse_identifier()?;
            self.consume(':')?;
            self.skip_whitespace();
            let field_type = self.parse_type()?;
            self.consume(';')?;

            fields.push(DbriefServiceField {
                direction: direction_upper,
                name: field_name,
                field_type,
            });
        }

        Ok(DbriefService {
            name,
            fields,
            description: None,
        })
    }

    fn parse_rule(&mut self) -> Result<DbriefRule, String> {
        self.consume_keyword("RULE")?;
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
        // Accept optional CHECK keyword (block format may have already consumed it)
        if self.starts_with("CHECK") || self.starts_with("check") {
            self.consume_keyword("CHECK")?;
        }
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
        // Handle optional @ prefix - some callers consume it, some don't
        if self.peek() == Some('@') {
            self.advance();
        }
        
        if self.starts_with("auto") || self.starts_with("AUTO") {
            if self.starts_with("auto") { self.pos += 4; } else { self.pos += 4; }
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
        
        match tok.to_lowercase().as_str() {
            "bool" => Ok(DbriefType::Bool),
            "int" | "Int" => {
                // Allow just "Int" as a generic integer (defaults to 32-bit)
                if self.peek() == Some('[') {
                    self.consume('[')?;
                    let size: usize = self.parse_number::<usize>()?;
                    self.consume(']')?;
                    Ok(DbriefType::Int(size))
                } else {
                    Ok(DbriefType::Int(32)) // Default to 32-bit
                }
            }
            "uint" | "Uint" => {
                if self.peek() == Some('[') {
                    self.consume('[')?;
                    let size: usize = self.parse_number::<usize>()?;
                    self.consume(']')?;
                    Ok(DbriefType::UInt(size))
                } else {
                    Ok(DbriefType::UInt(32)) // Default to 32-bit
                }
            }
            "float" => Ok(DbriefType::Float),
            "string" => Ok(DbriefType::String),
            "data" => Ok(DbriefType::Data),
            "addr" => Ok(DbriefType::Addr),
            "regoffset" => Ok(DbriefType::RegOffset),
            "vector" => {
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
            "option" => {
                self.consume('[')?;
                let inner = Box::new(self.parse_type()?);
                self.consume(']')?;
                Ok(DbriefType::Option(inner))
            }
            "fn" => {
                self.consume('(')?;
                let mut param_types = Vec::new();
                if self.peek() != Some(')') {
                    loop {
                        param_types.push(self.parse_type()?);
                        self.skip_whitespace();
                        if self.peek() == Some(',') {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                self.consume(')')?;
                self.skip_whitespace();
                // consume ->
                if self.starts_with("->") {
                    self.pos += 2;
                } else {
                    return Err("Expected '->' in function type".to_string());
                }
                let return_type = Box::new(self.parse_type()?);
                Ok(DbriefType::Fn(param_types, return_type))
            }
            "trigger" => {
                self.consume('(')?;
                let inner = Box::new(self.parse_type()?);
                self.consume(')')?;
                Ok(DbriefType::Trigger(inner))
            }
            "result" => {
                self.consume('[')?;
                let success = Box::new(self.parse_type()?);
                self.skip_whitespace();
                self.consume(',')?;
                let error = Box::new(self.parse_type()?);
                self.consume(']')?;
                Ok(DbriefType::Result(success, error))
            }
            _ => Ok(DbriefType::Named(tok)),
        }
    }


    fn parse_literal(&mut self) -> Result<DbriefLiteral, String> {
        match self.peek() {
            Some('t') | Some('T') if self.starts_with("true") || self.starts_with("TRUE") => {
                if self.starts_with("true") { self.pos += 4; } else { self.pos += 4; }
                Ok(DbriefLiteral::Bool(true))
            }
            Some('f') | Some('F') if self.starts_with("false") || self.starts_with("FALSE") => {
                if self.starts_with("false") { self.pos += 5; } else { self.pos += 5; }
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
            Some('t') | Some('T') if self.starts_with("true") || self.starts_with("TRUE") => {
                if self.starts_with("true") { self.pos += 4; } else { self.pos += 4; }
                Ok(DbriefExpr::Bool(true))
            }
            Some('f') | Some('F') if self.starts_with("false") || self.starts_with("FALSE") => {
                if self.starts_with("false") { self.pos += 5; } else { self.pos += 5; }
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

pub fn parse_dbvs(input: &str) -> Result<DbvsProgram, String> {
    let mut parser = DbriefParser::new(input.to_string());
    let mut program = parser.parse()?;

    Ok(DbvsProgram {
        imports: program.imports,
        registers: program.registers,
        structs: program.structs,
        enums: program.enums,
        services: program.services,
        aliases: program.aliases,
        depends: program.depends,
    })
}

pub fn parse_dbvl(input: &str) -> Result<DbvlProgram, String> {
    let mut parser = DbriefParser::new(input.to_string());
    let mut program = parser.parse()?;
    
    let mut records = Vec::new();
    let mut operations = Vec::new();
    
    for record in program.records {
        records.push(DbvlRecord {
            address: record.address,
            fields: record.fields,
        });
    }
    
    Ok(DbvlProgram {
        imports: program.imports,
        records,
        operations,
        depends: program.depends,
    })
}

#[cfg(test)]
mod dbvs_tests {
    use super::*;

    #[test]
    fn test_parse_dbvs_schema() {
        let input = r#"
            REGISTER @1: Vector[Person];
            
            STRUCT Person {
                name: String;
                age: UInt[8];
                role: String;
            }
            
            ALIAS status_reg: UInt[32];
            ALIAS led: Bool;
        "#;
        let result = parse_dbvs(input);
        assert!(result.is_ok(), "Failed to parse dbvs: {:?}", result);
        let program = result.unwrap();
        assert_eq!(program.registers.len(), 1);
        assert_eq!(program.structs.len(), 1);
        assert_eq!(program.aliases.len(), 2);
    }

    #[test]
    fn test_dbvs_aliases_are_declarations_only() {
        let input = r#"
            ALIAS status_reg: UInt[32];
            ALIAS led: Bool;
        "#;
        let result = parse_dbvs(input);
        assert!(result.is_ok());
        let program = result.unwrap();

        for alias in &program.aliases {
            assert!(alias.address.is_none(), "dbvs aliases should not have addresses");
        }
    }

    #[test]
    fn test_parse_service_basic() {
        let input = r#"
            SERVICE ImageClassifier {
                INPUT img_data: Vector[UInt[8], 4096];
                OUTPUT label: String;
                OUTPUT confidence: Float;
            }
        "#;
        let result = parse_dbvs(input);
        assert!(result.is_ok(), "Failed to parse service: {:?}", result);
        let program = result.unwrap();
        assert_eq!(program.services.len(), 1);

        let service = &program.services[0];
        assert_eq!(service.name, "ImageClassifier");
        assert_eq!(service.fields.len(), 3);

        let input_field = &service.fields[0];
        assert_eq!(input_field.direction, "INPUT");
        assert_eq!(input_field.name, "img_data");

        let output_field = &service.fields[1];
        assert_eq!(output_field.direction, "OUTPUT");
        assert_eq!(output_field.name, "label");
    }

    #[test]
    fn test_parse_service_multiple() {
        let input = r#"
            SERVICE WeatherApi {
                INPUT city: String;
                OUTPUT temperature: Float;
                OUTPUT humidity: Float;
            }

            SERVICE ImageClassifier {
                INPUT img_data: Vector[UInt[8], 4096];
                OUTPUT label: String;
            }
        "#;
        let result = parse_dbvs(input);
        assert!(result.is_ok(), "Failed to parse services: {:?}", result);
        let program = result.unwrap();
        assert_eq!(program.services.len(), 2);
        assert_eq!(program.services[0].name, "WeatherApi");
        assert_eq!(program.services[1].name, "ImageClassifier");
    }
}

#[cfg(test)]
mod dbvl_tests {
    use super::*;

    #[test]
    fn test_parse_dbvl_records() {
        let input = "@1 { name: \"Alice\"; age: 30; }";
        let result = parse_dbvl(input);
        assert!(result.is_ok(), "Failed to parse dbvl: {:?}", result);
        let program = result.unwrap();
        assert_eq!(program.records.len(), 1);
        
        // Verify the record fields
        let record = &program.records[0];
        assert!(matches!(record.address, DbriefAddress::Numeric(1)));
    }

    #[test]
    fn test_parse_dbvl_with_hex_address() {
        let input = "@0xFF5E0000 { name: \"LED\"; state: \"off\"; }";
        let result = parse_dbvl(input);
        assert!(result.is_ok(), "Failed to parse dbvl: {:?}", result);
        let program = result.unwrap();
        assert_eq!(program.records.len(), 1);
        
        if let DbriefAddress::Hex(addr) = program.records[0].address {
            assert_eq!(addr, 0xFF5E0000);
        } else {
            panic!("Expected hex address");
        }
    }

    #[test]
    fn test_parse_dbvl_multiple_records() {
        let input = "@1 { name: \"Alice\"; }\n@2 { name: \"Bob\"; }";
        let result = parse_dbvl(input);
        assert!(result.is_ok(), "Failed to parse dbvl: {:?}", result);
        let program = result.unwrap();
        assert_eq!(program.records.len(), 2);
    }

    #[test]
    fn test_dbvl_to_json() {
        use crate::dbrief::dbvl_to_json;
        
        let input = "@1 { name: \"Alice\"; age: 30; }\n@2 { name: \"Bob\"; age: 25; }";
        let program = parse_dbvl(input).unwrap();
        
        let json = dbvl_to_json(&program, false).unwrap();
        assert!(json.contains("records"));
        assert!(json.contains("Alice"));
        assert!(json.contains("Bob"));
        
        let pretty_json = dbvl_to_json(&program, true).unwrap();
        assert!(pretty_json.contains("  \"records\""));
        assert!(pretty_json.len() > json.len());
    }
}