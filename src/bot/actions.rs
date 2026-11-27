//! Kill, launch, and remote-control handlers for the Telegram bot.

use super::api::{answer_callback, escape_html, send_text, CallbackQuery};
use super::ui::{kill_keyboard, remote_keyboard};
use super::BotContext;

pub(super) fn handle_kill_command(ctx: &BotContext, chat_id: i64, input: &str) {
    let parts: Vec<&str> = input.split_whitespace().collect();
    let Some(target) = parts.get(1) else {
        send_kill_menu(ctx, chat_id);
        return;
    };

    match ctx.tv.force_stop(target) {
        Ok((pkg, name)) => {
            let msg = format!(
                "🛑 <b>Force-Stopped:</b> {}\n📦 <code>{}</code>\n🏠 Returned to Home.",
                escape_html(&name),
                escape_html(&pkg)
            );
            let _ = send_text(ctx.client, &ctx.config.bot_token, chat_id, &msg, None);
        }
        Err(e) => {
            let _ = send_text(
                ctx.client,
                &ctx.config.bot_token,
                chat_id,
                &format!(
                    "⚠️ Failed to kill {}: {}",
                    escape_html(target),
                    escape_html(&e.to_string())
                ),
                None,
            );
        }
    }
}

pub(super) fn send_kill_menu(ctx: &BotContext, chat_id: i64) {
    let text = "🛑 <b>App Murderer — Select App to Kill:</b>";
    let _ = send_text(
        ctx.client,
        &ctx.config.bot_token,
        chat_id,
        text,
        Some(kill_keyboard()),
    );
}

pub(super) fn send_remote_menu(ctx: &BotContext, chat_id: i64) {
    let text = "🎮 <b>Terebi Virtual Remote Control:</b>";
    let _ = send_text(
        ctx.client,
        &ctx.config.bot_token,
        chat_id,
        text,
        Some(remote_keyboard()),
    );
}

pub(super) fn handle_open_command(ctx: &BotContext, chat_id: i64, app: &str) {
    match ctx.tv.launch_app(app) {
        Ok(name) => {
            let msg = format!(
                "🚀 <b>Launched:</b> {} on {}",
                escape_html(&name),
                escape_html(&ctx.config.friendly_name)
            );
            let _ = send_text(ctx.client, &ctx.config.bot_token, chat_id, &msg, None);
        }
        Err(e) => {
            let _ = send_text(
                ctx.client,
                &ctx.config.bot_token,
                chat_id,
                &format!(
                    "⚠️ Failed to launch {}: {}",
                    escape_html(app),
                    escape_html(&e.to_string())
                ),
                None,
            );
        }
    }
}

pub(super) fn handle_kill_callback(ctx: &BotContext, cb: &CallbackQuery, target: &str) {
    let kill_target = if target == "current" {
        match ctx.tv.get_status() {
            Ok(s) => s.package,
            Err(e) => {
                report_kill_error(
                    ctx,
                    cb,
                    "Status failed",
                    "Could not detect foreground app",
                    &e.to_string(),
                );
                return;
            }
        }
    } else {
        target.to_string()
    };

    match ctx.tv.force_stop(&kill_target) {
        Ok((pkg, name)) => {
            let _ = answer_callback(
                ctx.client,
                &ctx.config.bot_token,
                &cb.id,
                &format!("Terminated {name}!"),
            );
            if let Some(ref msg) = cb.message {
                let alert = format!(
                    "🛑 <b>App Terminated:</b> {}\n📦 <code>{}</code>\n🏠 Returned to Home.",
                    escape_html(&name),
                    escape_html(&pkg)
                );
                let _ = send_text(ctx.client, &ctx.config.bot_token, msg.chat.id, &alert, None);
            }
        }
        Err(e) => report_kill_error(ctx, cb, "Kill failed", "Failed to kill app", &e.to_string()),
    }
}

fn report_kill_error(
    ctx: &BotContext,
    cb: &CallbackQuery,
    toast: &str,
    prefix: &str,
    detail: &str,
) {
    let _ = answer_callback(ctx.client, &ctx.config.bot_token, &cb.id, toast);
    if let Some(ref msg) = cb.message {
        let text = format!("⚠️ {prefix}: {}", escape_html(detail));
        let _ = send_text(ctx.client, &ctx.config.bot_token, msg.chat.id, &text, None);
    }
}
