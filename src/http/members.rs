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

// Omitted fields preserve stored values; explicit null clears optional fields.
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
/// Optional fields omitted from an update are preserved. Explicit null or an empty string
/// clears optional text fields; null clears role. Joined dates cannot be updated.
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

// This preflight detects stale forms. Atomic compare-and-update requires upstream RPC support.
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
        }
    }
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct MemberDetails {
    updated_at: String,
    #[serde(flatten)]
    member: Member,
    linkedin_username: Option<String>,
    instagram_username: Option<String>,
    career: Option<String>,
    industry: Option<String>,
    profile: Option<String>,
    role: Option<MemberRole>,
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
        Self {
            updated_at: value
                .updated_at
                .map(|date| date.to_string())
                .unwrap_or_default(),
            linkedin_username: value.linkedin_username.clone(),
            instagram_username: value.instagram_username.clone(),
            career: value.career.clone(),
            industry: value.industry.clone(),
            profile: value.profile.clone(),
            role: match value
                .role
                .and_then(|role| members::MemberRole::try_from(role).ok())
            {
                Some(members::MemberRole::Board) => Some(MemberRole::Board),
                Some(members::MemberRole::Mentor) => Some(MemberRole::Mentor),
                Some(members::MemberRole::Mentee) => Some(MemberRole::Mentee),
                _ => None,
            },
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
#[path = "members_tests.rs"]
mod tests;
