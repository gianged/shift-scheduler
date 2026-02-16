use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::NaiveDate;
use serde::Deserialize;
use shared::responses::ApiResponse;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{api::state::SchedulingAppState, error::SchedulingServiceError};

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateScheduleRequest {
    pub staff_group_id: Uuid,
    pub period_begin_date: NaiveDate,
}

#[utoipa::path(
    post,
    path = "/api/v1/schedules",
    tag = "Schedules",
    request_body = CreateScheduleRequest,
    responses(
        (status = 202, description = "Schedule job submitted", body = ApiResponse<shared::types::ScheduleJob>)
    )
)]
#[tracing::instrument(skip(state))]
pub async fn submit_schedule(
    State(state): State<Arc<SchedulingAppState>>,
    Json(req): Json<CreateScheduleRequest>,
) -> Result<impl IntoResponse, SchedulingServiceError> {
    let job = state
        .scheduling_service
        .submit_schedule(req.staff_group_id, req.period_begin_date)
        .await?;

    Ok((StatusCode::ACCEPTED, Json(ApiResponse::ok(job))))
}

#[utoipa::path(
    get,
    path = "/api/v1/schedules/{schedule_id}/status",
    tag = "Schedules",
    params(
        ("schedule_id" = Uuid, Path, description = "Schedule job ID")
    ),
    responses(
        (status = 200, description = "Schedule job status", body = ApiResponse<shared::types::ScheduleJob>)
    )
)]
#[tracing::instrument(skip(state))]
pub async fn get_status(
    State(state): State<Arc<SchedulingAppState>>,
    Path(schedule_id): Path<Uuid>,
) -> Result<Json<ApiResponse<shared::types::ScheduleJob>>, SchedulingServiceError> {
    let job = state.scheduling_service.get_status(schedule_id).await?;

    Ok(Json(ApiResponse::ok(job)))
}

#[utoipa::path(
    get,
    path = "/api/v1/schedules/{schedule_id}/result",
    tag = "Schedules",
    params(
        ("schedule_id" = Uuid, Path, description = "Schedule job ID")
    ),
    responses(
        (status = 200, description = "Schedule result with shift assignments", body = ApiResponse<shared::types::ScheduleResult>)
    )
)]
#[tracing::instrument(skip(state))]
pub async fn get_result(
    State(state): State<Arc<SchedulingAppState>>,
    Path(schedule_id): Path<Uuid>,
) -> Result<Json<ApiResponse<shared::types::ScheduleResult>>, SchedulingServiceError> {
    let output = state.scheduling_service.get_result(schedule_id).await?;

    Ok(Json(ApiResponse::ok(output)))
}
