// Copyright 2026 Randy Smits-Schreuder Goedheijt
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// ── Intrinsic Call Dispatch ──────────────────────────────────────────
//
// This submodule owns the Intrinsic enum dispatch for the interpreter.
// `eval_intrinsic` is the entry point — a flat match on every Intrinsic
// variant. Non-trivial arm bodies are extracted into named helpers.
//
// Every helper follows max 2 nesting with guard clauses.
// Trivial arms (one-liners returning Ok or Err) stay inline.
//
// 2026-07-12: Extracted from eval.rs during Phase 4 clean-up.

use super::intrinsics::{bits_to_f64, bits_to_i64, f64_to_bits, i64_to_bits, value_as_bool, value_as_f64, value_as_i64};
use super::{execute_intrinsic, Interpreter, RuntimeError, Value};
use crate::ast::*;
use crate::ffi::FFI_REGISTRY;
use std::collections::{HashMap, HashSet};
use std::os::unix::io::FromRawFd;

impl Interpreter {
    /// Dispatch an Intrinsic call.
    ///
    /// Evaluates all argument expressions, then dispatches by variant.
    /// Every non-trivial arm extracts its body to a named helper function.
    pub(crate) fn eval_intrinsic(
        &mut self,
        intrinsic: &Intrinsic,
        args: &[Expr],
    ) -> Result<Value, RuntimeError> {
        let values: Result<Vec<Value>, _> = args.iter().map(|a| self.eval_expr(a)).collect();
        let mut values = values?;

        match intrinsic {
            // ── Math (extracted to shared unary helpers) ─────────────---
            Intrinsic::Sqrt => eval_unary_float(&values, |f| f.sqrt(), "sqrt"),
            Intrinsic::Fabs => eval_unary_float(&values, |f| f.abs(), "fabs"),
            Intrinsic::Ceil => eval_unary_float(&values, |f| f.ceil(), "ceil"),
            Intrinsic::Floor => eval_unary_float(&values, |f| f.floor(), "floor"),
            Intrinsic::Sin => eval_unary_float(&values, |f| f.sin(), "sin"),
            Intrinsic::Cos => eval_unary_float(&values, |f| f.cos(), "cos"),
            Intrinsic::Abs => eval_unary_int(&values, |n| n.abs(), "abs"),
            Intrinsic::Ctpop => eval_unary_int(&values, |n| n.count_ones() as i64, "ctpop"),
            Intrinsic::Ctlz => eval_unary_int(&values, |n| n.leading_zeros() as i64, "ctlz"),
            Intrinsic::Cttz => eval_unary_int(&values, |n| n.trailing_zeros() as i64, "cttz"),
            Intrinsic::Bitreverse => eval_unary_int(&values, |n| n.reverse_bits() as i64, "bitreverse"),
            Intrinsic::Pow => self.eval_intrinsic_pow(&values),
            Intrinsic::ByteCount => self.eval_intrinsic_byte_count(&values),

            // ── String / Memory ────────────────────────────────────────
            Intrinsic::StrBytes => eval_str_bytes(&values),
            Intrinsic::Strlen => eval_str_len(&values),
            Intrinsic::Size => self.eval_intrinsic_size(&values),
            Intrinsic::Pop => self.eval_intrinsic_pop(&values),
            Intrinsic::Contains => self.eval_intrinsic_contains(&values),
            Intrinsic::Keys => self.eval_intrinsic_keys(&values),
            Intrinsic::Values => self.eval_intrinsic_values(&values),
            Intrinsic::StringConcat => eval_string_concat(&values),
            Intrinsic::StringEq => eval_string_eq(&values),
            Intrinsic::StringFind => eval_string_find(&values),
            Intrinsic::StringCompare => eval_string_compare(&values),
            Intrinsic::StringToInt => eval_string_to_int(&values),
            Intrinsic::IntToString => eval_int_to_string(&values),
            Intrinsic::StringToFloat => eval_string_to_float(&values),
            Intrinsic::FloatToString => eval_float_to_string(&values),

            // ── Print / Input / Exit ──────────────────────────────────
            Intrinsic::Println => eval_println(&values),
            Intrinsic::Print => eval_print(&values),
            Intrinsic::Readln => eval_readln(),
            Intrinsic::Exit => eval_exit(&values),
            Intrinsic::Halt => Err(RuntimeError::Escaped),
            Intrinsic::Assert => self.eval_intrinsic_assert(&values),
            Intrinsic::Panic => eval_panic(&values),
            Intrinsic::Abort => panic!("Abort called"),

            // ── Time / Sleep / Random ─────────────────────────────────
            Intrinsic::Time => eval_time(),
            Intrinsic::RealTime => eval_realtime(),
            Intrinsic::Monotonic => eval_monotonic(),
            Intrinsic::Sleep => eval_sleep(&values),
            Intrinsic::GetRandom => Ok(Value::Bits(rand::random::<[u8; 8]>().to_vec())),
            Intrinsic::NanoSleep => eval_nanosleep(&values),
            Intrinsic::Yield => { std::thread::yield_now(); Ok(Value::Bits(vec![1u8])) },
            Intrinsic::CPUCycles => Ok(Value::Bits(i64_to_bits(0))),

            // ── Process / Env ──────────────────────────────────────────
            Intrinsic::Argv => eval_argv(),
            Intrinsic::GetEnv => eval_get_env(&values),
            Intrinsic::SetEnv => self.eval_set_env(&values),
            Intrinsic::Spawn => self.eval_spawn(&values),
            Intrinsic::SpawnWithOutput => self.eval_spawn_with_output(&values),
            Intrinsic::GetPid => Ok(Value::Bits(i64_to_bits(unsafe { libc::getpid() } as i64))),
            Intrinsic::GetPPid => Ok(Value::Bits(i64_to_bits(unsafe { libc::getppid() } as i64))),
            Intrinsic::GetUid => Ok(Value::Bits(i64_to_bits(unsafe { libc::getuid() } as i64))),
            Intrinsic::GetGid => Ok(Value::Bits(i64_to_bits(unsafe { libc::getgid() } as i64))),
            Intrinsic::GetEuid => Ok(Value::Bits(i64_to_bits(unsafe { libc::geteuid() } as i64))),
            Intrinsic::GetEgid => Ok(Value::Bits(i64_to_bits(unsafe { libc::getegid() } as i64))),
            Intrinsic::GetSid => Ok(Value::Bits(i64_to_bits(unsafe { libc::getsid(0) } as i64))),
            Intrinsic::GetPgid => Ok(Value::Bits(i64_to_bits(unsafe { libc::getpgid(0) } as i64))),
            Intrinsic::SetPgid => self.eval_set_pgid(&values),
            Intrinsic::GetCwd => eval_get_cwd(),
            Intrinsic::ChDir => self.eval_chdir(&values),
            Intrinsic::ThreadId => Ok(Value::Bits(i64_to_bits(unsafe { libc::gettid() } as i64))),

            // ── File I/O ───────────────────────────────────────────────
            Intrinsic::ReadFile => self.eval_read_file(&values),
            Intrinsic::WriteFile => self.eval_write_file(&values),
            Intrinsic::Open => self.eval_open(&values),
            Intrinsic::Close => eval_close(&values),
            Intrinsic::Read => self.eval_read_fd(&values),
            Intrinsic::Write => self.eval_write_fd(&values),
            Intrinsic::LSeek => eval_lseek(&values),
            Intrinsic::PRead => self.eval_pread(&values),
            Intrinsic::PWrite => eval_pwrite(&values),
            Intrinsic::Stat => self.eval_stat(&values),
            Intrinsic::FStat => self.eval_fstat(&values),
            Intrinsic::FTruncate => eval_ftruncate(&values),
            Intrinsic::FSync => eval_fsync(&values),
            Intrinsic::FDup => eval_fdup(&values),
            Intrinsic::FDup2 => eval_fdup2(&values),
            Intrinsic::FCntl => eval_fcntl(&values),
            Intrinsic::Pipe => eval_pipe(),
            Intrinsic::Dup => eval_fdup(&values),
            Intrinsic::Dup2 => eval_fdup2(&values),
            Intrinsic::MkDir => self.eval_mkdir(&values),
            Intrinsic::RmDir => self.eval_rmdir(&values),
            Intrinsic::Remove => self.eval_remove(&values),
            Intrinsic::Rename => self.eval_rename(&values),
            Intrinsic::ReadDir => self.eval_read_dir(&values),
            Intrinsic::Canonicalize => self.eval_canonicalize(&values),
            Intrinsic::RealPath => self.eval_canonicalize(&values),
            Intrinsic::Access => eval_access(&values),
            Intrinsic::UMask => eval_umask(&values),

            // ── Memory ─────────────────────────────────────────────────
            Intrinsic::Alloc => self.eval_alloc(&values),
            Intrinsic::Free => self.eval_free(&values),
            Intrinsic::Realloc => self.eval_realloc(&values),
            Intrinsic::MemoryCopy => self.eval_memory_copy(&values),
            Intrinsic::MemoryMove => self.eval_memory_move(&values),
            Intrinsic::MemorySet => self.eval_memory_set(&values),
            Intrinsic::VolatileLoad => self.eval_volatile_load(&values),
            Intrinsic::VolatileStore => self.eval_volatile_store(&values),
            Intrinsic::Mmap => eval_mmap(&values),
            Intrinsic::Munmap => eval_munmap(&values),
            Intrinsic::MProtect => eval_mprotect(&values),
            Intrinsic::MAdvise => eval_madvise(&values),
            Intrinsic::MLock => eval_mlock(&values),
            Intrinsic::MUnlock => eval_munlock(&values),

            // ── TTY ────────────────────────────────────────────────────
            Intrinsic::TtyRawMode => {
                let enable = bits_to_bool(&values[0]).unwrap_or(false);
                let result = ffi::set_tty_raw_mode(enable);
                Ok(Value::Bits(vec![if result { 1u8 } else { 0u8 }]))
            }
            Intrinsic::TtySize => {
                let (cols, rows) = ffi::get_terminal_size();
                Ok(Value::Bits(i64_to_bits(cols * 10000 + rows)))
            }
            Intrinsic::TtyReadKey => {
                match ffi::read_key_nonblocking() {
                    Some(c) => Ok(Value::Bits(i64_to_bits(c as i64))),
                    None => Ok(Value::Bits(i64_to_bits(-1))),
                }
            }
            Intrinsic::IoCtl => {
                let _fd = value_as_i64(&values[0]).unwrap_or(0);
                let _request = value_as_i64(&values[1]).unwrap_or(0);
                Ok(Value::Bits(i64_to_bits(-1)))
            }
            Intrinsic::IsTty => eval_is_tty(&values),

            // ── Encoding / Hashing ─────────────────────────────────────
            Intrinsic::SHA256 => eval_sha256(&values),
            Intrinsic::SHA512 => eval_sha512(&values),
            Intrinsic::MD5 => eval_md5(&values),
            Intrinsic::Base64Encode => eval_base64_encode(&values),
            Intrinsic::Base64Decode => eval_base64_decode(&values),
            Intrinsic::HexEncode => eval_hex_encode(&values),
            Intrinsic::HexDecode => eval_hex_decode(&values),
            Intrinsic::UUID => Ok(Value::Bits(uuid::Uuid::new_v4().to_string().into_bytes())),
            Intrinsic::URLEncode => eval_url_encode(&values),
            Intrinsic::URLDecode => eval_url_decode(&values),
            Intrinsic::Format => eval_format(&values),
            Intrinsic::Printf => self.eval_printf(&values),
            Intrinsic::Sprintf => eval_sprintf(&values),

            // ── Type Info ──────────────────────────────────────────────
            Intrinsic::TypeId => eval_type_id(&values),
            Intrinsic::TypeName => eval_type_name(&values),
            Intrinsic::SizeOf => eval_sizeof(&values),
            Intrinsic::AlignOf => Ok(Value::Bits(i64_to_bits(8))),
            Intrinsic::OffsetOf => Ok(Value::Bits(i64_to_bits(0))),
            Intrinsic::FieldOffset => Ok(Value::Bits(i64_to_bits(0))),

            // ── Pointer ────────────────────────────────────────────────
            Intrinsic::PtrToInt => Ok(Value::Bits(i64_to_bits(value_as_i64(&values[0]).unwrap_or(0)))),
            Intrinsic::IntToPtr => Ok(Value::Bits(i64_to_bits(value_as_i64(&values[0]).unwrap_or(0)))),
            Intrinsic::PtrOffset => eval_ptr_offset(&values),
            Intrinsic::PtrDiff => eval_ptr_diff(&values),
            Intrinsic::IsNull => {
                let is_null = value_as_i64(&values[0]).unwrap_or(0) == 0;
                Ok(Value::Bits(vec![if is_null { 1u8 } else { 0u8 }]))
            }

            // ── Collections ────────────────────────────────────────────
            Intrinsic::Slice => self.eval_intrinsic_slice(&values),
            Intrinsic::Reverse => self.eval_intrinsic_reverse(&values),
            Intrinsic::Sort => self.eval_intrinsic_sort(&values),
            Intrinsic::Append => self.eval_intrinsic_append(&values),
            Intrinsic::Prepend => self.eval_intrinsic_prepend(&values),
            Intrinsic::Length => eval_intrinsic_length(&values),
            Intrinsic::IsEmpty => eval_intrinsic_is_empty(&values),
            Intrinsic::First => ok_first(&values),
            Intrinsic::Last => ok_last(&values),
            Intrinsic::Nth => ok_nth(&values),
            Intrinsic::Take => ok_take(&values),
            Intrinsic::DropItems => ok_drop(&values),
            Intrinsic::Unique => self.eval_intrinsic_unique(&values),
            Intrinsic::Flatten => self.eval_intrinsic_flatten(&values),
            Intrinsic::Filter => self.eval_intrinsic_filter(&values),
            Intrinsic::Map => self.eval_intrinsic_map(&values),
            Intrinsic::Reduce => self.eval_intrinsic_reduce(&values),
            Intrinsic::Zip => eval_zip(&values),
            Intrinsic::Enumerate => eval_enumerate(&values),
            Intrinsic::GroupBy => self.eval_intrinsic_group_by(&values),
            Intrinsic::Chunks => eval_chunks(&values),
            Intrinsic::Windows => eval_windows(&values),
            Intrinsic::All => eval_all(&values),
            Intrinsic::Any => eval_any(&values),
            Intrinsic::Fold => self.eval_intrinsic_fold(&values),
            Intrinsic::Count => eval_count(&values),
            Intrinsic::IndexOf => eval_index_of(&values),
            Intrinsic::Range => eval_range(&values),
            Intrinsic::Fill => eval_fill(&values),
            Intrinsic::Repeat => eval_repeat(&values),
            Intrinsic::Interleave => eval_interleave(&values),
            Intrinsic::Partition => self.eval_intrinsic_partition(&values),
            Intrinsic::Find => eval_find(&values),
            Intrinsic::RingPush => self.eval_ring_push(&values),
            Intrinsic::RingPop => self.eval_ring_pop(&values),

            // ── Enum Constructors / Queries ────────────────────────────
            Intrinsic::Some => {
                Ok(Value::Enum("Option".into(), "Some".into(), HashMap::from([("value".into(), values.remove(0))])))
            }
            Intrinsic::NoneVal => Ok(Value::Enum("Option".into(), "None".into(), HashMap::new())),
            Intrinsic::Ok => {
                Ok(Value::Enum("Result".into(), "Ok".into(), HashMap::from([("value".into(), values.remove(0))])))
            }
            Intrinsic::ErrVal => {
                Ok(Value::Enum("Result".into(), "Err".into(), HashMap::from([("value".into(), values.remove(0))])))
            }
            Intrinsic::IsSome => ok_is_variant(&values, "Some"),
            Intrinsic::IsNone => ok_is_variant(&values, "None"),
            Intrinsic::IsOk => ok_is_variant(&values, "Ok"),
            Intrinsic::IsErr => ok_is_variant(&values, "Err"),
            Intrinsic::Unwrap => self.eval_unwrap(&values),
            Intrinsic::Expect => self.eval_expect(&values),

            // ── Hash / Misc ────────────────────────────────────────────
            Intrinsic::HashCombine => {
                let seed = value_as_i64(&values[0]).unwrap_or(0);
                let val = value_as_i64(&values[1]).unwrap_or(0);
                let combined = seed.wrapping_mul(0x100000001b3).wrapping_add(val);
                Ok(Value::Bits(i64_to_bits(combined)))
            }
            Intrinsic::DebugLog => {
                eprintln!("[DEBUG] {:?}", &values[0]);
                Ok(Value::Bits(vec![1u8]))
            }
            Intrinsic::DebugBreak => {
                #[cfg(debug_assertions)]
                std::thread::sleep(std::time::Duration::from_millis(1));
                Ok(Value::Bits(vec![1u8]))
            }
            Intrinsic::Unreachable => Err(RuntimeError::TypeMismatch("unreachable code".into())),
            Intrinsic::Todo => Err(RuntimeError::TypeMismatch("not yet implemented".into())),
            Intrinsic::Assume => Ok(Value::Bits(vec![1u8])),

            // ── DBVL ───────────────────────────────────────────────────
            Intrinsic::DbvlLoad => self.eval_dbvl_load(&values),
            Intrinsic::DbvlLookup => self.eval_dbvl_lookup(&values),
            Intrinsic::DbvlFilter => self.eval_dbvl_filter(&values),

            // ── Socket (placeholders) ──────────────────────────────────
            Intrinsic::Socket => Ok(Value::Bits(i64_to_bits(-1))),
            Intrinsic::Bind => Ok(Value::Bits(vec![0u8])),
            Intrinsic::Listen => Ok(Value::Bits(vec![0u8])),
            Intrinsic::Accept => Ok(Value::Bits(i64_to_bits(-1))),
            Intrinsic::Connect => Ok(Value::Bits(i64_to_bits(-1))),
            Intrinsic::Send => Ok(Value::Bits(i64_to_bits(0))),
            Intrinsic::Recv => Ok(Value::Bits(Vec::new())),
            Intrinsic::SendTo => Ok(Value::Bits(i64_to_bits(0))),
            Intrinsic::RecvFrom => Ok(Value::Bits(Vec::new())),
            Intrinsic::GetPeerName => Ok(Value::Bits(Vec::new())),
            Intrinsic::GetSockName => Ok(Value::Bits(Vec::new())),
            Intrinsic::GetSockOpt => Ok(Value::Bits(i64_to_bits(-1))),
            Intrinsic::SetSockOpt => Ok(Value::Bits(i64_to_bits(-1))),
            Intrinsic::GetAddrInfo => Ok(Value::Bits(i64_to_bits(-1))),
            Intrinsic::Select => Ok(Value::Bits(i64_to_bits(0))),
            Intrinsic::Poll => Ok(Value::Bits(i64_to_bits(0))),
            Intrinsic::EpollCreate => Ok(Value::Bits(i64_to_bits(-1))),
            Intrinsic::EpollCtl => Ok(Value::Bits(i64_to_bits(-1))),
            Intrinsic::EpollWait => Ok(Value::Bits(Vec::new())),
            Intrinsic::KQueue => Ok(Value::Bits(i64_to_bits(-1))),

            // ── Futex / Synchronization (placeholders) ─────────────────
            Intrinsic::Futex => Ok(Value::Bits(i64_to_bits(-1))),
            Intrinsic::FutexWake => Ok(Value::Bits(i64_to_bits(0))),
            Intrinsic::FutexWait => Ok(Value::Bits(i64_to_bits(-1))),
            Intrinsic::SemInit => Ok(Value::Bits(i64_to_bits(-1))),
            Intrinsic::SemWait => Ok(Value::Bits(i64_to_bits(-1))),
            Intrinsic::SemPost => Ok(Value::Bits(i64_to_bits(-1))),
            Intrinsic::SemDestroy => Ok(Value::Bits(i64_to_bits(-1))),
            Intrinsic::MutexInit => Ok(Value::Bits(i64_to_bits(-1))),
            Intrinsic::MutexLock => Ok(Value::Bits(i64_to_bits(-1))),
            Intrinsic::MutexUnlock => Ok(Value::Bits(i64_to_bits(-1))),
            Intrinsic::MutexDestroy => Ok(Value::Bits(i64_to_bits(-1))),
            Intrinsic::CondInit => Ok(Value::Bits(i64_to_bits(-1))),
            Intrinsic::CondWait => Ok(Value::Bits(i64_to_bits(-1))),
            Intrinsic::CondSignal => Ok(Value::Bits(i64_to_bits(-1))),
            Intrinsic::CondBroadcast => Ok(Value::Bits(i64_to_bits(-1))),
            Intrinsic::RwLockInit => Ok(Value::Bits(i64_to_bits(-1))),
            Intrinsic::RwLockRdLock => Ok(Value::Bits(i64_to_bits(-1))),
            Intrinsic::RwLockWrLock => Ok(Value::Bits(i64_to_bits(-1))),
            Intrinsic::RwLockUnlock => Ok(Value::Bits(i64_to_bits(-1))),
            Intrinsic::RwLockDestroy => Ok(Value::Bits(i64_to_bits(-1))),
            Intrinsic::BarrierInit => Ok(Value::Bits(i64_to_bits(-1))),
            Intrinsic::BarrierWait => Ok(Value::Bits(i64_to_bits(-1))),
            Intrinsic::BarrierDestroy => Ok(Value::Bits(i64_to_bits(-1))),
            Intrinsic::SpinLockInit => Ok(Value::Bits(i64_to_bits(-1))),
            Intrinsic::SpinLockLock => Ok(Value::Bits(i64_to_bits(-1))),
            Intrinsic::SpinLockUnlock => Ok(Value::Bits(i64_to_bits(-1))),
            Intrinsic::SpinLockDestroy => Ok(Value::Bits(i64_to_bits(-1))),

            // ── Atomics (placeholders) ─────────────────────────────────
            Intrinsic::AtomicLoad => Ok(Value::Bits(i64_to_bits(0))),
            Intrinsic::AtomicStore => Ok(Value::Bits(i64_to_bits(0))),
            Intrinsic::AtomicAdd => Ok(Value::Bits(i64_to_bits(0))),
            Intrinsic::AtomicSub => Ok(Value::Bits(i64_to_bits(0))),
            Intrinsic::AtomicAnd => Ok(Value::Bits(i64_to_bits(0))),
            Intrinsic::AtomicOr => Ok(Value::Bits(i64_to_bits(0))),
            Intrinsic::AtomicXor => Ok(Value::Bits(i64_to_bits(0))),
            Intrinsic::AtomicCAS => Ok(Value::Bits(i64_to_bits(0))),
            Intrinsic::AtomicSwap => Ok(Value::Bits(i64_to_bits(0))),

            // ── GPU / Fence (placeholders) ─────────────────────────────
            Intrinsic::GetGlobalId => Ok(Value::Bits(i64_to_bits(0))),
            Intrinsic::GetLocalId => Ok(Value::Bits(i64_to_bits(0))),
            Intrinsic::GetGroupId => Ok(Value::Bits(i64_to_bits(0))),
            Intrinsic::GetNumGroups => Ok(Value::Bits(i64_to_bits(1))),
            Intrinsic::GetLocalSize => Ok(Value::Bits(i64_to_bits(1))),
            Intrinsic::Barrier => Ok(Value::Bits(vec![0u8])),
            Intrinsic::MemFence => Ok(Value::Bits(vec![0u8])),
            Intrinsic::Fence => Ok(Value::Bits(vec![0u8])),

            // ── Fallback: user-defined intrinsic ──────────────────────
            _ => {
                let name = intrinsic.name();
                match execute_intrinsic(name, &values) {
                    Ok(v) => Ok(v),
                    Err(e) => Err(e),
                }
            }
        }
    }

