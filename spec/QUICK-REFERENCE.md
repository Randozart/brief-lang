# Brief Quick Reference

## Syntax at a Glance

### Basic Declarations

```brief
// State declaration
state <name>: <type> = <expr>?

// Transaction
rct txn <name>(<params>) [pre][post] {
    // body
}

// Function definition
defn <name>(<params>) -> <outputs> [pre][post] {
    // body
}

// Signature (FFI)
sig <name>: <type> -> <result_type> from <path>
```

### FFI Keywords

| Keyword | Returns | Use |
|---------|---------|-----|
| `frgn` | `Result<T, E>` | Import foreign function, handle errors |
| `frgn!` | `void` | Fire-and-forget FFI call |
| `syscall` | `Result<Int, E>` | Kernel call with return value |
| `syscall!` | `void` | Kernel call without return |

### Address Operators

| Operator | Meaning |
|----------|----------|
| `@addr` | Target-dependent address |
| `@raw:0xADDR` | Raw physical address (embedded) |
| `@stack:OFFSET` | Stack-relative |
| `@heap:OFFSET` | Heap-relative |

### Control Flow

```brief
// Guards (branching)
[guard_expr] {
    // executes when guard is true
}

// Pattern matching
unbinding <name>(<pattern>) = <expr>
```

### Result Type Methods

```brief
result.is_ok()     // Bool
result.is_err()    // Bool  
result.value       // Unwrapped value
result.error.code  // Error code
result.error.message  // Error message
```

## Types Quick Reference

| Type | Description |
|------|-------------|
| `Int` | Signed 64-bit int |
| `Float` | 32-bit float |
| `UInt` | Unsigned 64-bit int |
| `Bool` | Boolean (1 bit) |
| `String` | UTF-8 string |
| `Data` | Opaque binary data |
| `Void` | Unit/empty type |
| `Vector[T]` | Fixed-size vector |
| `Option[T]` | Nullable type |
| `Sig[T]` | Signature reference |
| `Result[T, E]` | FFI return type |

## Common Patterns

### Error Handling

```brief
let result = read_file(path);
[result.is_ok()] {
    term result.value;
} [result.is_err()] {
    term "default";
};
```

### Fire-and-Forget FFI

```brief
frgn! send_message(msg: String);
```

### Importing

```brief
import std.io;
import std.math as math;
import {File, Dir} from "std.fs" from "fs.toml";
```

### Multi-Output

```brief
defn get_pair() -> (Int, String) [true] {
    term (42, "answer");
};
```

## See Also

* [Full Specification](SPEC.md)
* [Language Tutorial](LANGUAGE-TUTORIAL.md)
* [FFI Guide](FFI-GUIDE.md)
* [Examples](examples/)

*Quick reference - last updated v0.10.0 (2026-04-20)*