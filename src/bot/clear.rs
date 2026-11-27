//! App-data wipe flows: installed-app menu, delay selection, and callback execution.

use anyhow::Result;

use crate::apps::display_name;
use crate::bot::api::{answer_callback, edit_message_text, escape_html, send_text, CallbackQuery};
use crate::bot::ui::{
    cancelled_keyboard, clear_apps_keyboard, wipe_delay_keyboard, wipe_scheduled_keyboard,
};

use super::BotContext;

mod commands;
mod scheduler;

pub(super) use commands::handle_clear_command;
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
        Ok(_) => {
            let (text, keyboard) = build_clear_page(ctx, 0);
            let _ = send_text(
                ctx.client,
                &ctx.config.bot_token,
                chat_id,
                &text,
                Some(keyboard),
            );
        }
        Err(e) => {
            let _ = send_text(
                ctx.client,
                &ctx.config.bot_token,
                chat_id,
                &format!(
                    "⚠️ Failed to list installed apps: {}",
                    escape_html(&e.to_string())
                ),
                None,
            );
        }
    }
}

fn build_clear_page(ctx: &BotContext, page: usize) -> (String, serde_json::Value) {
    let apps = ctx.apps.borrow();
    let text = format!(
        "🧹 <b>Clear App Data</b>\n{} installed apps.\nTap an app to choose when to wipe its data.",
        apps.len()
    );
    let keyboard = clear_apps_keyboard(&apps, page);
    (text, keyboard)
}

pub(super) fn handle_clear_list(ctx: &BotContext, cb: &CallbackQuery, page: usize) {
    let _ = answer_callback(ctx.client, &ctx.config.bot_token, &cb.id, "");
    if ctx.apps.borrow().is_empty() {
        let _ = refresh_app_list(ctx);
    }
    let (text, keyboard) = build_clear_page(ctx, page);
    replace_or_send(ctx, cb, &text, Some(keyboard));
}

pub(super) fn handle_clear_refresh(ctx: &BotContext, cb: &CallbackQuery) {
    let _ = answer_callback(ctx.client, &ctx.config.bot_token, &cb.id, "Refreshing");
    let Some(ref msg) = cb.message else { return };
    match refresh_app_list(ctx) {
        Ok(_) => {
            let (text, keyboard) = build_clear_page(ctx, 0);
            replace_or_send(ctx, cb, &text, Some(keyboard));
        }
        Err(e) => {
            let _ = send_text(
                ctx.client,
                &ctx.config.bot_token,
                msg.chat.id,
                &format!(
                    "⚠️ Failed to list installed apps: {}",
                    escape_html(&e.to_string())
                ),
                None,
            );
        }
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
    let _ = answer_callback(
        ctx.client,
        &ctx.config.bot_token,
        &cb.id,
        "Select when to clear",
    );

    let Some(confirm) = confirm_target(target, &package) else {
        if let Some(ref msg) = cb.message {
            let hint = format!(
                "⚠️ {} has a package name too long for a button.\nUse <code>/clear {} now</code> instead.",
                escape_html(&label),
                escape_html(&package)
            );
            let _ = send_text(ctx.client, &ctx.config.bot_token, msg.chat.id, &hint, None);
        }
        return;
    };

    let text = delay_prompt_text(&label, &package);
    let keyboard = wipe_delay_keyboard(&confirm);
    replace_or_send(ctx, cb, &text, Some(keyboard));
}

pub(super) fn handle_clear_execute_delayed(ctx: &BotContext, cb: &CallbackQuery, payload: &str) {
    let mut parts = payload.splitn(2, ':');
    let (Some(delay_str), Some(target)) = (parts.next(), parts.next()) else {
        let _ = answer_callback(ctx.client, &ctx.config.bot_token, &cb.id, "Invalid request");
        return;
    };
    let Ok(delay_mins) = delay_str.parse::<u64>() else {
        let _ = answer_callback(ctx.client, &ctx.config.bot_token, &cb.id, "Invalid delay");
        return;
    };

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
                "🧹 <b>App Data Cleared:</b> {}\n📦 <code>{}</code>\n🏠 Returned to Home.",
                escape_html(&name),
                escape_html(&pkg)
            );
            replace_or_send(ctx, cb, &alert, Some(cancelled_keyboard()));
        }
        Err(e) => {
            let _ = answer_callback(ctx.client, &ctx.config.bot_token, &cb.id, "Clear failed");
            let alert = format!(
                "⚠️ Failed to clear data for {}: {}",
                escape_html(label),
                escape_html(&e.to_string())
            );
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
    let delay_label = format_delay_label(delay_mins);

    let id = ctx.scheduler.schedule(ScheduleArgs {
        client: ctx.client.clone(),
        token: ctx.config.bot_token.clone(),
        chat_id,
        message_id: Some(msg.message_id),
        tv: ctx.tv.clone(),
        package: package.to_string(),
        app_name: label.to_string(),
        delay_minutes: delay_mins,
    });

    let _ = answer_callback(
        ctx.client,
        &ctx.config.bot_token,
        &cb.id,
        &format!("Scheduled in {delay_label}"),
    );
    let text = format!(
        "⏳ <b>Wipe Scheduled</b>\n🧹 <b>{}</b>\n📦 <code>{}</code>\n⏱️ Clearing in <b>{}</b>.\n\nTap below to cancel anytime before execution.",
        escape_html(label),
        escape_html(package),
        delay_label
    );
    let keyboard = wipe_scheduled_keyboard(id);
    replace_or_send(ctx, cb, &text, Some(keyboard));
}

