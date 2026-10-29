use std::time::Duration;

use axum::{
    Json,
    body::Body,
    extract::{FromRequest, rejection::JsonRejection},
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
};
use http_body_util::BodyExt;
use members_grpc_service::members::{
    self,
    admin::{CreateMemberResponse, SensitiveMember},
};
use tonic::{Code, Response as GrpcResponse, Status};

use super::{ListResponse, NewMember, collect_members, create_with};

#[derive(Clone, Copy)]
enum Reply {
    Success,
    MissingMember,
    Error(Code),
}

fn sensitive_member() -> SensitiveMember {
    SensitiveMember {
        member_id: 42,
        email: "person@example.com".into(),
        first_name: "First".into(),
        last_name: "Last".into(),
        joined_at: Some("2026-10-24T12:00:00Z".parse().unwrap()),
        discord_id: Some("123".into()),
        discord_username: Some("person".into()),
        profile: Some("private notes".into()),
        ..Default::default()
    }
}

async fn create_reply(body: Result<Json<NewMember>, JsonRejection>, reply: Reply) -> Response {
    create_with(body, Duration::from_secs(1), |request| async move {
        assert_eq!(request.metadata().get("profile").unwrap(), "core_admin");
        assert!(request.metadata().contains_key("grpc-timeout"));
        let input = request.into_inner();
        assert_eq!(input.email, "person@example.com");
        assert_eq!(input.first_name, "First");
        assert_eq!(input.last_name, "Last");
        let member = match reply {
            Reply::Success => Some(sensitive_member()),
            Reply::MissingMember => None,
            Reply::Error(code) => return Err(Status::new(code, "private upstream details")),
        };
        Ok(GrpcResponse::new(CreateMemberResponse { member }))
    })
    .await
    .into_response()
}

async fn list_reply(reply: Reply) -> Response {
    collect_members(Duration::from_secs(1), |request| async move {
        assert_eq!(request.page_size, 100);
        assert!(!request.include_archived);
        if let Reply::Error(code) = reply {
            return Err(Status::new(code, "private upstream details").into());
        }
        Ok(members::ListMembersResponse {
            members: vec![members::Member {
                member_id: 42,
                joined_at: sensitive_member().joined_at,
                ..Default::default()
            }],
            next_page_token: String::new(),
        })
    })
    .await
    .map(ListResponse::Success)
    .into_response()
}

async fn request_body(body: &str) -> Result<Json<NewMember>, JsonRejection> {
    let request = Request::builder()
        .header("content-type", "application/json")
        .body(Body::from(body.to_owned()))
        .unwrap();
    Json::from_request(request, &()).await
}

async fn json_body(response: Response) -> serde_json::Value {
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}

const VALID_MEMBER: &str =
    r#"{"email":" person@example.com ","first_name":" First ","last_name":" Last "}"#;

#[tokio::test]
async fn create_sends_normalized_fields_and_admin_metadata_and_returns_created_member() {
    let response = create_reply(request_body(VALID_MEMBER).await, Reply::Success).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(response.headers()["content-type"], "application/json");
    assert_eq!(
        json_body(response).await,
        serde_json::json!({
            "member_id":42, "email":"person@example.com", "first_name":"First", "last_name":"Last",
            "joined_at":"2026-10-24T12:00:00Z", "discord_id":"123", "discord_username":"person"
        })
    );
}

