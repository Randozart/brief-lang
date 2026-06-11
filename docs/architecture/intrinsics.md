# Intrinsics: `name#()` Syntax

**Date added:** 2026-06-11
**Status:** Implementation complete (14 system/data intrinsics added 2026-06-11, 29 total)

## Motivation

Operators like `+`, `-`, `*` are language built-ins that map directly to
compiler-known operations (LLVM `add`, `sub`, `mul`). Functions like `sqrt`,
`pop`, and `size` are semantically identical — they're compiler-known operations
that happen to be named rather than symbolized. The `#` postfix makes this
relationship explicit and visible to the programmer.

## Syntax

```brief
// Prefix-style (primary)
let dist: Float = sqrt#(dsq);
let count: Int = size#(list);
let first: Int = pop#(list);

// Method-style (optional, for ergonomic consistency)
let bytes: Int = string.bytes#();
```

The `#` postfix indicates the name is a compiler-known intrinsic, not a
user-defined function or a `frgn` import.

## Design Principles

1. **No separate declaration needed** — intrinsics are built into the compiler,
   not declared in `frgn` bindings
2. **Per-target dispatch** — the same intrinsic name resolves to the best
   implementation per backend (LLVM intrinsic, Rust native, or FPGA circuit)
3. **Type dispatch** — `sqrt#(Float)` → 32-bit sqrt, `sqrt#(Float64)` → 64-bit sqrt
4. **Replaces `as intrinsic`** — no more `frgn sqrt_f32(x: Float) -> Float as intrinsic "llvm.sqrt.f32"`

## Intrinsic Table (29 total)

### Math & Bitwise (10)

| Intrinsic | Args | Returns | Description | LLVM mapping |
|---|---|---|---|---|
| `sqrt#(x)` | Float/Float64 | same as input | Square root | `llvm.sqrt.f32` / `llvm.sqrt.f64` |
| `fabs#(x)` | Float/Float64 | same as input | Absolute value | `llvm.fabs.f32` / `llvm.fabs.f64` |
| `ceil#(x)` | Float/Float64 | same as input | Ceiling | `llvm.ceil.f32` / `llvm.ceil.f64` |
| `floor#(x)` | Float/Float64 | same as input | Floor | `llvm.floor.f32` / `llvm.floor.f64` |
| `ctpop#(x)` | Int | Int | Population count | `llvm.ctpop.i64` |
| `cttz#(x)` | Int | Int | Count trailing zeros | `llvm.cttz.i64(i64, i1)` |
| `ctlz#(x)` | Int | Int | Count leading zeros | `llvm.ctlz.i64(i64, i1)` |
| `abs#(x)` | Int | Int | Integer absolute value | `llvm.abs.i64(i64, i1)` |
| `bitreverse#(x)` | Int | Int | Bit-reverse | `llvm.bitreverse.i64` |

### Collection (5)

| Intrinsic | Args | Returns | Description | Backend dispatch |
|---|---|---|---|---|
| `pop#(c)` | List/HashMap/Stack/Queue | element type | Remove and return last | ArrowDiscard dispatch |
| `size#(c)` | List/HashMap/HashSet | Int | Element count | List.len / HashMap.size |
| `contains#(m, k)` | HashMap/HashSet | Bool | Key membership | HashMap.contains |
| `keys#(m)` | HashMap | List | All keys | HashMap.keys |
| `values#(m)` | HashMap | List | All values | HashMap.values |

### Meta (1)

| Intrinsic | Args | Returns | Description | LLVM mapping |
|---|---|---|---|---|
| `bytes#(v)` | Float/Int/String | Int | Byte width | Constant per type |

### System I/O (11) [Added 2026-06-11]

| Intrinsic | Args | Returns | Description | LLVM mapping |
|---|---|---|---|---|
| `println#(val)` | Any | Bool | Print value with newline | `printf` with per-type format |
| `readln#()` | — | String | Read line from stdin | `fgets(stdin)` / `syscall read(0)` |
| `exit#(code)` | Int | ! | Terminate program | `syscall exit(60)` — `noreturn` |
| `time#()` | — | Int | Nanosecond timestamp | `clock_gettime` / `rdtsc` |
| `read_file#(path)` | String | String | Read file to string | `open` + `mmap` / `read` |
| `write_file#(path, data)` | String, String | Bool | Write string to file | `open` + `write` |
| `sleep#(ms)` | Int | Bool | Sleep for ms | `nanosleep` |
| `socket#(d, t, p)` | Int, Int, Int | Int | Create socket | `syscall socket(41)` |
| `bind#(fd, port)` | Int, Int | Bool | Bind socket to port | `syscall bind(49)` |
| `listen#(fd, backlog)` | Int, Int | Bool | Listen on socket | `syscall listen(50)` |
| `accept#(fd)` | Int | Int | Accept connection | `syscall accept(43)` |

### Data (3) [Added 2026-06-11]

