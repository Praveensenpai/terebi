use anyhow::{Context, Result};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

fn default_port() -> u16 {
    5555
}

fn default_ip() -> String {
    "auto".to_string()
}

fn default_name() -> String {
    "Smart TV".to_string()
}

fn default_log_dir() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    format!("{home}/.local/share/terebi/logs")
}

/// Remote-key capture settings.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct LogConfig {
    /// Start the logcat reader when the daemon boots.
    #[serde(default)]
    pub enabled: bool,
    /// Directory holding one `YYYY-MM-DD.log` file per TV day.
    #[serde(default = "default_log_dir")]
    pub dir: String,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            dir: default_log_dir(),
        }
    }
}

impl LogConfig {
    /// Resolved log directory, expanding a leading `~/`.
    #[must_use]
    pub fn path(&self) -> PathBuf {
        if let Some(rest) = self.dir.strip_prefix("~/") {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            return Path::new(&home).join(rest);
        }
        PathBuf::from(&self.dir)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct TerebiConfig {
    pub bot_token: String,
    pub chat_id: String,
    #[serde(default = "default_ip")]
    pub tv_ip: String,
    #[serde(default = "default_port")]
    pub tv_port: u16,
    #[serde(default = "default_name")]
    pub friendly_name: String,
    #[serde(default)]
    pub logging: LogConfig,
}

/// Resolve the config file path from the two env values it depends on.
fn resolve_config_path(xdg: Option<&str>, home: &str) -> PathBuf {
    match xdg {
        Some(dir) if !dir.is_empty() => Path::new(dir).join("terebi/config.json"),
        _ => Path::new(home).join(".config/terebi/config.json"),
    }
}

impl TerebiConfig {
    /// `$XDG_CONFIG_HOME/terebi/config.json`, falling back to `~/.config`.
    #[must_use]
    pub fn config_path() -> PathBuf {
        let xdg = std::env::var("XDG_CONFIG_HOME").ok();
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        resolve_config_path(xdg.as_deref(), &home)
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        if !path.exists() {
            anyhow::bail!(
                "Config file not found at {}. Run `terebi setup` or start interactively.",
                path.display()
            );
        }
        let data = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        serde_json::from_str(&data).with_context(|| format!("Failed to parse {}", path.display()))
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        if path.exists() {
            let backup = path.with_extension("json.bak");
            fs::copy(&path, &backup)
                .with_context(|| format!("Failed to back up {}", path.display()))?;
        }
        let data = serde_json::to_string_pretty(self)?;
        fs::write(&path, data)?;
        Ok(())
    }

    fn prompt_line(label: &str, default: Option<&str>) -> Result<String> {
        match default {
            Some(d) => print!("  Enter {label} [{d}]: "),
            None => print!("  Enter {label}: "),
        }
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let trimmed = input.trim();
        let value = match (trimmed.is_empty(), default) {
            (true, Some(d)) => d.to_string(),
            _ => trimmed.to_string(),
        };
        Ok(value)
    }

    pub fn prompt_setup() -> Result<Self> {
        println!(
            "\n  {} {}",
            "📺".bold(),
            "Terebi (テレビ) Setup Wizard".bold().cyan()
        );
        println!("  {}\n", "─".repeat(40).dimmed());

        let bot_token = Self::prompt_line("Telegram Bot Token", None)?;
        let chat_id = Self::prompt_line("Authorized Telegram Chat ID", None)?;
        let tv_ip = Self::prompt_line("TV IP Address", Some("auto"))?;
        let friendly_name = Self::prompt_line("TV Name", Some("Living Room TV"))?;

        if bot_token.is_empty() || chat_id.is_empty() {
            anyhow::bail!("Bot token and chat ID cannot be empty.");
        }

        let config = Self {
            bot_token,
            chat_id,
            tv_ip,
            tv_port: 5555,
            friendly_name,
            logging: LogConfig::default(),
        };

        config.save()?;
        println!(
            "\n  {} Configuration saved to {}\n",
            "✔".green().bold(),
            Self::config_path().display()
        );
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() -> Result<(), Box<dyn std::error::Error>> {
        let json = r#"{"bot_token": "token", "chat_id": "123", "tv_ip": "192.168.1.10"}"#;
        let cfg: TerebiConfig = serde_json::from_str(json)?;
        assert_eq!(cfg.tv_port, 5555);
        assert_eq!(cfg.friendly_name, "Smart TV");
        assert!(!cfg.logging.enabled);
        assert!(cfg.logging.dir.ends_with("/.local/share/terebi/logs"));
        Ok(())
    }

    #[test]
    fn test_logging_block_parses() -> Result<(), Box<dyn std::error::Error>> {
        let json = r#"{"bot_token": "t", "chat_id": "1", "logging": {"enabled": true, "dir": "~/tvlogs"}}"#;
        let cfg: TerebiConfig = serde_json::from_str(json)?;
        assert!(cfg.logging.enabled);
        assert!(cfg.logging.path().ends_with("tvlogs"));
        Ok(())
    }

    #[test]
    fn test_resolve_config_path_prefers_xdg() {
        let path = resolve_config_path(Some("/tmp/xdg"), "/home/u");
        assert_eq!(path, Path::new("/tmp/xdg/terebi/config.json"));
    }

    #[test]
    fn test_resolve_config_path_falls_back_to_home() {
        let expected = Path::new("/home/u/.config/terebi/config.json");
        assert_eq!(resolve_config_path(None, "/home/u"), expected);
        assert_eq!(resolve_config_path(Some(""), "/home/u"), expected);
    }
}