#[tokio::test]
async fn create_forwards_extra_fields_and_keeps_profile_notes_separate_from_admin_metadata() {
    let body = request_body(
        r#"{
        "email":"person@example.com", "first_name":"First", "last_name":"Last",
        "linkedin_username":" person-linkedin ", "instagram_username":" person.instagram ",
        "career":" Engineer ", "industry":" Technology ", "profile":" Private notes "
    }"#,
    )
    .await;
    let response = create_with(body, Duration::from_secs(1), |request| async move {
        assert_eq!(request.metadata().get("profile").unwrap(), "core_admin");
        let input = request.into_inner();
        assert_eq!(input.linkedin_username.as_deref(), Some("person-linkedin"));
        assert_eq!(
            input.instagram_username.as_deref(),
            Some("person.instagram")
        );
        assert_eq!(input.career.as_deref(), Some("Engineer"));
        assert_eq!(input.industry.as_deref(), Some("Technology"));
        assert_eq!(input.profile.as_deref(), Some("Private notes"));
        Ok(GrpcResponse::new(CreateMemberResponse {
            member: Some(sensitive_member()),
        }))
    })
    .await
    .into_response();
    assert_eq!(response.status(), StatusCode::CREATED);
    assert!(json_body(response).await.get("profile").is_none());
}

#[tokio::test]
async fn create_rejects_malformed_or_invalid_input() {
    for body in [
        "{",
        "{}",
        r#"{"email":" ","first_name":"First","last_name":"Last"}"#,
    ] {
        let response = create_with(
            request_body(body).await,
            Duration::from_secs(1),
            |_| async { panic!("invalid input must not reach gRPC") },
        )
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            json_body(response).await,
            serde_json::json!({"error":"invalid request"})
        );
    }
}

#[tokio::test]
async fn create_rejects_a_successful_rpc_without_a_member() {
    let response = create_reply(request_body(VALID_MEMBER).await, Reply::MissingMember).await;
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    assert_eq!(
        json_body(response).await,
        serde_json::json!({"error":"upstream request failed"})
    );
}

#[tokio::test]
async fn member_handlers_preserve_upstream_error_statuses_without_leaking_details() {
    for (code, expected, message) in [
        (Code::AlreadyExists, StatusCode::CONFLICT, "already exists"),
        (
            Code::PermissionDenied,
            StatusCode::FORBIDDEN,
            "permission denied",
        ),
        (
            Code::Unavailable,
            StatusCode::SERVICE_UNAVAILABLE,
            "service unavailable",
        ),
        (
            Code::DeadlineExceeded,
            StatusCode::GATEWAY_TIMEOUT,
            "upstream timeout",
        ),
    ] {
        let responses = [
            create_reply(request_body(VALID_MEMBER).await, Reply::Error(code)).await,
            list_reply(Reply::Error(code)).await,
        ];
        for response in responses {
            assert_eq!(response.status(), expected);
            assert_eq!(
                json_body(response).await,
                serde_json::json!({"error":message})
            );
        }
    }
}

#[tokio::test]
async fn list_returns_members_and_total_as_json() {
    let response = list_reply(Reply::Success).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["members"][0]["member_id"], 42);
    assert_eq!(body["members"][0]["joined_at"], "2026-10-24T12:00:00Z");
}

#[test]
fn openapi_describes_member_success_and_shared_error_bodies() {
    use utoipa::OpenApi;
    let schema = serde_json::to_value(crate::http::openapi::ApiDoc::openapi()).unwrap();
    for (method, success) in [("post", "201"), ("get", "200")] {
        let responses = &schema["paths"]["/api/members"][method]["responses"];
        assert!(responses[success]["content"]["application/json"].is_object());
        for status in [
            "400", "401", "403", "404", "409", "429", "502", "503", "504",
        ] {
            assert!(responses[status]["content"]["application/json"]["schema"].is_object());
        }
    }
}

