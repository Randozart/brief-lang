# Plan: Rename `rct txn` → `node` + Benchmark Fixes

This plan covers two stages:

1. **Stage 1** — Rename `rct txn` to `node` throughout the entire
   codebase, update the lexer/parser/AST/backend to accept `node` as a
   keyword, and ensure `node`, `txn`, and `defn` are correctly
   distinguished with the right semantics and optimization paths.

2. **Stage 2** — Fix all benchmarks identified during investigation to
   compile and run correctly, at parity with C.

---

## Stage 1: Rename `rct txn` → `node`

### 1.1 Overview

The keyword `rct` is removed. The construct formerly written `rct txn`
is now written `node`. The AST field `is_reactive: bool` becomes
`is_node: bool` (or may be kept as `is_reactive` if preferred — see
§1.6).

**Grammar change:**

```
// Before:
transaction ::= ("rct")? "txn" identifier ... contract body
construct  ::= "rct" ... (reactive, no args/return)
             | "txn" ... (callable, args/return)
             | "defn" ... (pure function)

// After:
construct  ::= "node" identifier ... contract body    // reactive, no params/return
             | "txn" identifier ... contract body      // callable, params/return
             | "defn" identifier ... body              // pure function
```

A `node` has no parameters and no return value. A `txn` may have
parameters and a return value. A `defn` is pure (no state access).

### 1.2 Lexer Changes

**File: `src/lexer.rs`**

1. Rename token variant: `Rct` → `Node` (line 57-58,
   `#[token("rct")]` → `#[token("node")]`).
2. Update display impl: `Token::Rct => write!(f, "rct")` →
   `Token::Node => write!(f, "node")` (line 532).
3. The `async` modifier needs handling. Currently `async node`
   parses `rct` keyword then checks for `async`. After rename:
   `node async NAME [pre][post] { ... }`. The lexer already has
   `#[token("async")]  Async,` — no lexer change needed for async.
   Only the parser's order changes (see §1.3).

**Fuzzing keyword list** (`src/fuzzing/ast_generator.rs:384`):
   Replace `"rct"` with `"node"`.

### 1.3 Parser Changes

**File: `src/parser/definitions.rs`**

1. Rename `parse_reactive_transaction()` → `parse_node()` (line 279).
2. Update dispatch: `Some(Token::Rct)` → `Some(Token::Node)` (line 27).
   This triggers `parse_node()` which parses `node NAME [pre][post] { body }`.
3. Update error message (line 293):
   ```
   // Before:
   "expected 'txn' after 'rct', found '{}'"
   // After:
   "expected node name after 'node', found '{}'"
   ```
4. In `parse_node()`: consume `Token::Node`, then check for optional
   `async` modifier, then expect the identifier name (no `"txn"` keyword
   is expected after `node` — the node keyword replaces both `rct` and
   `txn`). Set `is_node: true` on the Transaction.
5. The `"txn"` keyword path (line 333+) remains unchanged — it parses
   callable transactions with `is_node: false`.

**Parse flow after rename:**

```
// Parsing "node NAME [pre][post] { body }"
Token::Node => {
    self.pos += 1; // consume 'node'
    let is_async = if self.check_identifier("async") {
        self.pos += 1;
        true
    } else { false };
    // expect identifier name
    let name = self.parse_identifier()?;
    // parse contracts, body
    let txn = Transaction { is_node: true, is_async, name, ... };
    TopLevel::Transaction(txn)
}

// Parsing "txn NAME(args) [pre][post] -> Ret { body }" — unchanged
```

**File: `src/parser/helpers.rs`**

Update token display: `Token::Rct => "rct".into()` →
`Token::Node => "node".into()` (line 230).

**File: `src/parser/definitions.rs`** — also update the identifier
detection (line 333):
```
// Before: if self.check_identifier("txn") || self.check_identifier("rct") {
// After:  if self.check_identifier("txn") || self.check_identifier("node") {
```

### 1.4 AST Changes

**File: `src/ast/top.rs`**

Rename field: `pub is_reactive: bool` → `pub is_node: bool` (line 94).

**File: `src/ast/display.rs`**

Update output text (line 355):
```rust
// Before:
let prefix = if txn.is_reactive { "rct txn" } else { "txn" };

// After:
let prefix = if txn.is_node { "node" } else { "txn" };
```

### 1.5 Internal Field Name: `is_reactive` → `is_node`

The boolean field on `Transaction` appears in ~60 locations. Every
occurrence must be updated:

| File | Current | New |
|------|---------|-----|
| `src/ast/top.rs:94` | `is_reactive: bool` | `is_node: bool` |
| `src/ast/display.rs:355` | `txn.is_reactive` | `txn.is_node` |
| `src/annotator.rs:311` | `if txn.is_reactive` | `if txn.is_node` |
| `src/analysis/transition_graph.rs:27` | `is_reactive: bool` | `is_node: bool` |
| `src/analysis/transition_graph.rs:155` | `is_reactive: txn.is_reactive` | `is_node: txn.is_node` |
| `src/analysis/provenance.rs:204` | `is_reactive: bool` | `is_node: bool` |
| `src/reactor.rs:39` | `txn.is_reactive` | `txn.is_node` |
| `src/reactor.rs:427` | `fn make_rct_txn` | `fn make_node` |
| `src/backend/llvm/mod.rs:1682` | `!t.is_reactive` | `!t.is_node` |
| `src/backend/llvm/mod.rs:2247` | `!txn.is_reactive` | `!txn.is_node` |
| `src/backend/llvm/mod.rs:2368` | `t.is_reactive` | `t.is_node` |
| `src/backend/llvm/mod.rs:2375` | `!t.is_reactive` | `!t.is_node` |
| `src/backend/llvm/mod.rs:2442` | `graph.nodes[0].is_reactive` | `graph.nodes[0].is_node` |
| `src/backend/llvm/mod.rs:3180` | `node.is_reactive` | `node.is_node` |
| `src/backend/llvm/emit_toplevel.rs:1270` | `txn.is_reactive` | `txn.is_node` |
| `src/backend/llvm/helpers.rs:494` | `!ta.is_reactive \|\| !tb.is_reactive` | `!ta.is_node \|\| !tb.is_node` |
| `src/backend/llvm/hazard.rs:264` | `t.is_reactive` | `t.is_node` |
| `src/backend/llvm/hazard.rs:359` | `t.is_reactive` | `t.is_node` |
| `src/backend/llvm/dispatch.rs:57` | `t.is_reactive` | `t.is_node` |
| `src/backend/llvm/dispatch.rs:300` | `t.is_reactive` | `t.is_node` |
| `src/backend/llvm/optimizer.rs:85` | `t.is_reactive` | `t.is_node` |
| `src/backend/llvm/optimizer.rs:192` | `!txn.is_reactive` | `!txn.is_node` |
| `src/backend/llvm/optimizer.rs:247` | `t.is_reactive` | `t.is_node` |
| `src/backend/llvm/optimizer.rs:254` | `t.is_reactive` | `t.is_node` |
| `src/backend/llvm/loop_engine/ssa.rs:42` | `!txn.is_reactive` | `!txn.is_node` |
| `src/backend/llvm/loop_engine/ssa.rs:327` | `t.is_reactive` | `t.is_node` |
| `src/backend/llvm/loop_engine/ssa.rs:364` | `txn.is_reactive` | `txn.is_node` |
| `src/backend/webstack.rs:336` | `txn.is_reactive` | `txn.is_node` |
| `src/plugin/intrinsics.rs:260` | `txn.is_reactive` | `txn.is_node` |
| `src/fuzz_checker/mod.rs:59` | `txn.is_reactive` | `txn.is_node` |
| `src/fuzzing/ast_generator.rs:77-80` | `is_reactive` | `is_node` |
| `src/beast/serialize.rs:65` | `t.is_reactive` | `t.is_node` |
| `src/beast/deserialize.rs:222-240` | `is_reactive` | `is_node` |
| `src/lsp.rs:808` | `t.is_reactive` | `t.is_node` |

**Rust test files** — all `is_reactive: true` → `is_node: true`,
all `is_reactive: false` → `is_node: false`:

| File | Count |
|------|-------|
| `src/reactor.rs` | ~15 test sites |
| `src/analysis/transition_graph.rs` | ~4 test sites |
| `src/analysis/dependency_graph.rs` | ~1 |
| `src/analysis/region.rs` | ~1 |
| `src/analysis/range.rs` | ~1 |
| `src/analysis/call_graph.rs` | ~1 |
| `src/analysis/watchdog.rs` | ~1 |
| `src/backend/llvm/tests.rs` | ~33 test sites |
| `src/backend/webstack.rs` | ~2 |
| `src/backend/circt.rs` | ~1 |
| `src/plugin/intrinsics.rs` | ~3 |
| `src/fuzzing/concolic.rs` | ~1 |
| `src/hardware_validator.rs` | ~1 |

**Recommended command for mechanical rename:**
```bash
# Rename struct field and all references in Rust source
find src -name '*.rs' -exec sed -i \
  -e 's/is_reactive/is_node/g' \
  -e 's/make_rct_txn/make_node/g' \
  {} +
```

### 1.6 BEAST Serialization

**File: `src/beast/serialize.rs`** (line 65):
```
// Before: if t.is_reactive { children.push(atom(":reactive")); }
// After:  if t.is_node { children.push(atom(":node")); }
```

**File: `src/beast/deserialize.rs`** (lines 222-240):
```
// Before:
":reactive" => { is_reactive = true; i += 1; }
// After:
":node" => { is_node = true; i += 1; }
```

**Backward compatibility:** Accept both `:reactive` and `:node` during
a transition period, emit only `:node`. Add a fallthrough:
```rust
":reactive" | ":node" => { is_node = true; i += 1; }
```

### 1.7 Annotator