    // ── Shared Unary Helpers ──────────────────────────────────────────

    /// Apply a float operation to values[0]. Guard: reject non-Bits.
    fn eval_pow(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let a = match &values[0] { Value::Bits(b) => bits_to_f64(&Value::Bits(b.clone()))?, _ => return Err(RuntimeError::TypeMismatch("pow requires Float".into())) };
        let b = match &values[1] { Value::Bits(b) => bits_to_f64(&Value::Bits(b.clone()))?, _ => return Err(RuntimeError::TypeMismatch("pow requires Float".into())) };
        Ok(Value::Bits(f64_to_bits(a.powf(b))))
    }

    fn eval_intrinsic_byte_count(&self, values: &[Value]) -> Result<Value, RuntimeError> {
        let len = match &values[0] {
            Value::Bits(b) => b.len(),
            Value::List(l) => l.len(),
            Value::HashMap(m) => m.len(),
            Value::Tuple(t) => t.len(),
            _ => return Err(RuntimeError::TypeMismatch("byte_count requires a value with size".into())),
        };
        Ok(Value::Bits(i64_to_bits(len as i64)))
    }

    fn eval_intrinsic_size(&self, values: &[Value]) -> Result<Value, RuntimeError> {
        let len = match &values[0] {
            Value::Bits(b) => b.len(),
            Value::List(l) => l.len(),
            Value::HashMap(m) => m.len(),
            Value::Tuple(t) => t.len(),
            _ => return Err(RuntimeError::TypeMismatch("size requires a collection or string".into())),
        };
        Ok(Value::Bits(i64_to_bits(len as i64)))
    }

