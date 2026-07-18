use serde::{Deserialize, Serialize};
use sqlx_core::{from_row::FromRow, row::Row};
use sqlx_sqlite::SqliteRow;

#[derive(Debug, Clone, Serialize)]
pub struct ChatSummary {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}

impl<'r> FromRow<'r, SqliteRow> for ChatSummary {
    fn from_row(row: &'r SqliteRow) -> Result<Self, sqlx_core::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            title: row.try_get("title")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Message {
    pub id: String,
    pub chat_id: String,
    pub role: String,
    pub content: String,
    pub thinking: String,
    /// JSON array of tool-call records, or the empty string when none.
    pub tool_calls: String,
    pub status: String,
    pub created_at: String,
}

impl<'r> FromRow<'r, SqliteRow> for Message {
    fn from_row(row: &'r SqliteRow) -> Result<Self, sqlx_core::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            chat_id: row.try_get("chat_id")?,
            role: row.try_get("role")?,
            content: row.try_get("content")?,
            thinking: row.try_get("thinking")?,
            tool_calls: row.try_get("tool_calls")?,
            status: row.try_get("status")?,
            created_at: row.try_get("created_at")?,
        })
    }
}

#[derive(Debug, Serialize)]
pub struct ChatDetail {
    pub chat: ChatSummary,
    pub messages: Vec<Message>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateChat {
    pub title: String,
}

#[derive(Debug, Deserialize)]
pub struct SendMessage {
    pub content: String,
    #[serde(default)]
    pub web_search: bool,
}

#[derive(Debug, Deserialize)]
pub struct RetryInput {
    #[serde(default)]
    pub web_search: bool,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub gateway: &'static str,
    pub database: &'static str,
    pub ollama: &'static str,
    pub model: String,
    pub model_available: bool,
}

#[derive(Debug, Serialize)]
pub struct StreamStarted {
    pub user_message_id: String,
    pub assistant_message_id: String,
}

#[derive(Debug, Serialize)]
pub struct StreamDelta<'a> {
    pub message_id: &'a str,
    pub text: &'a str,
}

#[derive(Debug, Serialize)]
pub struct StreamToolCall<'a> {
    pub message_id: &'a str,
    pub call_index: u32,
    pub name: &'a str,
    /// Tool arguments as a JSON string.
    pub arguments: String,
}

#[derive(Debug, Serialize)]
pub struct StreamToolResult<'a> {
    pub message_id: &'a str,
    pub call_index: u32,
    pub name: &'a str,
    /// Full tool-call record as a JSON string.
    pub record: String,
}

#[derive(Debug, Serialize)]
pub struct StreamError {
    pub code: &'static str,
    pub message: String,
    pub retryable: bool,
}
