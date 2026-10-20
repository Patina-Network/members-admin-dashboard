use axum::{
    Json,
    extract::{Query, State},
};
use hello_world_grpc_service::helloworld::{
    GetGreetingsByNameRequest, GreetingResponse, SayGreetingRequest,
};
use serde::{Deserialize, Serialize};

use crate::{
    grpc::{call, error::UpstreamError},
    http::{json::JsonBody, state::AppState},
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SendGreetingBody {
    sender_name: String,
    recipient_name: String,
    greeting: String,
}

#[derive(Serialize)]
pub struct SendGreetingResponse {}

pub async fn send(
    State(mut state): State<AppState>,
    JsonBody(input): JsonBody<SendGreetingBody>,
) -> Result<Json<SendGreetingResponse>, UpstreamError> {
    call(
        state.config.hello_world_service_grpc_timeout,
        SayGreetingRequest {
            sender_name: input.sender_name,
            recipient_name: input.recipient_name,
            greeting: input.greeting,
        },
        |r| state.client.say_greeting(r),
    )
    .await?;
    Ok(Json(SendGreetingResponse {}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListGreetingsQuery {
    recipient_name: Option<String>,
}

#[derive(Serialize)]
pub struct ListGreetingsResponse {
    replies: Vec<GreetingReplyResponse>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GreetingReplyResponse {
    id: u32,
    message: String,
    sender_name: String,
    recipient_name: String,
    /// RFC 3339 ts
    received_at: Option<String>,
}

impl From<GreetingResponse> for GreetingReplyResponse {
    fn from(reply: GreetingResponse) -> Self {
        Self {
            id: reply.id,
            message: reply.message,
            sender_name: reply.sender_name,
            recipient_name: reply.recipient_name,
            received_at: reply.received_at.map(|timestamp| timestamp.to_string()),
        }
    }
}

pub async fn list(
    State(mut state): State<AppState>,
    Query(query): Query<ListGreetingsQuery>,
) -> Result<Json<ListGreetingsResponse>, UpstreamError> {
    let r = call(
        state.config.hello_world_service_grpc_timeout,
        GetGreetingsByNameRequest {
            recipient_name: query.recipient_name,
        },
        |r| state.client.get_greetings_by_name(r),
    )
    .await?;

    Ok(ListGreetingsResponse {
        replies: r.replies.into_iter().map(Into::into).collect(),
    }
    .into())
}
