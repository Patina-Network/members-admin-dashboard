pub mod domain;

use std::{
    collections::{BTreeMap, HashSet},
    time::Duration,
};

use axum::{
    Json, Router,
    extract::{
        DefaultBodyLimit, Path, Query, State,
        rejection::{JsonRejection, QueryRejection},
    },
    routing::{get, post},
};
use members_grpc_service::members::{
    self,
    admin::{BulkMemberOperationsRequest, CreateMemberRequest, SensitiveMember},
};
use serde::Deserialize;

use crate::{
    grpc::{self, MembersAdminGrpcClient, MembersGrpcClient, error::UpstreamError},
    metrics::GrpcMethod,
};
use domain::{
    ArchiveResponse, BulkMembers, BulkOutcome, BulkResult, BulkResults, CreateResponse,
    DetailsResponse, ListResponse, Member, MemberDetails, MemberList, MemberUpdate, NewMember,
    UpdateResponse,
};

#[derive(Clone)]
pub struct MembersApi<MGC: MembersGrpcClient, MAGC: MembersAdminGrpcClient> {
    members_grpc_client: MGC,
    members_admin_grpc_client: MAGC,
    timeout: Duration,
}

impl<MGC: MembersGrpcClient, MAGC: MembersAdminGrpcClient> MembersApi<MGC, MAGC> {
    pub fn new(
        members_grpc_client: MGC,
        members_admin_grpc_client: MAGC,
        timeout: Duration,
    ) -> Self {
        Self {
            members_grpc_client,
            members_admin_grpc_client,
            timeout,
        }
    }

    pub fn router<S: Clone + Send + Sync + 'static>(self) -> Router<S> {
        Router::new()
            .route("/members", get(list::<MGC, MAGC>).post(create::<MGC, MAGC>))
            .route(
                "/members/bulk",
                post(bulk_import::<MGC, MAGC>).layer(DefaultBodyLimit::max(2 * 1024 * 1024)),
            )
            .route(
                "/members/{member_id}",
                get(details::<MGC, MAGC>)
                    .put(update::<MGC, MAGC>)
                    .delete(archive::<MGC, MAGC>),
            )
            .with_state(self)
    }
}

#[utoipa::path(
    put,
    path = "/api/members/{member_id}",
    params(("member_id" = i32, Path)),
    request_body = MemberUpdate,
    responses(UpdateResponse, UpstreamError),
)]
pub async fn update<MGC: MembersGrpcClient, MAGC: MembersAdminGrpcClient>(
    State(MembersApi {
        members_admin_grpc_client: client,
        timeout,
        ..
    }): State<MembersApi<MGC, MAGC>>,
    Path(member_id): Path<i32>,
    body: Result<Json<MemberUpdate>, JsonRejection>,
) -> Result<UpdateResponse, UpstreamError> {
    let Json(body) = body.map_err(|_| UpstreamError::InvalidArgument)?;
    let expected = body.expected_updated_at.clone();
    let request = body.into_request(member_id)?;

    let current = sensitive_member(client.clone(), timeout, member_id).await?;
    if current
        .updated_at
        .as_ref()
        .map(ToString::to_string)
        .as_deref()
        != Some(expected.as_str())
    {
        return Err(UpstreamError::Conflict);
    }

    let mut client = client;
    let member = grpc::call(
        GrpcMethod::UpdateMember,
        timeout,
        request,
        |request| async { client.update_member(request).await },
    )
    .await?
    .member
    .ok_or(UpstreamError::Failed)?;

    Ok(UpdateResponse::Success(member.into()))
}

#[utoipa::path(
    delete,
    path = "/api/members/{member_id}",
    params(("member_id" = i32, Path)),
    responses(ArchiveResponse, UpstreamError),
)]
pub async fn archive<MGC: MembersGrpcClient, MAGC: MembersAdminGrpcClient>(
    State(MembersApi {
        members_admin_grpc_client: mut client,
        timeout,
        ..
    }): State<MembersApi<MGC, MAGC>>,
    Path(member_id): Path<i32>,
) -> Result<ArchiveResponse, UpstreamError> {
    if member_id <= 0 {
        return Err(UpstreamError::InvalidArgument);
    }

    grpc::call(
        GrpcMethod::ArchiveMember,
        timeout,
        members::admin::ArchiveMemberRequest { member_id },
        |request| async { client.archive_member(request).await },
    )
    .await?
    .member
    .ok_or(UpstreamError::Failed)?;

    Ok(ArchiveResponse::Success)
}

#[utoipa::path(
    get,
    path = "/api/members/{member_id}",
    params(("member_id" = i32, Path)),
    responses(DetailsResponse, UpstreamError),
)]
pub async fn details<MGC: MembersGrpcClient, MAGC: MembersAdminGrpcClient>(
    State(MembersApi {
        members_admin_grpc_client: client,
        timeout,
        ..
    }): State<MembersApi<MGC, MAGC>>,
    Path(member_id): Path<i32>,
) -> Result<DetailsResponse, UpstreamError> {
    if member_id <= 0 {
        return Err(UpstreamError::InvalidArgument);
    }

    let member = sensitive_member(client, timeout, member_id).await?;
    Ok(DetailsResponse::Success(MemberDetails::from(member)))
}

