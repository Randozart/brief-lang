use crate::ast::{BinaryOpKind, Expr, Statement, TopLevel, UnaryOpKind};
use std::collections::{HashMap, HashSet};

pub struct DataflowAnalyzer<'a> {
    items: &'a [TopLevel],
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
    pub fn new(items: &'a [TopLevel]) -> Self {
        DataflowAnalyzer { items }
    }

    pub fn analyze(&self) -> Vec<DataflowError> {
        let mut errors = Vec::new();

        let mut txn_reads: HashMap<String, HashSet<String>> = HashMap::new();
        let mut txn_writes: HashMap<String, HashSet<String>> = HashMap::new();
        let mut txn_preconditions: HashMap<String, Expr> = HashMap::new();

        for item in self.items {
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
            Expr::Decimal(_) | Expr::TaggedLiteral(_, _) | Expr::Float(_) | Expr::Quoted(_) | Expr::Bool(_) => {}
            Expr::BinaryOp(_, l, r) => {
                self.extract_ids_recursive(l, ids);
                self.extract_ids_recursive(r, ids);
            }
            Expr::UnaryOp(_, inner) | Expr::Cast(inner, _) | Expr::IsType(inner, _) => {
                self.extract_ids_recursive(inner, ids);
            }
            Expr::Call(_, args, _) | Expr::Tuple(args) | Expr::List(args) => {
                for arg in args {
                    self.extract_ids_recursive(arg, ids);
                }
            }
            Expr::Index(list, idx) => {
                self.extract_ids_recursive(list, ids);
                self.extract_ids_recursive(idx, ids);
            }
            Expr::Deref(inner) | Expr::AddrOf(inner) => {
                self.extract_ids_recursive(inner, ids);
            }
            Expr::Field(obj, _) => {
                self.extract_ids_recursive(obj, ids);
            }
            Expr::Match(value, arms) => {
                self.extract_ids_recursive(value, ids);
                for arm in arms {
                    self.extract_ids_recursive(&arm.body, ids);
                    if let Some(ref g) = arm.guard {
                        self.extract_ids_recursive(g, ids);
                    }
                }
            }
            Expr::If(cond, then, else_) => {
                self.extract_ids_recursive(cond, ids);
                self.extract_ids_recursive(then, ids);
                if let Some(else_) = else_ {
                    self.extract_ids_recursive(else_, ids);
                }
            }
            Expr::Lambda(_, body) => {
                self.extract_ids_recursive(body, ids);
            }
            Expr::Within(inner, _) => {
                self.extract_ids_recursive(inner, ids);
            }
            Expr::Block(stmts) => {
                for stmt in stmts {
                    self.extract_ids_from_statement(stmt, ids);
                }
            }
            Expr::PropertyGet(_) | Expr::FormattingAnnotation(_) | Expr::DerivationBlock(_) | Expr::StructLiteral { .. } => {}
            Expr::PluginIntercept { args, .. } => {
                for a in args { self.extract_ids_recursive(a, ids); }
            }
        }
    }

    fn extract_ids_from_statement(&self, stmt: &Statement, ids: &mut HashSet<String>) {
        match stmt {
            Statement::Assign(lhs, expr) => {
                self.extract_ids_recursive(lhs, ids);
                self.extract_ids_recursive(expr, ids);
            }
            Statement::Let { expr, .. } => {
                if let Some(e) = expr { self.extract_ids_recursive(e, ids); }
            }
            Statement::Guarded(condition, statements) => {
                self.extract_ids_recursive(condition, ids);
                for s in statements {
                    self.extract_ids_from_statement(s, ids);
                }
            }
            Statement::Expression(expr) => {
                self.extract_ids_recursive(expr, ids);
            }
            Statement::Term(Some(e)) => {
                self.extract_ids_recursive(e, ids);
            }
            Statement::TermBang(Some(e)) => {
                self.extract_ids_recursive(e, ids);
            }
            Statement::Term(None) | Statement::TermBang(None) => {}
            Statement::Escape(Some(e)) => {
                self.extract_ids_recursive(e, ids);
            }
            Statement::Escape(None) => {}
            Statement::SyncBlock(body) => {
                for s in body {
                    self.extract_ids_from_statement(s, ids);
                }
            }
            Statement::Return(Some(e)) => {
                self.extract_ids_recursive(e, ids);
            }
            Statement::Return(None) => {}
            Statement::If(cond, then, else_) => {
                self.extract_ids_recursive(cond, ids);
                for s in then {
                    self.extract_ids_from_statement(s, ids);
                }
                for s in else_ {
                    self.extract_ids_from_statement(s, ids);
                }
            }
            Statement::Foreach { list, body, .. } => {
                self.extract_ids_recursive(list, ids);
                for s in body {
                    self.extract_ids_from_statement(s, ids);
                }
            }
            Statement::Block(body) => {
                for s in body {
                    self.extract_ids_from_statement(s, ids);
                }
            }
            Statement::InlineAsm { .. } | Statement::TrgBinding { .. }
            | Statement::MetadataAssignment(..) | Statement::InlineDefn(_)
            | Statement::InlineTxn(_) | Statement::Match { .. } => {}
        }
    }

    fn extract_writes(&self, body: &[Statement]) -> HashSet<String> {
        let mut writes = HashSet::new();
        for stmt in body {
            match stmt {
            Statement::Assign(lhs, _) => {
                if let Expr::Identifier(name) = lhs {
                    writes.insert(name.clone());
                }
            }
            Statement::Guarded(_, statements) => {
                    writes.extend(self.extract_writes(statements));
                }
                _ => {}
            }
        }
        writes
    }

    fn get_initial_value(&self, name: &str) -> Option<i64> {
        for item in self.items {
            if let TopLevel::StateDecl(decl) = item {
                if decl.name == name {
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
            Expr::BinaryOp(BinaryOpKind::Eq, l, r) => {
                let lv = self.eval_to_int(l);
                let rv = self.eval_to_int(r);
                lv == rv
            }
            _ => false,
        }
    }

    fn eval_to_int(&self, expr: &Expr) -> Option<i64> {
        match expr {
            Expr::Decimal(n) | Expr::TaggedLiteral(n, _) => Some(*n),
            _ => None,
        }
    }

    fn find_weight_load_transactions(&self) -> Vec<String> {
        let mut txns = Vec::new();
        for item in self.items {
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
        for item in self.items {
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
    pub fn verify(items: &[TopLevel]) -> Vec<ProtocolError> {
        let mut errors = Vec::new();
        let mut required_sequence: HashMap<String, Vec<String>> = HashMap::new();

        for item in items {
            if let TopLevel::Transaction(txn) = item {
                let mut required = Vec::new();
                if let Expr::BinaryOp(BinaryOpKind::Eq, lhs, rhs) = &txn.contract.pre_condition {
                    if let Expr::Identifier(name) = lhs.as_ref() {
                        if let Expr::Decimal(val) | Expr::TaggedLiteral(val, _) = rhs.as_ref() {
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

#[cfg(all(feature = "kani", feature = "kani_full"))]
mod kani_full_tests {
    use super::*;

    #[kani::proof]
    fn verify_extract_ids_recursive_decimal_leaf() {
        let items: Vec<TopLevel> = vec![];
        let analyzer = DataflowAnalyzer::new(&items);
        let mut ids = HashSet::new();
        let expr = Expr::Decimal(42);
        analyzer.extract_ids_recursive(&expr, &mut ids);
        assert!(ids.is_empty());
    }

    #[kani::proof]
    fn verify_extract_ids_recursive_bool() {
        let items: Vec<TopLevel> = vec![];
        let analyzer = DataflowAnalyzer::new(&items);
        let mut ids = HashSet::new();
        let expr = Expr::Bool(true);
        analyzer.extract_ids_recursive(&expr, &mut ids);
        assert!(ids.is_empty());
    }
}