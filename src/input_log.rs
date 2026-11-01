//! Remote-key capture and on-screen text inspection over ADB.
//!
//! Key presses are read from the `WindowManager` logcat channel, which is the
//! only place an unrooted Android TV exposes physical remote events. Each
//! event is appended to a per-day file named after the TV's own clock date, so
//! no timezone or host-clock assumptions are made.

use anyhow::{Context, Result};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use crate::adb::AdbDevice;

const KEY_MARKER: &str = "WindowManager: handleKeyBeforeQueueing event=";
const STAMP_LEN: usize = 23;

/// One parsed remote-key event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyEntry {
    /// TV-local date, `YYYY-MM-DD`.
    pub date: String,
    /// TV-local time of day, `HH:MM:SS.mmm`.
    pub time: String,
    /// Android key name such as `KEYCODE_DPAD_UP`.
    pub key: String,
    /// `down` or `up`.
    pub action: &'static str,
}

impl KeyEntry {
    /// Render the entry as a single log file line.
    #[must_use]
    pub fn to_line(&self) -> String {
        format!("{} {} {} {}", self.date, self.time, self.action, self.key)
    }
}

/// Parse a `WindowManager` logcat line into a key event.
///
/// Returns `None` for any line that is not a key event, which keeps the caller
/// free of filtering logic.
#[must_use]
pub fn parse_key_line(line: &str) -> Option<KeyEntry> {
    let rest = line.split_once(KEY_MARKER)?.1;
    let key = field(rest, "keyCode=")?;
    let action = match field(rest, "action=")? {
        "ACTION_DOWN" => "down",
        "ACTION_UP" => "up",
        _ => return None,
    };
    let (date, time) = split_stamp(line)?;
    Some(KeyEntry {
        date,
        time,
        key: key.to_string(),
        action,
    })
}

/// Extract the date and time portion of a `-v year` logcat prefix.
fn split_stamp(line: &str) -> Option<(String, String)> {
    let stamp = line.get(..STAMP_LEN)?;
    let bytes = stamp.as_bytes();
    if bytes[10] != b' ' || bytes[13] != b':' {
        return None;
    }
    Some((stamp[..10].to_string(), stamp[11..].to_string()))
}

/// Read `key=value` from a whitespace-separated key event body.
fn field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.split_whitespace()
        .find_map(|token| token.strip_prefix(key))
        .map(|value| value.trim_end_matches(','))
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

