use anyhow::Result;
use reqwest::blocking::multipart::Part;
use reqwest::blocking::{multipart, Client};
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
pub struct TelegramResponse<T> {
    pub result: Option<T>,
}

#[derive(Debug, Deserialize)]
pub struct Update {
    #[serde(rename = "update_id")]
    pub id: i64,
    pub message: Option<Message>,
    pub callback_query: Option<CallbackQuery>,
}

#[derive(Debug, Deserialize)]
pub struct Message {
    pub from: Option<User>,
    pub chat: Chat,
    pub text: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    pub id: String,
    pub from: User,
    pub message: Option<MessageSummary>,
    pub data: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MessageSummary {
    pub chat: Chat,
}

#[derive(Debug, Deserialize)]
pub struct User {
    pub id: i64,
}

#[derive(Debug, Deserialize)]
pub struct Chat {
    pub id: i64,
}

pub fn fetch_updates(client: &Client, token: &str, offset: i64) -> Result<Vec<Update>> {
    let url = format!("https://api.telegram.org/bot{token}/getUpdates");
    let resp: TelegramResponse<Vec<Update>> = client
        .get(&url)
        .query(&[
            ("offset", offset.to_string()),
            ("timeout", "25".to_string()),
        ])
        .send()?
        .json()?;

    Ok(resp.result.unwrap_or_default())
}

pub fn send_text(
    client: &Client,
    token: &str,
    chat_id: i64,
    text: &str,
    reply_markup: Option<serde_json::Value>,
) -> Result<()> {
    let url = format!("https://api.telegram.org/bot{token}/sendMessage");
    let mut payload = json!({
        "chat_id": chat_id,
        "text": text,
        "parse_mode": "HTML"
    });

    if let Some(markup) = reply_markup {
        payload["reply_markup"] = markup;
    }

    client
        .post(&url)
        .json(&payload)
        .send()?
        .error_for_status()?;
    Ok(())
}

/// Telegram rejects callback answers longer than this.
const CALLBACK_MAX: usize = 200;
/// Telegram rejects photo captions longer than this.
pub const CAPTION_MAX: usize = 1024;

/// Escape the five HTML entities Telegram's `HTML` parse mode recognises.
#[must_use]
pub fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Truncate to at most `max` UTF-16 code units, which is how Telegram counts.
#[must_use]
pub fn truncate_utf16(text: &str, max: usize) -> String {
    let mut used = 0;
    let mut out = String::new();
    for ch in text.chars() {
        let cost = ch.len_utf16();
        if used + cost > max {
            break;
        }
        out.push(ch);
        used += cost;
    }
    out
}

pub fn answer_callback(client: &Client, token: &str, cb_id: &str, text: &str) -> Result<()> {
    let url = format!("https://api.telegram.org/bot{token}/answerCallbackQuery");
    let payload = json!({
        "callback_query_id": cb_id,
        "text": truncate_utf16(text, CALLBACK_MAX)
    });
    client
        .post(&url)
        .json(&payload)
        .send()?
        .error_for_status()?;
    Ok(())
}

/// Register the bot's `/` command menu so every command is discoverable.
pub fn set_my_commands(client: &Client, token: &str, commands: &serde_json::Value) -> Result<()> {
    let url = format!("https://api.telegram.org/bot{token}/setMyCommands");
    client
        .post(&url)
        .json(&json!({ "commands": commands }))
        .send()?
        .error_for_status()?;
    Ok(())
}

pub fn send_photo(
    client: &Client,
    token: &str,
    chat_id: i64,
    caption: &str,
    image_bytes: Vec<u8>,
) -> Result<()> {
    let url = format!("https://api.telegram.org/bot{token}/sendPhoto");
    let part = Part::bytes(image_bytes)
        .file_name("screen.png")
        .mime_str("image/png")
        .unwrap_or_else(|_| Part::bytes(vec![]));
    let form = multipart::Form::new()
        .text("chat_id", chat_id.to_string())
        .text("caption", truncate_utf16(caption, CAPTION_MAX))
        .text("parse_mode", "HTML")
        .part("photo", part);

    client
        .post(&url)
        .multipart(form)
        .send()?
        .error_for_status()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_html() {
        assert_eq!(escape_html("a<b>&c"), "a&lt;b&gt;&amp;c");
        assert_eq!(escape_html("plain"), "plain");
    }

    #[test]
    fn test_truncate_counts_utf16_units() {
        // Each emoji is a surrogate pair: 2 UTF-16 units, 1 char.
        assert_eq!(truncate_utf16("📺📺📺", 4), "📺📺");
        assert_eq!(truncate_utf16("abcd", 2), "ab");
        assert_eq!(truncate_utf16("abc", 10), "abc");
    }

    #[test]
    fn test_truncate_respects_caption_limit() {
        let long = "x".repeat(CAPTION_MAX + 500);
        assert_eq!(truncate_utf16(&long, CAPTION_MAX).len(), CAPTION_MAX);
    }
}
