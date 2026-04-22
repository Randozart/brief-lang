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
//
// Runtime Exception for Use as a Language:
// When the Work or any Derivative Work thereof is used to generate code
// ("generated code"), such generated code shall not be subject to the
// terms of this License, provided that the generated code itself is not
// a Derivative Work of the Work. This exception does not apply to code
// that is itself a compiler, interpreter, or similar tool that incorporates
// or embeds the Work.

/// Adaptive Reactor Scheduler
///
/// Manages polling frequency for multiple reactive files:
/// - Global reactor runs at max(@Hz) across all files
/// - Each file with a slower speed is intelligently skipped
/// - Pure libraries (no rct blocks) consume zero overhead
use std::collections::HashMap;

/// Metadata about a file's reactor requirements
#[derive(Debug, Clone)]
pub struct FileSchedule {
    /// The file's polling speed in Hz
    pub speed_hz: u32,

    /// How often to check this file (calculated from global/local ratio)
    pub check_interval: u32,

    /// Counter for this file's checks
    pub check_counter: u64,
}

/// Manages adaptive scheduling across multiple files
#[derive(Debug)]
pub struct ReactorScheduler {
    /// Mapping of file ID to schedule
    files: HashMap<usize, FileSchedule>,

    /// Global reactor speed (maximum of all file speeds)
    global_speed_hz: u32,

    /// Current tick counter
    current_tick: u64,
}

impl ReactorScheduler {
    /// Create a new scheduler with default speed
    pub fn new() -> Self {
        ReactorScheduler {
            files: HashMap::new(),
            global_speed_hz: 10, // Default @10Hz
            current_tick: 0,
        }
    }

    /// Register a new file with its reactor speed
    pub fn register_file(&mut self, file_id: usize, speed_hz: Option<u32>) {
        let speed = speed_hz.unwrap_or(10); // Default @10Hz

        // Calculate check interval based on current global speed
        let check_interval = if speed > 0 && self.global_speed_hz > 0 {
            (self.global_speed_hz as f32 / speed as f32).ceil() as u32
        } else {
            u32::MAX // Never check
        };

        self.files.insert(
            file_id,
            FileSchedule {
                speed_hz: speed,
                check_interval,
                check_counter: 0,
            },
        );

        // Recalculate global speed
        self.recalculate_global_speed();
    }

    /// Recalculate global speed and update all intervals
    fn recalculate_global_speed(&mut self) {
        // Find max speed among all files
        if let Some(max_speed) = self.files.values().map(|f| f.speed_hz).max() {
            self.global_speed_hz = max_speed;

            // Update check intervals for all files
            for (_, schedule) in &mut self.files {
                schedule.check_interval = if schedule.speed_hz > 0 {
                    (self.global_speed_hz as f32 / schedule.speed_hz as f32).ceil() as u32
                } else {
                    u32::MAX
                };
            }
        }
    }

    /// Check if a file should have its preconditions evaluated this tick
    pub fn should_check_file(&self, file_id: usize) -> bool {
        if let Some(schedule) = self.files.get(&file_id) {
            if schedule.check_interval == u32::MAX {
                return false; // Never check (reactor inactive)
            }

            self.current_tick % schedule.check_interval as u64 == 0
        } else {
            false
        }
    }

    /// Advance to the next tick
    pub fn tick(&mut self) {
        self.current_tick += 1;
    }

    /// Get the global reactor speed in Hz
    pub fn global_speed_hz(&self) -> u32 {
        self.global_speed_hz
    }

    /// Get the number of registered files
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Get the schedule for a specific file
    pub fn get_schedule(&self, file_id: usize) -> Option<FileSchedule> {
        self.files.get(&file_id).cloned()
    }
}

impl Default for ReactorScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_speed() {
        let scheduler = ReactorScheduler::new();
        assert_eq!(scheduler.global_speed_hz(), 10);
    }

    #[test]
    fn test_single_file_at_10hz() {
        let mut scheduler = ReactorScheduler::new();
        scheduler.register_file(0, Some(10));

        assert_eq!(scheduler.global_speed_hz(), 10);
        assert!(scheduler.should_check_file(0)); // Every tick at global speed
    }

    #[test]
    fn test_multiple_files_different_speeds() {
        let mut scheduler = ReactorScheduler::new();
        scheduler.register_file(0, Some(10)); // Slow: every 6 ticks
        scheduler.register_file(1, Some(60)); // Fast: every tick

        // Global speed should be 60Hz (max)
        assert_eq!(scheduler.global_speed_hz(), 60);

        // At tick 0: both should check
        assert!(scheduler.should_check_file(0));
        assert!(scheduler.should_check_file(1));

        // At tick 1: only fast file checks
        scheduler.tick();
        assert!(!scheduler.should_check_file(0));
        assert!(scheduler.should_check_file(1));

        // At tick 6: slow file checks again
        for _ in 0..5 {
            scheduler.tick();
        }
        assert!(scheduler.should_check_file(0));
        assert!(scheduler.should_check_file(1));
    }

    #[test]
    fn test_pure_library_no_files() {
        let scheduler = ReactorScheduler::new();
        assert_eq!(scheduler.file_count(), 0);
        assert!(!scheduler.should_check_file(0));
    }

    #[test]
    fn test_file_without_reactor() {
        let mut scheduler = ReactorScheduler::new();
        // Register with speed 0 (no reactor)
        scheduler.register_file(0, Some(0));

        // Should never check
        for _ in 0..100 {
            assert!(!scheduler.should_check_file(0));
            scheduler.tick();
        }
    }

    #[test]
    fn test_intelligent_skipping_pattern() {
        let mut scheduler = ReactorScheduler::new();
        scheduler.register_file(0, Some(10)); // Check every 6 ticks at 60Hz global
        scheduler.register_file(1, Some(30)); // Check every 2 ticks at 60Hz global
        scheduler.register_file(2, Some(60)); // Check every tick at 60Hz global

        let mut checks_file0 = 0;
        let mut checks_file1 = 0;
        let mut checks_file2 = 0;

        // Run for 60 ticks (one cycle at 60Hz)
        for _ in 0..60 {
            if scheduler.should_check_file(0) {
                checks_file0 += 1;
            }
            if scheduler.should_check_file(1) {
                checks_file1 += 1;
            }
            if scheduler.should_check_file(2) {
                checks_file2 += 1;
            }
            scheduler.tick();
        }

        // Verify ratios match expected frequencies
        assert_eq!(checks_file0, 10); // 60Hz / 10Hz = 6 ticks apart → 10 checks per 60 ticks
        assert_eq!(checks_file1, 30); // 60Hz / 30Hz = 2 ticks apart → 30 checks per 60 ticks
        assert_eq!(checks_file2, 60); // 60Hz / 60Hz = 1 tick apart → 60 checks per 60 ticks
    }
}
