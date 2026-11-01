//! Installed-package listing, usage ranking, and pagination for app menus.

use std::cmp::Ordering;
use std::collections::HashMap;

use crate::media::get_app_display;

/// Number of apps rendered per Telegram menu page.
pub const PAGE_SIZE: usize = 10;

/// Parse `pm list packages` output into a sorted, deduplicated package list.
#[must_use]
pub fn parse_package_list(output: &str) -> Vec<String> {
    let mut packages: Vec<String> = output
        .lines()
        .filter_map(|line| line.trim().strip_prefix("package:"))
        .map(str::trim)
        .filter(|pkg| !pkg.is_empty())
        .map(ToString::to_string)
        .collect();
    packages.sort_unstable();
    packages.dedup();
    packages
}

/// Number of pages needed for `total` items, never less than one.
#[must_use]
pub fn page_count(total: usize, per_page: usize) -> usize {
    if per_page == 0 {
        return 1;
    }
    total.div_ceil(per_page).max(1)
}

/// Clamp `page` into range and return the clamped index with its visible slice.
#[must_use]
pub fn page_slice(items: &[String], page: usize) -> (usize, &[String]) {
    let pages = page_count(items.len(), PAGE_SIZE);
    let clamped = page.min(pages - 1);
    let start = clamped * PAGE_SIZE;
    let end = (start + PAGE_SIZE).min(items.len());
    (clamped, &items[start..end])
}

/// Friendly label for a package, falling back to the package name itself.
#[must_use]
pub fn display_name(package: &str) -> String {
    match get_app_display(package) {
        ("App", _) => package.to_string(),
        (name, _) => name.to_string(),
    }
}

/// Most-recent-use ranking extracted from `dumpsys usagestats`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct UsageRank {
    /// `YYYY-MM-DD HH:MM:SS` of the last foreground use, lexicographically sortable.
    pub last_used: String,
    /// Number of activity launches in the current daily bucket.
    pub launches: u64,
}

/// Read the quoted value following `key` (for example `time="..."`).
fn quoted<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let start = line.find(key)? + key.len();
    let rest = line.get(start..)?.strip_prefix('"')?;
    let end = rest.find('"')?;
    rest.get(..end)
}

/// Read a bare whitespace-delimited `key=value` token.
fn token_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    line.split_whitespace()
        .find_map(|token| token.strip_prefix(key))
        .map(|value| value.trim_matches('"'))
}

fn newest(current: &mut String, candidate: Option<&str>) {
    let Some(value) = candidate else { return };
    if value.starts_with("1970-") {
        return;
    }
    if value > current.as_str() {
        *current = value.to_string();
    }
}

/// Parse `dumpsys usagestats` into a per-package recency/launch ranking.
///
/// Packages that only appear in non-foreground events (standby buckets, install
/// churn) are ignored, so they rank with genuinely unused apps.
#[must_use]
pub fn parse_usage_ranking(dump: &str) -> HashMap<String, UsageRank> {
    let mut ranks: HashMap<String, UsageRank> = HashMap::new();

    for line in dump.lines() {
        let Some(package) = token_value(line, "package=") else {
            continue;
        };

        let mut last_used = String::new();
        if line.contains("type=ACTIVITY_RESUMED") {
            newest(&mut last_used, quoted(line, "time="));
        }
        newest(&mut last_used, quoted(line, "lastTimeUsed="));
        newest(&mut last_used, quoted(line, "lastTimeVisible="));
        let launches = token_value(line, "appLaunchCount=")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);

        if last_used.is_empty() && launches == 0 {
            continue;
        }

        let entry = ranks.entry(package.to_string()).or_default();
        newest(&mut entry.last_used, Some(&last_used));
        entry.launches = entry.launches.max(launches);
    }

    ranks
}

