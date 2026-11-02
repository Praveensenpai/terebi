use anyhow::{Context, Result};
use std::fs::{self, File};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::discovery;

/// Upper bound for any single ADB round trip. A TV that drops off the network
/// leaves `adb` blocked forever; without this the bot's single-threaded loop
/// would stop answering Telegram as well.
const ADB_TIMEOUT: Duration = Duration::from_secs(20);

/// Poll interval while waiting for an ADB child to finish.
const POLL_INTERVAL: Duration = Duration::from_millis(25);

static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
pub struct AdbDevice {
    pub ip: String,
    pub port: u16,
    resolved_ip: Arc<Mutex<Option<String>>>,
}

impl AdbDevice {
    #[must_use]
    pub fn new(ip: String, port: u16) -> Self {
        Self {
            ip,
            port,
            resolved_ip: Arc::new(Mutex::new(None)),
        }
    }

    pub fn resolve_target(&self) -> Result<String> {
        if self.ip != "auto" && !self.ip.is_empty() {
            return Ok(format!("{}:{}", self.ip, self.port));
        }

        let mut lock = self
            .resolved_ip
            .lock()
            .map_err(|e| anyhow::anyhow!("Lock poisoned: {e}"))?;

        if let Some(ref cached_ip) = *lock {
            let candidate = format!("{cached_ip}:{}", self.port);
            if Self::is_target_connected(&candidate) {
                return Ok(candidate);
            }
        }

        if let Some(discovered) = discovery::discover_tv_ip(self.port) {
            let target = format!("{discovered}:{}", self.port);
            *lock = Some(discovered);
            return Ok(target);
        }

        anyhow::bail!(
            "Failed to auto-discover TV on local network. Ensure ADB network debugging is enabled."
        );
    }

    #[must_use]
    pub fn target(&self) -> String {
        self.resolve_target()
            .unwrap_or_else(|_| format!("{}:{}", self.ip, self.port))
    }

    fn is_target_connected(target: &str) -> bool {
        Self::device_state(target).as_deref() == Some("device")
    }

    /// State ADB reports for `target` (`device`, `offline`, `unauthorized`, ...).
    fn device_state(target: &str) -> Option<String> {
        let output = Command::new("adb").args(["devices"]).output().ok()?;
        parse_device_state(&String::from_utf8_lossy(&output.stdout), target)
    }

    pub fn connect(&self) -> Result<bool> {
        let target = self.resolve_target()?;

        // `adb connect` keeps an existing transport that is in a broken state,
        // so a device stuck in `offline`/`authorizing` never recovers. The
        // transport has to be torn down first.
        if Self::device_state(&target).is_some_and(|state| state != "device") {
            let _ = Command::new("adb").args(["disconnect", &target]).output();
        }

        let output = Command::new("adb")
            .args(["connect", &target])
            .output()
            .with_context(|| format!("Failed to run adb connect {target}"))?;

        let text = String::from_utf8_lossy(&output.stdout);
        Ok(text.contains("connected to") || text.contains("already connected"))
    }

    #[must_use]
    pub fn is_connected(&self) -> bool {
        let Ok(target) = self.resolve_target() else {
            return false;
        };
        Self::is_target_connected(&target)
    }

    pub fn ensure_connected(&self) -> Result<()> {
        if !self.is_connected() {
            let connected = self.connect()?;
            if !connected {
                let target = self.resolve_target()?;
                anyhow::bail!("Could not connect to TV at {target}");
            }
            std::thread::sleep(Duration::from_millis(300));
        }
        Ok(())
    }

    pub fn shell(&self, args: &[&str]) -> Result<String> {
        self.ensure_connected()?;
        let target = self.target();
        let mut cmd = vec!["-s", &target, "shell"];
        cmd.extend_from_slice(args);

        let mut command = Command::new("adb");
        command.args(&cmd);
        let output = run_captured(&mut command, ADB_TIMEOUT)
            .with_context(|| format!("adb shell failed on {target}"))?;

        Ok(String::from_utf8_lossy(&output).to_string())
    }

    /// Ask the device to reboot; `power_off` shuts it down instead.
    ///
    /// ADB normally closes the transport while the device restarts, so a
    /// disconnect reported after the request is treated as success.
    pub fn reboot(&self, power_off: bool) -> Result<()> {
        self.ensure_connected()?;
        let target = self.target();
        let mut args = vec!["-s", target.as_str(), "reboot"];
        if power_off {
            args.push("-p");
        }

        let mut command = Command::new("adb");
        command.args(&args);
        run_captured(&mut command, ADB_TIMEOUT)
            .map(|_| ())
            .or_else(|err| {
                if is_transport_drop(&err) {
                    Ok(())
                } else {
                    Err(err)
                }
            })
    }

