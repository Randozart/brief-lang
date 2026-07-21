// ── Intrinsic Execution ────────────────────────────────────────────────
// 2026-07-14: Generic operations. For polymorphic ops (Add#, Eq#, etc.)
// the implementation checks argument types to dispatch int vs float.
// Flat dispatch: one match arm per intrinsic name, first match wins.

use crate::errors::RuntimeError;
use crate::interpreter::{bool_to_bits, f64_to_bits, i64_to_bits, zero_bits, Value, VirtualHeap};

// 2026-07-15: SysCall# abstract op → syscall number mapping (x86_64)
fn resolve_syscall_number_interp(name: &str) -> Option<i64> {
    Some(match name {
        "Read" => 0, "Write" => 1, "Open" => 2, "Close" => 3,
        "Stat" => 4, "FStat" => 5, "LSeek" => 8, "Mmap" => 9,
        "Munmap" => 11, "Brk" => 12, "RtSigAction" => 13,
        "RtSigProcmask" => 14, "IoCtl" => 16, "Pipe" => 22,
        "SchedYield" => 24, "NanoSleep" => 35,
        "GetPid" => 39, "GetPPid" => 40, "Socket" => 41,
        "Connect" => 42, "Accept" => 43, "Send" => 44,
        "Recv" => 45, "SendTo" => 44, "RecvFrom" => 45,
        "Bind" => 49, "Listen" => 50, "Exit" => 60,
        "Fcntl" => 72, "FTruncate" => 77, "GetCwd" => 79,
        "ChDir" => 80, "MkDir" => 83, "RmDir" => 84,
        "Unlink" => 87, "Dup" => 32, "Dup2" => 33,
        "FSync" => 74, "MkDt" => 85, "ReadLink" => 89,
        "ChMod" => 90, "ChOwn" => 92, "Clone" => 56,
        "GetEgid" => 108, "GetEuid" => 107, "GetGid" => 104,
        "GetPgid" => 109, "GetSid" => 124,
        "GetSockOpt" => 55, "GetUid" => 102, "Mlock" => 149,
        "Mprotect" => 10, "SetSockOpt" => 54,
        "ShmGet" => 29, "Shutdown" => 48, "UMask" => 95,
        "ShmAt" => 30, "ShmDt" => 31, "SemGet" => 64,
        "SemOp" => 65, "SemCtl" => 66, "ClockGetTime" => 228,
        "ClockSetTime" => 229, "Futex" => 202,
        "GetRandom" => 318, "Openat" => 257,
        "Membarrier" => 324, "CopyFileRange" => 326,
        "PRead" => 17, "PWrite" => 18,
        _ => return None,
    })
}

