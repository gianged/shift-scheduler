use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
};
use serde::Deserialize;
use shared::{
    responses::{ApiResponse, EmptyApiResponse},
    types::{Staff, StaffGroup},
};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{api::state::DataServiceAppState, error::DataServiceError};

#[derive(Debug, Deserialize, ToSchema)]
pub struct AddMemberRequest {
    pub staff_id: Uuid,
}

#[utoipa::path(
    post,
    path = "/api/v1/groups/{group_id}/members",
    tag = "Membership",
    params(
        ("group_id" = Uuid, Path, description = "Group ID")
    ),
    request_body = AddMemberRequest,
    responses(
        (status = 200, description = "Member added", body = EmptyApiResponse)
    )
)]
#[tracing::instrument(skip(state))]
pub async fn add_member(
    State(state): State<Arc<DataServiceAppState>>,
    Path(group_id): Path<Uuid>,
    Json(staff): Json<AddMemberRequest>,
) -> Result<Json<ApiResponse<()>>, DataServiceError> {
    state
        .membership_repo
        .add_staff_to_group(group_id, staff.staff_id)
        .await?;

    Ok(Json(ApiResponse::ok(())))
}

#[utoipa::path(
    delete,
    path = "/api/v1/groups/{group_id}/members/{staff_id}",
    tag = "Membership",
    params(
        ("group_id" = Uuid, Path, description = "Group ID"),
        ("staff_id" = Uuid, Path, description = "Staff ID")
    ),
    responses(
        (status = 200, description = "Member removed", body = EmptyApiResponse)
    )
)]
#[tracing::instrument(skip(state))]
pub async fn remove_member(
    State(state): State<Arc<DataServiceAppState>>,
    Path((group_id, staff_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiResponse<()>>, DataServiceError> {
    state
        .membership_repo
        .remove_staff_from_group(group_id, staff_id)
        .await?;

    Ok(Json(ApiResponse::ok(())))
}

#[utoipa::path(
    get,
    path = "/api/v1/groups/{group_id}/members",
    tag = "Membership",
    params(
        ("group_id" = Uuid, Path, description = "Group ID")
    ),
    responses(
        (status = 200, description = "List group members", body = ApiResponse<Vec<Staff>>)
    )
)]
#[tracing::instrument(skip(state))]
pub async fn get_group_members(
    State(state): State<Arc<DataServiceAppState>>,
    Path(group_id): Path<Uuid>,
) -> Result<Json<ApiResponse<Vec<Staff>>>, DataServiceError> {
    let output = state.membership_repo.get_group_members(group_id).await?;

    Ok(Json(ApiResponse::ok(output)))
}

#[utoipa::path(
    get,
    path = "/api/v1/staff/{id}/groups",
    tag = "Membership",
    params(
        ("id" = Uuid, Path, description = "Staff ID")
    ),
    responses(
        (status = 200, description = "List staff's groups", body = ApiResponse<Vec<StaffGroup>>)
    )
)]
#[tracing::instrument(skip(state))]
pub async fn get_staff_groups(
    State(state): State<Arc<DataServiceAppState>>,
    Path(staff_id): Path<Uuid>,
) -> Result<Json<ApiResponse<Vec<StaffGroup>>>, DataServiceError> {
    let output = state.membership_repo.get_staff_groups(staff_id).await?;

    Ok(Json(ApiResponse::ok(output)))
}

#[utoipa::path(
    get,
    path = "/api/v1/groups/{group_id}/resolved-members",
    tag = "Membership",
    params(
        ("group_id" = Uuid, Path, description = "Group ID")
    ),
    responses(
        (status = 200, description = "List resolved group members (including sub-groups)", body = ApiResponse<Vec<Staff>>)
    )
)]
#[tracing::instrument(skip(state))]
pub async fn resolve_members(
    State(state): State<Arc<DataServiceAppState>>,
    Path(group_id): Path<Uuid>,
) -> Result<Json<ApiResponse<Vec<Staff>>>, DataServiceError> {
    let output = state.membership_repo.resolve_members(group_id).await?;

    Ok(Json(ApiResponse::ok(output)))
}
