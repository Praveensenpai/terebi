use serde_json::json;

use crate::apps::{display_name, page_count, page_slice};
use crate::bot::api::escape_html;
use crate::session::format_duration;
use crate::tv::TvStatus;

#[must_use]
pub fn format_status_card(friendly_name: &str, status: &TvStatus) -> String {
    let state_icon = if status.media.is_playing {
        "▶️ Playing"
    } else {
        "⏸ Paused / Idle"
    };
    let screen_icon = if status.is_screen_on {
        "🟢 On"
    } else {
        "🔴 Off / Sleep"
    };
    let track = if status.media.has_session {
        status
            .media
            .title
            .as_deref()
            .unwrap_or("Playing (title not exposed by app)")
    } else {
        status.media.title.as_deref().unwrap_or("No media session")
    };

    let progress_line = match (status.media.position_ms, status.media.duration_ms) {
        (Some(pos), Some(dur)) if pos > 0 && dur > 0 => {
            let remaining = dur.saturating_sub(pos);
            format!(
                "\n⏳ <b>Progress:</b> {} / {} <i>({} left)</i>",
                format_duration(pos),
                format_duration(dur),
                format_duration(remaining)
            )
        }
        (Some(pos), _) if pos > 0 => format!("\n⏳ <b>Position:</b> {}", format_duration(pos)),
        _ => String::new(),
    };

    let friendly = escape_html(friendly_name);
    let app_name = escape_html(&status.app_name);
    let package = escape_html(&status.package);
    let safe_track = escape_html(track);

    format!(
        "📺 <b>TEREBI • {friendly}</b>\n\
        ━━━━━━━━━━━━━━━━━━━━━━━\n\
        🎬 <b>Foreground App:</b> {app_name} {}\n\
        📦 <b>Package:</b> <code>{package}</code>\n\
        🎵 <b>Media:</b> <code>{safe_track}</code>\n\
        ⏱ <b>Playback:</b> {state_icon}{progress_line}\n\
        💡 <b>Screen:</b> {screen_icon}\n\
        ━━━━━━━━━━━━━━━━━━━━━━━",
        status.app_icon
    )
}

fn btn(text: &str, data: &str) -> serde_json::Value {
    json!({ "text": text, "callback_data": data })
}

/// Telegram rejects a message if any `callback_data` exceeds 64 bytes, so
/// long package names fall back to actions that resolve the app at tap time.
fn fits_callback(prefix: &str, package: &str) -> bool {
    prefix.len() + package.len() <= 64
}

#[must_use]
pub fn status_keyboard(package: &str, app_name: &str) -> serde_json::Value {
    let kill_data = if fits_callback("kill:", package) {
        format!("kill:{package}")
    } else {
        "kill:current".to_string()
    };
    let clear_data = if fits_callback("clearask:", package) {
        format!("clearask:{package}")
    } else {
        "clearask:current".to_string()
    };
    json!({
        "inline_keyboard": [
            [btn(&format!("🛑 Kill {app_name}"), &kill_data), btn("🧹 Erase All Data", &clear_data)],
            [btn("📸 Screen", "cmd:screen"), btn("🎮 Remote", "cmd:remote")],
            [btn("🔄 Refresh", "cmd:status")]
        ]
    })
}

#[must_use]
pub fn kill_keyboard() -> serde_json::Value {
    json!({
        "inline_keyboard": [
            [btn("🛑 YouTube", "kill:com.google.android.youtube.tv"), btn("🛑 SmartTube", "kill:org.smarttube.stable")],
            [btn("🛑 Netflix", "kill:com.netflix.ninja"), btn("🛑 Stremio", "kill:com.frolo.stremio")],
            [btn("🛑 Prime Video", "kill:com.amazon.amazonvideo.livingroom"), btn("🛑 Hotstar", "kill:in.startv.hotstar")],
            [btn("🛑 Kill Current App", "kill:current"), btn("🏠 TV Home", "key:KEYCODE_HOME")]
        ]
    })
}

