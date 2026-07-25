use super::*;

#[derive(Debug)]
pub(super) struct ApiError {
    pub(super) status: StatusCode,
    pub(super) message: String,
}
impl ApiError {
    pub(super) fn fixture(error: light_fixture::FixtureError) -> Self {
        match error {
            light_fixture::FixtureError::RevisionConflict { .. } => {
                Self::conflict(error.to_string())
            }
            _ => Self::bad_request(error.to_string()),
        }
    }
    pub(super) fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }
    pub(super) fn not_found(what: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: format!("{} not found", what.into()),
        }
    }
    pub(super) fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: message.into(),
        }
    }
    pub(super) fn forbidden(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            message: message.into(),
        }
    }
    pub(super) fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: message.into(),
        }
    }
    pub(super) fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
    pub(super) fn unavailable(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: message.into(),
        }
    }
    pub(super) fn store(error: light_show::StoreError) -> Self {
        match error {
            light_show::StoreError::RevisionConflict { .. } => Self::conflict(error.to_string()),
            _ => Self::bad_request(error.to_string()),
        }
    }
    pub(super) fn io(error: std::io::Error) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: error.to_string(),
        }
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(light_wire::v2::command_line::CommandErrorResponse {
                error: self.message,
            }),
        )
            .into_response()
    }
}
