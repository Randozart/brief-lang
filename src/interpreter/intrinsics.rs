// ── Intrinsic Execution ────────────────────────────────────────────────
// 2026-07-12: Phase 3.2 — execute_intrinsic(name, args, heap).
// One flat match arm per # intrinsic name, alphabetically sorted.
// Absorbs all of the deleted intrinsic_dispatch.rs.
// The _ => .. fallthrough must remain unchanged.

use crate::errors::RuntimeError;
use crate::interpreter::{bool_to_bits, f64_to_bits, i64_to_bits, zero_bits, Value, VirtualHeap};

/// Execute a named intrinsic with the given evaluated arguments.
/// Flat dispatch: one match arm per intrinsic name.
pub fn execute_intrinsic(
    name: &str,
    args: &[Value],
    heap: &mut VirtualHeap,
) -> Result<Value, RuntimeError> {
    match name {
        "AddI64#" => {
            let a = arg_as_i64(args, 0)?;
            let b = arg_as_i64(args, 1)?;
            Ok(i64_to_bits(a.wrapping_add(b)))
        }
        "DivI64#" => {
            let a = arg_as_i64(args, 0)?;
            let b = arg_as_i64(args, 1)?;
            if b == 0 {
                return Err(RuntimeError::DivisionByZero);
            }
            Ok(i64_to_bits(a.wrapping_div(b)))
        }
        "EqI1#" => {
            let a = arg_as_bool(args, 0)?;
            let b = arg_as_bool(args, 1)?;
            Ok(bool_to_bits(a == b))
        }
        "EqI32#" => {
            let a = arg_as_i64(args, 0)?;
            let b = arg_as_i64(args, 1)?;
            Ok(bool_to_bits(a == b))
        }
        "EqI64#" => {
            let a = arg_as_i64(args, 0)?;
            let b = arg_as_i64(args, 1)?;
            Ok(bool_to_bits(a == b))
        }
        "FAddF64#" => {
            let a = arg_as_f64(args, 0)?;
            let b = arg_as_f64(args, 1)?;
            Ok(f64_to_bits(a + b))
        }
        "FDivF64#" => {
            let a = arg_as_f64(args, 0)?;
            let b = arg_as_f64(args, 1)?;
            if b == 0.0 {
                return Err(RuntimeError::DivisionByZero);
            }
            Ok(f64_to_bits(a / b))
        }
        "FEqF64#" => {
            let a = arg_as_f64(args, 0)?;
            let b = arg_as_f64(args, 1)?;
            Ok(bool_to_bits((a - b).abs() < 1e-10))
        }
        "FLtF64#" => {
            let a = arg_as_f64(args, 0)?;
            let b = arg_as_f64(args, 1)?;
            Ok(bool_to_bits(a < b))
        }
        "FMulF64#" => {
            let a = arg_as_f64(args, 0)?;
            let b = arg_as_f64(args, 1)?;
            Ok(f64_to_bits(a * b))
        }
        "FSubF64#" => {
            let a = arg_as_f64(args, 0)?;
            let b = arg_as_f64(args, 1)?;
            Ok(f64_to_bits(a - b))
        }
        "FloatToInt#" => {
            let f = arg_as_f64(args, 0)?;
            Ok(i64_to_bits(f as i64))
        }
        "Free#" => {
            let ptr = arg_as_i64(args, 0)?;
            heap.free(ptr as u64)
                .map_err(|_| RuntimeError::HeapError("free failed".into()))?;
            Ok(Value::Void)
        }
        "GetEnvInt#" => {
            let name = arg_as_string(args, 0)?;
            let val = std::env::var(&name)
                .ok()
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0);
            Ok(i64_to_bits(val))
        }
        "IntToFloat#" => {
            let n = arg_as_i64(args, 0)?;
            Ok(f64_to_bits(n as f64))
        }
        "LtI64#" => {
            let a = arg_as_i64(args, 0)?;
            let b = arg_as_i64(args, 1)?;
            Ok(bool_to_bits(a < b))
        }
        "Malloc#" => {
            let size = arg_as_i64(args, 0)?;
            let addr = heap.allocate(size as usize);
            Ok(i64_to_bits(addr as i64))
        }
        "Memcpy#" => {
            let dst = arg_as_i64(args, 0)?;
            let src = arg_as_i64(args, 1)?;
            let n = arg_as_i64(args, 2)? as usize;
            let data = heap
                .read(src, n)
                .ok_or_else(|| RuntimeError::HeapError("memcpy source read failed".into()))?;
            let data_vec = data.to_vec();
            heap.write(dst, &data_vec)
                .map_err(|_| RuntimeError::HeapError("memcpy dest write failed".into()))?;
            Ok(Value::Void)
        }
        "MulI64#" => {
            let a = arg_as_i64(args, 0)?;
            let b = arg_as_i64(args, 1)?;
            Ok(i64_to_bits(a.wrapping_mul(b)))
        }
        "PrintInt#" => {
            let n = arg_as_i64(args, 0)?;
            eprintln!("{}", n);
            Ok(Value::Void)
        }
        "PrintString#" => {
            let s = arg_as_string(args, 0)?;
            eprintln!("{}", s);
            Ok(Value::Void)
        }
        "RemI64#" => {
            let a = arg_as_i64(args, 0)?;
            let b = arg_as_i64(args, 1)?;
            if b == 0 {
                return Err(RuntimeError::DivisionByZero);
            }
            Ok(i64_to_bits(a.wrapping_rem(b)))
        }
        "Sqrt#" => {
            let x = arg_as_f64(args, 0)?;
            Ok(f64_to_bits(x.sqrt()))
        }
        "SubI64#" => {
            let a = arg_as_i64(args, 0)?;
            let b = arg_as_i64(args, 1)?;
            Ok(i64_to_bits(a.wrapping_sub(b)))
        }
        _ => Err(RuntimeError::UnsupportedIntrinsic(name.to_string())),
    }
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