/// Execute a named intrinsic with the given evaluated arguments.
pub fn execute_intrinsic(
    name: &str,
    args: &[Value],
    heap: &mut VirtualHeap,
) -> Result<Value, RuntimeError> {
    match name {
        // ── Arithmetic (type-polymorphic) ───────────────────────────
        "Add#" => exec_binop(args, |a, b| a + b, |a, b| a + b),
        "Sub#" => exec_binop(args, |a, b| a - b, |a, b| a - b),
        "Mul#" => exec_binop(args, |a, b| a * b, |a, b| a * b),
        "Div#" => exec_div(args),
        "Rem#" => exec_rem(args),
        "Neg#" => {
            let a = arg_as_i64(args, 0)?;
            Ok(i64_to_bits(a.wrapping_neg()))
        }
        "Abs#" => {
            let a = arg_as_i64(args, 0)?;
            Ok(i64_to_bits(a.wrapping_abs()))
        }

        // ── Comparison (type-polymorphic) ───────────────────────────
        "Eq#"  => exec_cmp(args, |a, b| a == b, |a, b| (a - b).abs() < 1e-10),
        "Neq#" => exec_cmp(args, |a, b| a != b, |a, b| (a - b).abs() >= 1e-10),
        "Lt#"  => exec_cmp(args, |a, b| a < b,  |a, b| a < b),
        "Gt#"  => exec_cmp(args, |a, b| a > b,  |a, b| a > b),
        "Le#"  => exec_cmp(args, |a, b| a <= b, |a, b| a <= b),
        "Ge#"  => exec_cmp(args, |a, b| a >= b, |a, b| a >= b),

        // ── Bitwise (int-only — no float path) ────────────────────────
        "BitAnd#" => { let a = arg_as_i64(args, 0)?; let b = arg_as_i64(args, 1)?; Ok(i64_to_bits(a & b)) }
        "BitOr#" => { let a = arg_as_i64(args, 0)?; let b = arg_as_i64(args, 1)?; Ok(i64_to_bits(a | b)) }
        "BitXor#" => { let a = arg_as_i64(args, 0)?; let b = arg_as_i64(args, 1)?; Ok(i64_to_bits(a ^ b)) }
        "Shl#" => {
            let a = arg_as_i64(args, 0)?;
            let b = arg_as_i64(args, 1)?;
            Ok(i64_to_bits(a.wrapping_shl(b as u32)))
        }
        "Shr#" => {
            let a = arg_as_i64(args, 0)?;
            let b = arg_as_i64(args, 1)?;
            Ok(i64_to_bits(a.wrapping_shr(b as u32)))
        }
        "BitNot#" => {
            let a = arg_as_i64(args, 0)?;
            Ok(i64_to_bits(!a))
        }
        // ── Logical ──────────────────────────────────────────────────
        "Not#" => {
            let a = arg_as_i64(args, 0)?;
            Ok(i64_to_bits(if a == 0 { 1 } else { 0 }))
        }
        // ── Pointer operations (interpreter: evaluate inner) ──────────
        "Deref#" => {
            let ptr_val = args.get(0).cloned().unwrap_or(Value::Int(0));
            // In the interpreter, Deref# just returns the value (identity).
            Ok(ptr_val)
        }
        "Cast#" => {
            // Cast# is identity in the interpreter — type reinterpretation only.
            Ok(args.get(0).cloned().unwrap_or(Value::Int(0)))
        }
        "Ptr#" => {
            // Ptr# is identity — inttoptr doesn't change bits.
            Ok(args.get(0).cloned().unwrap_or(Value::Int(0)))
        }
        // ── Float math ──────────────────────────────────────────────
        "Sqrt#"  => { let x = arg_as_f64(args, 0)?; Ok(f64_to_bits(x.sqrt())) }
        "Sin#"   => { let x = arg_as_f64(args, 0)?; Ok(f64_to_bits(x.sin())) }
        "Cos#"   => { let x = arg_as_f64(args, 0)?; Ok(f64_to_bits(x.cos())) }
        "Fabs#"  => { let x = arg_as_f64(args, 0)?; Ok(f64_to_bits(x.abs())) }
        "Ceil#"  => { let x = arg_as_f64(args, 0)?; Ok(f64_to_bits(x.ceil())) }
        "Floor#" => { let x = arg_as_f64(args, 0)?; Ok(f64_to_bits(x.floor())) }
        "Pow#"   => { let a = arg_as_f64(args, 0)?; let b = arg_as_f64(args, 1)?; Ok(f64_to_bits(a.powf(b))) }

        // ── Memory (observable) ─────────────────────────────────────
        "Malloc#" => {
            let size = arg_as_i64(args, 0)?;
            let addr = heap.allocate(size as usize);
            Ok(i64_to_bits(addr as i64))
        }
        "Alloc#" => {
            let size = arg_as_i64(args, 0)?;
            let addr = heap.allocate(size as usize);
            Ok(i64_to_bits(addr as i64))
        }
        "Free#" => {
            let ptr = arg_as_i64(args, 0)?;
            heap.free(ptr as u64)
                .map_err(|_| RuntimeError::HeapError("free failed".into()))?;
            Ok(Value::Void)
        }
         "Load#" => {
            let addr = arg_as_i64(args, 0)? as u64;
            let bytes = args.get(1).and_then(|a| if let Value::Int(n) = a { Some(*n as usize) } else { None }).unwrap_or(8);
            let data = heap.read(addr, bytes)
                .ok_or_else(|| RuntimeError::HeapError("Load# read failed".into()))?;
            let mut buf = data.to_vec();
            buf.resize(8, 0);
            let val = i64::from_le_bytes([buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7]]);
            Ok(i64_to_bits(val))
        }
        "Store#" => {
            let addr = arg_as_i64(args, 0)? as u64;
            let val = arg_as_i64(args, 1)?;
            let bytes = args.get(2).and_then(|a| if let Value::Int(n) = a { Some(*n as usize) } else { None }).unwrap_or(8);
            let data = val.to_le_bytes()[..bytes].to_vec();
            heap.write(addr, &data)
                .map_err(|_| RuntimeError::HeapError("Store# write failed".into()))?;
            Ok(Value::Void)
        }
        "Copy#" => {
            let dst = arg_as_i64(args, 0)? as u64;
            let src = arg_as_i64(args, 1)? as u64;
            let n = arg_as_i64(args, 2)? as usize;
            let data = heap.read(src, n)
                .ok_or_else(|| RuntimeError::HeapError("Copy# read failed".into()))?;
            let data_vec = data.to_vec();
            heap.write(dst, &data_vec)
                .map_err(|_| RuntimeError::HeapError("Copy# write failed".into()))?;
            Ok(Value::Void)
        }
        "Fill#" => {
            let ptr = arg_as_i64(args, 0)? as u64;
            let val = arg_as_i64(args, 1)?;
            let n = arg_as_i64(args, 2)? as usize;
            let data = vec![val as u8; n];
            heap.write(ptr, &data)
                .map_err(|_| RuntimeError::HeapError("Fill# failed".into()))?;
            Ok(Value::Void)
        }

        // ── String / Conversion ─────────────────────────────────────
        "Concat#"   => Err(RuntimeError::UnsupportedIntrinsic("Concat#".to_string())),
        "Length#"   => Err(RuntimeError::UnsupportedIntrinsic("Length#".to_string())),
        "ToInt#"    => { let f = arg_as_f64(args, 0)?; Ok(i64_to_bits(f as i64)) }
        "ToFloat#"  => { let n = arg_as_i64(args, 0)?; Ok(f64_to_bits(n as f64)) }
        "ToString#" => {
            let s = if let Ok(n) = arg_as_i64(args, 0) {
                n.to_string()
            } else if let Ok(f) = arg_as_f64(args, 0) {
                f.to_string()
            } else {
                arg_as_string(args, 0)?
            };
            let bytes = s.into_bytes();
            let len = bytes.len() as i64;
            let mut header = len.to_le_bytes().to_vec();
            header.extend_from_slice(&bytes);
            let addr = heap.allocate(header.len());
            if heap.write(addr as u64, &header).is_ok() {
                Ok(i64_to_bits(addr as i64))
            } else {
                Ok(i64_to_bits(0))
            }
        },

        // ── Collection ──────────────────────────────────────────────
        "Get#"    => Err(RuntimeError::UnsupportedIntrinsic("Get#".to_string())),
        "Insert#" => Err(RuntimeError::UnsupportedIntrinsic("Insert#".to_string())),

        // ── GPU ─────────────────────────────────────────────────────
        "GetGlobalId#"   => Err(RuntimeError::UnsupportedIntrinsic("GetGlobalId#".to_string())),
        "GetGlobalSize#" => Err(RuntimeError::UnsupportedIntrinsic("GetGlobalSize#".to_string())),
        "GetLocalId#"    => Err(RuntimeError::UnsupportedIntrinsic("GetLocalId#".to_string())),
        "AddressOf#" => {
            let id = arg_as_string(args, 0)?;
            let addr = resolve_address_for_interp(&id);
            Ok(i64_to_bits(addr as i64))
        }

        // ── POSIX sysconf (observable) ────────────────────────────────
        "SysConf#" => {
            if args.is_empty() { return Ok(i64_to_bits(0)); }
            let name: i64 = match &args[0] {
                Value::Int(n) => *n,
                Value::Bits(b) => {
                    let mut arr = [0u8; 8];
                    let copy_len = b.len().min(8);
                    arr[..copy_len].copy_from_slice(&b[..copy_len]);
                    i64::from_le_bytes(arr)
                }
                other => {
                    let s = format!("{:?}", other);
                    match s.as_str() {
                        "PageSize" => 30,
                        "CpuCount" => 83,
                        "HostNameMax" => 180,
                        "OpenMax" => 4,
                        _ => {
                            eprintln!("SysConf#: unknown abstract name '{}', using 0", s);
                            0
                        }
                    }
                }
            };
            let result = unsafe { libc::sysconf(name as i32) };
            Ok(i64_to_bits(result as i64))
        }

        // ── OS Syscall (observable) ───────────────────────────────────
        // 2026-07-15: SysCall# executes a raw OS syscall via libc::syscall().
        // In check mode, returns 0 for successful execution or -1 for error.
        // The first arg is the syscall number (Int) or PascalCase abstract op
        // name (resolved to a number here).
        "SysCall#" => {
            if args.is_empty() { return Ok(zero_bits(0)); }
            let mut sysno: i64 = 0;
            match &args[0] {
                Value::Int(n) => { sysno = *n; }
                Value::Bits(b) => {
                    // Convert Vec<u8> to i64 (little-endian)
                    let mut arr = [0u8; 8];
                    let copy_len = b.len().min(8);
                    arr[..copy_len].copy_from_slice(&b[..copy_len]);
                    sysno = i64::from_le_bytes(arr);
                }
                other => {
                    let name = format!("{:?}", other);
                    sysno = resolve_syscall_number_interp(&name).unwrap_or(0);
                }
            }
            // Extract up to 6 Int arguments, default 0
            let arg_val = |i: usize| -> i64 {
                args.get(i).map(|v| match v {
                    Value::Int(n) => *n,
                    Value::Bits(b) => {
                        let mut arr = [0u8; 8];
                        let copy_len = b.len().min(8);
                        arr[..copy_len].copy_from_slice(&b[..copy_len]);
                        i64::from_le_bytes(arr)
                    }
                    _ => 0
                }).unwrap_or(0)
            };
            let a1 = arg_val(1); let a2 = arg_val(2); let a3 = arg_val(3);
            let a4 = arg_val(4); let a5 = arg_val(5); let a6 = arg_val(6);
            let result = unsafe { libc::syscall(sysno, a1, a2, a3, a4, a5, a6) };
            Ok(i64_to_bits(result as i64))
        }

        // ── Atomic operations (non-atomic in check mode) ──────────────
        // 2026-07-15: Single-threaded check mode — just do regular heap
        // load/store. Full LLVM atomicrmw is emitted at runtime.
        "AtomicLoad#" => {
            let ptr = arg_as_i64(args, 0)? as u64;
            let data = heap.read(ptr, 8).map(|b| i64::from_le_bytes([b[0],b[1],b[2],b[3],b[4],b[5],b[6],b[7]])).unwrap_or(0);
            Ok(i64_to_bits(data))
        }
        "AtomicStore#" => {
            let ptr = arg_as_i64(args, 0)? as u64;
            let val = arg_as_i64(args, 1)?;
            let bytes = val.to_le_bytes();
            heap.write(ptr, &bytes).ok();
            Ok(Value::Void)
        }
        "AtomicCas#" => {
            let ptr = arg_as_i64(args, 0)? as u64;
            let expected = arg_as_i64(args, 1)?;
            let desired = arg_as_i64(args, 2)?;
            let current = heap.read(ptr, 8).map(|b| i64::from_le_bytes([b[0],b[1],b[2],b[3],b[4],b[5],b[6],b[7]])).unwrap_or(0);
            if current == expected {
                heap.write(ptr, &desired.to_le_bytes()).ok();
            }
            Ok(i64_to_bits(current))
        }
        "AtomicXchg#" => {
            let ptr = arg_as_i64(args, 0)? as u64;
            let val = arg_as_i64(args, 1)?;
            let old = heap.read(ptr, 8).map(|b| i64::from_le_bytes([b[0],b[1],b[2],b[3],b[4],b[5],b[6],b[7]])).unwrap_or(0);
            heap.write(ptr, &val.to_le_bytes()).ok();
            Ok(i64_to_bits(old))
        }
        "AtomicAdd#" => {
            let ptr = arg_as_i64(args, 0)? as u64;
            let val = arg_as_i64(args, 1)?;
            let old = heap.read(ptr, 8).map(|b| i64::from_le_bytes([b[0],b[1],b[2],b[3],b[4],b[5],b[6],b[7]])).unwrap_or(0);
            let new = old.wrapping_add(val);
            heap.write(ptr, &new.to_le_bytes()).ok();
            Ok(i64_to_bits(old))
        }
        "Fence#" => {
            Ok(Value::Void)
        }

        // ── Dynamic linker intrinsics (observable) ────────────────────
        // 2026-07-15: In check mode, call host libc functions.
        "DlOpen#" => {
            let path_ptr = arg_as_i64(args, 0)? as *const libc::c_char;
            let flags = arg_as_i64(args, 1)? as libc::c_int;
            let result = unsafe { libc::dlopen(path_ptr, flags) };
            Ok(i64_to_bits(result as i64))
        }
        "DlSym#" => {
            let handle = arg_as_i64(args, 0)? as *mut libc::c_void;
            let symbol_ptr = arg_as_i64(args, 1)? as *const libc::c_char;
            let result = unsafe { libc::dlsym(handle, symbol_ptr) };
            Ok(i64_to_bits(result as i64))
        }
        "DlClose#" => {
            let handle = arg_as_i64(args, 0)? as *mut libc::c_void;
            let result = unsafe { libc::dlclose(handle) };
            Ok(i64_to_bits(result as i64))
        }

        // ── Backtrace intrinsic (observable) ──────────────────────────
        // 2026-07-15: In check mode, stub returning 0.
        "Backtrace#" => {
            Ok(i64_to_bits(0))
        }

        _ => Err(RuntimeError::UnsupportedIntrinsic(name.to_string())),
    }
}

