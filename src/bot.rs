use anyhow::{Context, Result};
use reqwest::blocking::Client;
use std::cell::RefCell;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::config::TerebiConfig;
use crate::input_log::InputLogger;
use crate::tv::TvController;

mod actions;
mod api;
mod clear;
mod commands;
mod logs;
mod power;
mod ui;

use actions::{handle_kill_callback, handle_kill_command, handle_open_command, send_remote_menu};
use api::{
    answer_callback, escape_html, fetch_updates, send_photo, send_text, set_my_commands,
    CallbackQuery, Message, User,
};
use ui::{format_status_card, status_keyboard, PowerAction};

struct BotContext<'a> {
    client: &'a Client,
    config: &'a TerebiConfig,
    tv: &'a TvController,
    /// Installed third-party packages, refreshed whenever a menu opens.
    apps: RefCell<Vec<String>>,
    /// Active logcat reader, present only while key capture is running.
    logger: RefCell<Option<InputLogger>>,
    /// Power action awaiting confirmation, if any.
    pending_power: RefCell<Option<power::Pending>>,
    /// When the last power action actually ran.
    power_guard: RefCell<Option<Instant>>,
    /// Background scheduler for delayed app data wipes.
    scheduler: Arc<clear::WipeScheduler>,
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
        apps: RefCell::new(Vec::new()),
        logger: RefCell::new(None),
        pending_power: RefCell::new(None),
        power_guard: RefCell::new(None),
        scheduler: Arc::new(clear::WipeScheduler::new()),
    };

    let mut menu_registered = register_command_menu(&client, &config.bot_token);
    start_input_logger(&ctx);

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

        if !menu_registered {
            menu_registered =
                set_my_commands(ctx.client, &config.bot_token, &commands::command_menu()).is_ok();
        }

        for u in updates {
            offset = u.id + 1;
            if let Some(msg) = u.message {
                handle_message(&ctx, &msg);
            } else if let Some(cb) = u.callback_query {
                handle_callback(&ctx, &cb);
            }
        }
    }
}

fn register_command_menu(client: &Client, token: &str) -> bool {
    let ok = set_my_commands(client, token, &commands::command_menu()).is_ok();
    if !ok {
        println!("  ⚠️ Command menu registration deferred (will retry when online)");
    }
    ok
}

fn start_input_logger(ctx: &BotContext) {
    if !ctx.config.logging.enabled {
        return;
    }
    let dir = ctx.config.logging.path();
    match InputLogger::start(&ctx.tv.adb, &dir) {
        Ok(logger) => {
            *ctx.logger.borrow_mut() = Some(logger);
            println!("  🗒 Key capture active → {}", dir.display());
        }
        Err(e) => println!("  ⚠️ Key capture failed to start: {e}"),
    }
}

fn is_authorized(user_id: Option<&User>, chat_id: Option<i64>, authorized_id: &str) -> bool {
    let Ok(auth_id) = authorized_id.parse::<i64>() else {
        return false;
    };
    if auth_id < 0 {
        return chat_id == Some(auth_id);
    }
    user_id.is_some_and(|u| u.id == auth_id) && chat_id.is_none_or(|c| c == auth_id)
}

fn handle_message(ctx: &BotContext, msg: &Message) {
    if !is_authorized(msg.from.as_ref(), Some(msg.chat.id), &ctx.config.chat_id) {
        return;
    }

    let Some(text) = msg.text.as_deref() else {
        return;
    };
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
        clear::handle_clear_command(ctx, msg.chat.id, trimmed);
    } else if trimmed == "/cancelwipe" {
        clear::handle_clear_command(ctx, msg.chat.id, "/clear cancel");
    } else if trimmed == "/logs" {
        logs::send_log_tail(ctx, msg.chat.id);
    } else if trimmed.starts_with("/log") {
        logs::handle_log_toggle(ctx, msg.chat.id, trimmed);
    } else if trimmed == "/text" {
        logs::send_screen_text(ctx, msg.chat.id);
    } else if let Some(action) = PowerAction::from_command(trimmed) {
        power::handle_power_command(ctx, msg, action);
    } else if trimmed == "/remote" {
        send_remote_menu(ctx, msg.chat.id);
    } else if let Some(app) = trimmed.strip_prefix("/open ") {
        handle_open_command(ctx, msg.chat.id, app.trim());
    }
}

