use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::prelude::{FromRow, Type};
use utoipa::ToSchema;
use uuid::Uuid;

// region: Data Service Types

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Type, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(type_name = "staff_status", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StaffStatus {
    Active,
    Inactive,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct Staff {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub position: String,
    pub status: StaffStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct StaffGroup {
    pub id: Uuid,
    pub name: String,
    pub parent_group_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct GroupMembership {
    pub staff_id: Uuid,
    pub group_id: Uuid,
}

// endregion: Data Service Types

// region: Scheduling Service Types

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Type, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(type_name = "job_status", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JobStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    WaitingForRetry,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Type, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(type_name = "shift_type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ShiftType {
    Morning,
    Evening,
    DayOff,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct ScheduleJob {
    pub id: Uuid,
    pub staff_group_id: Uuid,
    pub period_begin_date: NaiveDate,
    pub status: JobStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct ShiftAssignment {
    pub id: Uuid,
    pub job_id: Uuid,
    pub staff_id: Uuid,
    pub date: NaiveDate,
    pub shift_type: ShiftType,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ScheduleResult {
    pub schedule_id: Uuid,
    pub period_begin_date: NaiveDate,
    pub staff_group_id: Uuid,
    pub assignments: Vec<ShiftAssignment>,
}

// endregion: Scheduling Service Types
