//! Single source of truth for the bot's commands: the `/help` card and the
//! Telegram `/` menu are both generated from [`COMMANDS`].

use serde_json::json;

/// One bot command, used for both `/help` and Telegram's `/` menu so the two
/// can never describe the same command differently.
struct CommandSpec {
    /// Command name without the leading slash.
    name: &'static str,
    /// Argument hint rendered after the name, e.g. `[app]`.
    args: &'static str,
    /// Single-line description of what the command actually does.
    description: &'static str,
    /// Icon shown in the `/help` card only.
    icon: &'static str,
    /// Whether the command appears in Telegram's `/` menu.
    in_menu: bool,
}

/// Shorthand for a command that also appears in Telegram's `/` menu.
const fn cmd(
    name: &'static str,
    args: &'static str,
    description: &'static str,
    icon: &'static str,
) -> CommandSpec {
    CommandSpec {
        name,
        args,
        description,
        icon,
        in_menu: true,
    }
}

const COMMANDS: &[CommandSpec] = &[
    cmd(
        "status",
        "",
        "Foreground app, playback state and screen power",
        "🎬",
    ),
    cmd(
        "screen",
        "",
        "Capture the TV screen and send it as a photo",
        "📸",
    ),
    cmd(
        "kill",
        " [app]",
        "Force-stop an app and return Home; no arg opens a picker",
        "🛑",
    ),
    cmd(
        "clear",
        " [app]",
        "Erase ALL app data like a fresh install; confirms first",
        "🧹",
    ),
    cmd("open", " [app]", "Launch an app on the TV", "🚀"),
    cmd(
        "remote",
        "",
        "Virtual D-pad, volume and media buttons",
        "🎮",
    ),
    cmd("logs", "", "Show the last 40 remote key presses", "🗒"),
    cmd(
        "log",
        " [on|off]",
        "Start, stop or check remote key capture",
        "⏺",
    ),
    cmd(
        "text",
        "",
        "Read the text currently visible on screen",
        "🔎",
    ),
    cmd("reboot", "", "Reboot the TV; asks for confirmation", "🔄"),
    cmd(
        "poweroff",
        "",
        "Power off the TV (falls back to sleep); asks first",
        "🌙",
    ),
    cmd("help", "", "Show this command guide", "❓"),
    CommandSpec {
        name: "start",
        args: "",
        description: "Show this command guide",
        icon: "❓",
        in_menu: false,
    },
];

#[must_use]
pub fn help_text(friendly_name: &str) -> String {
    use std::fmt::Write as _;

    let mut body = String::new();
    for spec in COMMANDS.iter().filter(|spec| spec.name != "start") {
        let _ = writeln!(
            body,
            "{} <b>/{}{}</b> — {}",
            spec.icon, spec.name, spec.args, spec.description
        );
    }
    format!(
        "📺 <b>TEREBI • {friendly_name}</b>\n\
        ━━━━━━━━━━━━━━━━━━━━━━━\n\
        ⚡ <b>Smart TV Telegram Control Hub</b>\n\n\
        {body}\
        ━━━━━━━━━━━━━━━━━━━━━━━"
    )
}

/// Telegram `setMyCommands` payload, derived from [`COMMANDS`].
#[must_use]
pub fn command_menu() -> serde_json::Value {
    let entries: Vec<serde_json::Value> = COMMANDS
        .iter()
        .filter(|spec| spec.in_menu)
        .map(|spec| {
            json!({
                "command": spec.name,
                "description": spec.description,
            })
        })
        .collect();
    json!(entries)
}
