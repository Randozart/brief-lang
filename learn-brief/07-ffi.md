# Foreign Function Interface (FFI)

FFI allows Brief to call functions in other languages (C, Rust, Python, etc.).

## 1. FFI Signatures

Declare foreign functions with `frgn sig`:

```brief
// C function: double sqrt(double x);
frgn sig sqrt(x: Float) -> Result<Float, MathError> from "math.toml";

// Rust function: fn log_message(msg: &str);
frgn! sig log_message(msg: String) -> void from "io.toml";

// Python function: def read_file(path: str) -> str;
frgn sig read_file(path: String) -> Result<String, IOError> from "io.toml";
```

**FFI Keywords:**
- `frgn` - Returns `Result<T, E>` (must handle error)
- `frgn!` - Returns `void` (fire-and-forget)
- `syscall` - Kernel call returning `Result<Int, E>`
- `syscall!` - Kernel call returning `void`

## 2. Calling Foreign Functions

```brief
import std.math;

txn calculate() [true][true] {
    let result = math.sqrt(16.0);
    
    // Handle Result type
    [result.is_ok()] {
        let value = result.value;
        println("Square root: " + String(value));
    };
    [result.is_err()] {
        let error = result.error;
        println("Error: " + error.message);
    };
    
    term;
};
```

## 3. Error Handling

Always handle FFI errors:

```brief
// ❌ BAD - ignores error
let result = read_file("data.txt");  // Result type ignored!

// ✅ GOOD - handles error
let result = read_file("data.txt");
[result.is_ok()] {
    let content = result.value;
    println(content);
};
[result.is_err()] {
    println("Failed to read file");
};
```

## 4. Metropolitan FFI (Zero-Copy)

For high-performance FFI, use Metropolitan FFI:

```brief
import std.metropolitan_ffi;

// Create shared memory channel
let channel = create_metropolitan_channel("ml_inference", "python")?;

// Send data (zero-copy)
metropolitan_send(channel, image_data)?;

// Receive response
let result = metropolitan_receive(channel, 100)?;  // 100ms timeout

term;
```

**Benefits:**
- ✅ Zero-copy (data shared in-place)
- ✅ No marshalling overhead
- ✅ Built-in synchronization
- ✅ 10-100x faster than traditional FFI

See [METROPOLITAN_FFI.md](../METROPOLITAN_FFI.md) for complete guide.

## 5. Type Mapping

| Brief Type | C Type | Rust Type | Python Type |
|------------|--------|-----------|-------------|
| `Int` | `int64_t` | `i64` | `int` |
| `UInt` | `uint64_t` | `u64` | `int` |
| `Float` | `float` | `f32` | `float` |
| `Float64` | `double` | `f64` | `float` |
| `Bool` | `bool` | `bool` | `bool` |
| `Char` | `char32_t` | `char` | `str` (len=1) |
| `String` | `const char*` | `&str` | `str` |
| `Data` | `uint8_t*` | `&[u8]` | `bytes` |

## 6. Complete Example: Database Access

```brief
// database.bv

// Foreign function signatures
frgn sig db_connect(host: String, port: Int) -> Result<DbHandle, DbError> from "db.toml";
frgn sig db_query(handle: DbHandle, sql: String) -> Result<ResultSet, DbError> from "db.toml";
frgn sig db_close(handle: DbHandle) -> Result<Void, DbError> from "db.toml";

struct DbHandle {
    id: Int
};

struct DbError {
    code: Int,
    message: String
};

txn connect_to_database() [true][true] {
    let result = db_connect("localhost", 5432);
    
    [result.is_ok()] {
        let handle = result.value;
        
        let query_result = db_query(handle, "SELECT * FROM users");
        [query_result.is_ok()] {
            let rows = query_result.value;
            process_rows(rows);
        };
        
        db_close(handle);
    };
    
    [result.is_err()] {
        println("Connection failed: " + result.error.message);
    };
    
    term;
};
```

## 7. Complete Example: HTTP Client

```brief
// http_client.bv

frgn sig http_get(url: String) -> Result<String, HttpError> from "http.toml";
frgn sig http_post(url: String, body: String) -> Result<String, HttpError> from "http.toml";

defn fetch_json(url: String) -> Result<String, String> {
    let result = http_get(url);
    
    [result.is_ok()] {
        term Ok(result.value);
    };
    
    [result.is_err()] {
        term Err(result.error.message);
    };
    
    term Err("unreachable");
};

txn main() [true][true] {
    let url = "https://api.example.com/data";
    let response = fetch_json(url);
    
    [response.is_ok()] {
        println("Response: " + response.unwrap());
    };
    
    [response.is_err()] {
        println("Error: " + response.unwrap_err());
    };
    
    term;
};
```

## Exercises

1. Create FFI bindings for a C math library
2. Implement a Python plugin system using Metropolitan FFI
3. Build a database wrapper with error handling
4. Create an HTTP client with timeout support

---

*Next: [08-examples.md](08-examples.md) - Complete examples*
