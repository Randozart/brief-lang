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

//! Metropolitan FFI - Shared Memory Negotiation System
//!
//! Implements zero-copy FFI through OS-level shared memory regions.
//! Brief negotiates memory layouts with foreign languages and treats
//! them as districts within the Brief metropolis.

use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// Cross-platform shared memory
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

/// Status word values for synchronization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum MetroStatus {
    Ready = 0,
    Processing = 1,
    Complete = 2,
    Error(u32) = 0x8000_0000_0000_0000,
}

impl MetroStatus {
    pub fn from_u64(val: u64) -> Self {
        if val & 0x8000_0000_0000_0000 != 0 {
            MetroStatus::Error(val as u32)
        } else {
            match val {
                0 => MetroStatus::Ready,
                1 => MetroStatus::Processing,
                2 => MetroStatus::Complete,
                _ => MetroStatus::Ready,
            }
        }
    }

    pub fn to_u64(&self) -> u64 {
        match self {
            MetroStatus::Ready => 0,
            MetroStatus::Processing => 1,
            MetroStatus::Complete => 2,
            MetroStatus::Error(code) => 0x8000_0000_0000_0000 | (*code as u64),
        }
    }
}

/// Shared memory region backed by OS-level mmap
pub struct SharedRegion {
    pub id: String,
    pub base_addr: usize,
    pub size: usize,
    pub permissions: String,
    pub foreign_id: String,
    #[cfg(unix)]
    pub mmap_ptr: *mut u8,
    #[cfg(windows)]
    pub handle: std::ptr::NonNull<std::ffi::c_void>,
    pub data: UnsafeCell<Vec<u8>>, // Fallback for testing without real mmap
}

unsafe impl Send for SharedRegion {}
unsafe impl Sync for SharedRegion {}

impl SharedRegion {
    /// Create a new shared region (real mmap on Unix/Windows, Vec fallback for tests)
    pub fn new(id: &str, size: usize, permissions: &str, foreign_lang: &str) -> Result<Self, String> {
        #[cfg(unix)]
        {
            Self::new_unix(id, size, permissions, foreign_lang)
        }
        #[cfg(windows)]
        {
            Self::new_windows(id, size, permissions, foreign_lang)
        }
        #[cfg(not(any(unix, windows)))]
        {
            Self::new_fallback(id, size, permissions, foreign_lang)
        }
    }

    #[cfg(unix)]
    fn new_unix(id: &str, size: usize, permissions: &str, foreign_lang: &str) -> Result<Self, String> {
        use libc::{mmap, munmap, PROT_READ, PROT_WRITE, MAP_ANONYMOUS, MAP_SHARED, MAP_FAILED};
        
        let prot = if permissions.contains('w') {
            PROT_READ | PROT_WRITE
        } else {
            PROT_READ
        };
        
        let ptr = unsafe {
            mmap(
                std::ptr::null_mut(),
                size,
                prot,
                MAP_ANONYMOUS | MAP_SHARED,
                -1,
                0,
            )
        };
        
        if ptr == MAP_FAILED {
            return Err(format!("mmap failed for region {}", id));
        }
        
        Ok(SharedRegion {
            id: id.to_string(),
            base_addr: ptr as usize,
            size,
            permissions: permissions.to_string(),
            foreign_id: format!("{}_{}", foreign_lang, id),
            mmap_ptr: ptr as *mut u8,
            #[cfg(windows)]
            handle: std::ptr::NonNull::dangling(),
            data: UnsafeCell::new(Vec::new()),
        })
    }

