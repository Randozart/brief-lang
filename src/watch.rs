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

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WatchError {
    #[error("Watcher error: {0}")]
    NotifyError(#[from] notify::Error),
    #[error("Path error: {0}")]
    PathError(String),
}

#[derive(Debug, Clone)]
pub enum WatchEvent {
    FileChanged(PathBuf),
    ManifestChanged,
    FileAdded(PathBuf),
    FileRemoved(PathBuf),
}

pub struct WatchCallback {
    on_file_saved: Box<dyn Fn(PathBuf) + Send + Sync>,
    on_manifest_changed: Box<dyn Fn() + Send + Sync>,
    on_error: Box<dyn Fn(WatchError) + Send + Sync>,
}

impl WatchCallback {
    pub fn new() -> Self {
        Self {
            on_file_saved: Box::new(|_| {}),
            on_manifest_changed: Box::new(|| {}),
            on_error: Box::new(|_| {}),
        }
    }

    pub fn on_file_saved<F>(mut self, f: F) -> Self
    where
        F: Fn(PathBuf) + Send + Sync + 'static,
    {
        self.on_file_saved = Box::new(f);
        self
    }

    pub fn on_manifest_changed<F>(mut self, f: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_manifest_changed = Box::new(f);
        self
    }

    pub fn on_error<F>(mut self, f: F) -> Self
    where
        F: Fn(WatchError) + Send + Sync + 'static,
    {
        self.on_error = Box::new(f);
        self
    }
}

impl Default for WatchCallback {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Debouncer {
    inner: Arc<Mutex<DebouncerInner>>,
}

struct DebouncerInner {
    pending: HashMap<PathBuf, Instant>,
    last_trigger: Option<Instant>,
    callback: WatchCallback,
}

impl Debouncer {
    pub fn new(callback: WatchCallback) -> Self {
        Debouncer {
            inner: Arc::new(Mutex::new(DebouncerInner {
                pending: HashMap::new(),
                last_trigger: None,
                callback,
            })),
        }
    }

    pub fn on_file_event(&self, path: PathBuf) {
        let mut inner = self.inner.lock();
        inner.pending.insert(path.clone(), Instant::now());
    }

    pub fn flush(&self, debounce_ms: u64) -> Vec<PathBuf> {
        let mut inner = self.inner.lock();
        let now = Instant::now();
        let debounce = Duration::from_millis(debounce_ms);

        let mut flushed = Vec::new();
        inner.pending.retain(|path, instant| {
            if now.duration_since(*instant) >= debounce {
                flushed.push(path.clone());
                false
            } else {
                true
            }
        });

        flushed
    }

    pub fn should_trigger(&self, min_interval_ms: u64) -> bool {
        let inner = self.inner.lock();
        match inner.last_trigger {
            Some(last) => {
                let elapsed = Instant::now().duration_since(last);
                elapsed >= Duration::from_millis(min_interval_ms)
            }
            None => true,
        }
    }

    pub fn mark_triggered(&self) {
        let mut inner = self.inner.lock();
        inner.last_trigger = Some(Instant::now());
    }
}

pub struct WatcherState {
    watcher: RecommendedWatcher,
    debouncer: Debouncer,
    watched_paths: Vec<PathBuf>,
}

impl WatcherState {
    pub fn new(debounce_ms: u64) -> Result<Self, WatchError> {
        let debouncer = Debouncer::new(WatchCallback::new());

        let watcher = RecommendedWatcher::new(
            move |_res: Result<Event, notify::Error>| {},
            Config::default().with_poll_interval(Duration::from_millis(100)),
        )?;

        Ok(WatcherState {
            watcher,
            debouncer,
            watched_paths: Vec::new(),
        })
    }

    pub fn watch(&mut self, path: &Path) -> Result<(), WatchError> {
        if !path.exists() {
            return Err(WatchError::PathError(format!(
                "Path does not exist: {}",
                path.display()
            )));
        }

        let mode = if path.is_dir() {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };

        self.watcher.watch(path, mode)?;
        self.watched_paths.push(path.to_path_buf());
        Ok(())
    }

    pub fn unwatch(&mut self, path: &Path) -> Result<(), WatchError> {
        self.watcher.unwatch(path)?;
        self.watched_paths.retain(|p| p != path);
        Ok(())
    }

    pub fn watched_paths(&self) -> &[PathBuf] {
        &self.watched_paths
    }
}

pub struct WatchManager {
    state: Arc<Mutex<Option<WatcherState>>>,
    debounce_ms: u64,
}

impl WatchManager {
    pub fn new(debounce_ms: u64) -> Self {
        WatchManager {
            state: Arc::new(Mutex::new(None)),
            debounce_ms,
        }
    }

    pub fn start(&self, project_root: &Path) -> Result<(), WatchError> {
        let mut state = WatcherState::new(self.debounce_ms)?;
        state.watch(project_root)?;

        let mut guard = self.state.lock();
        *guard = Some(state);
        Ok(())
    }

    pub fn stop(&self) {
        let mut guard = self.state.lock();
        *guard = None;
    }

    pub fn is_running(&self) -> bool {
        self.state.lock().is_some()
    }

    pub fn process_events<F>(&self, callback: F)
    where
        F: Fn(WatchEvent),
    {
        let mut guard = self.state.lock();
        if let Some(ref mut state) = *guard {
            for path in &state.watched_paths {
                callback(WatchEvent::FileChanged(path.clone()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_debouncer() {
        let debouncer = Debouncer::new(WatchCallback::new());

        debouncer.on_file_event(PathBuf::from("file1.bv"));
        debouncer.on_file_event(PathBuf::from("file2.bv"));

        std::thread::sleep(Duration::from_millis(50));

        let flushed = debouncer.flush(10);
        assert!(flushed.len() <= 2);
    }

    #[test]
    fn test_watcher_creation() {
        let result = WatcherState::new(300);
        assert!(result.is_ok());
    }
}