    fn eval_intrinsic_pop(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let mut list = match &values[0] {
            Value::List(l) => l.clone(),
            _ => return Err(RuntimeError::TypeMismatch("pop requires List".into())),
        };
        if list.is_empty() {
            Ok(Value::Bits(Vec::new()))
        } else {
            Ok(list.remove(0))
        }
    }

    fn eval_intrinsic_contains(&self, values: &[Value]) -> Result<Value, RuntimeError> {
        match (&values[0], &values[1]) {
            (Value::Bits(h), Value::Bits(n)) => {
                let h = String::from_utf8_lossy(h);
                let n = String::from_utf8_lossy(n);
                Ok(Value::Bits(vec![if h.contains(&*n) { 1u8 } else { 0u8 }]))
            }
            (Value::List(list), _) => {
                let found = list.contains(&values[1]);
                Ok(Value::Bits(vec![if found { 1u8 } else { 0u8 }]))
            }
            _ => Err(RuntimeError::TypeMismatch("contains requires String or List, String".into())),
        }
    }

    fn eval_intrinsic_keys(&self, values: &[Value]) -> Result<Value, RuntimeError> {
        match &values[0] {
            Value::HashMap(map) => {
                let keys: Vec<Value> = map.keys().map(|k| Value::Bits(k.as_bytes().to_vec())).collect();
                Ok(Value::List(keys))
            }
            other => Err(RuntimeError::TypeMismatch(format!("keys requires HashMap, got {:?}", other))),
        }
    }

    fn eval_intrinsic_values(&self, values: &[Value]) -> Result<Value, RuntimeError> {
        match &values[0] {
            Value::HashMap(map) => Ok(Value::List(map.values().cloned().collect())),
            other => Err(RuntimeError::TypeMismatch(format!("values requires HashMap, got {:?}", other))),
        }
    }

    fn eval_intrinsic_assert(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let cond = &values[0];
        let is_true = value_as_bool(cond).unwrap_or(false);
        if is_true {
            return Ok(Value::Bits(vec![1u8]));
        }
        let msg = values.get(1).map(|v| match v { Value::Bits(b) => String::from_utf8_lossy(b).to_string(), _ => "assertion failed".to_string() }).unwrap_or_else(|| "assertion failed".to_string());
        Err(RuntimeError::TypeMismatch(msg))
    }

    fn eval_intrinsic_append(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let mut list = match &values[0] { Value::List(l) => l.clone(), _ => return Err(RuntimeError::TypeMismatch("append requires List".into())) };
        list.push(values[1].clone());
        Ok(Value::List(list))
    }

    fn eval_intrinsic_prepend(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let mut list = match &values[0] { Value::List(l) => l.clone(), _ => return Err(RuntimeError::TypeMismatch("prepend requires List".into())) };
        list.insert(0, values[1].clone());
        Ok(Value::List(list))
    }

    fn eval_intrinsic_reverse(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let mut list = match &values[0] { Value::List(l) => l.clone(), _ => return Err(RuntimeError::TypeMismatch("reverse requires List".into())) };
        list.reverse();
        Ok(Value::List(list))
    }

    fn eval_intrinsic_sort(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let mut list = match &values[0] { Value::List(l) => l.clone(), _ => return Err(RuntimeError::TypeMismatch("sort requires List".into())) };
        list.sort_by(|a, b| match (value_as_i64(a), value_as_i64(b)) {
            (Some(ai), Some(bi)) => ai.cmp(&bi),
            _ => std::cmp::Ordering::Equal,
        });
        Ok(Value::List(list))
    }

    fn eval_intrinsic_slice(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let list = match &values[0] { Value::List(l) => l.clone(), _ => return Err(RuntimeError::TypeMismatch("slice requires List".into())) };
        let start = value_as_i64(&values[1]).unwrap_or(0) as usize;
        let end = value_as_i64(&values[2]).unwrap_or(list.len() as i64) as usize;
        let end = end.min(list.len());
        let start = start.min(end);
        Ok(Value::List(list[start..end].to_vec()))
    }

    fn eval_intrinsic_unique(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let list = match &values[0] { Value::List(l) => l.clone(), _ => return Err(RuntimeError::TypeMismatch("unique requires List".into())) };
        let mut seen = HashSet::new();
        let mut result = Vec::new();
        for item in list {
            if seen.insert(item.clone()) {
                result.push(item);
            }
        }
        Ok(Value::List(result))
    }

