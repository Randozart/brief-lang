use crate::ast::{Contract, Expr, Program, Statement, TopLevel, WatchdogSpec};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub enum WatchdogError {
    UnknownTrigger(String, String),
    NoHandler(String, String),
    NoConflict(String, String, String),
    HandlerDoesNotFalsify(String, String, String),
    HandlerRestoresPrecondition(String, String, String),
}

impl std::fmt::Display for WatchdogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WatchdogError::UnknownTrigger(txn, trigger) => {
                write!(f, "Watchdog in '{}': @{} is not a declared frgn trg", txn, trigger)
            }
            WatchdogError::NoHandler(txn, trigger) => {
                write!(f, "Watchdog in '{}': no transaction guards on @{}", txn, trigger)
            }
            WatchdogError::NoConflict(txn, trigger, var) => {
                write!(f, "Watchdog in '{}': @{} handler writes to '{}' but no conflict with precondition", txn, trigger, var)
            }
            WatchdogError::HandlerDoesNotFalsify(txn, trigger, var) => {
                write!(f, "Watchdog in '{}': @{} handler writes to '{}' but does not falsify precondition", txn, trigger, var)
            }
            WatchdogError::HandlerRestoresPrecondition(txn, trigger, var) => {
                write!(f, "Watchdog in '{}': @{} handler chain restores '{}' — loop would restart", txn, trigger, var)
            }
        }
    }
}

pub type WatchdogResult = Vec<WatchdogError>;

pub fn analyze(program: &Program) -> WatchdogResult {
    let trigger_names: HashSet<String> = program
        .items
        .iter()
        .filter_map(|item| {
            if let TopLevel::Trigger(trg) = item {
                Some(trg.name.clone())
            } else {
                None
            }
        })
        .collect();

    let mut errors = Vec::new();

    for item in &program.items {
        if let TopLevel::Transaction(txn) = item {
            if let Some(ref watchdog) = txn.contract.watchdog {
                if is_trigger_watchdog(watchdog) {
                    let trigger_name = extract_trigger_name(watchdog);
                    let txn_name = txn.name.clone();

                    if let Some(ref trg) = trigger_name {
                        if !trigger_names.contains(trg) {
                            errors.push(WatchdogError::UnknownTrigger(txn_name.clone(), trg.clone()));
                            continue;
                        }

                        let handlers = find_handlers(program, trg);
                        if handlers.is_empty() {
                            errors.push(WatchdogError::NoHandler(txn_name.clone(), trg.clone()));
                            continue;
                        }

                        let handler_writes = collect_handler_writes(program, &handlers);
                        let pre_vars = extract_variables(&txn.contract.pre_condition);

                        let intersecting: Vec<String> = handler_writes
                            .intersection(&pre_vars)
                            .cloned()
                            .collect();

                        if intersecting.is_empty() {
                            if !watchdog.is_required {
                                if txn.contract.pre_condition != Expr::Bool(true) {
                                    let has_convergence = check_convergent_loop(&txn.contract);
                                    if has_convergence {
                                        continue;
                                    }
                                }
                            }
                            let handler_writes_any_pre_var = pre_vars.iter().any(|v| handler_writes.contains(v));
                            if handler_writes_any_pre_var {
                                for var in pre_vars.iter() {
                                    if handler_writes.contains(var) {
                                        errors.push(WatchdogError::NoConflict(txn_name.clone(), trg.clone(), var.clone()));
                                    }
                                }
                            } else if watchdog.is_required {
                                let vars_list: Vec<String> = pre_vars.iter().cloned().collect();
                                let vars_str = vars_list.join(", ");
                                errors.push(WatchdogError::NoConflict(txn_name.clone(), trg.clone(), vars_str));
                            }
                            continue;
                        }

                        for var in &intersecting {
                            let falsifies = check_falsifies(program, &handlers, &txn.contract, var);
                            if !falsifies {
                                errors.push(WatchdogError::HandlerDoesNotFalsify(txn_name.clone(), trg.clone(), var.clone()));
                                continue;
                            }

                            let restores = check_restores(program, &handlers, &txn.contract, var);
                            if restores {
                                errors.push(WatchdogError::HandlerRestoresPrecondition(txn_name.clone(), trg.clone(), var.clone()));
                            }
                        }
                    }
                }
            }
        }
    }

    errors
}