#[utoipa::path(
    post,
    path = "/api/members",
    request_body = NewMember,
    responses(CreateResponse, UpstreamError),
)]
pub async fn create<MGC: MembersGrpcClient, MAGC: MembersAdminGrpcClient>(
    State(MembersApi {
        members_admin_grpc_client: mut client,
        timeout,
        ..
    }): State<MembersApi<MGC, MAGC>>,
    body: Result<Json<NewMember>, JsonRejection>,
) -> Result<CreateResponse, UpstreamError> {
    let Json(body) = body.map_err(|_| UpstreamError::InvalidArgument)?;

    let member = grpc::call(
        GrpcMethod::CreateMember,
        timeout,
        body.try_into()?,
        |request| async { client.create_member(request).await },
    )
    .await?
    .member
    .ok_or(UpstreamError::Failed)?;

    Ok(CreateResponse::Created(member.into()))
}

#[utoipa::path(
    post,
    path = "/api/members/bulk",
    request_body = BulkMembers,
    responses((status = 200, body = BulkResults), UpstreamError),
)]
pub async fn bulk_import<MGC: MembersGrpcClient, MAGC: MembersAdminGrpcClient>(
    State(MembersApi {
        members_admin_grpc_client: mut client,
        timeout,
        ..
    }): State<MembersApi<MGC, MAGC>>,
    body: Result<Json<BulkMembers>, JsonRejection>,
) -> Result<Json<BulkResults>, UpstreamError> {
    let Json(BulkMembers { members, mode }) = body.map_err(|_| UpstreamError::InvalidArgument)?;

    if members.is_empty() || members.len() > 1000 {
        return Err(UpstreamError::InvalidArgument);
    }

    let mut results = Vec::with_capacity(members.len());
    let mut operations = Vec::new();
    let mut indices = Vec::new();

    for (index, member) in members.into_iter().enumerate() {
        match CreateMemberRequest::try_from(member) {
            Ok(request) => {
                indices.push(index);
                operations.push(mode.operation(request));
            }
            Err(_) => results.push(BulkResult {
                index,
                outcome: BulkOutcome::Failed {
                    error: "Invalid member fields".into(),
                },
            }),
        }
    }

    if !operations.is_empty() {
        let response = grpc::call(
            GrpcMethod::BulkMemberOperations,
            timeout,
            BulkMemberOperationsRequest { operations },
            |request| async { client.bulk_member_operations(request).await },
        )
        .await?;

        // Upstream indexes results by position in `operations`, not by request row.
        let mut outcomes: BTreeMap<_, _> = response
            .results
            .into_iter()
            .map(|row| (row.index as usize, row.outcome))
            .collect();

        results.extend(
            indices
                .into_iter()
                .enumerate()
                .map(|(position, index)| BulkResult {
                    index,
                    outcome: outcomes.remove(&position).flatten().into(),
                }),
        );
    }

    results.sort_by_key(|row| row.index);
    Ok(Json(BulkResults { results }))
}

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct MemberListQuery {
    /// Case-insensitive prefix on name, email, Discord id, Discord username, Instagram, LinkedIn, and member id.
    #[serde(default)]
    query: String,
}

#[utoipa::path(
    get,
    path = "/api/members",
    params(MemberListQuery),
    responses(ListResponse, UpstreamError)
)]
pub async fn list<MGC: MembersGrpcClient, MAGC: MembersAdminGrpcClient>(
    State(MembersApi {
        members_grpc_client: client,
        timeout,
        ..
    }): State<MembersApi<MGC, MAGC>>,
    query: Result<Query<MemberListQuery>, QueryRejection>,
) -> Result<ListResponse, UpstreamError> {
    let Query(MemberListQuery { query }) = query.map_err(|_| UpstreamError::InvalidArgument)?;

    collect_members(client, timeout, &query)
        .await
        .map(ListResponse::Success)
}

async fn sensitive_member<MAGC: MembersAdminGrpcClient>(
    mut client: MAGC,
    timeout: Duration,
    member_id: i32,
) -> Result<SensitiveMember, UpstreamError> {
    grpc::call(
        GrpcMethod::GetSensitiveMember,
        timeout,
        members::GetMemberRequest {
            identifier: Some(members::get_member_request::Identifier::MemberId(member_id)),
        },
        |request| async { client.get_sensitive_member(request).await },
    )
    .await?
    .member
    .ok_or(UpstreamError::Failed)
}

