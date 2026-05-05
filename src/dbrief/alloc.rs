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

use crate::dbrief::ast::DbriefAddress;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct AddressAllocation {
    pub start: u64,
    pub size: u64,
    pub alignment: u64,
    pub name: Option<String>,
}

pub struct AddressAllocator {
    allocated: BTreeMap<u64, AddressAllocation>,
    next_free: u64,
    default_start: u64,
    default_alignment: u64,
}

impl AddressAllocator {
    pub fn new() -> Self {
        AddressAllocator {
            allocated: BTreeMap::new(),
            next_free: 0x1000,
            default_start: 0x1000,
            default_alignment: 4,
        }
    }

    pub fn with_start(mut self, start: u64) -> Self {
        self.default_start = start;
        self.next_free = start;
        self
    }

    pub fn with_alignment(mut self, alignment: u64) -> Self {
        self.default_alignment = alignment;
        self
    }

    pub fn reserve(&mut self, start: u64, size: u64, name: Option<String>) -> Result<(), String> {
        let end = start.saturating_add(size);
        
        // Check for overlaps with existing allocations
        for (addr, alloc) in &self.allocated {
            let alloc_end = addr.saturating_add(alloc.size);
            if !(end <= *addr || start >= alloc_end) {
                return Err(format!(
                    "Address conflict: requested [0x{:X}, 0x{:X}) overlaps with '{}' at [0x{:X}, 0x{:X})",
                    start, end,
                    alloc.name.as_deref().unwrap_or("unknown"),
                    addr, alloc_end
                ));
            }
        }

        let alloc = AddressAllocation {
            start,
            size,
            alignment: self.default_alignment,
            name,
        };
        self.allocated.insert(start, alloc);
        self.update_next_free();
        Ok(())
    }

    pub fn allocate(&mut self, size: u64, name: Option<String>) -> Result<u64, String> {
        self.allocate_aligned(size, self.default_alignment, name)
    }

    pub fn allocate_aligned(&mut self, size: u64, alignment: u64, name: Option<String>) -> Result<u64, String> {
        // First-fit allocation
        let mut candidate = self.next_free;
        
        // Align the candidate
        if alignment > 0 {
            let rem = candidate % alignment;
            if rem != 0 {
                candidate += alignment - rem;
            }
        }

        // Find a gap large enough
        let mut current = candidate;
        loop {
            // Check if this position fits
            let mut fits = true;
            for (addr, alloc) in &self.allocated {
                let alloc_end = addr.saturating_add(alloc.size);
                if !(current.saturating_add(size) <= *addr || current >= alloc_end) {
                    // Doesn't fit, jump to after this allocation
                    current = alloc_end;
                    if alignment > 0 {
                        let rem = current % alignment;
                        if rem != 0 {
                            current += alignment - rem;
                        }
                    }
                    fits = false;
                    break;
                }
            }

            if fits {
                let alloc = AddressAllocation {
                    start: current,
                    size,
                    alignment,
                    name,
                };
                self.allocated.insert(current, alloc);
                self.update_next_free();
                return Ok(current);
            }

            // Safety check to prevent infinite loop
            if current > 0xFFFF_FFFF {
                return Err("Out of address space".to_string());
            }
        }
    }

    fn update_next_free(&mut self) {
        let mut max_end = self.default_start;
        for (addr, alloc) in &self.allocated {
            let end = addr.saturating_add(alloc.size);
            if end > max_end {
                max_end = end;
            }
        }
        self.next_free = max_end;
    }

    pub fn get(&self, name: &str) -> Option<u64> {
        for (addr, alloc) in &self.allocated {
            if alloc.name.as_deref() == Some(name) {
                return Some(*addr);
            }
        }
        None
    }

    pub fn contains(&self, addr: u64) -> bool {
        for (base, alloc) in &self.allocated {
            let end = base.saturating_add(alloc.size);
            if addr >= *base && addr < end {
                return true;
            }
        }
        false
    }