    #[cfg(windows)]
    fn new_windows(id: &str, size: usize, permissions: &str, foreign_lang: &str) -> Result<Self, String> {
        use windows::Win32::System::Memory::{
            VirtualAlloc, VirtualFree, MEM_COMMIT, MEM_RESERVE, MEM_RELEASE,
            PAGE_READWRITE, PAGE_READONLY,
        };
        
        let protect = if permissions.contains('w') {
            PAGE_READWRITE
        } else {
            PAGE_READONLY
        };
        
        let ptr = unsafe {
            VirtualAlloc(
                None,
                size,
                MEM_COMMIT | MEM_RESERVE,
                protect,
            )
        };
        
        if ptr.is_null() {
            return Err(format!("VirtualAlloc failed for region {}", id));
        }
        
        Ok(SharedRegion {
            id: id.to_string(),
            base_addr: ptr as usize,
            size,
            permissions: permissions.to_string(),
            foreign_id: format!("{}_{}", foreign_lang, id),
            #[cfg(unix)]
            mmap_ptr: std::ptr::null_mut(),
            handle: std::ptr::NonNull::new(ptr).unwrap(),
            data: UnsafeCell::new(Vec::new()),
        })
    }

    fn new_fallback(id: &str, size: usize, permissions: &str, foreign_lang: &str) -> Result<Self, String> {
        Ok(SharedRegion {
            id: id.to_string(),
            base_addr: 0,
            size,
            permissions: permissions.to_string(),
            foreign_id: format!("{}_{}", foreign_lang, id),
            #[cfg(unix)]
            mmap_ptr: std::ptr::null_mut(),
            #[cfg(windows)]
            handle: std::ptr::NonNull::dangling(),
            data: UnsafeCell::new(vec![0u8; size]),
        })
    }

    /// Write data to the shared region at offset
    pub fn write(&self, offset: usize, data: &[u8]) -> Result<(), String> {
        if offset + data.len() > self.size {
            return Err(format!("Write exceeds region bounds: {} + {} > {}", offset, data.len(), self.size));
        }
        
        #[cfg(unix)]
        {
            if !self.mmap_ptr.is_null() {
                unsafe {
                    std::ptr::copy_nonoverlapping(data.as_ptr(), self.mmap_ptr.add(offset), data.len());
                }
                return Ok(());
            }
        }
        
        #[cfg(windows)]
        {
            if !self.handle.is_null() {
                let ptr = self.handle.as_ptr() as *mut u8;
                unsafe {
                    std::ptr::copy_nonoverlapping(data.as_ptr(), ptr.add(offset), data.len());
                }
                return Ok(());
            }
        }
        
        // Fallback
        let region_data = unsafe { &mut *self.data.get() };
        region_data[offset..offset + data.len()].copy_from_slice(data);
        Ok(())
    }

    /// Read data from the shared region at offset
    pub fn read(&self, offset: usize, size: usize) -> Result<Vec<u8>, String> {
        if offset + size > self.size {
            return Err(format!("Read exceeds region bounds: {} + {} > {}", offset, size, self.size));
        }
        
        #[cfg(unix)]
        {
            if !self.mmap_ptr.is_null() {
                let mut buf = vec![0u8; size];
                unsafe {
                    std::ptr::copy_nonoverlapping(self.mmap_ptr.add(offset), buf.as_mut_ptr(), size);
                }
                return Ok(buf);
            }
        }
        
        #[cfg(windows)]
        {
            if !self.handle.is_null() {
                let ptr = self.handle.as_ptr() as *mut u8;
                let mut buf = vec![0u8; size];
                unsafe {
                    std::ptr::copy_nonoverlapping(ptr.add(offset), buf.as_mut_ptr(), size);
                }
                return Ok(buf);
            }
        }
        
        // Fallback
        let region_data = unsafe { &*self.data.get() };
        Ok(region_data[offset..offset + size].to_vec())
    }

    /// Read atomic status word
    pub fn read_status(&self, offset: usize) -> Result<MetroStatus, String> {
        if offset + 8 > self.size {
            return Err("Status offset out of bounds".to_string());
        }
        
        #[cfg(unix)]
        {
            if !self.mmap_ptr.is_null() {
                let atomic_ptr = unsafe { &*(self.mmap_ptr.add(offset) as *const AtomicU64) };
                return Ok(MetroStatus::from_u64(atomic_ptr.load(Ordering::SeqCst)));
            }
        }
        
        // Fallback
        let data = self.read(offset, 8)?;
        let val = u64::from_le_bytes(data.try_into().unwrap());
        Ok(MetroStatus::from_u64(val))
    }

