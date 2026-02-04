use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    error: anyhow::Error,
}

impl<E> From<E> for ApiError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: err.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        tracing::error!("api error: {}", self.error);
        let body = Json(serde_json::json!({
            "error": self.error.to_string(),
        }));
        (self.status, body).into_response()
    }
}

impl ApiError {
    pub fn unauthorized(msg: &str) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            error: anyhow::anyhow!(msg.to_string()),
        }
    }
}
