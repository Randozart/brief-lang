# Tamer Completion and Bugfixes
## 2026-07-26

## Overview

The self-hosted Briev tamer (written in Briev, compiled to `.lair` bytecode via
`BackendKind::Vm`) works end-to-end within the C interpreter harness. It loads a
user program's `.lair` file, validates the header, allocates buffers, and
interprets the program via a convergent `txn` loop (`vm_loop`) with a 20+ arm
`match` dispatch (`exec_op`).

This plan covers the remaining bugs and features needed to make the tamer a
self-contained system tool that can process `.bounty` files and produce native
binaries.

---

## Remaining Items

### 1. Fix CALL Multi-Call Bug in C Interpreter

**Severity:** High — blocks all function calls from within a compiled `.lair`
program. Currently worked around by inlining all reads in `tame`.

**Symptoms:** When a function makes two or more `call` instructions, the first
call returns the correct value but the second and subsequent calls return 0.
This affects ANY function called more than once from the same caller, even if
the callees are different functions.

**Example of the bug:**
```c
// test_two_calls.lair — calling get_val() twice
defn read_i64(bc: Ptr<Int>, addr: Int) -> Int {
    // inline read at byte offset
    let base: Int = bc as Int;
    let word_idx = addr / 8;
    let byte_addr: Int = base + word_idx * 8;
    let word = *(byte_addr as Ptr<Int>);
    term (word >> ((addr % 8) * 8));
};
defn get_val(data: Ptr<Int>) -> Int { term read_i64(data, 48); };
defn test(data: Ptr<Int>) -> Int {
    let a = get_val(data);  // returns correct value (121)
    let b = get_val(data);  // returns 0!
    term a + b;             // returns 121, not 242
};
```

**Root cause analysis:** The `call` instruction handler in `tamer/interp.c`
(pages 535-561) pushes a new frame for the callee and copies arguments from
the operand stack. The `ret` instruction handler (lines 564-580) pops the
frame and pushes the return value back on the caller's stack. Something in
this round-trip corrupts state for subsequent calls.

**Files to debug:**
- `tamer/interp.c` — `case OP_CALL:` and `case OP_RET:` and `push_frame`/`pop_frame`

**Hypotheses to test:**
1. `pop_frame()` doesn't restore `vm->locals_len` correctly, causing the
   second call's argument copy to write to the wrong local slots.
2. `push_frame()` zeroes the callee's locals, which includes the caller's
   frame region (buffer overflow).
3. The operand stack (`vm->stack`) is shared across all frames, and after
   the first call returns, stale values remain that confuse the second call.

**Testing strategy:**
1. Write a micro-benchmark `.bv` file that calls a function twice and adds
   the results (as above).
2. Compile with `--backend vm`, load into the C interpreter, and check
   return value.
3. Add `fprintf(stderr, ...)` debug output to `OP_CALL` and `OP_RET` to
   trace locals_len, stack_len, and frame_count before/after each call.
4. Verify that `locals_len` after the first `ret` equals `locals_len` before
   the first `call`.
5. Check that the second `call` pops the correct number of arguments from
   the stack.

**Fix approach (if root cause is found):**
- If `pop_frame` is the issue: add explicit `vm->locals_len` restore.
- If the stack is corrupted: use a temporary buffer for return values.
- If the frame management is fundamentally flawed: switch to the recursive
  `vm_execute` approach (call `vm_execute` instead of setting `pc`).

---

### 2. `not` Instruction is Bitwise NOT (Not Logical NOT)

**Severity:** Medium — affects `!=` comparisons and any code that inverts a
boolean. Currently fixed for `Neq` via a workaround (use `xor 1` instead of
`not`).

**Symptom:** The `not` instruction (`OP_NOT = 0x0E`) performs bitwise NOT
(`~a`), producing `0xFFFFFFFFFFFFFFFE` when given input `1`. Code expecting
logical NOT (0→1, 1→0) breaks.

