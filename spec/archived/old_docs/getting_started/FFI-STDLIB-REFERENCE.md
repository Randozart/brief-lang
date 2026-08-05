# Briv FFI Standard Library Reference v6.2

Complete reference for all standard library FFI bindings included with Briv v6.2.

**Location**: `std/bindings/`  
**Status**: Production Ready  
**Total Functions**: 59 across 4 modules

---

## Quick Index

- [I/O Module](#io-module) - File system operations (10 functions)
- [Math Module](#math-module) - Mathematical operations (14 functions)
- [String Module](#string-module) - String manipulation (15 functions)
- [Time Module](#time-module) - Timing operations (5 functions)

---

## I/O Module

**File**: `std/bindings/io.toml`  
**Description**: File system and stream operations  
**Error Type**: `IoError { code: Int, message: String }`

### read_file

Read entire file contents as string.

```briv
frgn read_file(path: String) -> Result<String, IoError> from "std::io";
```

**Parameters**:
- `path` (String): File path to read

**Success Returns**:
- `content` (String): Complete file contents

**Error Codes**:
- `1`: File not found
- `2`: Permission denied
- `3`: I/O error

**Example**:
```briv
defn load_config(path: String) -> String [true] [true] {
    let config: String = read_file(path);
    config;
};
```

### write_file

Write string contents to file (creates or truncates).

```briv
frgn write_file(path: String, content: String) -> Result<Void, IoError> from "std::io";
```

**Parameters**:
- `path` (String): File path to write to
- `content` (String): Contents to write

**Success Returns**: None (Void)

**Error Codes**:
- `1`: Permission denied
- `2`: Invalid path
- `3`: I/O error

### append_file

Append string contents to end of file.

```briv
frgn append_file(path: String, content: String) -> Result<Int, IoError> from "std::io";
```

**Parameters**:
- `path` (String): File path
- `content` (String): Contents to append

**Success Returns**:
- `bytes_written` (Int): Number of bytes written

**Error Codes**: Same as `write_file`

### file_exists

Check if file or directory exists.

```briv
frgn file_exists(path: String) -> Result<Bool, IoError> from "std::io";
```

**Parameters**:
- `path` (String): Path to check

**Success Returns**:
- `exists` (Bool): True if exists, false otherwise

**Error Codes**:
- `1`: Invalid path format
- `2`: Permission error (can't access parent directory)

### delete_file

Delete a file.

```briv
frgn delete_file(path: String) -> Result<Void, IoError> from "std::io";
```

**Parameters**:
- `path` (String): File to delete

**Success Returns**: None (Void)

**Error Codes**:
- `1`: File not found
- `2`: Permission denied
- `3`: Is a directory (not a file)

### create_dir

Create a single directory.

```briv
frgn create_dir(path: String) -> Result<Void, IoError> from "std::io";
```

**Parameters**:
- `path` (String): Directory path to create

**Success Returns**: None (Void)

**Error Codes**:
- `1`: Parent directory doesn't exist
- `2`: Permission denied
- `3`: Directory already exists

### create_dir_all

Create directory and all parent directories.

```briv
frgn create_dir_all(path: String) -> Result<Void, IoError> from "std::io";
```

**Parameters**:
- `path` (String): Directory path to create

**Success Returns**: None (Void)

**Error Codes**:
- `1`: Permission denied
- `2`: Invalid path

### delete_dir

Delete an empty directory.

```briv
frgn delete_dir(path: String) -> Result<Void, IoError> from "std::io";
```

**Parameters**:
- `path` (String): Directory to delete

**Success Returns**: None (Void)

**Error Codes**:
- `1`: Directory not found
- `2`: Directory not empty
- `3`: Permission denied

### delete_dir_all

Delete directory and all contents recursively.

```briv
frgn delete_dir_all(path: String) -> Result<Void, IoError> from "std::io";
```

**Parameters**:
- `path` (String): Directory to delete

**Success Returns**: None (Void)

**Error Codes**:
- `1`: Directory not found
- `2`: Permission denied
- `3`: I/O error during recursion

---

## Math Module

**File**: `std/bindings/math.toml`  
**Description**: Mathematical operations and functions  
**Error Type**: `MathError { code: Int, message: String }`

### sqrt

Compute square root of a float.

```briv
frgn sqrt(value: Float) -> Result<Float, MathError> from "std::math";
```

**Parameters**:
- `value` (Float): Number to get square root of

**Success Returns**:
- `result` (Float): Square root value

**Error Codes**:
- `1`: Negative number (NaN result)

### pow

Raise float to a power.

```briv
frgn pow(base: Float, exponent: Float) -> Result<Float, MathError> from "std::math";
```

**Parameters**:
- `base` (Float): Base value
- `exponent` (Float): Power to raise to

**Success Returns**:
- `result` (Float): Result of base^exponent

**Error Codes**:
- `1`: Overflow
- `2`: Invalid arguments

### abs_int

Compute absolute value of integer.

```briv
frgn abs_int(value: Int) -> Result<Int, MathError> from "std::math";
```

**Parameters**:
- `value` (Int): Integer value

**Success Returns**:
- `result` (Int): Absolute value

**Error Codes**:
- `1`: Overflow (only for minimum Int value)

### abs_float

Compute absolute value of float.

```briv
frgn abs_float(value: Float) -> Result<Float, MathError> from "std::math";
```

**Parameters**:
- `value` (Float): Float value

**Success Returns**:
- `result` (Float): Absolute value

### floor

Round float down to nearest integer.

```briv
frgn floor(value: Float) -> Result<Float, MathError> from "std::math";
```

**Parameters**:
- `value` (Float): Float value

**Success Returns**:
- `result` (Float): Floored value

### ceil

Round float up to nearest integer.

```briv
frgn ceil(value: Float) -> Result<Float, MathError> from "std::math";
```

**Parameters**:
- `value` (Float): Float value

**Success Returns**:
- `result` (Float): Ceiled value

### round

Round float to nearest integer.

```briv
frgn round(value: Float) -> Result<Float, MathError> from "std::math";
```

**Parameters**:
- `value` (Float): Float value

**Success Returns**:
- `result` (Float): Rounded value

### sin

Compute sine of angle in radians.

```briv
frgn sin(radians: Float) -> Result<Float, MathError> from "std::math";
```

**Parameters**:
- `radians` (Float): Angle in radians

**Success Returns**:
- `result` (Float): Sine value (-1.0 to 1.0)

### cos

Compute cosine of angle in radians.

```briv
frgn cos(radians: Float) -> Result<Float, MathError> from "std::math";
```

**Parameters**:
- `radians` (Float): Angle in radians

**Success Returns**:
- `result` (Float): Cosine value (-1.0 to 1.0)

### tan

Compute tangent of angle in radians.

```briv
frgn tan(radians: Float) -> Result<Float, MathError> from "std::math";
```

**Parameters**:
- `radians` (Float): Angle in radians

**Success Returns**:
- `result` (Float): Tangent value

**Error Codes**:
- `1`: Undefined (angle = π/2 + nπ)

### log

Compute natural logarithm.

```briv
frgn log(value: Float) -> Result<Float, MathError> from "std::math";
```

**Parameters**:
- `value` (Float): Positive number

**Success Returns**:
- `result` (Float): Natural logarithm

**Error Codes**:
- `1`: Non-positive value
- `2`: Infinity or NaN

### exp

Compute e raised to the power.

```briv
frgn exp(value: Float) -> Result<Float, MathError> from "std::math";
```

**Parameters**:
- `value` (Float): Exponent value

**Success Returns**:
- `result` (Float): e^value

**Error Codes**:
- `1`: Overflow

### min_int

Return minimum of two integers.

```briv
frgn min_int(a: Int, b: Int) -> Result<Int, MathError> from "std::math";
```

**Parameters**:
- `a` (Int): First value
- `b` (Int): Second value

**Success Returns**:
- `result` (Int): Minimum value

### max_int

Return maximum of two integers.

```briv
frgn max_int(a: Int, b: Int) -> Result<Int, MathError> from "std::math";
```

**Parameters**:
- `a` (Int): First value
- `b` (Int): Second value

**Success Returns**:
- `result` (Int): Maximum value

### min_float

Return minimum of two floats.

```briv
frgn min_float(a: Float, b: Float) -> Result<Float, MathError> from "std::math";
```

**Parameters**:
- `a` (Float): First value
- `b` (Float): Second value

**Success Returns**:
- `result` (Float): Minimum value

### max_float

Return maximum of two floats.

```briv
frgn max_float(a: Float, b: Float) -> Result<Float, MathError> from "std::math";
```

**Parameters**:
- `a` (Float): First value
- `b` (Float): Second value

**Success Returns**:
- `result` (Float): Maximum value

---

## String Module

**File**: `std/bindings/string.toml`  
**Description**: String manipulation and conversion functions  
**Error Types**: 
- `StringError { code: Int, message: String }` - For string operations
- `ParseError { code: Int, message: String }` - For parsing operations

### string_length

Get length of string in bytes.

```briv
frgn string_length(text: String) -> Result<Int, StringError> from "std::string";
```

**Parameters**:
- `text` (String): String to measure

**Success Returns**:
- `length` (Int): Length in bytes

### substring

Extract substring from start position for length.

```briv
frgn substring(text: String, start: Int, length: Int) -> Result<String, StringError> from "std::string";
```

**Parameters**:
- `text` (String): Source string
- `start` (Int): Starting position
- `length` (Int): Number of bytes

**Success Returns**:
- `result` (String): Extracted substring

**Error Codes**:
- `1`: Start position out of bounds
- `2`: Length exceeds available text

### string_replace

Replace all occurrences of pattern with replacement.

```briv
frgn string_replace(text: String, pattern: String, replacement: String) -> Result<String, StringError> from "std::string";
```

**Parameters**:
- `text` (String): Source string
- `pattern` (String): Pattern to find
- `replacement` (String): Replacement text

**Success Returns**:
- `result` (String): String with replacements

**Error Codes**:
- `1`: Invalid pattern

### string_contains

Check if string contains substring.

```briv
frgn string_contains(text: String, substring: String) -> Result<Bool, StringError> from "std::string";
```

**Parameters**:
- `text` (String): Source string
- `substring` (String): Substring to find

**Success Returns**:
- `found` (Bool): True if found, false otherwise

### string_starts_with

Check if string starts with prefix.

```briv
frgn string_starts_with(text: String, prefix: String) -> Result<Bool, StringError> from "std::string";
```

**Parameters**:
- `text` (String): Source string
- `prefix` (String): Prefix to check

**Success Returns**:
- `result` (Bool): True if starts with prefix

### string_ends_with

Check if string ends with suffix.

```briv
frgn string_ends_with(text: String, suffix: String) -> Result<Bool, StringError> from "std::string";
```

**Parameters**:
- `text` (String): Source string
- `suffix` (String): Suffix to check

**Success Returns**:
- `result` (Bool): True if ends with suffix

### string_to_upper

Convert string to uppercase.

```briv
frgn string_to_upper(text: String) -> Result<String, StringError> from "std::string";
```

**Parameters**:
- `text` (String): String to convert

**Success Returns**:
- `result` (String): Uppercase version

### string_to_lower

Convert string to lowercase.

```briv
frgn string_to_lower(text: String) -> Result<String, StringError> from "std::string";
```

**Parameters**:
- `text` (String): String to convert

**Success Returns**:
- `result` (String): Lowercase version

### string_trim

Remove leading and trailing whitespace.

```briv
frgn string_trim(text: String) -> Result<String, StringError> from "std::string";
```

**Parameters**:
- `text` (String): String to trim

**Success Returns**:
- `result` (String): Trimmed string

### string_split

Split string by delimiter (returns first part).

```briv
frgn string_split(text: String, delimiter: String) -> Result<String, StringError> from "std::string";
```

**Parameters**:
- `text` (String): String to split
- `delimiter` (String): Delimiter string

**Success Returns**:
- `part` (String): First part before delimiter

**Error Codes**:
- `1`: Delimiter not found

### parse_int

Parse string as signed integer.

```briv
frgn parse_int(text: String) -> Result<Int, ParseError> from "std::string";
```

**Parameters**:
- `text` (String): String to parse

**Success Returns**:
- `value` (Int): Parsed integer

**Error Codes**:
- `1`: Invalid format
- `2`: Out of range

### parse_float

Parse string as floating-point number.

```briv
frgn parse_float(text: String) -> Result<Float, ParseError> from "std::string";
```

**Parameters**:
- `text` (String): String to parse

**Success Returns**:
- `value` (Float): Parsed float

**Error Codes**:
- `1`: Invalid format
- `2`: Special values (Infinity, NaN)

### int_to_string

Convert integer to string.

```briv
frgn int_to_string(value: Int) -> Result<String, StringError> from "std::string";
```

**Parameters**:
- `value` (Int): Integer to convert

**Success Returns**:
- `result` (String): String representation

### float_to_string

Convert float to string.

```briv
frgn float_to_string(value: Float) -> Result<String, StringError> from "std::string";
```

**Parameters**:
- `value` (Float): Float to convert

**Success Returns**:
- `result` (String): String representation

### string_concat

Concatenate two strings.

```briv
frgn string_concat(left: String, right: String) -> Result<String, StringError> from "std::string";
```

**Parameters**:
- `left` (String): First string
- `right` (String): Second string

**Success Returns**:
- `result` (String): Concatenated result

---

## Time Module

**File**: `std/bindings/time.toml`  
**Description**: Timing operations and measurements  
**Error Type**: `TimeError { code: Int, message: String }`

### current_timestamp

Get current Unix timestamp in milliseconds.

```briv
frgn current_timestamp() -> Result<Int, TimeError> from "std::time";
```

**Success Returns**:
- `timestamp` (Int): Milliseconds since Unix epoch (Jan 1, 1970)

**Error Codes**:
- `1`: System time unavailable

**Example**:
```briv
defn log_with_timestamp(msg: String) -> String [true] [true] {
    let now: Int = current_timestamp();
    now;
};
```

### sleep_ms

Sleep for specified milliseconds.

```briv
frgn sleep_ms(milliseconds: Int) -> Result<Void, TimeError> from "std::time";
```

**Parameters**:
- `milliseconds` (Int): Duration to sleep (must be >= 0)

**Success Returns**: None (Void)

**Error Codes**:
- `1`: Negative duration
- `2`: Sleep interrupted

### sleep_seconds

Sleep for specified seconds.

```briv
frgn sleep_seconds(seconds: Int) -> Result<Void, TimeError> from "std::time";
```

**Parameters**:
- `seconds` (Int): Duration to sleep (must be >= 0)

**Success Returns**: None (Void)

**Error Codes**:
- `1`: Negative duration
- `2`: Sleep interrupted

### measure_time_ms

Start timing measurement (returns handle).

```briv
frgn measure_time_ms() -> Result<Int, TimeError> from "std::time";
```

**Success Returns**:
- `handle` (Int): Timer handle for later reference

**Error Codes**:
- `1`: Timer system unavailable

**Note**: This creates a handle that can be used with `elapsed_ms` to measure elapsed time.

### elapsed_ms

Get elapsed milliseconds since measurement started.

```briv
frgn elapsed_ms(handle: Int) -> Result<Int, TimeError> from "std::time";
```

**Parameters**:
- `handle` (Int): Timer handle from `measure_time_ms`

**Success Returns**:
- `elapsed` (Int): Milliseconds elapsed

**Error Codes**:
- `1`: Invalid handle
- `2`: Handle expired

---

## Complete Example

```briv
// Using multiple stdlib bindings together

frgn read_file(path: String) -> Result<String, IoError> from "std::io";
frgn string_length(text: String) -> Result<Int, StringError> from "std::string";
frgn int_to_string(value: Int) -> Result<String, StringError> from "std::string";
frgn current_timestamp() -> Result<Int, TimeError> from "std::time";

defn analyze_file(path: String) -> String [true] [true] {
    let content: String = read_file(path);
    let len: Int = string_length(content);
    let len_str: String = int_to_string(len);
    let now: Int = current_timestamp();
    now;
};
```

---

## Version Information

| Component | Version | Status |
|-----------|---------|--------|
| FFI System | 6.2 | Production |
| Stdlib Bindings | 6.2 | Production |
| I/O Module | 6.2 | Stable |
| Math Module | 6.2 | Stable |
| String Module | 6.2 | Stable |
| Time Module | 6.2 | Stable |

---

## Future Additions (Planned)

Modules planned for future releases:

- **Crypto Module** (v6.3) - Hashing, encryption
- **Database Module** (v6.3) - SQL operations
- **Network Module** (v6.4) - HTTP, TCP, UDP
- **JSON Module** (v6.4) - JSON parsing/serialization
- **Archive Module** (v6.5) - ZIP, TAR support

---

**Last Updated**: 2026-04-05  
**Maintained By**: Briv Core Team
