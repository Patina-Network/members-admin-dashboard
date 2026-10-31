use std::{
    collections::{BTreeMap, HashSet},
    future::Future,
    time::Duration,
};

use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use members_grpc_service::members::{
    self,
    admin::{CreateMemberRequest, CreateMemberResponse, SensitiveMember},
};
use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    grpc::{self, error::UpstreamError},
    http::state::AppState,
};

#[derive(Clone, Copy, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MemberRole {
    Board,
    Mentor,
    Mentee,
}

impl From<MemberRole> for i32 {
    fn from(role: MemberRole) -> Self {
        match role {
            MemberRole::Board => members::MemberRole::Board as i32,
            MemberRole::Mentor => members::MemberRole::Mentor as i32,
            MemberRole::Mentee => members::MemberRole::Mentee as i32,
        }
    }
}

fn role_from_proto(role: Option<i32>) -> Option<MemberRole> {
    match role.and_then(|role| members::MemberRole::try_from(role).ok()) {
        Some(members::MemberRole::Board) => Some(MemberRole::Board),
        Some(members::MemberRole::Mentor) => Some(MemberRole::Mentor),
        Some(members::MemberRole::Mentee) => Some(MemberRole::Mentee),
        _ => None,
    }
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct NewMember {
    email: String,
    first_name: String,
    last_name: String,
    joined_at: Option<String>,
    discord_id: Option<String>,
    discord_username: Option<String>,
    linkedin_username: Option<String>,
    instagram_username: Option<String>,
    career: Option<String>,
    industry: Option<String>,
    profile: Option<String>,
    role: Option<MemberRole>,
}

impl TryFrom<NewMember> for CreateMemberRequest {
    type Error = UpstreamError;

    fn try_from(input: NewMember) -> Result<Self, Self::Error> {
        let email = input.email.trim().to_owned();
        let first_name = input.first_name.trim().to_owned();
        let last_name = input.last_name.trim().to_owned();

        if email.is_empty() || first_name.is_empty() || last_name.is_empty() {
            return Err(UpstreamError::InvalidArgument);
        }

        let joined_at = optional(input.joined_at)
            .map(|value| value.parse().map_err(|_| UpstreamError::InvalidArgument))
            .transpose()?;

        Ok(Self {
            email,
            first_name,
            last_name,
            joined_at,
            discord_id: optional(input.discord_id),
            discord_username: optional(input.discord_username),
            linkedin_username: optional(input.linkedin_username),
            instagram_username: optional(input.instagram_username),
            career: optional(input.career),
            industry: optional(input.industry),
            profile: optional(input.profile),
            role: input.role.map(Into::into),
        })
    }
}

fn validate_update_fields(
    input: &members::admin::UpdateMemberRequest,
) -> Result<(), UpstreamError> {
    if input.email.as_ref().is_some_and(|email| {
        email.chars().any(char::is_whitespace)
            || email.matches('@').count() != 1
            || !email.split_once('@').is_some_and(|(local, domain)| {
                !local.is_empty()
                    && domain.contains('.')
                    && !domain.starts_with('.')
                    && !domain.ends_with('.')
            })
    }) || input.first_name.as_ref().is_some_and(String::is_empty)
        || input.last_name.as_ref().is_some_and(String::is_empty)
    {
        return Err(UpstreamError::InvalidArgument);
    }

    if input.discord_id.is_some() != input.discord_username.is_some() {
        return Err(UpstreamError::InvalidArgument);
    }

    if let (Some(id), Some(username)) = (&input.discord_id, &input.discord_username)
        && (id.is_empty() != username.is_empty()
            || (!id.is_empty()
                && (!id.bytes().all(|byte| byte.is_ascii_digit())
                    || id.parse::<u64>().map_or(true, |id| id == 0))))
    {
        return Err(UpstreamError::InvalidArgument);
    }

    for value in [&input.linkedin_username, &input.instagram_username]
        .into_iter()
        .flatten()
    {
        if value
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '/' | '@' | '?' | '#'))
        {
            return Err(UpstreamError::InvalidArgument);
        }
    }

    Ok(())
}

fn deserialize_update<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

fn update_string(value: Option<Option<String>>) -> Option<String> {
    value.map(|value| optional(value).unwrap_or_default())
}

fn optional(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MemberUpdate {
    expected_updated_at: String,
    email: Option<String>,
    first_name: Option<String>,
    last_name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_update")]
    discord_id: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_update")]
    discord_username: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_update")]
    linkedin_username: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_update")]
    instagram_username: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_update")]
    career: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_update")]
    industry: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_update")]
    profile: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_update")]
    role: Option<Option<MemberRole>>,
}

impl MemberUpdate {
    fn into_request(
        self,
        member_id: i32,
    ) -> Result<members::admin::UpdateMemberRequest, UpstreamError> {
        if member_id <= 0 {
            return Err(UpstreamError::InvalidArgument);
        }

        if self.expected_updated_at.is_empty() {
            return Err(UpstreamError::InvalidArgument);
        }

        let request = members::admin::UpdateMemberRequest {
            member_id,
            email: self.email.map(|v| v.trim().to_owned()),
            first_name: self.first_name.map(|v| v.trim().to_owned()),
            last_name: self.last_name.map(|v| v.trim().to_owned()),
            joined_at: None,
            discord_id: update_string(self.discord_id),
            discord_username: update_string(self.discord_username),
            linkedin_username: update_string(self.linkedin_username),
            instagram_username: update_string(self.instagram_username),
            career: update_string(self.career),
            industry: update_string(self.industry),
            profile: update_string(self.profile),
            role: self.role.map(|role| {
                role.map(Into::into)
                    .unwrap_or(members::MemberRole::Unspecified as i32)
            }),
        };

        validate_update_fields(&request)?;
        Ok(request)
    }
}

#[derive(utoipa::IntoResponses)]
pub enum UpdateResponse {
    #[response(status = 200, description = "Member updated")]
    Success(Member),
}

impl IntoResponse for UpdateResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Success(member) => Json(member).into_response(),
        }
    }
}

