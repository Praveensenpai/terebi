//! Background delay scheduler for deferred app data wipes.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::bot::api::send_text;
use crate::tv::TvController;

/// Arguments needed to schedule a background delayed wipe.
pub struct ScheduleArgs {
    pub client: reqwest::blocking::Client,
    pub token: String,
    pub chat_id: i64,
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
    pub cancelled: Arc<AtomicBool>,
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
        let delay = Duration::from_secs(args.delay_minutes * 60);
        let execute_at = Instant::now() + delay;

        let wipe = ScheduledWipe {
            id,
            package: args.package.clone(),
            app_name: args.app_name.clone(),
            cancelled: Arc::clone(&cancelled),
        };

        if let Ok(mut lock) = self.wipes.lock() {
            lock.push(wipe);
        }

        let scheduler = Arc::clone(self);
        let cancel_flag = Arc::clone(&cancelled);
        let pkg_clone = args.package;
        let name_clone = args.app_name;
        let client = args.client;
        let token = args.token;
        let chat_id = args.chat_id;
        let tv = args.tv;

        std::thread::spawn(move || {
            let deadline = execute_at;
            while Instant::now() < deadline {
                if cancel_flag.load(Ordering::SeqCst) {
                    scheduler.cleanup(id);
                    return;
                }
                let remaining = deadline.saturating_duration_since(Instant::now());
                std::thread::sleep(remaining.min(Duration::from_millis(500)));
            }

            if cancel_flag.load(Ordering::SeqCst) {
                scheduler.cleanup(id);
                return;
            }

            match tv.clear_app_data(&pkg_clone) {
                Ok((pkg, name)) => {
                    let alert = format!(
                        "🧹 <b>App Data Cleared (Scheduled):</b> {name}\n📦 <code>{pkg}</code>\n🏠 Returned to Home."
                    );
                    let _ = send_text(&client, &token, chat_id, &alert, None);
                }
                Err(e) => {
                    let alert = format!("⚠️ Failed scheduled wipe for {name_clone}: {e}");
                    let _ = send_text(&client, &token, chat_id, &alert, None);
                }
            }

            scheduler.cleanup(id);
        });

        id
    }

    pub fn cancel(&self, id: u64) -> Option<(String, String)> {
        let mut lock = self.wipes.lock().ok()?;
        let pos = lock.iter().position(|w| w.id == id)?;
        let wipe = lock.remove(pos);
        wipe.cancelled.store(true, Ordering::SeqCst);
        Some((wipe.package, wipe.app_name))
    }

    pub fn cancel_all(&self) -> Vec<(String, String)> {
        let Ok(mut lock) = self.wipes.lock() else {
            return Vec::new();
        };
        let mut cancelled = Vec::new();
        for wipe in lock.drain(..) {
            wipe.cancelled.store(true, Ordering::SeqCst);
            cancelled.push((wipe.package, wipe.app_name));
        }
        cancelled
    }

    fn cleanup(&self, id: u64) {
        if let Ok(mut lock) = self.wipes.lock() {
            lock.retain(|w| w.id != id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wipe_scheduler_cancel_missing() {
        let scheduler = WipeScheduler::new();
        assert_eq!(scheduler.cancel(999), None);
        assert_eq!(scheduler.cancel_all().len(), 0);
    }

    #[test]
    fn test_wipe_scheduler_schedule_and_cancel() {
        let scheduler = Arc::new(WipeScheduler::new());
        let tv = TvController::new("127.0.0.1".to_string(), 5555);
        let client = reqwest::blocking::Client::new();

        let id = scheduler.schedule(ScheduleArgs {
            client: client.clone(),
            token: "fake_token".to_string(),
            chat_id: 12345,
            tv: tv.clone(),
            package: "com.example.test".to_string(),
            app_name: "Test App".to_string(),
            delay_minutes: 5,
        });

        assert_eq!(id, 1);
        let cancelled = scheduler.cancel(id);
        assert_eq!(
            cancelled,
            Some(("com.example.test".to_string(), "Test App".to_string()))
        );
        // Second cancel returns None
        assert_eq!(scheduler.cancel(id), None);
    }

    #[test]
    fn test_wipe_scheduler_cancel_all() {
        let scheduler = Arc::new(WipeScheduler::new());
        let tv = TvController::new("127.0.0.1".to_string(), 5555);
        let client = reqwest::blocking::Client::new();

        scheduler.schedule(ScheduleArgs {
            client: client.clone(),
            token: "fake_token".to_string(),
            chat_id: 12345,
            tv: tv.clone(),
            package: "com.example.app1".to_string(),
            app_name: "App 1".to_string(),
            delay_minutes: 1,
        });
        scheduler.schedule(ScheduleArgs {
            client,
            token: "fake_token".to_string(),
            chat_id: 12345,
            tv,
            package: "com.example.app2".to_string(),
            app_name: "App 2".to_string(),
            delay_minutes: 2,
        });

        let cancelled = scheduler.cancel_all();
        assert_eq!(cancelled.len(), 2);
        assert_eq!(scheduler.cancel_all().len(), 0);
    }
}
