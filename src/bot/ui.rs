use serde_json::json;

use crate::apps::{display_name, page_count, page_slice};
use crate::session::format_duration;
use crate::tv::TvStatus;

#[must_use]
pub fn help_text(friendly_name: &str) -> String {
    format!(
        "📺 <b>TEREBI • {friendly_name}</b>\n\
        ━━━━━━━━━━━━━━━━━━━━━━━\n\
        ⚡ <b>Smart TV Telegram Control Hub</b>\n\n\
        🎬 <b>/status</b> — Live TV app & playback info\n\
        📸 <b>/screen</b> — Take live TV screenshot\n\
        🛑 <b>/kill [app]</b> — Force stop an app\n\
        🧹 <b>/clear [app]</b> — Wipe app cache & data\n\
        🎮 <b>/remote</b> — Virtual D-pad controller\n\
        🚀 <b>/open [app]</b> — Launch app on TV\n\
        ━━━━━━━━━━━━━━━━━━━━━━━"
    )
}

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

    format!(
        "📺 <b>TEREBI • {friendly_name}</b>\n\
        ━━━━━━━━━━━━━━━━━━━━━━━\n\
        🎬 <b>Foreground App:</b> {} {}\n\
        📦 <b>Package:</b> <code>{}</code>\n\
        🎵 <b>Media:</b> <code>{}</code>\n\
        ⏱ <b>Playback:</b> {state_icon}{progress_line}\n\
        💡 <b>Screen:</b> {screen_icon}\n\
        ━━━━━━━━━━━━━━━━━━━━━━━",
        status.app_name, status.app_icon, status.package, track
    )
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
        "clearrefresh".to_string()
    };
    json!({
        "inline_keyboard": [
            [
                { "text": format!("🛑 Kill {app_name}"), "callback_data": kill_data },
                { "text": "🧹 Clear Data", "callback_data": clear_data }
            ],
            [
                { "text": "📸 Screen", "callback_data": "cmd:screen" },
                { "text": "🎮 Remote", "callback_data": "cmd:remote" }
            ],
            [
                { "text": "🔄 Refresh", "callback_data": "cmd:status" }
            ]
        ]
    })
}

#[must_use]
pub fn kill_keyboard() -> serde_json::Value {
    json!({
        "inline_keyboard": [
            [
                { "text": "🛑 YouTube", "callback_data": "kill:com.google.android.youtube.tv" },
                { "text": "🛑 SmartTube", "callback_data": "kill:com.teamsmart.videomanager.tv" }
            ],
            [
                { "text": "🛑 Netflix", "callback_data": "kill:com.netflix.ninja" },
                { "text": "🛑 Stremio", "callback_data": "kill:com.frolo.stremio" }
            ],
            [
                { "text": "🛑 Prime Video", "callback_data": "kill:com.amazon.amazonvideo.livingroom" },
                { "text": "🛑 Hotstar", "callback_data": "kill:in.startv.hotstar" }
            ],
            [
                { "text": "🛑 Kill Current App", "callback_data": "kill:current" },
                { "text": "🏠 TV Home", "callback_data": "key:KEYCODE_HOME" }
            ]
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

    let mut nav: Vec<serde_json::Value> = Vec::with_capacity(3);
    if page > 0 {
        nav.push(json!({ "text": "◀ Prev", "callback_data": format!("clearlist:{}", page - 1) }));
    }
    nav.push(json!({ "text": format!("📄 {}/{pages}", page + 1), "callback_data": "noop" }));
    if page + 1 < pages {
        nav.push(json!({ "text": "Next ▶", "callback_data": format!("clearlist:{}", page + 1) }));
    }
    rows.push(json!(nav));
    rows.push(json!([{ "text": "🔄 Refresh App List", "callback_data": "clearrefresh" }]));

    json!({ "inline_keyboard": rows })
}

/// Confirmation keyboard for a pending destructive wipe.
#[must_use]
pub fn confirm_clear_keyboard(target: &str) -> serde_json::Value {
    json!({
        "inline_keyboard": [
            [
                { "text": "✅ Wipe Data", "callback_data": format!("clear:{target}") },
                { "text": "❌ Cancel", "callback_data": "noop" }
            ]
        ]
    })
}

#[must_use]
pub fn remote_keyboard() -> serde_json::Value {
    json!({
        "inline_keyboard": [
            [
                { "text": " ", "callback_data": "noop" },
                { "text": "⬆️", "callback_data": "key:KEYCODE_DPAD_UP" },
                { "text": " ", "callback_data": "noop" }
            ],
            [
                { "text": "⬅️", "callback_data": "key:KEYCODE_DPAD_LEFT" },
                { "text": "🔘 OK", "callback_data": "key:KEYCODE_DPAD_CENTER" },
                { "text": "➡️", "callback_data": "key:KEYCODE_DPAD_RIGHT" }
            ],
            [
                { "text": " ", "callback_data": "noop" },
                { "text": "⬇️", "callback_data": "key:KEYCODE_DPAD_DOWN" },
                { "text": " ", "callback_data": "noop" }
            ],
            [
                { "text": "🔙 Back", "callback_data": "key:KEYCODE_BACK" },
                { "text": "🏠 Home", "callback_data": "key:KEYCODE_HOME" },
                { "text": "⏯ Pause", "callback_data": "key:KEYCODE_MEDIA_PLAY_PAUSE" }
            ],
            [
                { "text": "🔉 Vol -", "callback_data": "key:KEYCODE_VOLUME_DOWN" },
                { "text": "🔊 Vol +", "callback_data": "key:KEYCODE_VOLUME_UP" },
                { "text": "🔇 Mute", "callback_data": "key:KEYCODE_VOLUME_MUTE" }
            ]
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
            Some("clearrefresh")
        );
    }

    #[test]
    fn test_confirm_clear_keyboard_targets_index_or_package() {
        for target in ["3", "org.smarttube.stable"] {
            let expected = format!("clear:{target}");
            let kb = confirm_clear_keyboard(target);
            assert_eq!(
                kb["inline_keyboard"][0][0]["callback_data"].as_str(),
                Some(expected.as_str())
            );
        }
    }
}
