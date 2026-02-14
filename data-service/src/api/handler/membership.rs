use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
};
use serde::Deserialize;
use shared::{
    responses::ApiResponse,
    types::{Staff, StaffGroup},
};
use uuid::Uuid;

use crate::{api::state::DataServiceAppState, error::DataServiceError};

#[derive(Deserialize)]
pub struct AddMemberRequest {
    pub staff_id: Uuid,
}

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

pub async fn get_group_members(
    State(state): State<Arc<DataServiceAppState>>,
    Path(group_id): Path<Uuid>,
) -> Result<Json<ApiResponse<Vec<Staff>>>, DataServiceError> {
    let output = state.membership_repo.get_group_members(group_id).await?;

    Ok(Json(ApiResponse::ok(output)))
}

pub async fn get_staff_groups(
    State(state): State<Arc<DataServiceAppState>>,
    Path(staff_id): Path<Uuid>,
) -> Result<Json<ApiResponse<Vec<StaffGroup>>>, DataServiceError> {
    let output = state.membership_repo.get_staff_groups(staff_id).await?;

    Ok(Json(ApiResponse::ok(output)))
}

pub async fn resolve_members(
    State(state): State<Arc<DataServiceAppState>>,
    Path(group_id): Path<Uuid>,
) -> Result<Json<ApiResponse<Vec<Staff>>>, DataServiceError> {
    let output = state.membership_repo.resolve_members(group_id).await?;

    Ok(Json(ApiResponse::ok(output)))
}
