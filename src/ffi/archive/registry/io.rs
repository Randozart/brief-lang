use crate::errors::RuntimeError;
use crate::interpreter::{bool_to_bits, f64_to_bits, i64_to_bits, zero_bits, Value};
use std::sync::atomic::{self, Ordering};
use std::sync::atomic::AtomicU32;

fn value_to_string(args: &[Value], idx: usize) -> Result<String, RuntimeError> {
    match &args[idx] {
        Value::Bits(b) => Ok(String::from_UTF8_lossy(b).to_string()),
        _ => Err(RuntimeError::TypeError {
            expected: "String".into(),
            found: format!("{:?}", args.get(idx)),
        }),
    }
}

fn value_to_i64(args: &[Value], idx: usize) -> Result<i64, RuntimeError> {
    args[idx]
        .as_i64()
        .ok_or_else(|| RuntimeError::TypeError {
            expected: "Int".into(),
            found: format!("{:?}", args.get(idx)),
        })
}

fn value_to_i32(args: &[Value], idx: usize) -> Result<i32, RuntimeError> {
    let n = value_to_i64(args, idx)?;
    Ok(n as i32)
}

fn value_to_usize(args: &[Value], idx: usize) -> Result<usize, RuntimeError> {
    let n = value_to_i64(args, idx)?;
    Ok(n as usize)
}

fn value_to_u32(args: &[Value], idx: usize) -> Result<u32, RuntimeError> {
    let n = value_to_i64(args, idx)?;
    Ok(n as u32)
}

fn value_to_ptr_offset(args: &[Value], idx: usize) -> Result<*mut u8, RuntimeError> {
    let n = value_to_i64(args, idx)?;
    Ok(n as *mut u8)
}

// ── I/O ──

pub fn print_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    for arg in &args {
        match arg {
            Value::Bits(b) => print!("{}", String::from_UTF8_lossy(b)),
            Value::Void => print!("void"),
            _ => print!("{:?}", arg),
        }
    }
    Ok(Value::Void)
}
pub fn println_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    for arg in &args {
        match arg {
            Value::Bits(b) => print!("{}", String::from_UTF8_lossy(b)),
            Value::Void => print!("void"),
            _ => print!("{:?}", arg),
        }
    }
    println!();
    Ok(Value::Void)
}
pub fn input_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    use std::io::Write;
    if let Some(Value::Bits(prompt)) = args.first() {
        print!("{}", String::from_UTF8_lossy(prompt));
        std::io::stdout().flush().ok();
    }
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .map_err(|e| RuntimeError::HeapError(format!("stdin read: {}", e)))?;
    Ok(Value::Bits(line.trim_end().to_string().into_bytes()))
}

pub fn read_file_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let path = value_to_string(&args, 0)?;
    match std::fs::read_to_string(&path) {
        Ok(s) => Ok(Value::Bits(s.into_bytes())),
        Err(e) => Err(RuntimeError::HeapError(format!("read '{}': {}", path, e))),
    }
}
pub fn write_file_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let path = value_to_string(&args, 0)?;
    let content = value_to_string(&args, 1)?;
    match std::fs::write(&path, &content) {
        Ok(_) => Ok(Value::Void),
        Err(e) => Err(RuntimeError::HeapError(format!("write '{}': {}", path, e))),
    }
}
pub fn delete_file_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let path = value_to_string(&args, 0)?;
    match std::fs::remove_file(&path) {
        Ok(_) => Ok(Value::Void),
        Err(e) => Err(RuntimeError::HeapError(format!("delete '{}': {}", path, e))),
    }
}
pub fn create_dir_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let path = value_to_string(&args, 0)?;
    match std::fs::create_dir_all(&path) {
        Ok(_) => Ok(Value::Void),
        Err(e) => Err(RuntimeError::HeapError(format!("mkdir '{}': {}", path, e))),
    }
}
pub fn delete_dir_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let path = value_to_string(&args, 0)?;
    match std::fs::remove_dir_all(&path) {
        Ok(_) => Ok(Value::Void),
        Err(e) => Err(RuntimeError::HeapError(format!("rmdir '{}': {}", path, e))),
    }
}

