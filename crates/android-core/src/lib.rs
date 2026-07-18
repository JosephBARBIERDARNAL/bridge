use std::sync::{Arc, Mutex};

use futures_util::StreamExt;
use reqwest::{Method, StatusCode};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::{runtime::Runtime, task::AbortHandle};
use url::Url;

#[derive(Debug, Clone, Deserialize)]
pub struct ChatSummary {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Message {
    pub id: String,
    pub chat_id: String,
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub thinking: String,
    #[serde(default)]
    pub tool_calls: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct ChatDetail {
    pub chat: ChatSummary,
    pub messages: Vec<Message>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ChatDetailResponse {
    Nested {
        chat: ChatSummary,
        messages: Vec<Message>,
    },
    Flat {
        #[serde(flatten)]
        chat: ChatSummary,
        messages: Vec<Message>,
    },
}

impl<'de> Deserialize<'de> for ChatDetail {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let response = ChatDetailResponse::deserialize(deserializer)?;
        let (chat, messages) = match response {
            ChatDetailResponse::Nested { chat, messages }
            | ChatDetailResponse::Flat { chat, messages } => (chat, messages),
        };
        Ok(Self { chat, messages })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct HealthStatus {
    pub gateway: String,
    pub database: String,
    pub ollama: String,
    pub model: String,
    pub model_available: bool,
}

#[derive(Debug, Clone)]
pub struct StreamFailure {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

pub trait MessageStreamListener: Send + Sync {
    fn on_started(&self, user_message_id: String, assistant_message_id: String);
    fn on_thinking_delta(&self, assistant_message_id: String, text: String);
    fn on_delta(&self, assistant_message_id: String, text: String);
    fn on_tool_call(
        &self,
        assistant_message_id: String,
        call_index: u32,
        name: String,
        arguments_json: String,
    );
    fn on_tool_result(
        &self,
        assistant_message_id: String,
        call_index: u32,
        name: String,
        record_json: String,
    );
    fn on_completed(&self, message: Message);
    fn on_error(&self, error: StreamFailure);
}

#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("The gateway URL or token is invalid")]
    InvalidConfiguration,
    #[error("The API token was rejected")]
    Unauthorized,
    #[error("The requested item was not found")]
    NotFound,
    #[error("A response is already being generated")]
    Conflict,
    #[error("The Mac or Ollama is unavailable")]
    Unavailable,
    #[error("The gateway returned an invalid response")]
    InvalidResponse,
    #[error("The request failed")]
    RequestFailed,
}

#[derive(Deserialize)]
struct ApiErrorBody {
    code: String,
    message: String,
    retryable: bool,
}

#[derive(Deserialize)]
struct StartedEvent {
    user_message_id: String,
    assistant_message_id: String,
}

#[derive(Deserialize)]
struct DeltaEvent {
    message_id: String,
    text: String,
}

#[derive(Deserialize)]
struct ToolCallEvent {
    message_id: String,
    call_index: u32,
    name: String,
    arguments: String,
}

#[derive(Deserialize)]
struct ToolResultEvent {
    message_id: String,
    call_index: u32,
    name: String,
    record: String,
}

#[derive(Serialize)]
struct MessageInput<'a> {
    content: &'a str,
    web_search: bool,
}

#[derive(Serialize)]
struct RetryInput {
    web_search: bool,
}

#[derive(Serialize)]
struct RenameInput<'a> {
    title: &'a str,
}

pub struct RequestHandle {
    abort: Mutex<Option<AbortHandle>>,
}

impl RequestHandle {
    pub fn cancel(&self) {
        if let Ok(mut abort) = self.abort.lock()
            && let Some(handle) = abort.take()
        {
            handle.abort();
        }
    }
}

impl Drop for RequestHandle {
    fn drop(&mut self) {
        if let Ok(abort) = self.abort.get_mut()
            && let Some(handle) = abort.take()
        {
            handle.abort();
        }
    }
}

pub struct BridgeClient {
    base_url: Url,
    token: String,
    http: reqwest::Client,
    runtime: Runtime,
}

impl BridgeClient {
    pub fn new(base_url: String, token: String) -> Result<Self, BridgeError> {
        let mut base_url = Url::parse(&base_url).map_err(|_| BridgeError::InvalidConfiguration)?;
        if base_url.scheme() != "https" && !is_loopback(&base_url) {
            return Err(BridgeError::InvalidConfiguration);
        }
        if token.len() < 32 {
            return Err(BridgeError::InvalidConfiguration);
        }
        if !base_url.path().ends_with('/') {
            base_url.set_path(&format!("{}/", base_url.path()));
        }
        let http = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|_| BridgeError::InvalidConfiguration)?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("bridge-core")
            .build()
            .map_err(|_| BridgeError::InvalidConfiguration)?;
        Ok(Self {
            base_url,
            token,
            http,
            runtime,
        })
    }