#[tokio::test]
async fn details_returns_full_record_with_admin_metadata() {
    let response = super::details_with(42, Duration::from_secs(1), |request| async move {
        assert_eq!(request.metadata().get("profile").unwrap(), "core_admin");
        assert!(request.metadata().contains_key("grpc-timeout"));
        assert!(matches!(
            request.into_inner().identifier,
            Some(members::get_member_request::Identifier::MemberId(42))
        ));
        let mut member = sensitive_member();
        member.linkedin_username = Some("person-linkedin".into());
        member.instagram_username = Some("person.photo".into());
        member.updated_at = Some("2020-01-01T00:00:00Z".parse().unwrap());
        member.career = Some("Engineer".into());
        member.industry = Some("Technology".into());
        member.role = Some(members::MemberRole::Mentor as i32);
        Ok(GrpcResponse::new(
            members::admin::GetSensitiveMemberResponse {
                member: Some(member),
            },
        ))
    })
    .await
    .into_response();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["member_id"], 42);
    assert_eq!(body["profile"], "private notes");
    assert_eq!(body["linkedin_username"], "person-linkedin");
    assert_eq!(body["instagram_username"], "person.photo");
    assert_eq!(body["career"], "Engineer");
    assert_eq!(body["industry"], "Technology");
    assert_eq!(body["role"], "mentor");
    assert_eq!(body["updated_at"], "2020-01-01T00:00:00Z");
    assert!(body.get("archived_at").is_none());
}

#[tokio::test]
async fn details_handles_invalid_ids_missing_records_and_upstream_errors() {
    for id in [0, -1] {
        let response = super::details_with(id, Duration::from_secs(1), |_| async {
            panic!("invalid id must not reach gRPC")
        })
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    let response = super::details_with(42, Duration::from_secs(1), |_| async {
        Ok(GrpcResponse::new(
            members::admin::GetSensitiveMemberResponse { member: None },
        ))
    })
    .await
    .into_response();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    for (code, expected) in [
        (Code::NotFound, StatusCode::NOT_FOUND),
        (Code::PermissionDenied, StatusCode::FORBIDDEN),
        (Code::Unavailable, StatusCode::SERVICE_UNAVAILABLE),
    ] {
        let response = super::details_with(42, Duration::from_secs(1), |_| async move {
            Err(Status::new(code, "private upstream detail"))
        })
        .await
        .into_response();
        assert_eq!(response.status(), expected);
        assert!(
            !json_body(response)
                .await
                .to_string()
                .contains("private upstream detail")
        );
    }
    let response = super::details_with(42, Duration::from_millis(1), |_| std::future::pending())
        .await
        .into_response();
    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
}

#[tokio::test]
async fn update_validates_all_fields_before_calling_upstream() {
    for (field, value) in [
        ("email", "bad"),
        ("first_name", " "),
        ("last_name", " "),
        ("joined_at", "2020-01-01T00:00:00Z"),
        ("joined_at", "2999-01-01T00:00:00Z"),
        ("linkedin_username", "https://linkedin.com/in/person"),
        ("instagram_username", "@person"),
        ("role", "unknown"),
        ("discord_id", "abc"),
        ("discord_username", "person"),
    ] {
        let mut input = serde_json::json!({ "expected_updated_at": "2020-01-01T00:00:00Z", "email": "person@example.com", "first_name": "First", "last_name": "Last" });
        input[field] = value.into();
        let request = Request::builder()
            .header("content-type", "application/json")
            .body(Body::from(input.to_string()))
            .unwrap();
        let response = super::update_with(
            42,
            Json::from_request(request, &()).await,
            Duration::from_secs(1),
            |_| async { panic!("invalid input must not reach gRPC") },
        )
        .await
        .into_response();
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "{field}={value}"
        );
    }
}

#[tokio::test]
async fn update_preserves_omitted_fields_and_clears_explicit_nulls() {
    for clear in [false, true] {
        let mut input = serde_json::json!({ "expected_updated_at": "2020-01-01T00:00:00Z", "email": "person@example.com", "first_name": "First", "last_name": "Last" });
        if clear {
            for field in [
                "discord_id",
                "discord_username",
                "linkedin_username",
                "instagram_username",
                "career",
                "industry",
                "profile",
                "role",
            ] {
                input[field] = serde_json::Value::Null;
            }
        }
        let body = serde_json::from_value(input).unwrap();
        let response = super::update_with(
            42,
            Ok(Json(body)),
            Duration::from_secs(1),
            |request| async move {
                assert_eq!(request.metadata().get("profile").unwrap(), "core_admin");
                let input = request.into_inner();
                let expected = clear.then_some("");
                for field in [
                    input.discord_id,
                    input.discord_username,
                    input.linkedin_username,
                    input.instagram_username,
                    input.career,
                    input.industry,
                    input.profile,
                ] {
                    assert_eq!(field.as_deref(), expected);
                }
                assert_eq!(
                    input.role,
                    clear.then_some(members::MemberRole::Unspecified as i32)
                );
                assert!(input.joined_at.is_none());
                Ok(GrpcResponse::new(members::admin::UpdateMemberResponse {
                    member: Some(sensitive_member()),
                }))
            },
        )
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }
}

