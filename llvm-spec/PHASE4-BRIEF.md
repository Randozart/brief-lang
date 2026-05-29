# Phase 4 Brief: FFI declare + call with C ABI Marshaling

**Date:** 2026-05-29  
**Spec Reference:** `07-FFI-TO-DECLARE.md`  
**Prerequisite:** Phase 3 complete (match → switch)  
**Estimated Effort:** 1-2 days  

## Goal

`frgn strlen(s: String) -> Int from "libc.so.6"` generates `declare i64 @strlen(i8*) #1` in the module header, and call sites emit `call i64 @strlen(i8* %marshaled_arg)` with C ABI type conversion.

## Deliverables

### 1. `declare` Emission in Module Header

Every `TopLevel::ForeignBinding` emits one `declare` statement. The return type is `void` for fire-and-forget (`frgn!`) or the first output field type. Input types map per the ABI table.

```llvm
declare i64 @strlen(i8*) #1
declare i64 @write(i32, i8*, i64) #1
declare void @exit(i32) #1
```

### 2. Bootstrap Intrinsic Recognition

Four functions recognized by name, emitting optimized IR:

| Name | LLVM IR |
|------|---------|
| `__print` | String → `alloca` + `memcpy` + null-terminate → `write(1, cstr, len)` |
| `__exit` | `exit(0)` |
| `__read_file` | `open(path, O_RDONLY)` → `read(fd, buf, count)` |
| `__write_file` | `open(path, O_WRONLY|O_CREAT)` → `write(fd, buf, count)` |

### 3. ABI Type Marshaling

| Brief Type | C ABI Type | LLVM Conversion |
|------------|------------|-----------------|
| `Int` | `int64_t` | i64 pass-through |
| `Bool` | `int32_t` | `zext i8 %val to i32` |
| `Char` | `uint32_t` | `zext i32 %val to i32` |
| `String` | `const char*` | `alloca` + `memcpy` + null-terminate → `i8*` |

### 4. String Marshaling (no leak)

```llvm
%len = extractvalue { i8*, i64 } %str_val, 1
%ptr = extractvalue { i8*, i64 } %str_val, 0
%cstr = alloca i8, i64 %len
%dest = getelementptr i8, i8* %cstr, i64 0
%src = getelementptr i8, i8* %ptr, i64 0
call void @llvm.memcpy.p0i8.p0i8.i64(i8* %dest, i8* %src, i64 %len, i1 false)
%nul = getelementptr i8, i8* %cstr, i64 %len
store i8 0, i8* %nul
```

## Data Structure

```rust
frgn_map: HashMap<String, ForeignSignature>,
// Populated during generate() from TopLevel::ForeignBinding items
// Consumed by generate_expr() Expr::Call handler
```

## Test Fixtures

| Fixture | Tests |
|---------|-------|
| `ffi_print.bv` | `frgn __print(s: String)` → bootstrap `write` call |
| `ffi_declare.bv` | `frgn strlen(s: String) -> Int` → declare + call |

## Acceptance Criteria

```bash
for f in tests/fixtures/phase4/*.bv; do
  brief-compiler llvm "$f" --out /tmp/p4/
  llc /tmp/p4/$(basename "$f" .bv).ll -o /dev/null
done
grep "declare.*@strlen" /tmp/p4/ffi_declare.ll    # declare present
grep "declare.*@write" /tmp/p4/ffi_print.ll        # write declare for __print
grep "call.*@strlen" /tmp/p4/ffi_declare.ll        # call site
grep "alloca" /tmp/p4/ffi_print.ll                  # string marshaling
```

## Regression

All existing 16 fixtures must still pass `llc`. Unit tests: 270/270 passing.