fn send_help(ctx: &BotContext, chat_id: i64) {
    let text = commands::help_text(&ctx.config.friendly_name);
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
                &format!(
                    "⚠️ Failed to read TV status: {}",
                    escape_html(&e.to_string())
                ),
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
    let friendly = escape_html(&ctx.config.friendly_name);
    let caption = if is_blank_overlay {
        let now_playing = ctx.tv.get_status().ok().map_or_else(
            || ctx.config.friendly_name.clone(),
            |s| s.media.title.unwrap_or(s.app_name),
        );
        format!(
            "📺 {friendly} • Live Screenshot\n🎬 <b>Now Playing:</b> {}\nℹ️ <i>Video frame is protected by hardware overlay/DRM during active playback.</i>",
            escape_html(&now_playing)
        )
    } else {
        format!("📺 {friendly} • Live Screenshot")
    };

    if let Err(e) = send_photo(ctx.client, &ctx.config.bot_token, chat_id, &caption, bytes) {
        let _ = send_text(
            ctx.client,
            &ctx.config.bot_token,
            chat_id,
            &format!(
                "⚠️ Failed to send screenshot: {}",
                escape_html(&e.to_string())
            ),
            None,
        );
    }
}

fn handle_callback(ctx: &BotContext, cb: &CallbackQuery) {
    let chat_id = cb.message.as_ref().map(|m| m.chat.id);
    if !is_authorized(Some(&cb.from), chat_id, &ctx.config.chat_id) {
        return;
    }

    let Some(ref data) = cb.data else { return };
    if data == "noop" {
        let _ = answer_callback(ctx.client, &ctx.config.bot_token, &cb.id, "");
        return;
    }

    if let Some(target) = data.strip_prefix("kill:") {
        handle_kill_callback(ctx, cb, target);
    } else if data == "clearrefresh" {
        clear::handle_clear_refresh(ctx, cb);
    } else if let Some(page) = data.strip_prefix("clearlist:") {
        clear::handle_clear_list(ctx, cb, page.parse().unwrap_or(0));
    } else if let Some(target) = data
        .strip_prefix("clearask:")
        .or_else(|| data.strip_prefix("clear:"))
    {
        clear::handle_clear_ask(ctx, cb, target);
    } else if let Some(payload) = data.strip_prefix("wipe:") {
        clear::handle_clear_execute_delayed(ctx, cb, payload);
    } else if let Some(id_str) = data.strip_prefix("wipecancel:") {
        clear::handle_wipe_cancel_id(ctx, cb, id_str);
    } else if data == "wipecancel" {
        clear::handle_wipe_cancel(ctx, cb);
    } else if data.starts_with("power:") {
        power::handle_power_callback(ctx, cb, data);
    } else if let Some(key) = data.strip_prefix("key:") {
        let toast = if ctx.tv.send_key(key).is_ok() {
            "Key sent"
        } else {
            "Failed to send key"
        };
        let _ = answer_callback(ctx.client, &ctx.config.bot_token, &cb.id, toast);
    } else if let Some(cmd) = data.strip_prefix("cmd:") {
        handle_cmd_callback(ctx, cb, cmd);
    }
}

fn handle_cmd_callback(ctx: &BotContext, cb: &CallbackQuery, cmd: &str) {
    let toast = match cmd {
        "screen" => "Capturing screen...",
        "status" => "Refreshed",
        "remote" => "Opening remote",
        _ => return,
    };
    let _ = answer_callback(ctx.client, &ctx.config.bot_token, &cb.id, toast);
    let Some(ref msg) = cb.message else { return };
    match cmd {
        "screen" => send_screenshot(ctx, msg.chat.id),
        "status" => send_status(ctx, msg.chat.id),
        "remote" => send_remote_menu(ctx, msg.chat.id),
        _ => {}
    }
}