#[tokio::test]
async fn checked_update_rejects_stale_versions_without_writing() {
    let body = serde_json::from_value(serde_json::json!({
        "expected_updated_at": "2020-01-01T00:00:00Z", "first_name": "Changed"
    }))
    .unwrap();
    let response = super::checked_update_with(
        42,
        Ok(Json(body)),
        Duration::from_secs(1),
        |request| async move {
            assert_eq!(request.metadata().get("profile").unwrap(), "core_admin");
            let mut member = sensitive_member();
            member.updated_at = Some("2020-01-02T00:00:00Z".parse().unwrap());
            member.role = Some(members::MemberRole::Board as i32);
            Ok(GrpcResponse::new(
                members::admin::GetSensitiveMemberResponse {
                    member: Some(member),
                },
            ))
        },
        |_| async { panic!("stale forms must not write") },
    )
    .await
    .into_response();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert!(
        json_body(response).await["error"]
            .as_str()
            .unwrap()
            .contains("member changed")
    );
}

#[tokio::test]
async fn checked_update_allows_unrelated_edits_to_incomplete_discord_records() {
    let body = serde_json::from_value(serde_json::json!({
        "expected_updated_at": "2020-01-01T00:00:00Z", "first_name": "Changed"
    }))
    .unwrap();
    let response = super::checked_update_with(
        42,
        Ok(Json(body)),
        Duration::from_secs(1),
        |_| async {
            let mut member = sensitive_member();
            member.updated_at = Some("2020-01-01T00:00:00Z".parse().unwrap());
            member.discord_username = None;
            Ok(GrpcResponse::new(
                members::admin::GetSensitiveMemberResponse {
                    member: Some(member),
                },
            ))
        },
        |request| async move {
            let input = request.into_inner();
            assert_eq!(input.first_name.as_deref(), Some("Changed"));
            assert!(input.email.is_none());
            assert!(input.last_name.is_none());
            assert!(input.discord_id.is_none());
            assert!(input.discord_username.is_none());
            assert!(input.role.is_none());
            assert!(input.joined_at.is_none());
            Ok(GrpcResponse::new(members::admin::UpdateMemberResponse {
                member: Some(sensitive_member()),
            }))
        },
    )
    .await
    .into_response();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn checked_update_requires_version_and_stops_when_read_fails() {
    for input in [
        serde_json::json!({ "first_name": "Changed" }),
        serde_json::json!({ "expected_updated_at": "", "first_name": "Changed" }),
    ] {
        let request = Request::builder()
            .header("content-type", "application/json")
            .body(Body::from(input.to_string()))
            .unwrap();
        let response = super::checked_update_with(
            42,
            Json::from_request(request, &()).await,
            Duration::from_secs(1),
            |_| async { panic!("missing version must not read") },
            |_| async { panic!("missing version must not write") },
        )
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    let body = serde_json::from_value(serde_json::json!({ "expected_updated_at": "2020-01-01T00:00:00Z", "first_name": "Changed" })).unwrap();
    let response = super::checked_update_with(
        42,
        Ok(Json(body)),
        Duration::from_secs(1),
        |_| async { Err(Status::unavailable("private details")) },
        |_| async { panic!("failed reads must not write") },
    )
    .await
    .into_response();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[test]
fn update_rejects_invalid_email_shapes() {
    for email in [
        "",
        "person @example.com",
        "person@example.com\nextra",
        "a@@example.com",
        "@example.com",
        "person@example",
        "person@.example.com",
        "person@example.com.",
    ] {
        let body: super::MemberUpdate = serde_json::from_value(serde_json::json!({
            "expected_updated_at": "2020-01-01T00:00:00Z", "email": email
        }))
        .unwrap();
        assert!(
            matches!(
                body.into_request(42),
                Err(super::UpstreamError::InvalidArgument)
            ),
            "{email:?}"
        );
    }
}

#[test]
fn update_requires_discord_fields_to_be_set_or_cleared_together() {
    for (id, username) in [("", "person"), ("123", ""), (" ", "person"), ("123", " ")] {
        let body: super::MemberUpdate = serde_json::from_value(serde_json::json!({
            "expected_updated_at": "2020-01-01T00:00:00Z",
            "discord_id": id, "discord_username": username
        }))
        .unwrap();
        assert!(
            matches!(
                body.into_request(42),
                Err(super::UpstreamError::InvalidArgument)
            ),
            "{id:?}, {username:?}"
        );
    }
}

#[test]
fn update_rejects_reserved_characters_in_social_usernames() {
    for field in ["linkedin_username", "instagram_username"] {
        for value in [
            "two words",
            "two\twords",
            "user/name",
            "@name",
            "name?query",
            "name#fragment",
        ] {
            let mut input = serde_json::json!({"expected_updated_at": "2020-01-01T00:00:00Z"});
            input[field] = value.into();
            let body: super::MemberUpdate = serde_json::from_value(input).unwrap();
            assert!(
                matches!(
                    body.into_request(42),
                    Err(super::UpstreamError::InvalidArgument)
                ),
                "{field}={value:?}"
            );
        }
    }
}

#[tokio::test]
async fn details_serializes_every_role_and_handles_missing_timestamps() {
    for (role, expected) in [
        (
            Some(members::MemberRole::Board as i32),
            serde_json::json!("board"),
        ),
        (
            Some(members::MemberRole::Mentor as i32),
            serde_json::json!("mentor"),
        ),
        (
            Some(members::MemberRole::Mentee as i32),
            serde_json::json!("mentee"),
        ),
        (
            Some(members::MemberRole::Unspecified as i32),
            serde_json::Value::Null,
        ),
        (Some(i32::MAX), serde_json::Value::Null),
        (None, serde_json::Value::Null),
    ] {
        let response = super::details_with(42, Duration::from_secs(1), |_| async move {
            Ok(GrpcResponse::new(
                members::admin::GetSensitiveMemberResponse {
                    member: Some(SensitiveMember {
                        role,
                        ..sensitive_member()
                    }),
                },
            ))
        })
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["role"], expected);
        assert_eq!(body["updated_at"], "");
    }
}

#[tokio::test]
async fn checked_update_stops_on_missing_member_or_version() {
    for missing_member in [true, false] {
        let body = serde_json::from_value(serde_json::json!({
            "expected_updated_at": "2020-01-01T00:00:00Z", "first_name": "Changed"
        }))
        .unwrap();
        let response = super::checked_update_with(
            42,
            Ok(Json(body)),
            Duration::from_secs(1),
            |request| async move {
                assert!(matches!(
                    request.into_inner().identifier,
                    Some(members::get_member_request::Identifier::MemberId(42))
                ));
                Ok(GrpcResponse::new(
                    members::admin::GetSensitiveMemberResponse {
                        member: (!missing_member).then(sensitive_member),
                    },
                ))
            },
            |_| async { panic!("missing member or version must not write") },
        )
        .await
        .into_response();
        assert_eq!(
            response.status(),
            if missing_member {
                StatusCode::BAD_GATEWAY
            } else {
                StatusCode::CONFLICT
            }
        );
    }
}

#[tokio::test(start_paused = true)]
async fn checked_update_read_timeout_prevents_writes() {
    let body = serde_json::from_value(serde_json::json!({
        "expected_updated_at": "2020-01-01T00:00:00Z"
    }))
    .unwrap();
    let response = super::checked_update_with(
        42,
        Ok(Json(body)),
        Duration::from_secs(1),
        |_| std::future::pending(),
        |_| async { panic!("timed out reads must not write") },
    )
    .await
    .into_response();
    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    assert_eq!(
        json_body(response).await,
        serde_json::json!({"error": "upstream timeout"})
    );
}

struct ArchiveGrpc {
    reply: Option<Reply>,
}

#[tonic::async_trait]
impl members::admin::members_admin_service_server::MembersAdminService for ArchiveGrpc {
    async fn get_sensitive_member(
        &self,
        _: tonic::Request<members::GetMemberRequest>,
    ) -> Result<GrpcResponse<members::admin::GetSensitiveMemberResponse>, Status> {
        panic!("archive must not call get_sensitive_member")
    }
    async fn list_sensitive_members(
        &self,
        _: tonic::Request<members::ListMembersRequest>,
    ) -> Result<GrpcResponse<members::admin::ListSensitiveMembersResponse>, Status> {
        panic!("archive must not call list_sensitive_members")
    }
    async fn create_member(
        &self,
        _: tonic::Request<members::admin::CreateMemberRequest>,
    ) -> Result<GrpcResponse<members::admin::CreateMemberResponse>, Status> {
        panic!("archive must not call create_member")
    }
    async fn update_member(
        &self,
        _: tonic::Request<members::admin::UpdateMemberRequest>,
    ) -> Result<GrpcResponse<members::admin::UpdateMemberResponse>, Status> {
        panic!("archive must not call update_member")
    }
    async fn bulk_member_operations(
        &self,
        _: tonic::Request<members::admin::BulkMemberOperationsRequest>,
    ) -> Result<GrpcResponse<members::admin::BulkMemberOperationsResponse>, Status> {
        panic!("archive must not call bulk_member_operations")
    }
    async fn archive_member(
        &self,
        request: tonic::Request<members::admin::ArchiveMemberRequest>,
    ) -> Result<GrpcResponse<members::admin::ArchiveMemberResponse>, Status> {
        assert_eq!(request.metadata().get("profile").unwrap(), "core_admin");
        assert!(request.metadata().contains_key("grpc-timeout"));
        assert_eq!(request.into_inner().member_id, 42);
        match self.reply {
            Some(Reply::Success) => Ok(GrpcResponse::new(members::admin::ArchiveMemberResponse {
                member: Some(sensitive_member()),
            })),
            Some(Reply::MissingMember) => {
                Ok(GrpcResponse::new(members::admin::ArchiveMemberResponse {
                    member: None,
                }))
            }
            Some(Reply::Error(code)) => Err(Status::new(code, "private upstream details")),
            None => std::future::pending().await,
        }
    }
}

struct ArchiveServer(tokio::task::JoinHandle<()>);

impl Drop for ArchiveServer {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn archive_server(reply: Option<Reply>) -> (crate::http::state::AppState, ArchiveServer) {
    use members::admin::members_admin_service_server::MembersAdminServiceServer;
    use tokio_stream::wrappers::TcpListenerStream;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = ArchiveServer(tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(MembersAdminServiceServer::new(ArchiveGrpc { reply }))
            .serve_with_incoming(TcpListenerStream::new(listener))
            .await
            .unwrap();
    }));
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{address}"))
        .unwrap()
        .connect()
        .await
        .unwrap();
    let mut state = crate::http::state::test_state();
    state.client =
        members::admin::members_admin_service_client::MembersAdminServiceClient::new(channel);
    (state, server)
}

#[tokio::test]
async fn archive_handler_calls_generated_client_and_returns_empty_204() {
    let (state, _server) = archive_server(Some(Reply::Success)).await;
    let response = super::archive(axum::extract::State(state), axum::extract::Path(42))
        .await
        .into_response();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .is_empty()
    );
}

