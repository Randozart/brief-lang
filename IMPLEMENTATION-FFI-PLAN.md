# Brief Compiler FFI Implementation Plan

## Overview

This document outlines the implementation plan for restructuring the Brief compiler's foreign function interface (FFI) system to:
1. Replace `#` comments with `//` style comments
2. Convert as many `frgn sig` declarations as possible to native `defn` implementations
3. Create proper wrapper pattern with `__raw` prefix for FFI functions
4. Unify all FFI bindings under `lib/ffi/` structure
5. Remove hardcoded FFI registrations from the compiler
6. Enable users to create their own external FFI libraries

## Implementation Phases

---

### Phase 1: Replace `#` → `//` Comments

**Objective:** Convert all hash-style comments to double-slash style.

**Files to update:**
- `lib/std/` (7 files: math.bv, string.bv, time.bv, collections.bv, encoding.bv, json.bv, io.bv)
- `std/core.bv`
- `examples/` directory

**Pattern:** Replace `# comment` with `// comment`

---

### Phase 2: Convert `frgn sig` → Native `defn` + `__raw` Wrappers

**Objective:** Make standard library functions native where possible, with proper FFI wrapper pattern.

**Pattern:**
```brief
// Raw FFI - handles Result<T, Error> manually
frgn sig __println(msg: String) -> Result<Bool, IoError> from "lib/ffi/bindings/io.toml";

// Wrapper - returns Bool directly, handles error internally
defn println(msg: String) -> Bool [true] [true] {
  let result = __println(msg);
  term true;
};
```

**By file:**

| File | Convert to `defn` | Keep as `__raw` FFI | Notes |
|------|-------------------|---------------------|--------|
| math.bv | abs, min, max, clamp, div, mod, is_even, is_odd, is_positive, is_negative, is_zero, negate, double, triple, half, abs_diff, sum_range, factorial, fibonacci, gcd_native, lcm_native | sin, cos, tan, asin, acos, atan, atan2, sinh, cosh, tanh, sqrt, cbrt, pow, powi, exp, exp2, ln, log, log2, log10, floor, ceil, round, trunc, fract, is_nan, is_infinite, is_finite, is_normal, signum_float, clamp_float, min_float, max_float, div_rem, gcd, lcm, signum, random, random_int | ~20 → ~20 |
| string.bv | len, concat, append, prepend, trim, trim_left, trim_right, trim_start, trim_end, contains, starts_with, ends_with, find, char_at, substr, substr_range, replace, split, lines, join, pad_left, pad_right, pad_both, repeat, to_string, to_float, to_int | to_lower, to_upper, to_title, is_lower, is_upper, is_whitespace, is_alpha, is_alphanumeric, is_numeric, contains_at, find_from, rfind, get_chars, utf8_len, escape, unescape, quote, unquote, bytes, from_bytes, replace_all, splitn, rsplit, words, truncate, truncate_with, eq_ignore_case, cmp, parse_float, to_bool | ~25 → ~30 |
| time.bv | duration_seconds, duration_millis, duration_minutes, duration_hours, duration_days, add_seconds, add_minutes, add_hours, add_days, diff_seconds, diff_days | now, now_millis, now_micros, year, month, day, hour, minute, second, weekday, yearday, timestamp, timestamp_full, format_timestamp, format_date, format_time, format_datetime, parse_timestamp, parse_datetime, seconds_per_minute, seconds_per_hour, seconds_per_day, millis_per_second | ~15 → ~20 |
| collections.bv | len, append, prepend, concat, get, set, remove, slice, contains, find, take, drop, is_empty | reverse, maybe filter/map/reduce (need function types) | ~13 → ~3 |
| encoding.bv | hex_digit_to_int, int_to_hex_digit, byte_to_hex, url_encode_simple, url_decode_simple, is_hex_digit, is_hex_string, reverse_string, count_char | base64_encode, base64_decode, base64_url_encode, base64_url_decode, hex_encode, hex_decode, hex_encode_bytes, hex_decode_bytes, url_encode, url_decode, url_encode_component, url_decode_component, html_escape, html_unescape, utf8_encode, utf8_decode, codepoint_to_char, char_to_codepoint, md5, sha1, sha256, sha512, uuid_v4, is_uuid, uri_parse, uri_build, uri_get_param | ~9 → ~26 |
| json.bv | (none - Data type is foreign) | all functions (parse, stringify, is_null, get_*, etc.) | 0 → ~30 |
| io.bv | (none - needs stdio) | print, println, input | 0 → ~3 |

