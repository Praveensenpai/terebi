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

[⚡ Quick Start](#-quick-start) • [✨ Key Features](#-key-features) • [🔄 Architecture](#-architecture--workflow) • [📱 Telegram Commands](#-telegram-commands) • [🖥️ CLI Usage](#%EF%B8%8F-cli-usage) • [⚙️ Configuration](#%EF%B8%8F-configuration)

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
│  │  • /kill [inline keys] │   │  • input keyevent <key>   │   │  • package friendly name │  │
│  │  • /remote [D-Pad]     │   │  • connect <ip>:5555      │   │  • track & play state    │  │
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
| **🎬 Live Playback Inspector** | Query active foreground apps, parsed media titles, artists/channels, playback state (▶️ / ⏸), and screen power. |
| **📸 Lossless Screen Capture** | Streams raw TV screenshots via `adb exec-out screencap -p` directly as full-resolution photos to your Telegram chat. |
| **🎮 Virtual D-Pad Remote** | Full interactive inline keyboard with D-pad (`⬆️`, `⬇️`, `⬅️`, `➡️`, `OK`), Back, Home, Play/Pause, and Volume buttons. |
| **🚀 Direct App Launcher** | Launch favorite TV apps (`YouTube`, `SmartTube`, `Netflix`, `Stremio`, `VLC`, `Jellyfin`, `Prime`) with `/open [app]`. |
| **🛡️ Auto-Reconnecting Guardian** | Background ADB connection manager that automatically handles TV sleep, reboots, and network reconnects. |
| **🔒 Strict Chat Whitelist** | Ignores unauthorized users and requests; only the designated `chat_id` can trigger commands or receive alerts. |

---

## 🚀 Quick Start

### 🪄 One-Liner Magic (Recommended)

Install `terebi` in seconds with automated binary extraction and systemd service setup:

```bash
curl -fsSL https://raw.githubusercontent.com/Praveensenpai/terebi/main/install.sh | bash
```

<br>

### 🛠️ Manual Build from Source

```bash
git clone https://github.com/Praveensenpai/terebi.git
cd terebi
cargo build --release
install -Dm 755 target/release/terebi ~/.local/bin/terebi
```

---

## 📱 Telegram Commands

Send these commands directly to your private bot:

| Command | Description | Inline Actions |
|---|---|---|
| `/status`, `/now` | View current TV app, media title, playing state, and display status | `[ 🛑 Kill App ]`, `[ 📸 Screen ]`, `[ 🎮 Remote ]`, `[ 🔄 Refresh ]` |
| `/screen`, `/screenshot` | Take a live screenshot of the TV screen and send as a photo | — |
| `/kill` | Open the interactive App Murderer menu | Select from YouTube, Netflix, Stremio, SmartTube, etc. |
| `/kill [app]` | Force stop specific app (e.g. `/kill youtube`, `/kill netflix`, `/kill yt`) | Drops to home screen after termination |
| `/remote` | Render virtual D-pad remote control keypad | Directional arrows, OK, Home, Back, Volume +/-, Pause, Mute |
| `/open [app]` | Launch TV application (e.g. `/open youtube`, `/open stremio`) | — |
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

# Launch an app
terebi open stremio

# Send a remote keypress (up, down, left, right, ok, back, home, pause, volup, voldown, mute)
terebi remote home

# Test connection to TV ADB
terebi connect
```

---

## ⚙️ Configuration

Settings are stored at `~/.config/terebi/config.json`:

```json
{
  "bot_token": "YOUR_TELEGRAM_BOT_TOKEN",
  "chat_id": "YOUR_TELEGRAM_CHAT_ID",
  "tv_ip": "192.168.1.50",
  "tv_port": 5555,
  "friendly_name": "Living Room TV"
}
```

Run `terebi setup` at any time to reconfigure credentials interactively.

---

## 📜 License

Licensed under the [MIT License](LICENSE).  
© Praveen Senpai ([@Praveensenpai](https://github.com/Praveensenpai))