#[tokio::test]
async fn archive_handler_maps_rpc_failures_without_leaking_details() {
    for (reply, status, message) in [
        (
            Reply::MissingMember,
            StatusCode::BAD_GATEWAY,
            "upstream request failed",
        ),
        (
            Reply::Error(Code::NotFound),
            StatusCode::NOT_FOUND,
            "not found",
        ),
        (
            Reply::Error(Code::PermissionDenied),
            StatusCode::FORBIDDEN,
            "permission denied",
        ),
        (
            Reply::Error(Code::Unavailable),
            StatusCode::SERVICE_UNAVAILABLE,
            "service unavailable",
        ),
        (
            Reply::Error(Code::DeadlineExceeded),
            StatusCode::GATEWAY_TIMEOUT,
            "upstream timeout",
        ),
    ] {
        let (state, _server) = archive_server(Some(reply)).await;
        let response = super::archive(axum::extract::State(state), axum::extract::Path(42))
            .await
            .into_response();
        assert_eq!(response.status(), status);
        assert_eq!(
            json_body(response).await,
            serde_json::json!({ "error": message })
        );
    }
}

#[tokio::test]
async fn archive_handler_rejects_invalid_ids_without_calling_rpc() {
    for member_id in [0, -1] {
        let state = crate::http::state::test_state();
        let response = super::archive(axum::extract::State(state), axum::extract::Path(member_id))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            json_body(response).await,
            serde_json::json!({ "error": "invalid request" })
        );
    }
}