/// Order packages most-recently-used first, then by launch count, then name.
#[must_use]
pub fn sort_by_usage(mut apps: Vec<String>, usage: &HashMap<String, UsageRank>) -> Vec<String> {
    apps.sort_by(|a, b| match (usage.get(a), usage.get(b)) {
        (Some(x), Some(y)) => y
            .last_used
            .cmp(&x.last_used)
            .then_with(|| y.launches.cmp(&x.launches))
            .then_with(|| a.cmp(b)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => a.cmp(b),
    });
    apps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_package_list() {
        let output = "package:org.smarttube.stable\n\
                      package:com.netflix.ninja\n\
                      package:com.netflix.ninja\n\n";
        assert_eq!(
            parse_package_list(output),
            vec!["com.netflix.ninja", "org.smarttube.stable"]
        );
        assert_eq!(parse_package_list(""), Vec::<String>::new());
    }

    #[test]
    fn test_page_count() {
        assert_eq!(page_count(0, PAGE_SIZE), 1);
        assert_eq!(page_count(10, PAGE_SIZE), 1);
        assert_eq!(page_count(11, PAGE_SIZE), 2);
        assert_eq!(page_count(5, 0), 1);
    }

    #[test]
    fn test_page_slice_clamps() {
        let items: Vec<String> = (0..25).map(|i| format!("app{i}")).collect();
        let (page, slice) = page_slice(&items, 0);
        assert_eq!((page, slice.len()), (0, 10));
        let (page, slice) = page_slice(&items, 2);
        assert_eq!((page, slice.len()), (2, 5));
        let (page, slice) = page_slice(&items, 99);
        assert_eq!((page, slice.len()), (2, 5));
        let (page, slice) = page_slice(&[], 3);
        assert_eq!(page, 0);
        assert_eq!(slice.first(), None);
    }

    #[test]
    fn test_display_name() {
        assert_eq!(display_name("com.netflix.ninja"), "Netflix");
        assert_eq!(display_name("com.example.unknown"), "com.example.unknown");
    }

    const DUMP: &str = "\
        time=\"2026-11-01 18:00:00\" type=ACTIVITY_RESUMED package=org.smarttube.stable class=X \n\
        time=\"2026-11-01 20:30:00\" type=ACTIVITY_RESUMED package=com.amazon.amazonvideo.livingroom class=Y \n\
        package=org.smarttube.stable totalTimeUsed=\"00:10\" lastTimeUsed=\"2026-11-01 20:00:00\" appLaunchCount=7 \n\
        package=com.netflix.ninja lastTimeUsed=\"1970-01-01 05:30:00\" appLaunchCount=0 \n";

    #[test]
    fn test_parse_usage_ranking_picks_newest_and_max_launches() {
        let ranks = parse_usage_ranking(DUMP);
        let tube = ranks.get("org.smarttube.stable");
        assert_eq!(
            tube.map(|r| r.last_used.as_str()),
            Some("2026-11-01 20:00:00")
        );
        assert_eq!(tube.map(|r| r.launches), Some(7));
        let prime = ranks.get("com.amazon.amazonvideo.livingroom");
        assert_eq!(
            prime.map(|r| r.last_used.as_str()),
            Some("2026-11-01 20:30:00")
        );
        let netflix = ranks.get("com.netflix.ninja");
        assert_eq!(netflix, None);
    }

    #[test]
    fn test_parse_usage_ranking_ignores_non_foreground_events() {
        let dump = "time=\"2026-11-01 22:25:45\" type=STANDBY_BUCKET_CHANGED package=com.tailscale.ipn standbyBucket=40 reason=t flags=0x0 \n\
                    package=com.tailscale.ipn lastTimeUsed=\"1970-01-01 05:30:00\" appLaunchCount=0 \n";
        assert_eq!(parse_usage_ranking(dump).get("com.tailscale.ipn"), None);
    }

    #[test]
    fn test_sort_by_usage_recency_first() {
        let ranks = parse_usage_ranking(DUMP);
        let apps = vec![
            "com.netflix.ninja".to_string(),
            "org.smarttube.stable".to_string(),
            "com.unknown.app".to_string(),
            "com.amazon.amazonvideo.livingroom".to_string(),
        ];
        assert_eq!(
            sort_by_usage(apps, &ranks),
            vec![
                "com.amazon.amazonvideo.livingroom",
                "org.smarttube.stable",
                "com.netflix.ninja",
                "com.unknown.app",
            ]
        );
    }
}
