use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;

mod adb;
mod bot;
mod config;
mod discovery;
mod media;
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
    /// Wipe all data and cache for an app (pm clear)
    Clear {
        /// App package or friendly name (e.g., hotstar, youtube, netflix, or package id)
        app: String,
    },
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
    /// Connect to TV ADB interface
    Connect,
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
    if let Some(ref title) = status.media.title {
        println!("  {} {}", "Track/Video:".bold(), title.white().bold());
    }
    if let Some(ref artist) = status.media.artist {
        println!("  {} {}", "Artist/Channel:".bold(), artist.dimmed());
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
        Commands::Status => {
            let cfg = config.unwrap_or(TerebiConfig::load()?);
            let tv = TvController::new(cfg.tv_ip, cfg.tv_port);
            print_banner();
            handle_status(&tv)
        }
        Commands::Screen { output } => {
            let cfg = config.unwrap_or(TerebiConfig::load()?);
            let tv = TvController::new(cfg.tv_ip, cfg.tv_port);
            handle_screen(&tv, &output)
        }
        Commands::Kill { app } => {
            let cfg = config.unwrap_or(TerebiConfig::load()?);
            let tv = TvController::new(cfg.tv_ip, cfg.tv_port);
            let (pkg, name) = tv.force_stop(&app)?;
            println!(
                "  {} Force-stopped {} ({})",
                "🛑".red().bold(),
                name.bold(),
                pkg.dimmed()
            );
            Ok(())
        }
        Commands::Clear { app } => {
            let cfg = config.unwrap_or(TerebiConfig::load()?);
            let tv = TvController::new(cfg.tv_ip, cfg.tv_port);
            let (pkg, name) = tv.clear_app_data(&app)?;
            println!(
                "  {} Cleared data for {} ({})",
                "🧹".green().bold(),
                name.bold(),
                pkg.dimmed()
            );
            Ok(())
        }
        Commands::Open { app } => {
            let cfg = config.unwrap_or(TerebiConfig::load()?);
            let tv = TvController::new(cfg.tv_ip, cfg.tv_port);
            let name = tv.launch_app(&app)?;
            println!("  {} Launched {}", "🚀".green().bold(), name.bold());
            Ok(())
        }
        Commands::Remote { key } => {
            let cfg = config.unwrap_or(TerebiConfig::load()?);
            let tv = TvController::new(cfg.tv_ip, cfg.tv_port);
            handle_remote(&tv, &key)
        }
        Commands::Connect => {
            let cfg = config.unwrap_or(TerebiConfig::load()?);
            let tv = TvController::new(cfg.tv_ip, cfg.tv_port);
            println!("  🔌 Connecting to TV at {}...", tv.adb.target().cyan());
            let ok = tv.adb.connect()?;
            if ok {
                println!("  {} Connected successfully.", "✔".green().bold());
            } else {
                println!(
                    "  {} Connection failed or pending authorization.",
                    "✖".red().bold()
                );
            }
            Ok(())
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