    pub fn health(&self) -> Result<HealthStatus, BridgeError> {
        self.runtime
            .block_on(self.request_json(Method::GET, "v1/health", None::<&()>))
    }

    pub fn list_chats(&self) -> Result<Vec<ChatSummary>, BridgeError> {
        self.runtime
            .block_on(self.request_json(Method::GET, "v1/chats", None::<&()>))
    }

    pub fn create_chat(&self) -> Result<ChatSummary, BridgeError> {
        self.runtime
            .block_on(self.request_json(Method::POST, "v1/chats", None::<&()>))
    }

    pub fn get_chat(&self, chat_id: String) -> Result<ChatDetail, BridgeError> {
        self.runtime.block_on(self.request_json(
            Method::GET,
            &format!("v1/chats/{chat_id}"),
            None::<&()>,
        ))
    }

    pub fn rename_chat(&self, chat_id: String, title: String) -> Result<ChatSummary, BridgeError> {
        self.runtime.block_on(self.request_json(
            Method::PATCH,
            &format!("v1/chats/{chat_id}"),
            Some(&RenameInput { title: &title }),
        ))
    }

    pub fn delete_chat(&self, chat_id: String) -> Result<(), BridgeError> {
        self.runtime.block_on(async {
            let response = self
                .request(Method::DELETE, &format!("v1/chats/{chat_id}"))
                .send()
                .await
                .map_err(map_reqwest)?;
            if response.status().is_success() {
                Ok(())
            } else {
                Err(map_status(response.status()))
            }
        })
    }

    pub fn send_message(
        &self,
        chat_id: String,
        content: String,
        web_search: bool,
        listener: Box<dyn MessageStreamListener>,
    ) -> Arc<RequestHandle> {
        let body = serde_json::to_value(MessageInput {
            content: &content,
            web_search,
        })
        .expect("message input serializes");
        self.spawn_stream(format!("v1/chats/{chat_id}/messages"), Some(body), listener)
    }

    pub fn retry_message(
        &self,
        chat_id: String,
        user_message_id: String,
        web_search: bool,
        listener: Box<dyn MessageStreamListener>,
    ) -> Arc<RequestHandle> {
        let body = serde_json::to_value(RetryInput { web_search }).expect("retry input serializes");
        self.spawn_stream(
            format!("v1/chats/{chat_id}/messages/{user_message_id}/retry"),
            Some(body),
            listener,
        )
    }

    fn spawn_stream(
        &self,
        path: String,
        body: Option<serde_json::Value>,
        listener: Box<dyn MessageStreamListener>,
    ) -> Arc<RequestHandle> {
        let http = self.http.clone();
        let token = self.token.clone();
        let url = self
            .base_url
            .join(&path)
            .expect("validated relative API path");
        let task = self.runtime.spawn(async move {
            if let Err(error) = stream_request(http, token, url, body, listener.as_ref()).await {
                listener.on_error(StreamFailure {
                    code: "request_failed".into(),
                    message: error.to_string(),
                    retryable: !matches!(
                        error,
                        BridgeError::Unauthorized | BridgeError::InvalidConfiguration
                    ),
                });
            }
        });
        Arc::new(RequestHandle {
            abort: Mutex::new(Some(task.abort_handle())),
        })
    }

    fn request(&self, method: Method, path: &str) -> reqwest::RequestBuilder {
        self.http
            .request(
                method,
                self.base_url.join(path).expect("validated API path"),
            )
            .bearer_auth(&self.token)
    }

