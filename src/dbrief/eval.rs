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
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum QueryResult {
    Records(Vec<DbriefRecord>),
    Value(Value),
    Count(usize),
    Aggregated(AggregationResult),
}

#[derive(Debug, Clone)]
pub enum Value {
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    String(String),
}

#[derive(Debug, Clone)]
pub enum AggregationResult {
    Count(usize),
    Sum(i64),
    Avg(f64),
    Min(i64),
    Max(i64),
    First(DbriefRecord),
    Last(DbriefRecord),
}

pub struct QueryEngine {
    records: HashMap<DbriefAddress, Vec<DbriefRecord>>,
    aliases: HashMap<String, DbriefAddress>,
}

impl QueryEngine {
    pub fn new() -> Self {
        QueryEngine {
            records: HashMap::new(),
            aliases: HashMap::new(),
        }
    }

    pub fn add_record(&mut self, address: DbriefAddress, record: DbriefRecord) {
        self.records.entry(address).or_insert_with(Vec::new).push(record);
    }

    pub fn add_records(&mut self, address: DbriefAddress, records: Vec<DbriefRecord>) {
        self.records.entry(address).or_insert_with(Vec::new).extend(records);
    }

    pub fn register_alias(&mut self, name: String, address: DbriefAddress) {
        self.aliases.insert(name, address);
    }

    fn resolve_address(&self, addr: &DbriefAddress) -> Option<&Vec<DbriefRecord>> {
        match addr {
            DbriefAddress::Numeric(n) => self.records.get(&DbriefAddress::Numeric(*n)),
            DbriefAddress::Hex(h) => self.records.get(&DbriefAddress::Hex(*h)),
            DbriefAddress::Named(name) => {
                self.aliases.get(name).and_then(|a| self.resolve_address(a))
            }
            DbriefAddress::Auto => None,
            DbriefAddress::Remote(_) => None,
        }
    }

    pub fn query(&self, query: &DbriefQuery) -> Result<QueryResult, String> {
        match query {
            DbriefQuery::Pipeline(pipeline) => self.execute_pipeline(pipeline),
            DbriefQuery::Bracket(addr, filter) => self.query_bracket(addr, filter),
            DbriefQuery::Logical(addr, expr) => self.query_logical(addr, expr),
        }
    }

    fn execute_pipeline(&self, pipeline: &QueryPipeline) -> Result<QueryResult, String> {
        let records = self.resolve_address(&pipeline.source)
            .cloned()
            .unwrap_or_default();

        let mut state = QueryResult::Records(records);

        for op in &pipeline.operations {
            state = self.apply_operation(state, op)?;
        }

        Ok(state)
    }

    fn apply_operation(&self, state: QueryResult, op: &QueryOp) -> Result<QueryResult, String> {
        let records = match state {
            QueryResult::Records(recs) => recs,
            _ => return Err("Cannot apply further operations to a terminal query result".to_string()),
        };

        match op {
            QueryOp::Filter(expr) => {
                Ok(QueryResult::Records(records.into_iter()
                    .filter(|r| self.eval_filter(r, expr))
                    .collect()))
            }
            QueryOp::Map(fields) => {
                Ok(QueryResult::Records(records.into_iter()
                    .map(|r| self.project_fields(r, fields))
                    .collect()))
            }
            QueryOp::Count => {
                Ok(QueryResult::Count(records.len()))
            }
            QueryOp::Sum(field) => {
                let total: i64 = records.iter()
                    .filter_map(|r| {
                        if let Value::Int(v) = self.eval_field_access(r, &DbriefExpr::Ident(field.clone())) {
                            Some(v)
                        } else {
                            None
                        }
                    })
                    .sum();
                Ok(QueryResult::Aggregated(AggregationResult::Sum(total)))
            }
            QueryOp::Avg(field) => {
                let vals: Vec<f64> = records.iter()
                    .filter_map(|r| {
                        match self.eval_field_access(r, &DbriefExpr::Ident(field.clone())) {
                            Value::Int(v) => Some(v as f64),
                            Value::Float(v) => Some(v),
                            _ => None,
                        }
                    })
                    .collect();
                if vals.is_empty() {
                    Ok(QueryResult::Aggregated(AggregationResult::Avg(0.0)))
                } else {
                    let avg = vals.iter().sum::<f64>() / vals.len() as f64;
                    Ok(QueryResult::Aggregated(AggregationResult::Avg(avg)))
                }
            }
            QueryOp::Min(field) => {
                let min_val = records.iter()
                    .filter_map(|r| {
                        if let Value::Int(v) = self.eval_field_access(r, &DbriefExpr::Ident(field.clone())) {
                            Some(v)
                        } else {
                            None
                        }
                    })
                    .min()
                    .unwrap_or(0);
                Ok(QueryResult::Aggregated(AggregationResult::Min(min_val)))
            }
            QueryOp::Max(field) => {
                let max_val = records.iter()
                    .filter_map(|r| {
                        if let Value::Int(v) = self.eval_field_access(r, &DbriefExpr::Ident(field.clone())) {
                            Some(v)
                        } else {
                            None
                        }
                    })
                    .max()
                    .unwrap_or(0);
                Ok(QueryResult::Aggregated(AggregationResult::Max(max_val)))
            }
            QueryOp::First => {
                if let Some(first) = records.into_iter().next() {
                    Ok(QueryResult::Aggregated(AggregationResult::First(first)))
                } else {
                    Err("First called on empty record set".to_string())
                }
            }
            QueryOp::Last => {
                if let Some(last) = records.into_iter().last() {
                    Ok(QueryResult::Aggregated(AggregationResult::Last(last)))
                } else {
                    Err("Last called on empty record set".to_string())
                }
            }
            QueryOp::Sort(field, ascending) => {
                let mut sorted = records;
                sorted.sort_by(|a, b| {
                    let cmp = self.compare_field(a, b, field);
                    if *ascending { cmp } else { cmp.reverse() }
                });
                Ok(QueryResult::Records(sorted))
            }
            QueryOp::Limit(n) => {
                Ok(QueryResult::Records(records.into_iter().take(*n).collect()))
            }
            QueryOp::Skip(n) => {
                Ok(QueryResult::Records(records.into_iter().skip(*n).collect()))
            }
            QueryOp::Unique => {
                let mut unique: Vec<DbriefRecord> = Vec::new();
                for r in records {
                    if !unique.iter().any(|u| u == &r) {
                        unique.push(r);
                    }
                }
                Ok(QueryResult::Records(unique))
            }
            QueryOp::Join(_, _) | QueryOp::LeftJoin(_, _) => {
                Err("Join operations are not yet implemented".to_string())
            }
        }
    }