// ── Polymorphic operation dispatchers ──────────────────────────────────

/// Execute a binary arithmetic operation. Tries float first, then int.
fn exec_binop(
    args: &[Value],
    int_op: fn(i64, i64) -> i64,
    float_op: fn(f64, f64) -> f64,
) -> Result<Value, RuntimeError> {
    if let (Ok(a), Ok(b)) = (arg_as_f64(args, 0), arg_as_f64(args, 1)) {
        return Ok(f64_to_bits(float_op(a, b)));
    }
    let a = arg_as_i64(args, 0)?;
    let b = arg_as_i64(args, 1)?;
    Ok(i64_to_bits(int_op(a, b)))
}

/// Execute a comparison. Tries float first, then int.
fn exec_cmp(
    args: &[Value],
    int_cmp: fn(i64, i64) -> bool,
    float_cmp: fn(f64, f64) -> bool,
) -> Result<Value, RuntimeError> {
    if let (Ok(a), Ok(b)) = (arg_as_f64(args, 0), arg_as_f64(args, 1)) {
        return Ok(bool_to_bits(float_cmp(a, b)));
    }
    let a = arg_as_i64(args, 0)?;
    let b = arg_as_i64(args, 1)?;
    Ok(bool_to_bits(int_cmp(a, b)))
}

