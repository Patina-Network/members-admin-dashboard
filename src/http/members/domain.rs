use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use members_grpc_service::members::{
    self,
    admin::{
        CreateMemberRequest, MemberCreated, MemberOperation, MemberOperationFailed,
        MemberUnchanged, MemberUpdated, SensitiveMember, UpsertMemberRequest,
        member_operation::Operation, member_operation_result::Outcome,
    },
};
use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    grpc::error::UpstreamError,
    util::{trim_string, trim_string_or_clear},
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

impl TryFrom<i32> for MemberRole {
    type Error = ();

    fn try_from(role: i32) -> Result<Self, Self::Error> {
        match members::MemberRole::try_from(role) {
            Ok(members::MemberRole::Board) => Ok(MemberRole::Board),
            Ok(members::MemberRole::Mentor) => Ok(MemberRole::Mentor),
            Ok(members::MemberRole::Mentee) => Ok(MemberRole::Mentee),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CommunicationMethod {
    Email,
    PhoneNumber,
    Discord,
    Instagram,
    LinkedIn,
}

impl From<CommunicationMethod> for i32 {
    fn from(method: CommunicationMethod) -> Self {
        let method = match method {
            CommunicationMethod::Email => members::CommunicationMethod::Email,
            CommunicationMethod::PhoneNumber => members::CommunicationMethod::PhoneNumber,
            CommunicationMethod::Discord => members::CommunicationMethod::Discord,
            CommunicationMethod::Instagram => members::CommunicationMethod::Instagram,
            CommunicationMethod::LinkedIn => members::CommunicationMethod::LinkedIn,
        };

        method as i32
    }
}

impl TryFrom<i32> for CommunicationMethod {
    type Error = ();

    fn try_from(method: i32) -> Result<Self, Self::Error> {
        match members::CommunicationMethod::try_from(method) {
            Ok(members::CommunicationMethod::Email) => Ok(Self::Email),
            Ok(members::CommunicationMethod::PhoneNumber) => Ok(Self::PhoneNumber),
            Ok(members::CommunicationMethod::Discord) => Ok(Self::Discord),
            Ok(members::CommunicationMethod::Instagram) => Ok(Self::Instagram),
            Ok(members::CommunicationMethod::LinkedIn) => Ok(Self::LinkedIn),
            _ => Err(()),
        }
    }
}

fn stored_preferences(
    value: Option<members::PreferredCommunications>,
) -> Option<Vec<CommunicationMethod>> {
    let methods: Vec<_> = value?
        .values
        .into_iter()
        .filter_map(|method| method.try_into().ok())
        .collect();

    (!methods.is_empty()).then_some(methods)
}

fn preferences_request(methods: Vec<CommunicationMethod>) -> members::PreferredCommunications {
    members::PreferredCommunications {
        values: methods.into_iter().map(Into::into).collect(),
    }
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct Member {
    pub member_id: i32,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub joined_at: Option<String>,
    pub discord_id: Option<String>,
    pub discord_username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linkedin_username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instagram_username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub career: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub industry: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub school: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grad_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_communications: Option<Vec<CommunicationMethod>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<MemberRole>,
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
            school: value.school,
            grad_date: value.grad_date,
            preferred_communications: stored_preferences(value.preferred_communications),
            role: value.role.and_then(|role| role.try_into().ok()),
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
            school: value.school,
            grad_date: value.grad_date,
            preferred_communications: stored_preferences(value.preferred_communications),
            role: value.role.and_then(|role| role.try_into().ok()),
        }
    }
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct MemberDetails {
    pub updated_at: String,
    #[serde(flatten)]
    pub member: Member,
    pub profile: Option<String>,
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

#[derive(Serialize, utoipa::ToSchema)]
pub struct MemberList {
    pub members: Vec<Member>,
    pub total: usize,
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

#[derive(Deserialize, utoipa::ToSchema)]
pub struct NewMember {
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub joined_at: Option<String>,
    pub discord_id: Option<String>,
    pub discord_username: Option<String>,
    pub linkedin_username: Option<String>,
    pub instagram_username: Option<String>,
    pub career: Option<String>,
    pub industry: Option<String>,
    pub school: Option<String>,
    pub grad_date: Option<String>,
    pub preferred_communications: Option<Vec<CommunicationMethod>>,
    pub profile: Option<String>,
    pub role: Option<MemberRole>,
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

        let joined_at = trim_string(input.joined_at)
            .map(|value| value.parse().map_err(|_| UpstreamError::InvalidArgument))
            .transpose()?;

        Ok(Self {
            email,
            first_name,
            last_name,
            joined_at,
            discord_id: trim_string(input.discord_id),
            discord_username: trim_string(input.discord_username),
            linkedin_username: trim_string(input.linkedin_username),
            instagram_username: trim_string(input.instagram_username),
            career: trim_string(input.career),
            industry: trim_string(input.industry),
            school: trim_string(input.school),
            grad_date: trim_string(input.grad_date),
            preferred_communications: input
                .preferred_communications
                .filter(|methods| !methods.is_empty())
                .map(preferences_request),
            profile: trim_string(input.profile),
            role: input.role.map(Into::into),
        })
    }
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

#[derive(Clone, Copy, Default, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BulkMode {
    /// Fail the row.
    #[default]
    Create,
    /// Update the existing member. Blank optional fields keep their stored values.
    Upsert,
}

impl BulkMode {
    pub fn operation(self, member: CreateMemberRequest) -> MemberOperation {
        let operation = match self {
            Self::Create => Operation::Create(member),
            Self::Upsert => Operation::Upsert(UpsertMemberRequest {
                member: Some(member),
            }),
        };

        MemberOperation {
            operation: Some(operation),
        }
    }
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct BulkMembers {
    pub members: Vec<NewMember>,
    #[serde(default)]
    pub mode: BulkMode,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum BulkOutcome {
    Created { member: Member },
    Updated { member: Member },
    Unchanged { member: Member },
    Failed { error: String },
}

/// A missing outcome, or one without a member, means the write can't be confirmed.
impl From<Option<Outcome>> for BulkOutcome {
    fn from(outcome: Option<Outcome>) -> Self {
        match outcome {
            Some(Outcome::Created(MemberCreated {
                member: Some(member),
            })) => Self::Created {
                member: member.into(),
            },
            Some(Outcome::Updated(MemberUpdated {
                after: Some(member),
                ..
            })) => Self::Updated {
                member: member.into(),
            },
            Some(Outcome::Unchanged(MemberUnchanged {
                member: Some(member),
            })) => Self::Unchanged {
                member: member.into(),
            },
            Some(Outcome::Failed(MemberOperationFailed { error })) => Self::Failed { error },
            _ => Self::Failed {
                error: "Could not confirm result. Refresh before retrying.".into(),
            },
        }
    }
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct BulkResult {
    pub index: usize,
    #[serde(flatten)]
    pub outcome: BulkOutcome,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct BulkResults {
    pub results: Vec<BulkResult>,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MemberUpdate {
    pub expected_updated_at: String,
    pub email: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_update")]
    pub discord_id: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_update")]
    pub discord_username: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_update")]
    pub linkedin_username: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_update")]
    pub instagram_username: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_update")]
    pub career: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_update")]
    pub industry: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_update")]
    pub school: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_update")]
    pub grad_date: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_update")]
    pub preferred_communications: Option<Option<Vec<CommunicationMethod>>>,
    #[serde(default, deserialize_with = "deserialize_update")]
    pub profile: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_update")]
    pub role: Option<Option<MemberRole>>,
}

pub fn deserialize_update<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

impl MemberUpdate {
    pub fn into_request(
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
            discord_id: trim_string_or_clear(self.discord_id),
            discord_username: trim_string_or_clear(self.discord_username),
            linkedin_username: trim_string_or_clear(self.linkedin_username),
            instagram_username: trim_string_or_clear(self.instagram_username),
            career: trim_string_or_clear(self.career),
            industry: trim_string_or_clear(self.industry),
            school: trim_string_or_clear(self.school),
            grad_date: trim_string_or_clear(self.grad_date),
            preferred_communications: self
                .preferred_communications
                .map(|methods| preferences_request(methods.unwrap_or_default())),
            profile: trim_string_or_clear(self.profile),
            role: self.role.map(|role| {
                role.map(Into::into)
                    .unwrap_or(members::MemberRole::Unspecified as i32)
            }),
        };

        validate_update_fields(&request)?;
        Ok(request)
    }
}

pub fn validate_update_fields(
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

#[cfg(test)]
mod tests {
    use members_grpc_service::members::{
        self,
        admin::{CreateMemberRequest, SensitiveMember},
    };

    use crate::{
        grpc::error::UpstreamError,
        http::members::domain::{Member, NewMember},
    };

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
            school: None,
            grad_date: None,
            preferred_communications: None,
            profile: None,
            role: None,
        }
    }

    #[test]
    fn update_forwards_all_form_fields() {
        let input: super::MemberUpdate = serde_json::from_value(serde_json::json!({
        "expected_updated_at": "2020-01-01T00:00:00Z", "email": "person@example.com", "first_name": "First", "last_name": "Last",
        "linkedin_username": " alex ",
        "instagram_username": " alex.photo ", "career": " Engineer ",
        "industry": " Technology ", "school": " Example University ", "grad_date": " Spring 2028 ",
        "profile": " Private notes ", "role": "board"
    }))
    .unwrap();

        let request = input.into_request(42).unwrap();

        assert!(request.joined_at.is_none());
        assert_eq!(request.linkedin_username.as_deref(), Some("alex"));
        assert_eq!(request.instagram_username.as_deref(), Some("alex.photo"));
        assert_eq!(request.career.as_deref(), Some("Engineer"));
        assert_eq!(request.industry.as_deref(), Some("Technology"));
        assert_eq!(request.school.as_deref(), Some("Example University"));
        assert_eq!(request.grad_date.as_deref(), Some("Spring 2028"));
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
            school: Some(None),
            grad_date: None,
            preferred_communications: None,
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
        assert_eq!(request.school.as_deref(), Some(""));
        assert!(request.grad_date.is_none());
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
                school: None,
                grad_date: None,
                preferred_communications: None,
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
            school: None,
            grad_date: None,
            preferred_communications: None,
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
                school: None,
                grad_date: None,
                preferred_communications: None,
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
            school: Some(" Example University ".into()),
            grad_date: Some(" Spring 2028 ".into()),
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
        assert_eq!(request.school.as_deref(), Some("Example University"));
        assert_eq!(request.grad_date.as_deref(), Some("Spring 2028"));
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
            school: Some(" ".into()),
            grad_date: Some(" ".into()),
            profile: Some("\n".into()),
            ..new_member()
        };
        let request = CreateMemberRequest::try_from(input).unwrap();

        assert!(request.linkedin_username.is_none());
        assert!(request.instagram_username.is_none());
        assert!(request.career.is_none());
        assert!(request.industry.is_none());
        assert!(request.school.is_none());
        assert!(request.grad_date.is_none());
        assert!(request.profile.is_none());
    }

    #[test]
    fn preferred_communications_round_trip_between_json_and_upstream() {
        use super::CommunicationMethod as Method;

        let all = serde_json::json!(["email", "phone_number", "discord", "instagram", "linked_in"]);
        let methods: Vec<Method> = serde_json::from_value(all.clone()).unwrap();

        let request = CreateMemberRequest::try_from(NewMember {
            preferred_communications: Some(methods),
            ..new_member()
        })
        .unwrap();
        let sent = request.preferred_communications.clone().unwrap();
        assert_eq!(sent.values, [1, 2, 3, 4, 5]);

        let stored = SensitiveMember {
            preferred_communications: Some(members::PreferredCommunications {
                values: vec![1, 2, 3, 4, 5, 0, 99],
            }),
            ..Default::default()
        };
        let public = serde_json::to_value(Member::from(stored)).unwrap();
        assert_eq!(public["preferred_communications"], all);

        for empty in [None, Some(members::PreferredCommunications::default())] {
            let stored = SensitiveMember {
                preferred_communications: empty,
                ..Default::default()
            };
            let public = serde_json::to_value(Member::from(stored)).unwrap();
            assert!(public.get("preferred_communications").is_none());
        }

        let blank = CreateMemberRequest::try_from(NewMember {
            preferred_communications: Some(vec![]),
            ..new_member()
        })
        .unwrap();
        assert!(blank.preferred_communications.is_none());
    }

    #[test]
    fn update_keeps_clears_or_replaces_preferred_communications() {
        for (body, expected) in [
            (serde_json::json!({}), None),
            (
                serde_json::json!({ "preferred_communications": null }),
                Some(vec![]),
            ),
            (
                serde_json::json!({ "preferred_communications": [] }),
                Some(vec![]),
            ),
            (
                serde_json::json!({ "preferred_communications": ["linked_in", "email"] }),
                Some(vec![5, 1]),
            ),
        ] {
            let mut input = body;
            input["expected_updated_at"] = "2020-01-01T00:00:00Z".into();
            let input: super::MemberUpdate = serde_json::from_value(input).unwrap();

            let request = input.into_request(42).unwrap();

            assert_eq!(
                request.preferred_communications.map(|sent| sent.values),
                expected
            );
        }

        assert!(
            serde_json::from_value::<super::MemberUpdate>(serde_json::json!({
                "expected_updated_at": "2020-01-01T00:00:00Z",
                "preferred_communications": ["fax"]
            }))
            .is_err()
        );
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
                school: Some("Example University".into()),
                grad_date: Some("Spring 2028".into()),
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
                "school",
                "grad_date",
                "role",
            ] {
                assert_eq!(public.get(field), details.get(field), "{field}");
            }

            assert_eq!(public["linkedin_username"], "alex");
            assert_eq!(public["instagram_username"], "alex.photo");
            assert_eq!(public["career"], "Engineer");
            assert_eq!(public["industry"], "Technology");
            assert_eq!(public["school"], "Example University");
            assert_eq!(public["grad_date"], "Spring 2028");
            assert_eq!(
                public.get("role").and_then(|value| value.as_str()),
                expected
            );
            assert!(public.get("profile").is_none());
            assert_eq!(details["profile"], "Private notes");
        }
    }
}
