//! Reboot and power-off flows.
//!
//! Both commands are two-step: the request records who asked and when, then a
//! confirm button executes it. Requests expire, only the original sender may
//! confirm or cancel, and a short cooldown stops repeated taps from firing
//! back-to-back ADB requests.

use std::time::{Duration, Instant};

use crate::bot::api::{answer_callback, edit_message_text, send_text, CallbackQuery, Message};
use crate::bot::ui::{cancelled_keyboard, power_keyboard, PowerAction};
use crate::tv::PowerOutcome;

use super::BotContext;

/// How long a confirmation card stays valid.
const CONFIRM_TTL: Duration = Duration::from_secs(60);
/// Minimum gap between two executed power actions.
const REPEAT_GUARD: Duration = Duration::from_secs(30);

/// An outstanding power request awaiting confirmation.
pub(super) struct Pending {
    pub action: PowerAction,
    pub user_id: i64,
    pub issued: Instant,
}

impl Pending {
    fn is_expired(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.issued) > CONFIRM_TTL
    }
}

/// `/reboot` and `/poweroff` — send the confirmation card.
pub(super) fn handle_power_command(ctx: &BotContext, msg: &Message, action: PowerAction) {
    let Some(user_id) = msg.from.as_ref().map(|user| user.id) else {
        return;
    };
    if let Some(wait) = cooldown_remaining(ctx) {
        let text = format!(
            "⏳ A power command ran {}s ago.\nTry again in {}s.",
            REPEAT_GUARD.as_secs().saturating_sub(wait.as_secs()),
            wait.as_secs()
        );
        let _ = send_text(ctx.client, &ctx.config.bot_token, msg.chat.id, &text, None);
        return;
    }

    *ctx.pending_power.borrow_mut() = Some(Pending {
        action,
        user_id,
        issued: Instant::now(),
    });

    let text = format!(
        "{} <b>{}</b>\n⚠️ This interrupts everything on the TV.\n\
        Confirm within {}s. Only you can approve it.",
        action.icon(),
        action.title(),
        CONFIRM_TTL.as_secs()
    );
    let _ = send_text(
        ctx.client,
        &ctx.config.bot_token,
        msg.chat.id,
        &text,
        Some(power_keyboard(action)),
    );
}

/// Handle `power:confirm:*` and `power:cancel` button taps.
pub(super) fn handle_power_callback(ctx: &BotContext, cb: &CallbackQuery, data: &str) {
    let Some(pending) = ctx.pending_power.borrow_mut().take() else {
        let _ = answer_callback(
            ctx.client,
            &ctx.config.bot_token,
            &cb.id,
            "No pending request — send the command again",
        );
        return;
    };

    if cb.from.id != pending.user_id {
        let _ = answer_callback(
            ctx.client,
            &ctx.config.bot_token,
            &cb.id,
            "Only the requester can answer this",
        );
        // Keep the request alive for its real owner.
        *ctx.pending_power.borrow_mut() = Some(pending);
        return;
    }

    if data == "power:cancel" {
        let _ = answer_callback(ctx.client, &ctx.config.bot_token, &cb.id, "Cancelled");
        replace_card(ctx, cb, "❌ <b>Cancelled.</b> Nothing was sent to the TV.");
        return;
    }

    if pending.is_expired(Instant::now()) {
        let _ = answer_callback(ctx.client, &ctx.config.bot_token, &cb.id, "Request expired");
        replace_card(
            ctx,
            cb,
            "⌛ <b>Request expired.</b> Send the command again if you still mean it.",
        );
        return;
    }

    if let Some(wait) = cooldown_remaining(ctx) {
        let _ = answer_callback(
            ctx.client,
            &ctx.config.bot_token,
            &cb.id,
            &format!("Already ran — wait {}s", wait.as_secs()),
        );
        return;
    }

    *ctx.power_guard.borrow_mut() = Some(Instant::now());
    execute(ctx, cb, pending.action);
}

fn execute(ctx: &BotContext, cb: &CallbackQuery, action: PowerAction) {
    let result = match action {
        PowerAction::Reboot => ctx.tv.reboot(),
        PowerAction::PowerOff => ctx.tv.power_off(),
    };

    let (toast, body) = match result {
        Ok(PowerOutcome::Rebooted) => (
            "Reboot sent",
            "🔄 <b>Reboot sent.</b>\nThe TV drops off ADB until it finishes booting.",
        ),
        Ok(PowerOutcome::PoweredOff) => (
            "Power off sent",
            "🌙 <b>Power off sent.</b>\nThe TV is shutting down.",
        ),
        Ok(PowerOutcome::Sleep) => (
            "Shutdown refused — sleep sent",
            "🌙 <b>Shutdown refused by the TV.</b>\nSent the power key instead, so the TV went to sleep.",
        ),
        Err(e) => {
            let _ = answer_callback(ctx.client, &ctx.config.bot_token, &cb.id, "Command failed");
            let text = format!("⚠️ <b>{} failed:</b> {e}", action.title());
            replace_card(ctx, cb, &text);
            return;
        }
    };

    let _ = answer_callback(ctx.client, &ctx.config.bot_token, &cb.id, toast);
    replace_card(ctx, cb, body);
}

/// Rewrite the confirmation card in place and strip its buttons.
fn replace_card(ctx: &BotContext, cb: &CallbackQuery, text: &str) {
    let Some(ref msg) = cb.message else { return };
    let _ = edit_message_text(
        ctx.client,
        &ctx.config.bot_token,
        msg.chat.id,
        msg.message_id,
        text,
        Some(cancelled_keyboard()),
    );
}

/// Remaining cooldown, or `None` when a power action may run now.
fn cooldown_remaining(ctx: &BotContext) -> Option<Duration> {
    let last = *ctx.power_guard.borrow();
    last.and_then(|at| REPEAT_GUARD.checked_sub(at.elapsed()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pending_expires_after_ttl() {
        let now = Instant::now();
        let fresh = Pending {
            action: PowerAction::Reboot,
            user_id: 1,
            issued: now,
        };
        assert!(!fresh.is_expired(now));
        assert!(fresh.is_expired(now + CONFIRM_TTL + Duration::from_secs(1)));
    }
}
