use axum::{
    Json,
    extract::{Query, State, rejection::QueryRejection},
    response::{IntoResponse, Response},
};
use members_grpc_service::members::{ListCareersRequest, ListIndustriesRequest};
use serde::{Deserialize, Serialize};

use crate::{
    grpc::{self, error::UpstreamError},
    http::state::AppState,
};

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

#[utoipa::path(
    get,
    path = "/api/suggestions/careers",
    params(SuggestionQuery),
    responses(SuggestionsResponse, UpstreamError)
)]
pub async fn career_suggestions(
    State(state): State<AppState>,
    query: Result<Query<SuggestionQuery>, QueryRejection>,
) -> Result<SuggestionsResponse, UpstreamError> {
    let Query(SuggestionQuery { query, limit }) =
        query.map_err(|_| UpstreamError::InvalidArgument)?;
    let mut client = state.members_client.clone();

    let response = grpc::call(
        state.config.members_admin_service_grpc_timeout,
        ListCareersRequest {
            query,
            limit: limit.unwrap_or_default(),
        },
        |request| client.list_careers(request),
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
pub async fn industry_suggestions(
    State(state): State<AppState>,
    query: Result<Query<SuggestionQuery>, QueryRejection>,
) -> Result<SuggestionsResponse, UpstreamError> {
    let Query(SuggestionQuery { query, limit }) =
        query.map_err(|_| UpstreamError::InvalidArgument)?;
    let mut client = state.members_client.clone();

    let response = grpc::call(
        state.config.members_admin_service_grpc_timeout,
        ListIndustriesRequest {
            query,
            limit: limit.unwrap_or_default(),
        },
        |request| client.list_industries(request),
    )
    .await?;

    Ok(SuggestionsResponse::Success(response.industries.into()))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use axum_extra::extract::CookieJar;
    use http_body_util::BodyExt;
    use members_grpc_service::members::{
        self, ListCareersResponse, ListIndustriesResponse,
        members_service_client::MembersServiceClient,
        members_service_server::{MembersService, MembersServiceServer},
    };
    use tokio_stream::wrappers::TcpListenerStream;
    use tonic::{Response as GrpcResponse, Status};
    use tower::ServiceExt;

    use crate::{
        auth::session::{self, Principal},
        http::state::test_state,
    };

    struct SuggestionGrpc;

    /// Echoes the forwarded query and limit, then repeats a value and adds an
    /// empty one so the response shaping is observable.
    fn reply(query: &str, limit: u32, value: &str) -> Result<Vec<String>, Status> {
        if query == "fail" {
            return Err(Status::unavailable("private upstream details"));
        }

        Ok(vec![
            format!("{query}:{limit}"),
            value.into(),
            String::new(),
            value.into(),
        ])
    }

    #[tonic::async_trait]
    impl MembersService for SuggestionGrpc {
        async fn get_member(
            &self,
            _: tonic::Request<members::GetMemberRequest>,
        ) -> Result<GrpcResponse<members::GetMemberResponse>, Status> {
            Err(Status::unimplemented("get_member"))
        }

        async fn list_members(
            &self,
            _: tonic::Request<members::ListMembersRequest>,
        ) -> Result<GrpcResponse<members::ListMembersResponse>, Status> {
            Err(Status::unimplemented("list_members"))
        }

        async fn list_careers(
            &self,
            request: tonic::Request<members::ListCareersRequest>,
        ) -> Result<GrpcResponse<ListCareersResponse>, Status> {
            assert!(request.metadata().contains_key("grpc-timeout"));
            let request = request.into_inner();
            let careers = reply(&request.query, request.limit, "Engineer")?;
            Ok(GrpcResponse::new(ListCareersResponse { careers }))
        }

        async fn list_industries(
            &self,
            request: tonic::Request<members::ListIndustriesRequest>,
        ) -> Result<GrpcResponse<ListIndustriesResponse>, Status> {
            assert!(request.metadata().contains_key("grpc-timeout"));
            let request = request.into_inner();
            let industries = reply(&request.query, request.limit, "Technology")?;
            Ok(GrpcResponse::new(ListIndustriesResponse { industries }))
        }
    }

    async fn get(router: axum::Router, uri: &str, cookie: &str) -> (StatusCode, serde_json::Value) {
        let response = router
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .header("cookie", cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = response.into_body().collect().await.unwrap().to_bytes();

        (status, serde_json::from_slice(&body).unwrap())
    }

    #[tokio::test]
    async fn suggestion_routes_forward_queries_and_map_upstream_results() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(
            tonic::transport::Server::builder()
                .add_service(MembersServiceServer::new(SuggestionGrpc))
                .serve_with_incoming(TcpListenerStream::new(listener)),
        );

        let mut state = test_state();
        state.members_client = MembersServiceClient::new(
            tonic::transport::Endpoint::from_shared(format!("http://{address}"))
                .unwrap()
                .connect()
                .await
                .unwrap(),
        );
        let jar = session::establish(
            CookieJar::new(),
            &state.auth.config,
            &Principal {
                id: "test".into(),
                name: "Admin".into(),
                email: Some("admin@example.com".into()),
                csrf_token: "csrf".into(),
                expires_at: time::OffsetDateTime::now_utc().unix_timestamp() + 60,
            },
        )
        .unwrap();
        let cookie = format!(
            "members_session={}",
            jar.get("members_session").unwrap().value()
        );
        let router = crate::http::app(state);

        for (uri, status, body) in [
            (
                "/api/suggestions/careers?query=eng&limit=5",
                StatusCode::OK,
                serde_json::json!({"suggestions": ["eng:5", "Engineer"]}),
            ),
            (
                "/api/suggestions/careers",
                StatusCode::OK,
                serde_json::json!({"suggestions": [":0", "Engineer"]}),
            ),
            (
                "/api/suggestions/industries?query=te%20ch",
                StatusCode::OK,
                serde_json::json!({"suggestions": ["te ch:0", "Technology"]}),
            ),
            (
                "/api/suggestions/industries?query=fail",
                StatusCode::SERVICE_UNAVAILABLE,
                serde_json::json!({"error": "service unavailable"}),
            ),
            (
                "/api/suggestions/careers?limit=many",
                StatusCode::BAD_REQUEST,
                serde_json::json!({"error": "invalid request"}),
            ),
        ] {
            assert_eq!(
                get(router.clone(), uri, &cookie).await,
                (status, body),
                "{uri}"
            );
        }

        let (status, _) = get(router, "/api/suggestions/careers?query=eng", "").await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        server.abort();
    }
}