/// Division with zero check. Tries float first, then int.
fn exec_div(args: &[Value]) -> Result<Value, RuntimeError> {
    if let (Ok(a), Ok(b)) = (arg_as_f64(args, 0), arg_as_f64(args, 1)) {
        if b == 0.0 { return Err(RuntimeError::DivisionByZero); }
        return Ok(f64_to_bits(a / b));
    }
    let a = arg_as_i64(args, 0)?;
    let b = arg_as_i64(args, 1)?;
    if b == 0 { return Err(RuntimeError::DivisionByZero); }
    Ok(i64_to_bits(a.wrapping_div(b)))
}

/// Remainder with zero check.
fn exec_rem(args: &[Value]) -> Result<Value, RuntimeError> {
    let a = arg_as_i64(args, 0)?;
    let b = arg_as_i64(args, 1)?;
    if b == 0 { return Err(RuntimeError::DivisionByZero); }
    Ok(i64_to_bits(a.wrapping_rem(b)))
}

// ── Argument extraction helpers ────────────────────────────────────────

fn arg_as_i64(args: &[Value], index: usize) -> Result<i64, RuntimeError> {
    args.get(index)
        .and_then(|v| v.as_i64())
        .ok_or_else(|| RuntimeError::TypeError {
            expected: "Int".into(),
            found: format!("{:?}", args.get(index)),
        })
}