    pub fn exec_raw(&self, args: &[&str]) -> Result<Vec<u8>> {
        self.ensure_connected()?;
        let target = self.target();
        let mut cmd = vec!["-s", &target, "exec-out"];
        cmd.extend_from_slice(args);

        let mut command = Command::new("adb");
        command.args(&cmd);
        run_captured(&mut command, ADB_TIMEOUT)
            .with_context(|| format!("adb exec-out failed on {target}"))
    }
}

/// Extract the state ADB lists for `target`, skipping the header line.
fn parse_device_state(devices_output: &str, target: &str) -> Option<String> {
    for line in devices_output.lines().skip(1) {
        let mut parts = line.split_whitespace();
        if parts.next() == Some(target) {
            return parts.next().map(str::to_string);
        }
    }
    None
}

/// True when an ADB error means the device hung up mid-command, which is the
/// expected outcome of a reboot request rather than a failure.
fn is_transport_drop(err: &anyhow::Error) -> bool {
    let message = err.to_string().to_lowercase();
    ["closed", "offline", "reset", "broken pipe"]
        .iter()
        .any(|needle| message.contains(needle))
}

/// Run a command with stdout/stderr captured to temp files and a hard deadline.
///
/// Files are used instead of pipes so a large payload (`screencap` is several
/// megabytes) can never block on a full pipe while the deadline is polled.
fn run_captured(command: &mut Command, timeout: Duration) -> Result<Vec<u8>> {
    let base = std::env::temp_dir().join(format!(
        "terebi-adb-{}-{}",
        std::process::id(),
        TEMP_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let out_path = base.with_extension("out");
    let err_path = base.with_extension("err");

    let out_file = File::create(&out_path)
        .with_context(|| format!("Failed to create {}", out_path.display()))?;
    let err_file = File::create(&err_path)
        .with_context(|| format!("Failed to create {}", err_path.display()))?;

    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(out_file))
        .stderr(Stdio::from(err_file));

    let result = spawn_and_wait(command, timeout, &out_path, &err_path);
    let _ = fs::remove_file(&out_path);
    let _ = fs::remove_file(&err_path);
    result
}

fn spawn_and_wait(
    command: &mut Command,
    timeout: Duration,
    out_path: &std::path::Path,
    err_path: &std::path::Path,
) -> Result<Vec<u8>> {
    let mut child = command.spawn().context("Failed to spawn adb")?;
    let deadline = Instant::now() + timeout;

    loop {
        if let Some(status) = child.try_wait().context("Failed to poll adb")? {
            let stdout = fs::read(out_path).unwrap_or_default();
            if status.success() {
                return Ok(stdout);
            }
            let stderr = fs::read_to_string(err_path).unwrap_or_default();
            anyhow::bail!("adb exited with {status}: {}", stderr.trim());
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("adb command timed out after {}s", timeout.as_secs());
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adb_target_string() {
        let dev = AdbDevice::new("192.168.1.100".to_string(), 5555);
        assert_eq!(dev.target(), "192.168.1.100:5555");
    }

    #[test]
    fn test_parse_device_state_reads_reported_state() {
        let output = concat!(
            "List of devices attached\n",
            "192.168.1.38:5555\toffline\n",
            "emulator-5554\tdevice\n",
            "\n"
        );
        assert_eq!(
            parse_device_state(output, "192.168.1.38:5555").as_deref(),
            Some("offline")
        );
        assert_eq!(
            parse_device_state(output, "emulator-5554").as_deref(),
            Some("device")
        );
        assert_eq!(parse_device_state(output, "10.0.0.2:5555"), None);
    }

    #[test]
    fn test_is_transport_drop_matches_disconnects() {
        for message in [
            "adb exited with exit status: 1: error: closed",
            "adb exited with exit status: 1: error: device offline",
            "adb exited with exit status: 1: Connection reset by peer",
        ] {
            assert!(is_transport_drop(&anyhow::anyhow!(message.to_string())));
        }
        let genuine = anyhow::anyhow!("adb exited with exit status: 1: error: no devices found");
        assert!(!is_transport_drop(&genuine));
    }

    #[test]
    fn test_run_captured_returns_stdout() -> Result<()> {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "printf hello"]);
        assert_eq!(run_captured(&mut cmd, Duration::from_secs(5))?, b"hello");
        Ok(())
    }

    #[test]
    fn test_run_captured_reports_failure() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "echo boom >&2; exit 3"]);
        let err = run_captured(&mut cmd, Duration::from_secs(5));
        let message = err.map_or_else(|e| e.to_string(), |_| String::new());
        assert!(message.contains("boom"), "unexpected message: {message}");
    }

    #[test]
    fn test_run_captured_enforces_timeout() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "sleep 5"]);
        let started = Instant::now();
        let err = run_captured(&mut cmd, Duration::from_millis(200));
        assert!(err.is_err());
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "timeout was not enforced"
        );
    }
}