#[utoipa::path(put, path = "/api/members/{member_id}", params(("member_id" = i32, Path)), request_body = MemberUpdate, responses(UpdateResponse, UpstreamError))]
pub async fn update(
    State(state): State<AppState>,
    Path(member_id): Path<i32>,
    body: Result<Json<MemberUpdate>, JsonRejection>,
) -> Result<UpdateResponse, UpstreamError> {
    let mut client = state.client.clone();
    let mut reader = state.client.clone();

    checked_update_with(
        member_id,
        body,
        state.config.members_admin_service_grpc_timeout,
        |request| async move { reader.get_sensitive_member(request).await },
        |request| async move { client.update_member(request).await },
    )
    .await
}

async fn checked_update_with<R, RFut, W, WFut>(
    member_id: i32,
    body: Result<Json<MemberUpdate>, JsonRejection>,
    timeout: Duration,
    read: R,
    write: W,
) -> Result<UpdateResponse, UpstreamError>
where
    R: FnOnce(tonic::Request<members::GetMemberRequest>) -> RFut,
    RFut: Future<
        Output = Result<tonic::Response<members::admin::GetSensitiveMemberResponse>, tonic::Status>,
    >,
    W: FnOnce(tonic::Request<members::admin::UpdateMemberRequest>) -> WFut,
    WFut: Future<
        Output = Result<tonic::Response<members::admin::UpdateMemberResponse>, tonic::Status>,
    >,
{
    let Json(body) = body.map_err(|_| UpstreamError::InvalidArgument)?;
    let expected = body.expected_updated_at.clone();
    let request = body.into_request(member_id)?;

    let current = grpc::call_admin(
        timeout,
        members::GetMemberRequest {
            identifier: Some(members::get_member_request::Identifier::MemberId(member_id)),
        },
        read,
    )
    .await?
    .member
    .ok_or(UpstreamError::Failed)?;

    if current
        .updated_at
        .as_ref()
        .map(ToString::to_string)
        .as_deref()
        != Some(expected.as_str())
    {
        return Err(UpstreamError::Conflict);
    }

    send_update(request, timeout, write).await
}

async fn send_update<F, Fut>(
    request: members::admin::UpdateMemberRequest,
    timeout: Duration,
    rpc: F,
) -> Result<UpdateResponse, UpstreamError>
where
    F: FnOnce(tonic::Request<members::admin::UpdateMemberRequest>) -> Fut,
    Fut: Future<
        Output = Result<tonic::Response<members::admin::UpdateMemberResponse>, tonic::Status>,
    >,
{
    let response = grpc::call_admin(timeout, request, rpc).await?;

    Ok(UpdateResponse::Success(
        response.member.ok_or(UpstreamError::Failed)?.into(),
    ))
}

#[derive(Debug, utoipa::IntoResponses)]
pub enum ArchiveResponse {
    #[response(status = 204, description = "Member archived, not permanently deleted")]
    Success,
}

impl IntoResponse for ArchiveResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Success => StatusCode::NO_CONTENT.into_response(),
        }
    }
}

#[utoipa::path(delete, path = "/api/members/{member_id}", params(("member_id" = i32, Path)), responses(ArchiveResponse, UpstreamError))]
pub async fn archive(
    State(state): State<AppState>,
    Path(member_id): Path<i32>,
) -> Result<ArchiveResponse, UpstreamError> {
    let mut client = state.client.clone();

    archive_with(
        member_id,
        state.config.members_admin_service_grpc_timeout,
        |request| async move { client.archive_member(request).await },
    )
    .await
}

