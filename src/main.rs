use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;

mod adb;
mod apps;
mod bot;
mod config;
mod discovery;
mod input_log;
mod media;
mod session;
mod tv;

use config::TerebiConfig;
use tv::TvController;

#[derive(Parser, Debug)]
#[command(
    name = "terebi",
    version,
    about = "📺 Aesthetic, pure-Rust Smart TV Telegram controller & watchdog via ADB"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start Telegram 2-way bot daemon (default)
    Daemon,
    /// Inspect live TV status and current playback
    Status,
    /// Capture TV screen and save to image file
    Screen {
        /// Output file path (default: screen.png)
        #[arg(short, long, default_value = "screen.png")]
        output: String,
    },
    /// Force stop an app and return to Home
    Kill {
        /// App package or friendly name (e.g., youtube, netflix, or package id)
        app: String,
    },
    /// Erase all app data like a fresh install (pm clear)
    Clear {
        /// App package or friendly name (e.g., hotstar, youtube, netflix, or package id)
        app: String,
    },
    /// List installed third-party apps, most recently used first
    Apps,
    /// Launch an app on TV
    Open {
        /// App package or friendly name (e.g., youtube, netflix)
        app: String,
    },
    /// Send remote control keycode or action
    Remote {
        /// Key action: up, down, left, right, ok, back, home, pause, volup, voldown, mute
        key: String,
    },
    /// Reboot the TV over ADB
    Reboot,
    /// Power the TV off over ADB
    Poweroff,
    /// Connect to TV ADB interface
    Connect,
    /// Show the tail of the newest remote-key log
    Logs {
        /// Number of lines to show (default: 40)
        #[arg(short, long, default_value_t = 40)]
        lines: usize,
    },
    /// Read the text currently visible on the TV screen
    Text,
    /// Run interactive setup wizard
    Setup,
}

fn print_banner() {
    println!(
        "\n  {} {}",
        "📺".bold(),
        "Terebi (テレビ) Smart TV Controller".bold().cyan()
    );
    println!("  {}\n", "─".repeat(45).dimmed());
}

fn resolve_keycode(key: &str) -> &'static str {
    match key.to_lowercase().as_str() {
        "up" => "KEYCODE_DPAD_UP",
        "down" => "KEYCODE_DPAD_DOWN",
        "left" => "KEYCODE_DPAD_LEFT",
        "right" => "KEYCODE_DPAD_RIGHT",
        "ok" | "select" | "enter" => "KEYCODE_DPAD_CENTER",
        "back" => "KEYCODE_BACK",
        "pause" | "play" | "playpause" => "KEYCODE_MEDIA_PLAY_PAUSE",
        "volup" | "volume_up" => "KEYCODE_VOLUME_UP",
        "voldown" | "volume_down" => "KEYCODE_VOLUME_DOWN",
        "mute" => "KEYCODE_VOLUME_MUTE",
        _ => "KEYCODE_HOME",
    }
}

fn handle_status(tv: &TvController) -> Result<()> {
    let status = tv.get_status()?;
    let state_str = if status.media.is_playing {
        "▶️ Playing".green()
    } else {
        "⏸ Paused / Idle".yellow()
    };
    let pwr_str = if status.is_screen_on {
        "🟢 Screen ON".green()
    } else {
        "🔴 Screen OFF".red()
    };

    println!("  {} {}", "App:".bold(), status.app_name.cyan());
    println!("  {} {}", "Package:".bold(), status.package.dimmed());
    match status.media.title.as_deref() {
        Some(title) => println!("  {} {}", "Track/Video:".bold(), title.white().bold()),
        None if status.media.has_session => println!(
            "  {} {}",
            "Track/Video:".bold(),
            "(title not exposed by this app)".dimmed()
        ),
        None => {}
    }
    if let Some(ref artist) = status.media.artist {
        println!("  {} {}", "Artist/Channel:".bold(), artist.dimmed());
    }
    if let Some(pos) = status.media.position_ms {
        if pos > 0 {
            let line = status.media.duration_ms.map_or_else(
                || session::format_duration(pos),
                |dur| {
                    format!(
                        "{} / {}",
                        session::format_duration(pos),
                        session::format_duration(dur)
                    )
                },
            );
            println!("  {} {}", "Position:".bold(), line);
        }
    }
    println!("  {} {}", "State:".bold(), state_str);
    println!("  {} {}", "Display:".bold(), pwr_str);
    Ok(())
}

fn handle_screen(tv: &TvController, output: &str) -> Result<()> {
    println!("  📸 Capturing TV screen...");
    let bytes = tv.take_screenshot()?;
    std::fs::write(output, &bytes)?;
    println!(
        "  {} Screenshot saved to {}",
        "✔".green().bold(),
        output.bold()
    );
    Ok(())
}

fn handle_remote(tv: &TvController, key: &str) -> Result<()> {
    let keycode = resolve_keycode(key);
    println!("  🎮 Sending {} ({keycode})...", key.cyan());
    tv.send_key(keycode)?;
    println!("  {} Key event dispatched.", "✔".green().bold());
    Ok(())
}

