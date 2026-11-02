//! Supervised `adb logcat` capture that reconnects when the TV drops.
//!
//! A TV that sleeps, reboots, or changes address closes the logcat pipe.
//! Instead of letting capture die there, the reader is a supervisor loop:
//! it reaps the dead child, reconnects, and starts a new reader.

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{Context, Result};

use crate::adb::AdbDevice;

use super::{parse_key_line, KeyEntry};

/// How long to wait before respawning `adb logcat` after the TV drops.
const RETRY_DELAY: Duration = Duration::from_secs(3);
/// Granularity of the interruptible retry wait.
const RETRY_STEP: Duration = Duration::from_millis(100);

/// State shared between the logger handle and its supervisor thread.
struct Shared {
    /// Live logcat child, kept so the handle can kill it.
    child: Mutex<Option<Child>>,
    /// Supervisor is alive and will keep retrying.
    running: AtomicBool,
    /// A logcat child is currently streaming.
    capturing: AtomicBool,
}

/// Date-stamped append writer that rolls over when the TV clock changes day.
struct DailyWriter {
    dir: PathBuf,
    date: String,
    file: Option<File>,
}

impl DailyWriter {
    fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            date: String::new(),
            file: None,
        }
    }

    fn write(&mut self, entry: &KeyEntry) -> Result<()> {
        if entry.date != self.date {
            fs::create_dir_all(&self.dir)
                .with_context(|| format!("Failed to create {}", self.dir.display()))?;
            let path = self.dir.join(format!("{}.log", entry.date));
            self.file = Some(
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)
                    .with_context(|| format!("Failed to open {}", path.display()))?,
            );
            self.date.clone_from(&entry.date);
        }
        if let Some(file) = self.file.as_mut() {
            writeln!(file, "{}", entry.to_line())?;
        }
        Ok(())
    }
}

/// Background key-capture supervisor. Dropping it stops capture and kills the child.
pub struct InputLogger {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl InputLogger {
    /// Start supervised `adb logcat` capture into `dir`.
    pub fn start(adb: &AdbDevice, dir: &Path) -> Result<Self> {
        adb.ensure_connected()?;
        fs::create_dir_all(dir).with_context(|| format!("Failed to create {}", dir.display()))?;

        let shared = Arc::new(Shared {
            child: Mutex::new(None),
            running: AtomicBool::new(true),
            capturing: AtomicBool::new(false),
        });

        let thread = {
            let adb = adb.clone();
            let dir = dir.to_path_buf();
            let shared = Arc::clone(&shared);
            thread::spawn(move || supervise(&adb, &dir, &shared))
        };

        Ok(Self {
            shared,
            thread: Some(thread),
        })
    }

    /// Whether the supervisor is alive (it may still be reconnecting).
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.shared.running.load(Ordering::Relaxed)
    }

    /// Whether a logcat child is currently streaming key events.
    #[must_use]
    pub fn is_capturing(&self) -> bool {
        self.shared.capturing.load(Ordering::Relaxed)
    }

    /// Stop the capture and join the supervisor thread.
    pub fn stop(&mut self) {
        self.shared.running.store(false, Ordering::Relaxed);
        reap_child(&self.shared);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for InputLogger {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Reconnect-and-stream loop. Exits only when `running` is cleared.
fn supervise(adb: &AdbDevice, dir: &Path, shared: &Shared) {
    let mut writer = DailyWriter::new(dir.to_path_buf());
    while shared.running.load(Ordering::Relaxed) {
        let _ = stream_once(adb, shared, &mut writer);
        shared.capturing.store(false, Ordering::Relaxed);
        wait_or_stop(shared, RETRY_DELAY);
    }
}

/// Connect, spawn one logcat reader, consume its lines until it ends.
fn stream_once(adb: &AdbDevice, shared: &Shared, writer: &mut DailyWriter) -> Result<()> {
    adb.ensure_connected()?;
    let target = adb.target();
    let mut child = spawn_logcat(&target)?;
    let stdout = child
        .stdout
        .take()
        .context("logcat produced no stdout pipe")?;

    match shared.child.lock() {
        Ok(mut guard) => *guard = Some(child),
        Err(_) => anyhow::bail!("logger state lock poisoned"),
    }
    shared.capturing.store(true, Ordering::Relaxed);

    let mut failure = None;
    for line in BufReader::new(stdout).lines() {
        if !shared.running.load(Ordering::Relaxed) {
            break;
        }
        let Ok(line) = line else { break };
        if let Some(entry) = parse_key_line(&line) {
            if let Err(e) = writer.write(&entry) {
                failure = Some(e);
                break;
            }
        }
    }

    reap_child(shared);
    match failure {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// Kill and reap the current child so it cannot linger as a zombie.
fn reap_child(shared: &Shared) {
    if let Ok(mut guard) = shared.child.lock() {
        if let Some(mut child) = guard.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn spawn_logcat(target: &str) -> Result<Child> {
    Command::new("adb")
        .args([
            "-s",
            target,
            "logcat",
            "-v",
            "year",
            "threadtime",
            "-T",
            "1",
            "-s",
            "WindowManager",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("Failed to start adb logcat")
}

/// Sleep for `total`, waking early if the supervisor is stopped.
fn wait_or_stop(shared: &Shared, total: Duration) {
    let mut left = total;
    while left > Duration::ZERO && shared.running.load(Ordering::Relaxed) {
        let step = left.min(RETRY_STEP);
        thread::sleep(step);
        left -= step;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(date: &str, time: &str, key: &str, action: &'static str) -> KeyEntry {
        KeyEntry {
            date: date.to_string(),
            time: time.to_string(),
            key: key.to_string(),
            action,
        }
    }

    fn temp_dir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("terebi-{tag}-{}", std::process::id()))
    }

    #[test]
    fn test_daily_writer_rotates_across_dates() -> Result<()> {
        let dir = temp_dir("rotate");
        let _ = fs::remove_dir_all(&dir);
        let mut writer = DailyWriter::new(dir.clone());
        for date in ["2026-11-01", "2026-11-01", "2026-11-02"] {
            writer.write(&sample(date, "10:00:00.000", "KEYCODE_HOME", "down"))?;
        }
        assert_eq!(
            super::super::tail(&dir.join("2026-11-01.log"), 10)?.len(),
            2
        );
        assert_eq!(
            super::super::tail(&dir.join("2026-11-02.log"), 10)?.len(),
            1
        );
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn test_wait_or_stop_returns_after_total() {
        let shared = Shared {
            child: Mutex::new(None),
            running: AtomicBool::new(true),
            capturing: AtomicBool::new(false),
        };
        let start = std::time::Instant::now();
        wait_or_stop(&shared, Duration::from_millis(250));
        assert!(start.elapsed() >= Duration::from_millis(200));
    }

    #[test]
    fn test_wait_or_stop_wakes_when_stopped() {
        let shared = Shared {
            child: Mutex::new(None),
            running: AtomicBool::new(false),
            capturing: AtomicBool::new(false),
        };
        let start = std::time::Instant::now();
        wait_or_stop(&shared, Duration::from_secs(30));
        assert!(start.elapsed() < Duration::from_secs(1));
    }
}