---

### Phase 3: Unify FFI Structure Under `lib/ffi/`

**Objective:** Create a unified FFI infrastructure that works the same for stdlib and user libraries.

**New directory structure:**
```
lib/
├── std/                     // Native Brief implementations
│   └── *.bv
├── ffi/                     // NEW: FFI infrastructure
│   ├── __init__.bv          // Main import file
│   ├── bindings/
│   │   ├── io.toml
│   │   ├── math.toml
│   │   ├── time.toml
│   │   ├── string.toml
│   │   ├── encoding.toml
│   │   └── json.toml
│   └── native/
│       ├── Cargo.toml
│       └── src/
│           └── lib.rs       // Rust implementations
```

**Migration:**
1. Move `std/bindings/*.toml` → `lib/ffi/bindings/`
2. Create `lib/ffi/native/` with Cargo.toml
3. Create `lib/ffi/__init__.bv` that exposes the FFI functions

---

### Phase 4: Refactor Compiler

**Objective:** Remove hardcoded FFI from interpreter, use dynamic loading.

**Changes:**

1. **Remove hardcoded registrations** (`src/interpreter.rs`)
   - Delete `register_std_io()`, `register_std_math()`, `register_std_string()`
   - Remove calls to these functions in `Interpreter::new()`

2. **Dynamic FFI loading**
   - At startup, scan `lib/ffi/bindings/*.toml`
   - Register implementations from `lib/ffi/native/`

3. **Import resolver** (`src/import_resolver.rs`)
   - When importing `ffi`, load all bindings
   - Handle TOML path resolution for user libraries

---

### Phase 5: User FFI Library Pattern

**Objective:** Enable users to create their own FFI libraries with the same pattern.

**Structure:**
```
my_project/
├── lib/
│   └── my_ffi/
│       ├── my_functions.bv  // FFI declarations and wrappers
│       ├── bindings.toml    // TOML binding specs
│       └── native/          // Rust implementations
│           ├── Cargo.toml
│           └── src/lib.rs
└── src/
    └── main.bv
```

**Usage:**
```brief
import my_ffi;

defn main() -> Int {
  let result = my_func();
  term result;
};
```

---

## Naming Conventions

| Raw FFI | Wrapper | Notes |
|---------|---------|-------|
| `__println` | `println` | Double underscore prefix |
| `__sin` | `sin` | Standard math names |
| `__now` | `now` | Standard time names |

The double underscore is:
- Minimal and non-distracting
- Stands out when writing code
- Feels like a "secret" prefix for internal use

---

## Error Handling Pattern

Users can handle FFI errors in multiple ways:

1. **Using wrapper** (returns basic type):
   ```brief
   defn println(msg: String) -> Bool { ... };  // Returns true
   ```

2. **Using raw FFI** (handles Result manually):
   ```brief
   frgn sig __println(msg: String) -> Result<Bool, IoError>;
   
   defn safe_print(msg: String) -> Bool {
     let result = __println(msg);
     [result is error] { term false };
     term true;
   };
   ```

3. **Using union types** (future):
   ```brief
   defn try_print(msg: String) -> Bool|String {
     let result = __println(msg);
     [result is error] { term result.error.message };
     term true;
   };
   ```

---

## Implementation Order

1. Phase 1: Comments (`#` → `//`)
2. Phase 2: Convert stdlib to native + wrappers
3. Phase 3: Create `lib/ffi/` structure
4. Phase 4: Refactor compiler (remove hardcoded, add dynamic loading)
5. Phase 5: Document user library pattern

---

## Notes

- The FFI system validates signatures against TOML at compile time
- Runtime uses dynamic loading from TOML + Rust implementations
- User libraries follow the same pattern as stdlib FFI
- This creates a consistent, unified experience across all FFI usage