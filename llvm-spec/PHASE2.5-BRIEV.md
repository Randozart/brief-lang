# Phase 2.5 Briev: Transition Fusing + Trigger Sampling

**Date:** 2026-05-29  
**Spec Reference:** `08a-TRIGGERS.md`, `08b-TRANSITION-FUSING.md`  
**Prerequisite:** Phase 2 complete (contract analysis for precondition/postcondition extraction)  
**Estimated Effort:** 3 days  

## Goal

Fuse guaranteed-sequential transactions into single-tick atomic transitions. Sample volatile triggers once per tick entry to enforce deterministic execution across the tick.

## Deliverables

### 1. Trigger Sampling Phase

At the entry of `reactor_tick()`, emit `load volatile` for every `TriggerDeclaration` in the program. Store results in SSA registers. All downstream precondition checks use the sampled value, never the raw pointer.

**Briev source:**
```briev
trg button: Bool @ 0x40001000;

rstruct Counter {
    count: Int;

    txn Counter.inc [button && count < 100] {
        &count = count + 1;
        term;
    };
}
```

**Expected LLVM:**
```llvm
define void @reactor_tick() local_unnamed_addr #0 {
entry:
  %trg_button_raw = load volatile i8, i8* inttoptr (i64 1073745920 to i8*), align 1
  %trg_button = icmp ne i8 %trg_button_raw, 0
  ; ... rest of tick (uses %trg_button, not re-loads)
}
```

### 2. Reactor Dispatch with Precondition Evaluation

The reactor loop evaluates all transaction preconditions in priority order and dispatches to the first-true transaction body.

```llvm
define void @reactor_tick() local_unnamed_addr #0 {
entry:
  ; sample triggers
  ; load state
  ; eval precond txn_1
  br i1 %t1_ready, label %t1_body, label %check_t2
check_t2:
  br i1 %t2_ready, label %t2_body, label %noop
t1_body:
  call void @txn_1(%State* @global_state)
  br label %commit
t2_body:
  call void @txn_2(%State* @global_state)
  br label %commit
noop:
  br label %commit
commit:
  ret void
}
```

### 3. Consume `detect_fusable_pairs`

`src/backend/mod.rs:291` already returns `Vec<(String, String)>` of fusable pairs. The LLVM backend reads this list and:

- **Applies inhibition rules**: reject fusion if:
  - `Txn_B`'s precondition references a `trg` identifier
  - `Txn_A` and `Txn_B` write to the same field (WAW hazard)
  - Either transaction is `is_async == true`
  - Combined body exceeds complexity budget (configurable)
- **Generates fused bodies**: concatenated statement lists with merged pre/post conditions
- **Emits fused transaction functions**: `define void @txn_A_B_fused(...)`

### 4. Fusing Inhibition Rules

**Trg dependency:** Scan `Txn_B`'s precondition identifiers against the program's `TriggerDeclaration` name list. If any match, refuse fusion.

**WAW hazard:** Compare `collect_assigned_identifiers(txn_A)` against `collect_assigned_identifiers(txn_B)`. If any overlapping field is written by both, refuse.

**Async flag:** Check `txn.is_async` on either. If true, refuse.

**Complexity budget:** Count statements in combined body. If > `MAX_FUSED_STMTS` (default 100), refuse.

## New Test Fixtures

| Fixture | Tests |
|---------|-------|
| `triggers_mmio.bv` | One MMIO trg, one txn depending on it |
| `triggers_poll.bv` | One polled trg, two txns |
| `fuse_simple.bv` | Two txns where post(A) → pre(B), no conflicts |
| `fuse_inhibited.bv` | Two txns with WAW hazard — fusion refused |

## Acceptance Criteria

```bash
for f in tests/fixtures/phase2_5/*.bv; do
  briev-compiler llvm "$f" --out /tmp/p25/
  llc /tmp/p25/$(basename "$f" .bv).ll -o /dev/null  # Must succeed
done
grep "load volatile" /tmp/p25/triggers_mmio.ll   # Trigger sampling present
grep "br i1" /tmp/p25/fuse_simple.ll              # Precondition dispatch branches
grep "fused" /tmp/p25/fuse_simple.ll               # Fused transaction name
```

## Implementation Checklist

- [ ] Collect `TriggerDeclaration` from `program.items`, build name→struct map
- [ ] Emit `load volatile` for each trigger in `reactor_tick` prologue
- [ ] Map trigger addresses: `LinkRef::Explicit(addr)` → `inttoptr`, `LinkRef::Linked(_)` → global symbol
- [ ] Call `detect_fusable_pairs` in `generate()` 
- [ ] Apply inhibition rules (trg dependency, WAW, async, complexity)
- [ ] Generate fused transaction bodies (concatenate statements)
- [ ] Emit precondition evaluation chain in `reactor_tick()` 
- [ ] Phase 0-2 regression fixtures still pass