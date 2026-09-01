//! Communication transport contracts and closed workflow values.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::{Validate, ValidationError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnnouncementPriority {
    Normal,
    Important,
    Urgent,
}
impl AnnouncementPriority {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Important => "important",
            Self::Urgent => "urgent",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnnouncementStatus {
    Draft,
    Submitted,
    Published,
    Cancelled,
}
impl AnnouncementStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Submitted => "submitted",
            Self::Published => "published",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudienceKind {
    Campus,
    Role,
    ClassGroup,
    Department,
    Individual,
}
impl AudienceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Campus => "campus",
            Self::Role => "role",
            Self::ClassGroup => "class_group",
            Self::Department => "department",
            Self::Individual => "individual",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[validate(schema(function = "validate_target"))]
pub struct AudienceTargetInput {
    pub kind: AudienceKind,
    pub target_id: Option<Uuid>,
    #[validate(length(min = 1, max = 80))]
    pub target_key: Option<String>,
    #[validate(length(min = 1, max = 200))]
    pub label: String,
}

fn validate_target(value: &AudienceTargetInput) -> Result<(), ValidationError> {
    let valid = match value.kind {
        AudienceKind::Campus => value.target_id.is_none() && value.target_key.is_none(),
        AudienceKind::Role => {
            value.target_id.is_none()
                && value
                    .target_key
                    .as_ref()
                    .is_some_and(|key| !key.trim().is_empty())
        }
        AudienceKind::ClassGroup | AudienceKind::Department | AudienceKind::Individual => {
            value.target_id.is_some() && value.target_key.is_none()
        }
    };
    if valid {
        Ok(())
    } else {
        Err(ValidationError::new("audience_target_shape"))
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateAnnouncementRequest {
    #[validate(length(min = 1, max = 180))]
    pub title: String,
    #[validate(length(min = 1, max = 10_000))]
    pub body: String,
    pub priority: AnnouncementPriority,
    #[validate(length(min = 1, max = 100), nested)]
    pub targets: Vec<AudienceTargetInput>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateAnnouncementRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(min = 1, max = 180))]
    pub title: String,
    #[validate(length(min = 1, max = 10_000))]
    pub body: String,
    pub priority: AnnouncementPriority,
    #[validate(length(min = 1, max = 100), nested)]
    pub targets: Vec<AudienceTargetInput>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct VersionRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ReasonedVersionRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(min = 1, max = 1000))]
    pub reason: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct DeleteAnnouncementQuery {
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize)]
pub struct AnnouncementListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub status: Option<AnnouncementStatus>,
    pub search: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct InboxListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub unread_only: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AudienceTarget {
    pub id: Uuid,
    pub kind: String,
    pub target_id: Option<Uuid>,
    pub target_key: Option<String>,
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnnouncementSummary {
    pub id: Uuid,
    pub title: String,
    pub priority: String,
    pub status: String,
    pub version: i32,
    pub creator_name: String,
    pub recipient_count: i64,
    pub read_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnnouncementDetail {
    #[serde(flatten)]
    pub summary: AnnouncementSummary,
    pub body: String,
    pub created_by: Uuid,
    pub targets: Vec<AudienceTarget>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub cancellation_reason: Option<String>,
    pub reopened_at: Option<DateTime<Utc>>,
    pub reopen_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct RoleReference {
    pub key: String,
    pub name: String,
}
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct UserReference {
    pub id: Uuid,
    pub full_name: String,
    pub email: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommunicationReferenceData {
    pub classes: Vec<cp_academics::dtos::CommunicationClassReference>,
    pub departments: Vec<cp_hr_payroll::models::CommunicationDepartmentReference>,
    pub roles: Vec<RoleReference>,
    pub users: Vec<UserReference>,
    pub campus_allowed: bool,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct DeliveryRecord {
    pub id: Uuid,
    pub announcement_id: Uuid,
    pub recipient_user_id: Uuid,
    pub recipient_name: String,
    pub channel: String,
    pub status: String,
    pub delivered_at: Option<DateTime<Utc>>,
    pub read_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct InboxItem {
    pub delivery_id: Uuid,
    pub announcement_id: Uuid,
    pub title: String,
    pub body: String,
    pub priority: String,
    pub sender_name: String,
    pub published_at: DateTime<Utc>,
    pub read_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct AnnouncementsPage {
    pub announcements: Vec<AnnouncementSummary>,
}
#[derive(Debug, Serialize)]
pub struct InboxPage {
    pub messages: Vec<InboxItem>,
}
#[derive(Debug, Serialize)]
pub struct AudiencePreview {
    pub recipient_count: i64,
    pub recipients: Vec<UserReference>,
}