**The bug in the C interpreter:**
```c
// tamer/interp.c, case OP_NOT:
stack_push(vm, ~a);  // bitwise NOT
```
For `a = 1`: `~1 = 0xFFFFFFFFFFFFFFFE`. This is non-zero, so subsequent `jz`
does NOT skip the guard body — opposite of what was intended.

**Files to fix:**
- `tamer/interp.c` — `case OP_NOT:` (line 372 or nearby)
- `tamer/interp.h` — documentation of opcodes

**Fix:** The cleanest approach is to define what `not` means in the VM and
be consistent. Options:
- **Option A (recommended):** Make `OP_NOT` a logical NOT: `stack_push(vm, a == 0 ? 1 : 0)`. This makes it consistent with how booleans work in conditionals.
- **Option B:** Keep bitwise NOT but document it. Change `Neq` (and any other logical NOT usage) to use `eq` + `xor 1` as a workaround.
- **Option C:** Add a separate `OP_LNOT` for logical NOT and rename the current `OP_NOT` to `OP_BNOT`.

Option A is simplest and least surprising. The VM doesn't have a concept of
"bits" vs "booleans" — all values are `uint64_t`. A logical NOT (0→1, else→0)
is the correct semantic for condition inversion.

**Impact analysis:** Currently only `Neq` uses `not`, and it's already
worked around with `xor 1` in the Briev compiler's VM backend
(`src/backend/vm/emit_expr.rs`). Fixing `OP_NOT` in the C interpreter will
make the `not` instruction usable for other purposes.

**Checklist:**
- [ ] Change `OP_NOT` in `tamer/interp.c` to use logical NOT
- [ ] Update the assembler's `emit_not()` documentation to describe the
      instruction as logical NOT (not bitwise)
- [ ] Test: `(a == b) → not →` should give 1 if `a != b`, 0 if `a == b`
- [ ] Optionally revert the `xor 1` workaround in `emit_expr.rs`

---

### 3. File I/O for Standalone Tamer

**Severity:** High — the tamer currently receives `.bounty` data in memory
(via the C test harness). To be a standalone system tool, it must read files
from disk.

**Context:** The `brievc bounty` command produces `.bounty` files containing
a `.lair` bytecode section, `.beastpack` section, and a JSON manifest. The
tamer needs to:
1. Read the `.bounty` file from disk (via `SysCall#(SYS_open/read/close)`)
2. Extract the `.lair` and `.beastpack` sections
3. Interpret the `.lair` bytecode
4. Generate LLVM IR from the `.beastpack`
5. Invoke `clang` via `ShellCmd#` to produce a native binary

**Current state:** Step 3 works (the tamer interprets `.lair` bytecode).
Steps 1 and 4-5 need implementation.

**Step 1: File reading**

The `.bounty` file is binary (contains compressed `.beastpack` and raw
`.lair` bytecode). It must be read into memory as raw bytes, not as a
Briev String (which might corrupt non-UTF-8 data).

**Approach:** Use `SysCall#` for raw binary I/O:
- `SysCall#(2, path_cstr, 0, 0, ...)` — `SYS_open` (returns fd)
- `SysCall#(8, fd, 0, 2, ...)` — `SYS_lseek(SEEK_END)` (get file size)
- `SysCall#(8, fd, 0, 0, ...)` — `SYS_lseek(SEEK_SET)` (rewind)
- `SysCall#(9, 0, size, ...)` — `SYS_mmap` (optional, for memory-mapped I/O)
- `SysCall#(0, fd, buf, size, ...)` — `SYS_read` (read into buffer)
- `SysCall#(3, fd, 0, ...)` — `SYS_close`

