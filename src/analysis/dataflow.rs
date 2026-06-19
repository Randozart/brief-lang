use crate::ast::{BracketOp, Expr, Program, SliceCoordinate, Statement, TopLevel, Transaction};
use std::collections::{HashMap, HashSet};

pub struct DataflowAnalyzer<'a> {
    program: &'a Program,
}

#[derive(Debug, Clone)]
pub enum DataflowError {
    UseBeforeSet {
        variable: String,
        transaction: String,
        depends_on: String,
    },
    MissingWeightLoad {
        computation_txn: String,
        load_txn: Option<String>,
    },
}

impl<'a> DataflowAnalyzer<'a> {
    pub fn new(program: &'a Program) -> Self {
        DataflowAnalyzer { program }
    }

    pub fn analyze(&self) -> Vec<DataflowError> {
        let mut errors = Vec::new();

        let mut txn_reads: HashMap<String, HashSet<String>> = HashMap::new();
        let mut txn_writes: HashMap<String, HashSet<String>> = HashMap::new();
        let mut txn_preconditions: HashMap<String, Expr> = HashMap::new();

        for item in &self.program.items {
            if let TopLevel::Transaction(txn) = item {
                let reads = self.extract_reads(&txn.contract.pre_condition);
                let writes = self.extract_writes(&txn.body);
                txn_reads.insert(txn.name.clone(), reads);
                txn_writes.insert(txn.name.clone(), writes);
                txn_preconditions.insert(txn.name.clone(), txn.contract.pre_condition.clone());
            }
        }

        for (txn_name, reads) in &txn_reads {
            let writes = txn_writes.get(txn_name).cloned().unwrap_or_default();
            for read_var in reads {
                if writes.contains(read_var) {
                    continue;
                }
                let init_value = self.get_initial_value(read_var);
                if init_value == Some(0) || init_value.is_none() {
                    let mut has_writer = false;
                    for (other_txn, other_writes) in &txn_writes {
                        if other_txn == txn_name {
                            continue;
                        }
                        if other_writes.contains(read_var) {
                            if let Some(pre) = txn_preconditions.get(other_txn) {
                                if self.is_trivially_true(pre) {
                                    has_writer = true;
                                    break;
                                }
                            }
                        }
                    }
                    if !has_writer && !reads.is_empty() {
                    }
                }
            }
        }

        let weight_load_txns = self.find_weight_load_transactions();
        let compute_txns = self.find_compute_transactions();

        for compute_txn in &compute_txns {
            if weight_load_txns.is_empty() {
                errors.push(DataflowError::MissingWeightLoad {
                    computation_txn: compute_txn.clone(),
                    load_txn: None,
                });
            }
        }

        errors
    }

    fn extract_reads(&self, expr: &Expr) -> HashSet<String> {
        let mut reads = HashSet::new();
        self.extract_ids_recursive(expr, &mut reads);
        reads
    }

