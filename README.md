<div align="center">

# 📺 テレビ (Terebi)
### Pure-Rust Smart TV Telegram Controller & Watchdog

> **Aesthetic, ultra-responsive Smart TV monitor, force-stop watchdog, and virtual remote powered by ADB over Wi-Fi and an authenticated 2-way Telegram bot.**

<br>

[![Rust](https://img.shields.io/badge/Rust-2021%20Edition-DEA584?style=for-the-badge&logo=rust)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/Platform-Linux-FCC624?style=for-the-badge&logo=linux&logoColor=black)](https://github.com/Praveensenpai/terebi)
[![ADB](https://img.shields.io/badge/Protocol-ADB%20over%20TCP-3DDC84?style=for-the-badge&logo=android)](https://developer.android.com/tools/adb)
[![Telegram](https://img.shields.io/badge/Telegram-Bot%20API-2CA5E0?style=for-the-badge&logo=telegram)](https://core.telegram.org/bots)
[![License](https://img.shields.io/badge/License-MIT-89b4fa?style=for-the-badge)](LICENSE)

<br>

[⚡ Quick Start](#-quick-start) • [✨ Key Features](#-key-features) • [🔄 Architecture](#-architecture--workflow) • [📱 Telegram Commands](#-telegram-commands) • [🖥️ CLI Usage](#%EF%B8%8F-cli-usage) • [⚙️ Service Management](#%EF%B8%8F-service-management-systemd) • [🔧 Configuration](#-configuration)

</div>

---

> [!TIP]
> **Zero Cloud Telemetry · Zero Open Ports · 100% Local Wi-Fi Execution**  
> `terebi` communicates directly with your Smart TV using the local Android Debug Bridge (port 5555) and long-polls the official Telegram Bot API over HTTPS. Your tokens, logs, and screen captures never touch intermediate third-party servers.

---

## 🔄 Architecture & Workflow

```text
                      ┌─────────────────────────────────────────────────────────┐
                      │                 Authenticated User                      │
                      │                   (Telegram App)                        │
                      └─────────────────────────┬───────────────────────────────┘
                                                │ Telegram Bot API (Long Polling HTTPS)
                                                ▼
┌─────────────────────────────────────────────────────────────────────────────────────────────┐
│                                   📺 terebi Daemon                                          │
│                                                                                             │
│  ┌────────────────────────┐   ┌───────────────────────────┐   ┌──────────────────────────┐  │
│  │   bot::dispatcher      │   │       adb::client         │   │       media::parser      │  │
│  │                        │   │                           │   │                          │  │
│  │  • /status, /now       ├──►│  • screencap -p (PNG)     ├──►│  • dumpsys window        │  │
│  │  • /screen             │   │  • am force-stop <pkg>    │   │  • dumpsys media_session │  │
│  │  • /kill [inline keys] │   │  • pm clear <pkg>         │   │  • package friendly name │  │
│  │  • /clear [inline keys]│   │  • input keyevent <key>   │   │  • track & play state    │  │
│  │  • /remote [D-Pad]     │   │  • connect <ip>:5555      │   │                          │  │
│  └────────────────────────┘   └─────────────┬─────────────┘   └──────────────────────────┘  │
└─────────────────────────────────────────────┼───────────────────────────────────────────────┘
                                              │ Local LAN (ADB over TCP: 5555)
                                              ▼
                      ┌─────────────────────────────────────────────────────────┐
                      │              Living Room Android TV / Google TV         │
                      │                 (e.g., 192.168.1.50:5555)               │
                      └─────────────────────────────────────────────────────────┘
```

---

## ✨ Key Features

| Feature | Description |
|---|---|
| **🛑 App Murderer & Force-Stop** | Instantly kill runaway apps (`am force-stop`) with 1-tap inline buttons or `/kill [app]`, returning the TV to Home launcher. |
| **🧹 App Data Erase** | Erase all data for an app the way a fresh install starts (`pm clear`): account, settings and cache. Paginated app picker with a confirmation step. |
| **⌨️ Remote Key Logger** | Capture every remote key press from the TV's `WindowManager` channel into daily `YYYY-MM-DD.log` files with millisecond timestamps. Read them with `/logs` or `terebi logs`. |
| **🔎 On-Screen Text** | Dump the accessibility tree on demand to read visible labels via `/text` or `terebi text`. Apps that draw text without exposing accessibility nodes will return nothing. |
| **🎬 Live Playback Inspector** | Query active foreground apps, parsed media titles, artists/channels, playback state (▶️ / ⏸), and screen power. |
| **📸 Lossless Screen Capture** | Streams raw TV screenshots via `adb exec-out screencap -p` directly as full-resolution photos to your Telegram chat. |
| **🎮 Virtual D-Pad Remote** | Full interactive inline keyboard with D-pad (`⬆️`, `⬇️`, `⬅️`, `➡️`, `OK`), Back, Home, Play/Pause, and Volume buttons. |
| **🚀 Direct App Launcher** | Launch favorite TV apps (`Disney+ Hotstar`, `YouTube`, `SmartTube`, `Netflix`, `Stremio`, `VLC`, `Jellyfin`, `Prime`) with `/open [app]`. |
| **🛡️ Auto-Reconnecting Guardian** | Background ADB connection manager that automatically handles TV sleep, reboots, and network reconnects. |
| **🔒 Strict Chat Whitelist** | Ignores unauthorized users and requests; only the designated `chat_id` can trigger commands or receive alerts. |

---

## 🚀 Quick Start

### 🪄 One-Liner Magic (Recommended)

Install `terebi` in seconds with automated binary extraction and systemd service setup:

```bash
curl -fsSL https://raw.githubusercontent.com/Praveensenpai/terebi/main/install.sh | bash
```

Prebuilt tarballs ship for `x86_64` and `aarch64` Linux with matching `SHA256SUMS.txt`. On any other target the installer falls back to a local `cargo build --release` when Rust is available.

<br>

### 🛠️ Manual Build from Source

```bash
# 1. Clone & compile release binary
git clone https://github.com/Praveensenpai/terebi.git
cd terebi
cargo build --release
install -Dm 755 target/release/terebi ~/.local/bin/terebi

# 2. Run interactive setup wizard
terebi setup

# 3. Create systemd user service unit
mkdir -p ~/.config/systemd/user
cat << 'EOF' > ~/.config/systemd/user/terebi.service
[Unit]
Description=Terebi (テレビ) Smart TV Telegram Controller & Watchdog
After=network.target

[Service]
Type=simple
ExecStart=%h/.local/bin/terebi daemon
Restart=on-failure
RestartSec=10s

[Install]
WantedBy=default.target
EOF

# 4. Enable and start daemon on boot
systemctl --user daemon-reload
systemctl --user enable --now terebi.service
```

---

## 📱 Telegram Commands

Send these commands directly to your private bot:

| Command | Description | Inline Actions |
|---|---|---|
| `/status`, `/now` | View current TV app, media title, playing state, and display status | `[ 🛑 Kill App ]`, `[ 🧹 Erase All Data ]`, `[ 📸 Screen ]`, `[ 🎮 Remote ]`, `[ 🔄 Refresh ]` |
| `/screen`, `/screenshot` | Take a live screenshot of the TV screen and send as a photo | — |
| `/kill` | Open the interactive App Murderer menu | Select from Hotstar, YouTube, Netflix, Stremio, SmartTube, etc. |
| `/kill [app]` | Force stop specific app (e.g. `/kill youtube`, `/kill netflix`, `/kill yt`) | Drops to home screen after termination |
| `/clear` | Browse every installed app, most-used first | Paginated list with a confirm step before any wipe |
| `/clear [app]` | Erase ALL data for an app like a fresh install: account, settings, cache (e.g. `/clear hotstar`) | Signs the app out, resets its settings, then returns to home |
| `/logs` | Show the last 40 captured remote key presses | — |
| `/log on` \| `/log off` | Start, stop, or check remote key capture | — |
| `/text` | Read the text currently visible on screen (best effort) | — |
| `/remote` | Render virtual D-pad remote control keypad | Directional arrows, OK, Home, Back, Volume +/-, Pause, Mute |
| `/open [app]` | Launch TV application (e.g. `/open hotstar`, `/open youtube`, `/open stremio`) | — |
| `/help` | Display quick commands guide | — |

---

## 🖥️ CLI Usage

`terebi` also functions as a standalone command-line tool:

```bash
# Interactive setup wizard
terebi setup

# Start the background Telegram daemon
terebi daemon

# Inspect TV status from terminal
terebi status

# Capture TV screenshot to disk
terebi screen -o tv_preview.png

# Force stop an app via CLI
terebi kill youtube

# Erase all app data like a fresh install (pm clear)
terebi clear hotstar

# Show the newest remote-key log
terebi logs --lines 40

# Read the text currently visible on screen
terebi text

# List installed third-party apps, most recently used first
terebi apps

# Launch an app
terebi open hotstar

# Send a remote keypress (up, down, left, right, ok, back, home, pause, volup, voldown, mute)
terebi remote home

# Test connection to TV ADB
terebi connect
```

---

## ⚙️ Service Management (systemd)

`terebi` runs as an unprivileged user daemon managed by `systemd`. Useful operations:

| Action | Command |
|---|---|
| **Check service status** | `systemctl --user status terebi.service` |
| **Start service** | `systemctl --user start terebi.service` |
| **Stop service** | `systemctl --user stop terebi.service` |
| **Restart service** | `systemctl --user restart terebi.service` |
| **Enable on boot** | `systemctl --user enable terebi.service` |
| **Disable from boot** | `systemctl --user disable terebi.service` |
| **Stream live watchdog logs** | `journalctl --user -u terebi.service -f` |

---

## 🔧 Configuration

Settings are stored at `$XDG_CONFIG_HOME/terebi/config.json` (`~/.config/terebi/config.json` by default). Running setup again backs up the previous file to `config.json.bak`:

```json
{
  "bot_token": "YOUR_TELEGRAM_BOT_TOKEN",
  "chat_id": "YOUR_TELEGRAM_CHAT_ID",
  "tv_ip": "192.168.1.50",
  "tv_port": 5555,
  "friendly_name": "Living Room TV",
  "logging": {
    "enabled": false,
    "dir": "~/.local/share/terebi/logs"
  }
}
```

| Key | Type | Meaning |
|---|---|---|
| `bot_token` | string | Telegram bot token from BotFather |
| `chat_id` | string | The only chat allowed to control the TV |
| `tv_ip` | string | TV address, or `auto` to discover it on the LAN |
| `tv_port` | number | ADB port, normally `5555` |
| `friendly_name` | string | Name shown in Telegram cards |
| `logging.enabled` | bool | Start remote-key capture when the daemon boots |
| `logging.dir` | string | Log directory; `~/` is expanded. Files are `YYYY-MM-DD.log` |

`/log on` and `/log off` override `logging.enabled` at runtime without editing the file.

Run `terebi setup` at any time to reconfigure credentials interactively.

---

## 📜 License

Licensed under the [MIT License](LICENSE).  
© Praveen Senpai ([@Praveensenpai](https://github.com/Praveensenpai))
