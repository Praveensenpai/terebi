//! Catalog of known Android TV and sideloaded applications.

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
        package: "org.smarttube.stable",
        icon: "📺",
    },
    AppInfo {
        name: "SmartTube (Beta)",
        package: "org.smarttube.beta",
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
        name: "Disney+ Hotstar",
        package: "in.startv.hotstar",
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
    AppInfo {
        name: "Google Play Store",
        package: "com.android.vending",
        icon: "🛍️",
    },
    AppInfo {
        name: "Projectivy Launcher",
        package: "com.spocky.projengmenu",
        icon: "🏠",
    },
    AppInfo {
        name: "Miru",
        package: "dev.praveensenpai.miru",
        icon: "🎬",
    },
    AppInfo {
        name: "LibreTube",
        package: "com.github.libretube.debug",
        icon: "📺",
    },
    AppInfo {
        name: "Findroid",
        package: "dev.jdtech.jellyfin",
        icon: "🍿",
    },
    AppInfo {
        name: "Confluence TV",
        package: "com.nlsn.confluencetv",
        icon: "📺",
    },
    AppInfo {
        name: "F-Droid",
        package: "org.fdroid.fdroid",
        icon: "📦",
    },
    AppInfo {
        name: "Tailscale",
        package: "com.tailscale.ipn",
        icon: "🛡️",
    },
    AppInfo {
        name: "Termux",
        package: "com.termux",
        icon: "💻",
    },
    AppInfo {
        name: "Termux:Boot",
        package: "com.termux.boot",
        icon: "💻",
    },
    AppInfo {
        name: "atvTools",
        package: "dev.vodik7.atvtoolsserver",
        icon: "🛠️",
    },
    AppInfo {
        name: "Moonlight",
        package: "com.limelight",
        icon: "🎮",
    },
    AppInfo {
        name: "Keylogger",
        package: "com.bshu2.androidkeylogger",
        icon: "⌨️",
    },
    AppInfo {
        name: "Crunchyroll",
        package: "com.crunchyroll.crunchyroid",
        icon: "🍥",
    },
    AppInfo {
        name: "JioCinema",
        package: "com.jio.media.ondemand",
        icon: "🎬",
    },
    AppInfo {
        name: "ZEE5",
        package: "com.graymatrix.did",
        icon: "🎬",
    },
    AppInfo {
        name: "SonyLIV",
        package: "com.sonyliv",
        icon: "🎬",
    },
    AppInfo {
        name: "Apple TV",
        package: "com.apple.atve.androidtv.appletv",
        icon: "🍎",
    },
    AppInfo {
        name: "Max",
        package: "com.wbd.stream",
        icon: "🎬",
    },
    AppInfo {
        name: "Hulu",
        package: "com.hulu.livingroomplus",
        icon: "📺",
    },
    AppInfo {
        name: "Paramount+",
        package: "com.cbs.ott",
        icon: "⛰️",
    },
    AppInfo {
        name: "Peacock",
        package: "com.peacocktv.peacockandroid",
        icon: "🦚",
    },
    AppInfo {
        name: "Tubi",
        package: "com.tubitv",
        icon: "📺",
    },
    AppInfo {
        name: "Pluto TV",
        package: "tv.pluto.android",
        icon: "📺",
    },
    AppInfo {
        name: "MX Player",
        package: "com.mxtech.videoplayer.ad",
        icon: "▶️",
    },
    AppInfo {
        name: "MX Player Pro",
        package: "com.mxtech.videoplayer.pro",
        icon: "▶️",
    },
    AppInfo {
        name: "Nova Video Player",
        package: "org.courville.nova",
        icon: "🎞️",
    },
    AppInfo {
        name: "CloudStream",
        package: "com.lagradost.cloudstream3",
        icon: "☁️",
    },
    AppInfo {
        name: "Aniyomi",
        package: "xyz.jmir.tachiyomi.mi",
        icon: "🍥",
    },
    AppInfo {
        name: "TiviMate",
        package: "ar.tvplayer.tv",
        icon: "📡",
    },
    AppInfo {
        name: "Send Files to TV",
        package: "com.yablio.sendfilestotv",
        icon: "📁",
    },
    AppInfo {
        name: "Steam Link",
        package: "com.valvesoftware.steamlink",
        icon: "🎮",
    },
];
