use anyhow::{Context, Result};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::discovery;

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
        let Ok(output) = Command::new("adb").args(["devices"]).output() else {
            return false;
        };
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            if line.starts_with(target) && line.contains("device") {
                return true;
            }
        }
        false
    }

    pub fn connect(&self) -> Result<bool> {
        let target = self.resolve_target()?;
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

        let output = Command::new("adb")
            .args(&cmd)
            .output()
            .with_context(|| format!("Failed to run adb shell on {target}"))?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    pub fn exec_raw(&self, args: &[&str]) -> Result<Vec<u8>> {
        self.ensure_connected()?;
        let target = self.target();
        let mut cmd = vec!["-s", &target, "exec-out"];
        cmd.extend_from_slice(args);

        let output = Command::new("adb")
            .args(&cmd)
            .output()
            .with_context(|| format!("Failed to run adb exec-out on {target}"))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("adb exec-out failed: {err}");
        }

        Ok(output.stdout)
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
}
