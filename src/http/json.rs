use axum::{
    Json,
    extract::{FromRequest, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::http::ErrorBody;

/// `Json` request body whose rejection renders as `{"error": "invalid JSON request"}`.
///
/// axum responds with the rejection before the handler runs, so route error enums never
/// need a bad-JSON variant.
#[derive(FromRequest)]
#[from_request(via(Json), rejection(InvalidJson))]
pub struct JsonBody<T>(pub T);

pub struct InvalidJson(StatusCode);

impl From<JsonRejection> for InvalidJson {
    fn from(rejection: JsonRejection) -> Self {
        Self(match rejection {
            JsonRejection::JsonDataError(_) | JsonRejection::JsonSyntaxError(_) => {
                StatusCode::BAD_REQUEST
            }
            other => other.status(),
        })
    }
}

impl IntoResponse for InvalidJson {
    fn into_response(self) -> Response {
        (
            self.0,
            Json(ErrorBody {
                error: "invalid JSON request",
            }),
        )
            .into_response()
    }
}