fn arg_as_f64(args: &[Value], index: usize) -> Result<f64, RuntimeError> {
    args.get(index)
        .and_then(|v| v.as_f64())
        .ok_or_else(|| RuntimeError::TypeError {
            expected: "Float".into(),
            found: format!("{:?}", args.get(index)),
        })
}

fn arg_as_string(args: &[Value], index: usize) -> Result<String, RuntimeError> {
    match args.get(index) {
        Some(Value::Bits(bytes)) => Ok(String::from_utf8_lossy(bytes).to_string()),
        _ => Err(RuntimeError::TypeError {
            expected: "String".into(),
            found: format!("{:?}", args.get(index)),
        }),
    }
}

/// 2026-07-15: Resolve a named address to its numeric value.
/// Delegates to the shared address_resolver module used by both
/// the interpreter and LLVM backend.
fn resolve_address_for_interp(id: &str) -> u64 {
    crate::address_resolver::resolve_address(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_i64() {
        let mut heap = VirtualHeap::new();
        let r = execute_intrinsic("Add#", &[i64_to_bits(2), i64_to_bits(3)], &mut heap).unwrap();
        assert_eq!(r.as_i64(), Some(5));
    }

    #[test]
    fn test_sub_i64() {
        let mut heap = VirtualHeap::new();
        let r = execute_intrinsic("Sub#", &[i64_to_bits(10), i64_to_bits(3)], &mut heap).unwrap();
        assert_eq!(r.as_i64(), Some(7));
    }

    #[test]
    fn test_mul_i64() {
        let mut heap = VirtualHeap::new();
        let r = execute_intrinsic("Mul#", &[i64_to_bits(4), i64_to_bits(5)], &mut heap).unwrap();
        assert_eq!(r.as_i64(), Some(20));
    }

    #[test]
    fn test_div_i64() {
        let mut heap = VirtualHeap::new();
        let r = execute_intrinsic("Div#", &[i64_to_bits(10), i64_to_bits(3)], &mut heap).unwrap();
        assert_eq!(r.as_i64(), Some(3));
    }

    #[test]
    fn test_div_by_zero() {
        let mut heap = VirtualHeap::new();
        let r = execute_intrinsic("Div#", &[i64_to_bits(1), i64_to_bits(0)], &mut heap);
        assert!(r.is_err());
    }

    #[test]
    fn test_eq_i64() {
        let mut heap = VirtualHeap::new();
        let r = execute_intrinsic("Eq#", &[i64_to_bits(42), i64_to_bits(42)], &mut heap).unwrap();
        assert!(r.is_true());
        let r = execute_intrinsic("Eq#", &[i64_to_bits(42), i64_to_bits(43)], &mut heap).unwrap();
        assert!(!r.is_true());
    }

    #[test]
    fn test_lt_i64() {
        let mut heap = VirtualHeap::new();
        let r = execute_intrinsic("Lt#", &[i64_to_bits(1), i64_to_bits(2)], &mut heap).unwrap();
        assert!(r.is_true());
        let r = execute_intrinsic("Lt#", &[i64_to_bits(2), i64_to_bits(1)], &mut heap).unwrap();
        assert!(!r.is_true());
    }

    #[test]
    fn test_fadd_f64() {
        let mut heap = VirtualHeap::new();
        let r = execute_intrinsic("Add#", &[f64_to_bits(1.5), f64_to_bits(2.5)], &mut heap).unwrap();
        assert!((r.as_f64().unwrap() - 4.0).abs() < 1e-10);
    }

    #[test]
    fn test_sqrt() {
        let mut heap = VirtualHeap::new();
        let r = execute_intrinsic("Sqrt#", &[f64_to_bits(16.0)], &mut heap).unwrap();
        assert!((r.as_f64().unwrap() - 4.0).abs() < 1e-10);
    }

    #[test]
    fn test_malloc_free() {
        let mut heap = VirtualHeap::new();
        let addr = execute_intrinsic("Malloc#", &[i64_to_bits(16)], &mut heap).unwrap();
        let ptr = addr.as_i64().unwrap() as u64;
        assert!(heap.contains(ptr));
        execute_intrinsic("Free#", &[addr], &mut heap).unwrap();
        assert!(!heap.contains(ptr));
    }

    #[test]
    fn test_alloc_interpreter() {
        let mut heap = VirtualHeap::new();
        let addr = execute_intrinsic("Alloc#", &[i64_to_bits(32)], &mut heap).unwrap();
        let ptr = addr.as_i64().unwrap() as u64;
        assert!(heap.contains(ptr));
    }

    #[test]
    fn test_unknown_intrinsic() {
        let mut heap = VirtualHeap::new();
        let r = execute_intrinsic("NonExistent#", &[], &mut heap);
        assert!(r.is_err());
    }

    #[test]
    fn test_float_to_int() {
        let mut heap = VirtualHeap::new();
        let r = execute_intrinsic("ToInt#", &[f64_to_bits(3.7)], &mut heap).unwrap();
        assert_eq!(r.as_i64(), Some(3));
    }

    #[test]
    fn test_int_to_float() {
        let mut heap = VirtualHeap::new();
        let r = execute_intrinsic("ToFloat#", &[i64_to_bits(42)], &mut heap).unwrap();
        assert!((r.as_f64().unwrap() - 42.0).abs() < 1e-10);
    }

    #[test]
    fn test_address_of_known() {
        let mut heap = VirtualHeap::new();
        let uart_str = Value::bits("uart".as_bytes().to_vec());
        let r = execute_intrinsic("AddressOf#", &[uart_str], &mut heap).unwrap();
        assert_eq!(r.as_i64(), Some(0xFFE01000i64));
    }

    #[test]
    fn test_address_of_unknown_defaults() {
        let mut heap = VirtualHeap::new();
        let dev_str = Value::bits("unknown_device".as_bytes().to_vec());
        let r = execute_intrinsic("AddressOf#", &[dev_str], &mut heap).unwrap();
        assert_eq!(r.as_i64(), Some(0xFE000000i64));
    }
}