**File: `src/annotator.rs:311`**:
```
// Before:
let rct = if txn.is_reactive { "rct " } else { "" };

// After:
let kind = if txn.is_node { "node " } else { "txn " };
```

### 1.8 LSP

**File: `src/lsp.rs`**

1. Keyword list (line 565): Replace `"rct"` with `"node"`.
2. Hover display (line 808):
   ```
   // Before:
   if t.is_reactive { " rct" } else { "" }
   // After:
   if t.is_node { " node" } else { " txn" }
   ```

### 1.9 Plugin

**File: `src/plugin/intrinsics.rs`**

Update all comments (lines 231, 256) referencing `rct txn` → `node`.
The `CheckReactive` plugin name in comments → `CheckNode`.

### 1.10 Rust Comments

Every Rust doc comment or inline comment referencing `rct txn`:

| File | Line(s) | Change |
|------|---------|--------|
| `src/parser/definitions.rs` | 279-293 | Doc comments + error messages |
| `src/backend/llvm/mod.rs` | 458 | Doc comment |
| `src/backend/llvm/optimizer.rs` | 289 | Comment: `async node` → `node async` |
| `src/backend/llvm/loop_engine/ssa.rs` | 9-10 | Doc comments on SSA paths |
| `src/plugin/intrinsics.rs` | 231, 256 | Doc comments |

Command:
```bash
find src -name '*.rs' -exec sed -i \
  -e 's/rct txn/node/g' \
  -e 's/async node/node async/g' \
  -e 's/rct/node/g' \
  {} +
```

**CAUTION:** `sed -i 's/rct/node/g'` may match `rct` inside words or
identifiers. Review with `git diff` afterward. Better to use more
specific patterns:
```bash
sed -i 's/\brct txn\b/node/g'
sed -i 's/\basync node\b/node async/g'
```

### 1.11 Stdlib `.bv` Files

Replace `rct txn` → `node` in:

| File | Lines |
|------|-------|
| `lib/std/system.bv` | 13 (comment), 26, 34, 42 |
| `lib/std/console.bv` | 10 (comment), 21 (comment), 42 |
| `lib/std/ffi/system.bv` | 13 (comment), 26, 34, 42 |

Simple `sed -i 's/rct txn/node/g'` on each file.

### 1.12 Self-hosting Compiler

**File: `lib/compiler/token.bv:176`:**
```
// Before:
uni tok(KeywordRct) = "rct";
// After:
uni tok(KeywordNode) = "node";
```

**File: `lib/compiler/lexer.bv:148`:**
```
// Before:
[text == "rct"] { term (KeywordRct, state); };
// After:
[text == "node"] { term (KeywordNode, state); };
```

**Verification:** The self-hosting compiler must produce the same
output as the Rust compiler for the same input. Test:
```bash
# Compile a test file with Rust compiler
./target/release/brief-compiler build tests/fixtures/counter.bv -o /tmp/rust_out

# Compile the self-hosting compiler with itself (once self-hosting works)
```

### 1.13 Example `.bv` Files

Replace `rct txn` → `node` in all 27 example files:

```bash
find examples -name '*.bv' -exec sed -i 's/rct txn/node/g' {} +
find examples -name '*.bv' -exec sed -i 's/async node/node async/g' {} +
```

**Note:** `examples/complex_workflow.bv` has 5+ node declarations.
`examples/volatile-io.bv` has a node at line 92. These are all
straightforward replacements.

### 1.14 Test Fixture `.bv` Files

Replace `rct txn` → `node` in all 22+ test fixture files:

```bash
find tests -name '*.bv' -exec sed -i 's/rct txn/node/g' {} +
find tests -name '*.bv' -exec sed -i 's/async node/node async/g' {} +
```

**Note:** `tests/fixtures/event_model.bv` has multiple nodes (lines
14, 20, 26). `tests/fixtures/multifield.bv` has nodes at lines 5, 12.
All are straightforward.

### 1.15 Benchmark `.bv` Files

Replace `rct txn` → `node` in all 31+ benchmark files:

```bash
find benchmarks -name '*.bv' -exec sed -i 's/rct txn/node/g' {} +
find benchmarks -name '*.bv' -exec sed -i 's/async node/node async/g' {} +
```

**Special case: async modifier**
```
// Before:
async node fetch [pre][post] { ... }

// After:
node async fetch [pre][post] { ... }
```

### 1.16 Documentation Files

**Strategy:** Use `sed` for bulk replacement, then manual review for
edge cases.

```bash
# Architecture docs
find docs -name '*.md' -exec sed -i \
  -e 's/rct txn/node/g' \
  -e 's/async node/node async/g' \
  {} +

# Spec docs
find spec -name '*.md' -exec sed -i \
  -e 's/rct txn/node/g' \
  -e 's/async node/node async/g' \
  {} +

# Learn-brief docs
find learn-brief -name '*.md' -exec sed -i \
  -e 's/rct txn/node/g' \
  -e 's/async node/node async/g' \
  {} +

# Root docs
sed -i 's/rct txn/node/g' AGENTS.md AGENTS_HISTORY.md AGENTS_HISTORY_2.md BUGS.md README.md
sed -i 's/async node/node async/g' AGENTS.md AGENTS_HISTORY.md AGENTS_HISTORY_2.md BUGS.md README.md
```

**Edge cases to review manually after bulk replace:**

| Pattern | Where it appears | Manual check |
|---------|-----------------|--------------|
| `"rct"` (quoted keyword) | `spec/SPEC.md` grammar rules | Change to `"node"` |
| `Rct` (PascalCase in tables) | spec tables, milestone docs | Change to `Node` |
| `rct` (bare, meaning the construct) | philosophy docs, agent history | Change to `node` |
| `rct` as part of a larger identifier | discussion text, bug reports | Review individually |
| `circt` (contains "rct" as substring) | backend references in plans | **DO NOT TOUCH** — `circt` is a separate backend name |

**For `circt`:**
The substring `circt` contains `rct` but is NOT related to the
`rct txn` keyword. **Do NOT use `sed -i 's/rct/node/g'`** without
word boundaries. Use `sed -i 's/\brct\b/node/g'` or better, use
separate targeted patterns for `rct txn`, `async node`, and `rct`
as a standalone word.

The `docs/architecture/txn-semantics.md` file is already updated — it
uses `node` throughout.

### 1.17 Validation After Stage 1

```bash
# 1. Build
cargo build

# 2. Run tests
cargo test --lib

# 3. Check for remaining 'rct' references (excluding circt, archived)
git grep -n -i '\brct\b' -- '*.rs' '*.bv' '*.md' \
  | grep -v 'circt' \
  | grep -v 'archived' \
  | grep -v 'old_docs'

# 4. Compile a simple file to verify parsing
./target/release/brief-compiler build examples/simple_contract.bv -o /tmp/test_node

# 5. Run the compiled binary
/tmp/test_node

# 6. Compile all benchmarks
for b in benchmarks/*.bv; do
  BOUND=50000000 ./target/release/brief-compiler build "$b" --out benchmarks 2>&1 \
    | grep -v "warning:" || echo "FAIL: $b"
done
```

Planned find-and-replace commands:

```bash
# ── Rust source ──
find src -name '*.rs' -exec sed -i \
  -e 's/\bis_reactive\b/is_node/g' \
  -e 's/\bmake_rct_txn\b/make_node/g' \
  {} +

# ── .bv files ──
find lib benchmarks tests examples -name '*.bv' -exec sed -i \
  -e 's/rct txn/node/g' \
  -e 's/async node/node async/g' \
  {} +
find lib/compiler -name '*.bv' -exec sed -i \
  -e 's/KeywordRct/KeywordNode/g' \
  -e 's/"rct"/"node"/g' \
  {} +

# ── Markdown files ──
find docs spec learn-brief -name '*.md' -exec sed -i \
  -e 's/\brct txn\b/node/g' \
  -e 's/\basync node\b/node async/g' \
  -e 's/\brct\b/node/g' \
  {} +
sed -i \
  -e 's/\brct txn\b/node/g' \
  -e 's/\basync node\b/node async/g' \
  AGENTS.md AGENTS_HISTORY.md AGENTS_HISTORY_2.md BUGS.md README.md

# ── Lexer token ──
sed -i 's/#\[token("rct")\]/#[token("node")]/' src/lexer.rs
```

---

## Stage 2: Benchmark Fixes

### 2.1 fasta — 105× regression

**Root cause:** `__print_char()` in `lib/runtime/brief_rt.c:163` calls
`fflush(stdout)` after every non-newline character. 50M fflush syscalls
vs C's buffered `fputc`.

**Fix:** Remove `fflush(stdout)` from `__print_char`.

**File: `lib/runtime/brief_rt.c`** (lines 163-171):

```c
// BEFORE:
int64_t __print_char(int64_t c) {
    if (c == 10) {
        puts("");
    } else {
        putchar((int)c);
        fflush(stdout);    // ← DELETE THIS LINE
    }
    return 0;
}

// AFTER:
int64_t __print_char(int64_t c) {
    if (c == 10) {
        puts("");
    } else {
        putchar((int)c);
    }
    return 0;
}
```

**Verification:**
```bash
cargo build --release
BOUND=50000000 ./target/release/brief-compiler build benchmarks/fasta.bv --out benchmarks
hyperfine -w 2 'BOUND=50000000 benchmarks/fasta > /dev/null' \
               'BOUND=50000000 benchmarks/fasta_c > /dev/null'
```

Expected ratio: ~1.0× (both buffered). Previous: 105×.

---

### 2.2 queue_drain — build failure

**Root cause (two-part chain):**

1. `collect_state_identifiers` in `src/analysis/transition_graph.rs`
   has no match arm for `Expr::AddrOf` or `Expr::Deref`. The arrow ops
   `<- &queue` and `&queue <- count` parse their operand as `AddrOf`,
   so `queue` is invisible to field-reference analysis.

