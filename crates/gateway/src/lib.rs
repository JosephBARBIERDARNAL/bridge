mod config;
mod error;
mod model;

use std::{
    collections::HashSet,
    convert::Infallible,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

use async_stream::stream;
use axum::{
    Json, Router,
    extract::{Path as AxumPath, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{
        IntoResponse, Response, Sse,
        sse::{Event, KeepAlive},
    },
    routing::{get, post},
};
use chrono::Utc;
use error::ApiError;
use futures_util::StreamExt;
use model::*;
use ollama_rs::{
    Ollama,
    generation::chat::{ChatMessage, MessageRole, request::ChatMessageRequest},
};
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use subtle::ConstantTimeEq;
use uuid::Uuid;

pub use config::Config;

#[derive(Clone)]
pub struct AppState {
    pool: SqlitePool,
    ollama: Ollama,
    model: String,
    token: Arc<str>,
    active_chats: Arc<Mutex<HashSet<String>>>,
}

struct ActiveGeneration {
    chat_id: String,
    active: Arc<Mutex<HashSet<String>>>,
}

impl Drop for ActiveGeneration {
    fn drop(&mut self) {
        if let Ok(mut active) = self.active.lock() {
            active.remove(&self.chat_id);
        }
    }
}

impl AppState {
    pub async fn connect(config: &Config) -> anyhow::Result<Self> {
        if let Some(parent) = config.database_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let options = SqliteConnectOptions::new()
            .filename(&config.database_path)
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        sqlx::query("UPDATE messages SET status = 'failed' WHERE status = 'streaming'")
            .execute(&pool)
            .await?;

        Ok(Self {
            pool,
            ollama: Ollama::builder()
                .host(config.ollama_host.clone())
                .port(config.ollama_port)
                .build(),
            model: config.model.clone(),
            token: Arc::from(config.token.as_str()),
            active_chats: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    pub async fn for_test(database_path: &Path, token: &str) -> anyhow::Result<Self> {
        let config = Config {
            bind: "127.0.0.1:0".parse().unwrap(),
            database_path: database_path.to_owned(),
            ollama_host: "http://127.0.0.1".into(),
            ollama_port: 9,
            model: "gemma4:26b".into(),
            token: token.into(),
        };
        Self::connect(&config).await
    }
}

pub fn router(state: AppState) -> Router {
    let protected = Router::new()
        .route("/health", get(health))
        .route("/chats", get(list_chats).post(create_chat))
        .route(
            "/chats/{chat_id}",
            get(get_chat).patch(update_chat).delete(delete_chat),
        )
        .route("/chats/{chat_id}/messages", post(send_message))
        .route(
            "/chats/{chat_id}/messages/{message_id}/retry",
            post(retry_message),
        )
        .route_layer(middleware::from_fn_with_state(state.clone(), authenticate));

    Router::new().nest("/v1", protected).with_state(state)
}

async fn authenticate(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let provided = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    let authorized = provided.is_some_and(|value| {
        value.len() == state.token.len()
            && bool::from(value.as_bytes().ct_eq(state.token.as_bytes()))
    });
    if !authorized {
        return ApiError {
            status: StatusCode::UNAUTHORIZED,
            code: "unauthorized",
            message: "A valid Bridge API token is required".into(),
            retryable: false,
        }
        .into_response();
    }
    next.run(request).await
}

async fn health(State(state): State<AppState>) -> Result<Json<HealthResponse>, ApiError> {
    sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    match state.ollama.list_local_models().await {
        Ok(models) => {
            let available = models.iter().any(|item| item.name == state.model);
            Ok(Json(HealthResponse {
                gateway: "ok",
                database: "ok",
                ollama: "ok",
                model: state.model,
                model_available: available,
            }))
        }
        Err(error) => Ok(Json(HealthResponse {
            gateway: "ok",
            database: "ok",
            ollama: "unavailable",
            model: format!("{} ({error})", state.model),
            model_available: false,
        })),
    }
}

async fn list_chats(State(state): State<AppState>) -> Result<Json<Vec<ChatSummary>>, ApiError> {
    let chats = sqlx::query_as::<_, ChatSummary>(
        "SELECT id, title, created_at, updated_at FROM chats ORDER BY updated_at DESC",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::internal)?;
    Ok(Json(chats))
}

async fn create_chat(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<ChatSummary>), ApiError> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO chats(id, title, created_at, updated_at) VALUES (?, 'New chat', ?, ?)",
    )
    .bind(&id)
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await
    .map_err(ApiError::internal)?;
    Ok((
        StatusCode::CREATED,
        Json(ChatSummary {
            id,
            title: "New chat".into(),
            created_at: now.clone(),
            updated_at: now,
        }),
    ))
}

async fn get_chat(
    State(state): State<AppState>,
    AxumPath(chat_id): AxumPath<String>,
) -> Result<Json<ChatDetail>, ApiError> {
    let chat = fetch_chat(&state.pool, &chat_id).await?;
    let messages = sqlx::query_as::<_, Message>("SELECT id, chat_id, role, content, status, created_at FROM messages WHERE chat_id = ? ORDER BY created_at, id")
        .bind(&chat_id).fetch_all(&state.pool).await.map_err(ApiError::internal)?;
    Ok(Json(ChatDetail { chat, messages }))
}

async fn update_chat(
    State(state): State<AppState>,
    AxumPath(chat_id): AxumPath<String>,
    Json(input): Json<UpdateChat>,
) -> Result<Json<ChatSummary>, ApiError> {
    let title = input.title.trim();
    if title.is_empty() || title.chars().count() > 120 {
        return Err(ApiError::bad_request(
            "Title must contain between 1 and 120 characters",
        ));
    }
    let result = sqlx::query("UPDATE chats SET title = ?, updated_at = ? WHERE id = ?")
        .bind(title)
        .bind(Utc::now().to_rfc3339())
        .bind(&chat_id)
        .execute(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("Chat"));
    }
    Ok(Json(fetch_chat(&state.pool, &chat_id).await?))
}

async fn delete_chat(
    State(state): State<AppState>,
    AxumPath(chat_id): AxumPath<String>,
) -> Result<StatusCode, ApiError> {
    let result = sqlx::query("DELETE FROM chats WHERE id = ?")
        .bind(&chat_id)
        .execute(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("Chat"));
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn send_message(
    State(state): State<AppState>,
    AxumPath(chat_id): AxumPath<String>,
    Json(input): Json<SendMessage>,
) -> Result<Response, ApiError> {
    let content = input.content.trim();
    if content.is_empty() || content.chars().count() > 32_000 {
        return Err(ApiError::bad_request(
            "Message must contain between 1 and 32000 characters",
        ));
    }
    fetch_chat(&state.pool, &chat_id).await?;
    let user_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let mut transaction = state.pool.begin().await.map_err(ApiError::internal)?;
    sqlx::query("INSERT INTO messages(id, chat_id, role, content, status, created_at) VALUES (?, ?, 'user', ?, 'complete', ?)")
        .bind(&user_id).bind(&chat_id).bind(content).bind(&now).execute(&mut *transaction).await.map_err(ApiError::internal)?;
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE chat_id = ? AND role = 'user'")
            .bind(&chat_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(ApiError::internal)?;
    if count == 1 {
        sqlx::query("UPDATE chats SET title = ?, updated_at = ? WHERE id = ?")
            .bind(title_from(content))
            .bind(&now)
            .bind(&chat_id)
            .execute(&mut *transaction)
            .await
            .map_err(ApiError::internal)?;
    } else {
        sqlx::query("UPDATE chats SET updated_at = ? WHERE id = ?")
            .bind(&now)
            .bind(&chat_id)
            .execute(&mut *transaction)
            .await
            .map_err(ApiError::internal)?;
    }
    transaction.commit().await.map_err(ApiError::internal)?;
    start_generation(state, chat_id, user_id).await
}

async fn retry_message(
    State(state): State<AppState>,
    AxumPath((chat_id, message_id)): AxumPath<(String, String)>,
) -> Result<Response, ApiError> {
    let role =
        sqlx::query_scalar::<_, String>("SELECT role FROM messages WHERE id = ? AND chat_id = ?")
            .bind(&message_id)
            .bind(&chat_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(ApiError::internal)?;
    match role.as_deref() {
        Some("user") => start_generation(state, chat_id, message_id).await,
        Some(_) => Err(ApiError::bad_request("Only user messages can be retried")),
        None => Err(ApiError::not_found("Message")),
    }
}

async fn start_generation(
    state: AppState,
    chat_id: String,
    user_id: String,
) -> Result<Response, ApiError> {
    let guard = {
        let mut active = state.active_chats.lock().map_err(ApiError::internal)?;
        if !active.insert(chat_id.clone()) {
            return Err(ApiError::conflict(
                "This chat already has an active response",
            ));
        }
        ActiveGeneration {
            chat_id: chat_id.clone(),
            active: state.active_chats.clone(),
        }
    };

    let rows = sqlx::query_as::<_, Message>("SELECT id, chat_id, role, content, status, created_at FROM messages WHERE chat_id = ? AND (role = 'user' OR status = 'complete') ORDER BY created_at, id")
        .bind(&chat_id).fetch_all(&state.pool).await.map_err(ApiError::internal)?;
    let mut history = Vec::new();
    for message in rows {
        history.push(if message.role == "user" {
            ChatMessage::user(message.content)
        } else {
            ChatMessage::assistant(message.content)
        });
        if message.id == user_id {
            break;
        }
    }
    if history
        .last()
        .is_none_or(|message| message.role != MessageRole::User)
    {
        return Err(ApiError::bad_request(
            "The retry message is not part of the active chat history",
        ));
    }

    let request = ChatMessageRequest::new(state.model.clone(), history);
    let mut ollama_stream = state
        .ollama
        .send_chat_messages_stream(request)
        .await
        .map_err(|error| ApiError::unavailable(error.to_string()))?;
    let assistant_id = Uuid::new_v4().to_string();
    let created_at = Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO messages(id, chat_id, role, content, status, created_at) VALUES (?, ?, 'assistant', '', 'streaming', ?)")
        .bind(&assistant_id).bind(&chat_id).bind(&created_at).execute(&state.pool).await.map_err(ApiError::internal)?;

    let pool = state.pool.clone();
    let stream_user_id = user_id.clone();
    let stream_assistant_id = assistant_id.clone();
    let output = stream! {
        let _guard = guard;
        yield Ok::<Event, Infallible>(Event::default().event("message_started").json_data(StreamStarted { user_message_id: stream_user_id, assistant_message_id: stream_assistant_id.clone() }).unwrap());
        let mut content = String::new();
        let mut failed = None;
        while let Some(item) = ollama_stream.next().await {
            match item {
                Ok(response) => {
                    let delta = response.message.content;
                    if !delta.is_empty() {
                        content.push_str(&delta);
                        yield Ok(Event::default().event("delta").json_data(StreamDelta { message_id: &stream_assistant_id, text: &delta }).unwrap());
                    }
                }
                Err(()) => { failed = Some("Ollama closed the response stream unexpectedly".to_owned()); break; }
            }
        }
        if let Some(message) = failed {
            let _ = sqlx::query("UPDATE messages SET content = ?, status = 'failed' WHERE id = ?").bind(&content).bind(&stream_assistant_id).execute(&pool).await;
            yield Ok(Event::default().event("error").json_data(StreamError { code: "ollama_stream_error", message, retryable: true }).unwrap());
        } else {
            let _ = sqlx::query("UPDATE messages SET content = ?, status = 'complete' WHERE id = ?").bind(&content).bind(&stream_assistant_id).execute(&pool).await;
            let _ = sqlx::query("UPDATE chats SET updated_at = ? WHERE id = ?").bind(Utc::now().to_rfc3339()).bind(&chat_id).execute(&pool).await;
            if let Ok(Some(message)) = sqlx::query_as::<_, Message>("SELECT id, chat_id, role, content, status, created_at FROM messages WHERE id = ?").bind(&stream_assistant_id).fetch_optional(&pool).await {
                yield Ok(Event::default().event("message_completed").json_data(message).unwrap());
            }
        }
    };
    Ok(Sse::new(output)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keep-alive"),
        )
        .into_response())
}

async fn fetch_chat(pool: &SqlitePool, chat_id: &str) -> Result<ChatSummary, ApiError> {
    sqlx::query_as::<_, ChatSummary>(
        "SELECT id, title, created_at, updated_at FROM chats WHERE id = ?",
    )
    .bind(chat_id)
    .fetch_optional(pool)
    .await
    .map_err(ApiError::internal)?
    .ok_or_else(|| ApiError::not_found("Chat"))
}

fn title_from(content: &str) -> String {
    let compact = content.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut title = compact.chars().take(60).collect::<String>();
    if compact.chars().count() > 60 {
        title.push('…');
    }
    title
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use tempfile::tempdir;
    use tower::ServiceExt;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef";

    async fn app() -> Router {
        let directory = tempdir().unwrap().keep();
        router(
            AppState::for_test(&directory.join("test.db"), TOKEN)
                .await
                .unwrap(),
        )
    }

    fn authorized(method: &str, uri: &str, body: Body) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", format!("Bearer {TOKEN}"))
            .header("content-type", "application/json")
            .body(body)
            .unwrap()
    }

    #[tokio::test]
    async fn rejects_missing_token() {
        let response = app()
            .await
            .oneshot(
                Request::builder()
                    .uri("/v1/chats")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn creates_lists_renames_and_deletes_chat() {
        let app = app().await;
        let response = app
            .clone()
            .oneshot(authorized("POST", "/v1/chats", Body::empty()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let created: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let id = created["id"].as_str().unwrap();

        let response = app
            .clone()
            .oneshot(authorized("GET", &format!("/v1/chats/{id}"), Body::empty()))
            .await
            .unwrap();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let detail: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(detail["chat"]["id"], id);
        assert_eq!(detail["messages"], serde_json::json!([]));

        let response = app
            .clone()
            .oneshot(authorized(
                "PATCH",
                &format!("/v1/chats/{id}"),
                Body::from(r#"{"title":"Renamed"}"#),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .clone()
            .oneshot(authorized("GET", "/v1/chats", Body::empty()))
            .await
            .unwrap();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let chats: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(chats[0]["title"], "Renamed");

        let response = app
            .oneshot(authorized(
                "DELETE",
                &format!("/v1/chats/{id}"),
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn keeps_chat_histories_isolated() {
        let directory = tempdir().unwrap().keep();
        let state = AppState::for_test(&directory.join("test.db"), TOKEN)
            .await
            .unwrap();
        let pool = state.pool.clone();
        let app = router(state);

        let first_response = app
            .clone()
            .oneshot(authorized("POST", "/v1/chats", Body::empty()))
            .await
            .unwrap();
        let first_bytes = first_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();
        let first: serde_json::Value = serde_json::from_slice(&first_bytes).unwrap();
        let first_id = first["id"].as_str().unwrap();

        let second_response = app
            .clone()
            .oneshot(authorized("POST", "/v1/chats", Body::empty()))
            .await
            .unwrap();
        let second_bytes = second_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();
        let second: serde_json::Value = serde_json::from_slice(&second_bytes).unwrap();
        let second_id = second["id"].as_str().unwrap();

        sqlx::query("INSERT INTO messages(id, chat_id, role, content, status, created_at) VALUES ('first-message', ?, 'user', 'private context', 'complete', ?)")
            .bind(first_id)
            .bind(Utc::now().to_rfc3339())
            .execute(&pool)
            .await
            .unwrap();

        let response = app
            .oneshot(authorized(
                "GET",
                &format!("/v1/chats/{second_id}"),
                Body::empty(),
            ))
            .await
            .unwrap();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let detail: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(detail["chat"]["id"], second_id);
        assert_eq!(detail["messages"], serde_json::json!([]));
    }

    #[test]
    fn builds_short_titles() {
        assert_eq!(
            title_from("  hello   private   model "),
            "hello private model"
        );
        assert!(title_from(&"a".repeat(100)).ends_with('…'));
    }
}