async fn collect_members<MGC: MembersGrpcClient>(
    client: MGC,
    timeout: Duration,
    query: &str,
) -> Result<MemberList, UpstreamError> {
    tokio::time::timeout(timeout, async {
        let mut members = BTreeMap::new();
        let mut page_token = String::new();
        let mut seen = HashSet::new();

        loop {
            let mut client = client.clone();
            let response = grpc::call(
                GrpcMethod::ListMembers,
                timeout,
                members::ListMembersRequest {
                    include_archived: false,
                    page_size: 100,
                    page_token,
                },
                |request| async { client.list_members(request).await },
            )
            .await?;

            for member in response.members {
                members.insert(member.member_id, Member::from(member));
            }

            page_token = response.next_page_token;
            if page_token.is_empty() {
                break;
            }

            if !seen.insert(page_token.clone()) {
                return Err(UpstreamError::Failed);
            }
        }

        let members: Vec<_> = members.into_values().collect();
        let total = members.len();
        let query = query.trim().to_lowercase();
        let members = if query.is_empty() {
            members
        } else {
            members
                .into_iter()
                .filter(|member| matches_query(member, &query))
                .collect()
        };

        Ok(MemberList { total, members })
    })
    .await
    .map_err(|_| UpstreamError::Timeout)?
}

