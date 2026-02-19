use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
};
use shared::{
    responses::{ApiResponse, EmptyApiResponse},
    types::StaffGroup,
};
use uuid::Uuid;

use crate::{
    api::state::DataServiceAppState,
    domain::group::{CreateGroup, UpdateGroup},
    error::DataServiceError,
};

#[utoipa::path(
    get,
    path = "/api/v1/groups",
    tag = "Groups",
    operation_id = "list_groups",
    responses(
        (status = 200, description = "List all groups", body = ApiResponse<Vec<StaffGroup>>)
    )
)]
#[tracing::instrument(skip(state))]
pub async fn find_all(
    State(state): State<Arc<DataServiceAppState>>,
) -> Result<Json<ApiResponse<Vec<StaffGroup>>>, DataServiceError> {
    let output = state.group_repo.find_all().await?;

    Ok(Json(ApiResponse::ok(output)))
}

#[utoipa::path(
    get,
    path = "/api/v1/groups/{id}",
    tag = "Groups",
    operation_id = "get_group",
    params(
        ("id" = Uuid, Path, description = "Group ID")
    ),
    responses(
        (status = 200, description = "Group found", body = ApiResponse<StaffGroup>),
        (status = 404, description = "Group not found")
    )
)]
#[tracing::instrument(skip(state))]
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

#[utoipa::path(
    post,
    path = "/api/v1/groups",
    tag = "Groups",
    operation_id = "create_group",
    request_body = CreateGroup,
    responses(
        (status = 200, description = "Group created", body = ApiResponse<StaffGroup>)
    )
)]
#[tracing::instrument(skip(state))]
pub async fn create(
    State(state): State<Arc<DataServiceAppState>>,
    Json(group): Json<CreateGroup>,
) -> Result<Json<ApiResponse<StaffGroup>>, DataServiceError> {
    if group.name.trim().is_empty() {
        return Err(DataServiceError::BadRequest(
            "Name must not be empty".into(),
        ));
    }
    let output = state.group_repo.create(group).await?;

    Ok(Json(ApiResponse::ok(output)))
}

#[utoipa::path(
    post,
    path = "/api/v1/groups/batch",
    tag = "Groups",
    operation_id = "batch_create_groups",
    request_body = Vec<CreateGroup>,
    responses(
        (status = 200, description = "Groups batch created", body = ApiResponse<Vec<StaffGroup>>)
    )
)]
#[tracing::instrument(skip(state))]
pub async fn batch_create(
    State(state): State<Arc<DataServiceAppState>>,
    Json(groups): Json<Vec<CreateGroup>>,
) -> Result<Json<ApiResponse<Vec<StaffGroup>>>, DataServiceError> {
    let output = state.group_repo.batch_create(groups).await?;

    Ok(Json(ApiResponse::ok(output)))
}

#[utoipa::path(
    put,
    path = "/api/v1/groups/{id}",
    tag = "Groups",
    operation_id = "update_group",
    params(
        ("id" = Uuid, Path, description = "Group ID")
    ),
    request_body = UpdateGroup,
    responses(
        (status = 200, description = "Group updated", body = ApiResponse<StaffGroup>)
    )
)]
#[tracing::instrument(skip(state))]
pub async fn update(
    State(state): State<Arc<DataServiceAppState>>,
    Path(id): Path<Uuid>,
    Json(group): Json<UpdateGroup>,
) -> Result<Json<ApiResponse<StaffGroup>>, DataServiceError> {
    let output = state.group_repo.update(id, group).await?;

    Ok(Json(ApiResponse::ok(output)))
}

#[utoipa::path(
    delete,
    path = "/api/v1/groups/{id}",
    tag = "Groups",
    operation_id = "delete_group",
    params(
        ("id" = Uuid, Path, description = "Group ID")
    ),
    responses(
        (status = 200, description = "Group deleted", body = EmptyApiResponse)
    )
)]
#[tracing::instrument(skip(state))]
pub async fn delete(
    State(state): State<Arc<DataServiceAppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<()>>, DataServiceError> {
    state.group_repo.delete(id).await?;

    Ok(Json(ApiResponse::ok(())))
}
