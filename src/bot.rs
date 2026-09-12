use anyhow::{Context, Result};
use reqwest::blocking::Client;
use std::time::Duration;

use crate::config::TerebiConfig;
use crate::tv::TvController;

mod api;
mod ui;

use api::{answer_callback, fetch_updates, send_photo, send_text, CallbackQuery, Message, User};
use ui::{
    clear_keyboard, format_status_card, help_text, kill_keyboard, remote_keyboard, status_keyboard,
};

struct BotContext<'a> {
    client: &'a Client,
    config: &'a TerebiConfig,
    tv: &'a TvController,
}

pub fn run_bot(config: &TerebiConfig) -> Result<()> {
    let tv = TvController::new(config.tv_ip.clone(), config.tv_port);
    let client = Client::builder()
        .timeout(Duration::from_secs(35))
        .build()
        .context("Failed to build HTTP client")?;

    let ctx = BotContext {
        client: &client,
        config,
        tv: &tv,
    };

    println!(
        "  ⚡ Terebi Telegram Bot started (Authorized chat: {})...",
        config.chat_id
    );
    let mut offset: i64 = 0;

    loop {
        let Ok(updates) = fetch_updates(ctx.client, &config.bot_token, offset) else {
            std::thread::sleep(Duration::from_secs(3));
            continue;
        };

        for u in updates {
            offset = u.id + 1;
            if let Some(msg) = u.message {
                handle_message(&ctx, msg);
            } else if let Some(cb) = u.callback_query {
                handle_callback(&ctx, cb);
            }
        }
    }
}

fn is_authorized(user_id: Option<&User>, chat_id: &str) -> bool {
    let Ok(auth_id) = chat_id.parse::<i64>() else {
        return false;
    };
    user_id.is_some_and(|u| u.id == auth_id)
}

fn handle_message(ctx: &BotContext, msg: Message) {
    if !is_authorized(msg.from.as_ref(), &ctx.config.chat_id) {
        return;
    }

    let Some(text) = msg.text else { return };
    let trimmed = text.trim();

    if trimmed == "/start" || trimmed == "/help" {
        send_help(ctx, msg.chat.id);
    } else if trimmed == "/status" || trimmed == "/now" {
        send_status(ctx, msg.chat.id);
    } else if trimmed == "/screen" || trimmed == "/screenshot" {
        send_screenshot(ctx, msg.chat.id);
    } else if trimmed.starts_with("/kill") || trimmed.starts_with("/stop") {
        handle_kill_command(ctx, msg.chat.id, trimmed);
    } else if trimmed.starts_with("/clear") {
        handle_clear_command(ctx, msg.chat.id, trimmed);
    } else if trimmed == "/remote" {
        send_remote_menu(ctx, msg.chat.id);
    } else if let Some(app) = trimmed.strip_prefix("/open ") {
        handle_open_command(ctx, msg.chat.id, app.trim());
    }
}

fn send_help(ctx: &BotContext, chat_id: i64) {
    let text = help_text(&ctx.config.friendly_name);
    let _ = send_text(ctx.client, &ctx.config.bot_token, chat_id, &text, None);
}

fn send_status(ctx: &BotContext, chat_id: i64) {
    let status = match ctx.tv.get_status() {
        Ok(s) => s,
        Err(e) => {
            let _ = send_text(
                ctx.client,
                &ctx.config.bot_token,
                chat_id,
                &format!("⚠️ Failed to read TV status: {e}"),
                None,
            );
            return;
        }
    };

    let text = format_status_card(&ctx.config.friendly_name, &status);
    let keyboard = status_keyboard(&status.package, &status.app_name);
    let _ = send_text(
        ctx.client,
        &ctx.config.bot_token,
        chat_id,
        &text,
        Some(keyboard),
    );
}