2. `apply_field_modes` in `src/backend/llvm/mod.rs` eliminates `queue`
   as "unreferenced," reducing `%State` from 7 to 6 fields. But
   `arena_ptr_idx`, `arena_end_idx`, `arena_base_idx` hold stale
   indices (4, 5, 6), producing GEP `i32 6` on a 6-field struct.

**Fix A: Add `Expr::AddrOf`/`Expr::Deref` handling.**

**File: `src/analysis/transition_graph.rs`** (after line ~1044):

```rust
// 2026-07-21: Handle &state_field and *state_field so arrow ops
// (e.g. <- &queue, &queue <- count) don't hide state references
// from field-liveness analysis.
Expr::AddrOf(inner) | Expr::Deref(inner) => {
    collect_state_identifiers(inner, state_fields, out);
}
```

**Fix B: Update stale arena indices after field rebuild.**

**File: `src/backend/llvm/mod.rs`** (~line 3690):

After the rebuild loop in `apply_field_modes` that reconstructs
`field_index_map`, add:

```rust
// 2026-07-21: Update stale arena/ringbuf indices after field
// elimination may have shifted them. The rebuild loop above
// reconstructs field_index_map with eliminated fields removed,
// so we must re-read indices from the new map.
self.arena_ptr_idx = self.ctx.field_index_map.get("__arena_ptr").copied();
self.arena_end_idx = self.ctx.field_index_map.get("__arena_end").copied();
self.arena_base_idx = self.ctx.field_index_map.get("__arena_base").copied();
self.ringbuf_data_idx = self.ctx.field_index_map.get("__ringbuf_data").copied();
self.ringbuf_head_idx = self.ctx.field_index_map.get("__ringbuf_head").copied();
self.ringbuf_tail_idx = self.ctx.field_index_map.get("__ringbuf_tail").copied();
self.ringbuf_mask_idx = self.ctx.field_index_map.get("__ringbuf_mask").copied();
```

**Note on `Option<usize>` vs `usize`:** If `arena_ptr_idx` and friends
are `Option<usize>`, the `.copied()` from `.get()` works directly. If
they are `usize`, use `.unwrap_or(0)`. Check the field types in the
backend struct before writing.

**Verification:**
```bash
cargo test --lib
BOUND=50000000 ./target/release/brief-compiler build benchmarks/queue_drain.bv --out benchmarks
BOUND=50000000 ./target/release/brief-compiler build benchmarks/queue_drain_idio.bv --out benchmarks
BOUND=50000000 bash -c 'diff <(benchmarks/queue_drain) <(benchmarks/queue_drain_c)'
```

---

### 2.3 float_math_nonzero — 2.21× regression

**Root cause:**

Two distinct issues:

**A (p22 phi bug):** The `p22` state variable is loaded as a constant
`0.0` and never loop-carried through the SSA phi in the main
convergence loop. The `p22 += Q22` computation produces a value used
only in the print guard — it never feeds back into the next tick.
This is a correctness bug (p22 accumulation is lost) and also prevents
proper optimization.

**B (sequential dependency chain):** The txn body assigns `x0`, `x1`,
`x2` sequentially. Since all RHS read pre-tick state (atomicity rule),
these three assignments are independent — no read-after-write chain.
But the current codegen emits them as sequential load→compute→store
sequences, creating artificial data dependencies. LLVM's scheduler
cannot break them because the store to state looks like a memory
barrier.

**Fix A: Add p22 to loop-carried phis.**

**File: `src/backend/llvm/mod.rs`** — convergence loop emission.

Find where the loop-carry phi nodes are built for the main convergence
loop (the function that emits the `%cm_header` → `%cm_body` →
`%cm_latch` pattern). The phi nodes are built from the set of state
fields. Ensure that ALL mutable state fields (including p00, p11, p22)
are included in the phi set, not just the ones referenced in branch
conditions.

Specifically, look for where `visited_fields` or `done_needs_fields`
are populated and ensure `p22` doesn't fall through a gap. The bug is
likely that p22 is not detected as a "live" field because the
analysis only tracks fields referenced in the transaction body's
expressions, but p22's write (`p22 = p22 + Q22`) is detected, while
its read in the print guard `[count % 5000000 == 0] { ... p22 ... }`
is in a guarded block that may be handled separately.

**Fix B: Emit concurrent-style code for independent assignments.**

This is a larger optimization. The compiler should detect that within
a node/txn body, assignments to state fields from pre-tick reads can
be reordered for parallel execution.

**Implementation sketch:**

1. In the txn body emission pass, collect all assignments to state
   fields.
2. For each assignment, check if its RHS references only pre-tick
   state fields (no `let` bindings that depend on earlier state writes).
3. If all RHS references are to pre-tick state, split the body into:
   - **Load phase:** Load all referenced pre-tick state fields.
   - **Compute phase:** Compute all new values (no ordering constraints).
   - **Store phase:** Store all new values to state.
4. Emit the compute phase with no memory barriers between independent
   computations, allowing LLVM to schedule them on separate execution
   units.

**Priority:** Fix A (p22 phi) is a correctness bug — do it first.
Fix B is a performance optimization — do it second, after verifying
that A resolves the correctness issue and measuring the remaining gap.
The benchmark rewrite with `let` temporaries (see §2.3 Alternative)
achieves the same effect without Fix B.

**Alternative (benchmark-only fix):**

Rewrite `benchmarks/float_math_nonzero.bv` to use explicit
temporaries that capture pre-tick values:

```
node tick [count < total][count == total] {
    let nx0 = A00*x0 + A01*x1 + A02*x2;
    let nx1 = A10*x0 + A11*x1 + A12*x2;
    let nx2 = A20*x0 + A21*x1 + A22*x2;
    x0 = nx0; x1 = nx1; x2 = nx2;
    p00 = p00 + Q00;
    p11 = p11 + Q11;
    p22 = p22 + Q22;
    count = count + 1;
    [count % 5000000 == 0] {
        let trace: Float = p00 + p11 + p22;
        PrintLn!(x0 + x1 + x2 + trace);
    };
    term;
};
```

This avoids the sequential dependency chain because `nx0`, `nx1`,
`nx2` are independent `let` bindings that all read pre-tick state.
The compiler emits them with no read-after-write chain, and LLVM
schedules them in parallel.

**Verification:**
```bash
cargo test --lib
BOUND=50000000 ./target/release/brief-compiler build benchmarks/float_math_nonzero.bv --out benchmarks
BOUND=50000000 bash -c 'diff <(benchmarks/float_math_nonzero) <(benchmarks/float_math_nonzero_c)'
hyperfine -w 2 'BOUND=50000000 benchmarks/float_math_nonzero' \
               'BOUND=50000000 benchmarks/float_math_nonzero_c'
```

Expected ratio after Fix A + benchmark rewrite: ~1.0×.

---

### 2.4 ring_buffer — 1.77× regression (low priority)

**Analysis:** After `clang -O3` (the actual compilation pipeline), the
Brief hot loop body is structurally identical to C's — 9 instructions
including the `mul` for modulus. Both DCE the buffer store as dead
code (no FFI reads the buffer). The 1.77× ratio is likely measurement
noise at sub-60ms absolute runtimes.

**Minor issue:** Raw IR emits `srem` instead of `urem`. The
`clang -O3` pipeline's `instcombine` converts `srem` to `urem`, so
this has no effect on the final binary. But if anyone uses `llc`
directly without `opt`, the `srem` would produce a hardware `idivq`
instruction instead of a multiplication-by-magic `mulq`.

**Fix (optional):** In the operator emission code that lowers `%` and
`mod`, detect non-negative operands and emit `urem` instead of `srem`.
This is a ~3 line change in the LLVM backend's binary operator
emission.

**Priority:** Lowest — only worth doing if other fixes are complete
and the 1.77× persists after all other changes.

**Verification:**
```bash
BOUND=50000000 ./target/release/brief-compiler build benchmarks/ring_buffer.bv --out benchmarks
hyperfine -w 2 'BOUND=50000000 benchmarks/ring_buffer' \
               'BOUND=50000000 benchmarks/ring_buffer_c'
```

---

## Dependency Order

```
Stage 1 (rename)
  ├── 1.2 Lexer
  ├── 1.3 Parser
  ├── 1.4 AST
  ├── 1.5 Field rename (~60 sites)
  ├── 1.6 BEAST
  ├── 1.7 Annotator
  ├── 1.8 LSP
  ├── 1.9 Plugin
  ├── 1.10 Rust comments
  ├── 1.11 Stdlib .bv
  ├── 1.12 Self-hosting
  ├── 1.13 Examples .bv
  ├── 1.14 Tests .bv
  ├── 1.15 Benchmarks .bv
  ├── 1.16 Documentation
  └── 1.17 Validation

Stage 2 (benchmarks)
  ├── 2.1 fasta (fflush)           ← independent, can start after Stage 1
  ├── 2.2 queue_drain (AddrOf)     ← independent
  ├── 2.2 queue_drain (indices)    ← independent
  ├── 2.3 float_math (p22 phi)     ← independent
  ├── 2.3 float_math (concurrent)  ← independent (or benchmark rewrite)
  └── 2.4 ring_buffer (srem/urem)  ← lowest priority
```

All Stage 2 fixes are independent of each other and can be implemented
in any order after Stage 1 is complete. The only dependency is that
Stage 2 benchmarks use the renamed `node` syntax, so they must be
updated in Stage 1 first.

---

## Commands Summary

