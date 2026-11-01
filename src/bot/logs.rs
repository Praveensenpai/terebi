//! Remote-key log commands: tail the daily key log, toggle capture, dump text.

use crate::input_log::{latest_log, tail, InputLogger};

use super::api::send_text;
use super::BotContext;

const TAIL_LINES: usize = 40;
const MAX_TEXT_ITEMS: usize = 25;

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
    let msg = format!(
        "🗒 <b>Remote Key Log</b> • <code>{name}</code>\n<pre>{}</pre>",
        escape_html(&body)
    );
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
            let suffix = if extra > 0 {
                format!("\n<i>… {extra} more</i>")
            } else {
                String::new()
            };
            let msg = format!(
                "🔎 <b>On-screen text</b> ({} items)\n<pre>{}</pre>{suffix}",
                texts.len(),
                escape_html(&body)
            );
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

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_html() {
        assert_eq!(escape_html("a<b>&c"), "a&lt;b&gt;&amp;c");
        assert_eq!(escape_html("plain"), "plain");
    }
}
