use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
};
use shared::{
    responses::{ApiResponse, EmptyApiResponse},
    types::Staff,
};
use uuid::Uuid;

use crate::{
    api::state::DataServiceAppState,
    domain::staff::{CreateStaff, UpdateStaff},
    error::DataServiceError,
};

#[utoipa::path(
    get,
    path = "/api/v1/staff",
    tag = "Staff",
    operation_id = "list_staff",
    responses(
        (status = 200, description = "List all staff", body = ApiResponse<Vec<Staff>>)
    )
)]
#[tracing::instrument(skip(state))]
pub async fn find_all(
    State(state): State<Arc<DataServiceAppState>>,
) -> Result<Json<ApiResponse<Vec<Staff>>>, DataServiceError> {
    let output = state.staff_repo.find_all().await?;
    Ok(Json(ApiResponse::ok(output)))
}

#[utoipa::path(
    get,
    path = "/api/v1/staff/{id}",
    tag = "Staff",
    operation_id = "get_staff",
    params(
        ("id" = Uuid, Path, description = "Staff ID")
    ),
    responses(
        (status = 200, description = "Staff found", body = ApiResponse<Staff>),
        (status = 404, description = "Staff not found")
    )
)]
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

#[utoipa::path(
    post,
    path = "/api/v1/staff",
    tag = "Staff",
    operation_id = "create_staff",
    request_body = CreateStaff,
    responses(
        (status = 200, description = "Staff created", body = ApiResponse<Staff>)
    )
)]
#[tracing::instrument(skip(state))]
pub async fn create(
    State(state): State<Arc<DataServiceAppState>>,
    Json(staff): Json<CreateStaff>,
) -> Result<Json<ApiResponse<Staff>>, DataServiceError> {
    let output = state.staff_repo.create(staff).await?;

    Ok(Json(ApiResponse::ok(output)))
}

#[utoipa::path(
    post,
    path = "/api/v1/staff/batch",
    tag = "Staff",
    operation_id = "batch_create_staff",
    request_body = Vec<CreateStaff>,
    responses(
        (status = 200, description = "Staff batch created", body = ApiResponse<Vec<Staff>>)
    )
)]
#[tracing::instrument(skip(state))]
pub async fn batch_create(
    State(state): State<Arc<DataServiceAppState>>,
    Json(staffs): Json<Vec<CreateStaff>>,
) -> Result<Json<ApiResponse<Vec<Staff>>>, DataServiceError> {
    let output = state.staff_repo.batch_create(staffs).await?;

    Ok(Json(ApiResponse::ok(output)))
}

#[utoipa::path(
    put,
    path = "/api/v1/staff/{id}",
    tag = "Staff",
    operation_id = "update_staff",
    params(
        ("id" = Uuid, Path, description = "Staff ID")
    ),
    request_body = UpdateStaff,
    responses(
        (status = 200, description = "Staff updated", body = ApiResponse<Staff>)
    )
)]
#[tracing::instrument(skip(state))]
pub async fn update(
    State(state): State<Arc<DataServiceAppState>>,
    Path(id): Path<Uuid>,
    Json(staff): Json<UpdateStaff>,
) -> Result<Json<ApiResponse<Staff>>, DataServiceError> {
    let output = state.staff_repo.update(id, staff).await?;

    Ok(Json(ApiResponse::ok(output)))
}

#[utoipa::path(
    patch,
    path = "/api/v1/staff/{id}/deactivate",
    tag = "Staff",
    operation_id = "deactivate_staff",
    params(
        ("id" = Uuid, Path, description = "Staff ID")
    ),
    responses(
        (status = 200, description = "Staff deactivated", body = EmptyApiResponse)
    )
)]
#[tracing::instrument(skip(state))]
pub async fn deactivate(
    State(state): State<Arc<DataServiceAppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<()>>, DataServiceError> {
    state.staff_repo.deactivate(id).await?;

    Ok(Json(ApiResponse::ok(())))
}

#[utoipa::path(
    delete,
    path = "/api/v1/staff/{id}",
    tag = "Staff",
    operation_id = "delete_staff",
    params(
        ("id" = Uuid, Path, description = "Staff ID")
    ),
    responses(
        (status = 200, description = "Staff deleted", body = EmptyApiResponse)
    )
)]
#[tracing::instrument(skip(state))]
pub async fn delete(
    State(state): State<Arc<DataServiceAppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<()>>, DataServiceError> {
    state.staff_repo.delete(id).await?;

    Ok(Json(ApiResponse::ok(())))
}