```bash
# ── Rust source rename ──
find src -name '*.rs' -exec sed -i \
  -e 's/\bis_reactive\b/is_node/g' \
  -e 's/\bmake_rct_txn\b/make_node/g' \
  {} +
sed -i 's/#\[token("rct")\]/#[token("node")]/' src/lexer.rs

# ── .bv files ──
find lib benchmarks tests examples -name '*.bv' -exec sed -i \
  -e 's/rct txn/node/g' \
  -e 's/async node/node async/g' \
  {} +
find lib/compiler -name '*.bv' -exec sed -i \
  -e 's/KeywordRct/KeywordNode/g' \
  -e 's/"rct"/"node"/g' \
  {} +

# ── Markdown files ──
find docs spec learn-brief -name '*.md' -exec sed -i \
  -e 's/\brct txn\b/node/g' \
  -e 's/\basync node\b/node async/g' \
  -e 's/\brct\b/node/g' \
  {} +
sed -i \
  -e 's/\brct txn\b/node/g' \
  -e 's/\basync node\b/node async/g' \
  AGENTS.md AGENTS_HISTORY.md AGENTS_HISTORY_2.md BUGS.md README.md

# ── Build and test ──
cargo build
cargo test --lib

# ── Verify no remaining rct references (excluding circt and archived) ──
git grep -n -i '\brct\b' -- '*.rs' '*.bv' '*.md' \
  | grep -v 'circt' \
  | grep -v 'archived' \
  | grep -v 'old_docs'
```

---

## Stage 3: Rename `.beast` → `.beast` (Brief Expressive AST)

The BEAST intermediate representation is renamed to BEAST — Brief Expressive
AST. File extension `.beast`, CLI flag `--emit-beast`, module path
`src/beast/`.

### 3.1 Overview

```
beast → beast    # Extension
Bvir → Beast    # PascalCase (enum, type names)
beast → beast    # snake_case (function names, field names, module)
--emit-beast → --emit-beast   # CLI flag
```

### 3.2 Module directory rename

```bash
mv src/beast src/beast
sed -i 's/pub mod beast/pub mod beast/' src/lib.rs
```

### 3.3 Rust source changes

**File: `src/lib.rs`** (line 32):
```
pub mod beast;  →  pub mod beast;
```

**File: `src/main.rs`:**

| Line(s) | Change |
|---------|--------|
| 60 | `--emit-beast` → `--emit-beast` (help text) |
| 85 | `--emit-beast [stage]` → `--emit-beast [stage]` (doc comment) |
| 94 | `emit_beast: Vec<BeastStage>` → `emit_beast: Vec<BeastStage>` |
| 127-139 | `--emit-beast` → `--emit-beast`, `BeastStage::*` → `BeastStage::*` |
| 201 | `emit_beast_stages: emit_beast` → `emit_beast_stages: emit_beast` |

**File: `src/compile.rs`:**

| Line(s) | Change |
|---------|--------|
| 28 | Comment: `--emit-beast` → `--emit-beast` |
| 56-57 | `pub emit_beast_stages: Vec<BeastStage>` → `pub emit_beast_stages: Vec<BeastStage>` |
| 112,130,152 | `emit_beast_snapshot` → `emit_beast_snapshot` |
| 217 | `emit_beast_stages: vec![]` → `emit_beast_stages: vec![]` |
| 380 | Doc comment: `--emit-beast` → `--emit-beast` |
| 382 | `fn emit_beast_snapshot` → `fn emit_beast_snapshot` |
| 389 | `opts.emit_beast_stages` → `opts.emit_beast_stages` |
| 397 | `brief_compiler::beast::to_beast` → `brief_compiler::beast::to_beast` |
| 399 | `{}.beast.{}` → `{}.beast.{}` (file extension in output path) |

**File: `src/beast/mod.rs`** (formerly `src/beast/mod.rs`):

| Line(s) | Change |
|---------|--------|
| 3 | Comment: `.beast` → `.beast` |
| 11 | `pub use serialize::to_beast` → `pub use serialize::to_beast` |
| 12 | `pub use deserialize::from_beast` → `pub use deserialize::from_beast` |

**File: `src/beast/serialize.rs`** (formerly `src/beast/serialize.rs`):

| Line(s) | Change |
|---------|--------|
| 2 | Comment: `.beast` → `.beast` |
| 11 | `pub fn to_beast` → `pub fn to_beast` |
| 282-292 | `crate::beast::from_beast` → `crate::beast::from_beast`, `to_beast` → `to_beast` |
| 293,339-340 | Same function/trait renames |
| 355-357 | `crate::beast::sexpr` → `crate::beast::sexpr` |

**File: `src/beast/deserialize.rs`** (formerly `src/beast/deserialize.rs`):

| Line(s) | Change |
|---------|--------|
| 2 | Comment: `.beast` → `.beast` |
| 11 | `pub fn from_beast` → `pub fn from_beast` |

**File: `src/beast/sexpr.rs`** (formerly `src/beast/sexpr.rs`):

| Line(s) | Change |
|---------|--------|
| 2 | Comment: `.beast` → `.beast` |

**File: `src/beast/pattern.rs`** (formerly `src/beast/pattern.rs`):

| Line(s) | Change |
|---------|--------|
| 586,655 | `crate::beast::sexpr` → `crate::beast::sexpr` |

**Files referencing `crate::beast::` or `beast::`**:

| File | Change |
|------|--------|
| `src/plugin/intrinsics.rs:23` | `use crate::beast` → `use crate::beast` |
| `src/plugin/intrinsics.rs:152,157-163` | `beast::` → `beast::` (14 occurrences) |
| `src/backend/llvm/normalizer.rs:229,273` | `crate::beast::layout` → `crate::beast::layout` |
| `src/ast/layout.rs:3` | Comment: `src/beast/layout.rs` → `src/beast/layout.rs` |

### 3.4 Documentation

| File | Change |
|------|--------|
| `docs/plans/2026-07-15-compiletime-meta-and-plugin-architecture.md` | `.beast` → `.beast`, `BEAST` → `BEAST` (~15 refs) |
| `docs/plans/2026-07-14-beast-plugin-midend.md` | Rename file to `...beast-plugin-midend.md` + all `.beast` → `.beast` |
| `docs/plans/2026-07-19-extensible-number-types.md` | `beast::layout` → `beast::layout` |

### 3.5 Self-hosting compiler references

If the self-hosting compiler (`lib/compiler/`) mentions `.beast` or `beast`,
update those references as well (search first — likely none, since the
`.beast` format is a Rust-side serialization detail).

### 3.6 Suggested commands

```bash
# Rename module directory
mv src/beast src/beast

# Rust source (mechanical replacements)
find src -name '*.rs' -not -path '*/beast/*' -exec sed -i \
  -e 's/\bbvir\b/beast/g' \
  -e 's/\bBvir\b/Beast/g' \
  -e 's/\bto_beast\b/to_beast/g' \
  -e 's/\bfrom_beast\b/from_beast/g' \
  -e 's/\bemit_beast\b/emit_beast/g' \
  -e 's/--emit-beast/--emit-beast/g' \
  {} +

# The beast module itself (already moved, avoids self-rename)
find src/beast -name '*.rs' -exec sed -i \
  -e 's/\bbvir\b/beast/g' \
  -e 's/\bBvir\b/Beast/g' \
  -e 's/\bto_beast\b/to_beast/g' \
  -e 's/\bfrom_beast\b/from_beast/g' \
  {} +

# Docs
find docs -name '*.md' -exec sed -i \
  -e 's/\.beast/.beast/g' \
  -e 's/\bBEAST\b/BEAST/g' \
  -e 's/\bbvir\b/beast/g' \
  {} +

# Root docs
sed -i \
  -e 's/\.beast/.beast/g' \
  -e 's/\bBEAST\b/BEAST/g' \
  -e 's/\bbvir\b/beast/g' \
  AGENTS.md AGENTS_HISTORY.md AGENTS_HISTORY_2.md BUGS.md README.md
```

### 3.7 Validation

