use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use serde::Serialize;

/// The `message` every 5xx body carries, in place of the error's `Display`.
///
/// `AppError::Database` renders `sqlx::Error`, which names tables and
/// constraints, and `AppError::Internal` interpolates whatever its call site
/// had to hand. Neither is safe on a wire a caller holding only a virtual key
/// can read.
pub const INTERNAL_ERROR_MESSAGE: &str = "Internal server error";

/// Response header echoing [`ErrorDetail::incident_id`], so one grep finds
/// both the access-log line (method, path) and the `log::error!` detail.
pub const INCIDENT_ID_HEADER: &str = "X-Llmtrack-Incident";

/// Error body. The shape is OpenAI's (`{"error": {"message", "type"}}`), so the
/// official SDKs surface `message` from the gateway the same way they do from
/// OpenAI itself, and the dashboard reads the same shape.
#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: ErrorDetail,
}

#[derive(Serialize)]
pub struct ErrorDetail {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<FieldError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub incident_id: Option<String>,
}

/// A per-field validation failure the dashboard pins to its form input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FieldError {
    pub field: String,
    pub code: FieldErrorCode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldErrorCode {
    Required,
    Invalid,
    AlreadyExists,
    TooLong,
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    /// A key or team spent its budget. 429 with OpenAI's `insufficient_quota`
    /// semantics, which is what clients already handle for a drained account.
    #[error("Budget exceeded: {0}")]
    BudgetExceeded(String),

    /// A key or team reached its requests or tokens per minute.
    #[error("Rate limit exceeded: {0}")]
    RateLimited(String),

    /// A provider failed. `status` is what it answered (`None`: it could not
    /// be reached); `body` is its error in the client's dialect, passed on
    /// as is. 401 and 403 become 502: the gateway's provider key is wrong,
    /// not the client's.
    #[error("{message}")]
    Provider {
        status: Option<u16>,
        message: String,
        body: Option<serde_json::Value>,
    },

    /// The provider could not be reached or answered with garbage.
    #[error("Upstream error: {0}")]
    Upstream(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Internal server error: {0}")]
    Internal(String),

    #[error("{inner}")]
    WithFields {
        inner: Box<AppError>,
        fields: Vec<FieldError>,
    },
}

impl AppError {
    /// Pins the error to one form field. Annotating twice appends.
    #[must_use]
    pub fn with_field(self, field: impl Into<String>, code: FieldErrorCode) -> Self {
        let field = FieldError {
            field: field.into(),
            code,
        };
        match self {
            AppError::WithFields { inner, mut fields } => {
                fields.push(field);
                AppError::WithFields { inner, fields }
            }
            inner => AppError::WithFields {
                inner: Box::new(inner),
                fields: vec![field],
            },
        }
    }

    /// The error with its field annotations peeled off.
    pub fn kind(&self) -> &AppError {
        match self {
            AppError::WithFields { inner, .. } => inner.kind(),
            other => other,
        }
    }

    fn error_type(&self) -> &'static str {
        match self.kind() {
            AppError::NotFound(_) => "NotFound",
            AppError::Validation(_) => "ValidationError",
            AppError::Conflict(_) => "Conflict",
            AppError::Unauthorized(_) => "Unauthorized",
            AppError::Forbidden(_) => "Forbidden",
            AppError::BudgetExceeded(_) => "insufficient_quota",
            AppError::RateLimited(_) => "rate_limit_exceeded",
            AppError::Upstream(_) | AppError::Provider { .. } => "UpstreamError",
            AppError::Database(_) => "DatabaseError",
            AppError::Internal(_) | AppError::WithFields { .. } => "InternalError",
        }
    }

    fn field_errors(&self) -> &[FieldError] {
        match self {
            AppError::WithFields { fields, .. } => fields,
            _ => &[],
        }
    }
}