async fn archive_with<F, Fut>(
    member_id: i32,
    timeout: Duration,
    rpc: F,
) -> Result<ArchiveResponse, UpstreamError>
where
    F: FnOnce(tonic::Request<members::admin::ArchiveMemberRequest>) -> Fut,
    Fut: Future<
        Output = Result<tonic::Response<members::admin::ArchiveMemberResponse>, tonic::Status>,
    >,
{
    if member_id <= 0 {
        return Err(UpstreamError::InvalidArgument);
    }

    let response = grpc::call_admin(
        timeout,
        members::admin::ArchiveMemberRequest { member_id },
        rpc,
    )
    .await?;
    response.member.ok_or(UpstreamError::Failed)?;

    Ok(ArchiveResponse::Success)
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct Member {
    member_id: i32,
    email: String,
    first_name: String,
    last_name: String,
    joined_at: Option<String>,
    discord_id: Option<String>,
    discord_username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    linkedin_username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    instagram_username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    career: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    industry: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<MemberRole>,
}

impl From<members::Member> for Member {
    fn from(value: members::Member) -> Self {
        Self {
            member_id: value.member_id,
            email: value.email,
            first_name: value.first_name,
            last_name: value.last_name,
            joined_at: value.joined_at.map(|v| v.to_string()),
            discord_id: value.discord_id,
            discord_username: value.discord_username,
            linkedin_username: value.linkedin_username,
            instagram_username: value.instagram_username,
            career: value.career,
            industry: value.industry,
            role: role_from_proto(value.role),
        }
    }
}

impl From<SensitiveMember> for Member {
    fn from(value: SensitiveMember) -> Self {
        Self {
            member_id: value.member_id,
            email: value.email,
            first_name: value.first_name,
            last_name: value.last_name,
            joined_at: value.joined_at.map(|v| v.to_string()),
            discord_id: value.discord_id,
            discord_username: value.discord_username,
            linkedin_username: value.linkedin_username,
            instagram_username: value.instagram_username,
            career: value.career,
            industry: value.industry,
            role: role_from_proto(value.role),
        }
    }
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct MemberDetails {
    updated_at: String,
    #[serde(flatten)]
    member: Member,
    profile: Option<String>,
}

#[derive(utoipa::IntoResponses)]
pub enum DetailsResponse {
    #[response(status = 200, description = "Full member details for editing")]
    Success(MemberDetails),
}

impl IntoResponse for DetailsResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Success(member) => Json(member).into_response(),
        }
    }
}

#[utoipa::path(get, path = "/api/members/{member_id}", params(("member_id" = i32, Path)), responses(DetailsResponse, UpstreamError))]
pub async fn details(
    State(state): State<AppState>,
    Path(member_id): Path<i32>,
) -> Result<DetailsResponse, UpstreamError> {
    let mut client = state.client.clone();

    details_with(
        member_id,
        state.config.members_admin_service_grpc_timeout,
        |request| async move { client.get_sensitive_member(request).await },
    )
    .await
}

async fn details_with<F, Fut>(
    member_id: i32,
    timeout: Duration,
    rpc: F,
) -> Result<DetailsResponse, UpstreamError>
where
    F: FnOnce(tonic::Request<members::GetMemberRequest>) -> Fut,
    Fut: Future<
        Output = Result<tonic::Response<members::admin::GetSensitiveMemberResponse>, tonic::Status>,
    >,
{
    if member_id <= 0 {
        return Err(UpstreamError::InvalidArgument);
    }

    let result = grpc::call_admin(
        timeout,
        members::GetMemberRequest {
            identifier: Some(members::get_member_request::Identifier::MemberId(member_id)),
        },
        rpc,
    )
    .await?;
    let value = result.member.ok_or(UpstreamError::Failed)?;

    Ok(DetailsResponse::Success(MemberDetails::from(value)))
}

impl From<SensitiveMember> for MemberDetails {
    fn from(value: SensitiveMember) -> Self {
        let updated_at = value
            .updated_at
            .map(|date| date.to_string())
            .unwrap_or_default();
        let profile = value.profile.clone();

        Self {
            updated_at,
            profile,
            member: value.into(),
        }
    }
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct MemberList {
    members: Vec<Member>,
    total: usize,
}

#[derive(utoipa::IntoResponses)]
pub enum CreateResponse {
    #[response(status = 201, description = "Member created")]
    Created(Member),
}

impl IntoResponse for CreateResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Created(member) => (StatusCode::CREATED, Json(member)).into_response(),
        }
    }
}

#[utoipa::path(post, path = "/api/members", request_body = NewMember, responses(CreateResponse, UpstreamError))]
pub async fn create(
    State(state): State<AppState>,
    body: Result<Json<NewMember>, JsonRejection>,
) -> Result<CreateResponse, UpstreamError> {
    let mut client = state.client.clone();

    create_with(
        body,
        state.config.members_admin_service_grpc_timeout,
        |request| async move { client.create_member(request).await },
    )
    .await
}

