// Dead backend: x86_64. Per AGENTS.md: dead backends receive zero fixes.
// This file is stubbed to prevent compilation failures.

#[test]
#[should_panic(expected = "dead backend: x86_64")]
fn test_x86_64_stub() {
    panic!("dead backend: x86_64");
}