fn send_screenshot(ctx: &BotContext, chat_id: i64) {
    let _ = send_text(
        ctx.client,
        &ctx.config.bot_token,
        chat_id,
        "📸 Capturing TV screen...",
        None,
    );
    let Ok(bytes) = ctx.tv.take_screenshot() else {
        let _ = send_text(
            ctx.client,
            &ctx.config.bot_token,
            chat_id,
            "⚠️ Failed to take screenshot from TV.",
            None,
        );
        return;
    };

    let is_blank_overlay = bytes.len() < 25_000;
    let caption = if is_blank_overlay {
        let now_playing = ctx.tv.get_status().ok().map_or_else(
            || ctx.config.friendly_name.clone(),
            |s| s.media.title.unwrap_or(s.app_name),
        );
        format!(
            "📺 {} • Live Screenshot\n🎬 <b>Now Playing:</b> {now_playing}\nℹ️ <i>Video frame is protected by hardware overlay/DRM during active playback.</i>",
            ctx.config.friendly_name
        )
    } else {
        format!("📺 {} • Live Screenshot", ctx.config.friendly_name)
    };

    let _ = send_photo(ctx.client, &ctx.config.bot_token, chat_id, &caption, bytes);
}

fn handle_kill_command(ctx: &BotContext, chat_id: i64, input: &str) {
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.len() > 1 {
        let target = parts[1];
        match ctx.tv.force_stop(target) {
            Ok((pkg, name)) => {
                let msg = format!(
                    "🛑 <b>Force-Stopped:</b> {name}\n📦 <code>{pkg}</code>\n🏠 Returned to Home."
                );
                let _ = send_text(ctx.client, &ctx.config.bot_token, chat_id, &msg, None);
            }
            Err(e) => {
                let _ = send_text(
                    ctx.client,
                    &ctx.config.bot_token,
                    chat_id,
                    &format!("⚠️ Failed to kill {target}: {e}"),
                    None,
                );
            }
        }
    } else {
        send_kill_menu(ctx, chat_id);
    }
}

fn send_kill_menu(ctx: &BotContext, chat_id: i64) {
    let text = "🛑 <b>App Murderer — Select App to Kill:</b>";
    let keyboard = kill_keyboard();
    let _ = send_text(
        ctx.client,
        &ctx.config.bot_token,
        chat_id,
        text,
        Some(keyboard),
    );
}

fn handle_clear_command(ctx: &BotContext, chat_id: i64, input: &str) {
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.len() > 1 {
        let target = parts[1];
        match ctx.tv.clear_app_data(target) {
            Ok((pkg, name)) => {
                let msg = format!(
                    "🧹 <b>Data Cleared:</b> {name}\n📦 <code>{pkg}</code>\n🏠 Returned to Home."
                );
                let _ = send_text(ctx.client, &ctx.config.bot_token, chat_id, &msg, None);
            }
            Err(e) => {
                let _ = send_text(
                    ctx.client,
                    &ctx.config.bot_token,
                    chat_id,
                    &format!("⚠️ Failed to clear data for {target}: {e}"),
                    None,
                );
            }
        }
    } else {
        send_clear_menu(ctx, chat_id);
    }
}

fn send_clear_menu(ctx: &BotContext, chat_id: i64) {
    let text = "🧹 <b>Clear App Data — Select App to Reset:</b>";
    let keyboard = clear_keyboard();
    let _ = send_text(
        ctx.client,
        &ctx.config.bot_token,
        chat_id,
        text,
        Some(keyboard),
    );
}

fn send_remote_menu(ctx: &BotContext, chat_id: i64) {
    let text = "🎮 <b>Terebi Virtual Remote Control:</b>";
    let keyboard = remote_keyboard();
    let _ = send_text(
        ctx.client,
        &ctx.config.bot_token,
        chat_id,
        text,
        Some(keyboard),
    );
}