impl ResponseError for AppError {
    fn status_code(&self) -> StatusCode {
        match self.kind() {
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Validation(_) => StatusCode::BAD_REQUEST,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            AppError::Forbidden(_) => StatusCode::FORBIDDEN,
            AppError::BudgetExceeded(_) | AppError::RateLimited(_) => StatusCode::TOO_MANY_REQUESTS,
            AppError::Upstream(_) => StatusCode::BAD_GATEWAY,
            AppError::Provider { status, .. } => status
                .filter(|s| !matches!(s, 401 | 403))
                .and_then(|s| StatusCode::from_u16(s).ok())
                .unwrap_or(StatusCode::BAD_GATEWAY),
            AppError::Database(_) | AppError::Internal(_) | AppError::WithFields { .. } => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }

    fn error_response(&self) -> HttpResponse {
        let status = self.status_code();
        if let AppError::Provider {
            body: Some(body), ..
        } = self.kind()
        {
            if status != StatusCode::BAD_GATEWAY {
                return HttpResponse::build(status).json(body);
            }
        }
        // 502 is a server error but its message is safe and useful: it says
        // which provider failed, never anything about our own storage.
        let redact = status.is_server_error()
            && status != StatusCode::BAD_GATEWAY
            && !matches!(self.kind(), AppError::Provider { .. });
        let incident_id = redact.then(|| uuid::Uuid::new_v4().to_string());

        let message = match &incident_id {
            Some(id) => {
                log::error!("incident {id}: {self}");
                INTERNAL_ERROR_MESSAGE.to_string()
            }
            None => self.to_string(),
        };

        let mut builder = HttpResponse::build(status);
        if let Some(id) = &incident_id {
            builder.insert_header((INCIDENT_ID_HEADER, id.as_str()));
        }
        builder.json(ErrorResponse {
            error: ErrorDetail {
                error_type: self.error_type().to_string(),
                message,
                fields: self.field_errors().to_vec(),
                incident_id,
            },
        })
    }
}

impl From<argon2::password_hash::Error> for AppError {
    fn from(e: argon2::password_hash::Error) -> Self {
        AppError::Internal(format!("password hashing failed: {e}"))
    }
}

impl From<argon2::password_hash::phc::Error> for AppError {
    fn from(e: argon2::password_hash::phc::Error) -> Self {
        AppError::Internal(format!("stored password hash is malformed: {e}"))
    }
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    async fn body_of(error: &AppError) -> serde_json::Value {
        let bytes = actix_web::body::to_bytes(error.error_response().into_body())
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[actix_web::test]
    async fn a_server_error_redacts_its_message_and_carries_an_incident_id() {
        let error = AppError::Internal("users_email_key violated".into());
        let response = error.error_response();
        let header = response.headers().get(INCIDENT_ID_HEADER).cloned();

        let body = body_of(&error).await;

        assert_eq!(body["error"]["message"], INTERNAL_ERROR_MESSAGE);
        assert!(header.is_some());
        assert!(uuid::Uuid::parse_str(body["error"]["incident_id"].as_str().unwrap()).is_ok());
    }

    #[actix_web::test]
    async fn a_client_error_keeps_its_message_and_has_no_incident() {
        let body = body_of(&AppError::Conflict("team 'acme' exists".into())).await;

        assert_eq!(body["error"]["message"], "Conflict: team 'acme' exists");
        assert!(body["error"].get("incident_id").is_none());
    }

    #[actix_web::test]
    async fn an_upstream_error_is_a_502_that_keeps_its_message() {
        let error = AppError::Upstream("openai answered 503".into());
        assert_eq!(error.status_code(), StatusCode::BAD_GATEWAY);
        let body = body_of(&error).await;
        assert_eq!(
            body["error"]["message"],
            "Upstream error: openai answered 503"
        );
    }

    #[actix_web::test]
    async fn a_spent_budget_is_openai_insufficient_quota() {
        let error = AppError::BudgetExceeded("team budget".into());
        assert_eq!(error.status_code(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body_of(&error).await["error"]["type"], "insufficient_quota");
    }

    #[test]
    fn annotating_twice_appends_rather_than_nesting() {
        let error = AppError::Conflict("taken".into())
            .with_field("name", FieldErrorCode::AlreadyExists)
            .with_field("slug", FieldErrorCode::AlreadyExists);

        assert_eq!(error.field_errors().len(), 2);
        assert!(matches!(error.kind(), AppError::Conflict(_)));
        assert_eq!(error.status_code(), StatusCode::CONFLICT);
    }
}