    /// Write atomic status word
    pub fn write_status(&self, offset: usize, status: MetroStatus) -> Result<(), String> {
        if offset + 8 > self.size {
            return Err("Status offset out of bounds".to_string());
        }
        
        #[cfg(unix)]
        {
            if !self.mmap_ptr.is_null() {
                let atomic_ptr = unsafe { &*(self.mmap_ptr.add(offset) as *const AtomicU64) };
                atomic_ptr.store(status.to_u64(), Ordering::SeqCst);
                return Ok(());
            }
        }
        
        // Fallback
        let val = status.to_u64();
        let bytes = val.to_le_bytes();
        self.write(offset, &bytes)
    }

    /// Atomic compare-and-swap
    pub fn atomic_cas(&self, offset: usize, expected: u64, new_value: u64) -> Result<u64, String> {
        if offset + 8 > self.size {
            return Err("CAS offset out of bounds".to_string());
        }
        
        #[cfg(unix)]
        {
            if !self.mmap_ptr.is_null() {
                let atomic_ptr = unsafe { &*(self.mmap_ptr.add(offset) as *const AtomicU64) };
                return Ok(atomic_ptr.compare_exchange(expected, new_value, Ordering::SeqCst, Ordering::SeqCst).unwrap_or_else(|v| v));
            }
        }
        
        // Fallback (not truly atomic, but works for single-threaded tests)
        let current = self.read_status(offset)?.to_u64();
        if current == expected {
            self.write_status(offset, MetroStatus::from_u64(new_value))?;
        }
        Ok(current)
    }

    /// Memory barrier
    pub fn memory_barrier() {
        std::sync::atomic::fence(Ordering::SeqCst);
    }
}

impl Drop for SharedRegion {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            if !self.mmap_ptr.is_null() {
                unsafe {
                    libc::munmap(self.mmap_ptr as *mut _, self.size);
                }
            }
        }
        
        #[cfg(windows)]
        {
            use windows::Win32::System::Memory::VirtualFree;
            if !self.handle.is_null() {
                unsafe {
                    VirtualFree(self.handle.as_ptr(), 0, windows::Win32::System::Memory::MEM_RELEASE);
                }
            }
        }
    }
}

/// Metropolitan Channel - Three-region communication channel
pub struct MetropolitanChannel {
    pub id: String,
    pub request_region: Arc<SharedRegion>,
    pub response_region: Arc<SharedRegion>,
    pub sync_region: Arc<SharedRegion>,
    pub input_size: usize,
    pub output_size: usize,
}

/// Layout negotiation result
pub struct NegotiatedLayout {
    pub input_offset: usize,
    pub input_size: usize,
    pub output_offset: usize,
    pub output_size: usize,
    pub status_offset: usize,
}

impl MetropolitanChannel {
    /// Create a new Metropolitan channel with negotiated shared memory
    pub fn create(channel_id: &str, foreign_lang: &str, input_size: usize, output_size: usize) -> Result<Self, String> {
        // Calculate total sync region size: request_status + response_status + barrier
        let sync_size = 64;
        
        // Request three shared regions
        let request_region = Arc::new(SharedRegion::new(
            &format!("{}_request", channel_id),
            input_size,
            "rw",
            foreign_lang,
        )?);
        
        let response_region = Arc::new(SharedRegion::new(
            &format!("{}_response", channel_id),
            output_size,
            "rw",
            foreign_lang,
        )?);
        
        let sync_region = Arc::new(SharedRegion::new(
            &format!("{}_sync", channel_id),
            sync_size,
            "rw",
            foreign_lang,
        )?);
        
        // Initialize status words to Ready
        request_region.write_status(0, MetroStatus::Ready)?;
        response_region.write_status(0, MetroStatus::Ready)?;
        sync_region.write_status(0, MetroStatus::Ready)?;  // Request status at offset 0
        sync_region.write_status(8, MetroStatus::Ready)?;  // Response status at offset 8
        
        Ok(MetropolitanChannel {
            id: channel_id.to_string(),
            request_region,
            response_region,
            sync_region,
            input_size,
            output_size,
        })
    }

