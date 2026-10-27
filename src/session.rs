#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MediaMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub is_playing: bool,
    /// True when a playback session for the focused app exists, even if the app
    /// publishes no metadata (Netflix keeps `metadata: null`).
    pub has_session: bool,
    pub package: Option<String>,
    pub position_ms: Option<u64>,
    pub duration_ms: Option<u64>,
}

#[must_use]
pub fn parse_media_session(media_dump: &str, focused_package: Option<&str>) -> MediaMetadata {
    let mut current: Option<String> = None;
    let mut focused = MediaMetadata::default();
    let mut fallback = MediaMetadata::default();

    for line in media_dump.lines() {
        let trimmed = line.trim();
        if let Some(pkg) = trimmed.strip_prefix("package=") {
            let pkg = pkg.trim();
            current = (!pkg.is_empty()).then(|| pkg.to_string());
            let target = if session_belongs(current.as_deref(), focused_package) {
                &mut focused
            } else {
                &mut fallback
            };
            target.package = Some(pkg.to_string());
            target.has_session = true;
            continue;
        }
        let target = if session_belongs(current.as_deref(), focused_package) {
            &mut focused
        } else {
            &mut fallback
        };
        apply_media_line(target, trimmed);
    }

    if focused_package.is_none() || focused.has_session {
        focused
    } else if fallback.has_session {
        fallback
    } else {
        MediaMetadata::default()
    }
}

/// A dump line belongs to the focused app when we either do not know the focus,
/// or the enclosing session block names the focused package.
fn session_belongs(session_package: Option<&str>, focused_package: Option<&str>) -> bool {
    match (session_package, focused_package) {
        (Some(session), Some(focused)) => session == focused,
        (Some(_) | None, None) => true,
        (None, Some(_)) => false,
    }
}

fn apply_media_line(meta: &mut MediaMetadata, trimmed: &str) {
    if trimmed.contains("description=") {
        parse_description_line(meta, trimmed);
    } else if trimmed.contains("state=PlaybackState") {
        if trimmed.contains("state=3") {
            meta.is_playing = true;
        }
        meta.position_ms = parse_playback_field(trimmed, "position=");
        meta.duration_ms = parse_playback_field(trimmed, "duration=");
    }
}

fn parse_description_line(meta: &mut MediaMetadata, trimmed: &str) {
    let Some(pos) = trimmed.find("description=") else {
        return;
    };
    let desc = trimmed[pos + "description=".len()..].trim();
    let parts: Vec<&str> = desc.split(',').map(str::trim).collect();
    if let Some(t) = parts.first() {
        if !t.is_empty() && *t != "null" {
            meta.title = Some((*t).to_string());
        }
    }
    if let Some(a) = parts.get(1) {
        if !a.is_empty() && *a != "null" {
            meta.artist = Some((*a).to_string());
        }
    }
    // SmartTube mirrors live playback position/duration as a machine token
    // (pos=<ms>;dur=<ms>) in the metadata description field, because Android's
    // dumpsys media_session never exposes a duration= field.
    if let Some(p) = parse_token(trimmed, "pos=") {
        meta.position_ms = Some(p);
    }
    if let Some(d) = parse_token(trimmed, "dur=") {
        meta.duration_ms = Some(d);
    }
}

fn parse_playback_field(line: &str, field: &str) -> Option<u64> {
    let start = line.find(field)? + field.len();
    let rest = &line[start..];
    let end = rest.find([',', '}', ' ']).unwrap_or(rest.len());
    rest[..end].trim().parse().ok()
}

/// Render a millisecond duration as `m:ss`, or `h:mm:ss` past one hour.
#[must_use]
pub fn format_duration(ms: u64) -> String {
    let total_secs = ms / 1000;
    let h = total_secs / 3600;
    let m = (total_secs % 3600) / 60;
    let s = total_secs % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// Parse an unsigned integer that immediately follows `token` in `line`.
fn parse_token(line: &str, token: &str) -> Option<u64> {
    let start = line.find(token)? + token.len();
    let rest = &line[start..];
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_media_session_smarttube_token() {
        // Real dumpsys order: package, PlaybackState, metadata.
        let dump = "\
        package=org.smarttube.stable\n\
        active=true\n\
        state=PlaybackState {state=3, position=0, buffered position=5500, speed=1.0, updated=6598866, actions=2360191, custom actions=[], active item id=0, error=null}\n\
        metadata: size=6, description=Saturday series, Madhu Gowda, pos=4662595;dur=600000, null";
        let meta = parse_media_session(dump, Some("org.smarttube.stable"));
        assert!(meta.is_playing);
        assert!(meta.has_session);
        assert_eq!(meta.title.as_deref(), Some("Saturday series"));
        assert_eq!(meta.artist.as_deref(), Some("Madhu Gowda"));
        assert_eq!(meta.position_ms, Some(4_662_595));
        assert_eq!(meta.duration_ms, Some(600_000));
    }

    #[test]
    fn test_parse_media_session_netflix_null_metadata() {
        let dump = "\
        package=com.netflix.ninja\n\
        active=true\n\
        state=PlaybackState {state=3, position=3381070, buffered position=0, speed=1.0, updated=47975870, actions=1049466, custom actions=[], active item id=-1, error=null}\n\
        metadata: null";
        let meta = parse_media_session(dump, Some("com.netflix.ninja"));
        assert!(meta.is_playing);
        assert!(meta.has_session);
        assert_eq!(meta.title, None);
        assert_eq!(meta.position_ms, Some(3_381_070));
        assert_eq!(meta.duration_ms, None);
        assert_eq!(meta.package.as_deref(), Some("com.netflix.ninja"));
    }

    #[test]
    fn test_parse_media_session_prefers_focused_package() {
        let dump = "\
        package=org.smarttube.stable\n\
        state=PlaybackState {state=3, position=1000, speed=1.0, actions=0, custom actions=[], active item id=0, error=null}\n\
        metadata: description=Stale Video, Channel, pos=1000;dur=9000\n\
        package=com.netflix.ninja\n\
        state=PlaybackState {state=3, position=2000, buffered position=0, speed=1.0, updated=1, actions=0, custom actions=[], active item id=-1, error=null}\n\
        metadata: null";
        let meta = parse_media_session(dump, Some("com.netflix.ninja"));
        assert_eq!(meta.title, None);
        assert_eq!(meta.position_ms, Some(2000));
        assert_eq!(meta.package.as_deref(), Some("com.netflix.ninja"));
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(0), "0:00");
        assert_eq!(format_duration(65_000), "1:05");
        assert_eq!(format_duration(3_661_000), "1:01:01");
    }

    #[test]
    fn test_parse_token_variants() {
        assert_eq!(parse_token("x pos=123;dur=456", "pos="), Some(123));
        assert_eq!(parse_token("x pos=123;dur=456", "dur="), Some(456));
        assert_eq!(parse_token("no token here", "pos="), None);
    }
}
