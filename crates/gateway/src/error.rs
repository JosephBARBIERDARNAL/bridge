use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
    pub retryable: bool,
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    code: &'a str,
    message: &'a str,
    retryable: bool,
}

impl ApiError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "bad_request",
            message: message.into(),
            retryable: false,
        }
    }

    pub fn not_found(kind: &'static str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: format!("{kind} was not found"),
            retryable: false,
        }
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code: "generation_in_progress",
            message: message.into(),
            retryable: true,
        }
    }

    pub fn unavailable(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "ollama_unavailable",
            message: message.into(),
            retryable: true,
        }
    }

    pub fn internal(error: impl std::fmt::Display) -> Self {
        tracing::error!(%error, "request failed");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal_error",
            message: "The gateway could not complete the request".into(),
            retryable: true,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        if self.status.is_server_error() {
            tracing::error!(
                status = self.status.as_u16(),
                code = self.code,
                retryable = self.retryable,
                message = %self.message,
                "Returning an API error response"
            );
        } else {
            tracing::warn!(
                status = self.status.as_u16(),
                code = self.code,
                retryable = self.retryable,
                message = %self.message,
                "Returning an API error response"
            );
        }
        let body = ErrorBody {
            code: self.code,
            message: &self.message,
            retryable: self.retryable,
        };
        (self.status, Json(body)).into_response()
    }
}