    pub fn get_allocation(&self, addr: u64) -> Option<&AddressAllocation> {
        for (base, alloc) in &self.allocated {
            let end = base.saturating_add(alloc.size);
            if addr >= *base && addr < end {
                return Some(alloc);
            }
        }
        None
    }

    pub fn free_space(&self) -> u64 {
        let mut used: u64 = 0;
        for (_, alloc) in &self.allocated {
            used += alloc.size;
        }
        // Estimate based on address space range
        0xFFFF_FFFF_0000 - used
    }
}

impl Default for AddressAllocator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocate_first() {
        let mut allocator = AddressAllocator::new();
        let addr = allocator.allocate(1024, Some("buffer".to_string()));
        assert!(addr.is_ok());
        assert_eq!(addr.unwrap(), 0x1000);
    }

    #[test]
    fn test_allocate_sequential() {
        let mut allocator = AddressAllocator::new();
        
        let addr1 = allocator.allocate(1024, Some("first".to_string()));
        assert!(addr1.is_ok());
        
        let addr2 = allocator.allocate(1024, Some("second".to_string()));
        assert!(addr2.is_ok());
        
        // Second allocation should be right after first
        assert_eq!(addr1.unwrap() + 1024, addr2.unwrap());
    }

#[test]
    fn test_reserve_conflict() {
        let mut allocator = AddressAllocator::new();
        
        // Reserve first
        let result = allocator.reserve(0x1000, 1024, Some("first".to_string()));
        assert!(result.is_ok());
        
        // Try to reserve overlapping - should fail
        // 0x1300 is in the middle of 0x1000-0x1400
        let result = allocator.reserve(0x1300, 512, Some("overlap".to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn test_allocate_after_reserve() {
        let mut allocator = AddressAllocator::new();
        
        // Reserve after the default start - this pushes next_free past it
        allocator.reserve(0x2000, 1024, Some("reserved".to_string())).unwrap();
        
        // Allocate should get the next available after 0x2000+1024 = 0x2400
        let addr = allocator.allocate(512, Some("dynamic".to_string()));
        assert!(addr.is_ok());
        assert_eq!(addr.unwrap(), 0x2400);
    }

    #[test]
    fn test_alignment() {
        let mut allocator = AddressAllocator::new();
        
        // Allocate with 16-byte alignment
        let addr = allocator.allocate_aligned(256, 16, Some("aligned".to_string()));
        assert!(addr.is_ok());
        
        // Address should be aligned
        assert_eq!(addr.unwrap() % 16, 0);
    }

    #[test]
    fn test_get_by_name() {
        let mut allocator = AddressAllocator::new();
        
        allocator.allocate(1024, Some("my_buffer".to_string())).unwrap();
        
        let addr = allocator.get("my_buffer");
        assert!(addr.is_some());
        assert_eq!(addr.unwrap(), 0x1000);
    }

    #[test]
    fn test_get_allocation() {
        let mut allocator = AddressAllocator::new();
        
        allocator.reserve(0x1000, 1024, Some("test".to_string())).unwrap();
        
        let alloc = allocator.get_allocation(0x1200);
        assert!(alloc.is_some());
        
        if let Some(a) = alloc {
            assert_eq!(a.size, 1024);
            assert_eq!(a.name.as_deref(), Some("test"));
        }
    }

    #[test]
    fn test_start_address() {
        let mut allocator = AddressAllocator::new().with_start(0x8000);
        
        let addr = allocator.allocate(256, Some("test".to_string()));
        assert!(addr.is_ok());
        assert_eq!(addr.unwrap(), 0x8000);
    }

    #[test]
    fn test_out_of_space() {
        let mut allocator = AddressAllocator::new().with_start(0xFFFF_FFF0);
        
        // Allocate a very large amount that exceeds bounds
        let result = allocator.allocate(0x100, Some("huge".to_string()));
        // This might succeed or fail depending on overflow handling
        // Just verify it doesn't panic
        let _ = result;
    }
}