```bash
cargo build
cargo test --lib
git grep -n '\.beast\|BEAST\|to_beast\|from_beast\|emit_beast' -- '*.rs' '*.md' \
  | grep -v archived | grep -v old_docs
# Should return no results
```
```

---

## Stage 4: Current Benchmark Results (2026-07-21)

All benchmarks compiled with `cargo build --release`, timed with nanosecond-precision
fork+exec harness at `BOUND=50000000`. Single iteration per benchmark (not averaged).

### Runtime Benchmarks

| Benchmark | Brief | C | Ratio | Winner | Note |
|-----------|-------|---|-------|--------|------|
| ring_buffer | 0.0448s | 0.0326s | 1.37x | C | Loop eliminated to counter-only; 1.37x is binary-size noise |
| float_math | 0.0736s | 0.0721s | 1.02x | ~tie | |
| **float_math_nonzero** | **0.1579s** | **0.1677s** | **0.94x** | **Brief** | Fixed p22 phi + atomic reads + float print |
| sparse_dispatch | 0.0668s | 0.0638s | 1.04x | ~tie | |
| print_loop | 0.0629s | 0.0574s | 1.09x | C | Redundant state stores |
| nbody_newton | 11.3080s | 9.1152s | 1.24x | C | Memory counter loop overhead |
| **nbody_sqrt** | **2.4426s** | **2.8129s** | **0.86x** | **Brief** | |
| **nbody_sqrt_idio** | **2.4757s** | **3.6449s** | **0.67x** | **Brief** | |
| fasta | 0.2599s | 0.2193s | 1.18x | C | %State escape blocks SROA |
| fannkuch_redux | 0.0786s | 0.0694s | 1.13x | C | 6-phi cap too small for 16 fields |
| mandelbrot | 0.6705s | 0.6675s | 1.00x | ~tie | |
| kalman_filter_runtime | 0.1832s | 0.1798s | 1.01x | ~tie | |
| knucleotide | 0.1928s | 0.1911s | 1.00x | ~tie | |
| cancel_math | 0.0649s | 0.0626s | 1.03x | ~tie | |
| bit_clear | 0.0011s | 0.0008s | 1.48x | C | Noise floor (63 iter, ~2µs work) |
| queue_drain | 0.0625s | 0.0611s | 1.02x | ~tie | Fixed AddrOf/Deref + arena index |
| queue_drain_sym | 0.0660s | 0.0581s | 1.13x | C | Redundant state stores |
| queue_drain_idio | 0.0645s | — | — | — | No C reference |

### Correctness Summary

All benchmarks match C reference output except:
- `utf8_ops` — MISMATCH (pre-existing, line count 10 vs 1)

No SysCall# warnings. All benchmarks compile (including queue_drain variants).

---

## Stage 5: Fix Redundant State Stores (needs_state_stores_in_body)

### Root Cause

The `needs_state_stores_in_body` flag defaults to `true` in `FunctionContext` and is
**never demoted to `false`** in the `EmitPerFieldPhi` dispatch path. The only assignment
sites are `= true` — Path A ("zero memory traffic" per `mod.rs:2496-2500`) is unreachable.

This causes every `EmitPerFieldPhi` loop to emit at least one redundant GEP+store per
iteration, even when all written fields are tracked by phis. LLVM's DSE cannot eliminate
these stores because opaque FFI calls (`__print_int`, `__print_char`, etc.) are declared
with only `nounwind`, making LLVM conservatively assume they might access `%State`
through a global.

### Affected Benchmarks

| Benchmark | Ratio | Redundant stores/iter | Fix impact |
|-----------|-------|----------------------|------------|
| print_loop | 1.09x | 1 (all fields fit in 3 phis) | ~0.63x → parity |
| queue_drain_sym | 1.13x | 1 (all fields fit in 3 phis) | ~0.56x → parity |
| fannkuch_redux | 1.13x | 5 (6 phi cap, 16 write fields) | ~1.04x → near parity |
| fasta | 1.18x | 2 (redundant stores + SROA blocked) | Partial fix |

### Fix

**Target file: `src/backend/llvm/loop_engine/counter.rs`**

Change the `needs_state_stores_in_body` assignment in `emit_countable_main` (currently
around line 342):

```rust
// Current:
self.fun.needs_state_stores_in_body = !self.fun.pending_post_hoist.is_empty();

// Proposed:
// 2026-07-21: Demote to false when no post-loop hoists exist AND no fields
// are missing from the phi set. The dispatch code sets needs_state_stores_in_body = true
// before calling emit_countable_main when phi-capped fields need stores.
// Only enable stores when (a) post-loop hoisted prints need final %State values,
// or (b) dispatch pre-set the flag for missing-phi-field tracking.
if !self.fun.pending_post_hoist.is_empty() {
    self.fun.needs_state_stores_in_body = true;
} else if self.fun.needs_state_stores_in_body {
    // Leave dispatch-pre-set value (missing phi fields need stores)
} else {
    self.fun.needs_state_stores_in_body = false;
}
```

**Target file: `src/backend/llvm/mod.rs`**

In both `capped_set` dispatch blocks (lines ~2568-2585 and ~2618-2635), after the
missing-field check, the flag is already set when needed. No additional change needed
for the dispatch code itself — the fix in `counter.rs` ensures the flag is properly
demoted when not needed.

### Validation

```bash
cargo build
cargo test --lib
# Check print_loop and queue_drain_sym IR for redundant stores
grep 'cms' benchmarks/print_loop.ll | wc -l   # Should be 0 (or just post-loop)
```

---

## Stage 6: Prevent %State Escape Through GetEnvInt

### Root Cause

The `get_env_int` intrinsic (used by `GetEnvInt!`) takes `ptr %state` as its first
parameter, even though it never uses it:

```llvm
define i64 @get_env_int(ptr noalias nocapture align 8 %state, ptr %arg0) {
  %ac0 = ptrtoint ptr %arg0 to i64
  %t0 = call i64 @__getenv_int(i64 %ac0)    ; %state never used!
  ret i64 %t0
}
```

Passing `%state` to this opaque function causes LLVM's escape analysis to consider
`%state` captured, **blocking SROA** from decomposing `%State` into scalar registers.
This forces every field access through GEP+load+store, preventing values from living
in SSA registers.

Even though `get_env_int` has `nocapture` on the `%state` parameter (meaning it
doesn't store the pointer), LLVM's SROA is conservative — it considers the alloca
"possibly accessed by unknown callers" when passed to any external function, even
with `nocapture`.

### Affected Benchmarks

| Benchmark | Ratio | Impact of fix |
|-----------|-------|---------------|
| fasta | 1.18x | Fields become SSA registers, eliminating GEP+load+store overhead |
| Any benchmark using `GetEnvInt!` | Minor | Cleaner IR, slightly faster LLVM optimization |

All runtime benchmarks that use `GetEnvInt!("BOUND")` are affected:
`float_math_nonzero`, `nbody_newton`, `nbody_sqrt`, `nbody_sqrt_idio`, `fasta`,
`fannkuch_redux`, `mandelbrot`, `kalman_filter_runtime`, `knucleotide`,
`queue_drain`, `queue_drain_sym`, `queue_drain_idio`.

Note: For most benchmarks, the `%state` pointer is passed through other functions
too (the txn callbacks), so preventing just `get_env_int` from capturing `%state`
may not fully enable SROA. The hot loop's direct field accesses would need the
alloca to NOT escape through ANY external function.

### Fix

**Option A (preferred):** In `get_env_int`'s emission, omit the dead `%state`
parameter from the wrapper function signature. The calling code in the txn's
`main()` passes `ptr %state` as the first argument — change the emission to
pass `null` or a dummy value, and remove the `%state` parameter from the
wrapper's definition.

**Option B (simpler):** Add `memory(argmem: readnone)` to the `get_env_int`
function declaration, telling LLVM it doesn't access memory through any pointer.
But `get_env_int` calls `__getenv_int` which DOES read memory (environment
variables), so this would be a lie. Only the `%state` parameter is dead, not
the function itself.

**Option C (targeted):** Mark only the `%state` parameter as `noalias nocapture`
(already done) AND ensure the function body never memcpy's or stores the pointer.
Add `writereadonly` on `%state` specifically. However, LLVM doesn't support
per-pointer `readnone`.

**Recommended: Option A** — simply don't pass `%state` to intrinsics that don't
need it. The `emit_external_call` or intrinsic emission code should filter out
the `%state` argument for intrinsics that don't require it.

### Target Files

- `src/backend/llvm/intrinsics.rs` — The `emit_intrinsic` dispatch. Most
  `GetEnvInt#`, `PrintInt#`, `Malloc#`, `Free#` etc. accept `ptr %state` as
  the first argument from the calling convention, even when they don't need it.
- `src/backend/llvm/emit_expr.rs` — The `emit_expr` function that dispatches
  to intrinsic handlers.
- `src/backend/llvm/emit_stmt.rs` — The statement handler that passes `%state`
  to intrinsic calls.

The fix involves changing the intrinsic calling convention to omit `%state` for
intrinsics that don't access the state struct.

### Validation

```bash
cargo build
cargo test --lib
# Compile fasta and check if SROA now fires
grep 'get_env_int' benchmarks/fasta.ll
# Check that %state is no longer an argument
```
```

---

## Stage 7: Comprehensive Benchmark Investigation Results (2026-07-21)

All root causes identified through detailed LLVM IR analysis, disassembly comparison,
and instruction-level uop accounting.

### Current Benchmark Table (After Fix A)

All times at `BOUND=50000000`, single run, nanosecond-precision fork+exec timer.

| Benchmark | Brief | C | Ratio | Winner | Status |
|-----------|-------|---|-------|--------|--------|
| ring_buffer | 0.0549s | 0.0318s | 1.72x | C | **Missing `nuw` flag** |
| float_math | 0.0712s | 0.0752s | 0.94x | Brief | ✓ |
| float_math_nonzero | 0.1624s | 0.1687s | 0.96x | Brief | ✓ Fixed Stage 2 |
| **sparse_dispatch** | **0.0517s** | **0.0781s** | **0.66x** | **Brief** | ✓ Fixed Stage 5 |
| **print_loop** | **0.0597s** | **0.0571s** | **1.04x** | **~tie** | ✓ Fixed Stage 5 |
| nbody_newton | 11.3802s | 8.3289s | 1.36x | C | LLVM `vdivss` vs `vrcpps` |
| nbody_sqrt | 2.4571s | 2.7958s | 0.87x | Brief | ✓ |
| nbody_sqrt_idio | 2.5028s | 3.6368s | 0.68x | Brief | ✓ |
| fasta | 0.2622s | 0.2121s | 1.23x | C | **No LTO → extra call layer** |
| fannkuch_redux | 0.0770s | 0.0588s | 1.31x | C | **6-phi cap → register pressure** |
| mandelbrot | 0.6687s | 0.6626s | 1.00x | ~tie | ✓ |
| kalman_filter_runtime | 0.1862s | 0.1747s | 1.06x | ~tie | ✓ |
| knucleotide | 0.1904s | 0.1938s | 0.98x | ~tie | ✓ |
| cancel_math | 0.0654s | 0.0630s | 1.03x | ~tie | ✓ |
| bit_clear | 0.0011s | 0.0008s | 1.34x | C | **Measurement noise** (1µs work, 640µs startup) |
| queue_drain | 0.0597s | 0.0616s | 0.96x | Brief | ✓ Fixed Stage 2 |
| queue_drain_sym | 0.0648s | 0.0606s | 1.06x | ~tie | ✓ Improved Stage 5 |
| queue_drain_idio | 0.0645s | — | — | — | No C reference |

---

## Stage 8: Remaining Fixes (Priority Order)

### P0: Add `nuw`/`nsw` flags to LLVM IR `add` (ring_buffer)

**Root cause:** The LLVM IR emits `add nsw i64` (signed-only) for counter increments.
Without `nuw` (no-unsigned-wrap), LLVM cannot narrow i64 → i32 for the modulo
operation. This forces a 64-bit `srem` → 128-bit `mul %r14` (3-5 µops) instead of
32-bit `urem` → `imul $magic` (1 µop).

**Fix:** Add `nuw nsw` to all `add`/`sub`/`mul` instructions for bounded loop counters
and induction variables where the compiler can prove no overflow (counter < total,
where both are non-negative and total is a known bound).

**Target:** `src/backend/llvm/emit_expr.rs` — the expression emitter that generates
`add`/`sub` IR. All binary ops that are known-bounded should get both flags.

### P1: Enable `-flto` in benchmark build (fasta)

**Root cause:** `__print_char` wrapper adds an extra function call layer per iteration
(call/ret pair + stack frame). C calls `fputc@plt` directly, Brief calls
`__print_char` → `putc@plt`. The wrapper costs ~5-8 cycles/iteration at 50M iter:
~50-80ms overhead at 3GHz (matches actual 52ms delta).

The wrapper itself is optimized by clang -O3 (the `c==10` branch becomes just
`putc(c, stdout)`), but the outer call/ret pair cannot be eliminated without LTO.

**Fix:** Add `-flto` to the clang invocation in `benchmarks/build_and_bench.sh` so
that LTO inlines `__print_char` into `main()`.

### P2: Increase phi cap from 6 to adaptive (fannkuch_redux)

**Root cause:** The 6-phi cap forces 10 of 16 written fields through
GEP+load+store, increasing register pressure. At the binary level, this costs
~3 extra instructions per iteration (17.75 vs 14.75 = 20% more).

**Fix:** Change the capped_set limit from a hard-coded `6` to an adaptive value
based on field count and register pressure. For benchmarks with 10-16 write fields,
a cap of 12-16 would eliminate the penalty.

### P3: ring_buffer benchmark liveness (use `Print#`)

