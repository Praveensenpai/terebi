//! App-data wipe flows: installed-app menu, confirmation, delay options, and execution.

use anyhow::Result;

use crate::apps::display_name;
use crate::bot::api::{answer_callback, edit_message_text, send_text, CallbackQuery};
use crate::bot::ui::{
    cancelled_keyboard, clear_apps_keyboard, confirm_clear_keyboard, wipe_delay_keyboard,
    wipe_scheduled_keyboard,
};

use super::BotContext;

mod scheduler;
pub use scheduler::{ScheduleArgs, WipeScheduler};

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

    if *target == "cancel" {
        handle_clear_cancel(ctx, chat_id);
        return;
    }

    let delay_mins: u64 = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    if delay_mins > 0 {
        schedule_command_wipe(ctx, chat_id, target, delay_mins);
        return;
    }

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

fn handle_clear_cancel(ctx: &BotContext, chat_id: i64) {
    let cancelled = ctx.scheduler.cancel_all();
    let msg = if cancelled.is_empty() {
        "ℹ️ No active scheduled wipes to cancel.".to_string()
    } else {
        let list = cancelled
            .iter()
            .map(|(_, name)| format!("• {name}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "🛑 <b>Cancelled {} scheduled wipe(s):</b>\n{list}",
            cancelled.len()
        )
    };
    let _ = send_text(ctx.client, &ctx.config.bot_token, chat_id, &msg, None);
}

fn schedule_command_wipe(ctx: &BotContext, chat_id: i64, target: &str, delay_mins: u64) {
    let (package, name) =
        resolve_target(ctx, target).unwrap_or_else(|| (target.to_string(), display_name(target)));
    let id = ctx.scheduler.schedule(ScheduleArgs {
        client: ctx.client.clone(),
        token: ctx.config.bot_token.clone(),
        chat_id,
        tv: ctx.tv.clone(),
        package: package.clone(),
        app_name: name.clone(),
        delay_minutes: delay_mins,
    });
    let msg = format!(
        "⏳ <b>Wipe Scheduled:</b> {name}\n📦 <code>{package}</code>\n⏱️ Clearing in <b>{delay_mins} min</b>."
    );
    let keyboard = wipe_scheduled_keyboard(id);
    let _ = send_text(
        ctx.client,
        &ctx.config.bot_token,
        chat_id,
        &msg,
        Some(keyboard),
    );
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

    let Some(confirm) = confirm_target(target, &package) else {
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

pub(super) fn handle_clear_prompt_delay(ctx: &BotContext, cb: &CallbackQuery, target: &str) {
    let Some((package, label)) = resolve_target(ctx, target) else {
        let _ = answer_callback(
            ctx.client,
            &ctx.config.bot_token,
            &cb.id,
            "Menu expired — send /clear again",
        );
        return;
    };
    let _ = answer_callback(ctx.client, &ctx.config.bot_token, &cb.id, "Select delay");

    let Some(confirm) = confirm_target(target, &package) else {
        if let Some(ref msg) = cb.message {
            let hint = format!(
                "⚠️ {label} has a package name too long for a button.\nUse <code>/clear {package}</code> instead."
            );
            let _ = send_text(ctx.client, &ctx.config.bot_token, msg.chat.id, &hint, None);
        }
        return;
    };

    let text = format!(
        "⏱️ <b>When should app data be cleared?</b>\n🧹 {label}\n📦 <code>{package}</code>\n\nChoose an execution delay:"
    );
    let keyboard = wipe_delay_keyboard(&confirm);
    replace_or_send(ctx, cb, &text, Some(keyboard));
}

pub(super) fn handle_clear_execute_delayed(ctx: &BotContext, cb: &CallbackQuery, payload: &str) {
    let mut parts = payload.splitn(2, ':');
    let (Some(delay_str), Some(target)) = (parts.next(), parts.next()) else {
        return;
    };
    let delay_mins: u64 = delay_str.parse().unwrap_or(0);

    let Some((package, label)) = resolve_target(ctx, target) else {
        let _ = answer_callback(
            ctx.client,
            &ctx.config.bot_token,
            &cb.id,
            "Menu expired — send /clear again",
        );
        return;
    };

    if delay_mins == 0 {
        execute_clear_now(ctx, cb, &package, &label);
    } else {
        schedule_clear(ctx, cb, &package, &label, delay_mins);
    }
}

fn execute_clear_now(ctx: &BotContext, cb: &CallbackQuery, package: &str, label: &str) {
    match ctx.tv.clear_app_data(package) {
        Ok((pkg, name)) => {
            let _ = answer_callback(
                ctx.client,
                &ctx.config.bot_token,
                &cb.id,
                &format!("Cleared {name}!"),
            );
            let alert = format!(
                "🧹 <b>App Data Cleared:</b> {name}\n📦 <code>{pkg}</code>\n🏠 Returned to Home."
            );
            replace_or_send(ctx, cb, &alert, Some(cancelled_keyboard()));
        }
        Err(e) => {
            let _ = answer_callback(ctx.client, &ctx.config.bot_token, &cb.id, "Clear failed");
            let alert = format!("⚠️ Failed to clear data for {label}: {e}");
            replace_or_send(ctx, cb, &alert, Some(cancelled_keyboard()));
        }
    }
}

fn schedule_clear(
    ctx: &BotContext,
    cb: &CallbackQuery,
    package: &str,
    label: &str,
    delay_mins: u64,
) {
    let Some(ref msg) = cb.message else { return };
    let chat_id = msg.chat.id;

    let id = ctx.scheduler.schedule(ScheduleArgs {
        client: ctx.client.clone(),
        token: ctx.config.bot_token.clone(),
        chat_id,
        tv: ctx.tv.clone(),
        package: package.to_string(),
        app_name: label.to_string(),
        delay_minutes: delay_mins,
    });

    let _ = answer_callback(
        ctx.client,
        &ctx.config.bot_token,
        &cb.id,
        &format!("Scheduled in {delay_mins}m"),
    );
    let text = format!(
        "⏳ <b>Wipe Scheduled</b>\n🧹 <b>{label}</b>\n📦 <code>{package}</code>\n⏱️ Clearing in <b>{delay_mins} min</b>.\n\nTap below to cancel anytime before execution."
    );
    let keyboard = wipe_scheduled_keyboard(id);
    replace_or_send(ctx, cb, &text, Some(keyboard));
}

pub(super) fn handle_wipe_cancel_id(ctx: &BotContext, cb: &CallbackQuery, id_str: &str) {
    let Ok(id) = id_str.parse::<u64>() else {
        return;
    };
    if let Some((pkg, label)) = ctx.scheduler.cancel(id) {
        let _ = answer_callback(ctx.client, &ctx.config.bot_token, &cb.id, "Wipe cancelled");
        let alert = format!("🛑 <b>Scheduled wipe cancelled</b> for {label} (<code>{pkg}</code>).");
        replace_or_send(ctx, cb, &alert, Some(cancelled_keyboard()));
    } else {
        let _ = answer_callback(
            ctx.client,
            &ctx.config.bot_token,
            &cb.id,
            "Wipe completed or expired",
        );
    }
}

pub(super) fn handle_wipe_cancel(ctx: &BotContext, cb: &CallbackQuery) {
    let _ = answer_callback(ctx.client, &ctx.config.bot_token, &cb.id, "Cancelled");
    let alert = "❌ <b>Wipe cancelled.</b> No data was cleared.";
    replace_or_send(ctx, cb, alert, Some(cancelled_keyboard()));
}

fn replace_or_send(
    ctx: &BotContext,
    cb: &CallbackQuery,
    text: &str,
    keyboard: Option<serde_json::Value>,
) {
    let Some(ref msg) = cb.message else { return };
    if edit_message_text(
        ctx.client,
        &ctx.config.bot_token,
        msg.chat.id,
        msg.message_id,
        text,
        keyboard.clone(),
    )
    .is_err()
    {
        let _ = send_text(
            ctx.client,
            &ctx.config.bot_token,
            msg.chat.id,
            text,
            keyboard,
        );
    }
}

/// Build the confirmation payload, honouring Telegram's 64-byte `callback_data` limit.
fn confirm_target(target: &str, package: &str) -> Option<String> {
    const MAX_DATA: usize = 64;
    const PREFIX_LEN: usize = "wipe:5:".len();
    if package.len() + PREFIX_LEN <= MAX_DATA {
        Some(package.to_string())
    } else if target.len() + PREFIX_LEN <= MAX_DATA {
        Some(target.to_string())
    } else {
        None
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confirm_target_picks_package_when_fits() {
        let short = "org.smarttube.stable";
        assert_eq!(confirm_target("0", short), Some(short.to_string()));
    }

    #[test]
    fn test_confirm_target_falls_back_to_index_when_package_overlong() {
        let long =
            "com.very.long.package.name.that.definitely.exceeds.the.sixty.four.character.limit";
        assert_eq!(confirm_target("12", long), Some("12".to_string()));
    }
}
