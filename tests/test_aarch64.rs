// Dead backend: AArch64. Per AGENTS.md: dead backends receive zero fixes.
// This file is stubbed to prevent compilation failures.

#[test]
#[should_panic(expected = "dead backend: AArch64")]
fn test_aarch64_stub() {
    panic!("dead backend: AArch64");
}