fn is_trigger_watchdog(watchdog: &WatchdogSpec) -> bool {
    matches!(&watchdog.condition, Expr::PriorState(_))
}

fn extract_trigger_name(watchdog: &WatchdogSpec) -> Option<String> {
    if let Expr::PriorState(name) = &watchdog.condition {
        return Some(name.clone());
    }
    None
}

fn find_handlers(program: &Program, trigger_name: &str) -> Vec<String> {
    let mut handlers = Vec::new();
    for item in &program.items {
        if let TopLevel::Transaction(txn) = item {
            if contains_trigger_in_guard(&txn.contract.pre_condition, trigger_name) {
                handlers.push(txn.name.clone());
            }
        }
    }
    handlers
}

fn contains_trigger_in_guard(expr: &Expr, trigger_name: &str) -> bool {
    match expr {
        Expr::PriorState(name) => name == trigger_name,
        Expr::And(left, right) | Expr::Or(left, right) => {
            contains_trigger_in_guard(left, trigger_name)
                || contains_trigger_in_guard(right, trigger_name)
        }
        _ => false,
    }
}

fn collect_handler_writes(program: &Program, handler_names: &[String]) -> HashSet<String> {
    let handler_set: HashSet<String> = handler_names.iter().cloned().collect();
    let mut writes = HashSet::new();

    for item in &program.items {
        if let TopLevel::Transaction(txn) = item {
            if handler_set.contains(&txn.name) {
                collect_writes_from_body(&txn.body, &mut writes);
            }
        }
    }
    writes
}

fn collect_writes_from_body(body: &[Statement], writes: &mut HashSet<String>) {
    for stmt in body {
        match stmt {
            Statement::Assignment { lhs, .. } => {
                if let Expr::Identifier(name) = lhs {
                    writes.insert(name.clone());
                }
            }
            Statement::LocalTrigger { name, .. } => {
                writes.insert(name.clone());
            }
            Statement::Guarded { statements, .. } => {
                collect_writes_from_body(statements, writes);
            }
            Statement::SyncBlock { body: inner } => {
                collect_writes_from_body(inner, writes);
            }
            _ => {}
        }
    }
}

fn extract_variables(expr: &Expr) -> HashSet<String> {
    let mut vars = HashSet::new();
    collect_vars(expr, &mut vars);
    vars
}

fn collect_vars(expr: &Expr, vars: &mut HashSet<String>) {
    match expr {
        Expr::Identifier(name) => {
            vars.insert(name.clone());
        }
        Expr::PriorState(name) | Expr::OwnedRef(name) => {
            vars.insert(name.clone());
        }
        Expr::Add(l, r)
        | Expr::Sub(l, r)
        | Expr::Mul(l, r)
        | Expr::Div(l, r)
        | Expr::Mod(l, r)
        | Expr::Eq(l, r)
        | Expr::Ne(l, r)
        | Expr::Lt(l, r)
        | Expr::Le(l, r)
        | Expr::Gt(l, r)
        | Expr::Ge(l, r)
        | Expr::And(l, r)
        | Expr::Or(l, r)
        | Expr::BitAnd(l, r)
        | Expr::BitOr(l, r)
        | Expr::BitXor(l, r)
        | Expr::Shl(l, r)
        | Expr::Shr(l, r) => {
            collect_vars(l, vars);
            collect_vars(r, vars);
        }
        Expr::Neg(inner) | Expr::Not(inner) | Expr::BitNot(inner) => {
            collect_vars(inner, vars);
        }
        Expr::Call(_, args) => {
            for arg in args {
                collect_vars(arg, vars);
            }
        }
        Expr::Projection { source, .. } => collect_vars(source, vars),
        Expr::ListLiteral(elems) => {
            for elem in elems {
                collect_vars(elem, vars);
            }
        }
        Expr::MapLiteral(entries) => {
            for (k, v) in entries {
                collect_vars(k, vars);
                collect_vars(v, vars);
            }
        }
        Expr::SetLiteral(entries) => {
            for e in entries {
                collect_vars(e, vars);
            }
        }
        _ => {}
    }
}

