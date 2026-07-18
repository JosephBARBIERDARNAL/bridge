use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ChatSummary {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
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