/// Paginated installed-app keyboard. Button payloads are cache indices, not
/// package names, so each callback stays far below Telegram's 64-byte limit.
#[must_use]
pub fn clear_apps_keyboard(apps: &[String], page: usize) -> serde_json::Value {
    let (page, visible) = page_slice(apps, page);
    let pages = page_count(apps.len(), crate::apps::PAGE_SIZE);
    let offset = page * crate::apps::PAGE_SIZE;

    let mut rows: Vec<serde_json::Value> = visible
        .iter()
        .enumerate()
        .map(|(i, pkg)| {
            json!([{ "text": format!("🧹 {}", display_name(pkg)), "callback_data": format!("clearask:{}", offset + i) }])
        })
        .collect();

    let mut nav = Vec::with_capacity(3);
    if page > 0 {
        nav.push(btn("◀ Prev", &format!("clearlist:{}", page - 1)));
    }
    nav.push(btn(&format!("📄 {}/{pages}", page + 1), "noop"));
    if page + 1 < pages {
        nav.push(btn("Next ▶", &format!("clearlist:{}", page + 1)));
    }
    rows.push(json!(nav));
    rows.push(json!([btn("🔄 Refresh App List", "clearrefresh")]));

    json!({ "inline_keyboard": rows })
}

/// Delay selection keyboard offered when wiping app data.
#[must_use]
pub fn wipe_delay_keyboard(target: &str) -> serde_json::Value {
    json!({
        "inline_keyboard": [
            [btn("⚡ Instant", &format!("wipe:0:{target}"))],
            [
                btn("⏱️ 5 mins", &format!("wipe:5:{target}")),
                btn("⏱️ 15 mins", &format!("wipe:15:{target}"))
            ],
            [
                btn("⏱️ 30 mins", &format!("wipe:30:{target}")),
                btn("⏱️ 1 hour", &format!("wipe:60:{target}"))
            ],
            [btn("❌ Cancel", "wipecancel")]
        ]
    })
}

/// Keyboard on an active scheduled wipe allowing cancellation.
#[must_use]
pub fn wipe_scheduled_keyboard(id: u64) -> serde_json::Value {
    json!({
        "inline_keyboard": [[btn("❌ Cancel Scheduled Wipe", &format!("wipecancel:{id}"))]]
    })
}

/// A power action the bot can request from the TV.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerAction {
    /// Restart Android on the TV.
    Reboot,
    /// Shut the TV down completely.
    PowerOff,
}

impl PowerAction {
    /// Parse a command message such as `/reboot` or `/poweroff`.
    #[must_use]
    pub fn from_command(command: &str) -> Option<Self> {
        match command {
            "/reboot" | "/restart" => Some(Self::Reboot),
            "/poweroff" | "/shutdown" => Some(Self::PowerOff),
            _ => None,
        }
    }

    /// Callback payload for this action's confirm button.
    #[must_use]
    pub fn callback(self) -> &'static str {
        match self {
            Self::Reboot => "power:confirm:reboot",
            Self::PowerOff => "power:confirm:poweroff",
        }
    }

    /// Short human label used in cards and results.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Reboot => "Reboot TV",
            Self::PowerOff => "Power Off TV",
        }
    }

    /// Icon paired with [`Self::title`].
    #[must_use]
    pub fn icon(self) -> &'static str {
        match self {
            Self::Reboot => "🔄",
            Self::PowerOff => "🌙",
        }
    }
}

/// Confirmation keyboard for a pending reboot or shutdown.
#[must_use]
pub fn power_keyboard(action: PowerAction) -> serde_json::Value {
    let label = match action {
        PowerAction::Reboot => "🔄 Reboot Now",
        PowerAction::PowerOff => "🌙 Power Off Now",
    };
    json!({
        "inline_keyboard": [[btn(label, action.callback()), btn("❌ Cancel", "power:cancel")]]
    })
}

/// Keyboard left on a card once its request has been answered or expired.
#[must_use]
pub fn cancelled_keyboard() -> serde_json::Value {
    json!({ "inline_keyboard": [] })
}

