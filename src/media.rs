mod catalog;
pub use catalog::KNOWN_APPS;

/// Validate an Android package identifier so it is safe to pass to `adb shell`.
#[must_use]
pub fn is_valid_package_id(package: &str) -> bool {
    if package.is_empty() || package.len() > 128 || !package.contains('.') {
        return false;
    }
    package
        .split('.')
        .all(|seg| !seg.is_empty() && seg.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
}

/// Validate an Android `KEYCODE_*` constant before passing to `input keyevent`.
#[must_use]
pub fn is_valid_keycode(keycode: &str) -> bool {
    keycode.strip_prefix("KEYCODE_").is_some_and(|rest| {
        !rest.is_empty()
            && rest
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
    })
}

#[must_use]
pub fn get_app_display(package: &str) -> (&'static str, &'static str) {
    for app in KNOWN_APPS {
        if app.package == package {
            return (app.name, app.icon);
        }
    }
    ("App", "📱")
}

#[must_use]
pub fn resolve_package(input: &str) -> Option<String> {
    let lower = input.trim().to_lowercase();
    let alias = match lower.as_str() {
        "yt" | "youtube" => Some("com.google.android.youtube.tv"),
        "cobalt" | "tizentube" => Some("io.gh.reisxd.tizentube.cobalt"),
        "smarttube" | "st" => Some("org.smarttube.stable"),
        "netflix" => Some("com.netflix.ninja"),
        "prime" | "amazon" => Some("com.amazon.amazonvideo.livingroom"),
        "stremio" => Some("com.frolo.stremio"),
        "vlc" => Some("org.videolan.vlc"),
        "jellyfin" => Some("org.jellyfin.androidtv"),
        "hotstar" | "disneyhotstar" => Some("in.startv.hotstar"),
        "playstore" | "play" => Some("com.android.vending"),
        "miru" => Some("dev.praveensenpai.miru"),
        "libretube" => Some("com.github.libretube.debug"),
        "findroid" => Some("dev.jdtech.jellyfin"),
        "fdroid" => Some("org.fdroid.fdroid"),
        "tailscale" => Some("com.tailscale.ipn"),
        "termux" => Some("com.termux"),
        "moonlight" => Some("com.limelight"),
        "crunchyroll" => Some("com.crunchyroll.crunchyroid"),
        _ => None,
    };
    if let Some(pkg) = alias {
        return Some(pkg.to_string());
    }

    for app in KNOWN_APPS {
        if app.name.eq_ignore_ascii_case(&lower) || app.package.eq_ignore_ascii_case(&lower) {
            return Some(app.package.to_string());
        }
    }

    if lower.len() >= 4 {
        for app in KNOWN_APPS {
            if app.name.to_lowercase().starts_with(&lower) {
                return Some(app.package.to_string());
            }
        }
    }

    is_valid_package_id(input.trim()).then(|| input.trim().to_string())
}

fn extract_package_from_token(token: &str) -> Option<String> {
    if !token.contains('/') {
        return None;
    }
    let clean = token.trim_matches(|c| c == '{' || c == '}' || c == ',');
    let pkg = clean.split('/').next()?;
    if !pkg.is_empty() && pkg.contains('.') {
        Some(pkg.to_string())
    } else {
        None
    }
}

fn extract_package_from_line(line: &str) -> Option<String> {
    let matches_focus = line.contains("mCurrentFocus")
        || line.contains("mFocusedApp")
        || line.contains("ResumedActivity");
    if !matches_focus {
        return None;
    }
    let start = line.find('{')?;
    let sub = &line[start..];
    sub.split_whitespace().find_map(extract_package_from_token)
}

#[must_use]
pub fn parse_focused_package(window_dump: &str) -> Option<String> {
    window_dump.lines().find_map(extract_package_from_line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_package() {
        assert_eq!(
            resolve_package("youtube"),
            Some("com.google.android.youtube.tv".to_string())
        );
        assert_eq!(
            resolve_package("yt"),
            Some("com.google.android.youtube.tv".to_string())
        );
        assert_eq!(
            resolve_package("netflix"),
            Some("com.netflix.ninja".to_string())
        );
        assert_eq!(
            resolve_package("custom.app.tv"),
            Some("custom.app.tv".to_string())
        );
        assert_eq!(
            resolve_package("hotstar"),
            Some("in.startv.hotstar".to_string())
        );
        assert_eq!(
            get_app_display("in.startv.hotstar"),
            ("Disney+ Hotstar", "🏰")
        );
    }

    #[test]
    fn test_parse_focused_package() {
        let dump = "  mCurrentFocus=Window{8d1a141 u0 com.google.android.youtube.tv/com.google.android.apps.youtube.tv.activity.ShellActivity}";
        assert_eq!(
            parse_focused_package(dump),
            Some("com.google.android.youtube.tv".to_string())
        );
    }

    #[test]
    fn test_package_and_keycode_validation() {
        assert!(is_valid_package_id("dev.praveensenpai.miru"));
        assert!(!is_valid_package_id("com.foo;rm -rf /"));
        assert!(!is_valid_package_id("nodot"));
        assert!(is_valid_keycode("KEYCODE_DPAD_UP"));
        assert!(!is_valid_keycode("KEYCODE_HOME;id"));
        assert_eq!(resolve_package("app"), None);
        assert_eq!(
            resolve_package("miru"),
            Some("dev.praveensenpai.miru".to_string())
        );
    }
}