fn handle_open_command(ctx: &BotContext, chat_id: i64, app: &str) {
    match ctx.tv.launch_app(app) {
        Ok(name) => {
            let msg = format!("🚀 <b>Launched:</b> {name} on {}", ctx.config.friendly_name);
            let _ = send_text(ctx.client, &ctx.config.bot_token, chat_id, &msg, None);
        }
        Err(e) => {
            let _ = send_text(
                ctx.client,
                &ctx.config.bot_token,
                chat_id,
                &format!("⚠️ Failed to launch {app}: {e}"),
                None,
            );
        }
    }
}

fn handle_callback(ctx: &BotContext, cb: CallbackQuery) {
    if !is_authorized(Some(&cb.from), &ctx.config.chat_id) {
        return;
    }

    let Some(ref data) = cb.data else { return };
    if data == "noop" {
        let _ = answer_callback(ctx.client, &ctx.config.bot_token, &cb.id, "");
        return;
    }

    if data.starts_with("kill:") {
        handle_kill_callback(ctx, &cb, data.trim_start_matches("kill:"));
    } else if data.starts_with("clear:") {
        handle_clear_callback(ctx, &cb, data.trim_start_matches("clear:"));
    } else if data.starts_with("key:") {
        let key = data.trim_start_matches("key:");
        let _ = ctx.tv.send_key(key);
        let _ = answer_callback(ctx.client, &ctx.config.bot_token, &cb.id, "Key sent");
    } else if data == "cmd:screen" {
        let _ = answer_callback(
            ctx.client,
            &ctx.config.bot_token,
            &cb.id,
            "Capturing screen...",
        );
        if let Some(msg) = cb.message {
            send_screenshot(ctx, msg.chat.id);
        }
    } else if data == "cmd:status" {
        let _ = answer_callback(ctx.client, &ctx.config.bot_token, &cb.id, "Refreshed");
        if let Some(msg) = cb.message {
            send_status(ctx, msg.chat.id);
        }
    } else if data == "cmd:remote" {
        let _ = answer_callback(ctx.client, &ctx.config.bot_token, &cb.id, "Opening remote");
        if let Some(msg) = cb.message {
            send_remote_menu(ctx, msg.chat.id);
        }
    }
}

fn handle_kill_callback(ctx: &BotContext, cb: &CallbackQuery, target: &str) {
    let kill_target = if target == "current" {
        ctx.tv.get_status().map_or_else(
            |_| "com.google.android.youtube.tv".to_string(),
            |s| s.package,
        )
    } else {
        target.to_string()
    };

    if let Ok((pkg, name)) = ctx.tv.force_stop(&kill_target) {
        let _ = answer_callback(
            ctx.client,
            &ctx.config.bot_token,
            &cb.id,
            &format!("Terminated {name}!"),
        );
        if let Some(ref msg) = cb.message {
            let alert = format!(
                "🛑 <b>App Terminated:</b> {name}\n📦 <code>{pkg}</code>\n🏠 Returned to Home."
            );
            let _ = send_text(ctx.client, &ctx.config.bot_token, msg.chat.id, &alert, None);
        }
    }
}

fn handle_clear_callback(ctx: &BotContext, cb: &CallbackQuery, target: &str) {
    let clear_target = if target == "current" {
        ctx.tv.get_status().map_or_else(
            |_| "com.google.android.youtube.tv".to_string(),
            |s| s.package,
        )
    } else {
        target.to_string()
    };

    if let Ok((pkg, name)) = ctx.tv.clear_app_data(&clear_target) {
        let _ = answer_callback(
            ctx.client,
            &ctx.config.bot_token,
            &cb.id,
            &format!("Cleared {name}!"),
        );
        if let Some(ref msg) = cb.message {
            let alert = format!(
                "🧹 <b>App Data Cleared:</b> {name}\n📦 <code>{pkg}</code>\n🏠 Returned to Home."
            );
            let _ = send_text(ctx.client, &ctx.config.bot_token, msg.chat.id, &alert, None);
        }
    }
}
