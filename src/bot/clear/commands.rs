//! Text command handlers (`/clear`, `/cancelwipe`) and delay parsing.

use crate::bot::api::{edit_message_text, escape_html, send_text, send_text_returning_id};
use crate::bot::ui::{wipe_delay_keyboard, wipe_scheduled_keyboard};

use super::super::BotContext;
use super::{
    confirm_target, delay_prompt_text, format_delay_label, resolve_command_target, send_clear_menu,
    ScheduleArgs,
};

const MAX_DELAY_MINUTES: u64 = 24 * 60;

pub(in crate::bot) fn handle_clear_command(ctx: &BotContext, chat_id: i64, input: &str) {
    let parts: Vec<&str> = input.split_whitespace().collect();
    let Some(target) = parts.get(1) else {
        send_clear_menu(ctx, chat_id);
        return;
    };

    if *target == "cancel" {
        handle_clear_cancel(ctx, chat_id);
        return;
    }
    if *target == "list" || *target == "scheduled" {
        handle_clear_scheduled_list(ctx, chat_id);
        return;
    }

    let Some(delay_arg) = parts.get(2) else {
        send_command_delay_prompt(ctx, chat_id, target);
        return;
    };

    let Some(delay_mins) = parse_delay_minutes(delay_arg) else {
        let _ = send_text(
            ctx.client,
            &ctx.config.bot_token,
            chat_id,
            "⚠️ Invalid delay. Use <code>now</code>, <code>5</code>, <code>15</code>, <code>30</code>, or <code>1h</code> (60).",
            None,
        );
        return;
    };

    if delay_mins > 0 {
        schedule_command_wipe(ctx, chat_id, target, delay_mins);
    } else {
        execute_command_wipe(ctx, chat_id, target);
    }
}

fn send_command_delay_prompt(ctx: &BotContext, chat_id: i64, target: &str) {
    let (package, label) = resolve_command_target(target);
    let Some(confirm) = confirm_target(&package, &package) else {
        let hint = format!(
            "⚠️ {} has a package name too long for a button.\nUse <code>/clear {} now</code> or <code>/clear {} 15</code> instead.",
            escape_html(&label),
            escape_html(&package),
            escape_html(&package)
        );
        let _ = send_text(ctx.client, &ctx.config.bot_token, chat_id, &hint, None);
        return;
    };

    let text = delay_prompt_text(&label, &package);
    let _ = send_text(
        ctx.client,
        &ctx.config.bot_token,
        chat_id,
        &text,
        Some(wipe_delay_keyboard(&confirm)),
    );
}

fn execute_command_wipe(ctx: &BotContext, chat_id: i64, target: &str) {
    match ctx.tv.clear_app_data(target) {
        Ok((pkg, name)) => {
            let msg = format!(
                "🧹 <b>Data Cleared:</b> {}\n📦 <code>{}</code>\n🏠 Returned to Home.",
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
                    "⚠️ Failed to clear data for {}: {}",
                    escape_html(target),
                    escape_html(&e.to_string())
                ),
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
            .map(|(_, name)| format!("• {}", escape_html(name)))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "🛑 <b>Cancelled {} scheduled wipe(s):</b>\n{list}",
            cancelled.len()
        )
    };
    let _ = send_text(ctx.client, &ctx.config.bot_token, chat_id, &msg, None);
}

fn handle_clear_scheduled_list(ctx: &BotContext, chat_id: i64) {
    let items = ctx.scheduler.list();
    let msg = if items.is_empty() {
        "ℹ️ No active scheduled wipes.".to_string()
    } else {
        let lines = items
            .iter()
            .map(|(id, pkg, name, rem_secs)| {
                let mins = rem_secs.div_ceil(60);
                format!(
                    "• #{} <b>{}</b> (<code>{}</code>) — ~{} min left",
                    id,
                    escape_html(name),
                    escape_html(pkg),
                    mins
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "⏳ <b>Active Scheduled Wipes ({}):</b>\n{lines}\n\nSend <code>/cancelwipe</code> to cancel all.",
            items.len()
        )
    };
    let _ = send_text(ctx.client, &ctx.config.bot_token, chat_id, &msg, None);
}

fn schedule_command_wipe(ctx: &BotContext, chat_id: i64, target: &str, delay_mins: u64) {
    let (package, name) = resolve_command_target(target);
    let placeholder = format!(
        "⏳ <b>Wipe Scheduled:</b> {}\n📦 <code>{}</code>\n⏱️ Clearing in <b>{}</b>.",
        escape_html(&name),
        escape_html(&package),
        format_delay_label(delay_mins)
    );
    let msg_id = send_text_returning_id(
        ctx.client,
        &ctx.config.bot_token,
        chat_id,
        &placeholder,
        None,
    )
    .ok();
    let id = ctx.scheduler.schedule(ScheduleArgs {
        client: ctx.client.clone(),
        token: ctx.config.bot_token.clone(),
        chat_id,
        message_id: msg_id,
        tv: ctx.tv.clone(),
        package,
        app_name: name,
        delay_minutes: delay_mins,
    });
    if let Some(mid) = msg_id {
        let _ = edit_message_text(
            ctx.client,
            &ctx.config.bot_token,
            chat_id,
            mid,
            &placeholder,
            Some(wipe_scheduled_keyboard(id)),
        );
    }
}

/// Parse user-supplied delay tokens like `now`, `instant`, `0`, `15`, `15m`, `1h`.
#[must_use]
pub(super) fn parse_delay_minutes(raw: &str) -> Option<u64> {
    let lower = raw.trim().to_lowercase();
    if matches!(lower.as_str(), "now" | "instant" | "0") {
        return Some(0);
    }
    if let Some(hours_str) = lower.strip_suffix('h') {
        let hours: u64 = hours_str.parse().ok()?;
        let mins = hours.checked_mul(60)?;
        return (mins <= MAX_DELAY_MINUTES).then_some(mins);
    }
    let num_str = lower
        .strip_suffix("mins")
        .or_else(|| lower.strip_suffix("min"))
        .or_else(|| lower.strip_suffix('m'))
        .unwrap_or(&lower);
    let mins: u64 = num_str.parse().ok()?;
    (mins <= MAX_DELAY_MINUTES).then_some(mins)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_delay_minutes_variants() {
        assert_eq!(parse_delay_minutes("now"), Some(0));
        assert_eq!(parse_delay_minutes("instant"), Some(0));
        assert_eq!(parse_delay_minutes("0"), Some(0));
        assert_eq!(parse_delay_minutes("5"), Some(5));
        assert_eq!(parse_delay_minutes("15m"), Some(15));
        assert_eq!(parse_delay_minutes("30min"), Some(30));
        assert_eq!(parse_delay_minutes("1h"), Some(60));
        assert_eq!(parse_delay_minutes("abc"), None);
        assert_eq!(parse_delay_minutes("999999"), None);
    }
}