#[tokio::test]
async fn archive_handler_enforces_configured_timeout() {
    let (mut state, _server) = archive_server(None).await;
    state.config.members_admin_service_grpc_timeout = Duration::from_millis(50);
    let response = tokio::time::timeout(
        Duration::from_secs(2),
        super::archive(axum::extract::State(state), axum::extract::Path(42)),
    )
    .await
    .expect("archive handler must enforce its timeout")
    .into_response();
    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    assert_eq!(
        json_body(response).await,
        serde_json::json!({ "error": "upstream timeout" })
    );
}

#[test]
fn openapi_describes_archive_success_and_error_responses() {
    use utoipa::OpenApi;
    let schema = serde_json::to_value(crate::http::openapi::ApiDoc::openapi()).unwrap();
    let responses = &schema["paths"]["/api/members/{member_id}"]["delete"]["responses"];
    assert!(responses["204"]["content"].is_null());
    assert!(
        responses["204"]["description"]
            .as_str()
            .unwrap()
            .contains("not permanently deleted")
    );
    for status in [
        "400", "401", "403", "404", "409", "429", "502", "503", "504",
    ] {
        assert!(responses[status]["content"]["application/json"]["schema"].is_object());
    }
}

#[tokio::test]
async fn public_handlers_report_transport_failures_without_leaking_details() {
    use axum::extract::{Path, State};
    use members_grpc_service::members::{
        admin::members_admin_service_client::MembersAdminServiceClient,
        members_service_client::MembersServiceClient,
    };
    let channel = tonic::transport::Endpoint::from_static("http://127.0.0.1:0").connect_lazy();
    let mut state = crate::http::state::test_state();
    state.client = MembersAdminServiceClient::new(channel.clone());
    state.members_client = MembersServiceClient::new(channel);
    state.config.members_admin_service_grpc_timeout = Duration::from_secs(1);
    let update = serde_json::from_value(serde_json::json!({
        "expected_updated_at": "2020-01-01T00:00:00Z", "first_name": "Changed"
    }))
    .unwrap();
    let responses = [
        super::create(State(state.clone()), request_body(VALID_MEMBER).await)
            .await
            .into_response(),
        super::update(State(state.clone()), Path(42), Ok(Json(update)))
            .await
            .into_response(),
        super::archive(State(state.clone()), Path(42))
            .await
            .into_response(),
        super::details(State(state.clone()), Path(42))
            .await
            .into_response(),
        super::list(State(state)).await.into_response(),
    ];
    for response in responses {
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            json_body(response).await,
            serde_json::json!({"error": "service unavailable"})
        );
    }
}
