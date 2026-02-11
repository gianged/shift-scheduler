use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// region: Data Service Types

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StaffStatus {
    Active,
    Inactive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Staff {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub position: String,
    pub status: StaffStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaffGroups {
    pub id: Uuid,
    pub name: String,
    pub parent_group_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMemberships {
    pub staff_id: Uuid,
    pub group_id: Uuid,
}

// endregion: Data Service Types

// region: Scheduling Service Types

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JobStatus {
    Pending,
    Processing,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ShiftType {
    Morning,
    Evening,
    DayOff,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleJobs {
    pub id: Uuid,
    pub group_id: Uuid,
    pub run_at: NaiveDate,
    pub status: JobStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShiftAssignments {
    pub id: Uuid,
    pub job_id: Uuid,
    pub staff_id: Uuid,
    pub date: NaiveDate,
    pub shift_type: ShiftType,
}

// endregion: Scheduling Service Types
