// Dead backend: old WASM. Per AGENTS.md: dead backends receive zero fixes.
// This file is stubbed to prevent compilation failures.

#[test]
#[should_panic(expected = "dead backend: WASM")]
fn test_wasm_stub() {
    panic!("dead backend: WASM");
}