**Challenge:** `SysCall#` expects integer arguments. A file path is a
`String ` in Briev, but `SYS_open` expects a C string pointer (the raw
address of the path's data). Converting a Briev `String` to a C string
requires extracting the data pointer from the string handle.

Briev's `String` type stores data as: `{i64 length, i8 data[]}` with tag bits
in the handle. Untagging: `handle & ~3ULL` gives the pointer to the
`{length, data}` struct. Adding 8 gives the data pointer.

For the tamer, you can avoid the String→CString conversion by taking the
file path as an additional `Ptr<Int>` argument (the C test harness passes
it directly as a pointer to a NUL-terminated string).

**Files to modify:**
- `lib/tamer/main.bv` — add file reading before the `.lair` parse step
- `lib/tamer/bounty.bv` — `.bounty` format parsing (section extraction)
- `tamer/tests/test_briev_tamer.c` — update test to pass file path

**Step 4: LLVM IR Generation**

The `.beastpack` contains the typed AST. The tamer must walk this AST and
emit LLVM IR text. This is the most complex step.

**Simplified approach for MVP:** The `.lair` bytecode (when interpreted)
emits LLVM IR text as a side effect via `hcall` to `host_llvm_emit`. The
host function stores this text, and after the VM finishes, the accumulated
LLVM IR is written to a `.ll` file.

**Implementation sketch:**
- Add a new host function `host_llvm_emit` that concatenates the emitted
  IR text to a string buffer.
- After the VM finishes, write the buffer to a file and invoke `clang`.
- The `host_llvm_emit` function is called from the `.lair` bytecode when
  the installation compiler processes the `.beastpack`.

**Step 5: clang Invocation**

Use `ShellCmd#` to invoke the system C compiler:

```briev
let cmd = "clang -O3 " + ll_path + " -o " + output_path;
let result = ShellCmd#(cmd);
```

`ShellCmd#` is already implemented and verified working (compiled binaries
run the shell command and return stdout).

**Testing the flow:**
```bash
# Create a .bounty
brievc bounty test.bv -o test.bounty

# Run the tamer with the .bounty
./target/debug/brievc --backend llvm lib/tamer/main.bv -o tamer_native
./tamer_native test.bounty
# → produces native binary
```

---

### 4. Fix `fcntl` redirect in compile.rs

**Severity:** Medium — Blocks `brievc` from outputting compiled `.ll` or
`.lair` files to stdout, which breaks any pipeline that reads from stdout.

**Symptom:** When compiling with `brievc build --backend vm`, the `.lair`
file is written to disk but the CLI tool also tries to redirect fcntl which
fails on some systems.

**File:** `src/compile.rs` — `run_compile` or `run_build` function.

**Checklist:**
- [ ] Find the `libc::dup2` or `fcntl` call that's failing
- [ ] Add error handling (don't crash if redirect fails)
- [ ] Test: `brievc build --backend vm test.bv` should produce test.lair

---

### 5. Clean Up loader.bv — Remove Duplicate Function Versions

**Severity:** Low — cosmetic but fixes wasted bytecode size.

**File:** `lib/tamer/loader.bv`

The file currently has THREE versions of each read function (`read_u8`,
`read_u16`, `read_u32`, `read_i64`, etc.):

1. **Version 1 (lines 20-27):** Cast-based inline reads with `as Int`
   and `as Ptr<Int>` casts. Correct for the current VM architecture.
2. **Version 2 (lines 50-55):** Direct Ptr+Int arithmetic without
   scaling. Crashes — uses `*(bc as Ptr<Int> + addr / 8)` which produces
   wrong byte offsets in the VM.
3. **Version 3 (lines 80-85):** Function-call-based reads that call
   `read_u8` for each byte. Hits the multi-call bug.

**Action:** Remove versions 2 and 3, keeping only version 1. The tamer
currently avoids these functions entirely (uses inlined reads in `main.bv`),
but they should still be correct for future use.

**Checklist:**
- [ ] Remove duplicate function definitions, keeping only the Cast-based
      version (version 1)
- [ ] Verify with `cargo test --lib`

---

### 6. Byte Order Mark (BOM) in Generated `.ll` Files

**Severity:** Low — cosmetic issue in LLVM output.

**File:** The LLVM IR generation code (`src/backend/llvm/` or related)

The `.ll` files written by `brievc build --backend llvm` may have a UTF-8
BOM (`\xEF\xBB\xBF`) at the start, which causes `clang` to emit a warning.

**Checklist:**
- [ ] Check if generated `.ll` files start with a BOM
- [ ] Remove BOM if present
- [ ] Verify with `brievc build --backend llvm test.bv && head -1 test.ll`

---

## Implementation Order

| Priority | Item | Effort | Dependencies | Risk |
|----------|------|--------|-------------|------|
| 1 | Fix CALL multi-call bug | 2-3 days | C interpreter knowledge | High — subtle frame bug |
| 2 | Fix `not` instruction (logical vs bitwise) | 2 hours | Understanding of `Neq` usage | Low |
| 3 | File I/O for standalone tamer | 1-2 days | `SysCall#` working | Medium |
| 4 | LLVM IR generation + clang invocation | 3-5 days | Items 1, 3 | High — complex |
| 5 | Clean up loader.bv duplicates | 30 minutes | Understanding of versions | Low |
| 6 | BOM fix in `.ll` output | 30 minutes | LLVM backend knowledge | Low |

Items 1 and 2 are the highest priority because they affect the reliability of
ALL `.lair` programs, not just the tamer. Items 3-4 are needed for the tamer
to operate as a standalone system tool.

---

## Testing Strategy

### For each bugfix:

1. **Regression test:** Add a `.bv` test case that triggers the bug
2. **Compile:** `brievc build --backend vm test.bv` → produces `.lair`
3. **Execute:** Load `.lair` into C interpreter and check expected result
4. **Verify:** `cargo test --lib` still passes (1006+ tests)
5. **Commit:** With rationale comment referencing this plan

### For feature additions:

1. **Unit test:** Write a minimal test case that exercises the new path
2. **Integration test:** Run the full pipeline: `.bv` → `--backend vm` →
   `.lair` → C interpreter → result
3. **End-to-end:** See the "tamer completion" section below

### Tamer completion test:

```bash
# 1. Build a test program
cat > /tmp/hello.bv << 'EOF'
defn main() -> Int { term 42; }
EOF

# 2. Package as .bounty
brievc bounty /tmp/hello.bv -o /tmp/hello.bounty

# 3. Run tamer on the .bounty
tamer /tmp/hello.bounty -o /tmp/hello_bin
/tmp/hello_bin  # should print nothing (42 exit code)
```

---

## Key Files Reference

| File | Purpose |
|------|---------|
| `tamer/interp.c` | C interpreter — fetch-decode-execute loop, CALL/RET handlers |
| `tamer/interp.h` | VM state struct, opcode definitions |
| `lib/tamer/main.bv` | Tamer entry point — `tame` function with inlined reads |
| `lib/tamer/loader.bv` | Bytecode read helpers, `.lair`/`.bounty` format parsers |
| `lib/tamer/vm.bv` | VM interpreter in Briev — `vm_loop` txn, `exec_op` dispatch |
| `src/backend/vm/assembler.rs` | `.lair` bytecode assembler |
| `src/backend/vm/emit_expr.rs` | Expression → bytecode emission (includes Ptr scaling) |
| `src/backend/vm/emit_stmt.rs` | Statement → bytecode emission |
| `src/backend/vm/emit_toplevel.rs` | Top-level → function table (populates ptr_slots) |
| `src/backend/vm/mod.rs` | `VmBackend` struct, `generate()` entry point |
| `src/backend/llvm/emit_stmt.rs` | LLVM ret type narrowing fix (trunc from i64 to fn_ret_ty) |
| `src/backend/llvm/emit_toplevel.rs` | LLVM function header emission, `llvm_type()` with narrowing |
| `tamer/tests/test_briev_tamer.c` | C test harness for Briev-compiled tamer |