// ── DBVL ──

pub fn dbvl_append_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(RuntimeError::HeapError(
            "dbvl_append requires at least 2 arguments: path (String), values (List)".into()
        ));
    }
    let path = match &args[0] {
        Value::Bits(b) => String::from_UTF8_lossy(b).to_string(),
        other => {
            return Err(RuntimeError::TypeError {
                expected: "String".into(),
                found: format!("{:?}", other),
            });
        }
    };
    let values = match &args[1] {
        Value::List(items) => items.clone(),
        other => {
            return Err(RuntimeError::TypeError {
                expected: "List".into(),
                found: format!("{:?}", other),
            });
        }
    };
    let csv_parts: Vec<String> = values.iter().map(|v| {
        let Value::Bits(b) = v else { return format!("{:?}", v); };
        if b.contains(&0u8) || !b.iter().all(|&x| x.is_ASCII_graphic() || x.is_ASCII_whitespace()) {
            let mut arr = [0u8; 8];
            let copy_len = b.len().min(8);
            arr[..copy_len].copy_from_slice(&b[..copy_len]);
            i64::from_le_bytes(arr).to_string()
        } else {
            let s = String::from_UTF8_lossy(b);
            if s.contains(',') || s.contains('"') || s.contains('\n') {
                format!("\"{}\"", s.replace('"', "\"\""))
            } else {
                s.to_string()
            }
        }
    }).collect();
    let line = csv_parts.join(",") + "\n";
    use std::io::Write;
    match std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        Ok(mut file) => {
            if let Err(e) = file.write_all(line.as_bytes()) {
                Err(RuntimeError::HeapError(format!("Failed to append to '{}': {}", path, e)))
            } else {
                Ok(Value::Bits(vec![1u8]))
            }
        }
        Err(e) => Err(RuntimeError::HeapError(format!("Failed to open '{}' for appending: {}", path, e))),
    }
}

// ── Math ──

pub fn abs_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let x = args.first()
        .and_then(|v| v.as_f64())
        .ok_or_else(|| RuntimeError::TypeError {
            expected: "Float".into(),
            found: format!("{:?}", args.first()),
        })?;
    Ok(f64_to_bits(x.abs()))
}
pub fn sqrt_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let x = args.first()
        .and_then(|v| v.as_f64())
        .ok_or_else(|| RuntimeError::TypeError {
            expected: "Float".into(),
            found: format!("{:?}", args.first()),
        })?;
    Ok(f64_to_bits(x.sqrt()))
}
pub fn pow_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let base = args.first()
        .and_then(|v| v.as_f64())
        .ok_or_else(|| RuntimeError::TypeError {
            expected: "Float".into(),
            found: format!("{:?}", args.first()),
        })?;
    let exp = args.get(1)
        .and_then(|v| v.as_f64())
        .ok_or_else(|| RuntimeError::TypeError {
            expected: "Float".into(),
            found: format!("{:?}", args.get(1)),
        })?;
    Ok(f64_to_bits(base.powf(exp)))
}
pub fn sin_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let x = args.first()
        .and_then(|v| v.as_f64())
        .ok_or_else(|| RuntimeError::TypeError {
            expected: "Float".into(),
            found: format!("{:?}", args.first()),
        })?;
    Ok(f64_to_bits(x.sin()))
}
pub fn cos_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let x = args.first()
        .and_then(|v| v.as_f64())
        .ok_or_else(|| RuntimeError::TypeError {
            expected: "Float".into(),
            found: format!("{:?}", args.first()),
        })?;
    Ok(f64_to_bits(x.cos()))
}
pub fn floor_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let x = args.first()
        .and_then(|v| v.as_f64())
        .ok_or_else(|| RuntimeError::TypeError {
            expected: "Float".into(),
            found: format!("{:?}", args.first()),
        })?;
    Ok(f64_to_bits(x.floor()))
}
pub fn ceil_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let x = args.first()
        .and_then(|v| v.as_f64())
        .ok_or_else(|| RuntimeError::TypeError {
            expected: "Float".into(),
            found: format!("{:?}", args.first()),
        })?;
    Ok(f64_to_bits(x.ceil()))
}
pub fn round_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let x = args.first()
        .and_then(|v| v.as_f64())
        .ok_or_else(|| RuntimeError::TypeError {
            expected: "Float".into(),
            found: format!("{:?}", args.first()),
        })?;
    Ok(f64_to_bits(x.round()))
}

