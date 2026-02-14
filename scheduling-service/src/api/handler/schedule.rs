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
use uuid::Uuid;

use crate::{api::state::SchedulingAppState, error::SchedulingServiceError};

#[derive(Debug, Deserialize)]
pub struct CreateScheduleRequest {
    pub staff_group_id: Uuid,
    pub period_begin_date: NaiveDate,
}

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

#[tracing::instrument(skip(state))]
pub async fn get_status(
    State(state): State<Arc<SchedulingAppState>>,
    Path(schedule_id): Path<Uuid>,
) -> Result<Json<ApiResponse<shared::types::ScheduleJob>>, SchedulingServiceError> {
    let job = state.scheduling_service.get_status(schedule_id).await?;

    Ok(Json(ApiResponse::ok(job)))
}

#[tracing::instrument(skip(state))]
pub async fn get_result(
    State(state): State<Arc<SchedulingAppState>>,
    Path(schedule_id): Path<Uuid>,
) -> Result<Json<ApiResponse<Vec<shared::types::ShiftAssignment>>>, SchedulingServiceError> {
    let assignments = state.scheduling_service.get_result(schedule_id).await?;

    Ok(Json(ApiResponse::ok(assignments)))
}
