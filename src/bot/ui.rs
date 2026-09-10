use serde_json::json;

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
    let track = status.media.title.as_deref().unwrap_or("No media session");

    format!(
        "📺 <b>TEREBI • {friendly_name}</b>\n\
        ━━━━━━━━━━━━━━━━━━━━━━━\n\
        🎬 <b>Foreground App:</b> {} {}\n\
        📦 <b>Package:</b> <code>{}</code>\n\
        🎵 <b>Media:</b> <code>{}</code>\n\
        ⏱ <b>Playback:</b> {state_icon}\n\
        💡 <b>Screen:</b> {screen_icon}\n\
        ━━━━━━━━━━━━━━━━━━━━━━━",
        status.app_name, status.app_icon, status.package, track
    )
}

#[must_use]
pub fn status_keyboard(package: &str, app_name: &str) -> serde_json::Value {
    let kill_data = format!("kill:{package}");
    json!({
        "inline_keyboard": [
            [
                { "text": format!("🛑 Kill {app_name}"), "callback_data": kill_data },
                { "text": "📸 Screen", "callback_data": "cmd:screen" }
            ],
            [
                { "text": "🎮 Remote", "callback_data": "cmd:remote" },
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
                { "text": "🛑 Disney+", "callback_data": "kill:com.disney.disneyplus" }
            ],
            [
                { "text": "🛑 Kill Current App", "callback_data": "kill:current" },
                { "text": "🏠 TV Home", "callback_data": "key:KEYCODE_HOME" }
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
