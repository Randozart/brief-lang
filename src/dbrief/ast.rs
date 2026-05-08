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
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum DbriefAddress {
    Numeric(u64),
    Hex(u64),
    Auto,
    Named(String),
    Remote(RemoteSpec),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct RemoteSpec {
    pub protocol: String,
    pub location: String,
    pub register: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum DbriefLiteral {
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    String(String),
    Struct(HashMap<String, DbriefLiteral>),
    Vector(Vec<DbriefLiteral>),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DbriefRecord {
    pub address: DbriefAddress,
    pub fields: Vec<(String, DbriefLiteral)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DbriefRegister {
    pub address: DbriefAddress,
    pub name: Option<String>,
    pub register_type: DbriefType,
    pub check: Option<DbriefContract>,
    pub location: Option<String>,
    pub target: Option<String>,
    pub description: Option<String>,
    pub input_params: Vec<(String, DbriefType)>,
    pub output_type: Option<DbriefType>,
    pub error_type: Option<DbriefType>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
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

#[derive(Debug, Clone, Serialize)]
pub struct DbriefAlias {
    pub name: String,
    pub alias_type: DbriefType,
    pub address: Option<DbriefAddress>,
    pub optional: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DbriefContract {
    pub conditions: Vec<DbriefExpr>,
}

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Serialize)]
pub enum BinaryOp {
    Add, Sub, Mul, Div, Mod,
    Eq, Ne, Lt, Le, Gt, Ge,
    And, Or,
    Contains,
}

#[derive(Debug, Clone, Serialize)]
pub enum UnaryOp {
    Not,
    Neg,
}

#[derive(Debug, Clone, Serialize)]
pub struct DbriefRule {
    pub head: RuleHead,
    pub body: Vec<RuleBody>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuleHead {
    pub name: String,
    pub params: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub enum RuleBody {
    Fact(String, Vec<(String, DbriefExpr)>),
    Not(Box<RuleBody>),
    And(Box<RuleBody>, Box<RuleBody>),
    Or(Box<RuleBody>, Box<RuleBody>),
}

#[derive(Debug, Clone, Serialize)]
pub enum DbriefQuery {
    Pipeline(QueryPipeline),
    Bracket(DbriefAddress, DbriefExpr),
    Logical(DbriefAddress, DbriefExpr),
}

#[derive(Debug, Clone, Serialize)]
pub struct QueryPipeline {
    pub source: DbriefAddress,
    pub operations: Vec<QueryOp>,
}

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Serialize)]
pub struct DbriefStruct {
    pub name: String,
    pub fields: Vec<(String, DbriefType)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DbriefEnum {
    pub name: String,
    pub variants: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DbriefServiceField {
    pub direction: String,
    pub name: String,
    pub field_type: DbriefType,
}

#[derive(Debug, Clone, Serialize)]
pub struct DbriefService {
    pub name: String,
    pub fields: Vec<DbriefServiceField>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DbriefDependency {
    pub name: String,
    pub version_constraint: Option<String>,
    pub platform: Vec<String>,
    pub features: Vec<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DbriefProgram {
    pub imports: Vec<ImportStmt>,
    pub registers: Vec<DbriefRegister>,
    pub aliases: Vec<DbriefAlias>,
    pub structs: Vec<DbriefStruct>,
    pub enums: Vec<DbriefEnum>,
    pub services: Vec<DbriefService>,
    pub rules: Vec<DbriefRule>,
    pub records: Vec<DbriefRecord>,
    pub checks: Vec<DbriefContract>,
    pub depends: Vec<DbriefDependency>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DbvsProgram {
    pub imports: Vec<ImportStmt>,
    pub registers: Vec<DbriefRegister>,
    pub structs: Vec<DbriefStruct>,
    pub enums: Vec<DbriefEnum>,
    pub services: Vec<DbriefService>,
    pub aliases: Vec<DbriefAlias>,
    pub depends: Vec<DbriefDependency>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DbvlProgram {
    pub imports: Vec<ImportStmt>,
    pub records: Vec<DbvlRecord>,
    pub operations: Vec<DbvlOperation>,
    pub depends: Vec<DbriefDependency>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DbvlRecord {
    pub address: DbriefAddress,
    pub fields: Vec<(String, DbriefLiteral)>,
}

#[derive(Debug, Clone, Serialize)]
pub enum DbvlOperation {
    Insert { address: DbriefAddress, fields: Vec<(String, DbriefLiteral)> },
    Update { address: DbriefAddress, filter: Option<DbriefExpr>, set: Vec<(String, DbriefLiteral)> },
    Delete { address: DbriefAddress, filter: Option<DbriefExpr> },
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportStmt {
    pub path: String,
    pub alias: Option<String>,
}