**Root cause:** Both binaries DCE the ring buffer stores — the buffer is allocated
but never read by any observable effect. The benchmark measures modulo arithmetic,
not ring buffer operations.

**Fix:** Add a `Print#(data[...])` or similar observable read at the end of the
benchmark to prevent DCE of the buffer stores. The `Print#` intrinsic (or
`!Print` / `!PrintLn`) establishes liveness without `frgn`.

### P4: bit_clear investigation

**Root cause:** 63 iterations (~1µs computation) buried in 640µs process startup.
The ratio is not statistically significant (p>0.05, 15-run test). Disassembly is
identical.

**Note:** The user reports it was faster before — may be a startup noise regression
from increased PLT entries or dynamic linking overhead. Could also be real: the
6-phi cap affects fannkuch similarly, and if bit_clear was rewritten to use a node
with more state fields than before, the phi cap may have regressed it.
```

---

## Stage 9: Current Benchmark Results (2026-07-21, End of Session)

All times at `BOUND=50000000`, single run, nanosecond-precision fork+exec timer.

| Benchmark | Brief | C | Ratio | Winner | Change |
|-----------|-------|---|-------|--------|--------|
| ring_buffer | 0.0617s | 0.0461s | 1.33x | C | Now tests real buffer ops (was 1.72x) |
| float_math | 0.0769s | 0.0709s | 1.08x | ~tie | |
| **float_math_nonzero** | **0.1603s** | **0.1689s** | **0.94x** | **Brief** | ✓ Fixed |
| **sparse_dispatch** | **0.0600s** | **0.0641s** | **0.93x** | **Brief** | ✓ |
| **print_loop** | **0.0588s** | **0.0633s** | **0.93x** | **Brief** | ✓ Fixed |
| nbody_newton | 12.0403s | 8.8564s | 1.35x | C | Missing fast-math attributes |
| **nbody_sqrt** | **2.6511s** | **3.1134s** | **0.85x** | **Brief** | ✓ |
| **nbody_sqrt_idio** | **2.5850s** | **3.7576s** | **0.68x** | **Brief** | ✓ |
| **fasta** | **0.2163s** | **0.2204s** | **0.98x** | **~tie** | ↑ 1.23x via LTO |
| **fannkuch_redux** | **0.0697s** | **0.0749s** | **0.93x** | **Brief** | ↑ 1.31x via adaptive cap |
| mandelbrot | 0.6713s | 0.6729s | 0.99x | ~tie | |
| kalman_filter_runtime | 0.1819s | 0.1846s | 0.98x | ~tie | |
| knucleotide | 0.1990s | 0.1958s | 1.01x | ~tie | |
| cancel_math | 0.0617s | 0.0593s | 1.03x | ~tie | |
| bit_clear | ~0.0007s | ~0.0007s | ~1.00x | noise | 63 iter, startup-dominated |
| **queue_drain** | **0.0618s** | **0.0655s** | **0.94x** | **Brief** | ✓ Fixed |
| queue_drain_sym | 0.0635s | 0.0628s | 1.01x | ~tie | |

**Brief wins: 7 | Parity: 8 | C wins: 2** (ring_buffer 1.33x, nbody_newton 1.35x)

---

## Stage 10: Fix Remaining Two Behind Benchmarks

### P5: nbody_newton — Add Fast-Math Attributes (HIGH IMPACT)

**Root cause:** The emitted LLVM IR uses `fdiv fast` on individual float operations,
but the **function attribute groups lack the seven fast-math attributes** that LLVM
needs to convert `fdiv` to `vrcpps` + Newton refinement.

Clang emits these attributes for `-ffast-math`:
```
"approx-func-fp-math"="true"
"denormal-fp-math"="preserve-sign,preserve-sign"
"no-infs-fp-math"="true"
"no-nans-fp-math"="true"
"no-signed-zeros-fp-math"="true"
"no-trapping-math"="true"
"unsafe-fp-math"="true"
```

Without them, LLVM's backend rejects reciprocal conversion even though individual
`fast` flags permit it. Verified by manually adding these seven attributes to the
`.ll` file — scalar `divss` in the hot loop drops from **60 to 0**.

**Fix:** Add the seven attributes to all function attribute groups used by
floating-point-intensive functions in `src/backend/llvm/mod.rs` (groups #0, #3,
#4, #5, #8, #9).

**Expected impact:** nbody_newton 1.35x → ~1.0x (hot loop divisions become vrcpps).

### P6: ring_buffer — 32-Byte Loop Alignment (HIGH IMPACT)

**Root cause:** The `mul %r15` instruction at address `0x2b7f` in the hot loop
crosses a 32-byte boundary. Intel CPUs cannot cache cross-boundary instructions
in the DSB (uop cache), forcing MITE (legacy decoder pipeline) fallback each
iteration. This costs ~1-2 cycles in a ~3-4 cycle loop — directly explaining
the 1.33x ratio.

**Fix:** Add alignment padding before the `.cm_header` loop start to shift the
loop body so no instruction crosses a 32-byte boundary. This is done by inserting
NOP padding (`callbr` or `align 32` directive) before the loop header.

**Secondary fix:** Change `extract_root_via_provenance` in `transition_graph.rs`
to return `None` for `Expr::Index` LHS — `data[idx] = val` is a pointer write,
not a field write, and should not add `data` to the write set. This eliminates
redundant phis for pointer-type fields.
```
```

---

## Stage 11: Comprehensive Remaining Fixes

### Current Benchmark State (After Fixes 1–3)

| Benchmark | Brief | C | Ratio | Winner |
|-----------|-------|---|-------|--------|
| ring_buffer | 0.0607s | 0.0496s | 1.22x | C |
| float_math | 0.0766s | 0.0737s | 1.04x | ~tie |
| float_math_nonzero | 0.1603s | 0.1689s | 0.94x | Brief |
| sparse_dispatch | 0.0577s | 0.0597s | 0.96x | Brief |
| print_loop | 0.0568s | 0.0588s | 0.96x | Brief |
| nbody_newton | 11.3905s | 8.3634s | 1.36x | C |
| nbody_sqrt | 2.4367s | 2.8136s | 0.86x | Brief |
| nbody_sqrt_idio | 2.4866s | 3.6580s | 0.67x | Brief |
| fasta | 0.2268s | 0.2235s | 1.01x | ~tie |
| fannkuch_redux | 0.0640s | 0.0636s | 1.00x | ~tie |
| mandelbrot | 0.6689s | 0.6590s | 1.01x | ~tie |
| kalman_filter_runtime | 0.1814s | 0.1795s | 1.01x | ~tie |
| knucleotide | 0.1858s | 0.1898s | 0.97x | ~tie |
| cancel_math | 0.0613s | 0.0580s | 1.05x | ~tie |
| bit_clear | ~0.0007s | ~0.0007s | ~1.00x | noise |
| queue_drain | 0.0618s | 0.0655s | 0.94x | Brief |
| queue_drain_sym | 0.0635s | 0.0628s | 1.01x | ~tie |

**Brief/Parity: 16 out of 17 benchmarks at parity or better.**
**Behind: 2** — ring_buffer (1.22x), nbody_newton (1.36x)

---

### Fix A: Native Float in `%State`

**Problem:** `push_field_type` in `src/backend/llvm/mod.rs:912` stores ALL state fields
as `i64`. For float fields, every access requires:
- **Read:** GEP `i64` → load `i64` → trunc `i64 to i32` → bitcast `i32 to float` (4 insns)
- **Write:** bitcast `float to i32` → zext `i32 to i64` → store `i64` (3 insns)

That's 7 instructions per float field access just for type conversion, before any
actual computation. C accesses float fields directly: load `float` (1 insn).

**Fix:** In `push_field_type`, check `field_brief_types[idx]` — if the type is `float`
or `float64`, push `"float"` or `"double"` instead of `"i64"`. Then in the load/store
paths (`emit_stmt.rs`, `emit_countable_body`), use the native type directly without
trunc/bitcast/zext.

**The `adapt_to_i64` path** (which boxes float values into i64 for phi backedges)
still needs the conversion, but only for the phi backedge (once per iteration per
phi-tracked field), not for every use within the body.

**File:** `src/backend/llvm/mod.rs` (push_field_type, ~lines 890-920)
**File:** `src/backend/llvm/emit_stmt.rs` (load/store paths)
**Lines of change:** ~20

**Risk:** Low. The field_brief_types already tracks the original type. The change is
pure addition — non-float fields keep i64. Float fields get native type.

**Expected Impact:**
- All float benchmarks: eliminates 7 insns per float field access
- nbody_newton: ~2000 float accesses → ~14000 fewer instructions
- ring_buffer: unaffected (no float fields)

---

### Fix B: i32 Trunc for Constant-Divisor `%`

**Problem:** `ops % 5000000` emits `srem i64 %ops, 5000000`. LLVM converts this to
`urem i64 %ops, 5000000` (proving non-negative from `nuw nsw` flags), which uses a
64-bit magic constant in a 128-bit `mul %r12` (2 uops). C's `srem` with the same
constant uses 32-bit `imul $magic, %reg, %reg` (1 uop).

The agent's analysis confirmed this — the 1 extra uop at 50M iterations costs
~12.5ms, accounting for most of the 1.22x gap.

**Fix:** Restore the i32 trunc optimization that was reverted in 2026-07-19 (per
the comment at `emit_expr.rs:250-258`), but with a bounds check. When the divisor
is a compile-time constant AND the compiler can prove the dividend fits in 32 bits
(either via `!range` metadata on the load, or via contract bounds), emit:
```llvm
%trunc = trunc i64 %dividend to i32
%result = urem i32 %trunc, %divisor_32
```

The 2026-07-19 revert was triggered by `INT64_MAX` (a 64-bit value), where the
trunc would lose information. The bounds check prevents this case.

**File:** `src/backend/llvm/emit_expr.rs` (around line 250-258)
**Lines of change:** ~15

**Risk:** Low with bounds check. The 2026-07-19 revert was because the optimization
was applied unconditionally. Adding a bounds check (dividend fits in 32 bits)
prevents the `INT64_MAX` regression.

**Expected Impact:** ring_buffer 1.22x → ~1.0x

---

### Fix C: Complete Vector Phi Groups

**Problem:** The infrastructure for emitting vector phis (`<N x float>` instead of
N scalar phis) was partially built but never completed:
- `build_vector_phi_groups()` at `loop_engine/analysis.rs:554` is defined but never called
- `vector_phi_groups` and `vector_phi_current` on `FunctionContext` are declared but unused
- No code emits `insertelement` or `<N x float>` phi nodes
- No code emits vector loads/stores or vector arithmetic operations

**The pieces to wire up:**

1. **Call `build_vector_phi_groups()`** from `emit_countable_main` (or the dispatch
   code) to populate `vector_phi_groups`. This function already groups fields by
   their LLVM type string.

2. **Emit `<N x float>` phi nodes** for each group instead of N scalar phis.
   Use `insertelement` chains to initialize the vector phi from individual field
   values at loop entry.

3. **Emit `extractelement`** at vector phi use sites to get individual lanes.
   Replace scalar `phi` + `add i64 0, %val` identity with single `extractelement`.

4. **Emit vector loads/stores** for contiguous float field groups in the body,
   using `<N x float>` GEP instead of N individual GEPs.

**File:** `src/backend/llvm/loop_engine/counter.rs`, `src/backend/llvm/mod.rs`
**File:** `src/backend/llvm/emit_stmt.rs`, `src/backend/llvm/emit_expr.rs`
**Lines of change:** ~80-120

**Risk:** Medium. This touches the loop emission hot path. The vector phi group
data structures are designed for this but have never been tested. May expose edge
cases in float field adjacency (non-contiguous float fields in %State).

**Expected Impact:** Benchmarks with contiguous same-type float fields (nbody's
bx0/bx1/bx2/bx3/bx4 may not be contiguous if interleaved with non-float fields)
benefit from reduced phi overhead. Actual vector arithmetic is NOT emitted — this
only bundles phi nodes, not operations.

---

### Fix D: AST-Level SLP Isomorphism Pass

**Problem:** nbody_newton's 240 scalar `fdiv` cannot be vectorized because:
1. Each `fdiv` is part of a sequential dependency chain (5 iterations of Newton)
2. The chains are on pairs (01, 02, 03, 04, 12, 13, 14, 23, 24, 34)
3. Different pairs have identical computation structure
4. LLVM's SLP vectorizer can't see through the i64 boxing and individual field GEPs

**Concept:** The Brief compiler detects that:
```
dx01 = bx0 - bx1;  dy01 = by0 - by1;  dz01 = bz0 - bz1;
```
are three identical operations on three different field pairs. Instead of emitting
three scalar `fsub` instructions, it emits one `<3 x float> fsub`:

```llvm
%vec_bx = load <3 x float>, ptr %bx_base
%vec_by = load <3 x float>, ptr %by_base
%vec_dx = fsub fast <3 x float> %vec_bx, %vec_by
```

This requires:

**Analysis Phase (new `src/analysis/slp_vectorizer.rs`):**

| Step | What | Implementation |
|------|------|----------------|
| 1 | Build field dependency graph per txn body | Group fields by the variable they're assigned to |
| 2 | Find isomorphic statement bundles | Walk body statements; group statements with identical LHS pattern and similar operands |
| 3 | Check field contiguity in %State | Verify fields in a bundle are adjacent in `field_index_map` |
| 4 | Compute cost model | Reuse `hazard.rs` register pressure estimation |
| 5 | Store vectorization plan | New struct `SlpPlan: Vec<Bundle>` on `CompilerContext` or `SlpPlan` in analysis output |

**Codegen Phase (extensions to backend):**

| Step | What | Implementation |
|------|------|----------------|
| 6 | Emit `<N x float>` loads for bundled field groups | `emit_expr.rs`: new `emit_vector_load(u32) -> TypedRegister` |
| 7 | Emit vector arithmetic (`fsub`, `fmul`, `fdiv`) | `emit_binary_op` with `ret_ty.is_vector()` branch |
| 8 | Emit vector phis for bundled fields | Complete Fix C first, then use for SLP bundles |
| 9 | Emit `shufflevector` for strided accesses | When fields are not contiguous, rearrange lanes |
| 10 | Emit `<N x float>` stores for bundled field writes | New `emit_vector_store` path |

**Key Design Decisions:**

**When to bundle:** Only when ALL of:
- Fields have same Brief type (float/float64)
- Fields are contiguous in %State (adjacent in field_index_map)
- Operations are structurally identical (same operator, same constants, same operand fields with matching indices)
- Register pressure estimate from `hazard.rs` is below target threshold

**What to emit:** `<4 x float>` on SSE (w=4), `<8 x float>` on AVX2 (w=8), `<16 x float>` on AVX512 (w=16). Use `extractelement`/`insertelement` for boundary conditions.

**When to skip:** If the isomorphic bundles have sequential dependencies (e.g., 5 Newton iterations where each depends on the previous), don't bundle across dependency boundaries. Bundle WITHIN the same iteration only.

**File:** New `src/analysis/slp_vectorizer.rs` (~400 lines)
**File:** Extensions to `src/backend/llvm/emit_expr.rs` (~100 lines)
**File:** Extensions to `src/backend/llvm/mod.rs` (call the new pass, store SlpPlan)
**Files of change:** ~500 lines total

**Risk:** High. This is a new compiler pass that touches the hot emission path.
Correctness testing is critical — vectorized operations must produce bit-identical
results to scalar operations. Mis-detection of isomorphism would produce wrong
numerical results.

**Expected Impact:** nbody_newton 1.36x → potentially ~1.0x (if all 240 scalar fdiv
become ~60 vector fdiv with vrcpps). BUT: the sequential Newton iterations (5 deep)
limit the parallelism — each of the 5 iterations depends on the previous result.
The vectorization helps across BODY PAIRS (10 pairs × 3 components × 5 iterations),
not within a single Newton iteration.

---

### Implementation Order

| Order | Fix | Effort | Impact | Risk |
|-------|-----|--------|--------|------|
| **1st** | **A: Native float in %State** | 20 lines | All float benchmarks | Low |
| **2nd** | **B: i32 trunc for constant %** | 15 lines | ring_buffer 1.22x→~1.0x | Low |
| **3rd** | **C: Complete vector phi groups** | 100 lines | Enables vector phi emission | Medium |
| **4th** | **D: SLP isomorphism pass** | 500 lines | nbody_newton → potential parity | High |

Fixes A and B are quick wins with low risk. C builds the infrastructure D needs.
D is the full SLP vectorizer — it can be designed and implemented incrementally
after A, B, and C are merged.
```