    fn eval_intrinsic_flatten(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let list = match &values[0] { Value::List(l) => l.clone(), _ => return Err(RuntimeError::TypeMismatch("flatten requires List".into())) };
        let mut result = Vec::new();
        for item in list {
            if let Value::List(inner) = item { result.extend(inner); } else { result.push(item); }
        }
        Ok(Value::List(result))
    }

    fn eval_intrinsic_filter(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let list = match &values[0] { Value::List(l) => l.clone(), _ => return Err(RuntimeError::TypeMismatch("filter requires List".into())) };
        Ok(Value::List(list))
    }

    fn eval_intrinsic_map(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let list = match &values[0] { Value::List(l) => l.clone(), _ => return Err(RuntimeError::TypeMismatch("map requires List".into())) };
        Ok(Value::List(list))
    }

    fn eval_intrinsic_reduce(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let list = match &values[0] { Value::List(l) => l.clone(), _ => return Err(RuntimeError::TypeMismatch("reduce requires List".into())) };
        let mut acc = values.get(1).cloned().unwrap_or(Value::Bits(i64_to_bits(0)));
        for item in list { acc = item; }
        Ok(acc)
    }

    fn eval_intrinsic_fold(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let list = match &values[0] { Value::List(l) => l.clone(), _ => return Err(RuntimeError::TypeMismatch("fold requires List".into())) };
        let mut acc = values.get(1).cloned().unwrap_or(Value::Bits(i64_to_bits(0)));
        for item in list { acc = item; }
        Ok(acc)
    }

    fn eval_intrinsic_group_by(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let list = match &values[0] { Value::List(l) => l.clone(), _ => return Err(RuntimeError::TypeMismatch("group_by requires List".into())) };
        let mut map: HashMap<String, Value> = HashMap::new();
        for item in list {
            let key = "0".to_string();
            let entry = map.entry(key).or_insert_with(|| Value::List(Vec::new()));
            if let Value::List(ref mut l) = entry { l.push(item); }
        }
        Ok(Value::HashMap(map))
    }

    fn eval_intrinsic_partition(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let list = match &values[0] { Value::List(l) => l.clone(), _ => return Err(RuntimeError::TypeMismatch("partition requires List".into())) };
        let mut trues = Vec::new();
        let mut falses = Vec::new();
        for item in list {
            if value_as_bool(&item).unwrap_or(false) { trues.push(item); } else { falses.push(item); }
        }
        Ok(Value::Tuple(vec![Value::List(trues), Value::List(falses)]))
    }

    // ── Ring Buffer ───────────────────────────────────────────────────

    fn eval_ring_push(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let name = match &values[0] { Value::Bits(b) => String::from_utf8_lossy(b).to_string(), _ => return Ok(Value::Bits(vec![1u8])) };
        let val = values[1].clone();
        let list = self.state.entry(name).or_insert_with(|| Value::List(Vec::new()));
        if let Value::List(ref mut items) = list { items.push(val); }
        Ok(Value::Bits(vec![1u8]))
    }

    fn eval_ring_pop(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let name = match &values[0] { Value::Bits(b) => String::from_utf8_lossy(b).to_string(), _ => return Ok(Value::Bits(Vec::new())) };
        if let Some(Value::List(ref mut items)) = self.state.get_mut(&name) {
            if !items.is_empty() { return Ok(items.remove(0)); }
        }
        Ok(Value::Bits(Vec::new()))
    }

    // ── Volatile Memory ───────────────────────────────────────────────

    fn eval_volatile_load(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let addr = value_as_i64(&values[0]).unwrap_or(0) as u64;
        Ok(self.virtual_heap.read(addr, 8).map(|b| Value::Bits(b.to_vec())).unwrap_or(Value::Bits(i64_to_bits(0))))
    }

    fn eval_volatile_store(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let addr = value_as_i64(&values[0]).unwrap_or(0) as u64;
        let data = match &values[1] { Value::Bits(b) => b.clone(), v => format!("{:?}", v).into_bytes() };
        self.virtual_heap.write(addr, &data).ok();
        Ok(Value::Bits(vec![1u8]))
    }

    // ── Virtual Heap ──────────────────────────────────────────────────

    fn eval_alloc(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let size = value_as_i64(&values[0]).unwrap_or(0);
        if size <= 0 { return Err(RuntimeError::TypeMismatch("alloc requires positive size".into())); }
        let addr = self.virtual_heap.alloc(&vec![0u8; size as usize]);
        Ok(Value::Bits(i64_to_bits(addr as i64)))
    }

    fn eval_free(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let addr = value_as_i64(&values[0]).unwrap_or(0);
        self.virtual_heap.free(addr as u64);
        Ok(Value::Bits(vec![1u8]))
    }

    fn eval_realloc(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let addr = value_as_i64(&values[0]).unwrap_or(0);
        self.virtual_heap.free(addr as u64);
        let new_addr = self.virtual_heap.alloc(&vec![0u8; 0]);
        Ok(Value::Bits(i64_to_bits(new_addr as i64)))
    }

    fn eval_memory_copy(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let dst = value_as_i64(&values[0]).unwrap_or(0) as u64;
        let src = value_as_i64(&values[1]).unwrap_or(0) as u64;
        let n = value_as_i64(&values[2]).unwrap_or(0) as u64;
        if let Some(src_data) = self.virtual_heap.read(src, n) {
            self.virtual_heap.write(dst, src_data).ok();
        }
        Ok(Value::Bits(vec![1u8]))
    }

    fn eval_memory_move(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let dst = value_as_i64(&values[0]).unwrap_or(0) as u64;
        let src = value_as_i64(&values[1]).unwrap_or(0) as u64;
        let n = value_as_i64(&values[2]).unwrap_or(0) as u64;
        if let Some(src_data) = self.virtual_heap.read(src, n) {
            let data = src_data.to_vec();
            self.virtual_heap.write(dst, &data).ok();
        }
        Ok(Value::Bits(vec![1u8]))
    }

    fn eval_memory_set(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let dst = value_as_i64(&values[0]).unwrap_or(0) as u64;
        let byte = value_as_i64(&values[1]).unwrap_or(0) as u8;
        let n = value_as_i64(&values[2]).unwrap_or(0) as u64;
        let data = vec![byte; n as usize];
        self.virtual_heap.write(dst, &data).ok();
        Ok(Value::Bits(vec![1u8]))
    }

    // ── Process / Env ─────────────────────────────────────────────────

    fn eval_set_env(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let name = match &values[0] { Value::Bits(b) => String::from_utf8_lossy(b).to_string(), _ => return Err(RuntimeError::TypeMismatch("setenv requires String name".into())) };
        let val = match &values[1] { Value::Bits(b) => String::from_utf8_lossy(b).to_string(), _ => return Err(RuntimeError::TypeMismatch("setenv requires String value".into())) };
        std::env::set_var(&name, &val);
        Ok(Value::Bits(vec![1u8]))
    }

    fn eval_spawn(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let cmd = match &values[0] { Value::Bits(b) => String::from_utf8_lossy(b).to_string(), _ => return Err(RuntimeError::TypeMismatch("spawn requires String command".into())) };
        let args_list: Vec<&str> = values[1..].iter().map(|v| match v { Value::Bits(b) => Box::leak(String::from_utf8_lossy(b).to_string().into_boxed_str()), _ => "", }).collect();
        match std::process::Command::new(&cmd).args(&args_list).spawn() {
            Ok(mut child) => {
                let pid = child.id() as i64;
                std::thread::spawn(move || { let _ = child.wait(); });
                Ok(Value::Bits(i64_to_bits(pid)))
            }
            Err(e) => Err(RuntimeError::TypeMismatch(format!("spawn failed: {}", e))),
        }
    }

    fn eval_spawn_with_output(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let cmd = match &values[0] { Value::Bits(b) => String::from_utf8_lossy(b).to_string(), _ => return Err(RuntimeError::TypeMismatch("spawn requires String command".into())) };
        let args_list: Vec<&str> = values[1..].iter().map(|v| match v { Value::Bits(b) => Box::leak(String::from_utf8_lossy(b).to_string().into_boxed_str()), _ => "", }).collect();
        let output = std::process::Command::new(&cmd).args(&args_list).output().map_err(|e| RuntimeError::TypeMismatch(format!("spawn failed: {}", e)))?;
        Ok(Value::Bits(output.stdout))
    }

    fn eval_set_pgid(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let pid = value_as_i64(&values[0]).unwrap_or(0) as i32;
        let pgid = value_as_i64(&values[1]).unwrap_or(0) as i32;
        let result = unsafe { libc::setpgid(pid, pgid) };
        Ok(Value::Bits(i64_to_bits(result as i64)))
    }

    // ── File I/O ──────────────────────────────────────────────────────

    fn eval_read_file(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let path = match &values[0] { Value::Bits(b) => String::from_utf8_lossy(b).to_string(), _ => return Err(RuntimeError::TypeMismatch("read_file requires String path".into())) };
        match std::fs::read_to_string(&path) {
            Ok(content) => Ok(Value::Bits(content.into_bytes())),
            Err(e) => Err(RuntimeError::TypeMismatch(format!("read_file failed: {}", e))),
        }
    }

    fn eval_write_file(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let path = match &values[0] { Value::Bits(b) => String::from_utf8_lossy(b).to_string(), _ => return Err(RuntimeError::TypeMismatch("write_file requires String path".into())) };
        let content = match &values[1] { Value::Bits(b) => String::from_utf8_lossy(b).to_string(), _ => return Err(RuntimeError::TypeMismatch("write_file requires String content".into())) };
        match std::fs::write(&path, &content) {
            Ok(_) => Ok(Value::Bits(b"OK".to_vec())),
            Err(e) => Err(RuntimeError::TypeMismatch(format!("write_file failed: {}", e))),
        }
    }

