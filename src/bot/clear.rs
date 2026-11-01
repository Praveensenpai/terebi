//! App-data wipe flows: installed-app menu, confirmation, and execution.

use anyhow::Result;

use crate::apps::display_name;
use crate::bot::api::{answer_callback, send_text, CallbackQuery};
use crate::bot::ui::{clear_apps_keyboard, confirm_clear_keyboard};

use super::BotContext;

/// Re-query installed packages from the TV into the shared cache.
pub(super) fn refresh_app_list(ctx: &BotContext) -> Result<usize> {
    let apps = ctx.tv.list_apps()?;
    let count = apps.len();
    *ctx.apps.borrow_mut() = apps;
    Ok(count)
}

pub(super) fn send_clear_menu(ctx: &BotContext, chat_id: i64) {
    match refresh_app_list(ctx) {
        Ok(_) => send_clear_page(ctx, chat_id, 0),
        Err(e) => {
            let _ = send_text(
                ctx.client,
                &ctx.config.bot_token,
                chat_id,
                &format!("⚠️ Failed to list installed apps: {e}"),
                None,
            );
        }
    }
}

fn send_clear_page(ctx: &BotContext, chat_id: i64, page: usize) {
    let apps = ctx.apps.borrow();
    let text = format!(
        "🧹 <b>Clear App Data</b>\n{} installed third-party apps.\nTap an app to confirm a full data wipe.",
        apps.len()
    );
    let keyboard = clear_apps_keyboard(&apps, page);
    drop(apps);
    let _ = send_text(
        ctx.client,
        &ctx.config.bot_token,
        chat_id,
        &text,
        Some(keyboard),
    );
}

pub(super) fn handle_clear_command(ctx: &BotContext, chat_id: i64, input: &str) {
    let parts: Vec<&str> = input.split_whitespace().collect();
    let Some(target) = parts.get(1) else {
        send_clear_menu(ctx, chat_id);
        return;
    };

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
}

pub(super) fn handle_clear_list(ctx: &BotContext, cb: &CallbackQuery, page: usize) {
    let _ = answer_callback(ctx.client, &ctx.config.bot_token, &cb.id, "");
    if let Some(ref msg) = cb.message {
        send_clear_page(ctx, msg.chat.id, page);
    }
}

pub(super) fn handle_clear_refresh(ctx: &BotContext, cb: &CallbackQuery) {
    let _ = answer_callback(ctx.client, &ctx.config.bot_token, &cb.id, "Refreshing");
    if let Some(ref msg) = cb.message {
        send_clear_menu(ctx, msg.chat.id);
    }
}

pub(super) fn handle_clear_ask(ctx: &BotContext, cb: &CallbackQuery, target: &str) {
    let Some((package, label)) = resolve_target(ctx, target) else {
        let _ = answer_callback(
            ctx.client,
            &ctx.config.bot_token,
            &cb.id,
            "Menu expired — send /clear again",
        );
        return;
    };
    let _ = answer_callback(ctx.client, &ctx.config.bot_token, &cb.id, "");

    // Encode the package itself in the confirmation button so a refreshed
    // cache between tapping and confirming can never target a different app.
    let Some(confirm) = confirm_payload(&package) else {
        if let Some(ref msg) = cb.message {
            let hint = format!(
                "⚠️ {label} has a package name too long for a button.\nUse <code>/clear {package}</code> instead."
            );
            let _ = send_text(ctx.client, &ctx.config.bot_token, msg.chat.id, &hint, None);
        }
        return;
    };

    if let Some(ref msg) = cb.message {
        let text = format!(
            "⚠️ <b>Wipe app data?</b>\n🧹 {label}\n📦 <code>{package}</code>\n\nThis resets the app to a fresh-install state. The action cannot be undone."
        );
        let keyboard = confirm_clear_keyboard(&confirm);
        let _ = send_text(
            ctx.client,
            &ctx.config.bot_token,
            msg.chat.id,
            &text,
            Some(keyboard),
        );
    }
}

pub(super) fn handle_clear_execute(ctx: &BotContext, cb: &CallbackQuery, target: &str) {
    let Some((package, label)) = resolve_target(ctx, target) else {
        let _ = answer_callback(
            ctx.client,
            &ctx.config.bot_token,
            &cb.id,
            "Menu expired — send /clear again",
        );
        return;
    };

    match ctx.tv.clear_app_data(&package) {
        Ok((pkg, name)) => {
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
        Err(e) => {
            let _ = answer_callback(ctx.client, &ctx.config.bot_token, &cb.id, "Clear failed");
            if let Some(ref msg) = cb.message {
                let _ = send_text(
                    ctx.client,
                    &ctx.config.bot_token,
                    msg.chat.id,
                    &format!("⚠️ Failed to clear data for {label}: {e}"),
                    None,
                );
            }
        }
    }
}

/// Build the confirmation payload for a package, honouring Telegram's
/// 64-byte `callback_data` limit (the `clear:` prefix takes 6 bytes).
fn confirm_payload(package: &str) -> Option<String> {
    const MAX_DATA: usize = 64;
    const PREFIX_LEN: usize = "clear:".len();
    (package.len() + PREFIX_LEN <= MAX_DATA).then(|| package.to_string())
}

/// Resolve a callback target, which is either a cache index or a package name.
fn resolve_target(ctx: &BotContext, target: &str) -> Option<(String, String)> {
    if let Ok(index) = target.parse::<usize>() {
        let apps = ctx.apps.borrow();
        let package = apps.get(index)?.clone();
        drop(apps);
        let label = display_name(&package);
        return Some((package, label));
    }

    if target == "current" {
        let status = ctx.tv.get_status().ok()?;
        return Some((status.package, status.app_name));
    }

    Some((target.to_string(), display_name(target)))
}
