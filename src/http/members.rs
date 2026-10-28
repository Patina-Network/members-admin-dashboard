use std::{
    collections::{BTreeMap, HashSet},
    future::Future,
    time::Duration,
};

use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use members_grpc_service::members::{
    self,
    admin::{CreateMemberRequest, CreateMemberResponse, SensitiveMember},
};
use serde::{Deserialize, Serialize};

use crate::{
    grpc::{self, error::UpstreamError},
    http::state::AppState,
};

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
        })
    }
}

fn optional(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
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
        }
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