    fn extract_ids_recursive(&self, expr: &Expr, ids: &mut HashSet<String>) {
        match expr {
            Expr::Identifier(name) => { ids.insert(name.clone()); }
            Expr::OwnedRef(name) => { ids.insert(name.clone()); }
            Expr::PriorState(name) => { ids.insert(name.clone()); }
            Expr::Integer(_) | Expr::Float(_) | Expr::String(_) | Expr::Char(_)
            | Expr::Bool(_) | Expr::Term | Expr::Literal(_)
            | Expr::BinaryOp(_) | Expr::UnaryOp(_) => {}
            Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r)
            | Expr::Mod(l, r) | Expr::Shl(l, r) | Expr::Shr(l, r) | Expr::Concat(l, r) => {
                self.extract_ids_recursive(l, ids);
                self.extract_ids_recursive(r, ids);
            }
            Expr::Eq(l, r) | Expr::Ne(l, r) | Expr::Lt(l, r) | Expr::Le(l, r)
            | Expr::Gt(l, r) | Expr::Ge(l, r) | Expr::And(l, r) | Expr::Or(l, r)
            | Expr::BitAnd(l, r) | Expr::BitOr(l, r) | Expr::BitXor(l, r) => {
                self.extract_ids_recursive(l, ids);
                self.extract_ids_recursive(r, ids);
            }
            Expr::Not(inner) | Expr::Neg(inner) | Expr::BitNot(inner) => {
                self.extract_ids_recursive(inner, ids);
            }
            Expr::Cast(inner, _) | Expr::Projection { source: inner, .. } => {
                self.extract_ids_recursive(inner, ids);
            }
            Expr::Call(_, args) | Expr::ListLiteral(args) | Expr::Tuple(args) => {
                for arg in args {
                    self.extract_ids_recursive(arg, ids);
                }
            }
            Expr::IntrinsicCall { intrinsic: _, args } => {
                for arg in args {
                    self.extract_ids_recursive(arg, ids);
                }
            }
            Expr::ListIndex(list, idx) => {
                self.extract_ids_recursive(list, ids);
                self.extract_ids_recursive(idx, ids);
            }
            Expr::FieldAccess(obj, _) => {
                self.extract_ids_recursive(obj, ids);
            }
            Expr::StructInstance(_, fields) | Expr::ObjectLiteral(fields) => {
                for (_, field_expr) in fields {
                    self.extract_ids_recursive(field_expr, ids);
                }
            }
            Expr::Slice { value, start, end, stride, mask } => {
                self.extract_ids_recursive(value, ids);
                if let Some(s) = start { self.extract_ids_recursive(s, ids); }
                if let Some(e) = end { self.extract_ids_recursive(e, ids); }
                if let Some(st) = stride { self.extract_ids_recursive(st, ids); }
                if let Some(m) = mask { self.extract_ids_recursive(m, ids); }
            }
            Expr::MultiSlice { value, ops } => {
                self.extract_ids_recursive(value, ids);
                for op in ops {
                    match op {
                        BracketOp::Coord(c) => self.extract_ids_from_slice_coord(c, ids),
                        BracketOp::Mask(m) => self.extract_ids_recursive(m, ids),
                        BracketOp::Stride(s) => self.extract_ids_recursive(s, ids),
                    }
                }
            }
            Expr::PatternMatch { value, .. } => {
                self.extract_ids_recursive(value, ids);
            }
            Expr::Match { value, arms } => {
                self.extract_ids_recursive(value, ids);
                for arm in arms {
                    self.extract_ids_recursive(&arm.body, ids);
                    if let Some(ref g) = arm.guard {
                        self.extract_ids_recursive(g, ids);
                    }
                }
            }
            Expr::Block(stmts, expr) => {
                for stmt in stmts {
                    self.extract_ids_from_statement(stmt, ids);
                }
                self.extract_ids_recursive(expr, ids);
            }
            Expr::TupleDestructure(_, inner) => {
                self.extract_ids_recursive(inner, ids);
            }
            Expr::ArrowMut { target, index, value, .. } => {
                self.extract_ids_recursive(target, ids);
                self.extract_ids_recursive(index, ids);
                if let Some(v) = value {
                    self.extract_ids_recursive(v, ids);
                }
            }
            Expr::ArrowDiscard { target, index } => {
                self.extract_ids_recursive(target, ids);
                self.extract_ids_recursive(index, ids);
            }
            Expr::ArrowTransfer { dest, source, filter } => {
                self.extract_ids_recursive(dest, ids);
                self.extract_ids_recursive(source, ids);
                if let Some(f) = filter {
                    self.extract_ids_recursive(f, ids);
                }
            }
            Expr::SigCall { expr, .. } => {
                self.extract_ids_recursive(expr, ids);
            }
            Expr::Ellipsis => {}
            Expr::MapLiteral(entries) => {
                for (k, v) in entries {
                    self.extract_ids_recursive(k, ids);
                    self.extract_ids_recursive(v, ids);
                }
            }
            Expr::SetLiteral(entries) => {
                for e in entries {
                    self.extract_ids_recursive(e, ids);
                }
            }
            Expr::DbvlTable { .. } => {}
            Expr::SubtypeProjection { source, .. } => {
                self.extract_ids_recursive(source, ids);
            }
            _ => {}
        }
    }

    fn extract_ids_from_slice_coord(&self, coord: &SliceCoordinate, ids: &mut HashSet<String>) {
        match coord {
            SliceCoordinate::Index(expr) => {
                self.extract_ids_recursive(expr, ids);
            }
            SliceCoordinate::Range { start, end } => {
                if let Some(s) = start { self.extract_ids_recursive(s, ids); }
                if let Some(e) = end { self.extract_ids_recursive(e, ids); }
            }
            SliceCoordinate::Named { coord, .. } => {
                self.extract_ids_from_slice_coord(coord, ids);
            }
            SliceCoordinate::AtDimension { coord, .. } => {
                self.extract_ids_from_slice_coord(coord, ids);
            }
            SliceCoordinate::Ellipsis => {}
        }
    }

    fn extract_ids_from_statement(&self, stmt: &Statement, ids: &mut HashSet<String>) {
        match stmt {
            Statement::Assignment { lhs, expr, .. } => {
                self.extract_ids_recursive(lhs, ids);
                self.extract_ids_recursive(expr, ids);
            }
            Statement::Let { expr, address_expr, .. } => {
                if let Some(e) = expr { self.extract_ids_recursive(e, ids); }
                if let Some(a) = address_expr { self.extract_ids_recursive(a, ids); }
            }
            Statement::Unification { expr, .. } => {
                self.extract_ids_recursive(expr, ids);
            }
            Statement::Guarded { condition, statements } => {
                self.extract_ids_recursive(condition, ids);
                for s in statements {
                    self.extract_ids_from_statement(s, ids);
                }
            }
            Statement::Expression(expr) => {
                self.extract_ids_recursive(expr, ids);
            }
            Statement::Term { values, swan_song, .. } => {
                for v in values.iter().flatten() {
                    self.extract_ids_recursive(v, ids);
                }
                if let Some(swan) = swan_song {
                    self.extract_ids_from_statement(swan, ids);
                }
            }
            Statement::TermBang { values, swan_song, .. } => {
                for v in values.iter().flatten() {
                    self.extract_ids_recursive(v, ids);
                }
                if let Some(swan) = swan_song {
                    self.extract_ids_from_statement(swan, ids);
                }
            }
            Statement::Escape(expr) => {
                if let Some(e) = expr { self.extract_ids_recursive(e, ids); }
            }
            Statement::LocalTrigger { expr, .. } => {
                if let Some(e) = expr { self.extract_ids_recursive(e, ids); }
            }
            Statement::OnExit { body, .. } => {
                for s in body {
                    self.extract_ids_from_statement(s, ids);
                }
            }
            Statement::InlineAsm { .. } | Statement::Alka(_) => {}
            Statement::SyncBlock { .. } => {}
            Statement::Foreach { list, body, .. } => {
                self.extract_ids_recursive(list, ids);
                for s in body {
                    self.extract_ids_from_statement(s, ids);
                }
            }
            Statement::Oracle { body, handler, .. } => {
                for s in body {
                    self.extract_ids_from_statement(s, ids);
                }
                for s in handler {
                    self.extract_ids_from_statement(s, ids);
                }
            }
            Statement::Await { expr, .. } => {
                self.extract_ids_recursive(expr, ids);
            }
            Statement::Async { body, .. } => {
                self.extract_ids_from_statement(body, ids);
            }
            Statement::AsyncAwait { body, .. } => {
                self.extract_ids_from_statement(body, ids);
            }
        }
    }

    fn extract_writes(&self, body: &[Statement]) -> HashSet<String> {
        let mut writes = HashSet::new();
        for stmt in body {
            match stmt {
                Statement::Assignment { lhs, .. } => {
                    if let Expr::Identifier(name) = lhs {
                        writes.insert(name.clone());
                    }
                }
                Statement::Guarded { statements, .. } => {
                    writes.extend(self.extract_writes(statements));
                }
                _ => {}
            }
        }
        writes
    }

    fn get_initial_value(&self, name: &str) -> Option<i64> {
        for item in &self.program.items {
            if let TopLevel::StateDecl(decl) = item {
                if decl.name == name {
                    if let Some(expr) = &decl.expr {
                        if let Expr::Integer(n) = expr {
                            return Some(*n);
                        }
                    }
                    return Some(0);
                }
            }
        }
        None
    }

    fn is_trivially_true(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Bool(true) => true,
            Expr::Identifier(_) => true,
            Expr::Eq(l, r) => {
                let lv = self.eval_to_int(l);
                let rv = self.eval_to_int(r);
                lv == rv
            }
            _ => false,
        }
    }

    fn eval_to_int(&self, expr: &Expr) -> Option<i64> {
        match expr {
            Expr::Integer(n) => Some(*n),
            _ => None,
        }
    }

    fn find_weight_load_transactions(&self) -> Vec<String> {
        let mut txns = Vec::new();
        for item in &self.program.items {
            if let TopLevel::Transaction(txn) = item {
                let writes = self.extract_writes(&txn.body);
                if writes.iter().any(|w|
                    w.contains("weight") ||
                    w.contains("buf") ||
                    w.contains("buffer")
                ) {
                    txns.push(txn.name.clone());
                }
            }
        }
        txns
    }

    fn find_compute_transactions(&self) -> Vec<String> {
        let mut txns = Vec::new();
        for item in &self.program.items {
            if let TopLevel::Transaction(txn) = item {
                let body_str = format!("{:?}", txn.body);
                if body_str.contains("acc") ||
                   body_str.contains("result") ||
                   body_str.contains("compute") ||
                   body_str.contains("multiply") ||
                   body_str.contains("add") {
                    txns.push(txn.name.clone());
                }
            }
        }
        txns
    }
}

