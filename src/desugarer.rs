use crate::ast::*;

pub struct Desugarer {
    generated_signatures: Vec<Signature>,
    generated_state: Vec<StateDecl>,
}

impl Desugarer {
    pub fn new() -> Self {
        Desugarer {
            generated_signatures: Vec::new(),
            generated_state: Vec::new(),
        }
    }

    pub fn desugar(&mut self, program: &Program) -> Program {
        let mut items = Vec::new();

        for item in &program.items {
            match item {
                TopLevel::Transaction(txn) => {
                    if txn.is_reactive && self.needs_desugaring(txn) {
                        let (new_txn, sigs, state) = self.desugar_reactive_txn(txn);
                        items.extend(state.into_iter().map(TopLevel::StateDecl));
                        items.extend(sigs.into_iter().map(TopLevel::Signature));
                        items.push(TopLevel::Transaction(new_txn));
                    } else {
                        items.push(item.clone());
                    }
                }
                _ => {
                    items.push(item.clone());
                }
            }
        }

        if !self.generated_state.is_empty() {
            for state in self.generated_state.drain(..) {
                if !items.iter().any(|i| {
                    if let TopLevel::StateDecl(s) = i {
                        s.name == state.name
                    } else {
                        false
                    }
                }) {
                    items.insert(0, TopLevel::StateDecl(state));
                }
            }
        }

        if !self.generated_signatures.is_empty() {
            for sig in self.generated_signatures.drain(..) {
                if !items.iter().any(|i| {
                    if let TopLevel::Signature(s) = i {
                        s.name == sig.name
                    } else {
                        false
                    }
                }) {
                    items.insert(0, TopLevel::Signature(sig));
                }
            }
        }

        Program {
            items,
            comments: program.comments.clone(),
        }
    }

    fn needs_desugaring(&self, txn: &Transaction) -> bool {
        if let Expr::Not(inner) = &txn.contract.pre_condition {
            if let Expr::Identifier(name) = &**inner {
                if name == "done"
                    && matches!(&txn.contract.post_condition, Expr::Identifier(n) if n == "done")
                {
                    return self.has_term_with_expression(&txn.body);
                }
            }
        }
        false
    }

    fn has_term_with_expression(&self, body: &[Statement]) -> bool {
        for stmt in body {
            if let Statement::Term(outputs) = stmt {
                if let Some(Some(_)) = outputs.first() {
                    return true;
                }
            }
        }
        false
    }

    fn desugar_reactive_txn(
        &mut self,
        txn: &Transaction,
    ) -> (Transaction, Vec<Signature>, Vec<StateDecl>) {
        let mut sigs = Vec::new();
        let mut state = Vec::new();

        state.push(StateDecl {
            name: "done".to_string(),
            ty: Type::Bool,
            expr: Some(Expr::Bool(false)),
            span: None,
        });

        let mut new_body_items = Vec::new();
        for stmt in &txn.body {
            if let Statement::Term(outputs) = stmt {
                if let Some(Some(expr)) = outputs.first() {
                    let fn_sigs = self.extract_function_call(expr);
                    sigs.extend(fn_sigs);

                    new_body_items.push(Statement::Expression(expr.clone()));
                    new_body_items.push(Statement::Assignment {
                        is_owned: true,
                        name: "done".to_string(),
                        expr: Expr::Bool(true),
                    });
                    new_body_items.push(Statement::Term(vec![]));
                    continue;
                }
            }
            new_body_items.push(stmt.clone());
        }

        let new_txn = Transaction {
            is_async: txn.is_async,
            is_reactive: txn.is_reactive,
            name: txn.name.clone(),
            contract: Contract {
                pre_condition: Expr::Not(Box::new(Expr::Identifier("done".to_string()))),
                post_condition: Expr::Identifier("done".to_string()),
                span: None,
            },
            body: new_body_items,
            span: None,
        };

        (new_txn, sigs, state)
    }

    fn extract_function_call(&mut self, expr: &Expr) -> Vec<Signature> {
        if let Expr::Call(name, args) = expr {
            let input_types: Vec<Type> =
                args.iter().map(|_| Type::Custom("_".to_string())).collect();

            if !self.generated_signatures.iter().any(|s| s.name == *name) {
                let sig = Signature {
                    name: name.clone(),
                    input_types: input_types.clone(),
                    result_type: ResultType::Projection(vec![Type::Bool]),
                    source: None,
                    alias: None,
                };
                self.generated_signatures.push(Signature {
                    name: name.clone(),
                    input_types,
                    result_type: ResultType::TrueAssertion,
                    source: None,
                    alias: None,
                });
                return vec![sig];
            }
        }
        vec![]
    }
}

impl Default for Desugarer {
    fn default() -> Self {
        Self::new()
    }
}