    fn eval_filter(&self, record: &DbriefRecord, expr: &DbriefExpr) -> bool {
        match expr {
            DbriefExpr::BinaryOp(lhs, op, rhs) => {
                let left_val = self.eval_field_access(record, lhs);
                let right_val = self.eval_expr(record, rhs);
                self.compare_values(&left_val, op, &right_val)
            }
            DbriefExpr::Ident(name) => {
                self.get_field(record, name).is_some()
            }
            _ => true,
        }
    }

    fn eval_expr(&self, _record: &DbriefRecord, expr: &DbriefExpr) -> Value {
        match expr {
            DbriefExpr::Bool(b) => Value::Bool(*b),
            DbriefExpr::Int(i) => Value::Int(*i),
            DbriefExpr::UInt(u) => Value::UInt(*u),
            DbriefExpr::Float(f) => Value::Float(*f),
            DbriefExpr::String(s) => Value::String(s.clone()),
            DbriefExpr::Ident(name) => Value::String(name.clone()),
            _ => Value::Bool(false),
        }
    }

    fn eval_field_access(&self, record: &DbriefRecord, expr: &DbriefExpr) -> Value {
        match expr {
            DbriefExpr::Ident(name) => {
                self.get_field(record, name).map(|lit| match lit {
                    DbriefLiteral::Bool(b) => Value::Bool(*b),
                    DbriefLiteral::Int(i) => Value::Int(*i),
                    DbriefLiteral::UInt(u) => Value::UInt(*u),
                    DbriefLiteral::Float(f) => Value::Float(*f),
                    DbriefLiteral::String(s) => Value::String(s.clone()),
                    _ => Value::Bool(false),
                }).unwrap_or(Value::Bool(false))
            }
            _ => Value::Bool(false),
        }
    }

