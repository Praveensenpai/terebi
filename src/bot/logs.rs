//! Remote-key log commands: tail the daily key log, toggle capture, dump text.

use crate::input_log::{latest_log, tail, InputLogger};

use super::api::send_text;
use super::BotContext;

const TAIL_LINES: usize = 40;
const MAX_TEXT_ITEMS: usize = 25;

/// Telegram rejects any message longer than this with HTTP 400.
const TELEGRAM_MAX: usize = 4096;
const TRUNCATION_MARKER: &str = "\n… (truncated)";

/// `/logs` — show the tail of the newest key log file.
pub(super) fn send_log_tail(ctx: &BotContext, chat_id: i64) {
    let dir = ctx.config.logging.path();
    let Some(path) = latest_log(&dir) else {
        let msg = format!(
            "🗒 <b>No key logs yet</b>\n📁 <code>{}</code>\nStart capture with <b>/log on</b>.",
            dir.display()
        );
        let _ = send_text(ctx.client, &ctx.config.bot_token, chat_id, &msg, None);
        return;
    };

    let name = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().to_string());
    let lines = match tail(&path, TAIL_LINES) {
        Ok(lines) => lines,
        Err(e) => {
            let _ = send_text(
                ctx.client,
                &ctx.config.bot_token,
                chat_id,
                &format!("⚠️ Failed to read {name}: {e}"),
                None,
            );
            return;
        }
    };

    let body = if lines.is_empty() {
        "(empty — no key presses captured yet)".to_string()
    } else {
        lines.join("\n")
    };
    let prefix = format!("🗒 <b>Remote Key Log</b> • <code>{name}</code>\n<pre>");
    let suffix = "</pre>";
    let msg = format!("{prefix}{}{suffix}", clamp_body(&prefix, &body, suffix));
    let _ = send_text(ctx.client, &ctx.config.bot_token, chat_id, &msg, None);
}

/// `/log on|off` — start, stop, or report the logcat reader.
pub(super) fn handle_log_toggle(ctx: &BotContext, chat_id: i64, input: &str) {
    let arg = input.split_whitespace().nth(1).unwrap_or("status");
    match arg {
        "on" => start_capture(ctx, chat_id),
        "off" => stop_capture(ctx, chat_id),
        _ => send_capture_status(ctx, chat_id),
    }
}

pub(super) fn start_capture(ctx: &BotContext, chat_id: i64) {
    let mut slot = ctx.logger.borrow_mut();
    if slot.is_some() {
        let _ = send_text(
            ctx.client,
            &ctx.config.bot_token,
            chat_id,
            "🗒 Key capture is already running.",
            None,
        );
        return;
    }

    let dir = ctx.config.logging.path();
    match InputLogger::start(&ctx.tv.adb, &dir) {
        Ok(logger) => {
            *slot = Some(logger);
            let msg = format!(
                "🗒 <b>Key capture started</b>\n📁 <code>{}</code>",
                dir.display()
            );
            let _ = send_text(ctx.client, &ctx.config.bot_token, chat_id, &msg, None);
        }
        Err(e) => {
            let _ = send_text(
                ctx.client,
                &ctx.config.bot_token,
                chat_id,
                &format!("⚠️ Failed to start key capture: {e}"),
                None,
            );
        }
    }
}

pub(super) fn stop_capture(ctx: &BotContext, chat_id: i64) {
    let mut slot = ctx.logger.borrow_mut();
    if let Some(mut logger) = slot.take() {
        logger.stop();
        let _ = send_text(
            ctx.client,
            &ctx.config.bot_token,
            chat_id,
            "🗒 Key capture stopped.",
            None,
        );
    } else {
        let _ = send_text(
            ctx.client,
            &ctx.config.bot_token,
            chat_id,
            "🗒 Key capture is not running.",
            None,
        );
    }
}

