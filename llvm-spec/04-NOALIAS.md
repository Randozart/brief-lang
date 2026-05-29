# noalias Synthesis: The Biggest Single Optimization

## Why This Works

Brief prohibits arbitrary pointers. There is no `&` operator, no pointer arithmetic, no reference escaping. Every `rstruct` field access is a named GEP on the global `%State*`. Therefore:

> **No two pointers in any transaction can ever alias.**

This is a guarantee that C and Rust compilers cannot make without `restrict`/`noalias` annotations. LLVM cannot prove it on its own — we must tell it.

## The Optimization Chain

### Step 1: `noalias` + `nocapture` on every state pointer

```llvm
define void @txn_increment(%struct.State* noalias nocapture %state) local_unnamed_addr #0 {
```

- **`noalias`**: LLVM assumes nothing written through any other pointer can affect memory reachable from `%state`. This enables load-to-store forwarding, dead store elimination, and instruction reordering.
- **`nocapture`**: The pointer is not stored or returned. LLVM can prove `%state` is not escaped, enabling allocas to be promoted to SSA registers.

### Step 2: Register Promotion

Because `%state` is `noalias` + `nocapture`, LLVM's `mem2reg` pass promotes ALL struct field loads to SSA registers for the duration of the transaction:

```llvm
; Without noalias: every field access is load-from-memory
%a = load i64, i64* %field_a_ptr
store i64 %new_a, i64* %field_a_ptr
%b = load i64, i64* %field_b_ptr  ; must re-load — a might alias b!

; With noalias: fields are SSA registers
%a = load i64, i64* %field_a_ptr  ; loads once at entry
%new_a = add i64 %a, 1
%b = load i64, i64* %field_b_ptr  ; independent load (can be hoisted)
store i64 %new_a, i64* %field_a_ptr  ; single store at commit
```

### Step 3: Dead Store Elimination

With register promotion, intermediate stores inside the transaction body become dead:

```llvm
; Brief: &count = count + 1; &count = count * 2;
; Without noalias: two stores
store i64 %tmp, i64* %count_ptr
store i64 %result, i64* %count_ptr  ; first store is dead

; With noalias (and mem2reg): only the final commit store survives
```

### Step 4: `local_unnamed_addr`

```llvm
define void @txn_increment(...) local_unnamed_addr #0 {
```

This tells LLVM the function's address is never taken. Combined with `norecurse`, the inliner aggressively merges all transaction bodies into the reactor loop.

## Implementation

In `src/backend/llvm.rs`, the `generate_txn_function` method must:

1. Emit `define void @name(%struct.State* noalias nocapture %state) local_unnamed_addr #0 {`
2. Load all fields used by the transaction at entry (promote to SSA)
3. Execute the lowered body using SSA values
4. Store modified fields back at the commit point (single store per field)
5. Close with `ret void`

For acyclic call graphs, also emit `mustprogress nofree norecurse nosync nounwind willreturn memory(argmem: readwrite)` attributes.