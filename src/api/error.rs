use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};

fn format_error_chain(error: &anyhow::Error) -> String {
    let mut parts = Vec::new();
    for (idx, cause) in error.chain().enumerate() {
        parts.push(format!("#{idx}: {cause}"));
    }
    parts.join(" | ")
}

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
        let status = self.status;
        let error = self.error;
        let error_chain = format_error_chain(&error);
        let body = Json(serde_json::json!({
            "error": error.to_string(),
        }));
        tracing::error!(
            status = %status,
            error = %error,
            error_chain = %error_chain,
            "api error"
        );
        (status, body).into_response()
    }
}

impl ApiError {
    pub fn unauthorized(msg: &str) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            error: anyhow::anyhow!(msg.to_string()),
        }
    }

    pub fn bad_request(msg: &str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            error: anyhow::anyhow!(msg.to_string()),
        }
    }

    pub fn not_found(msg: &str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            error: anyhow::anyhow!(msg.to_string()),
        }
    }

    pub fn conflict(msg: &str) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            error: anyhow::anyhow!(msg.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn format_error_chain_keeps_root_causes() {
        let err = anyhow::anyhow!("sqlite busy")
            .context("insert agent session failed")
            .context("start_agent failed");
        let chain = super::format_error_chain(&err);
        assert!(chain.contains("start_agent failed"));
        assert!(chain.contains("insert agent session failed"));
        assert!(chain.contains("sqlite busy"));
    }
}
