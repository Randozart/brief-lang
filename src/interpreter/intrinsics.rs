// ── Intrinsic Execution ────────────────────────────────────────────────
// 2026-07-14: Generic operations. For polymorphic ops (Add#, Eq#, etc.)
// the implementation checks argument types to dispatch int vs float.
// Flat dispatch: one match arm per intrinsic name, first match wins.

use crate::errors::RuntimeError;
use crate::interpreter::{bool_to_bits, f64_to_bits, i64_to_bits, zero_bits, Value, VirtualHeap};

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
        "Free#" => {
            let ptr = arg_as_i64(args, 0)?;
            heap.free(ptr as u64)
                .map_err(|_| RuntimeError::HeapError("free failed".into()))?;
            Ok(Value::Void)
        }
        "Memcpy#" => {
            let dst = arg_as_i64(args, 0)?;
            let src = arg_as_i64(args, 1)?;
            let n = arg_as_i64(args, 2)? as usize;
            let data = heap.read(src as u64, n)
                .ok_or_else(|| RuntimeError::HeapError("memcpy source read failed".into()))?;
            let data_vec = data.to_vec();
            heap.write(dst as u64, &data_vec)
                .map_err(|_| RuntimeError::HeapError("memcpy dest write failed".into()))?;
            Ok(Value::Void)
        }
        "Memset#" => {
            let ptr = arg_as_i64(args, 0)?;
            let val = arg_as_i64(args, 1)?;
            let n = arg_as_i64(args, 2)? as usize;
            let data = vec![val as u8; n];
            heap.write(ptr as u64, &data)
                .map_err(|_| RuntimeError::HeapError("memset failed".into()))?;
            Ok(Value::Void)
        }

        // ── I/O (observable) ────────────────────────────────────────
        "Print#" => {
            if let Ok(n) = arg_as_i64(args, 0) { eprintln!("{}", n); }
            else if let Ok(f) = arg_as_f64(args, 0) { eprintln!("{}", f); }
            else { let s = arg_as_string(args, 0)?; eprintln!("{}", s); }
            Ok(Value::Void)
        }
        "GetEnv#" => {
            let name = arg_as_string(args, 0)?;
            let val = std::env::var(&name)
                .ok().and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
            Ok(i64_to_bits(val))
        }

        // ── String / Conversion ─────────────────────────────────────
        "Concat#"   => Err(RuntimeError::UnsupportedIntrinsic("Concat#".to_string())),
        "Length#"   => Err(RuntimeError::UnsupportedIntrinsic("Length#".to_string())),
        "ToInt#"    => { let f = arg_as_f64(args, 0)?; Ok(i64_to_bits(f as i64)) }
        "ToFloat#"  => { let n = arg_as_i64(args, 0)?; Ok(f64_to_bits(n as f64)) }
        "ToString#" => Err(RuntimeError::UnsupportedIntrinsic("ToString#".to_string())),

        // ── Collection ──────────────────────────────────────────────
        "Get#"    => Err(RuntimeError::UnsupportedIntrinsic("Get#".to_string())),
        "Insert#" => Err(RuntimeError::UnsupportedIntrinsic("Insert#".to_string())),

        // ── GPU ─────────────────────────────────────────────────────
        "GetGlobalId#"   => Err(RuntimeError::UnsupportedIntrinsic("GetGlobalId#".to_string())),
        "GetGlobalSize#" => Err(RuntimeError::UnsupportedIntrinsic("GetGlobalSize#".to_string())),
        "GetLocalId#"    => Err(RuntimeError::UnsupportedIntrinsic("GetLocalId#".to_string())),

        // ── Pointers ─────────────────────────────────────────────────
        // 2026-07-15: AddressOf# resolves a named address to a pointer value.
        // In the interpreter, we look up the address map and return the i64 value.
        // For known addresses (from config/address-map.toml), this returns the
        // configured address. Unknown addresses return a default (0xFE000000).
        "AddressOf#" => {
            let id = arg_as_string(args, 0)?;
            let addr = resolve_address_for_interp(&id);
            Ok(i64_to_bits(addr as i64))
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
    fn test_print_int() {
        let mut heap = VirtualHeap::new();
        let r = execute_intrinsic("Print#", &[i64_to_bits(42)], &mut heap);
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