    fn eval_open(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let path = match &values[0] { Value::Bits(b) => String::from_utf8_lossy(b).to_string(), _ => return Err(RuntimeError::TypeMismatch("open requires String path".into())) };
        let flags = value_as_i64(&values[1]).unwrap_or(0) as i32;
        let c_path = std::ffi::CString::new(path).map_err(|_| RuntimeError::TypeMismatch("open: invalid path".into()))?;
        let fd = unsafe { libc::open(c_path.as_ptr(), flags) };
        Ok(Value::Bits(i64_to_bits(if fd >= 0 { fd as i64 } else { -1 })))
    }

    fn eval_read_fd(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let fd = value_as_i64(&values[0]).unwrap_or(-1) as i32;
        let count = value_as_i64(&values[1]).unwrap_or(0) as usize;
        if fd < 0 || count == 0 { return Ok(Value::Bits(Vec::new())); }
        let mut buf = vec![0u8; count];
        let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, count) };
        if n > 0 { buf.truncate(n as usize); Ok(Value::Bits(buf)) } else { Ok(Value::Bits(Vec::new())) }
    }

    fn eval_write_fd(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let fd = value_as_i64(&values[0]).unwrap_or(-1) as i32;
        let data = match &values[1] { Value::Bits(b) => b.clone(), _ => return Ok(Value::Bits(i64_to_bits(0))) };
        let n = unsafe { libc::write(fd, data.as_ptr() as *const libc::c_void, data.len()) };
        Ok(Value::Bits(i64_to_bits(n as i64)))
    }

    fn eval_pread(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let fd = value_as_i64(&values[0]).unwrap_or(-1) as i32;
        let count = value_as_i64(&values[1]).unwrap_or(0) as usize;
        let offset = value_as_i64(&values[2]).unwrap_or(0) as i64;
        if fd < 0 || count == 0 { return Ok(Value::Bits(Vec::new())); }
        let mut buf = vec![0u8; count];
        let n = unsafe { libc::pread(fd, buf.as_mut_ptr() as *mut libc::c_void, count, offset) };
        if n > 0 { buf.truncate(n as usize); Ok(Value::Bits(buf)) } else { Ok(Value::Bits(Vec::new())) }
    }

    fn eval_stat(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let path = match &values[0] { Value::Bits(b) => String::from_utf8_lossy(b).to_string(), _ => return Err(RuntimeError::TypeMismatch("stat requires String path".into())) };
        match std::fs::metadata(&path) {
            Ok(m) => {
                let mut fields = HashMap::new();
                fields.insert("size".into(), Value::Bits(i64_to_bits(m.len() as i64)));
                fields.insert("is_dir".into(), Value::Bits(vec![if m.is_dir() { 1u8 } else { 0u8 }]));
                fields.insert("is_file".into(), Value::Bits(vec![if m.is_file() { 1u8 } else { 0u8 }]));
                Ok(Value::Instance { typename: "FileStat".into(), fields })
            }
            Err(e) => Err(RuntimeError::TypeMismatch(format!("stat failed: {}", e))),
        }
    }

    fn eval_fstat(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let fd = value_as_i64(&values[0]).unwrap_or(-1) as i32;
        match unsafe { nix::sys::stat::fstat(fd) } {
            Ok(s) => {
                let mut fields = HashMap::new();
                fields.insert("size".into(), Value::Bits(i64_to_bits(s.st_size)));
                fields.insert("mode".into(), Value::Bits(i64_to_bits(s.st_mode as i64)));
                fields.insert("uid".into(), Value::Bits(i64_to_bits(s.st_uid as i64)));
                fields.insert("gid".into(), Value::Bits(i64_to_bits(s.st_gid as i64)));
                Ok(Value::Instance { typename: "FileStat".into(), fields })
            }
            Err(e) => Err(RuntimeError::TypeMismatch(format!("fstat failed: {:?}", e))),
        }
    }

    fn eval_mkdir(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let path = match &values[0] { Value::Bits(b) => String::from_utf8_lossy(b).to_string(), _ => return Err(RuntimeError::TypeMismatch("mkdir requires String path".into())) };
        match std::fs::create_dir_all(&path) {
            Ok(_) => Ok(Value::Bits(vec![1u8])),
            Err(e) => Ok(Value::Bits(format!("Error: {}", e).into_bytes())),
        }
    }

    fn eval_rmdir(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let path = match &values[0] { Value::Bits(b) => String::from_utf8_lossy(b).to_string(), _ => return Err(RuntimeError::TypeMismatch("rmdir requires String path".into())) };
        match std::fs::remove_dir_all(&path) {
            Ok(_) => Ok(Value::Bits(vec![1u8])),
            Err(e) => Ok(Value::Bits(format!("Error: {}", e).into_bytes())),
        }
    }

    fn eval_remove(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let path = match &values[0] { Value::Bits(b) => String::from_utf8_lossy(b).to_string(), _ => return Err(RuntimeError::TypeMismatch("remove requires String path".into())) };
        match std::fs::remove_file(&path) {
            Ok(_) => Ok(Value::Bits(vec![1u8])),
            Err(e) => Ok(Value::Bits(format!("Error: {}", e).into_bytes())),
        }
    }

    fn eval_rename(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let from = match &values[0] { Value::Bits(b) => String::from_utf8_lossy(b).to_string(), _ => return Err(RuntimeError::TypeMismatch("rename requires String from".into())) };
        let to = match &values[1] { Value::Bits(b) => String::from_utf8_lossy(b).to_string(), _ => return Err(RuntimeError::TypeMismatch("rename requires String to".into())) };
        match std::fs::rename(&from, &to) {
            Ok(_) => Ok(Value::Bits(vec![1u8])),
            Err(e) => Ok(Value::Bits(format!("Error: {}", e).into_bytes())),
        }
    }

    fn eval_read_dir(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let path = match &values[0] { Value::Bits(b) => String::from_utf8_lossy(b).to_string(), _ => return Err(RuntimeError::TypeMismatch("read_dir requires String path".into())) };
        match std::fs::read_dir(&path) {
            Ok(entries) => {
                let result: Vec<Value> = entries.flatten().map(|e| Value::Bits(e.file_name().to_string_lossy().to_string().into_bytes())).collect();
                Ok(Value::List(result))
            }
            Err(e) => Err(RuntimeError::TypeMismatch(format!("read_dir failed: {}", e))),
        }
    }

    fn eval_canonicalize(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let path = match &values[0] { Value::Bits(b) => String::from_utf8_lossy(b).to_string(), _ => return Err(RuntimeError::TypeMismatch("canonicalize requires String path".into())) };
        match std::fs::canonicalize(&path) {
            Ok(p) => Ok(Value::Bits(p.to_string_lossy().to_string().into_bytes())),
            Err(e) => Err(RuntimeError::TypeMismatch(format!("canonicalize failed: {}", e))),
        }
    }

    fn eval_chdir(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let path = match &values[0] { Value::Bits(b) => String::from_utf8_lossy(b).to_string(), _ => return Err(RuntimeError::TypeMismatch("chdir requires String path".into())) };
        let c_path = std::ffi::CString::new(path).map_err(|_| RuntimeError::TypeMismatch("chdir: invalid path".into()))?;
        let result = unsafe { libc::chdir(c_path.as_ptr()) };
        Ok(Value::Bits(i64_to_bits(result as i64)))
    }

    // ── Printf ────────────────────────────────────────────────────────

    fn eval_printf(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let fmt = match &values[0] { Value::Bits(b) => String::from_utf8_lossy(b).to_string(), _ => return Err(RuntimeError::TypeMismatch("printf requires String format".into())) };
        print!("{}", fmt);
        for arg in &values[1..] {
            match arg { Value::Bits(b) => print!(" {}", String::from_utf8_lossy(b)), v => print!(" {:?}", v) }
        }
        println!();
        Ok(Value::Bits(vec![1u8]))
    }

    // ── Unwrap / Expect ───────────────────────────────────────────────

    fn eval_unwrap(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        match &values[0] {
            Value::Enum(_, _, fields) => Ok(fields.into_values().next().unwrap_or(Value::Bits(Vec::new()))),
            _ => Err(RuntimeError::TypeMismatch("unwrap requires Option or Result".into())),
        }
    }

    fn eval_expect(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        match &values[0] {
            Value::Enum(_, _, fields) => Ok(fields.into_values().next().unwrap_or(Value::Bits(Vec::new()))),
            _ => Err(RuntimeError::TypeMismatch("expect requires Option or Result".into())),
        }
    }

    // ── DBVL ──────────────────────────────────────────────────────────

    fn eval_dbvl_load(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let path = match &values[0] { Value::Bits(b) => String::from_utf8_lossy(b).to_string(), _ => return Err(RuntimeError::TypeMismatch("dbvl_load requires String path".into())) };
        self.load_dbvl_table(&path, None)
    }

    fn eval_dbvl_lookup(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        let table = match &values[0] { Value::DbvlTable(t) => t.clone(), _ => return Err(RuntimeError::TypeMismatch("dbvl_lookup requires DbvlTable".into())) };
        let key = match &values[1] { Value::Bits(b) => String::from_utf8_lossy(b).to_string(), _ => return Err(RuntimeError::TypeMismatch("dbvl_lookup requires String key".into())) };
        let results = self.resolve_dbvl_key(&table, &key)?;
        Ok(if results.len() == 1 { results.into_iter().next().unwrap() } else { Value::List(results) })
    }

    fn eval_dbvl_filter(&mut self, values: &[Value]) -> Result<Value, RuntimeError> {
        Ok(Value::List(Vec::new()))
    }
}

// ── Free Helper Functions (no self) ───────────────────────────────────

/// Extract a bool from a Bits value (first byte).
fn bits_to_bool(v: &Value) -> Option<bool> {
    match v { Value::Bits(b) => Some(b.first().copied().unwrap_or(0) != 0), _ => None }
}

