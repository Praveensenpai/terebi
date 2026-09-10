use anyhow::{Context, Result};
use reqwest::blocking::multipart::Part;
use reqwest::blocking::{multipart, Client};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

use crate::config::TerebiConfig;
use crate::tv::TvController;

mod ui;
use ui::{format_status_card, help_text, kill_keyboard, remote_keyboard, status_keyboard};

#[derive(Debug, Deserialize)]
struct TelegramResponse<T> {
    result: Option<T>,
}

#[derive(Debug, Deserialize)]
struct Update {
    #[serde(rename = "update_id")]
    id: i64,
    message: Option<Message>,
    callback_query: Option<CallbackQuery>,
}

#[derive(Debug, Deserialize)]
struct Message {
    from: Option<User>,
    chat: Chat,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CallbackQuery {
    id: String,
    from: User,
    message: Option<MessageSummary>,
    data: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MessageSummary {
    chat: Chat,
}

#[derive(Debug, Deserialize)]
struct User {
    id: i64,
}

#[derive(Debug, Deserialize)]
struct Chat {
    id: i64,
}

pub fn run_bot(config: &TerebiConfig) -> Result<()> {
    let tv = TvController::new(config.tv_ip.clone(), config.tv_port);
    let client = Client::builder()
        .timeout(Duration::from_secs(35))
        .build()
        .context("Failed to build HTTP client")?;

    println!(
        "  ⚡ Terebi Telegram Bot started (Authorized chat: {})...",
        config.chat_id
    );
    let mut offset: i64 = 0;

    loop {
        let Ok(updates) = fetch_updates(&client, &config.bot_token, offset) else {
            std::thread::sleep(Duration::from_secs(3));
            continue;
        };

        for u in updates {
            offset = u.id + 1;
            if let Some(msg) = u.message {
                handle_message(&client, config, &tv, msg);
            } else if let Some(cb) = u.callback_query {
                handle_callback(&client, config, &tv, cb);
            }
        }
    }
}

fn fetch_updates(client: &Client, token: &str, offset: i64) -> Result<Vec<Update>> {
    let url = format!("https://api.telegram.org/bot{token}/getUpdates");
    let resp: TelegramResponse<Vec<Update>> = client
        .get(&url)
        .query(&[
            ("offset", offset.to_string()),
            ("timeout", "25".to_string()),
        ])
        .send()?
        .json()?;

    Ok(resp.result.unwrap_or_default())
}

fn is_authorized(user_id: Option<&User>, chat_id: &str) -> bool {
    let Ok(auth_id) = chat_id.parse::<i64>() else {
        return false;
    };
    user_id.is_some_and(|u| u.id == auth_id)
}

fn handle_message(client: &Client, config: &TerebiConfig, tv: &TvController, msg: Message) {
    if !is_authorized(msg.from.as_ref(), &config.chat_id) {
        return;
    }

    let Some(text) = msg.text else { return };
    let trimmed = text.trim();

    if trimmed == "/start" || trimmed == "/help" {
        send_help(client, config, msg.chat.id);
    } else if trimmed == "/status" || trimmed == "/now" {
        send_status(client, config, tv, msg.chat.id);
    } else if trimmed == "/screen" || trimmed == "/screenshot" {
        send_screenshot(client, config, tv, msg.chat.id);
    } else if trimmed.starts_with("/kill") || trimmed.starts_with("/stop") {
        handle_kill_command(client, config, tv, msg.chat.id, trimmed);
    } else if trimmed == "/remote" {
        send_remote_menu(client, config, msg.chat.id);
    } else if let Some(app) = trimmed.strip_prefix("/open ") {
        handle_open_command(client, config, tv, msg.chat.id, app.trim());
    }
}

fn send_help(client: &Client, config: &TerebiConfig, chat_id: i64) {
    let text = help_text(&config.friendly_name);
    let _ = send_text(client, &config.bot_token, chat_id, &text, None);
}

fn send_status(client: &Client, config: &TerebiConfig, tv: &TvController, chat_id: i64) {
    let status = match tv.get_status() {
        Ok(s) => s,
        Err(e) => {
            let _ = send_text(
                client,
                &config.bot_token,
                chat_id,
                &format!("⚠️ Failed to read TV status: {e}"),
                None,
            );
            return;
        }
    };

    let text = format_status_card(&config.friendly_name, &status);
    let keyboard = status_keyboard(&status.package, &status.app_name);
    let _ = send_text(client, &config.bot_token, chat_id, &text, Some(keyboard));
}

fn send_screenshot(client: &Client, config: &TerebiConfig, tv: &TvController, chat_id: i64) {
    let _ = send_text(
        client,
        &config.bot_token,
        chat_id,
        "📸 Capturing TV screen...",
        None,
    );
    let Ok(bytes) = tv.take_screenshot() else {
        let _ = send_text(
            client,
            &config.bot_token,
            chat_id,
            "⚠️ Failed to take screenshot from TV.",
            None,
        );
        return;
    };

    let is_blank_overlay = bytes.len() < 25_000;
    let caption = if is_blank_overlay {
        if let Ok(status) = tv.get_status() {
            let title = status.media.title.as_deref().unwrap_or(&status.app_name);
            format!(
                "📺 {} • Live Screenshot\n🎬 <b>Now Playing:</b> {title}\nℹ️ <i>Video frame is protected by hardware overlay/DRM during active playback.</i>",
                config.friendly_name
            )
        } else {
            format!(
                "📺 {} • Live Screenshot\nℹ️ <i>Video frame is protected by hardware overlay/DRM during active playback.</i>",
                config.friendly_name
            )
        }
    } else {
        format!("📺 {} • Live Screenshot", config.friendly_name)
    };

    let url = format!("https://api.telegram.org/bot{}/sendPhoto", config.bot_token);
    let part = Part::bytes(bytes)
        .file_name("screen.png")
        .mime_str("image/png")
        .unwrap_or_else(|_| Part::bytes(vec![]));
    let form = multipart::Form::new()
        .text("chat_id", chat_id.to_string())
        .text("caption", caption)
        .text("parse_mode", "HTML")
        .part("photo", part);

    let _ = client.post(&url).multipart(form).send();
}

fn handle_kill_command(
    client: &Client,
    config: &TerebiConfig,
    tv: &TvController,
    chat_id: i64,
    input: &str,
) {
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.len() > 1 {
        let target = parts[1];
        match tv.force_stop(target) {
            Ok((pkg, name)) => {
                let msg = format!(
                    "🛑 <b>Force-Stopped:</b> {name}\n📦 <code>{pkg}</code>\n🏠 Returned to Home."
                );
                let _ = send_text(client, &config.bot_token, chat_id, &msg, None);
            }
            Err(e) => {
                let _ = send_text(
                    client,
                    &config.bot_token,
                    chat_id,
                    &format!("⚠️ Failed to kill {target}: {e}"),
                    None,
                );
            }
        }
    } else {
        send_kill_menu(client, config, chat_id);
    }
}

fn send_kill_menu(client: &Client, config: &TerebiConfig, chat_id: i64) {
    let text = "🛑 <b>App Murderer — Select App to Kill:</b>";
    let keyboard = kill_keyboard();
    let _ = send_text(client, &config.bot_token, chat_id, text, Some(keyboard));
}

fn send_remote_menu(client: &Client, config: &TerebiConfig, chat_id: i64) {
    let text = "🎮 <b>Terebi Virtual Remote Control:</b>";
    let keyboard = remote_keyboard();
    let _ = send_text(client, &config.bot_token, chat_id, text, Some(keyboard));
}

fn handle_open_command(
    client: &Client,
    config: &TerebiConfig,
    tv: &TvController,
    chat_id: i64,
    app: &str,
) {
    match tv.launch_app(app) {
        Ok(name) => {
            let msg = format!("🚀 <b>Launched:</b> {name} on {}", config.friendly_name);
            let _ = send_text(client, &config.bot_token, chat_id, &msg, None);
        }
        Err(e) => {
            let _ = send_text(
                client,
                &config.bot_token,
                chat_id,
                &format!("⚠️ Failed to launch {app}: {e}"),
                None,
            );
        }
    }
}

fn handle_callback(client: &Client, config: &TerebiConfig, tv: &TvController, cb: CallbackQuery) {
    if !is_authorized(Some(&cb.from), &config.chat_id) {
        return;
    }

    let Some(data) = cb.data else { return };
    if data == "noop" {
        let _ = answer_callback(client, &config.bot_token, &cb.id, "");
        return;
    }

    if data.starts_with("kill:") {
        let target = data.trim_start_matches("kill:");
        let kill_target = if target == "current" {
            tv.get_status().map_or_else(
                |_| "com.google.android.youtube.tv".to_string(),
                |s| s.package,
            )
        } else {
            target.to_string()
        };

        if let Ok((pkg, name)) = tv.force_stop(&kill_target) {
            let _ = answer_callback(
                client,
                &config.bot_token,
                &cb.id,
                &format!("Terminated {name}!"),
            );
            if let Some(msg) = cb.message {
                let alert = format!(
                    "🛑 <b>App Terminated:</b> {name}\n📦 <code>{pkg}</code>\n🏠 Returned to Home."
                );
                let _ = send_text(client, &config.bot_token, msg.chat.id, &alert, None);
            }
        }
    } else if data.starts_with("key:") {
        let key = data.trim_start_matches("key:");
        let _ = tv.send_key(key);
        let _ = answer_callback(client, &config.bot_token, &cb.id, "Key sent");
    } else if data == "cmd:screen" {
        let _ = answer_callback(client, &config.bot_token, &cb.id, "Capturing screen...");
        if let Some(msg) = cb.message {
            send_screenshot(client, config, tv, msg.chat.id);
        }
    } else if data == "cmd:status" {
        let _ = answer_callback(client, &config.bot_token, &cb.id, "Refreshed");
        if let Some(msg) = cb.message {
            send_status(client, config, tv, msg.chat.id);
        }
    } else if data == "cmd:remote" {
        let _ = answer_callback(client, &config.bot_token, &cb.id, "Opening remote");
        if let Some(msg) = cb.message {
            send_remote_menu(client, config, msg.chat.id);
        }
    }
}

fn send_text(
    client: &Client,
    token: &str,
    chat_id: i64,
    text: &str,
    reply_markup: Option<serde_json::Value>,
) -> Result<()> {
    let url = format!("https://api.telegram.org/bot{token}/sendMessage");
    let mut payload = json!({
        "chat_id": chat_id,
        "text": text,
        "parse_mode": "HTML"
    });

    if let Some(markup) = reply_markup {
        payload["reply_markup"] = markup;
    }

    client.post(&url).json(&payload).send()?;
    Ok(())
}

fn answer_callback(client: &Client, token: &str, cb_id: &str, text: &str) -> Result<()> {
    let url = format!("https://api.telegram.org/bot{token}/answerCallbackQuery");
    let payload = json!({
        "callback_query_id": cb_id,
        "text": text
    });
    client.post(&url).json(&payload).send()?;
    Ok(())
}