pub(super) fn send_capture_status(ctx: &BotContext, chat_id: i64) {
    let slot = ctx.logger.borrow();
    let state = match slot.as_ref() {
        Some(logger) if logger.is_running() => "🟢 capturing",
        Some(_) => "🟡 reader exited (TV unreachable?)",
        None => "🔴 stopped",
    };
    let dir = ctx.config.logging.path();
    let msg = format!(
        "🗒 <b>Key capture:</b> {state}\n📁 <code>{}</code>\nUse <b>/log on</b> or <b>/log off</b>.",
        dir.display()
    );
    drop(slot);
    let _ = send_text(ctx.client, &ctx.config.bot_token, chat_id, &msg, None);
}

/// `/text` — dump the visible on-screen texts for input diagnostics.
pub(super) fn send_screen_text(ctx: &BotContext, chat_id: i64) {
    let _ = send_text(
        ctx.client,
        &ctx.config.bot_token,
        chat_id,
        "🔎 Reading on-screen text...",
        None,
    );
    match ctx.tv.screen_texts() {
        Ok(texts) if texts.is_empty() => {
            let _ = send_text(
                ctx.client,
                &ctx.config.bot_token,
                chat_id,
                "🔎 No text nodes found on the current screen.",
                None,
            );
        }
        Ok(texts) => {
            let body = texts
                .iter()
                .take(MAX_TEXT_ITEMS)
                .map(|t| format!("• {t}"))
                .collect::<Vec<_>>()
                .join("\n");
            let extra = texts.len().saturating_sub(MAX_TEXT_ITEMS);
            let more = if extra > 0 {
                format!("\n<i>… {extra} more</i>")
            } else {
                String::new()
            };
            let prefix = format!("🔎 <b>On-screen text</b> ({} items)\n<pre>", texts.len());
            let suffix = format!("</pre>{more}");
            let msg = format!("{prefix}{}{suffix}", clamp_body(&prefix, &body, &suffix));
            let _ = send_text(ctx.client, &ctx.config.bot_token, chat_id, &msg, None);
        }
        Err(e) => {
            let _ = send_text(
                ctx.client,
                &ctx.config.bot_token,
                chat_id,
                &format!("⚠️ Failed to read screen text: {e}"),
                None,
            );
        }
    }
}

/// Escape `body` for the HTML parse mode and truncate it so that
/// `prefix + escaped + suffix` stays within Telegram's 4096 UTF-16 unit cap.
///
/// Truncation is applied to the escaped form because escaping can expand a
/// single character up to five times (`&` becomes `&amp;`).
fn clamp_body(prefix: &str, body: &str, suffix: &str) -> String {
    let reserved = prefix.encode_utf16().count()
        + suffix.encode_utf16().count()
        + TRUNCATION_MARKER.encode_utf16().count();
    let budget = TELEGRAM_MAX.saturating_sub(reserved);

    let mut out = String::new();
    let mut used = 0;
    for ch in body.chars() {
        let replacement = match ch {
            '&' => "&amp;",
            '<' => "&lt;",
            '>' => "&gt;",
            _ => "",
        };
        let cost = if replacement.is_empty() {
            ch.len_utf16()
        } else {
            replacement.len()
        };
        if used + cost > budget {
            out.push_str(TRUNCATION_MARKER);
            break;
        }
        if replacement.is_empty() {
            out.push(ch);
        } else {
            out.push_str(replacement);
        }
        used += cost;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clamp_escapes_html() {
        assert_eq!(clamp_body("", "a<b>&c", ""), "a&lt;b&gt;&amp;c");
        assert_eq!(clamp_body("", "plain", ""), "plain");
    }

    #[test]
    fn test_clamp_truncates_overlong_body() {
        let body = "&".repeat(10_000);
        let out = clamp_body("<pre>", &body, "</pre>");
        assert!(out.ends_with(TRUNCATION_MARKER));
        assert!(out.encode_utf16().count() < TELEGRAM_MAX);
    }

    #[test]
    fn test_clamp_keeps_short_body_intact() {
        let body = "line one\nline two";
        assert_eq!(clamp_body("<pre>", body, "</pre>"), body);
    }
}