async fn create_with<F, Fut>(
    body: Result<Json<NewMember>, JsonRejection>,
    timeout: Duration,
    rpc: F,
) -> Result<CreateResponse, UpstreamError>
where
    F: FnOnce(tonic::Request<CreateMemberRequest>) -> Fut,
    Fut: Future<Output = Result<tonic::Response<CreateMemberResponse>, tonic::Status>>,
{
    let Json(body) = body.map_err(|_| UpstreamError::InvalidArgument)?;

    let response = grpc::call_admin(timeout, body.try_into()?, rpc).await?;
    let member = response.member.ok_or(UpstreamError::Failed)?;

    Ok(CreateResponse::Created(member.into()))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct BulkMembers {
    members: Vec<NewMember>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct BulkResult {
    index: usize,
    member: Option<Member>,
    error: Option<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct BulkResults {
    results: Vec<BulkResult>,
}

#[utoipa::path(post, path = "/api/members/bulk", request_body = BulkMembers, responses((status = 200, body = BulkResults), UpstreamError))]
pub async fn bulk_create(
    State(state): State<AppState>,
    body: Result<Json<BulkMembers>, JsonRejection>,
) -> Result<Json<BulkResults>, UpstreamError> {
    use members::admin::{
        BulkMemberOperationsRequest, MemberOperation, member_operation::Operation,
        member_operation_result::Outcome,
    };

    let Json(body) = body.map_err(|_| UpstreamError::InvalidArgument)?;
    if body.members.is_empty() || body.members.len() > 1000 {
        return Err(UpstreamError::InvalidArgument);
    }

    let mut results = Vec::new();
    let mut operations = Vec::new();
    let mut indices = Vec::new();

    for (index, member) in body.members.into_iter().enumerate() {
        match CreateMemberRequest::try_from(member) {
            Ok(request) => {
                indices.push(index);
                operations.push(MemberOperation {
                    operation: Some(Operation::Create(request)),
                });
            }
            Err(_) => results.push(BulkResult {
                index,
                member: None,
                error: Some("Invalid member fields".into()),
            }),
        }
    }

    if !operations.is_empty() {
        let mut client = state.client.clone();
        let response = grpc::call_admin(
            state.config.members_admin_service_grpc_timeout,
            BulkMemberOperationsRequest { operations },
            |request| async move { client.bulk_member_operations(request).await },
        )
        .await?;

        let mut outcomes: BTreeMap<_, _> = response
            .results
            .into_iter()
            .map(|row| (row.index as usize, row.outcome))
            .collect();

        for (position, index) in indices.into_iter().enumerate() {
            let outcome = outcomes.remove(&position).flatten();
            let (member, error) = match outcome {
                Some(Outcome::Created(created)) if created.member.is_some() => {
                    (created.member.map(Into::into), None)
                }
                Some(Outcome::Failed(failed)) => (None, Some(failed.error)),
                _ => (
                    None,
                    Some("Could not confirm result. Refresh before retrying.".into()),
                ),
            };

            results.push(BulkResult {
                index,
                member,
                error,
            });
        }
    }

    results.sort_by_key(|row| row.index);
    Ok(Json(BulkResults { results }))
}

#[derive(utoipa::IntoResponses)]
pub enum ListResponse {
    #[response(status = 200, description = "Members and total count")]
    Success(MemberList),
}

impl IntoResponse for ListResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Success(list) => Json(list).into_response(),
        }
    }
}

#[utoipa::path(get, path = "/api/members", responses(ListResponse, UpstreamError))]
pub async fn list(State(state): State<AppState>) -> Result<ListResponse, UpstreamError> {
    let timeout = state.config.members_admin_service_grpc_timeout;

    collect_members(timeout, |message| {
        let mut client = state.members_client.clone();
        async move { grpc::call(timeout, message, |request| client.list_members(request)).await }
    })
    .await
    .map(ListResponse::Success)
}

