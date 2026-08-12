# FFI: Foreign Function Interface → LLVM declare/call

## Overview

When the LLVM backend encounters a `ForeignBinding` or an `Expr::Call` to a foreign function, it emits:

1. An LLVM `declare` statement in the module header
2. A `call` instruction at the call site with C ABI argument marshaling

## Declaration

```briev
frgn strlen(s: String) -> Int from "libc.so.6";
```

```llvm
; Module-level declaration
declare i64 @strlen(i8*) #1
```

**Return type mapping:**
- `frgn` returning `Result<T, E>` → LLVM type is `T` (the `Ok` type). The error path is handled by the runtime.
- `frgn!` (fire-and-forget) → LLVM type is `void`

## Call Site

```briev
let len = strlen("hello");
```

```llvm
; String → i8* (auto-null-terminated)
%s = call i8* @briev_string_to_cstr(%string_val)
%len = call i64 @strlen(i8* %s)
```

## ABI Marshaling Table

| Briev Type | C ABI Type | LLVM Conversion |
|------------|------------|-----------------|
| `Int` | `int64_t` | Pass direct |
| `Float` | `float` | Pass direct |
| `Bool` | `int32_t` | `zext i1 %val to i32` |
| `Char` | `uint32_t` | `zext i32 %val to i32` |
| `String` | `const char*` | Emit `@briev_string_to_cstr` call (see memory lifecycle below) |
| `Void` | `void` | No return value |

## C-String Memory Lifecycle

The `@briev_string_to_cstr` intrinsic must **not** leak. Use stack allocation for small strings:

```llvm
; Strategy 1: Stack allocation (preferred — no leak, no malloc)
%len = extractvalue { i8*, i64 } %string_val, 1
%cstr = alloca i8, i64 %len
%void = call void @llvm.memcpy.p0i8.p0i8.i64(i8* %cstr, i8* %ptr, i64 %len, i1 false)
%nul = getelementptr i8, i8* %cstr, i64 %len
store i8 0, i8* %nul
call i64 @strlen(i8* %cstr)  ; safe — stack allocation, no free needed
```

If using heap allocation instead, the backend MUST emit a corresponding `free` call after the foreign function returns. The reactor loop is infinite, so every leaked allocation accumulates.

## Tier 2: Metropolitan Protocol

For complex types (`List`, `Enum`, `Struct`, `Tuple`), the backend emits a different pattern:

```briev
frgn process_json(input: JsonValue) -> JsonValue from "libprocessor" via metropolitan;
```

```llvm
; Instead of a direct call, emit shared memory protocol:
%channel = call i32 @metro_open(i8* getelementptr inbounds ([...], @metro_process_json, ...))
%layout = call i32 @metro_pack_layout(%struct.JsonValue* %input)
call void @metro_pack(i32 %channel, i32 %layout, %struct.JsonValue* byval(%struct.JsonValue) %input)
call void @metro_signal(i32 %channel, i32 1)  ; REQ
; ... (spin on status word)
call void @metro_unpack(i32 %channel, i32 %layout, %struct.JsonValue* %output)
```

The metropolitan intrinsics (`@metro_open`, `@metro_pack`, `@metro_signal`, `@metro_unpack`) are declared in a companion module and linked at runtime.

## Bootstrap Functions

The 4 bootstrap functions (`__read_file`, `__write_file`, `__print`, `__exit`) are recognized by name and get special IR emission:

```llvm
; __print(msg: String)
%cstr = call i8* @briev_string_to_cstr(%msg)
%len = call i64 @strlen(i8* %cstr)
call i64 @write(i32 1, i8* %cstr, i64 %len)  ; fd=1 (stdout)
```

All other `frgn` declarations follow the standard `declare`/`call` pattern.

## Attribute for Foreign Calls

```llvm
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
;                                  ^-- foreign functions can't access our %State
```

This is critical: the `memory(inaccessiblemem: readwrite)` tells LLVM the foreign function does NOT alias `%State`, preserving our `noalias` guarantees across FFI boundaries.