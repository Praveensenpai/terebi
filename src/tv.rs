use anyhow::Result;

use crate::adb::AdbDevice;
use crate::media::{self, get_app_display};
use crate::session::{self, MediaMetadata};

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
        let mut media = session::parse_media_session(&media_dump, Some(&package));

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

    pub fn list_apps(&self) -> Result<Vec<String>> {
        let output = self.adb.shell(&["pm", "list", "packages", "-3"])?;
        let apps = crate::apps::parse_package_list(&output);
        let usage = self
            .adb
            .shell(&["dumpsys", "usagestats"])
            .unwrap_or_default();
        let ranks = crate::apps::parse_usage_ranking(&usage);
        Ok(crate::apps::sort_by_usage(apps, &ranks))
    }

    pub fn take_screenshot(&self) -> Result<Vec<u8>> {
        self.adb.exec_raw(&["screencap", "-p"])
    }

    /// Dump the current UI hierarchy and return the unique visible texts.
    ///
    /// Typed characters cannot be streamed over ADB, so text input is captured
    /// on demand from the accessibility node tree instead.
    pub fn screen_texts(&self) -> Result<Vec<String>> {
        const REMOTE: &str = "/sdcard/terebi_ui.xml";
        let dump = self.adb.shell(&["uiautomator", "dump", REMOTE])?;
        if !dump.contains("dumped to") {
            anyhow::bail!("uiautomator dump failed: {}", dump.trim());
        }
        let xml = self.adb.shell(&["cat", REMOTE])?;
        let _ = self.adb.shell(&["rm", "-f", REMOTE]);
        Ok(parse_ui_texts(&xml))
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

/// Pull unique non-empty label values out of a uiautomator XML dump.
#[must_use]
pub fn parse_ui_texts(xml: &str) -> Vec<String> {
    const ATTRS: [&str; 2] = ["text=", "content-desc="];
    const QUOTE: char = '\u{22}';
    let mut out: Vec<String> = Vec::new();
    for attr in ATTRS {
        let marker = format!("{attr}{QUOTE}");
        let mut rest = xml;
        while let Some(pos) = rest.find(&marker) {
            rest = &rest[pos + marker.len()..];
            let Some(end) = rest.find(QUOTE) else { break };
            let value = unescape(&rest[..end]);
            let value = value.trim();
            if !value.is_empty() && !out.iter().any(|seen| seen == value) {
                out.push(value.to_string());
            }
            rest = &rest[end + 1..];
        }
    }
    out
}

/// Decode the five XML entities `uiautomator` emits in attribute values.
fn unescape(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
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
    fn test_parse_ui_texts_dedupes_and_skips_blank() {
        let xml = r#"<node text="Search" /><node text="" /><node text="Search" /><node text="Sign in" />"#;
        assert_eq!(parse_ui_texts(xml), vec!["Search", "Sign in"]);
    }

    #[test]
    fn test_parse_ui_texts_includes_content_desc() {
        let attr = concat!("content-", "desc=");
        let xml = format!(r#"<node text="Play" /><node {attr}"Episodes" />"#);
        assert_eq!(parse_ui_texts(&xml), vec!["Play", "Episodes"]);
    }

    #[test]
    fn test_parse_ui_texts_unescapes_entities() {
        let xml = r#"<node text="Settings &gt; Battery &amp; Power" />"#;
        assert_eq!(parse_ui_texts(xml), vec!["Settings > Battery & Power"]);
    }

    #[test]
    fn test_parse_ui_texts_handles_missing_text() {
        assert_eq!(parse_ui_texts("<hierarchy />"), Vec::<String>::new());
        assert_eq!(parse_ui_texts("text=\"unterminated"), Vec::<String>::new());
    }

    #[test]
    fn test_parse_audio_position() {
        let sample = "10-04 18:05:03.786 389 3961 D AudioHw : [00000002][deep-buffer] presentation-position: written=223809600, delays=5008, offset=0, frames=223804592, position=4662595ms, time=4771079ms, estimated-start=108484ms";
        assert_eq!(parse_audio_position(sample), Some(4_662_595));
        assert_eq!(parse_audio_position("invalid log line"), None);
    }
}