    async fn request_json<T: DeserializeOwned, B: Serialize + ?Sized>(
        &self,
        method: Method,
        path: &str,
        body: Option<&B>,
    ) -> Result<T, BridgeError> {
        let mut request = self.request(method, path);
        if let Some(body) = body {
            request = request.json(body);
        }
        let response = request.send().await.map_err(map_reqwest)?;
        if !response.status().is_success() {
            return Err(map_status(response.status()));
        }
        response
            .json()
            .await
            .map_err(|_| BridgeError::InvalidResponse)
    }
}

async fn stream_request(
    http: reqwest::Client,
    token: String,
    url: Url,
    body: Option<serde_json::Value>,
    listener: &dyn MessageStreamListener,
) -> Result<(), BridgeError> {
    let mut request = http
        .post(url)
        .bearer_auth(token)
        .header("accept", "text/event-stream");
    if let Some(body) = &body {
        request = request.json(body);
    }
    let response = request.send().await.map_err(map_reqwest)?;
    if !response.status().is_success() {
        return Err(map_status(response.status()));
    }

    let mut bytes = response.bytes_stream();
    let mut buffer = String::new();
    while let Some(chunk) = bytes.next().await {
        let chunk = chunk.map_err(map_reqwest)?;
        buffer.push_str(std::str::from_utf8(&chunk).map_err(|_| BridgeError::InvalidResponse)?);
        while let Some(boundary) = buffer.find("\n\n") {
            let frame = buffer[..boundary].to_owned();
            buffer.drain(..boundary + 2);
            dispatch_sse(&frame, listener)?;
        }
    }
    if !buffer.trim().is_empty() {
        dispatch_sse(&buffer, listener)?;
    }
    Ok(())
}

fn dispatch_sse(frame: &str, listener: &dyn MessageStreamListener) -> Result<(), BridgeError> {
    let mut event = "message";
    let mut data = String::new();
    for line in frame.lines() {
        if let Some(value) = line.strip_prefix("event:") {
            event = value.trim();
        }
        if let Some(value) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(value.trim_start());
        }
    }
    match event {
        "message_started" => {
            let value: StartedEvent =
                serde_json::from_str(&data).map_err(|_| BridgeError::InvalidResponse)?;
            listener.on_started(value.user_message_id, value.assistant_message_id);
        }
        "delta" => {
            let value: DeltaEvent =
                serde_json::from_str(&data).map_err(|_| BridgeError::InvalidResponse)?;
            listener.on_delta(value.message_id, value.text);
        }
        "thinking_delta" => {
            let value: DeltaEvent =
                serde_json::from_str(&data).map_err(|_| BridgeError::InvalidResponse)?;
            listener.on_thinking_delta(value.message_id, value.text);
        }
        "tool_call" => {
            let value: ToolCallEvent =
                serde_json::from_str(&data).map_err(|_| BridgeError::InvalidResponse)?;
            listener.on_tool_call(
                value.message_id,
                value.call_index,
                value.name,
                value.arguments,
            );
        }
        "tool_result" => {
            let value: ToolResultEvent =
                serde_json::from_str(&data).map_err(|_| BridgeError::InvalidResponse)?;
            listener.on_tool_result(value.message_id, value.call_index, value.name, value.record);
        }
        "message_completed" => {
            let value: Message =
                serde_json::from_str(&data).map_err(|_| BridgeError::InvalidResponse)?;
            listener.on_completed(value);
        }
        "error" => {
            let value: ApiErrorBody =
                serde_json::from_str(&data).map_err(|_| BridgeError::InvalidResponse)?;
            listener.on_error(StreamFailure {
                code: value.code,
                message: value.message,
                retryable: value.retryable,
            });
        }
        _ => {}
    }
    Ok(())
}

fn is_loopback(url: &Url) -> bool {
    matches!(
        url.host_str(),
        Some("127.0.0.1" | "localhost" | "[::1]" | "::1")
    )
}

fn map_reqwest(error: reqwest::Error) -> BridgeError {
    if error.is_connect() || error.is_timeout() {
        BridgeError::Unavailable
    } else {
        BridgeError::RequestFailed
    }
}

fn map_status(status: StatusCode) -> BridgeError {
    match status {
        StatusCode::UNAUTHORIZED => BridgeError::Unauthorized,
        StatusCode::NOT_FOUND => BridgeError::NotFound,
        StatusCode::CONFLICT => BridgeError::Conflict,
        StatusCode::SERVICE_UNAVAILABLE | StatusCode::BAD_GATEWAY | StatusCode::GATEWAY_TIMEOUT => {
            BridgeError::Unavailable
        }
        _ => BridgeError::RequestFailed,
    }
}