/// Apply a unary float operation with error handling.
fn eval_unary_float(values: &[Value], op: fn(f64) -> f64, name: &str) -> Result<Value, RuntimeError> {
    let v = &values[0];
    let b = match v { Value::Bits(b) => b, _ => return Err(RuntimeError::TypeMismatch(format!("{} requires Float, got {:?}", name, v))) };
    let f = bits_to_f64(&Value::Bits(b.clone()))?;
    Ok(Value::Bits(f64_to_bits(op(f))))
}

/// Apply a unary int operation with error handling.
fn eval_unary_int(values: &[Value], op: fn(i64) -> i64, name: &str) -> Result<Value, RuntimeError> {
    let v = &values[0];
    let n = value_as_i64(v).unwrap_or(0);
    Ok(Value::Bits(i64_to_bits(op(n))))
}

fn eval_str_bytes(values: &[Value]) -> Result<Value, RuntimeError> {
    match &values[0] { Value::Bits(b) => Ok(Value::Bits(b.clone())), _ => Ok(Value::Bits(Vec::new())) }
}

fn eval_str_len(values: &[Value]) -> Result<Value, RuntimeError> {
    let len = match &values[0] { Value::Bits(b) => b.len(), _ => 0 };
    Ok(Value::Bits(i64_to_bits(len as i64)))
}

fn eval_string_concat(values: &[Value]) -> Result<Value, RuntimeError> {
    match (&values[0], &values[1]) {
        (Value::Bits(a), Value::Bits(b)) => { let mut r = a.clone(); r.extend(b); Ok(Value::Bits(r)) }
        _ => Ok(Value::Bits(Vec::new())),
    }
}

fn eval_string_eq(values: &[Value]) -> Result<Value, RuntimeError> {
    let eq = match (&values[0], &values[1]) { (Value::Bits(a), Value::Bits(b)) => a == b, _ => false };
    Ok(Value::Bits(vec![if eq { 1u8 } else { 0u8 }]))
}

fn eval_string_find(values: &[Value]) -> Result<Value, RuntimeError> {
    let pos = match (&values[0], &values[1]) {
        (Value::Bits(h), Value::Bits(n)) => {
            let h = String::from_utf8_lossy(h);
            let n = String::from_utf8_lossy(n);
            h.find(&*n).map(|i| i as i64).unwrap_or(-1)
        }
        _ => -1,
    };
    Ok(Value::Bits(i64_to_bits(pos)))
}

fn eval_string_compare(values: &[Value]) -> Result<Value, RuntimeError> {
    let cmp = match (&values[0], &values[1]) {
        (Value::Bits(a), Value::Bits(b)) => {
            let a = String::from_utf8_lossy(a);
            let b = String::from_utf8_lossy(b);
            match a.cmp(&b) { std::cmp::Ordering::Less => -1, std::cmp::Ordering::Equal => 0, std::cmp::Ordering::Greater => 1 }
        }
        _ => 0,
    };
    Ok(Value::Bits(i64_to_bits(cmp)))
}

fn eval_string_to_int(values: &[Value]) -> Result<Value, RuntimeError> {
    let n = match &values[0] { Value::Bits(b) => String::from_utf8_lossy(b).parse::<i64>().unwrap_or(0), _ => 0 };
    Ok(Value::Bits(i64_to_bits(n)))
}

fn eval_int_to_string(values: &[Value]) -> Result<Value, RuntimeError> {
    let s = match &values[0] { Value::Bits(b) => value_as_i64(&Value::Bits(b.clone())).unwrap_or(0).to_string(), _ => "0".to_string() };
    Ok(Value::Bits(s.into_bytes()))
}

fn eval_string_to_float(values: &[Value]) -> Result<Value, RuntimeError> {
    let f = match &values[0] { Value::Bits(b) => String::from_utf8_lossy(b).parse::<f64>().unwrap_or(0.0), _ => 0.0 };
    Ok(Value::Bits(f64_to_bits(f)))
}

fn eval_float_to_string(values: &[Value]) -> Result<Value, RuntimeError> {
    let s = match &values[0] { Value::Bits(b) => bits_to_f64(&Value::Bits(b.clone())).unwrap_or(0.0).to_string(), _ => "0.0".to_string() };
    Ok(Value::Bits(s.into_bytes()))
}

fn eval_println(values: &[Value]) -> Result<Value, RuntimeError> {
    match &values[0] {
        Value::Bits(s) => println!("{}", String::from_utf8_lossy(s)),
        v => println!("{:?}", v),
    }
    Ok(Value::Bits(vec![1u8]))
}

fn eval_print(values: &[Value]) -> Result<Value, RuntimeError> {
    match &values[0] {
        Value::Bits(s) => print!("{}", String::from_utf8_lossy(s)),
        v => print!("{:?}", v),
    }
    Ok(Value::Bits(vec![1u8]))
}

fn eval_readln() -> Result<Value, RuntimeError> {
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok();
    Ok(Value::Bits(input.trim().to_string().into_bytes()))
}

fn eval_exit(values: &[Value]) -> Result<Value, RuntimeError> {
    let code = value_as_i64(&values[0]).unwrap_or(0);
    std::process::exit(code as i32);
}

fn eval_panic(values: &[Value]) -> Result<Value, RuntimeError> {
    let msg = match &values[0] { Value::Bits(b) => String::from_utf8_lossy(b).to_string(), _ => "panic triggered".to_string() };
    Err(RuntimeError::TypeMismatch(msg))
}

fn eval_time() -> Result<Value, RuntimeError> {
    use std::time::{SystemTime, UNIX_EPOCH};
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => Ok(Value::Bits(i64_to_bits(d.as_millis() as i64))),
        Err(_) => Ok(Value::Bits(i64_to_bits(0))),
    }
}

fn eval_realtime() -> Result<Value, RuntimeError> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let d = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    Ok(Value::Bits(i64_to_bits(d.as_nanos() as i64)))
}

fn eval_monotonic() -> Result<Value, RuntimeError> {
    use std::time::Instant;
    let d = Instant::now().duration_since(Instant::now());
    Ok(Value::Bits(i64_to_bits(d.as_nanos() as i64)))
}

fn eval_sleep(values: &[Value]) -> Result<Value, RuntimeError> {
    let ms = value_as_i64(&values[0]).unwrap_or(0);
    std::thread::sleep(std::time::Duration::from_millis(ms as u64));
    Ok(Value::Bits(vec![1u8]))
}

fn eval_nanosleep(values: &[Value]) -> Result<Value, RuntimeError> {
    let ns = value_as_i64(&values[0]).unwrap_or(0);
    if ns > 0 { unsafe { libc::nanosleep(&libc::timespec { tv_sec: 0, tv_nsec: ns as i64 }, std::ptr::null_mut()); } }
    Ok(Value::Bits(vec![1u8]))
}

fn eval_argv() -> Result<Value, RuntimeError> {
    let args: Vec<Value> = std::env::args().skip(1).map(|a| Value::Bits(a.into_bytes())).collect();
    Ok(Value::List(args))
}

fn eval_get_env(values: &[Value]) -> Result<Value, RuntimeError> {
    let name = match &values[0] { Value::Bits(b) => String::from_utf8_lossy(b).to_string(), _ => return Err(RuntimeError::TypeMismatch("getenv requires String name".into())) };
    match std::env::var(&name) {
        Ok(val) => Ok(Value::Bits(val.into_bytes())),
        Err(_) => Ok(Value::Bits(Vec::new())),
    }
}

fn eval_get_cwd() -> Result<Value, RuntimeError> {
    let mut buf = vec![0u8; 4096];
    let ptr = buf.as_mut_ptr() as *mut libc::c_char;
    let result = unsafe { libc::getcwd(ptr, 4096) };
    if result.is_null() { Ok(Value::Bits(Vec::new())) } else {
        let len = buf.iter().position(|&b| b == 0).unwrap_or(0);
        buf.truncate(len);
        Ok(Value::Bits(buf))
    }
}

fn eval_close(values: &[Value]) -> Result<Value, RuntimeError> {
    let fd = value_as_i64(&values[0]).unwrap_or(-1) as i32;
    if fd >= 0 { unsafe { libc::close(fd); } }
    Ok(Value::Bits(vec![1u8]))
}

fn eval_lseek(values: &[Value]) -> Result<Value, RuntimeError> {
    let fd = value_as_i64(&values[0]).unwrap_or(-1) as i32;
    let offset = value_as_i64(&values[1]).unwrap_or(0) as i64;
    let whence = value_as_i64(&values[2]).unwrap_or(0) as i32;
    let result = unsafe { libc::lseek(fd, offset, whence) };
    Ok(Value::Bits(i64_to_bits(result as i64)))
}

fn eval_pwrite(values: &[Value]) -> Result<Value, RuntimeError> {
    let fd = value_as_i64(&values[0]).unwrap_or(-1) as i32;
    let data = match &values[1] { Value::Bits(b) => b.clone(), _ => return Ok(Value::Bits(i64_to_bits(0))) };
    let offset = value_as_i64(&values[2]).unwrap_or(0) as i64;
    let n = unsafe { libc::pwrite(fd, data.as_ptr() as *const libc::c_void, data.len(), offset) };
    Ok(Value::Bits(i64_to_bits(n as i64)))
}

fn eval_ftruncate(values: &[Value]) -> Result<Value, RuntimeError> {
    let fd = value_as_i64(&values[0]).unwrap_or(-1) as i32;
    let len = value_as_i64(&values[1]).unwrap_or(0) as i64;
    let result = unsafe { libc::ftruncate(fd, len) };
    Ok(Value::Bits(i64_to_bits(result as i64)))
}

fn eval_fsync(values: &[Value]) -> Result<Value, RuntimeError> {
    let fd = value_as_i64(&values[0]).unwrap_or(-1) as i32;
    unsafe { libc::fsync(fd); }
    Ok(Value::Bits(vec![1u8]))
}

