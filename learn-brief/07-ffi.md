# Foreign Function Interface (FFI)

FFI allows Brief to call functions in other languages (C, Rust, Python, etc.).

## 1. FFI Signatures

Declare foreign functions with `frgn`:

```brief
// C function: double sqrt(double x);
frgn sqrt(x: Float) -> Result<Float, MathError>;

// Rust function: fn log_message(msg: &str);
frgn! log_message(msg: String);

// Python function: def read_file(path: str) -> str;
frgn read_file(path: String) -> Result<String, IOError>;
```

**FFI Keywords:**
- `frgn` - Returns `Result<T, E>` (must handle error)
- `frgn!` - Returns `void` (fire-and-forget)
- `syscall` - Kernel call returning `Result<Int, E>`
- `syscall!` - Kernel call returning `void`

Functions without `from` resolve through `import "link/..."` targets:

```brief
import "link/brief_rt.c";

frgn __print_int(n: Int) -> Result<Bool, Error>;
```

## 2. Calling Foreign Functions

```brief
import "std/math";

txn calculate() [true][true] {
    let result = math.sqrt(16.0);
    
    // Handle Result type
    [result.is_ok()] {
        let value = result.value;
        println("Square root: " + String(value));
    };
    [result.is_err()] {
        println("Error: " + String(result.error));
    };
};
```

## 3. Error Handling

FFI calls return `Result<T, E>`. You must handle both Ok and Err paths:

```brief
frgn read_file(path: String) -> Result<String, IOError>;

txn load_config() [true][true] {
    let content = read_file("config.toml");
    
    [content.is_ok()] {
        &config_data = content.value;
    };
    [content.is_err()] {
        println("Failed to load config: " + String(content.error));
    };
};
```

Use `frgn!` for fire-and-forget calls where you don't need the return value:

```brief
frgn! log_message(msg: String);
```

## 4. Type Mapping

| Brief type | C type | Rust type | Python type |
|------------|--------|-----------|-------------|
| `Int` | `int64_t` | `i64` | `int` |
| `Float` | `double` | `f64` | `float` |
| `Bool` | `bool` | `bool` | `bool` |
| `String` | `char*` | `String` | `str` |
| `Char` | `int32_t` | `char` | `str` (len 1) |
| `Data` | `uint8_t*` | `Vec<u8>` | `bytes` |
| `Ptr<T>` | `T*` | `*const T` | `ctypes` |

## 5. Custom Link Dependencies

Link external C/Rust/Zig libraries via `import "link/"`:

```brief
// Link C math library
import "link/libm.so.6";

frgn sqrt(x: Float) -> Result<Float, MathError>;

txn calculate() [true][true] {
    let result = sqrt(16.0);
    // Result handling...
};
```

The compiler:
1. Searches for the linked file in `lib/` and project paths
2. Compiles C/Rust/Zig sources to LLVM bitcode via `compile_to_bitcode()`
3. Links with `llvm-link` and optimizes with `opt -O2`
4. Inlines foreign function calls when contracts prove safety

## 6. Metropolitan FFI (Zero-Copy)

For high-performance shared memory and inter-process communication:

```brief
import "std/metropolitan/shm";

// Open a shared memory segment
let segment = shm::open("data_buffer", 1024, [true]);
```

See `METROPOLITAN_FFI.md` for the complete reference.