// ── Time ──

pub fn now_impl(_args: Vec<Value>) -> Result<Value, RuntimeError> {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    Ok(i64_to_bits(duration.as_secs() as i64))
}

// ── TTY / Process ──

pub fn tty_raw_mode_impl(_args: Vec<Value>) -> Result<Value, RuntimeError> {
    Ok(Value::Void)
}
pub fn tty_size_impl(_args: Vec<Value>) -> Result<Value, RuntimeError> {
    let (cols, rows) = (80u16, 24u16);
    Ok(Value::List(vec![i64_to_bits(cols as i64), i64_to_bits(rows as i64)]))
}
pub fn tty_read_key_impl(_args: Vec<Value>) -> Result<Value, RuntimeError> {
    Ok(Value::Void)
}
pub fn exec_cmd_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let cmd = value_to_string(&args, 0)?;
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .output()
        .map_err(|e| RuntimeError::HeapError(format!("exec '{}': {}", cmd, e)))?;
    let mut fields = std::collections::HashMap::new();
    fields.insert("stdout".to_string(), Value::Bits(output.stdout));
    fields.insert("stderr".to_string(), Value::Bits(output.stderr));
    fields.insert("status".to_string(), i64_to_bits(output.status.code().unwrap_or(-1) as i64));
    Ok(Value::Instance {
        typename: "CommandResult".to_string(),
        fields,
    })
}

// ── HTTP ──

pub fn http_get_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Bits(data)) => {
            let url = String::from_UTF8_lossy(data);
            let response = ureq::get(&url)
                .call()
                .map_err(|e| RuntimeError::HeapError(format!("http::get failed: {}", e)))?;
            let body = response
                .into_string()
                .map_err(|e| RuntimeError::HeapError(format!("http::get response read failed: {}", e)))?;
            Ok(Value::Bits(body.into_bytes()))
        }
        Some(other) => Ok(other.clone()),
        None => Err(RuntimeError::HeapError("http::get expects 1 argument (URL string)".to_string())),
    }
}

pub fn http_post_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match (args.first(), args.get(1)) {
        (Some(Value::Bits(url_data)), Some(Value::Bits(body_data))) => {
            let url = String::from_UTF8_lossy(url_data);
            let body_str = String::from_UTF8_lossy(body_data);
            let response = ureq::post(&url)
                .send_string(&body_str)
                .map_err(|e| RuntimeError::HeapError(format!("http::post failed: {}", e)))?;
            let body = response
                .into_string()
                .map_err(|e| RuntimeError::HeapError(format!("http::post response read failed: {}", e)))?;
            Ok(Value::Bits(body.into_bytes()))
        }
        _ => Err(RuntimeError::HeapError("http::post expects 2 arguments (URL string, body string)".to_string())),
    }
}

// ── Metropolitan SHM ──

pub fn metro_shm_open_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let name = value_to_string(&args, 0)?;
    let flags = value_to_i32(&args, 1)?;
    let mode = value_to_i32(&args, 2)?;
    unsafe {
        let fd = libc::shm_open(name.as_ptr() as *const i8, flags, mode as libc::mode_t);
        if fd < 0 {
            let err = std::io::Error::last_os_error();
            Ok(Value::Enum("ShmError".to_string(), "ShmOther".to_string(), {
                let mut m = std::collections::HashMap::new();
                m.insert("message".to_string(), Value::Bits(err.to_string().into_bytes()));
                m
            }))
        } else {
            Ok(i64_to_bits(fd as i64))
        }
    }
}