fn eval_fdup(values: &[Value]) -> Result<Value, RuntimeError> {
    let fd = value_as_i64(&values[0]).unwrap_or(-1) as i32;
    let new_fd = unsafe { libc::dup(fd) };
    Ok(Value::Bits(i64_to_bits(new_fd as i64)))
}

fn eval_fdup2(values: &[Value]) -> Result<Value, RuntimeError> {
    let old_fd = value_as_i64(&values[0]).unwrap_or(-1) as i32;
    let new_fd = value_as_i64(&values[1]).unwrap_or(-1) as i32;
    let result = unsafe { libc::dup2(old_fd, new_fd) };
    Ok(Value::Bits(i64_to_bits(result as i64)))
}

fn eval_fcntl(values: &[Value]) -> Result<Value, RuntimeError> {
    let fd = value_as_i64(&values[0]).unwrap_or(-1) as i32;
    let cmd = value_as_i64(&values[1]).unwrap_or(0) as i32;
    let result = unsafe { libc::fcntl(fd, cmd) };
    Ok(Value::Bits(i64_to_bits(result as i64)))
}

fn eval_pipe() -> Result<Value, RuntimeError> {
    let mut fds = [0i32; 2];
    let result = unsafe { libc::pipe(fds.as_mut_ptr()) };
    Ok(Value::List(vec![
        Value::Bits(i64_to_bits(if result == 0 { fds[0] as i64 } else { -1 })),
        Value::Bits(i64_to_bits(if result == 0 { fds[1] as i64 } else { -1 })),
    ]))
}

fn eval_access(values: &[Value]) -> Result<Value, RuntimeError> {
    let path = match &values[0] { Value::Bits(b) => std::ffi::CString::new(String::from_utf8_lossy(b).to_string()).ok(), _ => None };
    let mode = value_as_i64(&values[1]).unwrap_or(0) as i32;
    let result = match path { Some(p) => unsafe { libc::access(p.as_ptr(), mode) }, None => -1 };
    Ok(Value::Bits(i64_to_bits(result as i64)))
}

fn eval_umask(values: &[Value]) -> Result<Value, RuntimeError> {
    let mask = value_as_i64(&values[0]).unwrap_or(0o22) as u32;
    let old = unsafe { libc::umask(mask) };
    Ok(Value::Bits(i64_to_bits(old as i64)))
}

fn eval_mmap(values: &[Value]) -> Result<Value, RuntimeError> {
    let addr = value_as_i64(&values[0]).unwrap_or(0) as *mut libc::c_void;
    let length = value_as_i64(&values[1]).unwrap_or(0) as libc::size_t;
    let prot = value_as_i64(&values[2]).unwrap_or(0) as i32;
    let flags = value_as_i64(&values[3]).unwrap_or(0) as i32;
    let fd = value_as_i64(&values[4]).unwrap_or(-1) as i32;
    let offset = value_as_i64(&values[5]).unwrap_or(0) as i64;
    let result = unsafe { libc::mmap(addr, length, prot, flags, fd, offset) };
    Ok(Value::Bits(i64_to_bits(result as i64)))
}

fn eval_munmap(values: &[Value]) -> Result<Value, RuntimeError> {
    let addr = value_as_i64(&values[0]).unwrap_or(0) as *mut libc::c_void;
    let length = value_as_i64(&values[1]).unwrap_or(0) as libc::size_t;
    let result = unsafe { libc::munmap(addr, length) };
    Ok(Value::Bits(i64_to_bits(result as i64)))
}

fn eval_mprotect(values: &[Value]) -> Result<Value, RuntimeError> {
    let addr = value_as_i64(&values[0]).unwrap_or(0) as *mut libc::c_void;
    let length = value_as_i64(&values[1]).unwrap_or(0) as libc::size_t;
    let prot = value_as_i64(&values[2]).unwrap_or(0) as i32;
    let result = unsafe { libc::mprotect(addr, length, prot) };
    Ok(Value::Bits(i64_to_bits(result as i64)))
}

fn eval_madvise(values: &[Value]) -> Result<Value, RuntimeError> {
    let addr = value_as_i64(&values[0]).unwrap_or(0) as *mut libc::c_void;
    let length = value_as_i64(&values[1]).unwrap_or(0) as libc::size_t;
    let advice = value_as_i64(&values[2]).unwrap_or(0) as i32;
    let result = unsafe { libc::madvise(addr, length, advice) };
    Ok(Value::Bits(i64_to_bits(result as i64)))
}

fn eval_mlock(values: &[Value]) -> Result<Value, RuntimeError> {
    let addr = value_as_i64(&values[0]).unwrap_or(0) as *mut libc::c_void;
    let length = value_as_i64(&values[1]).unwrap_or(0) as libc::size_t;
    let result = unsafe { libc::mlock(addr, length) };
    Ok(Value::Bits(i64_to_bits(result as i64)))
}

fn eval_munlock(values: &[Value]) -> Result<Value, RuntimeError> {
    let addr = value_as_i64(&values[0]).unwrap_or(0) as *mut libc::c_void;
    let length = value_as_i64(&values[1]).unwrap_or(0) as libc::size_t;
    let result = unsafe { libc::munlock(addr, length) };
    Ok(Value::Bits(i64_to_bits(result as i64)))
}

fn eval_is_tty(values: &[Value]) -> Result<Value, RuntimeError> {
    let fd = value_as_i64(&values[0]).unwrap_or(0) as i32;
    #[cfg(unix)] { let is_tty = unsafe { libc::isatty(fd) != 0 }; Ok(Value::Bits(vec![if is_tty { 1u8 } else { 0u8 }])) }
    #[cfg(not(unix))] { let _ = fd; Ok(Value::Bits(vec![if fd == 0 || fd == 1 || fd == 2 { 1u8 } else { 0u8 }])) }
}

fn eval_sha256(values: &[Value]) -> Result<Value, RuntimeError> {
    let data = match &values[0] { Value::Bits(b) => b, _ => return Err(RuntimeError::TypeMismatch("sha256 requires Bits".into())) };
    let hash = ring::digest::digest(&ring::digest::SHA256, data);
    Ok(Value::Bits(hash.as_ref().to_vec()))
}

fn eval_sha512(values: &[Value]) -> Result<Value, RuntimeError> {
    let data = match &values[0] { Value::Bits(b) => b, _ => return Err(RuntimeError::TypeMismatch("sha512 requires Bits".into())) };
    let hash = ring::digest::digest(&ring::digest::SHA512, data);
    Ok(Value::Bits(hash.as_ref().to_vec()))
}

fn eval_md5(values: &[Value]) -> Result<Value, RuntimeError> {
    let data = match &values[0] { Value::Bits(b) => b, _ => return Err(RuntimeError::TypeMismatch("md5 requires Bits".into())) };
    let hash = ring::digest::digest(&ring::digest::MD5, data);
    Ok(Value::Bits(hash.as_ref().to_vec()))
}

fn eval_base64_encode(values: &[Value]) -> Result<Value, RuntimeError> {
    let data = match &values[0] { Value::Bits(b) => b, _ => return Err(RuntimeError::TypeMismatch("base64_encode requires Bits".into())) };
    use base64::Engine;
    Ok(Value::Bits(base64::engine::general_purpose::STANDARD.encode(data).into_bytes()))
}

fn eval_base64_decode(values: &[Value]) -> Result<Value, RuntimeError> {
    let s = match &values[0] { Value::Bits(b) => String::from_utf8_lossy(b).to_string(), _ => return Err(RuntimeError::TypeMismatch("base64_decode requires String".into())) };
    use base64::Engine;
    match base64::engine::general_purpose::STANDARD.decode(&s) {
        Ok(bytes) => Ok(Value::Bits(bytes)),
        Err(_) => Err(RuntimeError::TypeMismatch("base64_decode failed".into())),
    }
}

fn eval_hex_encode(values: &[Value]) -> Result<Value, RuntimeError> {
    let data = match &values[0] { Value::Bits(b) => b, _ => return Err(RuntimeError::TypeMismatch("hex_encode requires Bits".into())) };
    Ok(Value::Bits(hex::encode(data).into_bytes()))
}

fn eval_hex_decode(values: &[Value]) -> Result<Value, RuntimeError> {
    let s = match &values[0] { Value::Bits(b) => String::from_utf8_lossy(b).to_string(), _ => return Err(RuntimeError::TypeMismatch("hex_decode requires String".into())) };
    match hex::decode(&s) { Ok(bytes) => Ok(Value::Bits(bytes)), Err(_) => Err(RuntimeError::TypeMismatch("hex_decode failed".into())) }
}

fn eval_url_encode(values: &[Value]) -> Result<Value, RuntimeError> {
    let s = match &values[0] { Value::Bits(b) => String::from_utf8_lossy(b).to_string(), _ => return Err(RuntimeError::TypeMismatch("url_encode requires String".into())) };
    Ok(Value::Bits(urlencoding::encode(&s).into_owned().into_bytes()))
}

fn eval_url_decode(values: &[Value]) -> Result<Value, RuntimeError> {
    let s = match &values[0] { Value::Bits(b) => String::from_utf8_lossy(b).to_string(), _ => return Err(RuntimeError::TypeMismatch("url_decode requires String".into())) };
    Ok(Value::Bits(urlencoding::decode(&s).unwrap_or(std::borrow::Cow::Borrowed("")).into_owned().into_bytes()))
}

fn eval_format(values: &[Value]) -> Result<Value, RuntimeError> {
    let s = match &values[0] { Value::Bits(b) => String::from_utf8_lossy(b).to_string(), _ => format!("{:?}", &values[0]) };
    Ok(Value::Bits(s.into_bytes()))
}

fn eval_sprintf(values: &[Value]) -> Result<Value, RuntimeError> {
    let fmt = match &values[0] { Value::Bits(b) => String::from_utf8_lossy(b).to_string(), _ => return Err(RuntimeError::TypeMismatch("sprintf requires String format".into())) };
    Ok(Value::Bits(fmt.into_bytes()))
}

