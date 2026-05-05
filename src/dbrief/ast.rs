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

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DbriefAddress {
    Numeric(u64),
    Hex(u64),
    Auto,
    Named(String),
    Remote(RemoteSpec),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RemoteSpec {
    pub protocol: String,
    pub location: String,
    pub register: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DbriefLiteral {
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    String(String),
    Struct(HashMap<String, DbriefLiteral>),
    Vector(Vec<DbriefLiteral>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct DbriefRecord {
    pub address: DbriefAddress,
    pub fields: Vec<(String, DbriefLiteral)>,
}

#[derive(Debug, Clone)]
pub struct DbriefRegister {
    pub address: DbriefAddress,
    pub name: Option<String>,
    pub register_type: DbriefType,
    pub check: Option<DbriefContract>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DbriefType {
    Bool,
    Int(usize),
    UInt(usize),
    Float,
    String,
    Data,
    Addr,
    RegOffset,
    Vector(Box<DbriefType>, Option<usize>),
    Option(Box<DbriefType>),
    Result(Box<DbriefType>, Box<DbriefType>),
    Named(String),
    Struct(Vec<(String, DbriefType)>),
    Enum(Vec<String>),
}

#[derive(Debug, Clone)]
pub struct DbriefAlias {
    pub name: String,
    pub alias_type: DbriefType,
    pub address: Option<DbriefAddress>,
    pub optional: bool,
}

#[derive(Debug, Clone)]
pub struct DbriefContract {
    pub conditions: Vec<DbriefExpr>,
}

#[derive(Debug, Clone)]
pub enum DbriefExpr {
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    String(String),
    Ident(String),
    BinaryOp(Box<DbriefExpr>, BinaryOp, Box<DbriefExpr>),
    UnaryOp(UnaryOp, Box<DbriefExpr>),
    FieldAccess(Box<DbriefExpr>, String),
    Index(Box<DbriefExpr>, Box<DbriefExpr>),
    Call(String, Vec<DbriefExpr>),
}

#[derive(Debug, Clone)]
pub enum BinaryOp {
    Add, Sub, Mul, Div, Mod,
    Eq, Ne, Lt, Le, Gt, Ge,
    And, Or,
    Contains,
}

#[derive(Debug, Clone)]
pub enum UnaryOp {
    Not,
    Neg,
}

#[derive(Debug, Clone)]
pub struct DbriefRule {
    pub head: RuleHead,
    pub body: Vec<RuleBody>,
}

#[derive(Debug, Clone)]
pub struct RuleHead {
    pub name: String,
    pub params: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum RuleBody {
    Fact(String, Vec<(String, DbriefExpr)>),
    Not(Box<RuleBody>),
    And(Box<RuleBody>, Box<RuleBody>),
    Or(Box<RuleBody>, Box<RuleBody>),
}

#[derive(Debug, Clone)]
pub enum DbriefQuery {
    Pipeline(QueryPipeline),
    Bracket(DbriefAddress, DbriefExpr),
    Logical(DbriefAddress, DbriefExpr),
}

#[derive(Debug, Clone)]
pub struct QueryPipeline {
    pub source: DbriefAddress,
    pub operations: Vec<QueryOp>,
}

#[derive(Debug, Clone)]
pub enum QueryOp {
    Filter(DbriefExpr),
    Map(Vec<DbriefExpr>),
    Count,
    Sum(String),
    Avg(String),
    Min(String),
    Max(String),
    First,
    Last,
    Sort(String, bool),
    Limit(usize),
    Skip(usize),
    Unique,
    Join(DbriefAddress, String),
    LeftJoin(DbriefAddress, String),
}

#[derive(Debug, Clone)]
pub struct DbriefStruct {
    pub name: String,
    pub fields: Vec<(String, DbriefType)>,
}

#[derive(Debug, Clone)]
pub struct DbriefEnum {
    pub name: String,
    pub variants: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct DbriefProgram {
    pub imports: Vec<ImportStmt>,
    pub registers: Vec<DbriefRegister>,
    pub aliases: Vec<DbriefAlias>,
    pub structs: Vec<DbriefStruct>,
    pub enums: Vec<DbriefEnum>,
    pub rules: Vec<DbriefRule>,
    pub records: Vec<DbriefRecord>,
    pub checks: Vec<DbriefContract>,
}

#[derive(Debug, Clone)]
pub struct ImportStmt {
    pub path: String,
    pub alias: Option<String>,
}