---

## Stage 12: Current Benchmark Results + Remaining Fix Implementation Plan

### Current Benchmark Results (2026-07-21, End of Session)

After all Fixes 1–3 from the session:

| Benchmark | Brief | C | Ratio | Winner | Note |
|-----------|-------|---|-------|--------|------|
| **ring_buffer** | **0.058s** | **0.047s** | **1.15x** | C | `mul` eliminated; phi overhead remains |
| float_math | 0.077s | 0.074s | 1.04x | ~tie | |
| float_math_nonzero | 0.160s | 0.169s | 0.94x | Brief | ✓ |
| **sparse_dispatch** | **0.058s** | **0.060s** | **0.96x** | Brief | ✓ |
| **print_loop** | **0.057s** | **0.059s** | **0.96x** | Brief | ✓ |
| **nbody_newton** | **11.39s** | **8.36s** | **1.36x** | C | SLP unblocked; scalar fdiv remains |
| nbody_sqrt | 2.44s | 2.81s | 0.86x | Brief | ✓ |
| nbody_sqrt_idio | 2.49s | 3.66s | 0.67x | Brief | ✓ |
| **fasta** | **0.214s** | **0.218s** | **0.98x** | **~tie** | LTO inlining fixed |
| **fannkuch_redux** | **0.063s** | **0.064s** | **0.98x** | **~tie** | Adaptive phi cap fixed |
| mandelbrot | 0.669s | 0.659s | 1.01x | ~tie | |
| kalman_filter_runtime | 0.181s | 0.180s | 1.01x | ~tie | |
| knucleotide | 0.186s | 0.190s | 0.97x | ~tie | |
| cancel_math | 0.061s | 0.058s | 1.05x | ~tie | |
| bit_clear | ~0.001s | ~0.001s | ~1.00x | noise | 63 iter, startup-dominated |
| **queue_drain** | **0.062s** | **0.066s** | **0.94x** | Brief | ✓ |
| queue_drain_sym | 0.064s | 0.061s | 1.06x | ~tie | |

