//! Background delay scheduler for deferred app data wipes.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use crate::bot::api::{clear_inline_keyboard, escape_html, send_text};
use crate::tv::TvController;

/// Arguments needed to schedule a background delayed wipe.
pub struct ScheduleArgs {
    pub client: reqwest::blocking::Client,
    pub token: String,
    pub chat_id: i64,
    pub message_id: Option<i64>,
    pub tv: TvController,
    pub package: String,
    pub app_name: String,
    pub delay_minutes: u64,
}

/// A registered wipe request awaiting timed execution.
pub struct ScheduledWipe {
    pub id: u64,
    pub package: String,
    pub app_name: String,
    pub execute_at: Instant,
    pub cancelled: Arc<AtomicBool>,
    pub signal: Arc<(Mutex<bool>, Condvar)>,
}

/// Tracks background delay timers for app data wiping.
#[derive(Default)]
pub struct WipeScheduler {
    next_id: AtomicU64,
    wipes: Mutex<Vec<ScheduledWipe>>,
}

impl WipeScheduler {
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            wipes: Mutex::new(Vec::new()),
        }
    }

    pub fn schedule(self: &Arc<Self>, args: ScheduleArgs) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let cancelled = Arc::new(AtomicBool::new(false));
        let signal = Arc::new((Mutex::new(false), Condvar::new()));
        let secs = args.delay_minutes.saturating_mul(60);
        let delay = Duration::from_secs(secs);
        let execute_at = Instant::now()
            .checked_add(delay)
            .unwrap_or_else(Instant::now);

        let wipe = ScheduledWipe {
            id,
            package: args.package.clone(),
            app_name: args.app_name.clone(),
            execute_at,
            cancelled: Arc::clone(&cancelled),
            signal: Arc::clone(&signal),
        };

        if let Ok(mut lock) = self.wipes.lock() {
            lock.push(wipe);
        }

        let scheduler = Arc::clone(self);
        std::thread::spawn(move || {
            run_scheduled_wipe(&scheduler, id, execute_at, &cancelled, &signal, &args);
        });

        id
    }

    pub fn cancel(&self, id: u64) -> Option<(String, String)> {
        let mut lock = self.wipes.lock().ok()?;
        let pos = lock.iter().position(|w| w.id == id)?;
        let wipe = lock.remove(pos);
        wake_cancelled(&wipe);
        Some((wipe.package, wipe.app_name))
    }

    pub fn cancel_all(&self) -> Vec<(String, String)> {
        let Ok(mut lock) = self.wipes.lock() else {
            return Vec::new();
        };
        let mut cancelled = Vec::new();
        for wipe in lock.drain(..) {
            wake_cancelled(&wipe);
            cancelled.push((wipe.package, wipe.app_name));
        }
        cancelled
    }

    /// Snapshot active scheduled wipes as `(id, package, app_name, remaining_secs)`.
    #[must_use]
    pub fn list(&self) -> Vec<(u64, String, String, u64)> {
        let Ok(lock) = self.wipes.lock() else {
            return Vec::new();
        };
        let now = Instant::now();
        lock.iter()
            .map(|w| {
                let rem = w.execute_at.saturating_duration_since(now).as_secs();
                (w.id, w.package.clone(), w.app_name.clone(), rem)
            })
            .collect()
    }

    fn cleanup(&self, id: u64) {
        if let Ok(mut lock) = self.wipes.lock() {
            lock.retain(|w| w.id != id);
        }
    }
}

fn wake_cancelled(wipe: &ScheduledWipe) {
    wipe.cancelled.store(true, Ordering::SeqCst);
    if let Ok(mut guard) = wipe.signal.0.lock() {
        *guard = true;
        wipe.signal.1.notify_all();
    }
}

fn run_scheduled_wipe(
    scheduler: &WipeScheduler,
    id: u64,
    deadline: Instant,
    cancel_flag: &AtomicBool,
    signal: &(Mutex<bool>, Condvar),
    args: &ScheduleArgs,
) {
    if wait_until_deadline(deadline, cancel_flag, signal) {
        scheduler.cleanup(id);
        return;
    }

    if let Some(msg_id) = args.message_id {
        let _ = clear_inline_keyboard(&args.client, &args.token, args.chat_id, msg_id);
    }

    let alert = match args.tv.clear_app_data(&args.package) {
        Ok((pkg, name)) => format!(
            "🧹 <b>App Data Cleared (Scheduled):</b> {}\n📦 <code>{}</code>\n🏠 Returned to Home.",
            escape_html(&name),
            escape_html(&pkg)
        ),
        Err(e) => format!(
            "⚠️ Failed scheduled wipe for {}: {}",
            escape_html(&args.app_name),
            escape_html(&e.to_string())
        ),
    };
    let _ = send_text(&args.client, &args.token, args.chat_id, &alert, None);
    scheduler.cleanup(id);
}

fn wait_until_deadline(
    deadline: Instant,
    cancel_flag: &AtomicBool,
    signal: &(Mutex<bool>, Condvar),
) -> bool {
    let Ok(mut guard) = signal.0.lock() else {
        return true;
    };
    while !*guard && !cancel_flag.load(Ordering::SeqCst) {
        let now = Instant::now();
        if now >= deadline {
            return false;
        }
        let remaining = deadline.saturating_duration_since(now);
        let Ok((next, _)) = signal.1.wait_timeout(guard, remaining) else {
            return true;
        };
        guard = next;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_args(pkg: &str, name: &str, delay_minutes: u64) -> ScheduleArgs {
        ScheduleArgs {
            client: reqwest::blocking::Client::new(),
            token: "fake_token".to_string(),
            chat_id: 12345,
            message_id: None,
            tv: TvController::new("127.0.0.1".to_string(), 5555),
            package: pkg.to_string(),
            app_name: name.to_string(),
            delay_minutes,
        }
    }

    #[test]
    fn test_wipe_scheduler_cancel_missing() {
        let scheduler = WipeScheduler::new();
        assert_eq!(scheduler.cancel(999), None);
        assert_eq!(scheduler.cancel_all().len(), 0);
    }

    #[test]
    fn test_wipe_scheduler_schedule_list_and_cancel() {
        let scheduler = Arc::new(WipeScheduler::new());
        let id = scheduler.schedule(sample_args("com.example.test", "Test App", 15));

        assert_eq!(id, 1);
        assert_eq!(scheduler.list().len(), 1);
        let cancelled = scheduler.cancel(id);
        assert_eq!(
            cancelled,
            Some(("com.example.test".to_string(), "Test App".to_string()))
        );
        assert_eq!(scheduler.list().len(), 0);
        assert_eq!(scheduler.cancel(id), None);
    }

    #[test]
    fn test_wipe_scheduler_cancel_all() {
        let scheduler = Arc::new(WipeScheduler::new());
        scheduler.schedule(sample_args("com.example.app1", "App 1", 5));
        scheduler.schedule(sample_args("com.example.app2", "App 2", 60));

        let cancelled = scheduler.cancel_all();
        assert_eq!(cancelled.len(), 2);
        assert_eq!(scheduler.cancel_all().len(), 0);
    }
}
