use crate::interpreter::Interpreter;
use crate::ast::Program;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct PgoProfile {
    pub branch_counts: HashMap<String, (u64, u64)>,
}

pub fn has_pgo_candidate(profile: &PgoProfile, min_skew_ratio: u64) -> bool {
    profile.branch_counts.values().any(|&(t, f)| {
        if t == 0 && f == 0 {
            return false;
        }
        if t == 0 || f == 0 {
            return true;
        }
        let ratio = if t > f { t / f } else { f / t };
        ratio >= min_skew_ratio
    })
}

pub fn run_profile(program: &Program, max_ticks: u64) -> PgoProfile {
    let mut interpreter = Interpreter::new();
    interpreter.profile_mode = true;
    interpreter.load_program(program);

    if max_ticks > 0 {
        let mut executed = true;
        let mut iterations = 0;
        while executed && iterations < max_ticks as usize {
            iterations += 1;
            executed = false;
            for item in &program.items {
                if let crate::ast::TopLevel::Transaction(txn) = item {
                    if txn.is_reactive {
                        let pre_val = interpreter.eval_expr(&txn.contract.pre_condition).ok();
                        if pre_val == Some(crate::interpreter::Value::Bool(true)) {
                            interpreter.prior_state = interpreter.state.clone();
                            let mut transaction_escaped = false;
                            let mut transaction_failed = false;
                            for stmt in &txn.body {
                                if let Err(e) = interpreter.exec_stmt(stmt) {
                                    match e {
                                        crate::interpreter::RuntimeError::Escaped => {
                                            transaction_escaped = true;
                                        }
                                        _ => {
                                            interpreter.state = interpreter.prior_state.clone();
                                            transaction_failed = true;
                                        }
                                    }
                                    break;
                                }
                            }
                            if !transaction_failed && !transaction_escaped {
                                let post_val = interpreter.eval_expr(&txn.contract.post_condition).ok();
                                if post_val != Some(crate::interpreter::Value::Bool(true)) {
                                    interpreter.state = interpreter.prior_state.clone();
                                } else if interpreter.state != interpreter.prior_state {
                                    executed = true;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    PgoProfile {
        branch_counts: interpreter.branch_counts.clone(),
    }
}

pub fn emit_branch_weights(profile: &PgoProfile, guard_id: &str) -> Option<String> {
    let (t, f) = profile.branch_counts.get(guard_id)?;
    if *t == 0 && *f == 0 {
        return None;
    }
    let tw = (*t).min(i32::MAX as u64) as i32;
    let fw = (*f).min(i32::MAX as u64) as i32;
    Some(format!("!prof !{{!\"branch_weights\", i32 {}, i32 {}}}", tw, fw))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pgo_skew_heuristic_rejects_balanced() {
        let mut counts = HashMap::new();
        counts.insert("guard_0".to_string(), (50, 50));
        let profile = PgoProfile { branch_counts: counts };
        assert!(!has_pgo_candidate(&profile, 100),
            "50/50 balanced branch should not be a PGO candidate");
    }

    #[test]
    fn test_pgo_skew_heuristic_accepts_skewed() {
        let mut counts = HashMap::new();
        counts.insert("guard_0".to_string(), (99999, 1));
        let profile = PgoProfile { branch_counts: counts };
        assert!(has_pgo_candidate(&profile, 100),
            "99999:1 skewed branch should be a PGO candidate");
    }

    #[test]
    fn test_emit_branch_weights() {
        let mut counts = HashMap::new();
        counts.insert("guard_0".to_string(), (950, 50));
        let profile = PgoProfile { branch_counts: counts };
        let weights = emit_branch_weights(&profile, "guard_0");
        assert!(weights.is_some());
        let w = weights.unwrap();
        assert!(w.contains("branch_weights"));
        assert!(w.contains("i32 950"));
        assert!(w.contains("i32 50"));
    }
}