fn matches_query(member: &Member, query: &str) -> bool {
    let member_id = member.member_id.to_string();
    [
        Some(member.first_name.as_str()),
        Some(member.last_name.as_str()),
        Some(member.email.as_str()),
        member.discord_id.as_deref(),
        member.discord_username.as_deref(),
        member.instagram_username.as_deref(),
        member.linkedin_username.as_deref(),
        Some(member_id.as_str()),
    ]
    .into_iter()
    .flatten()
    .any(|value| value.to_lowercase().starts_with(query))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use axum::{
        Json,
        body::Body,
        extract::{
            FromRequest, Path, Query, State,
            rejection::{JsonRejection, QueryRejection},
        },
        http::{Request, StatusCode, Uri},
        response::{IntoResponse, Response},
    };
    use http_body_util::BodyExt;
    use members_grpc_service::members::{
        self,
        admin::{
            ArchiveMemberResponse, BulkMemberOperationsResponse, CreateMemberResponse,
            GetSensitiveMemberResponse, SensitiveMember, UpdateMemberResponse,
        },
    };
    use tonic::Code;

    use super::{MemberUpdate, MembersApi, NewMember, collect_members};
    use crate::grpc::error::UpstreamError;
    use crate::grpc::fake::{FakeAdmin, FakeMembers, Outcome, sent};

    type Api = State<MembersApi<FakeMembers, FakeAdmin>>;

    fn api_with_timeout(members: FakeMembers, admin: FakeAdmin, timeout: Duration) -> Api {
        State(MembersApi::new(members, admin, timeout))
    }

    fn api(admin: FakeAdmin) -> Api {
        api_with_timeout(FakeMembers::default(), admin, Duration::from_secs(1))
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

    fn found(member: Option<SensitiveMember>) -> Outcome<GetSensitiveMemberResponse> {
        Outcome::Reply(GetSensitiveMemberResponse { member })
    }

    fn current(updated_at: &str) -> Outcome<GetSensitiveMemberResponse> {
        found(Some(SensitiveMember {
            updated_at: Some(updated_at.parse().unwrap()),
            ..sensitive_member()
        }))
    }

    fn created() -> Outcome<CreateMemberResponse> {
        Outcome::Reply(CreateMemberResponse {
            member: Some(sensitive_member()),
        })
    }

    fn updated() -> Outcome<UpdateMemberResponse> {
        Outcome::Reply(UpdateMemberResponse {
            member: Some(sensitive_member()),
        })
    }

    fn page(ids: &[i32], next_page_token: &str) -> Outcome<members::ListMembersResponse> {
        Outcome::Reply(members::ListMembersResponse {
            members: ids
                .iter()
                .map(|&member_id| members::Member {
                    member_id,
                    joined_at: sensitive_member().joined_at,
                    ..Default::default()
                })
                .collect(),
            next_page_token: next_page_token.into(),
        })
    }

    async fn create_reply(
        body: Result<Json<NewMember>, JsonRejection>,
        create_response: Outcome<CreateMemberResponse>,
    ) -> (Response, FakeAdmin) {
        let admin = FakeAdmin {
            create_response,
            ..Default::default()
        };
        let response = super::create(api(admin.clone()), body)
            .await
            .into_response();
        (response, admin)
    }

    async fn list_reply(
        page: Outcome<members::ListMembersResponse>,
        query: Result<Query<super::MemberListQuery>, QueryRejection>,
    ) -> Response {
        super::list(
            api_with_timeout(
                FakeMembers::pages([page]),
                FakeAdmin::default(),
                Duration::from_secs(1),
            ),
            query,
        )
        .await
        .into_response()
    }

    fn member_list_query(query: &str) -> Result<Query<super::MemberListQuery>, QueryRejection> {
        let mut encoded = String::new();
        for byte in query.bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' => {
                    encoded.push(char::from(byte));
                }
                _ => encoded.push_str(&format!("%{byte:02X}")),
            }
        }
        let uri = format!("/api/members?query={encoded}");
        Query::try_from_uri(&uri.parse::<Uri>().unwrap())
    }

    fn search_page() -> Outcome<members::ListMembersResponse> {
        Outcome::Reply(members::ListMembersResponse {
            // Career and industry must not participate in search.
            members: vec![
                members::Member {
                    member_id: 10,
                    email: "angela@example.com".into(),
                    first_name: "Angela".into(),
                    last_name: "Yu".into(),
                    discord_id: Some("111".into()),
                    discord_username: Some("angela".into()),
                    linkedin_username: Some("angela-in".into()),
                    instagram_username: Some("angela.ig".into()),
                    career: Some("Foster".into()),
                    industry: Some("Frank".into()),
                    ..Default::default()
                },
                members::Member {
                    member_id: 42,
                    email: "nancy@example.com".into(),
                    first_name: "Nancy".into(),
                    last_name: "Huang".into(),
                    career: Some("Angela".into()),
                    industry: Some("gel".into()),
                    ..Default::default()
                },
                members::Member {
                    member_id: 7,
                    email: "frank@example.com".into(),
                    first_name: "Frank".into(),
                    last_name: "Lee".into(),
                    career: Some("oster".into()),
                    industry: Some("example.com".into()),
                    ..Default::default()
                },
                members::Member {
                    member_id: 100,
                    email: "anna@example.com".into(),
                    first_name: "Anna".into(),
                    last_name: "Foster".into(),
                    career: Some("111".into()),
                    industry: Some("Yu".into()),
                    ..Default::default()
                },
            ],
            next_page_token: String::new(),
        })
    }

    async fn details_reply(admin: FakeAdmin, member_id: i32) -> Response {
        super::details(api(admin), Path(member_id))
            .await
            .into_response()
    }

    async fn update_reply(
        admin: FakeAdmin,
        body: Result<Json<MemberUpdate>, JsonRejection>,
    ) -> Response {
        super::update(api(admin), Path(42), body)
            .await
            .into_response()
    }

    async fn archive_reply(admin: FakeAdmin, member_id: i32) -> Response {
        super::archive(api(admin), Path(member_id))
            .await
            .into_response()
    }

    async fn update_body(input: serde_json::Value) -> Result<Json<MemberUpdate>, JsonRejection> {
        let request = Request::builder()
            .header("content-type", "application/json")
            .body(Body::from(input.to_string()))
            .unwrap();

        Json::from_request(request, &()).await
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
        let (response, admin) = create_reply(request_body(VALID_MEMBER).await, created()).await;

        let sent = sent(&admin.create_requests);
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].email, "person@example.com");
        assert_eq!(sent[0].first_name, "First");
        assert_eq!(sent[0].last_name, "Last");

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

        let (response, admin) = create_reply(body, created()).await;

        let sent = sent(&admin.create_requests);
        let input = &sent[0];
        assert_eq!(input.linkedin_username.as_deref(), Some("person-linkedin"));
        assert_eq!(
            input.instagram_username.as_deref(),
            Some("person.instagram")
        );
        assert_eq!(input.career.as_deref(), Some("Engineer"));
        assert_eq!(input.industry.as_deref(), Some("Technology"));
        assert_eq!(input.profile.as_deref(), Some("Private notes"));

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
            let (response, _) = create_reply(request_body(body).await, Outcome::Unexpected).await;

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            assert_eq!(
                json_body(response).await,
                serde_json::json!({"error":"invalid request"})
            );
        }
    }

    #[tokio::test]
    async fn create_rejects_a_successful_rpc_without_a_member() {
        let (response, _) = create_reply(
            request_body(VALID_MEMBER).await,
            Outcome::Reply(CreateMemberResponse { member: None }),
        )
        .await;

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
                create_reply(request_body(VALID_MEMBER).await, Outcome::Fail(code))
                    .await
                    .0,
                list_reply(Outcome::Fail(code), member_list_query("")).await,
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
        let response = list_reply(page(&[42], ""), member_list_query("")).await;

        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["total"], 1);
        assert_eq!(body["members"][0]["member_id"], 42);
        assert_eq!(body["members"][0]["joined_at"], "2026-10-24T12:00:00Z");
    }

    #[tokio::test]
    async fn details_returns_full_record_with_admin_metadata() {
        let mut member = sensitive_member();
        member.linkedin_username = Some("person-linkedin".into());
        member.instagram_username = Some("person.photo".into());
        member.updated_at = Some("2020-01-01T00:00:00Z".parse().unwrap());
        member.career = Some("Engineer".into());
        member.industry = Some("Technology".into());
        member.role = Some(members::MemberRole::Mentor as i32);

        let admin = FakeAdmin {
            get_response: found(Some(member)),
            ..Default::default()
        };
        let response = details_reply(admin.clone(), 42).await;

        assert!(matches!(
            sent(&admin.get_requests)[0].identifier,
            Some(members::get_member_request::Identifier::MemberId(42))
        ));

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
            let response = details_reply(FakeAdmin::default(), id).await;
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        let response = details_reply(
            FakeAdmin {
                get_response: found(None),
                ..Default::default()
            },
            42,
        )
        .await;

        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);

        for (code, expected) in [
            (Code::NotFound, StatusCode::NOT_FOUND),
            (Code::PermissionDenied, StatusCode::FORBIDDEN),
            (Code::Unavailable, StatusCode::SERVICE_UNAVAILABLE),
        ] {
            let response = details_reply(
                FakeAdmin {
                    get_response: Outcome::Fail(code),
                    ..Default::default()
                },
                42,
            )
            .await;

            assert_eq!(response.status(), expected);
            assert!(
                !json_body(response)
                    .await
                    .to_string()
                    .contains("private upstream detail")
            );
        }

        let response = super::details(
            api_with_timeout(
                FakeMembers::default(),
                FakeAdmin {
                    get_response: Outcome::Hang,
                    ..Default::default()
                },
                Duration::from_millis(1),
            ),
            Path(42),
        )
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

            let response = update_reply(FakeAdmin::default(), update_body(input).await).await;

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

            let admin = FakeAdmin {
                get_response: current("2020-01-01T00:00:00Z"),
                update_response: updated(),
                ..Default::default()
            };
            let response = update_reply(admin.clone(), update_body(input).await).await;

            assert_eq!(response.status(), StatusCode::OK);

            let input = sent(&admin.update_requests).remove(0);
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
        }
    }

    #[tokio::test]
    async fn checked_update_rejects_stale_versions_without_writing() {
        let admin = FakeAdmin {
            get_response: current("2020-01-02T00:00:00Z"),
            ..Default::default()
        };
        let response = update_reply(
            admin,
            update_body(serde_json::json!({
                "expected_updated_at": "2020-01-01T00:00:00Z", "first_name": "Changed"
            }))
            .await,
        )
        .await;

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
        let admin = FakeAdmin {
            get_response: found(Some(SensitiveMember {
                updated_at: Some("2020-01-01T00:00:00Z".parse().unwrap()),
                discord_username: None,
                ..sensitive_member()
            })),
            update_response: updated(),
            ..Default::default()
        };
        let response = update_reply(
            admin.clone(),
            update_body(serde_json::json!({
                "expected_updated_at": "2020-01-01T00:00:00Z", "first_name": "Changed"
            }))
            .await,
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);

        let input = sent(&admin.update_requests).remove(0);
        assert_eq!(input.first_name.as_deref(), Some("Changed"));
        assert!(input.email.is_none());
        assert!(input.last_name.is_none());
        assert!(input.discord_id.is_none());
        assert!(input.discord_username.is_none());
        assert!(input.role.is_none());
        assert!(input.joined_at.is_none());
    }

    #[tokio::test]
    async fn checked_update_requires_version_and_stops_when_read_fails() {
        for input in [
            serde_json::json!({ "first_name": "Changed" }),
            serde_json::json!({ "expected_updated_at": "", "first_name": "Changed" }),
        ] {
            let response = update_reply(FakeAdmin::default(), update_body(input).await).await;

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        let response = update_reply(
            FakeAdmin {
                get_response: Outcome::Fail(Code::Unavailable),
                ..Default::default()
            },
            update_body(serde_json::json!({ "expected_updated_at": "2020-01-01T00:00:00Z", "first_name": "Changed" })).await,
        )
        .await;

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
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
            let admin = FakeAdmin {
                get_response: found(Some(SensitiveMember {
                    role,
                    ..sensitive_member()
                })),
                ..Default::default()
            };
            let response = details_reply(admin, 42).await;

            assert_eq!(response.status(), StatusCode::OK);
            let body = json_body(response).await;
            assert_eq!(body["role"], expected);
            assert_eq!(body["updated_at"], "");
        }
    }

    #[tokio::test]
    async fn checked_update_stops_on_missing_member_or_version() {
        for missing_member in [true, false] {
            let admin = FakeAdmin {
                get_response: found((!missing_member).then(sensitive_member)),
                ..Default::default()
            };
            let response = update_reply(
                admin.clone(),
                update_body(serde_json::json!({
                    "expected_updated_at": "2020-01-01T00:00:00Z", "first_name": "Changed"
                }))
                .await,
            )
            .await;

            assert!(matches!(
                sent(&admin.get_requests)[0].identifier,
                Some(members::get_member_request::Identifier::MemberId(42))
            ));
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
        let response = update_reply(
            FakeAdmin {
                get_response: Outcome::Hang,
                ..Default::default()
            },
            update_body(serde_json::json!({ "expected_updated_at": "2020-01-01T00:00:00Z" })).await,
        )
        .await;

        assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
        assert_eq!(
            json_body(response).await,
            serde_json::json!({"error": "upstream timeout"})
        );
    }

    #[tokio::test]
    async fn archive_returns_empty_204_for_archived_member() {
        let admin = FakeAdmin {
            archive_response: Outcome::Reply(ArchiveMemberResponse {
                member: Some(sensitive_member()),
            }),
            ..Default::default()
        };
        let response = archive_reply(admin.clone(), 42).await;

        assert_eq!(sent(&admin.archive_requests)[0].member_id, 42);
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
    async fn archive_maps_rpc_failures_without_leaking_details() {
        for (archive, status, message) in [
            (
                Outcome::Reply(ArchiveMemberResponse { member: None }),
                StatusCode::BAD_GATEWAY,
                "upstream request failed",
            ),
            (
                Outcome::Fail(Code::NotFound),
                StatusCode::NOT_FOUND,
                "not found",
            ),
            (
                Outcome::Fail(Code::PermissionDenied),
                StatusCode::FORBIDDEN,
                "permission denied",
            ),
            (
                Outcome::Fail(Code::Unavailable),
                StatusCode::SERVICE_UNAVAILABLE,
                "service unavailable",
            ),
            (
                Outcome::Fail(Code::DeadlineExceeded),
                StatusCode::GATEWAY_TIMEOUT,
                "upstream timeout",
            ),
        ] {
            let admin = FakeAdmin {
                archive_response: archive,
                ..Default::default()
            };
            let response = archive_reply(admin, 42).await;

            assert_eq!(response.status(), status);
            assert_eq!(
                json_body(response).await,
                serde_json::json!({ "error": message })
            );
        }
    }

    #[tokio::test]
    async fn archive_rejects_invalid_ids_without_calling_rpc() {
        for member_id in [0, -1] {
            let response = archive_reply(FakeAdmin::default(), member_id).await;

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            assert_eq!(
                json_body(response).await,
                serde_json::json!({ "error": "invalid request" })
            );
        }
    }

    #[tokio::test]
    async fn archive_enforces_configured_timeout() {
        let api = api_with_timeout(
            FakeMembers::default(),
            FakeAdmin {
                archive_response: Outcome::Hang,
                ..Default::default()
            },
            Duration::from_millis(50),
        );

        let response = tokio::time::timeout(Duration::from_secs(2), super::archive(api, Path(42)))
            .await
            .expect("archive handler must enforce its timeout")
            .into_response();

        assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
        assert_eq!(
            json_body(response).await,
            serde_json::json!({ "error": "upstream timeout" })
        );
    }

    #[tokio::test]
    async fn update_reads_current_version_then_writes_trimmed_changes() {
        let admin = FakeAdmin {
            get_response: current("2020-01-01T00:00:00Z"),
            update_response: Outcome::Reply(UpdateMemberResponse {
                member: Some(SensitiveMember {
                    first_name: "Changed".into(),
                    ..sensitive_member()
                }),
            }),
            ..Default::default()
        };
        let response = update_reply(
            admin.clone(),
            update_body(serde_json::json!({
                "expected_updated_at": "2020-01-01T00:00:00Z", "first_name": " Changed "
            }))
            .await,
        )
        .await;

        assert_eq!(sent(&admin.get_requests).len(), 1);
        let input = sent(&admin.update_requests).remove(0);
        assert_eq!(input.member_id, 42);
        assert_eq!(input.first_name.as_deref(), Some("Changed"));
        assert!(input.email.is_none());
        assert!(input.joined_at.is_none());

        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["member_id"], 42);
        assert_eq!(body["first_name"], "Changed");
        assert!(body.get("profile").is_none());
    }

    #[tokio::test]
    async fn update_maps_missing_member_and_write_errors() {
        for (update, status) in [
            (
                Outcome::Reply(UpdateMemberResponse { member: None }),
                StatusCode::BAD_GATEWAY,
            ),
            (Outcome::Fail(Code::AlreadyExists), StatusCode::CONFLICT),
        ] {
            let admin = FakeAdmin {
                get_response: current("2020-01-01T00:00:00Z"),
                update_response: update,
                ..Default::default()
            };
            let response = update_reply(
                admin,
                update_body(serde_json::json!({
                    "expected_updated_at": "2020-01-01T00:00:00Z", "first_name": "First"
                }))
                .await,
            )
            .await;

            assert_eq!(response.status(), status);
        }
    }

    fn bulk_outcome() -> Outcome<BulkMemberOperationsResponse> {
        use members::admin::{
            MemberCreated, MemberOperationFailed, MemberOperationResult, member_operation_result,
        };

        Outcome::Reply(BulkMemberOperationsResponse {
            results: vec![
                MemberOperationResult {
                    index: 1,
                    outcome: Some(member_operation_result::Outcome::Failed(
                        MemberOperationFailed {
                            error: "Duplicate email".into(),
                        },
                    )),
                },
                MemberOperationResult {
                    index: 0,
                    outcome: Some(member_operation_result::Outcome::Created(MemberCreated {
                        member: Some(sensitive_member()),
                    })),
                },
            ],
        })
    }

    #[tokio::test]
    async fn bulk_import_sends_create_operations_and_maps_indexed_results() {
        let admin = FakeAdmin {
            bulk_response: bulk_outcome(),
            ..Default::default()
        };
        let body = serde_json::from_value(serde_json::json!({ "members": [
        { "email": "person@example.com", "first_name": "A", "last_name": "B" },
        { "email": "person@example.com", "first_name": "A", "last_name": "B" }
    ] }))
        .unwrap();

        let response = super::bulk_import(api(admin.clone()), Ok(Json(body)))
            .await
            .into_response();

        let operations = sent(&admin.bulk_requests).remove(0).operations;
        assert_eq!(operations.len(), 2);
        for operation in operations {
            assert!(matches!(
                operation.operation,
                Some(members::admin::member_operation::Operation::Create(_))
            ));
        }

        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["results"][0]["index"], 0);
        assert_eq!(body["results"][0]["outcome"], "created");
        assert_eq!(body["results"][0]["member"]["member_id"], 42);
        assert_eq!(
            body["results"][1],
            serde_json::json!({ "index": 1, "outcome": "failed", "error": "Duplicate email" })
        );
    }

    #[tokio::test]
    async fn bulk_upsert_sends_upserts_without_blank_fields_and_maps_each_outcome() {
        use members::admin::{
            MemberCreated, MemberOperationFailed, MemberOperationResult, MemberUnchanged,
            MemberUpdated, member_operation::Operation, member_operation_result,
        };

        let updated = SensitiveMember {
            member_id: 7,
            career: Some("Engineer".into()),
            ..sensitive_member()
        };
        let admin = FakeAdmin {
            bulk_response: Outcome::Reply(BulkMemberOperationsResponse {
                results: vec![
                    MemberOperationResult {
                        index: 0,
                        outcome: Some(member_operation_result::Outcome::Updated(MemberUpdated {
                            before: Some(sensitive_member()),
                            after: Some(updated),
                            changed_fields: None,
                        })),
                    },
                    MemberOperationResult {
                        index: 1,
                        outcome: Some(member_operation_result::Outcome::Unchanged(
                            MemberUnchanged {
                                member: Some(sensitive_member()),
                            },
                        )),
                    },
                    MemberOperationResult {
                        index: 2,
                        outcome: Some(member_operation_result::Outcome::Created(MemberCreated {
                            member: Some(SensitiveMember {
                                member_id: 8,
                                ..sensitive_member()
                            }),
                        })),
                    },
                    MemberOperationResult {
                        index: 3,
                        outcome: Some(member_operation_result::Outcome::Failed(
                            MemberOperationFailed {
                                error: "Duplicate LinkedIn".into(),
                            },
                        )),
                    },
                ],
            }),
            ..Default::default()
        };
        // The invalid row comes first, so every valid row's CSV index is one past
        // its position in `operations`, which is what upstream numbers results by.
        let body = serde_json::from_value(serde_json::json!({ "mode": "upsert", "members": [
            { "email": " ", "first_name": "No", "last_name": "Email" },
            { "email": "person@example.com", "first_name": "A", "last_name": "B", "career": "Engineer", "industry": " " },
            { "email": "person@example.com", "first_name": "First", "last_name": "Last" },
            { "email": "new@example.com", "first_name": "N", "last_name": "M" },
            { "email": "taken@example.com", "first_name": "T", "last_name": "U" }
        ] }))
        .unwrap();

        let response = super::bulk_import(api(admin.clone()), Ok(Json(body)))
            .await
            .into_response();

        let operations = sent(&admin.bulk_requests).remove(0).operations;
        assert_eq!(operations.len(), 4, "invalid rows are not sent");
        let Some(Operation::Upsert(upsert)) = &operations[0].operation else {
            panic!("expected an upsert operation");
        };
        let member = upsert.member.as_ref().unwrap();
        assert_eq!(member.career.as_deref(), Some("Engineer"));
        assert_eq!(
            member.industry, None,
            "blank fields must not clear stored values"
        );
        assert_eq!(member.profile, None);
        assert!(
            operations
                .iter()
                .all(|operation| matches!(operation.operation, Some(Operation::Upsert(_))))
        );

        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        let summary: Vec<_> = body["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                (
                    row["index"].as_u64().unwrap(),
                    row["outcome"].as_str().unwrap(),
                    row["member"]["member_id"].as_i64(),
                    row["error"].as_str(),
                )
            })
            .collect();
        assert_eq!(
            summary,
            [
                (0, "failed", None, Some("Invalid member fields")),
                (1, "updated", Some(7), None),
                (2, "unchanged", Some(42), None),
                (3, "created", Some(8), None),
                (4, "failed", None, Some("Duplicate LinkedIn")),
            ]
        );
        assert_eq!(body["results"][1]["member"]["career"], "Engineer");
    }

    #[tokio::test]
    async fn bulk_rejects_an_unknown_mode_before_calling_upstream() {
        use tower::ServiceExt;

        let admin = FakeAdmin::default();
        let router = MembersApi::new(
            FakeMembers::default(),
            admin.clone(),
            Duration::from_secs(1),
        )
        .router::<()>();
        let body = serde_json::json!({ "mode": "replace", "members": [
            { "email": "person@example.com", "first_name": "A", "last_name": "B" }
        ] });

        let response = router
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/members/bulk")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(sent(&admin.bulk_requests).is_empty());
    }

    #[tokio::test]
    async fn bulk_route_enforces_the_encoded_request_limit() {
        use tower::ServiceExt;

        let router = MembersApi::new(
            FakeMembers::default(),
            FakeAdmin {
                bulk_response: bulk_outcome(),
                ..Default::default()
            },
            Duration::from_secs(1),
        )
        .router::<()>();
        let body = serde_json::json!({ "members": [
        { "email": "first@example.com", "first_name": "A", "last_name": "B", "profile": "x".repeat(1024 * 1024 - 200) },
        { "email": "second@example.com", "first_name": "C", "last_name": "D" },
    ] }).to_string();

        const LIMIT: usize = 2 * 1024 * 1024;

        for (size, status) in [
            (body.len(), StatusCode::OK),
            (LIMIT, StatusCode::OK),
            (LIMIT + 1, StatusCode::BAD_REQUEST),
        ] {
            let mut padded_body = body.clone();
            padded_body.extend(std::iter::repeat_n(' ', size - body.len()));

            let response = router
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/members/bulk")
                        .header("content-type", "application/json")
                        .body(Body::from(padded_body))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), status, "request size: {size}");
        }
    }

    #[tokio::test]
    async fn pagination_keeps_unique_members_and_passes_tokens_unchanged() {
        let client = FakeMembers::pages([page(&[1, 2], "0:2"), page(&[2, 3], "")]);

        let list = collect_members(client.clone(), Duration::from_secs(1), "")
            .await
            .unwrap();

        let requests = sent(&client.list_requests);
        assert_eq!(requests.len(), 2);
        for request in &requests {
            assert!(!request.include_archived);
            assert_eq!(request.page_size, 100);
        }
        assert!(requests[0].page_token.is_empty());
        assert_eq!(requests[1].page_token, "0:2");

        assert_eq!(list.total, 3);
        assert_eq!(
            list.members
                .iter()
                .map(|member| member.member_id)
                .collect::<Vec<_>>(),
            [1, 2, 3]
        );
    }

    #[tokio::test]
    async fn pagination_rejects_repeated_tokens_and_discards_partial_results() {
        let client = FakeMembers::pages([page(&[1], "0:2"), page(&[2], "0:2")]);

        let result = collect_members(client, Duration::from_secs(1), "").await;

        assert!(matches!(result, Err(UpstreamError::Failed)));
    }

    #[tokio::test]
    async fn pagination_has_a_deadline_even_if_upstream_never_finishes() {
        let client = FakeMembers::pages([Outcome::Hang]);

        let result = collect_members(client, Duration::from_millis(1), "").await;

        assert!(matches!(result, Err(UpstreamError::Timeout)));
    }

    #[tokio::test]
    async fn pagination_propagates_upstream_failure_after_a_successful_page() {
        let client = FakeMembers::pages([page(&[1], "0:1"), Outcome::Fail(Code::Unavailable)]);

        let result = collect_members(client.clone(), Duration::from_secs(1), "").await;

        assert_eq!(sent(&client.list_requests).len(), 2);
        assert!(matches!(result, Err(UpstreamError::Unavailable)));
    }

    #[tokio::test]
    async fn search_filters_by_prefix_and_keeps_the_unfiltered_total() {
        async fn search(
            query: Result<Query<super::MemberListQuery>, QueryRejection>,
        ) -> (u64, Vec<i64>) {
            let response = super::list(
                api_with_timeout(
                    FakeMembers::pages([search_page()]),
                    FakeAdmin::default(),
                    Duration::from_secs(1),
                ),
                query,
            )
            .await
            .into_response();

            assert_eq!(response.status(), StatusCode::OK);
            let body = json_body(response).await;
            let ids = body["members"]
                .as_array()
                .unwrap()
                .iter()
                .map(|member| member["member_id"].as_i64().unwrap())
                .collect();
            (body["total"].as_u64().unwrap(), ids)
        }

        let missing = Query::try_from_uri(&"/api/members".parse::<Uri>().unwrap());
        for query in [missing, member_list_query(""), member_list_query("   ")] {
            assert_eq!(search(query).await, (4, vec![7, 10, 42, 100]));
        }

        for query in ["ANG", "ang", "yu", " yu "] {
            assert_eq!(search(member_list_query(query)).await, (4, vec![10]));
        }

        assert_eq!(search(member_list_query("fos")).await, (4, vec![100]));
        assert_eq!(search(member_list_query("fra")).await, (4, vec![7]));
        assert_eq!(search(member_list_query("111")).await, (4, vec![10]));
        assert_eq!(search(member_list_query("angela-in")).await, (4, vec![10]));
        assert_eq!(search(member_list_query("angela.ig")).await, (4, vec![10]));
        assert_eq!(search(member_list_query("nancy@")).await, (4, vec![42]));
        assert_eq!(search(member_list_query("anna@")).await, (4, vec![100]));
        assert_eq!(
            search(member_list_query("example.com")).await,
            (4, Vec::new())
        );
        assert_eq!(search(member_list_query("100")).await, (4, vec![100]));
        assert_eq!(search(member_list_query("10")).await, (4, vec![10, 100]));
        assert_eq!(search(member_list_query("1")).await, (4, vec![10, 100]));
        assert_eq!(search(member_list_query("42")).await, (4, vec![42]));
        assert_eq!(search(member_list_query("gel")).await, (4, Vec::new()));
        assert_eq!(search(member_list_query("oster")).await, (4, Vec::new()));
    }
}
