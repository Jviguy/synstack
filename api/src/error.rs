//! Unified error types for the SynStack API
//!
//! This module defines error types for each layer:
//! - `DomainError`: Core business logic errors
//! - `GiteaError`: Gitea API client errors
//! - `AppError`: Application layer errors (wraps domain errors for HTTP responses)

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

/// Domain layer errors - pure business logic errors
#[derive(Debug, Error)]
pub enum DomainError {
    #[error("Entity not found: {0}")]
    NotFound(String),

    #[error("Entity already exists: {0}")]
    AlreadyExists(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Gitea API client errors
#[derive(Debug, Error)]
pub enum GiteaError {
    #[error("Request failed: {0}")]
    Request(#[from] reqwest::Error),

    #[error("API error: {status} - {message}")]
    Api { status: u16, message: String },

    #[error("User not found: {0}")]
    UserNotFound(String),

    #[error("Repository not found: {owner}/{repo}")]
    RepoNotFound { owner: String, repo: String },

    #[error("Organization not found: {0}")]
    OrgNotFound(String),

    #[error("Issue not found: {owner}/{repo}#{number}")]
    IssueNotFound {
        owner: String,
        repo: String,
        number: i64,
    },

    #[error("Rate limited")]
    RateLimited,

    #[error("Unauthorized - invalid token")]
    Unauthorized,

    #[error("Deserialization error: {0}")]
    Deserialization(String),
}

/// ClickHouse/Analytics errors
#[derive(Debug, Error)]
pub enum AnalyticsError {
    #[error("Request failed: {0}")]
    Request(#[from] reqwest::Error),

    #[error("Query error: {0}")]
    Query(String),

    #[error("Connection error: {0}")]
    Connection(String),
}

/// Application layer errors - used by HTTP handlers
#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Domain(#[from] DomainError),

    #[error("Gitea error: {0}")]
    Gitea(#[from] GiteaError),

    #[error("Analytics error: {0}")]
    Analytics(#[from] AnalyticsError),

    #[error("Invalid request: {0}")]
    BadRequest(String),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Forbidden")]
    Forbidden,

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Internal server error: {0}")]
    Internal(String),

    #[error("Parse error: {0}")]
    Parse(String),
}

/// Error response body for JSON responses
#[derive(Serialize)]
struct ErrorResponse {
    error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<String>,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error, details) = match &self {
            AppError::Domain(DomainError::NotFound(msg)) => {
                (StatusCode::NOT_FOUND, "Not found", Some(msg.clone()))
            }
            AppError::Domain(DomainError::AlreadyExists(msg)) => {
                (StatusCode::CONFLICT, "Already exists", Some(msg.clone()))
            }
            AppError::Domain(DomainError::Validation(msg)) => (
                StatusCode::BAD_REQUEST,
                "Validation error",
                Some(msg.clone()),
            ),
            AppError::Domain(DomainError::Unauthorized(msg)) => {
                (StatusCode::UNAUTHORIZED, "Unauthorized", Some(msg.clone()))
            }
            AppError::Domain(DomainError::Forbidden(msg)) => {
                (StatusCode::FORBIDDEN, "Forbidden", Some(msg.clone()))
            }
            AppError::Domain(DomainError::Conflict(msg)) => {
                (StatusCode::CONFLICT, "Conflict", Some(msg.clone()))
            }
            AppError::Domain(DomainError::Database(msg)) => {
                tracing::error!("Database error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error",
                    None,
                )
            }
            AppError::Domain(DomainError::Internal(msg)) => {
                tracing::error!("Internal error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error",
                    None,
                )
            }
            AppError::Gitea(e) => {
                tracing::error!("Gitea error: {}", e);
                match e {
                    GiteaError::Unauthorized => {
                        (StatusCode::INTERNAL_SERVER_ERROR, "Git service error", None)
                    }
                    GiteaError::UserNotFound(_)
                    | GiteaError::RepoNotFound { .. }
                    | GiteaError::OrgNotFound(_) => {
                        (StatusCode::NOT_FOUND, "Git resource not found", None)
                    }
                    GiteaError::RateLimited => {
                        (StatusCode::TOO_MANY_REQUESTS, "Rate limited", None)
                    }
                    GiteaError::Api { status, message } => {
                        // Propagate Gitea API errors with their message for better debugging
                        let http_status = if *status == 404 {
                            StatusCode::NOT_FOUND
                        } else if *status == 403 {
                            StatusCode::FORBIDDEN
                        } else if *status == 422 {
                            StatusCode::UNPROCESSABLE_ENTITY
                        } else {
                            StatusCode::BAD_GATEWAY
                        };
                        (http_status, "Git service error", Some(message.clone()))
                    }
                    _ => (StatusCode::INTERNAL_SERVER_ERROR, "Git service error", None),
                }
            }
            AppError::Analytics(e) => {
                tracing::error!("Analytics error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Analytics service error",
                    None,
                )
            }
            AppError::BadRequest(msg) => {
                (StatusCode::BAD_REQUEST, "Bad request", Some(msg.clone()))
            }
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized", None),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "Forbidden", None),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, "Not found", Some(msg.clone())),
            AppError::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error",
                    None,
                )
            }
            AppError::Parse(msg) => (StatusCode::BAD_REQUEST, "Parse error", Some(msg.clone())),
        };

        let body = Json(ErrorResponse {
            error: error.to_string(),
            details,
        });

        (status, body).into_response()
    }
}

/// Parse error for action parsing
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Unknown command: {0}")]
    UnknownCommand(String),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("Missing argument for: {0}")]
    MissingArgument(String),

    #[error("Invalid number: {0}")]
    InvalidNumber(#[from] std::num::ParseIntError),
}

impl From<ParseError> for AppError {
    fn from(e: ParseError) -> Self {
        AppError::Parse(e.to_string())
    }
}
