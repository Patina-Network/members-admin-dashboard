use axum::{
    extract::State,
    http::header,
    response::{IntoResponse, Response},
};

use crate::http::state::AppState;

/// Prometheus text exposition of every recorded metric.
pub struct Metrics(String);

impl IntoResponse for Metrics {
    fn into_response(self) -> Response {
        (
            [(
                header::CONTENT_TYPE,
                "text/plain; version=0.0.4; charset=utf-8",
            )],
            self.0,
        )
            .into_response()
    }
}

pub async fn metrics(State(state): State<AppState>) -> Metrics {
    // add more repo / non-auto metric collections here
    state.sys_collector.collect();
    Metrics(state.prometheus_handle.render())
}
