use std::time::Duration;

use axum::{
    Json, Router,
    extract::{Query, State, rejection::QueryRejection},
    response::{IntoResponse, Response},
    routing::get,
};
use members_grpc_service::members::{ListCareersRequest, ListIndustriesRequest};
use serde::{Deserialize, Serialize};

use crate::grpc::{self, MembersGrpcClient, error::UpstreamError};

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SuggestionQuery {
    /// Text typed so far.
    #[serde(default)]
    query: String,
    /// Maximum number of results; upstream defaults to 10 and caps at 50.
    limit: Option<u32>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct SuggestionList {
    suggestions: Vec<String>,
}

/// Drops empty and repeated values, keeping upstream order; the frontend
/// combobox rejects duplicate options.
impl From<Vec<String>> for SuggestionList {
    fn from(values: Vec<String>) -> Self {
        let mut suggestions = Vec::with_capacity(values.len());
        for value in values {
            if !value.is_empty() && !suggestions.contains(&value) {
                suggestions.push(value);
            }
        }

        Self { suggestions }
    }
}

#[derive(utoipa::IntoResponses)]
pub enum SuggestionsResponse {
    #[response(status = 200, description = "Matching values in upstream order")]
    Success(SuggestionList),
}

impl IntoResponse for SuggestionsResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Success(list) => Json(list).into_response(),
        }
    }
}

#[derive(Clone)]
pub struct SuggestionsApi<MGC: MembersGrpcClient> {
    members_grpc_client: MGC,
    timeout: Duration,
}

impl<MGC: MembersGrpcClient> SuggestionsApi<MGC> {
    pub fn new(members_grpc_client: MGC, timeout: Duration) -> Self {
        Self {
            members_grpc_client,
            timeout,
        }
    }

    pub fn router<S: Clone + Send + Sync + 'static>(self) -> Router<S> {
        Router::new()
            .route("/suggestions/careers", get(career_suggestions::<MGC>))
            .route("/suggestions/industries", get(industry_suggestions::<MGC>))
            .with_state(self)
    }
}

#[utoipa::path(
    get,
    path = "/api/suggestions/careers",
    params(SuggestionQuery),
    responses(SuggestionsResponse, UpstreamError)
)]
pub async fn career_suggestions<MGC: MembersGrpcClient>(
    State(SuggestionsApi {
        members_grpc_client: mut client,
        timeout,
    }): State<SuggestionsApi<MGC>>,
    query: Result<Query<SuggestionQuery>, QueryRejection>,
) -> Result<SuggestionsResponse, UpstreamError> {
    let Query(SuggestionQuery { query, limit }) =
        query.map_err(|_| UpstreamError::InvalidArgument)?;

    let response = grpc::call(
        timeout,
        ListCareersRequest {
            query,
            limit: limit.unwrap_or_default(),
        },
        |request| async move { client.list_careers(request).await },
    )
    .await?;

    Ok(SuggestionsResponse::Success(response.careers.into()))
}

#[utoipa::path(
    get,
    path = "/api/suggestions/industries",
    params(SuggestionQuery),
    responses(SuggestionsResponse, UpstreamError)
)]
pub async fn industry_suggestions<MGC: MembersGrpcClient>(
    State(SuggestionsApi {
        members_grpc_client: mut client,
        timeout,
    }): State<SuggestionsApi<MGC>>,
    query: Result<Query<SuggestionQuery>, QueryRejection>,
) -> Result<SuggestionsResponse, UpstreamError> {
    let Query(SuggestionQuery { query, limit }) =
        query.map_err(|_| UpstreamError::InvalidArgument)?;

    let response = grpc::call(
        timeout,
        ListIndustriesRequest {
            query,
            limit: limit.unwrap_or_default(),
        },
        |request| async move { client.list_industries(request).await },
    )
    .await?;

    Ok(SuggestionsResponse::Success(response.industries.into()))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use axum::{
        extract::{Query, State},
        http::{StatusCode, Uri},
        response::{IntoResponse, Response},
    };
    use http_body_util::BodyExt;
    use members_grpc_service::members::{ListCareersResponse, ListIndustriesResponse};
    use tonic::Code;

    use super::{SuggestionsApi, career_suggestions, industry_suggestions};
    use crate::grpc::fake::{FakeMembers, Outcome, sent};

    fn suggestions_api(client: FakeMembers) -> State<SuggestionsApi<FakeMembers>> {
        State(SuggestionsApi::new(client, Duration::from_secs(1)))
    }

    fn parse_query(
        uri: &str,
    ) -> Result<Query<super::SuggestionQuery>, axum::extract::rejection::QueryRejection> {
        Query::try_from_uri(&uri.parse::<Uri>().unwrap())
    }

    async fn status_and_body(response: Response) -> (StatusCode, serde_json::Value) {
        let status = response.status();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        (status, serde_json::from_slice(&body).unwrap())
    }

    fn duplicated_upstream_values() -> Vec<String> {
        vec![
            "Engineer".into(),
            String::new(),
            "Engineer".into(),
            "Designer".into(),
        ]
    }

    #[tokio::test]
    async fn careers_forwards_query_and_drops_empty_and_duplicate_values() {
        let members = FakeMembers {
            careers_response: Outcome::Reply(ListCareersResponse {
                careers: duplicated_upstream_values(),
            }),
            ..Default::default()
        };

        let response = career_suggestions(
            suggestions_api(members.clone()),
            parse_query("/?query=eng&limit=5"),
        )
        .await
        .into_response();

        let sent = sent(&members.careers_requests);
        assert_eq!((sent[0].query.as_str(), sent[0].limit), ("eng", 5));
        assert_eq!(
            status_and_body(response).await,
            (
                StatusCode::OK,
                serde_json::json!({"suggestions": ["Engineer", "Designer"]})
            )
        );
    }

    #[tokio::test]
    async fn industries_defaults_missing_query_and_limit() {
        let members = FakeMembers {
            industries_response: Outcome::Reply(ListIndustriesResponse {
                industries: vec!["Technology".into()],
            }),
            ..Default::default()
        };

        let response = industry_suggestions(suggestions_api(members.clone()), parse_query("/"))
            .await
            .into_response();

        let sent = sent(&members.industries_requests);
        assert_eq!((sent[0].query.as_str(), sent[0].limit), ("", 0));
        assert_eq!(
            status_and_body(response).await,
            (
                StatusCode::OK,
                serde_json::json!({"suggestions": ["Technology"]})
            )
        );
    }

    #[tokio::test]
    async fn upstream_failure_maps_status_without_leaking_details() {
        let members = FakeMembers {
            industries_response: Outcome::Fail(Code::Unavailable),
            ..Default::default()
        };

        let response = industry_suggestions(suggestions_api(members), parse_query("/?query=x"))
            .await
            .into_response();

        assert_eq!(
            status_and_body(response).await,
            (
                StatusCode::SERVICE_UNAVAILABLE,
                serde_json::json!({"error": "service unavailable"})
            )
        );
    }

    #[tokio::test]
    async fn invalid_query_is_rejected_before_calling_upstream() {
        let response = career_suggestions(
            suggestions_api(FakeMembers::default()),
            parse_query("/?limit=many"),
        )
        .await
        .into_response();

        assert_eq!(
            status_and_body(response).await,
            (
                StatusCode::BAD_REQUEST,
                serde_json::json!({"error": "invalid request"})
            )
        );
    }
}