pub fn metro_shm_unlink_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let name = value_to_string(&args, 0)?;
    unsafe {
        let ret = libc::shm_unlink(name.as_ptr() as *const i8);
        if ret == 0 { Ok(Value::Void) } else {
            let err = std::io::Error::last_os_error();
            Ok(Value::Enum("ShmError".to_string(), "ShmOther".to_string(), {
                let mut m = std::collections::HashMap::new();
                m.insert("message".to_string(), Value::Bits(err.to_string().into_bytes()));
                m
            }))
        }
    }
}

pub fn metro_ftruncate_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let fd = value_to_i32(&args, 0)?;
    let length = value_to_i64(&args, 1)?;
    unsafe {
        let ret = libc::ftruncate(fd, length);
        if ret == 0 { Ok(Value::Void) } else {
            let err = std::io::Error::last_os_error();
            Ok(Value::Enum("ShmError".to_string(), "ShmOther".to_string(), {
                let mut m = std::collections::HashMap::new();
                m.insert("message".to_string(), Value::Bits(err.to_string().into_bytes()));
                m
            }))
        }
    }
}

pub fn metro_shm_list_impl(_args: Vec<Value>) -> Result<Value, RuntimeError> {
    let mut names = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/dev/shm") {
        for entry in entries.flatten() {
            if let Ok(name) = entry.file_name().into_string() {
                names.push(Value::Bits(name.into_bytes()));
            }
        }
    }
    Ok(Value::List(names))
}

pub fn metro_shm_exists_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let name = value_to_string(&args, 0)?;
    let exists = std::path::Path::new(&format!("/dev/shm/{}", name.trim_start_matches('/'))).exists();
    Ok(Value::Bits(vec![if exists { 1u8 } else { 0u8 }]))
}

pub fn metro_shm_size_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let name = value_to_string(&args, 0)?;
    let name_c = std::ffi::CString::new(name.clone()).map_err(|_| {
        RuntimeError::TypeError { expected: "valid SHM name".into(), found: format!("{:?}", name) }
    })?;
    unsafe {
        let fd = libc::shm_open(name_c.as_ptr(), libc::O_RDONLY, 0);
        if fd < 0 {
            return Ok(Value::Enum("ShmError".to_string(), "ShmNotFound".to_string(), {
                let mut m = std::collections::HashMap::new();
                m.insert("message".to_string(), Value::Bits(b"Not found".to_vec()));
                m
            }));
        }
        let mut stat: libc::stat = std::mem::zeroed();
        let ret = libc::fstat(fd, &mut stat);
        libc::close(fd);
        if ret == 0 {
            Ok(i64_to_bits(stat.st_size as i64))
        } else {
            Ok(Value::Enum("ShmError".to_string(), "ShmOther".to_string(), {
                let mut m = std::collections::HashMap::new();
                m.insert("message".to_string(), Value::Bits(b"fstat failed".to_vec()));
                m
            }))
        }
    }
}

// ── Metropolitan MMAP ──

pub fn metro_mmap_anonymous_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let length = value_to_usize(&args, 0)?;
    let prot = value_to_i32(&args, 1)?;
    let flags = value_to_i32(&args, 2)?;
    unsafe {
        let addr = libc::mmap(std::ptr::null_mut(), length, prot, flags, -1, 0);
        if addr == libc::MAP_FAILED {
            let err = std::io::Error::last_os_error();
            Ok(Value::Enum("MmapError".to_string(), "MmapOther".to_string(), {
                let mut m = std::collections::HashMap::new();
                m.insert("message".to_string(), Value::Bits(err.to_string().into_bytes()));
                m
            }))
        } else {
            Ok(i64_to_bits(addr as i64))
        }
    }
}

pub fn metro_munmap_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let addr = value_to_ptr_offset(&args, 0)?;
    let length = value_to_usize(&args, 1)?;
    unsafe {
        let ret = libc::munmap(addr as *mut libc::c_void, length);
        if ret == 0 { Ok(Value::Void) } else {
            let err = std::io::Error::last_os_error();
            Ok(Value::Enum("MmapError".to_string(), "MmapOther".to_string(), {
                let mut m = std::collections::HashMap::new();
                m.insert("message".to_string(), Value::Bits(err.to_string().into_bytes()));
                m
            }))
        }
    }
}

