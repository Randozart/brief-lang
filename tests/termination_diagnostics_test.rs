// ── Term Termination Diagnostics — end-to-end ────────────────────────
// 2026-08-04: the fixtures in tests/fixtures/term_*.bv are run through the
// real `briefc check` binary. Behavioral tests: a .bv that provably cannot
// run its trailing statement must fail; valid shapes must pass.

use std::process::Command;

const PROJECT_ROOT: &str = env!("CARGO_MANIFEST_DIR");

fn briefc_check(bv: &str) -> (bool, String) {
    let briefc = env!("CARGO_BIN_EXE_briefc");
    let out = Command::new(briefc)
        .args(["check", bv])
        .output()
        .expect("failed to run briefc check");
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    (out.status.success(), stderr)
}

#[test]
fn check_rejects_unreachable_after_terminating_term() {
    let (ok, err) = briefc_check(&format!("{}/tests/fixtures/term_unreachable.bv", PROJECT_ROOT));
    assert!(!ok, "unreachable code after term! must fail check; stderr: {err}");
    assert!(err.contains("unreachable code"), "missing diagnostic: {err}");
    assert!(err.contains("termination errors"), "error must be tagged: {err}");
}

#[test]
fn check_warns_on_bare_term_guard_hint() {
    let (ok, err) = briefc_check(&format!("{}/tests/fixtures/term_guard_hint.bv", PROJECT_ROOT));
    assert!(ok, "bare-term guard is valid; check must pass; stderr: {err}");
    assert!(err.contains("checkpoint"), "hint warning missing: {err}");
}

#[test]
fn check_accepts_valid_swan_song() {
    let (ok, err) = briefc_check(&format!("{}/tests/fixtures/term_valid_swan_song.bv", PROJECT_ROOT));
    assert!(ok, "valid swan song must pass; stderr: {err}");
}

#[test]
fn check_rejects_unreachable_after_defn_term_return() {
    let (ok, err) = briefc_check(&format!("{}/tests/fixtures/term_defn_unreachable.bv", PROJECT_ROOT));
    assert!(!ok, "unreachable code after term x; in a defn must fail; stderr: {err}");
    assert!(err.contains("unreachable code"), "missing diagnostic: {err}");
}