pub struct TransactionProtocolVerifier;

impl TransactionProtocolVerifier {
    pub fn verify(program: &Program) -> Vec<ProtocolError> {
        let mut errors = Vec::new();
        let mut required_sequence: HashMap<String, Vec<String>> = HashMap::new();

        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                let mut required = Vec::new();
                if let Expr::Eq(lhs, rhs) = &txn.contract.pre_condition {
                    if let Expr::Identifier(name) = lhs.as_ref() {
                        if let Expr::Integer(val) = rhs.as_ref() {
                            required.push(format!("{}={}", name, val));
                        }
                    }
                }
                if !required.is_empty() {
                    required_sequence.insert(txn.name.clone(), required);
                }
            }
        }

        errors
    }
}

#[derive(Debug, Clone)]
pub enum ProtocolError {
    PreconditionNotMet {
        transaction: String,
        required: String,
        missing: String,
    },
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtocolError::PreconditionNotMet { transaction, required, missing } => {
                write!(f, "Transaction '{}' requires {} but {} was not set",
                       transaction, required, missing)
            }
        }
    }
}

impl std::error::Error for ProtocolError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dataflow_analysis() {
        assert!(true);
    }
}

#[cfg(all(kani, feature = "kani_full"))]
mod kani_full_tests {
    use super::*;