async fn collect_members<F, Fut>(
    timeout: Duration,
    mut fetch_page: F,
) -> Result<MemberList, UpstreamError>
where
    F: FnMut(members::ListMembersRequest) -> Fut,
    Fut: Future<Output = Result<members::ListMembersResponse, UpstreamError>>,
{
    tokio::time::timeout(timeout, async {
        let mut members = BTreeMap::new();
        let mut page_token = String::new();
        let mut seen = HashSet::new();

        loop {
            let response = fetch_page(members::ListMembersRequest {
                include_archived: false,
                page_size: 100,
                page_token,
            })
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
        Ok(MemberList {
            total: members.len(),
            members,
        })
    })
    .await
    .map_err(|_| UpstreamError::Timeout)?
}

#[cfg(test)]
mod tests {
    use std::{future::Future, time::Duration};

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

    use super::{
        CreateMemberRequest, ListResponse, Member, MemberUpdate, NewMember, UpdateResponse,
        UpstreamError, collect_members, create_with, send_update,
    };
    use crate::http::state::test_state;

    async fn update_with<F, Fut>(
        member_id: i32,
        body: Result<Json<MemberUpdate>, JsonRejection>,
        timeout: Duration,
        rpc: F,
    ) -> Result<UpdateResponse, UpstreamError>
    where
        F: FnOnce(tonic::Request<members::admin::UpdateMemberRequest>) -> Fut,
        Fut: Future<
            Output = Result<tonic::Response<members::admin::UpdateMemberResponse>, tonic::Status>,
        >,
    {
        let Json(body) = body.map_err(|_| UpstreamError::InvalidArgument)?;
        send_update(body.into_request(member_id)?, timeout, rpc).await
    }

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

        let response =
            super::details_with(42, Duration::from_millis(1), |_| std::future::pending())
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

            let response = update_with(
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
            let response = update_with(
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

    struct MemberGrpc {
        reply: Option<Reply>,
    }

    #[tonic::async_trait]
    impl members::admin::members_admin_service_server::MembersAdminService for MemberGrpc {
        async fn get_sensitive_member(
            &self,
            request: tonic::Request<members::GetMemberRequest>,
        ) -> Result<GrpcResponse<members::admin::GetSensitiveMemberResponse>, Status> {
            assert_eq!(request.metadata().get("profile").unwrap(), "core_admin");
            assert!(matches!(
                request.into_inner().identifier,
                Some(members::get_member_request::Identifier::MemberId(42))
            ));

            let mut member = sensitive_member();
            member.updated_at = Some("2020-01-01T00:00:00Z".parse().unwrap());

            Ok(GrpcResponse::new(
                members::admin::GetSensitiveMemberResponse {
                    member: Some(member),
                },
            ))
        }

        async fn list_sensitive_members(
            &self,
            _: tonic::Request<members::ListMembersRequest>,
        ) -> Result<GrpcResponse<members::admin::ListSensitiveMembersResponse>, Status> {
            panic!("archive must not call list_sensitive_members")
        }

        async fn create_member(
            &self,
            request: tonic::Request<members::admin::CreateMemberRequest>,
        ) -> Result<GrpcResponse<members::admin::CreateMemberResponse>, Status> {
            assert_eq!(request.metadata().get("profile").unwrap(), "core_admin");
            assert!(request.metadata().contains_key("grpc-timeout"));

            let input = request.into_inner();
            assert_eq!(input.email, "person@example.com");
            assert_eq!(input.first_name, "First");
            assert_eq!(input.last_name, "Last");

            Ok(GrpcResponse::new(CreateMemberResponse {
                member: Some(sensitive_member()),
            }))
        }

        async fn update_member(
            &self,
            request: tonic::Request<members::admin::UpdateMemberRequest>,
        ) -> Result<GrpcResponse<members::admin::UpdateMemberResponse>, Status> {
            assert_eq!(request.metadata().get("profile").unwrap(), "core_admin");
            assert!(request.metadata().contains_key("grpc-timeout"));

            let input = request.into_inner();
            assert_eq!(input.member_id, 42);
            assert_eq!(input.first_name.as_deref(), Some("Changed"));
            assert!(input.email.is_none());
            assert!(input.joined_at.is_none());

            Ok(GrpcResponse::new(members::admin::UpdateMemberResponse {
                member: Some(SensitiveMember {
                    first_name: "Changed".into(),
                    ..sensitive_member()
                }),
            }))
        }

        async fn bulk_member_operations(
            &self,
            request: tonic::Request<members::admin::BulkMemberOperationsRequest>,
        ) -> Result<GrpcResponse<members::admin::BulkMemberOperationsResponse>, Status> {
            use members::admin::{
                BulkMemberOperationsResponse, MemberCreated, MemberOperationFailed,
                MemberOperationResult, member_operation, member_operation_result,
            };

            assert_eq!(request.metadata().get("profile").unwrap(), "core_admin");
            assert!(request.metadata().contains_key("grpc-timeout"));

            let operations = request.into_inner().operations;
            assert_eq!(operations.len(), 2);

            for operation in operations {
                assert!(matches!(
                    operation.operation,
                    Some(member_operation::Operation::Create(_))
                ));
            }

            Ok(GrpcResponse::new(BulkMemberOperationsResponse {
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
            }))
        }

        async fn archive_member(
            &self,
            request: tonic::Request<members::admin::ArchiveMemberRequest>,
        ) -> Result<GrpcResponse<members::admin::ArchiveMemberResponse>, Status> {
            assert_eq!(request.metadata().get("profile").unwrap(), "core_admin");
            assert!(request.metadata().contains_key("grpc-timeout"));
            assert_eq!(request.into_inner().member_id, 42);

            match self.reply {
                Some(Reply::Success) => {
                    Ok(GrpcResponse::new(members::admin::ArchiveMemberResponse {
                        member: Some(sensitive_member()),
                    }))
                }
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

    struct MemberServer(tokio::task::JoinHandle<()>);

    impl Drop for MemberServer {
        fn drop(&mut self) {
            self.0.abort();
        }
    }

    async fn member_server(reply: Option<Reply>) -> (crate::http::state::AppState, MemberServer) {
        use members::admin::members_admin_service_server::MembersAdminServiceServer;
        use tokio_stream::wrappers::TcpListenerStream;

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();

        let server = MemberServer(tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(MembersAdminServiceServer::new(MemberGrpc { reply }))
                .serve_with_incoming(TcpListenerStream::new(listener))
                .await
                .unwrap();
        }));

        let channel = tonic::transport::Endpoint::from_shared(format!("http://{address}"))
            .unwrap()
            .connect()
            .await
            .unwrap();

        let mut state = test_state();
        state.client =
            members::admin::members_admin_service_client::MembersAdminServiceClient::new(channel);

        (state, server)
    }

    #[tokio::test]
    async fn archive_handler_calls_generated_client_and_returns_empty_204() {
        let (state, _server) = member_server(Some(Reply::Success)).await;

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
            let (state, _server) = member_server(Some(reply)).await;
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
            let state = test_state();
            let response =
                super::archive(axum::extract::State(state), axum::extract::Path(member_id))
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
        let (mut state, _server) = member_server(None).await;
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
        let mut state = test_state();
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

    #[tokio::test]
    async fn create_handler_calls_generated_client_and_returns_created_member() {
        let (state, _server) = member_server(Some(Reply::Success)).await;

        let response = super::create(
            axum::extract::State(state),
            request_body(VALID_MEMBER).await,
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::CREATED);
        let body = json_body(response).await;
        assert_eq!(body["member_id"], 42);
        assert_eq!(body["email"], "person@example.com");
        assert!(body.get("profile").is_none());
    }

    #[tokio::test]
    async fn details_handler_calls_generated_client_and_returns_version() {
        let (state, _server) = member_server(Some(Reply::Success)).await;

        let response = super::details(axum::extract::State(state), axum::extract::Path(42))
            .await
            .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["member_id"], 42);
        assert_eq!(body["updated_at"], "2020-01-01T00:00:00Z");
        assert_eq!(body["profile"], "private notes");
    }

    #[tokio::test]
    async fn update_handler_reads_current_version_then_calls_generated_write_client() {
        let (state, _server) = member_server(Some(Reply::Success)).await;

        let body = serde_json::from_value(serde_json::json!({
            "expected_updated_at": "2020-01-01T00:00:00Z", "first_name": " Changed "
        }))
        .unwrap();

        let response = super::update(
            axum::extract::State(state),
            axum::extract::Path(42),
            Ok(Json(body)),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["member_id"], 42);
        assert_eq!(body["first_name"], "Changed");
        assert!(body.get("profile").is_none());
    }

    #[tokio::test]
    async fn archive_validates_id_and_forwards_admin_metadata() {
        for id in [0, -1] {
            let result = super::archive_with(id, Duration::from_secs(1), |_| async {
                panic!("invalid IDs must not reach gRPC")
            })
            .await;

            assert!(matches!(result, Err(UpstreamError::InvalidArgument)));
        }

        for reply in [0, 1, 2] {
            let result = super::archive_with(42, Duration::from_secs(1), |request| async move {
                assert_eq!(request.metadata().get("profile").unwrap(), "core_admin");
                assert!(request.metadata().contains_key("grpc-timeout"));
                assert_eq!(request.into_inner().member_id, 42);

                if reply == 2 {
                    return Err(tonic::Status::not_found("private details"));
                }

                Ok(tonic::Response::new(
                    members::admin::ArchiveMemberResponse {
                        member: (reply == 0).then(|| SensitiveMember {
                            member_id: 42,
                            ..Default::default()
                        }),
                    },
                ))
            })
            .await;

            match reply {
                0 => assert_eq!(
                    axum::response::IntoResponse::into_response(result.unwrap()).status(),
                    axum::http::StatusCode::NO_CONTENT
                ),
                1 => assert!(matches!(result, Err(UpstreamError::Failed))),
                _ => assert!(result.is_err()),
            }
        }
    }

    fn new_member() -> NewMember {
        NewMember {
            email: " person@example.com ".into(),
            first_name: " First ".into(),
            last_name: " Last ".into(),
            joined_at: Some("2026-10-24T12:00:00Z".into()),
            discord_id: Some(" ".into()),
            discord_username: None,
            linkedin_username: None,
            instagram_username: None,
            career: None,
            industry: None,
            profile: None,
            role: None,
        }
    }

    #[tokio::test]
    async fn update_forwards_admin_metadata_and_propagates_errors() {
        for reply in [0, 1, 2] {
            let body = super::MemberUpdate {
                expected_updated_at: "2020-01-01T00:00:00Z".into(),
                email: Some("person@example.com".into()),
                first_name: Some("First".into()),
                last_name: Some("Last".into()),
                discord_id: None,
                discord_username: None,
                linkedin_username: None,
                instagram_username: None,
                career: None,
                industry: None,
                profile: None,
                role: None,
            };

            let result = update_with(
                42,
                Ok(axum::Json(body)),
                Duration::from_secs(1),
                |request| async move {
                    assert_eq!(request.metadata().get("profile").unwrap(), "core_admin");
                    assert!(request.metadata().contains_key("grpc-timeout"));
                    assert_eq!(request.into_inner().member_id, 42);

                    if reply == 2 {
                        return Err(tonic::Status::already_exists("private"));
                    }

                    Ok(tonic::Response::new(members::admin::UpdateMemberResponse {
                        member: (reply == 0).then(|| SensitiveMember {
                            member_id: 42,
                            ..Default::default()
                        }),
                    }))
                },
            )
            .await;

            match reply {
                0 => {
                    let super::UpdateResponse::Success(member) = result.unwrap();
                    assert_eq!(member.member_id, 42);
                }
                1 => assert!(matches!(result, Err(UpstreamError::Failed))),
                _ => assert!(result.is_err()),
            }
        }
    }

    #[test]
    fn update_forwards_all_form_fields() {
        let input: super::MemberUpdate = serde_json::from_value(serde_json::json!({
        "expected_updated_at": "2020-01-01T00:00:00Z", "email": "person@example.com", "first_name": "First", "last_name": "Last",
        "linkedin_username": " alex ",
        "instagram_username": " alex.photo ", "career": " Engineer ",
        "industry": " Technology ", "profile": " Private notes ", "role": "board"
    }))
    .unwrap();

        let request = input.into_request(42).unwrap();

        assert!(request.joined_at.is_none());
        assert_eq!(request.linkedin_username.as_deref(), Some("alex"));
        assert_eq!(request.instagram_username.as_deref(), Some("alex.photo"));
        assert_eq!(request.career.as_deref(), Some("Engineer"));
        assert_eq!(request.industry.as_deref(), Some("Technology"));
        assert_eq!(request.profile.as_deref(), Some("Private notes"));
        assert_eq!(request.role, Some(members::MemberRole::Board as i32));
    }

    #[test]
    fn update_normalizes_and_clears_optional_fields() {
        let input = super::MemberUpdate {
            expected_updated_at: "2020-01-01T00:00:00Z".into(),
            email: Some(" person@example.com ".into()),
            first_name: Some(" First ".into()),
            last_name: Some(" Last ".into()),
            discord_id: Some(None),
            discord_username: Some(Some(" ".into())),
            linkedin_username: Some(None),
            instagram_username: None,
            career: None,
            industry: None,
            profile: Some(None),
            role: None,
        };

        let request = input.into_request(42).unwrap();

        assert_eq!(request.member_id, 42);
        assert_eq!(request.email.as_deref(), Some("person@example.com"));
        assert_eq!(request.discord_id.as_deref(), Some(""));
        assert_eq!(request.discord_username.as_deref(), Some(""));
        assert_eq!(request.profile.as_deref(), Some(""));
        assert_eq!(request.linkedin_username.as_deref(), Some(""));
        assert!(request.joined_at.is_none());
    }

    #[test]
    fn update_checks_discord_numeric_boundaries_and_required_names() {
        for (id, valid) in [
            ("0", false),
            ("-1", false),
            ("+1", false),
            ("abc", false),
            ("18446744073709551616", false),
            ("18446744073709551615", true),
            ("123", true),
        ] {
            let input = super::MemberUpdate {
                expected_updated_at: "2020-01-01T00:00:00Z".into(),
                email: Some("person@example.com".into()),
                first_name: Some("First".into()),
                last_name: Some("Last".into()),
                discord_id: Some(Some(id.into())),
                discord_username: Some(Some("testuser".into())),
                linkedin_username: None,
                instagram_username: None,
                career: None,
                industry: None,
                profile: None,
                role: None,
            };

            assert_eq!(input.into_request(1).is_ok(), valid, "{id}");
        }

        let input = super::MemberUpdate {
            expected_updated_at: "2020-01-01T00:00:00Z".into(),
            email: Some("person@example.com".into()),
            first_name: Some(" ".into()),
            last_name: Some("Last".into()),
            discord_id: None,
            discord_username: None,
            linkedin_username: None,
            instagram_username: None,
            career: None,
            industry: None,
            profile: None,
            role: None,
        };

        assert!(matches!(
            input.into_request(1),
            Err(UpstreamError::InvalidArgument)
        ));
    }

    #[test]
    fn update_rejects_invalid_id_email_and_discord_pair() {
        for (id, email, discord_id) in [
            (0, "person@example.com", None),
            (1, "bad", None),
            (1, "person@example.com", Some("123")),
        ] {
            let input = super::MemberUpdate {
                expected_updated_at: "2020-01-01T00:00:00Z".into(),
                email: Some(email.into()),
                first_name: Some("First".into()),
                last_name: Some("Last".into()),
                discord_id: discord_id.map(|id| Some(id.to_owned())),
                discord_username: None,
                linkedin_username: None,
                instagram_username: None,
                career: None,
                industry: None,
                profile: None,
                role: None,
            };

            assert!(matches!(
                input.into_request(id),
                Err(UpstreamError::InvalidArgument)
            ));
        }
    }

    #[test]
    fn forwards_member_roles() {
        for (role, expected) in [
            (super::MemberRole::Board, members::MemberRole::Board),
            (super::MemberRole::Mentor, members::MemberRole::Mentor),
            (super::MemberRole::Mentee, members::MemberRole::Mentee),
        ] {
            let mut input = new_member();
            input.role = Some(role);
            let request = CreateMemberRequest::try_from(input).unwrap();

            assert_eq!(request.role, Some(expected as i32));
        }

        assert!(
            CreateMemberRequest::try_from(new_member())
                .unwrap()
                .role
                .is_none()
        );
    }

    #[test]
    fn normalizes_fields_and_parses_timestamp() {
        let request = CreateMemberRequest::try_from(new_member()).unwrap();

        assert_eq!(request.email, "person@example.com");
        assert_eq!(request.first_name, "First");
        assert_eq!(request.last_name, "Last");
        assert!(request.discord_id.is_none());
        assert_eq!(
            request.joined_at.unwrap().to_string(),
            "2026-10-24T12:00:00Z"
        );
    }

    #[test]
    fn create_response_preserves_id_and_excludes_sensitive_fields() {
        let member = Member::from(SensitiveMember {
            member_id: 42,
            profile: Some("private notes".into()),
            email: "person@example.com".into(),
            ..Default::default()
        });

        let body = serde_json::to_value(member).unwrap();

        assert_eq!(body["member_id"], 42);
        assert!(body.get("profile").is_none());
    }

    #[tokio::test]
    async fn pagination_keeps_unique_members_and_passes_tokens_unchanged() {
        let mut page = 0;

        let list = collect_members(Duration::from_secs(1), |request| {
            assert!(!request.include_archived);
            assert_eq!(request.page_size, 100);

            page += 1;
            let (ids, next_page_token) = if page == 1 {
                assert!(request.page_token.is_empty());
                (vec![1, 2], "0:2")
            } else {
                assert_eq!(request.page_token, "0:2");
                (vec![2, 3], "")
            };

            async move {
                Ok(members::ListMembersResponse {
                    members: ids
                        .into_iter()
                        .map(|member_id| members::Member {
                            member_id,
                            ..Default::default()
                        })
                        .collect(),
                    next_page_token: next_page_token.into(),
                })
            }
        })
        .await
        .unwrap();

        assert_eq!(page, 2);
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
        let result = collect_members(Duration::from_secs(1), |_| async {
            Ok(members::ListMembersResponse {
                members: vec![],
                next_page_token: "0:2".into(),
            })
        })
        .await;

        assert!(matches!(result, Err(UpstreamError::Failed)));
    }

    #[tokio::test]
    async fn pagination_has_a_deadline_even_if_upstream_never_finishes() {
        let result = collect_members(Duration::from_millis(1), |_| std::future::pending()).await;

        assert!(matches!(result, Err(UpstreamError::Timeout)));
    }

    #[tokio::test]
    async fn pagination_propagates_upstream_failure_after_a_successful_page() {
        let mut page = 0;

        let result = collect_members(Duration::from_secs(1), |_| {
            page += 1;
            let response = if page == 1 {
                Ok(members::ListMembersResponse {
                    members: vec![members::Member {
                        member_id: 1,
                        ..Default::default()
                    }],
                    next_page_token: "0:1".into(),
                })
            } else {
                Err(UpstreamError::Unavailable)
            };

            async move { response }
        })
        .await;

        assert_eq!(page, 2);
        assert!(matches!(result, Err(UpstreamError::Unavailable)));
    }

    #[test]
    fn preserves_optional_fields_and_leaves_service_defaults_unset() {
        let input = NewMember {
            joined_at: None,
            discord_id: Some(" 123 ".into()),
            discord_username: Some(" person ".into()),
            linkedin_username: Some(" person-linkedin ".into()),
            instagram_username: Some(" person.instagram ".into()),
            career: Some(" Engineer ".into()),
            industry: Some(" Technology ".into()),
            profile: Some(" Private notes ".into()),
            ..new_member()
        };
        let request = CreateMemberRequest::try_from(input).unwrap();

        assert_eq!(request.discord_id.as_deref(), Some("123"));
        assert_eq!(request.discord_username.as_deref(), Some("person"));
        assert_eq!(
            request.linkedin_username.as_deref(),
            Some("person-linkedin")
        );
        assert_eq!(
            request.instagram_username.as_deref(),
            Some("person.instagram")
        );
        assert_eq!(request.career.as_deref(), Some("Engineer"));
        assert_eq!(request.industry.as_deref(), Some("Technology"));
        assert_eq!(request.profile.as_deref(), Some("Private notes"));
        assert!(request.joined_at.is_none());
    }

    #[test]
    fn blank_extra_fields_are_omitted() {
        let input = NewMember {
            linkedin_username: Some(" ".into()),
            instagram_username: Some("\t".into()),
            career: Some(" ".into()),
            industry: Some(" ".into()),
            profile: Some("\n".into()),
            ..new_member()
        };
        let request = CreateMemberRequest::try_from(input).unwrap();

        assert!(request.linkedin_username.is_none());
        assert!(request.instagram_username.is_none());
        assert!(request.career.is_none());
        assert!(request.industry.is_none());
        assert!(request.profile.is_none());
    }

    #[test]
    fn rejects_invalid_fields() {
        for field in ["email", "first_name", "last_name", "joined_at"] {
            let mut body = new_member();
            match field {
                "email" => body.email = " ".into(),
                "first_name" => body.first_name = " ".into(),
                "last_name" => body.last_name = " ".into(),
                _ => body.joined_at = Some("invalid".into()),
            }

            assert!(matches!(
                CreateMemberRequest::try_from(body),
                Err(UpstreamError::InvalidArgument)
            ));
        }
    }

    #[test]
    fn public_member_fields_match_details_without_exposing_profile() {
        for (role, expected) in [
            (None, None),
            (Some(0), None),
            (Some(1), Some("board")),
            (Some(2), Some("mentee")),
            (Some(3), Some("mentor")),
            (Some(99), None),
        ] {
            let stored = SensitiveMember {
                member_id: 42,
                linkedin_username: Some("alex".into()),
                instagram_username: Some("alex.photo".into()),
                career: Some("Engineer".into()),
                industry: Some("Technology".into()),
                profile: Some("Private notes".into()),
                role,
                ..Default::default()
            };

            let public = serde_json::to_value(Member::from(stored.clone())).unwrap();
            let details = serde_json::to_value(super::MemberDetails::from(stored)).unwrap();

            for field in [
                "linkedin_username",
                "instagram_username",
                "career",
                "industry",
                "role",
            ] {
                assert_eq!(public.get(field), details.get(field), "{field}");
            }

            assert_eq!(public["linkedin_username"], "alex");
            assert_eq!(public["instagram_username"], "alex.photo");
            assert_eq!(public["career"], "Engineer");
            assert_eq!(public["industry"], "Technology");
            assert_eq!(
                public.get("role").and_then(|value| value.as_str()),
                expected
            );
            assert!(public.get("profile").is_none());
            assert_eq!(details["profile"], "Private notes");
        }
    }

    #[tokio::test]
    async fn bulk_handler_uses_create_operations_and_maps_indexed_results() {
        let (state, _server) = member_server(Some(Reply::Success)).await;

        let body = serde_json::from_value(serde_json::json!({ "members": [
        { "email": "person@example.com", "first_name": "A", "last_name": "B" },
        { "email": "person@example.com", "first_name": "A", "last_name": "B" }
    ] }))
        .unwrap();

        let response = super::bulk_create(axum::extract::State(state), Ok(Json(body)))
            .await
            .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["results"][0]["index"], 0);
        assert_eq!(body["results"][0]["member"]["member_id"], 42);
        assert_eq!(body["results"][1]["error"], "Duplicate email");
    }

    #[tokio::test]
    async fn bulk_http_route_enforces_the_encoded_request_limit() {
        use crate::auth::session::{self, Principal};
        use axum_extra::extract::CookieJar;
        use tower::ServiceExt;

        let (state, _server) = member_server(Some(Reply::Success)).await;
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
        let session_cookie = jar.get("members_session").unwrap();
        let cookie = format!("members_session={}", session_cookie.value());

        let router = crate::http::app(state);
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
                        .uri("/api/members/bulk")
                        .header("cookie", &cookie)
                        .header("x-csrf-token", "csrf")
                        .header("content-type", "application/json")
                        .body(Body::from(padded_body))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), status, "request size: {size}");
        }
    }
}