#[must_use]
pub fn remote_keyboard() -> serde_json::Value {
    json!({
        "inline_keyboard": [
            [btn(" ", "noop"), btn("⬆️", "key:KEYCODE_DPAD_UP"), btn(" ", "noop")],
            [btn("⬅️", "key:KEYCODE_DPAD_LEFT"), btn("🔘 OK", "key:KEYCODE_DPAD_CENTER"), btn("➡️", "key:KEYCODE_DPAD_RIGHT")],
            [btn(" ", "noop"), btn("⬇️", "key:KEYCODE_DPAD_DOWN"), btn(" ", "noop")],
            [btn("🔙 Back", "key:KEYCODE_BACK"), btn("🏠 Home", "key:KEYCODE_HOME"), btn("⏯ Pause", "key:KEYCODE_MEDIA_PLAY_PAUSE")],
            [btn("🔉 Vol -", "key:KEYCODE_VOLUME_DOWN"), btn("🔊 Vol +", "key:KEYCODE_VOLUME_UP"), btn("🔇 Mute", "key:KEYCODE_VOLUME_MUTE")]
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples(count: usize) -> Vec<String> {
        (0..count).map(|i| format!("com.example.app{i}")).collect()
    }

    #[test]
    fn test_clear_apps_keyboard_paginates() {
        let apps = samples(25);
        let kb = clear_apps_keyboard(&apps, 2);
        let rows = kb["inline_keyboard"].as_array();
        // 5 apps on the last page + nav row + refresh row.
        assert_eq!(rows.map(Vec::len), Some(7));
        assert_eq!(
            kb["inline_keyboard"][0][0]["callback_data"].as_str(),
            Some("clearask:20")
        );
        assert_eq!(
            kb["inline_keyboard"][5][0]["callback_data"].as_str(),
            Some("clearlist:1")
        );
    }

    #[test]
    fn test_clear_apps_keyboard_first_page_has_no_prev() {
        let kb = clear_apps_keyboard(&samples(15), 0);
        let nav = kb["inline_keyboard"][10].as_array();
        assert_eq!(nav.map(Vec::len), Some(2));
        assert_eq!(
            kb["inline_keyboard"][10][1]["callback_data"].as_str(),
            Some("clearlist:1")
        );
    }

    #[test]
    fn test_clear_apps_keyboard_empty() {
        let kb = clear_apps_keyboard(&[], 0);
        // Nav row plus refresh row only.
        assert_eq!(kb["inline_keyboard"].as_array().map(Vec::len), Some(2));
    }

    #[test]
    fn test_status_keyboard_falls_back_for_long_packages() {
        let short = status_keyboard("org.smarttube.stable", "SmartTube");
        assert_eq!(
            short["inline_keyboard"][0][0]["callback_data"].as_str(),
            Some("kill:org.smarttube.stable")
        );
        assert_eq!(
            short["inline_keyboard"][0][1]["callback_data"].as_str(),
            Some("clearask:org.smarttube.stable")
        );

        let long = "x".repeat(60);
        let wide = status_keyboard(&long, "Weird");
        assert_eq!(
            wide["inline_keyboard"][0][0]["callback_data"].as_str(),
            Some("kill:current")
        );
        assert_eq!(
            wide["inline_keyboard"][0][1]["callback_data"].as_str(),
            Some("clearask:current")
        );
    }

    #[test]
    fn test_wipe_delay_keyboard_options() {
        let kb = wipe_delay_keyboard("com.example.app");
        assert_eq!(
            kb["inline_keyboard"][0][0]["callback_data"].as_str(),
            Some("wipe:0:com.example.app")
        );
        assert_eq!(
            kb["inline_keyboard"][1][0]["callback_data"].as_str(),
            Some("wipe:5:com.example.app")
        );
        assert_eq!(
            kb["inline_keyboard"][1][1]["callback_data"].as_str(),
            Some("wipe:15:com.example.app")
        );
        assert_eq!(
            kb["inline_keyboard"][2][0]["callback_data"].as_str(),
            Some("wipe:30:com.example.app")
        );
        assert_eq!(
            kb["inline_keyboard"][2][1]["callback_data"].as_str(),
            Some("wipe:60:com.example.app")
        );
        assert_eq!(
            kb["inline_keyboard"][3][0]["callback_data"].as_str(),
            Some("wipecancel")
        );
    }

    #[test]
    fn test_wipe_scheduled_keyboard_encodes_id() {
        let kb = wipe_scheduled_keyboard(42);
        assert_eq!(
            kb["inline_keyboard"][0][0]["callback_data"].as_str(),
            Some("wipecancel:42")
        );
    }

    #[test]
    fn test_power_keyboard_encodes_action() {
        let kb = power_keyboard(PowerAction::Reboot);
        assert_eq!(
            kb["inline_keyboard"][0][0]["callback_data"].as_str(),
            Some("power:confirm:reboot")
        );
        let kb = power_keyboard(PowerAction::PowerOff);
        assert_eq!(
            kb["inline_keyboard"][0][0]["callback_data"].as_str(),
            Some("power:confirm:poweroff")
        );
        assert_eq!(
            kb["inline_keyboard"][0][1]["callback_data"].as_str(),
            Some("power:cancel")
        );
    }

    #[test]
    fn test_power_action_parses_commands() {
        assert_eq!(
            PowerAction::from_command("/reboot"),
            Some(PowerAction::Reboot)
        );
        assert_eq!(
            PowerAction::from_command("/poweroff"),
            Some(PowerAction::PowerOff)
        );
        assert_eq!(PowerAction::from_command("/power"), None);
    }
}