    /// Send data through the channel (zero-copy write to shared memory)
    pub fn send(&self, data: &[u8]) -> Result<(), String> {
        if data.len() > self.input_size {
            return Err(format!("Data size {} exceeds input buffer {}", data.len(), self.input_size));
        }
        
        // Write data to request region
        self.request_region.write(0, data)?;
        
        // Signal "request ready" via sync region
        self.sync_region.write_status(0, MetroStatus::Processing)?;
        
        // Memory barrier to ensure ordering
        SharedRegion::memory_barrier();
        
        Ok(())
    }

    /// Receive response from the channel with timeout
    pub fn receive(&self, timeout_ms: u64) -> Result<Vec<u8>, String> {
        let start = Instant::now();
        let timeout = Duration::from_millis(timeout_ms);
        
        while start.elapsed() < timeout {
            let status = self.sync_region.read_status(8)?;  // Response status at offset 8
            
            match status {
                MetroStatus::Complete => {
                    // Memory barrier before reading response
                    SharedRegion::memory_barrier();
                    
                    // Read response from response region
                    let response = self.response_region.read(0, self.output_size)?;
                    
                    // Reset response status to Ready
                    self.sync_region.write_status(8, MetroStatus::Ready)?;
                    
                    return Ok(response);
                }
                MetroStatus::Error(code) => {
                    return Err(format!("Foreign error code: {}", code));
                }
                _ => {
                    // Still processing, wait a bit
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
        }
        
        Err("Timeout waiting for response".to_string())
    }

    /// Get the negotiated memory layout for code generation
    pub fn get_layout(&self) -> NegotiatedLayout {
        NegotiatedLayout {
            input_offset: 0,
            input_size: self.input_size,
            output_offset: 0,
            output_size: self.output_size,
            status_offset: 0,  // Relative to sync_region
        }
    }

    /// Get the base addresses for foreign code generation
    pub fn get_addresses(&self) -> HashMap<String, usize> {
        let mut addrs = HashMap::new();
        addrs.insert("request_base".to_string(), self.request_region.base_addr);
        addrs.insert("response_base".to_string(), self.response_region.base_addr);
        addrs.insert("sync_base".to_string(), self.sync_region.base_addr);
        addrs.insert("input_size".to_string(), self.input_size);
        addrs.insert("output_size".to_string(), self.output_size);
        addrs
    }
}

/// Metropolitan Hub - Manages all channels
pub struct MetropolitanHub {
    channels: Mutex<HashMap<String, Arc<MetropolitanChannel>>>,
}

impl MetropolitanHub {
    pub fn new() -> Self {
        MetropolitanHub {
            channels: Mutex::new(HashMap::new()),
        }
    }

    /// Create and register a new channel
    pub fn create_channel(
        &self,
        channel_id: &str,
        foreign_lang: &str,
        input_size: usize,
        output_size: usize,
    ) -> Result<Arc<MetropolitanChannel>, String> {
        let channel = Arc::new(MetropolitanChannel::create(
            channel_id,
            foreign_lang,
            input_size,
            output_size,
        )?);
        
        self.channels.lock().unwrap().insert(channel_id.to_string(), channel.clone());
        Ok(channel)
    }

    /// Get an existing channel
    pub fn get_channel(&self, channel_id: &str) -> Option<Arc<MetropolitanChannel>> {
        self.channels.lock().unwrap().get(channel_id).cloned()
    }

    /// Remove and close a channel
    pub fn close_channel(&self, channel_id: &str) -> Result<(), String> {
        self.channels.lock().unwrap().remove(channel_id);
        Ok(())
    }

    /// Generate C header for foreign side
    pub fn generate_c_header(&self, channel_id: &str) -> Result<String, String> {
        let channel = self.get_channel(channel_id)
            .ok_or_else(|| format!("Channel {} not found", channel_id))?;
        
        let addrs = channel.get_addresses();
        
        Ok(format!(r#"
// Metropolitan FFI - C Header for channel: {channel_id}
// Generated by Brief Compiler
// DO NOT EDIT MANUALLY

#include <stdint.h>
#include <stdatomic.h>

// Shared memory regions (map these addresses)
#define REQUEST_BASE  ((volatile uint8_t*)0x{req:x})
#define RESPONSE_BASE ((volatile uint8_t*)0x{resp:x})
#define SYNC_BASE     ((volatile uint8_t*)0x{sync:x})

#define INPUT_SIZE  {input}
#define OUTPUT_SIZE {output}

// Status word offsets in sync region
#define REQUEST_STATUS  ((atomic_uint64_t*)&SYNC_BASE[0])
#define RESPONSE_STATUS ((atomic_uint64_t*)&SYNC_BASE[8])

// Status values
#define STATUS_READY     0
#define STATUS_PROCESSING 1
#define STATUS_COMPLETE  2
#define STATUS_ERROR     0x8000000000000000ULL

// Wait for Brief to send a request
static inline void wait_for_request() {{
    while (atomic_load_explicit(REQUEST_STATUS, memory_order_seq_cst) != STATUS_PROCESSING) {{
        // Spin wait
    }}
}}

// Signal that response is ready
static inline void signal_complete() {{
    atomic_store_explicit(RESPONSE_STATUS, STATUS_COMPLETE, memory_order_seq_cst);
}}

// Signal error
static inline void signal_error(uint32_t code) {{
    atomic_store_explicit(RESPONSE_STATUS, STATUS_ERROR | code, memory_order_seq_cst);
}}

// Reset request status after processing
static inline void reset_request() {{
    atomic_store_explicit(REQUEST_STATUS, STATUS_READY, memory_order_seq_cst);
}}
"#,
            channel_id = channel_id,
            req = addrs.get("request_base").unwrap_or(&0),
            resp = addrs.get("response_base").unwrap_or(&0),
            sync = addrs.get("sync_base").unwrap_or(&0),
            input = addrs.get("input_size").unwrap_or(&0),
            output = addrs.get("output_size").unwrap_or(&0),
        ))
    }

    /// Generate Rust module for foreign side
    pub fn generate_rust_module(&self, channel_id: &str) -> Result<String, String> {
        let channel = self.get_channel(channel_id)
            .ok_or_else(|| format!("Channel {} not found", channel_id))?;
        
        let addrs = channel.get_addresses();
        
        Ok(format!(r#"
// Metropolitan FFI - Rust Module for channel: {channel_id}
// Generated by Brief Compiler
// DO NOT EDIT MANUALLY

use std::sync::atomic::{{AtomicU64, Ordering}};
use std::slice;

// Shared memory regions (map these addresses via mmap or similar)
const REQUEST_BASE: usize = 0x{req:x};
const RESPONSE_BASE: usize = 0x{resp:x};
const SYNC_BASE: usize = 0x{sync:x};

const INPUT_SIZE: usize = {input};
const OUTPUT_SIZE: usize = {output};

// Status word values
const STATUS_READY: u64 = 0;
const STATUS_PROCESSING: u64 = 1;
const STATUS_COMPLETE: u64 = 2;
const STATUS_ERROR: u64 = 0x8000_0000_0000_0000;

/// Wait for Brief to send a request
pub fn wait_for_request() {{
    let request_status = unsafe {{ &*(SYNC_BASE as *const AtomicU64) }};
    while request_status.load(Ordering::SeqCst) != STATUS_PROCESSING {{
        std::hint::spin_loop();
    }}
}}

/// Signal that response is ready
pub fn signal_complete() {{
    let response_status = unsafe {{ &*((SYNC_BASE + 8) as *const AtomicU64) }};
    response_status.store(STATUS_COMPLETE, Ordering::SeqCst);
}}

/// Signal error
pub fn signal_error(code: u32) {{
    let response_status = unsafe {{ &*((SYNC_BASE + 8) as *const AtomicU64) }};
    response_status.store(STATUS_ERROR | (code as u64), Ordering::SeqCst);
}}

/// Get input data slice
pub fn get_input<'a>() -> &'a [u8] {{
    unsafe {{ slice::from_raw_parts(REQUEST_BASE as *const u8, INPUT_SIZE) }}
}}

/// Get output data slice (mutable)
pub fn get_output<'a>() -> &'a mut [u8] {{
    unsafe {{ slice::from_raw_parts_mut(RESPONSE_BASE as *mut u8, OUTPUT_SIZE) }}
}}

/// Reset request status after processing
pub fn reset_request() {{
    let request_status = unsafe {{ &*(SYNC_BASE as *const AtomicU64) }};
    request_status.store(STATUS_READY, Ordering::SeqCst);
}}
"#,
            channel_id = channel_id,
            req = addrs.get("request_base").unwrap_or(&0),
            resp = addrs.get("response_base").unwrap_or(&0),
            sync = addrs.get("sync_base").unwrap_or(&0),
            input = addrs.get("input_size").unwrap_or(&0),
            output = addrs.get("output_size").unwrap_or(&0),
        ))
    }

    /// Generate Python module for foreign side
    pub fn generate_python_module(&self, channel_id: &str) -> Result<String, String> {
        let channel = self.get_channel(channel_id)
            .ok_or_else(|| format!("Channel {} not found", channel_id))?;
        
        let addrs = channel.get_addresses();
        
        Ok(format!(r#"
# Metropolitan FFI - Python Module for channel: {channel_id}
# Generated by Brief Compiler
# DO NOT EDIT MANUALLY

import mmap
import ctypes
import time

# Shared memory region addresses
REQUEST_BASE = 0x{req:x}
RESPONSE_BASE = 0x{resp:x}
SYNC_BASE = 0x{sync:x}

INPUT_SIZE = {input}
OUTPUT_SIZE = {output}

# Status values
STATUS_READY = 0
STATUS_PROCESSING = 1
STATUS_COMPLETE = 2
STATUS_ERROR = 0x8000000000000000

# Map shared memory regions (requires actual mmap setup)
def map_region(address, size):
    # In practice, use mmap with the actual file descriptor
    # This is a placeholder for the concept
    return mmap.mmap(-1, size)

# Status word access
def read_request_status():
    # Read atomic uint64 from SYNC_BASE
    return ctypes.c_uint64.from_address(SYNC_BASE).value

def write_response_status(value):
    ctypes.c_uint64.from_address(SYNC_BASE + 8).value = value

def wait_for_request():
    while read_request_status() != STATUS_PROCESSING:
        time.sleep(0.001)

def signal_complete():
    write_response_status(STATUS_COMPLETE)

def signal_error(code):
    write_response_status(STATUS_ERROR | code)

def get_input():
    # Read from REQUEST_BASE
    pass

def get_output():
    # Write to RESPONSE_BASE
    pass

def reset_request():
    ctypes.c_uint64.from_address(SYNC_BASE).value = STATUS_READY
"#,
            channel_id = channel_id,
            req = addrs.get("request_base").unwrap_or(&0),
            resp = addrs.get("response_base").unwrap_or(&0),
            sync = addrs.get("sync_base").unwrap_or(&0),
            input = addrs.get("input_size").unwrap_or(&0),
            output = addrs.get("output_size").unwrap_or(&0),
        ))
    }
}

impl Default for MetropolitanHub {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shared_region_create_and_write() {
        let region = SharedRegion::new("test", 1024, "rw", "c").unwrap();
        assert_eq!(region.size, 1024);
        assert_eq!(region.id, "test");
        
        region.write(0, &[1, 2, 3, 4]).unwrap();
        let data = region.read(0, 4).unwrap();
        assert_eq!(data, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_status_word_atomic() {
        let region = SharedRegion::new("test_status", 64, "rw", "c").unwrap();
        
        region.write_status(0, MetroStatus::Ready).unwrap();
        assert_eq!(region.read_status(0).unwrap(), MetroStatus::Ready);
        
        region.write_status(0, MetroStatus::Processing).unwrap();
        assert_eq!(region.read_status(0).unwrap(), MetroStatus::Processing);
        
        region.write_status(0, MetroStatus::Complete).unwrap();
        assert_eq!(region.read_status(0).unwrap(), MetroStatus::Complete);
        
        region.write_status(0, MetroStatus::Error(42)).unwrap();
        assert_eq!(region.read_status(0).unwrap(), MetroStatus::Error(42));
    }

    #[test]
    fn test_atomic_cas() {
        let region = SharedRegion::new("test_cas", 64, "rw", "c").unwrap();
        region.write_status(0, MetroStatus::Ready).unwrap();
        
        let result = region.atomic_cas(0, 0, 1).unwrap();
        assert_eq!(result, 0);  // Returned old value
        
        let status = region.read_status(0).unwrap();
        assert_eq!(status, MetroStatus::Processing);
    }

    #[test]
    fn test_metropolitan_channel() {
        let hub = MetropolitanHub::new();
        let channel = hub.create_channel("test_channel", "c", 4096, 4096).unwrap();
        
        assert_eq!(channel.id, "test_channel");
        assert_eq!(channel.input_size, 4096);
        assert_eq!(channel.output_size, 4096);
    }

    #[test]
    fn test_channel_send_receive() {
        let hub = MetropolitanHub::new();
        let channel = hub.create_channel("test_send_recv", "c", 4096, 4096).unwrap();
        
        // Send data
        let input = vec![1, 2, 3, 4, 5];
        channel.send(&input).unwrap();
        
        // Verify data was written to request region
        let request_data = channel.request_region.read(0, 5).unwrap();
        assert_eq!(request_data, input);
        
        // Verify status was set to Processing
        let status = channel.sync_region.read_status(0).unwrap();
        assert_eq!(status, MetroStatus::Processing);
        
        // Simulate foreign side processing
        channel.response_region.write(0, &[10, 20, 30]).unwrap();
        channel.sync_region.write_status(8, MetroStatus::Complete).unwrap();
        
        // Receive response
        let response = channel.receive(1000).unwrap();
        assert_eq!(&response[..3], &[10, 20, 30]);
    }

    #[test]
    fn test_channel_timeout() {
        let hub = MetropolitanHub::new();
        let channel = hub.create_channel("test_timeout", "c", 4096, 4096).unwrap();
        
        // Send without foreign side responding
        channel.send(&[1, 2, 3]).unwrap();
        
        // Should timeout
        let result = channel.receive(10);  // 10ms timeout
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Timeout"));
    }

    #[test]
    fn test_generate_c_header() {
        let hub = MetropolitanHub::new();
        hub.create_channel("test_c", "c", 1024, 1024).unwrap();
        
        let header = hub.generate_c_header("test_c").unwrap();
        assert!(header.contains("REQUEST_BASE"));
        assert!(header.contains("RESPONSE_BASE"));
        assert!(header.contains("SYNC_BASE"));
        assert!(header.contains("STATUS_READY"));
    }

    #[test]
    fn test_generate_rust_module() {
        let hub = MetropolitanHub::new();
        hub.create_channel("test_rust", "rust", 2048, 2048).unwrap();
        
        let module = hub.generate_rust_module("test_rust").unwrap();
        assert!(module.contains("REQUEST_BASE"));
        assert!(module.contains("wait_for_request"));
        assert!(module.contains("signal_complete"));
    }

    #[test]
    fn test_generate_python_module() {
        let hub = MetropolitanHub::new();
        hub.create_channel("test_python", "python", 512, 512).unwrap();
        
        let module = hub.generate_python_module("test_python").unwrap();
        assert!(module.contains("REQUEST_BASE"));
        assert!(module.contains("wait_for_request"));
        assert!(module.contains("signal_complete"));
    }
}