fn handle_apps(tv: &TvController) -> Result<()> {
    let packages = tv.list_apps()?;
    println!("  📦 Installed third-party apps ({}):", packages.len());
    for (index, pkg) in packages.iter().enumerate() {
        println!(
            "  {}. {} {}",
            (index + 1).to_string().dimmed(),
            apps::display_name(pkg).bold(),
            format!("({pkg})").dimmed()
        );
    }
    Ok(())
}

fn handle_logs(dir: &std::path::Path, count: usize) -> Result<()> {
    let Some(path) = input_log::latest_log(dir) else {
        println!(
            "  ℹ No key logs yet in {}.\n  Start capture with `terebi daemon` after enabling logging.",
            dir.display()
        );
        return Ok(());
    };
    let name = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().to_string());
    let lines = input_log::tail(&path, count)?;
    println!("  🗒 {} ({} lines):", name.bold(), lines.len());
    for line in lines {
        println!("  {line}");
    }
    Ok(())
}

fn handle_text(tv: &TvController) -> Result<()> {
    println!("  🔎 Reading on-screen text...");
    let texts = tv.screen_texts()?;
    if texts.is_empty() {
        println!("  ℹ No text nodes found on the current screen.");
        return Ok(());
    }
    println!("  🔎 {} text node(s):", texts.len());
    for text in texts {
        println!("  • {text}");
    }
    Ok(())
}

fn build_tv(config: Option<TerebiConfig>) -> Result<TvController> {
    let cfg = match config {
        Some(cfg) => cfg,
        None => TerebiConfig::load()?,
    };
    Ok(TvController::new(cfg.tv_ip, cfg.tv_port))
}

fn handle_kill(tv: &TvController, app: &str) -> Result<()> {
    let (pkg, name) = tv.force_stop(app)?;
    println!(
        "  {} Force-stopped {} ({})",
        "🛑".red().bold(),
        name.bold(),
        pkg.dimmed()
    );
    Ok(())
}

fn handle_clear(tv: &TvController, app: &str) -> Result<()> {
    let (pkg, name) = tv.clear_app_data(app)?;
    println!(
        "  {} Cleared data for {} ({})",
        "🧹".green().bold(),
        name.bold(),
        pkg.dimmed()
    );
    Ok(())
}

fn handle_open(tv: &TvController, app: &str) -> Result<()> {
    let name = tv.launch_app(app)?;
    println!("  {} Launched {}", "🚀".green().bold(), name.bold());
    Ok(())
}

fn handle_power(tv: &TvController, power_off: bool) -> Result<()> {
    let label = if power_off { "Power-off" } else { "Reboot" };
    println!("  ⏻ Requesting {label} from TV...");
    let outcome = if power_off {
        tv.power_off()?
    } else {
        tv.reboot()?
    };
    let note = match outcome {
        tv::PowerOutcome::Rebooted => "The TV will drop off ADB while it boots.",
        tv::PowerOutcome::PoweredOff => "The TV is shutting down.",
        tv::PowerOutcome::Sleep => "Shutdown was refused; sent the power key instead.",
    };
    println!("  {} {label} requested. {note}", "✔".green().bold());
    Ok(())
}

fn handle_connect(tv: &TvController) -> Result<()> {
    println!("  🔌 Connecting to TV at {}...", tv.adb.target().cyan());
    if tv.adb.connect()? {
        println!("  {} Connected successfully.", "✔".green().bold());
    } else {
        println!(
            "  {} Connection failed or pending authorization.",
            "✖".red().bold()
        );
    }
    Ok(())
}

fn run_command(cmd: Commands, config: Option<TerebiConfig>) -> Result<()> {
    match cmd {
        Commands::Setup => {
            TerebiConfig::prompt_setup()?;
            Ok(())
        }
        Commands::Daemon => {
            let cfg = config.unwrap_or(TerebiConfig::load()?);
            print_banner();
            bot::run_bot(&cfg)
        }
        Commands::Apps => {
            print_banner();
            handle_apps(&build_tv(config)?)
        }
        Commands::Status => {
            print_banner();
            handle_status(&build_tv(config)?)
        }
        Commands::Screen { output } => handle_screen(&build_tv(config)?, &output),
        Commands::Kill { app } => handle_kill(&build_tv(config)?, &app),
        Commands::Clear { app } => handle_clear(&build_tv(config)?, &app),
        Commands::Open { app } => handle_open(&build_tv(config)?, &app),
        Commands::Remote { key } => handle_remote(&build_tv(config)?, &key),
        Commands::Reboot => handle_power(&build_tv(config)?, false),
        Commands::Poweroff => handle_power(&build_tv(config)?, true),
        Commands::Connect => handle_connect(&build_tv(config)?),
        Commands::Logs { lines } => {
            let cfg = config.unwrap_or(TerebiConfig::load()?);
            print_banner();
            handle_logs(&cfg.logging.path(), lines)
        }
        Commands::Text => {
            print_banner();
            handle_text(&build_tv(config)?)
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let loaded_config = TerebiConfig::load().ok();

    if loaded_config.is_none() && !matches!(cli.command, Some(Commands::Setup)) {
        println!("  {} Configuration not found.", "ℹ".yellow().bold());
        println!("  Starting initial setup...\n");
        let new_config = TerebiConfig::prompt_setup()?;
        return run_command(cli.command.unwrap_or(Commands::Daemon), Some(new_config));
    }

    run_command(cli.command.unwrap_or(Commands::Daemon), loaded_config)
}
