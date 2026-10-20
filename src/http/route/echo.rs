use axum::{
    Json,
    extract::{Query, State},
};
use hello_world_grpc_service::helloworld::EchoHelloRequest;
use serde::{Deserialize, Serialize};

use crate::{
    grpc::{call, error::UpstreamError},
    http::state::AppState,
};

pub async fn echo(
    State(mut state): State<AppState>,
    Query(query): Query<EchoQuery>,
) -> Result<Json<EchoResponse>, UpstreamError> {
    let r = call(
        state.config.hello_world_service_grpc_timeout,
        EchoHelloRequest { name: query.name },
        |r| state.client.echo_hello(r),
    )
    .await?;

    Ok(EchoResponse {
        response: r.response,
    }
    .into())
}

#[derive(Deserialize)]
pub struct EchoQuery {
    #[serde(default)]
    name: String,
}

#[derive(Serialize)]
pub struct EchoResponse {
    response: String,
}