pub fn metro_msync_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let addr = value_to_ptr_offset(&args, 0)?;
    let length = value_to_usize(&args, 1)?;
    let flags = value_to_i32(&args, 2)?;
    unsafe {
        let ret = libc::msync(addr as *mut libc::c_void, length, flags);
        if ret == 0 { Ok(Value::Void) } else {
            let err = std::io::Error::last_os_error();
            Ok(Value::Enum("MmapError".to_string(), "MmapOther".to_string(), {
                let mut m = std::collections::HashMap::new();
                m.insert("message".to_string(), Value::Bits(err.to_string().into_bytes()));
                m
            }))
        }
    }
}

pub fn metro_mmap_write_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let addr = value_to_ptr_offset(&args, 0)?;
    let offset = value_to_usize(&args, 1)?;
    let data = match &args[2] {
        Value::List(items) => items,
        _ => return Err(RuntimeError::TypeError {
            expected: "List<Int>".into(),
            found: format!("{:?}", args.get(2)),
        }),
    };
    let _len = value_to_usize(&args, 3)?;
    let target = unsafe { addr.add(offset) };
    for (i, item) in data.iter().enumerate() {
        let byte = match item.as_i64() {
            Some(n) => n as u8,
            None => return Err(RuntimeError::TypeError {
                expected: "Int".into(),
                found: format!("{:?}", item),
            }),
        };
        unsafe { *target.add(i) = byte; }
    }
    Ok(Value::Void)
}

pub fn metro_mmap_read_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let addr = value_to_ptr_offset(&args, 0)?;
    let offset = value_to_usize(&args, 1)?;
    let length = value_to_usize(&args, 2)?;
    let source = unsafe { addr.add(offset) };
    let mut result = Vec::with_capacity(length);
    for i in 0..length {
        unsafe { result.push(i64_to_bits(*source.add(i) as i64)); }
    }
    Ok(Value::List(result))
}

pub fn metro_mmap_read_u32_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let addr = value_to_ptr_offset(&args, 0)?;
    let offset = value_to_usize(&args, 1)?;
    unsafe {
        let ptr = addr.add(offset) as *const u32;
        let val = std::ptr::read_unaligned(ptr);
            Ok(i64_to_bits(val as i64))
    }
}

pub fn metro_mmap_write_u32_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let addr = value_to_ptr_offset(&args, 0)?;
    let offset = value_to_usize(&args, 1)?;
    let value = value_to_u32(&args, 2)?;
    unsafe {
        let ptr = addr.add(offset) as *mut u32;
        std::ptr::write_unaligned(ptr, value);
    }
    Ok(Value::Void)
}

// ── Metropolitan Atomic ──

pub fn metro_atomic_load_u32_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let addr = value_to_ptr_offset(&args, 0)?;
    let offset = value_to_usize(&args, 1)?;
    unsafe {
        let atomic_ref = &*(addr.add(offset) as *const AtomicU32);
        let val = atomic_ref.load(Ordering::SeqCst);
            Ok(i64_to_bits(val as i64))
    }
}

pub fn metro_atomic_store_u32_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let addr = value_to_ptr_offset(&args, 0)?;
    let offset = value_to_usize(&args, 1)?;
    let value = value_to_u32(&args, 2)?;
    unsafe {
        let atomic_ref = &*(addr.add(offset) as *const AtomicU32);
        atomic_ref.store(value, Ordering::SeqCst);
    }
    Ok(Value::Void)
}

pub fn metro_atomic_cas_u32_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let addr = value_to_ptr_offset(&args, 0)?;
    let offset = value_to_usize(&args, 1)?;
    let expected = value_to_u32(&args, 2)?;
    let new_value = value_to_u32(&args, 3)?;
    unsafe {
        let atomic_ref = &*(addr.add(offset) as *const AtomicU32);
        let prev = atomic_ref.compare_exchange(expected, new_value, Ordering::SeqCst, Ordering::SeqCst);
        Ok(i64_to_bits(prev.unwrap_or(expected) as i64))
    }
}