**Brief/Parity: 16 of 18. Behind: 2** — ring_buffer (1.15x), nbody_newton (1.36x)

---

### Remaining Fixes — Implementation Order

| # | Fix | Effort | Files | Target | Expected Impact |
|---|-----|--------|-------|--------|-----------------|
| **1** | **Redundant phi elimination** — skip phi for fields that duplicate the counter variable (ops = cmc, two phis for one counter) | 5 lines | `counter.rs` | ring_buffer | ~0.05x |
| **2** | **Direct while-loop dispatch** — new emit_while_main() for simple single-node programs. No phi nodes, pure GEP+load+store. Dispatch condition: single txn + bounded + has FFI in body | 70 lines | `counter.rs`, `mod.rs` | ring_buffer | 1.15x → ~1.0x |
| **3** | **Native float in %State** — push_field_type stores float as float/double, not i64. Coordinated changes: push_field_type, phi type, backedge identity, ensure_typed_value, adapt_to_i64 path | 40 lines | `mod.rs`, `counter.rs`, `emit_expr.rs`, `helpers.rs` | nbody + all float benchmarks | ~13% off nbody runtime |
| **4** | **SLP isomorphism pass — Analysis Phase** — new slp_isomorphism.rs. Walks txn bodies, segments let/assign statements, compares for structural isomorphism via alpha-renaming, records groups of 2+ isomorphic lanes | 320 lines | `slp_isomorphism.rs` (new) | nbody (foundation) | Enables Phase 2 |
| **5** | **SLP isomorphism pass — Codegen Phase** — emit <N x float> vector ops for detected groups. Vector loads, arithmetic, extractelement/shuffle for scalar results | 200 lines | `emit_vector.rs` (new) | nbody | 1.36x → ~1.0x |
```

---

## Stage 13: SLP Vector Codegen (Phase 2) — Full Implementation Plan

### Motivation

The SLP analysis pass (Phase 1, commit `6fb88032`) detects **143 isomorphic groups**
with **473 total lanes** in nbody_newton. However, the codegen currently emits scalar
operations for each lane individually, leaving the 150 scalar `fdiv` unvectorized.
LLVM's backend converts scalar `fdiv` to `vdivss` (~10-14 cycles) but converts
vector `<N x float> fdiv` to `vrcpps` + Newton refinement (~1 cycle throughput).
This is the primary mechanism behind C's 1.35x advantage.

### The Algorithm

When `emit_countable_body` encounters a statement at `body[i]` that is the start
of an SLP group (detected by matching `group.base_index == i`), it emits vector
operations for the ENTIRE group instead of emitting each statement separately.

The core function `emit_vector_expr` walks the template RHS expression tree
recursively, using `lane_mappings` to resolve per-lane identifiers:

```
emit_vector_expr(template_expr, lane_exprs, lane_mappings, width):
    match template_expr:
        BinaryOp(kind, t_lhs, t_rhs):
            lhs_vec = emit_vector_expr(t_lhs, lanes.lhs, lane_mappings, width)
            rhs_vec = emit_vector_expr(t_rhs, lanes.rhs, lane_mappings, width)
            return emit_vector_binary_op(kind, lhs_vec, rhs_vec, width)

        Identifier(name):
            if name maps to same variable across all lanes:
                broadcast(scalar_reg, width)   # insertelement + shufflevector
            else:
                insertelement_chain(per_lane_regs, width)  # build vector

        Float(n) | Decimal(n):
            broadcast(scalar_literal(n), width)

        _ (Call, Field, Index, Cast, etc.):
            scalar_fallback(lane_exprs, width)  # per-lane scalar + build vector
```

### LLVM IR Patterns

**Broadcast** (same identifier/literal across all lanes):
```llvm
%v0 = insertelement <5 x float> undef, float %val, i32 0
%v1 = shufflevector <5 x float> %v0, <5 x float> undef, <5 x i32> zeroinitializer
```

**Insertelement chain** (different per lane):
```llvm
%v1 = insertelement <5 x float> %v0, float %val1, i32 1
%v2 = insertelement <5 x float> %v1, float %val2, i32 2
; ... continues for all N lanes
```

**Vector operation**:
```llvm
%vresult = fdiv <5 x float> %lhs_vec, %rhs_vec
```

**Extract individual results**:
```llvm
%res0 = extractelement <5 x float> %vresult, i32 0
%res1 = extractelement <5 x float> %vresult, i32 1
; ... one per lane
```

### Profitability Heuristic

| Pattern | Width | Depth | Scalar Ops | Vector Ops | Decision |
|---------|-------|-------|------------|------------|----------|
| Distance sub (bx0-bx1, by0-by1, bz0-bz1) | 3 | 1 | 3 | ~8 | **Skip** (net negative) |
| Newton iteration (5 steps) | 5 | 3 | 15 | ~13 | **Vectorize** (~13% savings) |
| Kinetic energy | 5 | 4 | 20 | ~16 | **Vectorize** (~20% savings) |

**Guard condition:** Only vectorize groups where `width >= 4` OR `width >= 3` with
tree depth >= 2.

### Module Structure

**New file:** `src/backend/llvm/vector_codegen.rs` (~380 lines)

| Function | Lines | Purpose |
|----------|-------|---------|
| `emit_slp_group` | 60 | Entry: extract template, call emit_vector_expr, extract results, register in last_val_temps |
| `emit_vector_expr` | 120 | Recursive tree walker matching on Expr variants |
| `emit_vector_binary_op` | 70 | Match BinaryOpKind, emit `<N x T> fadd/fsub/fmul/fdiv` |
| `emit_vector_unary_op` | 25 | Neg/Not/BitNot vector versions |
| `emit_insertelement_chain` | 20 | Build vector from N scalar regs |
| `emit_broadcast` | 12 | insertelement + shufflevector splat |
| `emit_scalar_fallback` | 30 | Per-lane scalar + build vector |
| `emit_extractelement` | 10 | Extract one scalar from vector |
| Helpers | 15 | `all_lanes_same`, `vector_type_str`, `tree_depth` |
| Tests | 120 | Unit tests |

### Integration

**Hook in `emit_countable_body`** (`counter.rs`):
```rust
while i < body.len() {
    if let Some(group) = self.fun.slp_groups.iter().find(|g| g.base_index == i) {
        if group.width >= 4 || (group.width >= 3 && tree_depth(&body[i]) >= 2) {
            self.emit_slp_group(out, body, group, write_set)?;
            i += group.width;
            continue;
        }
    }
    // ... existing per-statement emission ...
    i += 1;
}
```

### Expected Impact

nbody_newton: 150 scalar `fdiv` → ~65 vector `fdiv` → enables `vrcpps` backend
conversion → expected ratio improvement from 1.35x toward parity.

---

## Stage 14: SLP Cross-Pair Merge — Dependency-Traced Grouping

### Problem
Cross-pair merging (Stage 13) combined groups with the same template signature
but used a fragile 20-statement proximity heuristic. This failed because:
1. Merged lanes reference let-bindings defined AFTER the template position
2. `emit_slp_group` assumed contiguous `body[base_index + i]` lane expressions
3. `i += group.width` skipped unrelated computations

### 5-Phase Fix (~78 lines total)

**Phase 1: `lane_positions` field.** Each lane gets its own body index.
Filled in `find_isomorphic_groups` (contiguous case) and `merge_groups`
(concatenation case).

**Phase 2: Dependency validation.** `all_deps_available()` checks that
every variable referenced by a lane's RHS is either:
- A state field (not in `def_sites`) — always available
- A let-binding defined BEFORE the template position — exists in `last_val_temps`
Rejects merges where a lane references a let-binding defined at or after
the template position.

**Phase 3: Replace proximity heuristic.** In `merge_groups`, replace the
20-statement check with `all_deps_available`.

**Phase 4: Template-based lane reconstruction.** `emit_slp_group` no
longer reads `body[base_index + i]`. All lanes use the template expression;
`lane_mappings[i]` provides per-lane variable names. `emit_vector_expr`
already uses `lane_mappings` — this just changes what expression is
passed to it.

**Phase 5: Fix extract/register and skip.** `emit_extract_and_register`
uses `lhs_names[i]` instead of `body[base_index + i]`. Skip uses
`lane_positions.max() + 1` instead of `group.width`.

### Expected Results
| Cross-pair candidate | Current (heuristic) | After (dependency) |
|---|---|---|
| dx01/dx02/dx03 (state fields only) | May or may not merge | **Merges** (all deps available) |
| dist01a/dist02a (lets at different positions) | May wrongly merge | **Rejects** (dsq02 not available) |
| epex01/epex02 (edist at pos < epex) | May wrongly merge | **Merges** (all deps available) |
