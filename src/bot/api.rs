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

    client.post(&url).json(&payload).send()?;
    Ok(())
}

pub fn answer_callback(client: &Client, token: &str, cb_id: &str, text: &str) -> Result<()> {
    let url = format!("https://api.telegram.org/bot{token}/answerCallbackQuery");
    let payload = json!({
        "callback_query_id": cb_id,
        "text": text
    });
    client.post(&url).json(&payload).send()?;
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
        .text("caption", caption.to_string())
        .text("parse_mode", "HTML")
        .part("photo", part);

    client.post(&url).multipart(form).send()?;
    Ok(())
}
