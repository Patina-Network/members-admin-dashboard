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

#[cfg(test)]
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
mod tests {
    use std::time::Duration;

    use members_grpc_service::members::{self, admin::SensitiveMember};

    use super::{CreateMemberRequest, Member, NewMember, UpstreamError, collect_members};

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
            let result = super::update_with(
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
}

#[cfg(test)]
#[path = "members_handler_tests.rs"]
mod handler_tests;
