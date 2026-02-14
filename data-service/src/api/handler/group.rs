use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
};
use shared::{responses::ApiResponse, types::StaffGroup};
use uuid::Uuid;

use crate::{
    api::state::DataServiceAppState,
    domain::group::{CreateGroup, UpdateGroup},
    error::DataServiceError,
};

pub async fn find_all(
    State(state): State<Arc<DataServiceAppState>>,
) -> Result<Json<ApiResponse<Vec<StaffGroup>>>, DataServiceError> {
    let output = state.group_repo.find_all().await?;

    Ok(Json(ApiResponse::ok(output)))
}

pub async fn find_by_id(
    State(state): State<Arc<DataServiceAppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<StaffGroup>>, DataServiceError> {
    let output = state.group_repo.find_by_id(id).await?;

    match output {
        Some(g) => Ok(Json(ApiResponse::ok(g))),
        None => Err(DataServiceError::NotFound("Group not found".to_string())),
    }
}

pub async fn create(
    State(state): State<Arc<DataServiceAppState>>,
    Json(group): Json<CreateGroup>,
) -> Result<Json<ApiResponse<StaffGroup>>, DataServiceError> {
    let output = state.group_repo.create(group).await?;

    Ok(Json(ApiResponse::ok(output)))
}

pub async fn batch_create(
    State(state): State<Arc<DataServiceAppState>>,
    Json(groups): Json<Vec<CreateGroup>>,
) -> Result<Json<ApiResponse<Vec<StaffGroup>>>, DataServiceError> {
    let output = state.group_repo.batch_create(groups).await?;

    Ok(Json(ApiResponse::ok(output)))
}

pub async fn update(
    State(state): State<Arc<DataServiceAppState>>,
    Path(id): Path<Uuid>,
    Json(group): Json<UpdateGroup>,
) -> Result<Json<ApiResponse<StaffGroup>>, DataServiceError> {
    let output = state.group_repo.update(id, group).await?;

    Ok(Json(ApiResponse::ok(output)))
}

pub async fn delete(
    State(state): State<Arc<DataServiceAppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<()>>, DataServiceError> {
    state.group_repo.delete(id).await?;

    Ok(Json(ApiResponse::ok(())))
}
