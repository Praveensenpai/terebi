use anyhow::{Context, Result};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

fn default_port() -> u16 {
    5555
}

fn default_name() -> String {
    "Smart TV".to_string()
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct TerebiConfig {
    pub bot_token: String,
    pub chat_id: String,
    pub tv_ip: String,
    #[serde(default = "default_port")]
    pub tv_port: u16,
    #[serde(default = "default_name")]
    pub friendly_name: String,
}

impl TerebiConfig {
    #[must_use]
    pub fn config_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        Path::new(&home).join(".config/terebi/config.json")
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
        let data = serde_json::to_string_pretty(self)?;
        fs::write(&path, data)?;
        Ok(())
    }

    pub fn prompt_setup() -> Result<Self> {
        println!(
            "\n  {} {}",
            "📺".bold(),
            "Terebi (テレビ) Setup Wizard".bold().cyan()
        );
        println!("  {}\n", "─".repeat(40).dimmed());

        print!("  Enter Telegram Bot Token: ");
        io::stdout().flush()?;
        let mut token = String::new();
        io::stdin().read_line(&mut token)?;
        let bot_token = token.trim().to_string();

        print!("  Enter Authorized Telegram Chat ID: ");
        io::stdout().flush()?;
        let mut chat = String::new();
        io::stdin().read_line(&mut chat)?;
        let chat_id = chat.trim().to_string();

        print!("  Enter TV IP Address (e.g. 192.168.1.50): ");
        io::stdout().flush()?;
        let mut ip = String::new();
        io::stdin().read_line(&mut ip)?;
        let tv_ip = ip.trim().to_string();

        print!("  Enter TV Name [Living Room TV]: ");
        io::stdout().flush()?;
        let mut name = String::new();
        io::stdin().read_line(&mut name)?;
        let friendly_name = if name.trim().is_empty() {
            "Living Room TV".to_string()
        } else {
            name.trim().to_string()
        };

        if bot_token.is_empty() || chat_id.is_empty() || tv_ip.is_empty() {
            anyhow::bail!("Bot token, chat ID, and TV IP cannot be empty.");
        }

        let config = Self {
            bot_token,
            chat_id,
            tv_ip,
            tv_port: 5555,
            friendly_name,
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
        Ok(())
    }
}
