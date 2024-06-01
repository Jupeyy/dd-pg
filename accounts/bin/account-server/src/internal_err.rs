use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

pub struct InternalErr(pub (String, anyhow::Error));

// Tell axum how to convert `InternalErr` into a response.
impl IntoResponse for InternalErr {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!(
                "[{}] Something went wrong: {} {}",
                self.0 .0,
                self.0 .1,
                self.0 .1.backtrace()
            ),
        )
            .into_response()
    }
}