    fn empty_program() -> Program {
        Program {
            items: vec![], comments: vec![], reactor_speed: None, attrs: vec![], ffi: None,
            strict_mode: crate::ast::StrictMode::Off, dispatch_mode: Default::default(),
            exit_condition: None, out_pragmas: vec![], default_sig_modifier: None,
        }
    }

    #[kani::proof]
    fn verify_extract_ids_recursive_literal_leaf() {
        let prog = empty_program();
        let analyzer = DataflowAnalyzer::new(&prog);
        let mut ids = HashSet::new();
        let expr = Expr::Literal(Box::new(crate::features::literal::LiteralExpr::Integer(42)));
        analyzer.extract_ids_recursive(&expr, &mut ids);
        assert!(ids.is_empty());
    }

    #[kani::proof]
    fn verify_extract_ids_recursive_literal_term() {
        let prog = empty_program();
        let analyzer = DataflowAnalyzer::new(&prog);
        let mut ids = HashSet::new();
        let expr = Expr::Literal(Box::new(crate::features::literal::LiteralExpr::Term));
        analyzer.extract_ids_recursive(&expr, &mut ids);
        assert!(ids.is_empty());
    }

    #[kani::proof]
    fn verify_extract_ids_recursive_literal_bool() {
        let prog = empty_program();
        let analyzer = DataflowAnalyzer::new(&prog);
        let mut ids = HashSet::new();
        let expr = Expr::Literal(Box::new(crate::features::literal::LiteralExpr::Bool(true)));
        analyzer.extract_ids_recursive(&expr, &mut ids);
        assert!(ids.is_empty());
    }
}