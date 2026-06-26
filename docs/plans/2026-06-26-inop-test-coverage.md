# Test Coverage: Interpreter Execution + LLVM Backend Verification

**Date:** 2026-06-26
**Status:** Active

## Gaps Identified

All 5 example files and 3 stdlib files pass `check` (parse + type-check),
but none have been **executed** to verify they produce correct output.
The inop BILD bodies have not been verified to produce valid LLVM IR.

---

## Phase 1: Interpreter Execution Tests

For each example, write a Rust unit test in `src/interpreter.rs` that:
1. Parses the example source
2. Creates an Interpreter with the parsed program
3. Executes the `demo()` function (or equivalent entry point)
4. Asserts the result is `Ok(Value::Bool(true))`

### Test 1: `inop-skiplist-dispatch.bv`

Parses source text `examples/inop-skiplist-dispatch.bv`, evaluates `demo()`,
verifies the skip list insert + size check returns `true`.

### Test 2: `inop-ring-buffer.bv`

Parses source text, evaluates `demo()`, verifies FIFO push/pop order: 10, 20, 30.

### Test 3: `inop-uart-mmap.bv`

Parses source, evaluates `self_check()`, verifies the flattened constant
addresses match the expected STM32F407 values (0x40011000 etc.).
Does not execute MMIO (addresses are arbitrary in interpreter).

### Test 4: `inop-syscall-io.bv`

Parses source, checks that the program loads (syscall fallbacks return -1,
so calling `exit_process` would return -1 and not actually exit).

### Test 5: `inop-isr-table.bv`

Parses source, checks that the `#section` attribute is preserved and
the inop declaration loads correctly.

### Test 6: `sl_contains_loop` txn convergence

End-to-end test: create a `SkipList<Int>` with 3 elements, call
`sl_contains` for a present and absent value, verify both results.

---

## Phase 2: LLVM Backend Verification Tests

### Test 7: `sl_insert` BILD body

Verify the emitted LLVM IR for `sl_insert` contains:
- `malloc`, `free`, `llvm.memcpy` calls
- Correct function signature `@sl_insert(i64, i64) -> i64`
- The `term %lhr;` → `ret i64 %lhr` translation

### Test 8: `sl_remove` BILD body

Verify multi-output `{ i64, i64 }` return, `insertvalue` chain.

### Test 9: Atomic inop BILD bodies

Verify `cmpxchg`, `atomicrmw add/sub/and/or/xor`, `load atomic`,
`store atomic` instructions are present in emitted LLVM IR.

### Test 10: Skiplist + atomic inop full-chain test

Build a test program that imports both modules and type-checks
correctly, verifying the type checker's inop generic resolution.

---

## Implementation Notes

### For interpreter tests:

```rust
#[test]
fn test_inop_skiplist_dispatch_execution() {
    let source = std::fs::read_to_string("examples/inop-skiplist-dispatch.bv").unwrap();
    let program = crate::parser::parse_program(&source).unwrap();
    let mut i = Interpreter::new();
    i.load_program(&program);
    let result = i.call_defn("demo", &[]).unwrap();
    assert_eq!(result, Value::Bool(true));
}
```

The key methods: `Interpreter::load_program()` or equivalent,
`Interpreter::call_defn()`, `Interpreter::call_txn()`.

### For LLVM tests:

```rust
#[test]
fn test_skiplist_llvm_emission() {
    let mut backend = LlvmBackend::new();
    let program = parse_program(&load_file("lib/std/skiplist.bv")).unwrap();
    let output = backend.generate(&program);
    assert!(output.contains("@sl_insert"));
    assert!(output.contains("malloc"));
    assert!(output.contains("llvm.memcpy"));
}
```

---

## Files to Change

| File | Change |
|------|--------|
| `src/interpreter.rs` | Add 6 new test functions |
| `src/backend/llvm/tests.rs` | Add 4 new test functions |
| `Cargo.toml` | Possibly not needed (test files already exist) |
