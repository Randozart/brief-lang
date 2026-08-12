# Phase 5 Briev: Reactor Loop + Acyclic Dispatch + Equilibrium Suspension

**Date:** 2026-05-29  
**Spec Reference:** `08-REACTOR-LOOP.md`, `08c-EQUILIBRIUM-SUSPENSION.md`  
**Prerequisite:** Phase 4 complete (FFI declare/call)  
**Estimated Effort:** 3 days  

## Goal

`reactor_tick()` becomes a real dispatch engine. Acyclic call graphs execute by-value with `extractvalue`/`insertvalue`/`phi`. Cyclic call graphs use pointer-based dispatch. When no transaction precondition is met, the system suspends via `__wait_for_event()` instead of busy-spinning.

## Deliverables

### 1. Acyclic Dispatch (by-value, inlined)

When `call_graph.has_cycle() == false`, the tick:

1. Loads `%State` from `@global_state` via `load %struct.State`
2. Evaluates all transaction preconditions in declaration order (priority-first)
3. Branches to the first-true transaction body
4. Body computes the new state as an SSA value using `extractvalue`/`insertvalue`
5. `phi` at commit point selects old state (noop) vs new state
6. `store` the final state back to `@global_state`

```llvm
define void @reactor_tick() local_unnamed_addr #0 {
entry:
  ; Trigger sampling (Phase 2.5)
  ; Load state by value
  %state = load %State, %State* @global_state
  ; Evaluate txn_1 precondition
  %c1 = call i1 @pre_condition_txn_1(...)
  br i1 %c1, label %t1_body, label %check_t2
t1_body:
  %new_state = call %State @txn_1_body(%State %state)
  br label %commit
check_t2:
  br i1 %c2, label %t2_body, label %noop
t2_body:
  %new_state2 = call %State @txn_2_body(%State %state)
  br label %commit
noop:
  call void @__wait_for_event()
  br label %commit
commit:
  %final = phi %State [%new_state, %t1_body], [%new_state2, %t2_body], [%state, %noop]
  store %State %final, %State* @global_state
  ret void
}
```

### 2. Cyclic Dispatch (pointer-based)

When `call_graph.has_cycle() == true`, transactions use `%State*` pointer:

```llvm
define void @reactor_tick() local_unnamed_addr #0 {
entry:
  ; Trigger sampling
  ; Evaluate preconditions via %State* calls
  %c1 = call i1 @pre_condition_txn_1(%State* @global_state)
  br i1 %c1, label %t1_body, label %check_t2
t1_body:
  call void @txn_1(%State* @global_state)
  br label %commit
...
}
```

### 3. Equilibrium Suspension: `__wait_for_event()`

When no precondition evaluates true, emit:

```llvm
noop:
  call void @__wait_for_event()
  br label %commit
```

`__wait_for_event()` is a bootstrap intrinsic (resolved per-target):
- Linux: `epoll_wait` (0% CPU)
- ARM: `wfi` (microwatt sleep)
- WASM: Asyncify yield

### 4. Precondition as Standalone `i1` Functions

Each transaction's precondition is extracted into a define-private function that returns `i1`:

```llvm
define internal i1 @pre_inc(%State* %state) {
  %count = load i64, i64* getelementptr inbounds (%State, %State* %state, i32 0, i32 0)
  %cmp = icmp slt i64 %count, 100
  ret i1 %cmp
}
```

This keeps the tick dispatcher clean and enables per-precondition `!range` metadata.

## Test Fixtures

| Fixture | Tests |
|---------|-------|
| `acyclic_dispatch.bv` | Two txns with disjoint fields, acyclic graph |
| `cyclic_dispatch.bv` | Two txns with mutual call dependency |
| `equilibrium.bv` | One txn with `[count > 100]` — will sleep on tick 0 |
| `precondition_tick.bv` | One txn with `[x < 10]` precondition evaluated in tick |

## Acceptance Criteria

```bash
for f in tests/fixtures/phase5/*.bv; do
  briev-compiler llvm "$f" --out /tmp/p5/
  llc /tmp/p5/$(basename "$f" .bv).ll -o /dev/null  # Must succeed
done
grep "phi" /tmp/p5/acyclic_dispatch.ll           # Phi for by-value state merge
grep "extractvalue" /tmp/p5/acyclic_dispatch.ll   # By-value state extraction
grep "__wait_for_event" /tmp/p5/equilibrium.ll    # Equilibrium suspension
grep "define internal i1 @pre_" /tmp/p5/*.ll      # Precondition functions
```

## Implementation Checklist

- [ ] Extract precondition evaluation into `define internal i1 @pre_txn()` functions
- [ ] Generate precondition calls in reactor_tick dispatch chain
- [ ] Acyclic: load state by-value, emit extractvalue/insertvalue/phi
- [ ] Cyclic: pointer-based dispatch, calls directly
- [ ] Equilibrium path: `__wait_for_event()` when no precondition true
- [ ] Commit phi: select new state (from body) or old state (from noop)
- [ ] Both paths fuse with existing trigger sampling phase
- [ ] Regression: all existing 17 fixtures still pass