use anyhow::{Context, Result};
use std::process::Command;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct AdbDevice {
    pub ip: String,
    pub port: u16,
}

impl AdbDevice {
    #[must_use]
    pub fn new(ip: String, port: u16) -> Self {
        Self { ip, port }
    }

    #[must_use]
    pub fn target(&self) -> String {
        format!("{}:{}", self.ip, self.port)
    }

    pub fn connect(&self) -> Result<bool> {
        let target = self.target();
        let output = Command::new("adb")
            .args(["connect", &target])
            .output()
            .with_context(|| format!("Failed to run adb connect {target}"))?;

        let text = String::from_utf8_lossy(&output.stdout);
        Ok(text.contains("connected to") || text.contains("already connected"))
    }

    #[must_use]
    pub fn is_connected(&self) -> bool {
        let target = self.target();
        let Ok(output) = Command::new("adb").args(["devices"]).output() else {
            return false;
        };
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            if line.starts_with(&target) && line.contains("device") {
                return true;
            }
        }
        false
    }

    pub fn ensure_connected(&self) -> Result<()> {
        if !self.is_connected() {
            let connected = self.connect()?;
            if !connected {
                anyhow::bail!("Could not connect to TV at {}", self.target());
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