pub(super) fn handle_wipe_cancel_id(ctx: &BotContext, cb: &CallbackQuery, id_str: &str) {
    let Ok(id) = id_str.parse::<u64>() else {
        let _ = answer_callback(ctx.client, &ctx.config.bot_token, &cb.id, "Invalid wipe ID");
        return;
    };
    if let Some((pkg, label)) = ctx.scheduler.cancel(id) {
        let _ = answer_callback(ctx.client, &ctx.config.bot_token, &cb.id, "Wipe cancelled");
        let alert = format!(
            "🛑 <b>Scheduled wipe cancelled</b> for {} (<code>{}</code>).",
            escape_html(&label),
            escape_html(&pkg)
        );
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

pub(super) fn delay_prompt_text(label: &str, package: &str) -> String {
    format!(
        "⚠️ <b>Wipe app data?</b>\n🧹 {}\n📦 <code>{}</code>\n\nThis resets the app to a fresh-install state.\n⏱️ <b>When should app data be cleared?</b>",
        escape_html(label),
        escape_html(package)
    )
}

pub(super) fn format_delay_label(delay_mins: u64) -> String {
    if delay_mins == 60 {
        "1 hour".to_string()
    } else {
        format!("{delay_mins} min")
    }
}

/// Build the confirmation payload, honouring Telegram's 64-byte `callback_data` limit.
pub(super) fn confirm_target(target: &str, package: &str) -> Option<String> {
    const MAX_DATA: usize = 64;
    const PREFIX_LEN: usize = "wipe:60:".len();
    if package.len() + PREFIX_LEN <= MAX_DATA {
        Some(package.to_string())
    } else if target.len() + PREFIX_LEN <= MAX_DATA {
        Some(target.to_string())
    } else {
        None
    }
}

pub(super) fn resolve_command_target(target: &str) -> (String, String) {
    let pkg = crate::media::resolve_package(target).unwrap_or_else(|| target.to_string());
    (pkg.clone(), display_name(&pkg))
}

/// Resolve a callback target, which is either a cache index, `"current"`, or a package name.
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
    fn test_confirm_target_rejects_57_byte_package_for_wipe_60() {
        let len_57 = "a".repeat(57);
        assert_eq!(confirm_target("12", &len_57), Some("12".to_string()));
    }
}
