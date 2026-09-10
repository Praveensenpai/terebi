pub struct AppInfo {
    pub name: &'static str,
    pub package: &'static str,
    pub icon: &'static str,
}

pub const KNOWN_APPS: &[AppInfo] = &[
    AppInfo {
        name: "YouTube",
        package: "com.google.android.youtube.tv",
        icon: "📺",
    },
    AppInfo {
        name: "YouTube (Cobalt)",
        package: "io.gh.reisxd.tizentube.cobalt",
        icon: "📺",
    },
    AppInfo {
        name: "SmartTube",
        package: "com.teamsmart.videomanager.tv",
        icon: "📺",
    },
    AppInfo {
        name: "FLauncher",
        package: "me.efesser.flauncher",
        icon: "🏠",
    },
    AppInfo {
        name: "Netflix",
        package: "com.netflix.ninja",
        icon: "🍿",
    },
    AppInfo {
        name: "Prime Video",
        package: "com.amazon.amazonvideo.livingroom",
        icon: "🎬",
    },
    AppInfo {
        name: "Disney+",
        package: "com.disney.disneyplus",
        icon: "🏰",
    },
    AppInfo {
        name: "Stremio",
        package: "com.frolo.stremio",
        icon: "🎥",
    },
    AppInfo {
        name: "Stremio",
        package: "com.stremio.one",
        icon: "🎥",
    },
    AppInfo {
        name: "VLC Player",
        package: "org.videolan.vlc",
        icon: "📼",
    },
    AppInfo {
        name: "Jellyfin TV",
        package: "org.jellyfin.androidtv",
        icon: "🍿",
    },
    AppInfo {
        name: "Plex",
        package: "com.plexapp.android",
        icon: "🍿",
    },
    AppInfo {
        name: "Spotify",
        package: "com.spotify.tv.android",
        icon: "🎵",
    },
    AppInfo {
        name: "Twitch",
        package: "tv.twitch.android.app",
        icon: "🟣",
    },
    AppInfo {
        name: "Kodi",
        package: "org.xbmc.kodi",
        icon: "📦",
    },
    AppInfo {
        name: "TV Home",
        package: "com.google.android.tvlauncher",
        icon: "🏠",
    },
    AppInfo {
        name: "Google TV",
        package: "com.google.android.apps.tv.launcherx",
        icon: "🏠",
    },
];

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
    let lower = input.to_lowercase();
    if lower == "yt" || lower == "youtube" {
        return Some("com.google.android.youtube.tv".to_string());
    }
    if lower == "cobalt" || lower == "tizentube" {
        return Some("io.gh.reisxd.tizentube.cobalt".to_string());
    }
    if lower == "smarttube" {
        return Some("com.teamsmart.videomanager.tv".to_string());
    }
    if lower == "netflix" {
        return Some("com.netflix.ninja".to_string());
    }
    if lower == "prime" || lower == "amazon" {
        return Some("com.amazon.amazonvideo.livingroom".to_string());
    }
    if lower == "stremio" {
        return Some("com.frolo.stremio".to_string());
    }
    if lower == "vlc" {
        return Some("org.videolan.vlc".to_string());
    }
    if lower == "jellyfin" {
        return Some("org.jellyfin.androidtv".to_string());
    }

    for app in KNOWN_APPS {
        if app.name.to_lowercase().contains(&lower) || app.package.contains(&lower) {
            return Some(app.package.to_string());
        }
    }

    if input.contains('.') {
        Some(input.to_string())
    } else {
        None
    }
}

#[must_use]
pub fn parse_focused_package(window_dump: &str) -> Option<String> {
    for line in window_dump.lines() {
        if line.contains("mCurrentFocus")
            || line.contains("mFocusedApp")
            || line.contains("ResumedActivity")
        {
            if let Some(start) = line.find('{') {
                let sub = &line[start..];
                for token in sub.split_whitespace() {
                    if token.contains('/') {
                        let clean = token.trim_matches(|c| c == '{' || c == '}' || c == ',');
                        if let Some(pkg) = clean.split('/').next() {
                            if !pkg.is_empty() && pkg.contains('.') {
                                return Some(pkg.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MediaMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub is_playing: bool,
    pub package: Option<String>,
}

#[must_use]
pub fn parse_media_session(media_dump: &str) -> MediaMetadata {
    let mut meta = MediaMetadata::default();
    for line in media_dump.lines() {
        let trimmed = line.trim();
        if let Some(pos) = trimmed.find("description=") {
            let desc = trimmed[pos + "description=".len()..].trim();
            let parts: Vec<&str> = desc.split(',').map(str::trim).collect();
            if let Some(t) = parts.first() {
                if !t.is_empty() && *t != "null" {
                    meta.title = Some((*t).to_string());
                }
            }
            if parts.len() > 1 {
                let a = parts[1];
                if !a.is_empty() && a != "null" {
                    meta.artist = Some(a.to_string());
                }
            }
        } else if trimmed.contains("state=PlaybackState {state=3") || trimmed.contains("state=3") {
            meta.is_playing = true;
        } else if trimmed.starts_with("package=") {
            let pkg = trimmed.trim_start_matches("package=").trim();
            if !pkg.is_empty() {
                meta.package = Some(pkg.to_string());
            }
        }
    }
    meta
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
    }

    #[test]
    fn test_parse_focused_package() {
        let dump = "  mCurrentFocus=Window{8d1a141 u0 com.google.android.youtube.tv/com.google.android.apps.youtube.tv.activity.ShellActivity}";
        assert_eq!(
            parse_focused_package(dump),
            Some("com.google.android.youtube.tv".to_string())
        );
    }
}
