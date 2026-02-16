use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
};
use shared::{
    responses::{ApiResponse, EmptyApiResponse},
    types::{Staff, StaffGroup},
};
use uuid::Uuid;

use crate::{
    api::state::DataServiceAppState, domain::membership::AddMembership, error::DataServiceError,
};

#[utoipa::path(
    post,
    path = "/api/v1/groups/{group_id}/members",
    tag = "Membership",
    operation_id = "add_member",
    params(
        ("group_id" = Uuid, Path, description = "Group ID")
    ),
    request_body = AddMembership,
    responses(
        (status = 200, description = "Member added", body = EmptyApiResponse)
    )
)]
#[tracing::instrument(skip(state))]
pub async fn add_member(
    State(state): State<Arc<DataServiceAppState>>,
    Path(group_id): Path<Uuid>,
    Json(body): Json<AddMembership>,
) -> Result<Json<ApiResponse<()>>, DataServiceError> {
    if body.group_id != group_id {
        return Err(DataServiceError::BadRequest(
            "Body group_id does not match path group_id".to_string(),
        ));
    }

    state
        .membership_repo
        .add_staff_to_group(group_id, body.staff_id)
        .await?;

    Ok(Json(ApiResponse::ok(())))
}

#[utoipa::path(
    delete,
    path = "/api/v1/groups/{group_id}/members/{staff_id}",
    tag = "Membership",
    operation_id = "remove_member",
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
    operation_id = "get_group_members",
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
    operation_id = "get_staff_groups",
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
    operation_id = "resolve_members",
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

#[utoipa::path(
    post,
    path = "/api/v1/memberships/batch",
    tag = "Membership",
    operation_id = "batch_add_members",
    request_body = Vec<AddMembership>,
    responses(
        (status = 200, description = "Memberships batch added", body = EmptyApiResponse)
    )
)]
#[tracing::instrument(skip(state))]
pub async fn batch_add_members(
    State(state): State<Arc<DataServiceAppState>>,
    Json(memberships): Json<Vec<AddMembership>>,
) -> Result<Json<ApiResponse<()>>, DataServiceError> {
    state.membership_repo.batch_add_members(memberships).await?;

    Ok(Json(ApiResponse::ok(())))
}
