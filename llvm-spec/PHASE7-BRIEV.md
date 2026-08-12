# Phase 7 Briev: Self-Hosted LLVM Backend Parity

**Date:** 2026-05-29  
**Spec Reference:** `11-SELF-HOSTED.md`  
**Prerequisite:** All Rust backend phases 0-5.5 complete  
**Estimated Effort:** 5 days  

## Goal

`lib/compiler/backends/llvm.bv` exists and mirrors every Rust backend phase. `lib/compiler/main.bv` has a `[state.backend == "llvm"]` dispatch arm. The self-hosted pipeline produces valid `.ll` files that pass `llc`.

## Architecture

Follows the established `c.bv`/`wasm.bv` pattern:

```
lib/compiler/backends/llvm.bv   — LLVM IR emitter (main backend file)
lib/compiler/backends/mod.bv     — Add "llvm" to supported_hashtags
lib/compiler/main.bv            — Add llvm dispatch arm
```

## Functions to Implement in `llvm.bv`

Each Rust backend method becomes a Briev `defn`:

| Rust Method | Briev defn | Phase Ported |
|-------------|-----------|--------------|
| `generate()` | `generate_llvm(program, cg) -> String` | Entry |
| `declare_state_type()` | Inline in entry | P0 |
| `build_field_index()` | `collect_fields(program) -> List<FieldInfo>` | P0 |
| `generate_transaction()` | `emit_txn(txn) -> String` | P1 |
| `generate_definition()` | `emit_defn(defn) -> String` | P1 |
| `emit_precondition()` | `emit_pre_condition(contract) -> String` | P2 |
| `generate_precondition_function()` | `emit_pre_func(txn) -> String` | P2 |
| `generate_fused_transaction()` | `emit_fused(a, b) -> String` | P2.5 |
| `emit_match_to_switch()` | `emit_match(expr) -> String` | P3 |
| `emit_unification()` | `emit_uni(uni) -> String` | P3 |
| `emit_ffi_declare()` | `emit_ffi_declare(binding) -> String` | P4 |
| `emit_call_with_marshaling()` | `emit_ffi_call(name, args) -> String` | P4 |
| `generate_reactor()` | `emit_reactor(txns, cg) -> String` | P5 |
| `write_main()` | `emit_main() -> String` | P5 |
| `generate_init_state()` | `emit_init(program) -> String` | P5 |

## String Builder Pattern

Every function returns a `String` built via `new_builder()` + `append_str()`:

```briev
defn emit_txn(txn: Transaction) -> String {
    let sb = new_builder();
    sb = sb.append_str("define void @");
    sb = sb.append_str(txn.name);
    sb = sb.append_str("(%State* noalias nocapture %state) #0 {\n");
    sb = sb.append_str("entry:\n");
    // ... emit body ...
    sb = sb.append_str("}\n");
    term sb.to_string();
}
```

## Implementation Order

| Day | Deliverable |
|-----|-------------|
| 1 | Module header, `%State` type, `@global_state`, `init_state()`, field index |
| 2 | Transaction body: load/store/let/term/guarded/arith/comparison/bitwise/call |
| 3 | Definitions, constants, preconditions (!range, assume, select) |
| 4 | Match→switch, unification, FFI declare/call, trigger sampling, fusing |
| 5 | Reactor loop, `__wait_for_event()`, `main()`, wire into `main.bv`, litmus test |

## Litmus Test

```bash
# Step 1: Rust backend compiles Briev-in-Briev to native binary
briev-compiler llvm lib/compiler/main.bv --out /tmp/stage1/
llc /tmp/stage1/main.ll -o /tmp/stage1/main

# Step 2: Self-hosted compiler uses llvm.bv backend
# This step requires the Briev source bugs in lib/compiler/ to be fixed first
# ./tmp/stage1/main selfhost lib/compiler/main.bv --target llvm --out /tmp/stage2/
# llc /tmp/stage2/main.ll -o /tmp/stage2/main

# Step 3: Fixed-point verification
# ./tmp/stage2/main selfhost lib/compiler/main.bv --target llvm --out /tmp/stage3/
# diff /tmp/stage2/main.ll /tmp/stage3/main.ll  # Should match
```

## Acceptance Criteria

```bash
# All 17 existing fixtures still pass via Rust backend
briev-compiler llvm tests/fixtures/counter.bv | llc -o /dev/null
briev-compiler llvm tests/fixtures/phase1/guarded.bv | llc -o /dev/null
# ... all 17 ...

# llvm.bv emits valid LLVM IR for its own test fixture
briev-compiler selfhost tests/fixtures/llvm_selfhost_test.bv --target llvm --out /tmp/llvm_bv_test/
llc /tmp/llvm_bv_test/*.ll -o /dev/null

# lib/compiler/backends/mod.bv supports "llvm" hashtags
briev-compiler llvm tests/fixtures/hashtag_llvm.bv | llc -o /dev/null
```

## Risks

- **Briev source bugs in lib/compiler/** — the typechecker rejects 7/10 modules with contract errors. These are not backend bugs but block the full self-hosting litmus test.
- **StringBuilder capacity** — LLVM IR files can be large. The self-hosted StringBuilder must handle multi-megabyte strings.
- **The `c.bv` backend is the template.** Every pattern used there (item iteration, `uni` pattern matching, `append_str`) works identically in `llvm.bv`.