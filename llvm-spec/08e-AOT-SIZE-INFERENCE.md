# AOT Size Inference: List → Vector[N] Promotion

## 1. The Problem: Heap Allocation for Lists

In `02-TYPE-MAPPING.md`, dynamic lists are defined as:

```llvm
%struct.List_I64 = type { i64*, i64, i64 }
; pointer, length, capacity
```

This requires heap allocation (`malloc`/`free`), which:
1. **Forces omission of `nofree`** on transaction attributes (see `03-TRANSACTIONS.md`)
2. **Breaks hardware isomorphism** — FPGAs/ASICs have no heap
3. **Prevents SIMD vectorization** — the pointer indirection defeats aliasing analysis
4. **Causes allocation overhead** in the hot reactor loop

## 2. The Solution: Static Size Inference During Lowering

During the lowering pass (`01-ARCHITECTURE.md`), the compiler attempts to prove that any `List<T>` has a fixed, known size at compile time. If proven, the type is rewritten from `List` to `Vector[N]` — a fixed-size stack-allocated array.

### Three Inference Paths

#### Path A: Literal Propagation
```briv
let data = [1, 2, 3];
// Compiler sees: literal with 3 elements
// → Rewrites to: Vector[3]
```

```llvm
%data = alloca [3 x i64], align 16  ; Stack allocation — zero overhead
```

#### Path B: Contract-Bound Inference
```briv
txn process [len(data) <= 16] {
    // Compiler sees: precondition bounds length to max 16
    // → Rewrites to: Vector[16]
}
```

```llvm
; Safe because @llvm.assume guarantees len <= 16
%local = alloca [16 x i64], align 32
```

#### Path C: Symbolic Interval Analysis
```briv
let buffer: [Int] = [];
for i in 0..array_size {
    buffer = append(buffer, data[i]);
};
// Compiler sees: loop runs exactly array_size iterations
// → Rewrites to: Vector[array_size]
```

```llvm
%buffer = alloca [array_size x i64], align 32
```

### Type Rewriting Table

| Before (List) | After (Vector[N]) | LLVM Type |
|---------------|-------------------|-----------|
| `[Int]` (size N) | `Vector[N]<Int>` | `[N x i64]` |
| `[Float]` (size N) | `Vector[N]<Float>` | `[N x float]` |
| `[Bool]` (size N) | `Vector[N]<Bool>` | `[N x i8]` |

## 3. Conditional `nofree` Attribute

The `nofree` attribute on `#0` (`03-TRANSACTIONS.md`) promises LLVM that the function never deallocates memory. If a transaction touches any heap-allocated `List` (where inference failed), `nofree` is a false promise — it causes LLVM to keep stale deallocated pointers in registers.

**Rule:** The backend's call graph analysis scans every transaction for heap operations:
- `malloc`/`calloc`/`realloc`/`free` calls (either direct or via list-append functions)
- `List` reservation that changes capacity

If ANY heap op exists in the transaction's call graph:
```llvm
; NO nofree — this transaction does heap operations
define void @process(%State* noalias nocapture %state) local_unnamed_addr #0a {
```

If ZERO heap ops exist (all Lists were promoted, or no Lists used):
```llvm
; With nofree — maximum optimization
define void @increment(%State* noalias nocapture %state) local_unnamed_addr #0 {
```

Where `#0a` is a separate attribute block identical to `#0` minus `nofree`.

## 4. Codegen Split: Heap vs. Stack

When inference succeeds → **Stack path**:
```llvm
; Stack-allocated Vector[16]
%local_arr = alloca [16 x i64], align 32
; Access via GEP
%elem_ptr = getelementptr inbounds [16 x i64], [16 x i64]* %local_arr, i64 0, i64 %idx
; No free needed — stack memory is reclaimed on ret
```

When inference fails → **Heap path**:
```llvm
; Heap-allocated List
%malloc_size = mul i64 8, %capacity       ; 8 bytes per i64
%ptr = call i8* @malloc(i64 %malloc_size)
%typed_ptr = bitcast i8* %ptr to i64*
store i64* %typed_ptr, i64** %list_data_ptr
store i64 0, i64* %list_len_ptr           ; length = 0
store i64 %capacity, i64* %list_cap_ptr   ; capacity = N
; ... must eventually call free ...
```

## 5. Integration with the FFI

When a foreign function receives a promoted `Vector[N]`:
```briv
frgn process_data(data: [Int]) -> Void from "lib.so"
```

If the compiler inferred size N, it passes a pointer to the stack-allocated array plus the length:
```llvm
; Passing promoted Vector[N] to FFI
%data_ptr = getelementptr inbounds [N x i64], [N x i64]* %arr, i64 0, i64 0
call void @process_data(i64* %data_ptr, i64 N)
```

No heap allocation, no marshaling, no leak.

## 6. When Inference Falls Back

If none of the three paths can prove a bound:
1. The `List` type remains un-promoted
2. Heap allocation is used (the reactor's bump allocator)
3. `nofree` is omitted from the transaction's attribute block
4. The transaction is marked as incompatible with hardware targets (error at compile time if targeting FPGA/ASIC)