fn check_falsifies(program: &Program, handler_names: &[String], contract: &Contract, var: &str) -> bool {
    let handler_set: HashSet<String> = handler_names.iter().cloned().collect();

    for item in &program.items {
        if let TopLevel::Transaction(txn) = item {
            if handler_set.contains(&txn.name) {
                if check_falsifies_in_body(&txn.body, contract, var) {
                    return true;
                }
            }
        }
    }
    false
}

fn check_falsifies_in_body(body: &[Statement], contract: &Contract, var: &str) -> bool {
    for stmt in body {
        match stmt {
            Statement::Assignment { lhs, expr, .. } => {
                if let Expr::Identifier(name) = lhs {
                    if name == var {
                        match &contract.pre_condition {
                            Expr::Eq(_, _) | Expr::Ne(_, _) => {
                                if evaluate_literal(expr).is_some() {
                                    return true;
                                }
                            }
                            Expr::Identifier(ident) if ident == var => {
                                if let Expr::Bool(false) = expr {
                                    return true;
                                }
                            }
                            Expr::Not(inner) => {
                                if let Expr::Identifier(ident) = inner.as_ref() {
                                    if ident == var {
                                        if let Expr::Bool(true) = expr {
                                            return true;
                                        }
                                    }
                                }
                            }
                            _ => return true,
                        }
                    }
                }
            }
            Statement::Guarded { statements, .. } => {
                if check_falsifies_in_body(statements, contract, var) {
                    return true;
                }
            }
            Statement::SyncBlock { body: inner } => {
                if check_falsifies_in_body(inner, contract, var) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

fn check_restores(program: &Program, handler_names: &[String], contract: &Contract, var: &str) -> bool {
    let handler_set: HashSet<String> = handler_names.iter().cloned().collect();

    for item in &program.items {
        if let TopLevel::Transaction(txn) = item {
            if handler_set.contains(&txn.name) {
                if check_restores_in_body(&txn.body, contract, var) {
                    return true;
                }
            }
        }
    }
    false
}

fn check_restores_in_body(body: &[Statement], contract: &Contract, var: &str) -> bool {
    let mut found_falsify = false;

    for stmt in body {
        match stmt {
            Statement::Assignment { lhs, expr, .. } => {
                if let Expr::Identifier(name) = lhs {
                    if name == var {
                        if !found_falsify {
                            match &contract.pre_condition {
                                Expr::Identifier(ident) if ident == var => {
                                    if let Expr::Bool(false) = expr {
                                        found_falsify = true;
                                        continue;
                                    }
                                }
                                _ => {
                                    found_falsify = true;
                                    continue;
                                }
                            }
                        } else {
                            match &contract.pre_condition {
                                Expr::Identifier(ident) if ident == var => {
                                    if let Expr::Bool(true) = expr {
                                        return true;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            Statement::Guarded { statements, .. } => {
                if check_restores_in_body(statements, contract, var) {
                    return true;
                }
            }
            Statement::SyncBlock { body: inner } => {
                if check_restores_in_body(inner, contract, var) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

fn evaluate_literal(expr: &Expr) -> Option<bool> {
    match expr {
        Expr::Bool(b) => Some(*b),
        Expr::Integer(n) => Some(*n != 0),
        Expr::Not(inner) => evaluate_literal(inner).map(|v| !v),
        _ => None,
    }
}

fn check_convergent_loop(contract: &Contract) -> bool {
    match (&contract.pre_condition, &contract.post_condition) {
        (Expr::Lt(_, _) | Expr::Le(_, _) | Expr::Gt(_, _) | Expr::Ge(_, _) | Expr::Ne(_, _), 
         Expr::Eq(_, _)) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;

        fn make_program(items: Vec<TopLevel>) -> Program {
        Program {
            items,
            comments: vec![],
            reactor_speed: None,
            attrs: vec![],
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: DispatchMode::Sequential,
            exit_condition: None,
            out_pragmas: vec![],
            default_sig_modifier: None,
                watchdog_defaults: (None, None),
        }
    }

    fn make_trigger(name: &str) -> TopLevel {
        TopLevel::Trigger(TriggerDeclaration {
            name: name.to_string(),
            ty: Type::Bool,
            address: LinkRef::Explicit(0x4000_0000),
            bit_range: None,
            stages: vec![],
            condition: None,
            is_wake: true,
            is_const: false,
            span: None,
        })
    }

    fn make_txn(name: &str, pre: Expr, post: Expr, watchdog: Option<WatchdogSpec>, body: Vec<Statement>) -> TopLevel {
        TopLevel::Transaction(Transaction {
            name: name.to_string(),
            is_reactive: true,
            is_async: false,
            parameters: vec![],
            contract: Contract { pre_condition: pre, post_condition: post, watchdog, span: None },
            body,
            reactor_speed: None,
            span: None,
            is_lambda: false,
            dependencies: vec![],
            attrs: vec![],
            modifiers: vec![],
            variant_bodies: vec![],
            outputs: vec![],
            output_type: None,
        })
    }

    fn assign(name: &str, expr: Expr) -> Statement {
        Statement::Assignment {
            lhs: Expr::Identifier(name.to_string()),
            expr,
            timeout: None,
            modifiers: vec![],
        }
    }

    fn watchdog_spec(trigger: &str, is_required: bool) -> WatchdogSpec {
        WatchdogSpec {
            cycles_bound: None,
            seconds_bound: None,
            is_proven: false,
            condition: Expr::PriorState(trigger.to_string()),
            is_required,
        }
    }

    #[test]
    fn test_unknown_trigger() {
        let program = make_program(vec![
            make_txn("main", Expr::Bool(true), Expr::Bool(true),
                Some(watchdog_spec("nonexistent", true)), vec![]),
        ]);
        let errors = analyze(&program);
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], WatchdogError::UnknownTrigger(..)));
    }

    #[test]
    fn test_no_handler() {
        let program = make_program(vec![
            make_trigger("btn"),
            make_txn("main", Expr::Bool(true), Expr::Bool(true),
                Some(watchdog_spec("btn", true)), vec![]),
        ]);
        let errors = analyze(&program);
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], WatchdogError::NoHandler(..)));
    }

    #[test]
    fn test_handler_writes_no_conflict() {
        let program = make_program(vec![
            make_trigger("btn"),
            make_txn("handler", Expr::PriorState("btn".to_string()), Expr::Bool(true),
                None, vec![assign("unrelated", Expr::Integer(42))]),
            make_txn("main", Expr::Identifier("ready".to_string()), Expr::Bool(true),
                Some(watchdog_spec("btn", true)), vec![]),
        ]);
        let errors = analyze(&program);
        assert_eq!(errors.len(), 1, "Required watchdog: handler writes unrelated var, can't preempt [ready]");
        assert!(matches!(&errors[0], WatchdogError::NoConflict(..)), "Expected NoConflict when handler can't affect precondition");
    }

    #[test]
    fn test_handler_does_not_falsify() {
        let program = make_program(vec![
            make_trigger("btn"),
            make_txn("handler", Expr::PriorState("btn".to_string()), Expr::Bool(true),
                None, vec![assign("ready", Expr::Bool(true))]),
            make_txn("main", Expr::Identifier("ready".to_string()), Expr::Bool(true),
                Some(watchdog_spec("btn", true)), vec![]),
        ]);
        let errors = analyze(&program);
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], WatchdogError::HandlerDoesNotFalsify(..)));
    }

    #[test]
    fn test_handler_falsifies_ok() {
        let program = make_program(vec![
            make_trigger("btn"),
            make_txn("handler", Expr::PriorState("btn".to_string()), Expr::Bool(true),
                None, vec![assign("ready", Expr::Bool(false))]),
            make_txn("main", Expr::Identifier("ready".to_string()), Expr::Bool(true),
                Some(watchdog_spec("btn", true)), vec![]),
        ]);
        let errors = analyze(&program);
        assert_eq!(errors.len(), 0, "Handler sets ready=false, precondition is [ready], should pass");
    }

    #[test]
    fn test_handler_restores_precondition() {
        let program = make_program(vec![
            make_trigger("btn"),
            make_txn("handler", Expr::PriorState("btn".to_string()), Expr::Bool(true),
                None, vec![
                    assign("ready", Expr::Bool(false)),
                    assign("ready", Expr::Bool(true)),
                ]),
            make_txn("main", Expr::Identifier("ready".to_string()), Expr::Bool(true),
                Some(watchdog_spec("btn", true)), vec![]),
        ]);
        let errors = analyze(&program);
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], WatchdogError::HandlerRestoresPrecondition(..)));
    }

    #[test]
    fn test_convergent_loop_skips_optional() {
        let program = make_program(vec![
            make_trigger("btn"),
            make_txn("handler", Expr::PriorState("btn".to_string()), Expr::Bool(true),
                None, vec![assign("unrelated", Expr::Integer(1))]),
            make_txn("main", Expr::Lt(
                Box::new(Expr::Identifier("i".to_string())),
                Box::new(Expr::Integer(10)),
            ), Expr::Eq(
                Box::new(Expr::Identifier("i".to_string())),
                Box::new(Expr::Integer(10)),
            ), Some(watchdog_spec("btn", false)), vec![]),
        ]);
        let errors = analyze(&program);
        assert_eq!(errors.len(), 0, "Optional watchdog on convergent loop should skip");
    }

    #[test]
    fn test_required_watchdog_on_convergent_loop() {
        let program = make_program(vec![
            make_trigger("btn"),
            make_txn("handler", Expr::PriorState("btn".to_string()), Expr::Bool(true),
                None, vec![assign("unrelated", Expr::Integer(1))]),
            make_txn("main", Expr::Lt(
                Box::new(Expr::Identifier("i".to_string())),
                Box::new(Expr::Integer(10)),
            ), Expr::Eq(
                Box::new(Expr::Identifier("i".to_string())),
                Box::new(Expr::Integer(10)),
            ), Some(watchdog_spec("btn", true)), vec![]),
        ]);
        let errors = analyze(&program);
        assert!(errors.len() > 0, "Required watchdog still needs preemptibility proof");
        assert!(matches!(&errors[0], WatchdogError::NoConflict(..)));
    }

    #[test]
    fn test_extract_trigger_name() {
        let spec = watchdog_spec("my_button", true);
        assert_eq!(extract_trigger_name(&spec), Some("my_button".to_string()));

        let spec_non_trigger = WatchdogSpec {
            cycles_bound: None,
            seconds_bound: None,
            is_proven: false,
            condition: Expr::Bool(false),
            is_required: true,
        };
        assert_eq!(extract_trigger_name(&spec_non_trigger), None);
    }

    #[test]
    fn test_is_trigger_watchdog() {
        let spec = watchdog_spec("btn", true);
        assert!(is_trigger_watchdog(&spec));

        let spec_var = WatchdogSpec {
            cycles_bound: None,
            seconds_bound: None,
            is_proven: false,
            condition: Expr::Identifier("timeout".to_string()),
            is_required: true,
        };
        assert!(!is_trigger_watchdog(&spec_var));
    }

    #[test]
    fn test_contains_trigger_in_guard() {
        let guard = Expr::And(
            Box::new(Expr::PriorState("btn".to_string())),
            Box::new(Expr::Identifier("ready".to_string())),
        );
        assert!(contains_trigger_in_guard(&guard, "btn"));
        assert!(!contains_trigger_in_guard(&guard, "other"));

        let guard_or = Expr::Or(
            Box::new(Expr::PriorState("a".to_string())),
            Box::new(Expr::PriorState("b".to_string())),
        );
        assert!(contains_trigger_in_guard(&guard_or, "a"));
        assert!(contains_trigger_in_guard(&guard_or, "b"));
        assert!(!contains_trigger_in_guard(&guard_or, "c"));
    }

    #[test]
    fn test_evaluate_literal() {
        assert_eq!(evaluate_literal(&Expr::Bool(true)), Some(true));
        assert_eq!(evaluate_literal(&Expr::Bool(false)), Some(false));
        assert_eq!(evaluate_literal(&Expr::Integer(0)), Some(false));
        assert_eq!(evaluate_literal(&Expr::Integer(1)), Some(true));
        assert_eq!(evaluate_literal(&Expr::Not(Box::new(Expr::Bool(true)))), Some(false));
        assert_eq!(evaluate_literal(&Expr::Identifier("x".to_string())), None);
    }

    #[test]
    fn test_check_convergent_loop() {
        let contract = Contract {
            pre_condition: Expr::Lt(
                Box::new(Expr::Identifier("i".to_string())),
                Box::new(Expr::Integer(10)),
            ),
            post_condition: Expr::Eq(
                Box::new(Expr::Identifier("i".to_string())),
                Box::new(Expr::Integer(10)),
            ),
            watchdog: None,
            span: None,
        };
        assert!(check_convergent_loop(&contract));

        let contract_flat = Contract {
            pre_condition: Expr::Bool(true),
            post_condition: Expr::Bool(true),
            watchdog: None,
            span: None,
        };
        assert!(!check_convergent_loop(&contract_flat));
    }

    #[test]
    fn test_collect_vars_from_precondition() {
        let pre = Expr::And(
            Box::new(Expr::Identifier("a".to_string())),
            Box::new(Expr::Gt(
                Box::new(Expr::Identifier("b".to_string())),
                Box::new(Expr::Integer(0)),
            )),
        );
        let vars = extract_variables(&pre);
        assert!(vars.contains("a"));
        assert!(vars.contains("b"));
        assert_eq!(vars.len(), 2);
    }

    #[test]
    fn test_collect_handler_writes_guarded() {
        let program = make_program(vec![
            make_trigger("btn"),
            make_txn("handler", Expr::PriorState("btn".to_string()), Expr::Bool(true),
                None, vec![
                    Statement::Guarded {
                        condition: Expr::PriorState("btn".to_string()),
                        statements: vec![assign("ready", Expr::Bool(false))],
                    },
                ]),
            make_txn("main", Expr::Identifier("ready".to_string()), Expr::Bool(true),
                Some(watchdog_spec("btn", true)), vec![]),
        ]);
        let errors = analyze(&program);
        assert_eq!(errors.len(), 0, "Handler writes inside Guarded block should be detected");
    }

    #[test]
    fn test_precondition_not_var_not_handled_as_conflict() {
        let program = make_program(vec![
            make_trigger("btn"),
            make_txn("handler", Expr::PriorState("btn".to_string()), Expr::Bool(true),
                None, vec![assign("counter", Expr::Integer(0))]),
            make_txn("main", Expr::Eq(
                Box::new(Expr::Identifier("counter".to_string())),
                Box::new(Expr::Integer(0)),
            ), Expr::Bool(true),
                Some(watchdog_spec("btn", true)), vec![]),
        ]);
        let errors = analyze(&program);
        assert_eq!(errors.len(), 0, "Handler sets counter=0, precondition is [counter==0], falsifies");
    }

    #[test]
    fn test_no_trigger_watchdog_not_analyzed() {
        let program = make_program(vec![
            make_txn("main", Expr::Identifier("timeout".to_string()), Expr::Bool(true),
                Some(WatchdogSpec {
                    cycles_bound: None,
                    seconds_bound: None,
                    is_proven: false,
                    condition: Expr::Identifier("timeout".to_string()), is_required: true,
                }),
                vec![]),
        ]);
        let errors = analyze(&program);
        assert_eq!(errors.len(), 0, "Non-trigger watchdogs are runtime only, not analyzed");
    }

    #[test]
    fn test_handler_natural_death_without_watchdog() {
        let program = make_program(vec![
            make_txn("main", Expr::Lt(
                Box::new(Expr::Identifier("i".to_string())),
                Box::new(Expr::Integer(5)),
            ), Expr::Eq(
                Box::new(Expr::Identifier("i".to_string())),
                Box::new(Expr::Integer(5)),
            ), None, vec![]),
        ]);
        let errors = analyze(&program);
        assert_eq!(errors.len(), 0, "No watchdog spec means no analysis");
    }
}