fn arg_as_bool(args: &[Value], index: usize) -> Result<bool, RuntimeError> {
    args.get(index)
        .and_then(|v| v.as_bool())
        .ok_or_else(|| RuntimeError::TypeError {
            expected: "Bool".into(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_i64() {
        let mut heap = VirtualHeap::new();
        let result =
            execute_intrinsic("AddI64#", &[i64_to_bits(2), i64_to_bits(3)], &mut heap).unwrap();
        assert_eq!(result.as_i64(), Some(5));
    }

    #[test]
    fn test_sub_i64() {
        let mut heap = VirtualHeap::new();
        let result =
            execute_intrinsic("SubI64#", &[i64_to_bits(10), i64_to_bits(3)], &mut heap).unwrap();
        assert_eq!(result.as_i64(), Some(7));
    }

    #[test]
    fn test_mul_i64() {
        let mut heap = VirtualHeap::new();
        let result =
            execute_intrinsic("MulI64#", &[i64_to_bits(4), i64_to_bits(5)], &mut heap).unwrap();
        assert_eq!(result.as_i64(), Some(20));
    }

    #[test]
    fn test_div_i64() {
        let mut heap = VirtualHeap::new();
        let result =
            execute_intrinsic("DivI64#", &[i64_to_bits(10), i64_to_bits(3)], &mut heap).unwrap();
        assert_eq!(result.as_i64(), Some(3));
    }

    #[test]
    fn test_div_by_zero() {
        let mut heap = VirtualHeap::new();
        let result = execute_intrinsic("DivI64#", &[i64_to_bits(1), i64_to_bits(0)], &mut heap);
        assert!(result.is_err());
    }

    #[test]
    fn test_eq_i64() {
        let mut heap = VirtualHeap::new();
        let r =
            execute_intrinsic("EqI64#", &[i64_to_bits(42), i64_to_bits(42)], &mut heap).unwrap();
        assert!(r.is_true());
        let r =
            execute_intrinsic("EqI64#", &[i64_to_bits(42), i64_to_bits(43)], &mut heap).unwrap();
        assert!(!r.is_true());
    }

    #[test]
    fn test_lt_i64() {
        let mut heap = VirtualHeap::new();
        let r = execute_intrinsic("LtI64#", &[i64_to_bits(1), i64_to_bits(2)], &mut heap).unwrap();
        assert!(r.is_true());
        let r = execute_intrinsic("LtI64#", &[i64_to_bits(2), i64_to_bits(1)], &mut heap).unwrap();
        assert!(!r.is_true());
    }

    #[test]
    fn test_fadd_f64() {
        let mut heap = VirtualHeap::new();
        let r = execute_intrinsic("FAddF64#", &[f64_to_bits(1.5), f64_to_bits(2.5)], &mut heap)
            .unwrap();
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
    fn test_print_int() {
        let mut heap = VirtualHeap::new();
        let r = execute_intrinsic("PrintInt#", &[i64_to_bits(42)], &mut heap);
        assert!(r.is_ok());
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
        let r = execute_intrinsic("FloatToInt#", &[f64_to_bits(3.7)], &mut heap).unwrap();
        assert_eq!(r.as_i64(), Some(3));
    }

    #[test]
    fn test_int_to_float() {
        let mut heap = VirtualHeap::new();
        let r = execute_intrinsic("IntToFloat#", &[i64_to_bits(42)], &mut heap).unwrap();
        assert!((r.as_f64().unwrap() - 42.0).abs() < 1e-10);
    }
}