uniffi::include_scaffolding!("bridge_core");

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct RecordingListener {
        thinking: Mutex<Vec<(String, String)>>,
        tool_calls: Mutex<Vec<(String, u32, String, String)>>,
        tool_results: Mutex<Vec<(String, u32, String, String)>>,
    }

    impl MessageStreamListener for RecordingListener {
        fn on_started(&self, _user_message_id: String, _assistant_message_id: String) {}

        fn on_thinking_delta(&self, assistant_message_id: String, text: String) {
            self.thinking
                .lock()
                .unwrap()
                .push((assistant_message_id, text));
        }

        fn on_delta(&self, _assistant_message_id: String, _text: String) {}

        fn on_tool_call(
            &self,
            assistant_message_id: String,
            call_index: u32,
            name: String,
            arguments_json: String,
        ) {
            self.tool_calls.lock().unwrap().push((
                assistant_message_id,
                call_index,
                name,
                arguments_json,
            ));
        }

        fn on_tool_result(
            &self,
            assistant_message_id: String,
            call_index: u32,
            name: String,
            record_json: String,
        ) {
            self.tool_results.lock().unwrap().push((
                assistant_message_id,
                call_index,
                name,
                record_json,
            ));
        }

        fn on_completed(&self, _message: Message) {}

        fn on_error(&self, _error: StreamFailure) {}
    }

    const CHAT: &str = r#"
        {
            "id": "chat-1",
            "title": "History",
            "created_at": "2026-07-16T08:00:00Z",
            "updated_at": "2026-07-16T08:01:00Z"
        }
    "#;

    #[test]
    fn requires_https_away_from_loopback() {
        assert!(matches!(
            BridgeClient::new("http://example.com".into(), "x".repeat(32)),
            Err(BridgeError::InvalidConfiguration)
        ));
        assert!(BridgeClient::new("http://127.0.0.1:8787".into(), "x".repeat(32)).is_ok());
    }

    #[test]
    fn accepts_nested_chat_details() {
        let detail: ChatDetail =
            serde_json::from_str(&format!(r#"{{"chat":{CHAT},"messages":[]}}"#)).unwrap();

        assert_eq!(detail.chat.id, "chat-1");
        assert!(detail.messages.is_empty());
    }

    #[test]
    fn accepts_flat_chat_details_from_older_gateways() {
        let detail: ChatDetail = serde_json::from_str(
            r#"
            {
                "id": "chat-1",
                "title": "History",
                "created_at": "2026-07-16T08:00:00Z",
                "updated_at": "2026-07-16T08:01:00Z",
                "messages": [{
                    "id": "message-1",
                    "chat_id": "chat-1",
                    "role": "user",
                    "content": "Hello",
                    "status": "complete",
                    "created_at": "2026-07-16T08:01:00Z"
                }]
            }
            "#,
        )
        .unwrap();

        assert_eq!(detail.chat.id, "chat-1");
        assert_eq!(detail.messages.len(), 1);
        assert_eq!(detail.messages[0].content, "Hello");
        assert!(detail.messages[0].thinking.is_empty());
        assert!(detail.messages[0].tool_calls.is_empty());
    }

    #[test]
    fn dispatches_thinking_deltas() {
        let listener = RecordingListener::default();
        dispatch_sse(
            "event: thinking_delta\ndata: {\"message_id\":\"assistant-1\",\"text\":\"Considering\"}",
            &listener,
        )
        .unwrap();

        assert_eq!(
            *listener.thinking.lock().unwrap(),
            vec![("assistant-1".into(), "Considering".into())]
        );
    }

    #[test]
    fn dispatches_tool_calls() {
        let listener = RecordingListener::default();
        dispatch_sse(
            "event: tool_call\ndata: {\"message_id\":\"assistant-1\",\"call_index\":0,\"name\":\"web_search\",\"arguments\":\"{\\\"query\\\":\\\"rust\\\"}\"}",
            &listener,
        )
        .unwrap();

        assert_eq!(
            *listener.tool_calls.lock().unwrap(),
            vec![(
                "assistant-1".into(),
                0,
                "web_search".into(),
                "{\"query\":\"rust\"}".into()
            )]
        );
    }

    #[test]
    fn dispatches_tool_results() {
        let listener = RecordingListener::default();
        dispatch_sse(
            "event: tool_result\ndata: {\"message_id\":\"assistant-1\",\"call_index\":1,\"name\":\"fetch_page\",\"record\":\"{\\\"status\\\":\\\"ok\\\"}\"}",
            &listener,
        )
        .unwrap();

        assert_eq!(
            *listener.tool_results.lock().unwrap(),
            vec![(
                "assistant-1".into(),
                1,
                "fetch_page".into(),
                "{\"status\":\"ok\"}".into()
            )]
        );
    }
}