/// Background logcat reader. Dropping it stops the capture and kills the child.
pub struct InputLogger {
    child: Arc<Mutex<Option<Child>>>,
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl InputLogger {
    /// Spawn `adb logcat` and start appending key events to `dir`.
    pub fn start(adb: &AdbDevice, dir: &Path) -> Result<Self> {
        adb.ensure_connected()?;
        let target = adb.target();
        fs::create_dir_all(dir).with_context(|| format!("Failed to create {}", dir.display()))?;

        let mut child = Command::new("adb")
            .args([
                "-s",
                &target,
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
            .context("Failed to start adb logcat")?;

        let stdout = child
            .stdout
            .take()
            .context("logcat produced no stdout pipe")?;
        let shared = Arc::new(Mutex::new(Some(child)));
        let running = Arc::new(AtomicBool::new(true));

        let flag = Arc::clone(&running);
        let dir_owned = dir.to_path_buf();
        let thread = thread::spawn(move || {
            let mut writer = DailyWriter::new(dir_owned);
            for line in BufReader::new(stdout).lines() {
                if !flag.load(Ordering::Relaxed) {
                    break;
                }
                let Ok(line) = line else { break };
                if let Some(entry) = parse_key_line(&line) {
                    if writer.write(&entry).is_err() {
                        break;
                    }
                }
            }
            flag.store(false, Ordering::Relaxed);
        });

        Ok(Self {
            child: shared,
            running,
            thread: Some(thread),
        })
    }

    /// Whether the reader thread is still consuming events.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Stop the capture and join the reader thread.
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Ok(mut guard) = self.child.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
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

/// Newest key log file in `dir`, chosen by the date in the file name.
#[must_use]
pub fn latest_log(dir: &Path) -> Option<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "log"))
        .collect();
    files.sort();
    files.pop()
}

/// Read the last `count` lines of a log file, oldest first.
pub fn tail(path: &Path, count: usize) -> Result<Vec<String>> {
    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(count);
    Ok(lines[start..]
        .iter()
        .map(|line| (*line).to_string())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOWN: &str = "2026-11-01 22:17:38.429   723  1833 D WindowManager: handleKeyBeforeQueueing event=KeyEvent { action=ACTION_DOWN, keyCode=KEYCODE_DPAD_DOWN, scanCode=0, metaState=0, flags=0x0, repeatCount=0, eventTime=1, downTime=1, deviceId=-1, source=0x0, displayId=-1 }";
    const UP: &str = "2026-11-01 22:17:38.510   723  1833 D WindowManager: handleKeyBeforeQueueing event=KeyEvent { action=ACTION_UP, keyCode=KEYCODE_DPAD_DOWN, scanCode=0, metaState=0, flags=0x0, repeatCount=0, eventTime=1, downTime=1, deviceId=-1, source=0x0, displayId=-1 }";

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
    fn test_parse_key_line_down() {
        assert_eq!(
            parse_key_line(DOWN),
            Some(sample(
                "2026-11-01",
                "22:17:38.429",
                "KEYCODE_DPAD_DOWN",
                "down"
            ))
        );
    }

    #[test]
    fn test_parse_key_line_up() {
        assert_eq!(
            parse_key_line(UP),
            Some(sample(
                "2026-11-01",
                "22:17:38.510",
                "KEYCODE_DPAD_DOWN",
                "up"
            ))
        );
    }

    #[test]
    fn test_parse_key_line_rejects_other_actions() {
        let multi = DOWN.replace("ACTION_DOWN", "ACTION_MULTIPLE");
        assert_eq!(parse_key_line(&multi), None);
    }

    #[test]
    fn test_parse_key_line_rejects_unrelated_lines() {
        assert_eq!(parse_key_line("--------- beginning of main"), None);
        assert_eq!(
            parse_key_line("2026-11-01 22:17:38.429   723  1833 D WindowManager: focus changed"),
            None
        );
        assert_eq!(parse_key_line("too short"), None);
    }

    #[test]
    fn test_parse_key_line_handles_unknown_keycodes() {
        let line = DOWN.replace("KEYCODE_DPAD_DOWN", "KEYCODE_UNKNOWN_42");
        assert_eq!(
            parse_key_line(&line),
            Some(sample(
                "2026-11-01",
                "22:17:38.429",
                "KEYCODE_UNKNOWN_42",
                "down"
            ))
        );
    }

    #[test]
    fn test_key_entry_line_format() {
        let entry = sample("2026-11-01", "22:17:38.429", "KEYCODE_HOME", "down");
        assert_eq!(entry.to_line(), "2026-11-01 22:17:38.429 down KEYCODE_HOME");
    }

    #[test]
    fn test_latest_log_missing_dir() {
        let dir = temp_dir("missing");
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(latest_log(&dir), None);
    }

    #[test]
    fn test_latest_log_picks_newest_date() -> Result<()> {
        let dir = temp_dir("logs");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir)?;
        for name in ["2026-10-31.log", "2026-11-02.log", "2026-11-01.log"] {
            fs::write(dir.join(name), "x")?;
        }
        let newest = latest_log(&dir).context("no log file found")?;
        assert_eq!(
            newest.file_name().and_then(|name| name.to_str()),
            Some("2026-11-02.log")
        );
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn test_tail_returns_last_lines() -> Result<()> {
        let dir = temp_dir("tail");
        fs::create_dir_all(&dir)?;
        let path = dir.join("2026-11-01.log");
        fs::write(&path, "a\nb\nc\nd\n")?;
        assert_eq!(tail(&path, 2)?, vec!["c", "d"]);
        assert_eq!(tail(&path, 99)?.len(), 4);
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn test_daily_writer_rotates_across_dates() -> Result<()> {
        let dir = temp_dir("rotate");
        let _ = fs::remove_dir_all(&dir);
        let mut writer = DailyWriter::new(dir.clone());
        for date in ["2026-11-01", "2026-11-01", "2026-11-02"] {
            writer.write(&sample(date, "10:00:00.000", "KEYCODE_HOME", "down"))?;
        }
        assert_eq!(tail(&dir.join("2026-11-01.log"), 10)?.len(), 2);
        assert_eq!(tail(&dir.join("2026-11-02.log"), 10)?.len(), 1);
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }
}
