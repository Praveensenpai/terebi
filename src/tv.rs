use anyhow::Result;

use crate::adb::AdbDevice;
use crate::media::{self, get_app_display, MediaMetadata};

#[derive(Debug, Clone)]
pub struct TvStatus {
    pub package: String,
    pub app_name: String,
    pub app_icon: String,
    pub media: MediaMetadata,
    pub is_screen_on: bool,
}

#[derive(Debug, Clone)]
pub struct TvController {
    pub adb: AdbDevice,
}

impl TvController {
    #[must_use]
    pub fn new(ip: String, port: u16) -> Self {
        Self {
            adb: AdbDevice::new(ip, port),
        }
    }

    pub fn get_status(&self) -> Result<TvStatus> {
        let win_dump = self.adb.shell(&["dumpsys", "window"])?;
        let mut pkg = media::parse_focused_package(&win_dump);

        if pkg.is_none() {
            if let Ok(act_dump) = self.adb.shell(&["dumpsys", "activity", "activities"]) {
                pkg = media::parse_focused_package(&act_dump);
            }
        }

        let package = pkg.unwrap_or_else(|| "com.google.android.tvlauncher".to_string());
        let (app_name, app_icon) = get_app_display(&package);

        let media_dump = self.adb.shell(&["dumpsys", "media_session"])?;
        let mut media = media::parse_media_session(&media_dump);

        if media.position_ms.is_none() || media.position_ms == Some(0) {
            media.position_ms = self.audio_position_ms();
        }

        let pwr_dump = self.adb.shell(&["dumpsys", "power"])?;
        let is_screen_on = pwr_dump.contains("mHoldingDisplaySuspendBlocker=true")
            || pwr_dump.contains("Display Power: state=ON");

        Ok(TvStatus {
            package,
            app_name: app_name.to_string(),
            app_icon: app_icon.to_string(),
            media,
            is_screen_on,
        })
    }

    fn audio_position_ms(&self) -> Option<u64> {
        let output = self
            .adb
            .shell(&["logcat", "-t", "50", "-s", "AudioHw:D"])
            .ok()?;
        output
            .lines()
            .rev()
            .find(|l| l.contains("presentation-position"))
            .and_then(parse_audio_position)
    }

    pub fn force_stop(&self, query: &str) -> Result<(String, String)> {
        let pkg = media::resolve_package(query).unwrap_or_else(|| query.to_string());

        let (name, _) = get_app_display(&pkg);
        let app_label = if name == "App" {
            pkg.clone()
        } else {
            name.to_string()
        };

        let _ = self.adb.shell(&["am", "force-stop", &pkg])?;
        let _ = self.adb.shell(&["input", "keyevent", "KEYCODE_HOME"])?;

        Ok((pkg, app_label))
    }

    pub fn clear_app_data(&self, query: &str) -> Result<(String, String)> {
        let pkg = media::resolve_package(query).unwrap_or_else(|| query.to_string());

        let (name, _) = get_app_display(&pkg);
        let app_label = if name == "App" {
            pkg.clone()
        } else {
            name.to_string()
        };

        let out = self.adb.shell(&["pm", "clear", &pkg])?;
        if !out.contains("Success") {
            anyhow::bail!("Failed to clear data for {pkg}: {out}");
        }
        let _ = self.adb.shell(&["input", "keyevent", "KEYCODE_HOME"])?;

        Ok((pkg, app_label))
    }

    pub fn take_screenshot(&self) -> Result<Vec<u8>> {
        self.adb.exec_raw(&["screencap", "-p"])
    }

    pub fn send_key(&self, keycode: &str) -> Result<()> {
        let _ = self.adb.shell(&["input", "keyevent", keycode])?;
        Ok(())
    }

    pub fn launch_app(&self, query: &str) -> Result<String> {
        let pkg = media::resolve_package(query).unwrap_or_else(|| query.to_string());

        let (name, _) = get_app_display(&pkg);
        let app_label = if name == "App" {
            pkg.clone()
        } else {
            name.to_string()
        };

        let _ = self.adb.shell(&[
            "monkey",
            "-p",
            &pkg,
            "-c",
            "android.intent.category.LAUNCHER",
            "1",
        ])?;

        Ok(app_label)
    }
}

fn parse_audio_position(line: &str) -> Option<u64> {
    let marker = "position=";
    let start = line.find(marker)? + marker.len();
    let rest = &line[start..];
    let end = rest.find("ms")?;
    rest[..end].trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_audio_position() {
        let sample = "10-04 18:05:03.786 389 3961 D AudioHw : [00000002][deep-buffer] presentation-position: written=223809600, delays=5008, offset=0, frames=223804592, position=4662595ms, time=4771079ms, estimated-start=108484ms";
        assert_eq!(parse_audio_position(sample), Some(4662595));
        assert_eq!(parse_audio_position("invalid log line"), None);
    }
}
