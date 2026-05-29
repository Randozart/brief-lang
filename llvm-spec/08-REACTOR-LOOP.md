# Reactor Loop: main() → Tick → Dispatch → Commit

## Structure

The generated `.ll` file always contains a `main()` function with an infinite tick loop:

```llvm
define i32 @main() local_unnamed_addr #0 {
entry:
    ; Initialize state to default values
    call void @init_state(%struct.State* @global_state)

    ; Infinite reactor loop
    br label %tick

tick:
    call void @reactor_tick()
    br label %tick
}

define void @reactor_tick() local_unnamed_addr #0 {
    ; Evaluate all transaction preconditions
    ; Execute the first transaction whose preconditions are met
    ; Commit state changes
    ret void
}
```

## Acyclic Optimization: Inlined Dispatch

When the call graph is acyclic, the reactor loop inlines ALL transaction bodies:

```llvm
define void @reactor_tick() norecurse #0 {
    ; Load state once
    %state = load %struct.State, %struct.State* @global_state

    ; Evaluate all preconditions — each is a select/compare
    %t1_cond = call i1 @txn_increment_precond(%struct.State %state)
    %t2_cond = call i1 @txn_decrement_precond(%struct.State %state)

    ; Dispatch: first-true wins (priority order)
    br i1 %t1_cond, label %t1_body, label %check_t2

t1_body:
    %new_state_t1 = call %struct.State @txn_increment_body(%struct.State %state)
    br label %commit

check_t2:
    br i1 %t2_cond, label %t2_body, label %noop

t2_body:
    %new_state_t2 = call %struct.State @txn_decrement_body(%struct.State %state)
    br label %commit

noop:
    br label %commit

commit:
    %final_state = phi %struct.State [%new_state_t1, %t1_body], [%new_state_t2, %t2_body], [%state, %noop]
    store %struct.State %final_state, %struct.State* @global_state
    ret void
}
```

**Key optimization**: By passing `%struct.State` by value (not pointer), LLVM sees every field as an SSA value. With `noalias` having proven no pointers alias, the `phi` at the commit point is the only stateful operation — everything between load and store is pure SSA computation.

## Acyclic Optimization: Full Inlining

With `norecurse` + `willreturn` + `local_unnamed_addr`, LLVM's inliner will inline `txn_increment_body`, `txn_increment_precond`, etc. into `reactor_tick`. The final result:

```llvm
define void @reactor_tick() norecurse #0 {
    ; Everything is SSA — no function calls at all
    %count = extractvalue %struct.State %state, 0
    %t1_cond = icmp slt i64 %count, 100
    br i1 %t1_cond, label %incr, label %check_t2

incr:
    %new_count = add nsw i64 %count, 1
    %new_state = insertvalue %struct.State %state, i64 %new_count, 0
    br label %commit
    ; ... etc
}
```

## Cyclic Graphs: Dynamic Dispatch

If cycles exist, the reactor loop uses a dispatch table:

```llvm
define void @reactor_tick() #0 {
    %state_ptr = alloca %struct.State
    store %struct.State @global_state, %struct.State* %state_ptr

    ; Precondition table (sorted by priority)
    %t1_ready = call i1 @txn_1_precond(%struct.State* %state_ptr)
    %t2_ready = call i1 @txn_2_precond(%struct.State* %state_ptr)

    ; First ready transaction wins
    br i1 %t1_ready, label %exec_t1, label %check_t2
check_t2:
    br i1 %t2_ready, label %exec_t2, label %done

exec_t1:
    call void @txn_1_body(%struct.State* %state_ptr)
    br label %done
exec_t2:
    call void @txn_2_body(%struct.State* %state_ptr)
    br label %done

done:
    %new_state = load %struct.State, %struct.State* %state_ptr
    store %struct.State %new_state, %struct.State* @global_state
    ret void
}
```

Here, functions are called by pointer and cannot be inlined. The `noalias` guarantee still applies, so `mem2reg` promotes fields within each transaction but not across the dispatch.