fn eval_type_id(values: &[Value]) -> Result<Value, RuntimeError> {
    let tid = match &values[0] {
        Value::Bits(_) => 1i64,
        Value::List(_) => 2i64,
        Value::HashMap(_) => 3i64,
        Value::Instance { typename, .. } => typename.bytes().fold(0i64, |hash, b| hash.wrapping_mul(31).wrapping_add(b as i64)),
        _ => 0,
    };
    Ok(Value::Bits(i64_to_bits(tid)))
}

fn eval_type_name(values: &[Value]) -> Result<Value, RuntimeError> {
    let name = match &values[0] {
        Value::Bits(_) => "Bits",
        Value::List(_) => "List",
        Value::HashMap(_) => "HashMap",
        Value::Instance { typename, .. } => typename.as_str(),
        _ => "Unknown",
    };
    Ok(Value::Bits(name.to_string().into_bytes()))
}

fn eval_sizeof(values: &[Value]) -> Result<Value, RuntimeError> {
    let len = match &values[0] {
        Value::Bits(b) => b.len(),
        Value::List(l) => l.len(),
        Value::HashMap(m) => m.len(),
        Value::Tuple(t) => t.len(),
        Value::Instance { fields, .. } => fields.len(),
        _ => 0,
    };
    Ok(Value::Bits(i64_to_bits(len as i64)))
}

fn eval_ptr_offset(values: &[Value]) -> Result<Value, RuntimeError> {
    let ptr = value_as_i64(&values[0]).unwrap_or(0);
    let offset = value_as_i64(&values[1]).unwrap_or(0);
    Ok(Value::Bits(i64_to_bits(ptr.wrapping_add(offset))))
}

fn eval_ptr_diff(values: &[Value]) -> Result<Value, RuntimeError> {
    let a = value_as_i64(&values[0]).unwrap_or(0);
    let b = value_as_i64(&values[1]).unwrap_or(0);
    Ok(Value::Bits(i64_to_bits(a.wrapping_sub(b))))
}

fn eval_intrinsic_length(values: &[Value]) -> Result<Value, RuntimeError> {
    let len = match &values[0] {
        Value::Bits(b) => b.len(),
        Value::List(l) => l.len(),
        Value::HashMap(m) => m.len(),
        Value::Tuple(t) => t.len(),
        _ => 0,
    };
    Ok(Value::Bits(i64_to_bits(len as i64)))
}

fn eval_intrinsic_is_empty(values: &[Value]) -> Result<Value, RuntimeError> {
    let empty = match &values[0] {
        Value::Bits(b) => b.is_empty(),
        Value::List(l) => l.is_empty(),
        Value::HashMap(m) => m.is_empty(),
        Value::Tuple(t) => t.is_empty(),
        _ => true,
    };
    Ok(Value::Bits(vec![if empty { 1u8 } else { 0u8 }]))
}

fn ok_first(values: &[Value]) -> Result<Value, RuntimeError> {
    match &values[0] { Value::List(l) => Ok(l.first().cloned().unwrap_or(Value::Bits(Vec::new()))), _ => Err(RuntimeError::TypeMismatch("first requires List".into())) }
}

fn ok_last(values: &[Value]) -> Result<Value, RuntimeError> {
    match &values[0] { Value::List(l) => Ok(l.last().cloned().unwrap_or(Value::Bits(Vec::new()))), _ => Err(RuntimeError::TypeMismatch("last requires List".into())) }
}

fn ok_nth(values: &[Value]) -> Result<Value, RuntimeError> {
    let n = value_as_i64(&values[1]).unwrap_or(0) as usize;
    match &values[0] { Value::List(l) => Ok(l.get(n).cloned().unwrap_or(Value::Bits(Vec::new()))), _ => Err(RuntimeError::TypeMismatch("nth requires List".into())) }
}

fn ok_take(values: &[Value]) -> Result<Value, RuntimeError> {
    let n = value_as_i64(&values[1]).unwrap_or(0) as usize;
    match &values[0] { Value::List(l) => Ok(Value::List(l.iter().take(n).cloned().collect())), _ => Err(RuntimeError::TypeMismatch("take requires List".into())) }
}

fn ok_drop(values: &[Value]) -> Result<Value, RuntimeError> {
    let n = value_as_i64(&values[1]).unwrap_or(0) as usize;
    match &values[0] { Value::List(l) => Ok(Value::List(l.iter().skip(n).cloned().collect())), _ => Err(RuntimeError::TypeMismatch("drop requires List".into())) }
}

fn eval_zip(values: &[Value]) -> Result<Value, RuntimeError> {
    match (&values[0], &values[1]) {
        (Value::List(a), Value::List(b)) => {
            let pairs: Vec<Value> = a.iter().zip(b.iter()).map(|(x, y)| Value::Tuple(vec![x.clone(), y.clone()])).collect();
            Ok(Value::List(pairs))
        }
        _ => Err(RuntimeError::TypeMismatch("zip requires List, List".into())),
    }
}

fn eval_enumerate(values: &[Value]) -> Result<Value, RuntimeError> {
    match &values[0] {
        Value::List(l) => {
            let enumerated: Vec<Value> = l.iter().enumerate().map(|(i, v)| Value::Tuple(vec![Value::Bits(i64_to_bits(i as i64)), v.clone()])).collect();
            Ok(Value::List(enumerated))
        }
        _ => Err(RuntimeError::TypeMismatch("enumerate requires List".into())),
    }
}

fn eval_chunks(values: &[Value]) -> Result<Value, RuntimeError> {
    let size = value_as_i64(&values[1]).unwrap_or(1) as usize;
    if size == 0 { return Err(RuntimeError::TypeMismatch("chunks requires positive size".into())); }
    match &values[0] { Value::List(l) => { let chunks: Vec<Value> = l.chunks(size).map(|c| Value::List(c.to_vec())).collect(); Ok(Value::List(chunks)) } _ => Err(RuntimeError::TypeMismatch("chunks requires List".into())) }
}

fn eval_windows(values: &[Value]) -> Result<Value, RuntimeError> {
    let size = value_as_i64(&values[1]).unwrap_or(1) as usize;
    if size == 0 { return Err(RuntimeError::TypeMismatch("windows requires positive size".into())); }
    match &values[0] { Value::List(l) => { let windows: Vec<Value> = l.windows(size).map(|w| Value::List(w.to_vec())).collect(); Ok(Value::List(windows)) } _ => Err(RuntimeError::TypeMismatch("windows requires List".into())) }
}

fn eval_all(values: &[Value]) -> Result<Value, RuntimeError> {
    match &values[0] { Value::List(l) => { let all_true = l.iter().all(|v| value_as_bool(v).unwrap_or(false)); Ok(Value::Bits(vec![if all_true { 1u8 } else { 0u8 }])) } _ => Err(RuntimeError::TypeMismatch("all requires List".into())) }
}

fn eval_any(values: &[Value]) -> Result<Value, RuntimeError> {
    match &values[0] { Value::List(l) => { let any_true = l.iter().any(|v| value_as_bool(v).unwrap_or(false)); Ok(Value::Bits(vec![if any_true { 1u8 } else { 0u8 }])) } _ => Err(RuntimeError::TypeMismatch("any requires List".into())) }
}

fn eval_count(values: &[Value]) -> Result<Value, RuntimeError> {
    match &values[0] { Value::List(l) => { let count = l.iter().filter(|v| *v == &values[1]).count() as i64; Ok(Value::Bits(i64_to_bits(count))) } _ => Err(RuntimeError::TypeMismatch("count requires List".into())) }
}

fn eval_index_of(values: &[Value]) -> Result<Value, RuntimeError> {
    match &values[0] { Value::List(l) => { let idx = l.iter().position(|v| v == &values[1]).map(|i| i as i64).unwrap_or(-1); Ok(Value::Bits(i64_to_bits(idx))) } _ => Err(RuntimeError::TypeMismatch("index_of requires List".into())) }
}

fn eval_range(values: &[Value]) -> Result<Value, RuntimeError> {
    let start = value_as_i64(&values[0]).unwrap_or(0);
    let end = value_as_i64(&values[1]).unwrap_or(0);
    let items: Vec<Value> = (start..end).map(|i| Value::Bits(i64_to_bits(i))).collect();
    Ok(Value::List(items))
}

fn eval_fill(values: &[Value]) -> Result<Value, RuntimeError> {
    let count = value_as_i64(&values[0]).unwrap_or(0);
    let items = vec![values[1].clone(); count as usize];
    Ok(Value::List(items))
}

fn eval_repeat(values: &[Value]) -> Result<Value, RuntimeError> {
    let count = value_as_i64(&values[1]).unwrap_or(0);
    if count.is_positive() { Ok(Value::List(vec![values[0].clone(); count as usize])) } else { Err(RuntimeError::TypeMismatch("repeat requires positive count".into())) }
}

fn eval_interleave(values: &[Value]) -> Result<Value, RuntimeError> {
    match (&values[0], &values[1]) {
        (Value::List(a), Value::List(b)) => {
            let mut result = Vec::new();
            let max_len = a.len().max(b.len());
            for i in 0..max_len { if i < a.len() { result.push(a[i].clone()); } if i < b.len() { result.push(b[i].clone()); } }
            Ok(Value::List(result))
        }
        _ => Err(RuntimeError::TypeMismatch("interleave requires List, List".into())),
    }
}

fn eval_find(values: &[Value]) -> Result<Value, RuntimeError> {
    match &values[0] { Value::List(l) => { let found = l.iter().find(|item| *item == &values[1]); Ok(found.cloned().unwrap_or(Value::Bits(Vec::new()))) } _ => Err(RuntimeError::TypeMismatch("find requires List".into())) }
}

fn ok_is_variant(values: &[Value], variant: &str) -> Result<Value, RuntimeError> {
    let is_it = matches!(&values[0], Value::Enum(_, name, _) if name == variant);
    Ok(Value::Bits(vec![if is_it { 1u8 } else { 0u8 }]))
}