| Intrinsic | Args | Returns | Description | Backend dispatch |
|---|---|---|---|---|
| `sort#(list)` | List\<T\> | List\<T\> | Sort list | `qsort` / SIMD sorting network |
| `reverse#(list)` | List\<T\> | List\<T\> | Reverse list | Tight in-place swap loop |
| `range#(end)` | Int | List\<Int\> | `[0..end)` | Constant → `.rodata`; runtime → loop |

## Interpreter dispatch

Each intrinsic has a native Rust implementation:

```rust
fn intrinsic_sqrt(args: &[Value]) -> Result<Value, RuntimeError> {
    let x = args[0].as_float()?;
    Ok(Value::Float(x.sqrt()))
}
```

No TOML-path resolution, no dynamic dispatch. The intrinsic is resolved at
compile time by the parser/typechecker.

## LLVM backend dispatch

```rust
fn emit_intrinsic(&mut self, out: &mut String,
                  intrinsic: &Intrinsic, args: &[Expr],
                  indent: &str) -> TypedRegister {
    match intrinsic {
        Intrinsic::Sqrt => {
            let arg = self.emit_expr(out, &args[0], indent);
            let fl = self.ensure_float_reg(out, indent, &arg);
            match arg.ty {
                Type::Float => {
                    let r = format!("%is{}", self.txn_counter);
                    // Just use llvm.sqrt.f32 — opt passes handle the rest
                    writeln!(out, "{}{} = call float @llvm.sqrt.f32(float {})",
                             indent, r, fl).ok();
                    TypedRegister { name: r, ty: Type::Float }
                }
                _ => unimplemented!(),
            }
        }
        Intrinsic::Pop => {
            let arg = self.emit_expr(out, &args[0], indent);
            // Dispatch on Value type (not function name)
            match arg.ty {
                Type::List(_) => self.emit_arrow_discard(out, indent, &arg),
                Type::HashMap(_, _) => self.emit_hashmap_pop(out, indent, &arg),
                _ => unimplemented!(),
            }
        }
        // ...
    }
}
```

## Parser

```rust
// In parse_call_suffix() or parse_postfix():
if let Some(Ok(Token::Hash)) = self.current_token() {
    self.advance();
    let name = self.expect_identifier()?;
    let intrinsic = Intrinsic::from_name(&name)?;
    self.expect(Token::LParen)?;
    let args = self.parse_call_args()?;
    self.expect(Token::RParen)?;
    return Expr::IntrinsicCall { intrinsic, args };
}
```

## AST

```rust
pub enum Intrinsic {
    Sqrt, Fabs, Ceil, Floor,
    Pop, Size, Bytes, Contains, Keys, Values,
    Ctpop, Cttz, Ctlz, Abs, Bitreverse,
    // Future: all Arrow operations (vectorized ops, etc.)
}

pub enum Expr {
    // ... existing variants ...
    IntrinsicCall {
        intrinsic: Intrinsic,
        args: Vec<Expr>,
    },
}
```

## Interpreter dispatch update (Pass A — 2026-06-11)

The interpreter implements real system calls for:
- `println#` — `println!("{}", v)` stdout
- `readln#` — `std::io::stdin().read_line`
- `exit#` — `std::process::exit(code)`
- `time#` — `SystemTime::now().duration_since(UNIX_EPOCH)`
- `read_file#` — `std::fs::read_to_string(path)`
- `write_file#` — `std::fs::write(path, data)`
- `sleep#` — `std::thread::sleep(Duration::from_millis(ms))`
- `socket#`, `bind#`, `listen#`, `accept#` — return failure stubs in interpreter
- `sort#`, `reverse#` — passthrough (no-op in interpreter)
- `range#(end)` — `Value::List((0..end).map(Value::Int).collect())`

## Migration from `as intrinsic` (completed 2026-06-11)

| Step | What | Status |
|---|---|---|
| 1 | Add `Expr::IntrinsicCall` + `Intrinsic` enum to AST | ✅ |
| 2 | Add parser support for `name#(...)` | ✅ |
| 3 | Add `emit_intrinsic` to LLVM backend, interpreter | ✅ |
| 4 | Remove `ForeignSignature.intrinsic_name` field | ✅ |
| 5 | Remove `as intrinsic` parser code | ✅ |
| 6 | Update `lib/std/llvm.bv` users to use `name#()` | ✅ |
| 7 | Delete `lib/std/llvm.bv` | ✅ |
| 8 | Delete `lib/std/math.bv` | ✅ |
| 9 | Update `nbody_sqrt.bv` to use `sqrt#()` | ✅ |

## Method-style sugar

`list.pop#()` is syntactically equivalent to `pop#(list)`. The parser desugars
the method form into the prefix form at parse time:

```rust
// In parse_method_call:
if let Some(Ok(Token::Hash)) = self.peek() {
    self.advance();
    let method_name = /* already parsed from receiver */;
    let args = self.parse_call_args()?;
    let mut all_args = vec![receiver_expr];
    all_args.extend(args);
    return Expr::IntrinsicCall {
        intrinsic: Intrinsic::from_name(&method_name)?,
        args: all_args,
    };
}
```