    fn get_field<'a>(&self, record: &'a DbriefRecord, name: &str) -> Option<&'a DbriefLiteral> {
        record.fields.iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v)
    }

    fn compare_values(&self, left: &Value, op: &BinaryOp, right: &Value) -> bool {
        match (left, right) {
            (Value::Int(l), Value::Int(r)) => {
                match op {
                    BinaryOp::Eq => l == r,
                    BinaryOp::Ne => l != r,
                    BinaryOp::Lt => l < r,
                    BinaryOp::Le => l <= r,
                    BinaryOp::Gt => l > r,
                    BinaryOp::Ge => l >= r,
                    _ => false,
                }
            }
            (Value::String(l), Value::String(r)) => {
                match op {
                    BinaryOp::Eq => l == r,
                    BinaryOp::Ne => l != r,
                    _ => false,
                }
            }
            (Value::UInt(l), Value::UInt(r)) => {
                match op {
                    BinaryOp::Eq => l == r,
                    BinaryOp::Ne => l != r,
                    BinaryOp::Lt => l < r,
                    BinaryOp::Le => l <= r,
                    BinaryOp::Gt => l > r,
                    BinaryOp::Ge => l >= r,
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn project_fields(&self, mut record: DbriefRecord, fields: &[DbriefExpr]) -> DbriefRecord {
        if fields.is_empty() {
            return record;
        }
        
        let kept: Vec<(String, DbriefLiteral)> = record.fields.into_iter()
            .filter(|(name, _)| {
                fields.iter().any(|f| {
                    if let DbriefExpr::Ident(n) = f {
                        n == name
                    } else {
                        false
                    }
                })
            })
            .collect();
        
        DbriefRecord {
            address: record.address,
            fields: kept,
        }
    }

    fn compare_field(&self, a: &DbriefRecord, b: &DbriefRecord, field: &str) -> std::cmp::Ordering {
        let a_val = self.get_field(a, field);
        let b_val = self.get_field(b, field);
        
        match (a_val, b_val) {
            (Some(DbriefLiteral::Int(av)), Some(DbriefLiteral::Int(bv))) => av.cmp(bv),
            (Some(DbriefLiteral::UInt(av)), Some(DbriefLiteral::UInt(bv))) => av.cmp(bv),
            (Some(DbriefLiteral::String(av)), Some(DbriefLiteral::String(bv))) => av.cmp(bv),
            _ => std::cmp::Ordering::Equal,
        }
    }

    fn query_bracket(&self, addr: &DbriefAddress, _filter: &DbriefExpr) -> Result<QueryResult, String> {
        let records = self.resolve_address(addr)
            .cloned()
            .unwrap_or_default();
        Ok(QueryResult::Records(records))
    }

    fn query_logical(&self, addr: &DbriefAddress, _expr: &DbriefExpr) -> Result<QueryResult, String> {
        let records = self.resolve_address(addr)
            .cloned()
            .unwrap_or_default();
        Ok(QueryResult::Records(records))
    }

    pub fn count(&self, addr: &DbriefAddress) -> usize {
        self.resolve_address(addr)
            .map(|v| v.len())
            .unwrap_or(0)
    }

    pub fn get(&self, addr: &DbriefAddress, index: usize) -> Option<&DbriefRecord> {
        self.resolve_address(addr)
            .and_then(|v| v.get(index))
    }
}

impl Default for QueryEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_records() -> Vec<DbriefRecord> {
        vec![
            DbriefRecord {
                address: DbriefAddress::Numeric(1),
                fields: vec![
                    ("name".to_string(), DbriefLiteral::String("Alice".to_string())),
                    ("age".to_string(), DbriefLiteral::Int(30)),
                    ("role".to_string(), DbriefLiteral::String("admin".to_string())),
                ],
            },
            DbriefRecord {
                address: DbriefAddress::Numeric(2),
                fields: vec![
                    ("name".to_string(), DbriefLiteral::String("Bob".to_string())),
                    ("age".to_string(), DbriefLiteral::Int(25)),
                    ("role".to_string(), DbriefLiteral::String("user".to_string())),
                ],
            },
            DbriefRecord {
                address: DbriefAddress::Numeric(3),
                fields: vec![
                    ("name".to_string(), DbriefLiteral::String("Charlie".to_string())),
                    ("age".to_string(), DbriefLiteral::Int(35)),
                    ("role".to_string(), DbriefLiteral::String("admin".to_string())),
                ],
            },
        ]
    }

    #[test]
    fn test_count_all_records() {
        let mut engine = QueryEngine::new();
        engine.add_records(DbriefAddress::Numeric(1), create_test_records());
        
        let count = engine.count(&DbriefAddress::Numeric(1));
        assert_eq!(count, 3);
    }

    #[test]
    fn test_count_empty_address() {
        let engine = QueryEngine::new();
        let count = engine.count(&DbriefAddress::Numeric(999));
        assert_eq!(count, 0);
    }

    #[test]
    fn test_get_record_by_index() {
        let mut engine = QueryEngine::new();
        engine.add_records(DbriefAddress::Numeric(1), create_test_records());
        
        let record = engine.get(&DbriefAddress::Numeric(1), 0);
        assert!(record.is_some());
        
        if let Some(r) = record {
            let name = r.fields.iter().find(|(n, _)| n == "name");
            assert!(name.is_some());
            if let Some((_, DbriefLiteral::String(n))) = name {
                assert_eq!(n, "Alice");
            }
        }
    }

    #[test]
    fn test_get_out_of_bounds() {
        let mut engine = QueryEngine::new();
        engine.add_records(DbriefAddress::Numeric(1), create_test_records());
        
        let record = engine.get(&DbriefAddress::Numeric(1), 10);
        assert!(record.is_none());
    }

    #[test]
    fn test_alias_resolution() {
        let mut engine = QueryEngine::new();
        engine.add_records(DbriefAddress::Numeric(1), create_test_records());
        engine.register_alias("users".to_string(), DbriefAddress::Numeric(1));
        
        let count = engine.count(&DbriefAddress::Named("users".to_string()));
        assert_eq!(count, 3);
    }

    #[test]
    fn test_filter_by_age_greater_than() {
        let mut engine = QueryEngine::new();
        engine.add_records(DbriefAddress::Numeric(1), create_test_records());
        
        let filter_expr = DbriefExpr::BinaryOp(
            Box::new(DbriefExpr::Ident("age".to_string())),
            BinaryOp::Gt,
            Box::new(DbriefExpr::Int(28)),
        );
        
        let pipeline = QueryPipeline {
            source: DbriefAddress::Numeric(1),
            operations: vec![QueryOp::Filter(filter_expr)],
        };
        
        let result = engine.execute_pipeline(&pipeline);
        assert!(result.is_ok());
        
        if let Ok(QueryResult::Records(records)) = result {
            assert_eq!(records.len(), 2);
        } else {
            panic!("Expected records result");
        }
    }

    #[test]
    fn test_filter_by_role() {
        let mut engine = QueryEngine::new();
        engine.add_records(DbriefAddress::Numeric(1), create_test_records());
        
        let filter_expr = DbriefExpr::BinaryOp(
            Box::new(DbriefExpr::Ident("role".to_string())),
            BinaryOp::Eq,
            Box::new(DbriefExpr::String("admin".to_string())),
        );
        
        let pipeline = QueryPipeline {
            source: DbriefAddress::Numeric(1),
            operations: vec![QueryOp::Filter(filter_expr)],
        };
        
        let result = engine.execute_pipeline(&pipeline);
        assert!(result.is_ok());
        
        if let Ok(QueryResult::Records(records)) = result {
            assert_eq!(records.len(), 2);
        } else {
            panic!("Expected records result");
        }
    }

    #[test]
    fn test_sort_by_age_ascending() {
        let mut engine = QueryEngine::new();
        engine.add_records(DbriefAddress::Numeric(1), create_test_records());
        
        let pipeline = QueryPipeline {
            source: DbriefAddress::Numeric(1),
            operations: vec![
                QueryOp::Sort("age".to_string(), true),
            ],
        };
        
        let result = engine.execute_pipeline(&pipeline);
        assert!(result.is_ok());
        
        if let Ok(QueryResult::Records(records)) = result {
            assert_eq!(records.len(), 3);
            
            let first_age = records[0].fields.iter()
                .find(|(n, _)| n == "age")
                .map(|(_, v)| v);
            if let Some(DbriefLiteral::Int(age)) = first_age {
                assert_eq!(*age, 25);
            }
        } else {
            panic!("Expected records result");
        }
    }

    #[test]
    fn test_limit_results() {
        let mut engine = QueryEngine::new();
        engine.add_records(DbriefAddress::Numeric(1), create_test_records());
        
        let pipeline = QueryPipeline {
            source: DbriefAddress::Numeric(1),
            operations: vec![
                QueryOp::Limit(2),
            ],
        };
        
        let result = engine.execute_pipeline(&pipeline);
        assert!(result.is_ok());
        
        if let Ok(QueryResult::Records(records)) = result {
            assert_eq!(records.len(), 2);
        } else {
            panic!("Expected records result");
        }
    }

    #[test]
    fn test_skip_results() {
        let mut engine = QueryEngine::new();
        engine.add_records(DbriefAddress::Numeric(1), create_test_records());
        
        let pipeline = QueryPipeline {
            source: DbriefAddress::Numeric(1),
            operations: vec![
                QueryOp::Skip(1),
            ],
        };
        
        let result = engine.execute_pipeline(&pipeline);
        assert!(result.is_ok());
        
        if let Ok(QueryResult::Records(records)) = result {
            assert_eq!(records.len(), 2);
        } else {
            panic!("Expected records result");
        }
    }

    #[test]
    fn test_chained_operations() {
        let mut engine = QueryEngine::new();
        engine.add_records(DbriefAddress::Numeric(1), create_test_records());
        
        let filter_expr = DbriefExpr::BinaryOp(
            Box::new(DbriefExpr::Ident("age".to_string())),
            BinaryOp::Gt,
            Box::new(DbriefExpr::Int(20)),
        );
        
        let pipeline = QueryPipeline {
            source: DbriefAddress::Numeric(1),
            operations: vec![
                QueryOp::Filter(filter_expr),
                QueryOp::Sort("age".to_string(), true),
                QueryOp::Limit(2),
            ],
        };
        
        let result = engine.execute_pipeline(&pipeline);
        assert!(result.is_ok());
        
        if let Ok(QueryResult::Records(records)) = result {
            assert_eq!(records.len(), 2);
        } else {
            panic!("Expected records result");
        }
    }
}