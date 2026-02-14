use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
};
use shared::{responses::ApiResponse, types::Staff};
use uuid::Uuid;

use crate::{
    api::state::DataServiceAppState,
    domain::staff::{CreateStaff, UpdateStaff},
    error::DataServiceError,
};

#[tracing::instrument(skip(state))]
pub async fn find_all(
    State(state): State<Arc<DataServiceAppState>>,
) -> Result<Json<ApiResponse<Vec<Staff>>>, DataServiceError> {
    let output = state.staff_repo.find_all().await?;
    Ok(Json(ApiResponse::ok(output)))
}

#[tracing::instrument(skip(state))]
pub async fn find_by_id(
    State(state): State<Arc<DataServiceAppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<Staff>>, DataServiceError> {
    let output = state.staff_repo.find_by_id(id).await?;

    match output {
        Some(s) => Ok(Json(ApiResponse::ok(s))),
        None => Err(DataServiceError::NotFound("Staff not found".to_string())),
    }
}

#[tracing::instrument(skip(state))]
pub async fn create(
    State(state): State<Arc<DataServiceAppState>>,
    Json(staff): Json<CreateStaff>,
) -> Result<Json<ApiResponse<Staff>>, DataServiceError> {
    let output = state.staff_repo.create(staff).await?;

    Ok(Json(ApiResponse::ok(output)))
}

#[tracing::instrument(skip(state))]
pub async fn batch_create(
    State(state): State<Arc<DataServiceAppState>>,
    Json(staffs): Json<Vec<CreateStaff>>,
) -> Result<Json<ApiResponse<Vec<Staff>>>, DataServiceError> {
    let output = state.staff_repo.batch_create(staffs).await?;

    Ok(Json(ApiResponse::ok(output)))
}

#[tracing::instrument(skip(state))]
pub async fn update(
    State(state): State<Arc<DataServiceAppState>>,
    Path(id): Path<Uuid>,
    Json(staff): Json<UpdateStaff>,
) -> Result<Json<ApiResponse<Staff>>, DataServiceError> {
    let output = state.staff_repo.update(id, staff).await?;

    Ok(Json(ApiResponse::ok(output)))
}

#[tracing::instrument(skip(state))]
pub async fn deactivate(
    State(state): State<Arc<DataServiceAppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<()>>, DataServiceError> {
    state.staff_repo.deactivate(id).await?;

    Ok(Json(ApiResponse::ok(())))
}

#[tracing::instrument(skip(state))]
pub async fn delete(
    State(state): State<Arc<DataServiceAppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<()>>, DataServiceError> {
    state.staff_repo.delete(id).await?;

    Ok(Json(ApiResponse::ok(())))
}