pub fn metro_atomic_fence_impl(_args: Vec<Value>) -> Result<Value, RuntimeError> {
    atomic::fence(Ordering::SeqCst);
    Ok(Value::Void)
}

pub fn metro_atomic_xchg_u32_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let addr = value_to_ptr_offset(&args, 0)?;
    let offset = value_to_usize(&args, 1)?;
    let value = value_to_u32(&args, 2)?;
    unsafe {
        let atomic_ref = &*(addr.add(offset) as *const AtomicU32);
        let prev = atomic_ref.swap(value, Ordering::SeqCst);
        Ok(i64_to_bits(prev as i64))
    }
}

pub fn metro_atomic_add_u32_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let addr = value_to_ptr_offset(&args, 0)?;
    let offset = value_to_usize(&args, 1)?;
    let value = value_to_u32(&args, 2)?;
    unsafe {
        let atomic_ref = &*(addr.add(offset) as *const AtomicU32);
        let prev = atomic_ref.fetch_add(value, Ordering::SeqCst);
        Ok(i64_to_bits(prev as i64))
    }
}

// ── Metropolitan Channel ──

use crate::ffi::metropolitan::MetropolitanHub;
use std::sync::Arc;

static GLOBAL_METRO_HUB: once_cell::sync::Lazy<Arc<MetropolitanHub>> =
    once_cell::sync::Lazy::new(|| Arc::new(MetropolitanHub::new()));

pub fn metro_channel_create_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let channel_id = value_to_string(&args, 0)?;
    let input_size = value_to_usize(&args, 1)?;
    let output_size = value_to_usize(&args, 2)?;
    match GLOBAL_METRO_HUB.create_channel(&channel_id, "native", input_size, output_size) {
        Ok(ch) => {
            let addrs = ch.get_addresses();
            let req_addr = *addrs.get("request").unwrap_or(&0);
            let resp_addr = *addrs.get("response").unwrap_or(&0);
            let sync_addr = *addrs.get("sync").unwrap_or(&0);
            let mut fields = std::collections::HashMap::new();
            fields.insert("request_addr".to_string(), i64_to_bits(req_addr as i64));
            fields.insert("response_addr".to_string(), i64_to_bits(resp_addr as i64));
            fields.insert("sync_addr".to_string(), i64_to_bits(sync_addr as i64));
            fields.insert("handle".to_string(), i64_to_bits(0));
            Ok(Value::Instance { typename: "MetroChannel".to_string(), fields })
        }
        Err(e) => Err(RuntimeError::UndefinedForeignFunction { name: "metro_channel_create".into(), source: e }),
    }
}

pub fn metro_channel_destroy_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let channel_id = value_to_string(&args, 0)?;
    let _ = GLOBAL_METRO_HUB.close_channel(&channel_id);
    Ok(Value::Void)
}

pub fn metro_channel_get_layout_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let channel_id = value_to_string(&args, 0)?;
    match GLOBAL_METRO_HUB.get_channel(&channel_id) {
        Some(ch) => {
            let addrs = ch.get_addresses();
            let mut fields = std::collections::HashMap::new();
            for (k, v) in addrs {
                fields.insert(k, i64_to_bits(v as i64));
            }
            Ok(Value::Instance { typename: "Layout".to_string(), fields })
        }
        None => Err(RuntimeError::UndefinedForeignFunction { name: "metro_channel_get_layout".into(), source: format!("Channel not found: {}", channel_id) }),
    }
}

pub fn metro_channel_gen_c_header_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let channel_id = value_to_string(&args, 0)?;
    match GLOBAL_METRO_HUB.generate_c_header(&channel_id) {
        Ok(header) => Ok(Value::Bits(header.into_bytes())),
        Err(e) => Err(RuntimeError::UndefinedForeignFunction { name: "metro_channel_gen_c_header".into(), source: e }),
    }
}
