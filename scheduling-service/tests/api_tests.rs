use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
    routing::{get, post},
};
use chrono::NaiveDate;
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;
use uuid::Uuid;

use scheduling_service::{
    api::{handler::schedule, state::SchedulingAppState},
    domain::{
        client::MockDataServiceClient, job::MockJobRepository, scheduler::SchedulingConfig,
        service::SchedulingService,
    },
};
use shared::types::{JobStatus, ScheduleJob, ShiftAssignment, ShiftType};

fn build_test_app(mock_repo: MockJobRepository, mock_client: MockDataServiceClient) -> Router {
    let svc = Arc::new(SchedulingService::new(
        Arc::new(mock_repo),
        Arc::new(mock_client),
        SchedulingConfig::default(),
    ));
    let state = Arc::new(SchedulingAppState {
        scheduling_service: svc,
    });

    Router::new()
        .route("/api/v1/schedules", post(schedule::submit_schedule))
        .route(
            "/api/v1/schedules/{schedule_id}/status",
            get(schedule::get_status),
        )
        .route(
            "/api/v1/schedules/{schedule_id}/result",
            get(schedule::get_result),
        )
        .with_state(state)
}

fn make_job(id: Uuid, status: JobStatus) -> ScheduleJob {
    ScheduleJob {
        id,
        staff_group_id: Uuid::new_v4(),
        period_begin_date: NaiveDate::from_ymd_opt(2026, 2, 16).unwrap(),
        status,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

#[tokio::test]
async fn submit_schedule_returns_202() {
    let mut repo = MockJobRepository::new();
    let job = make_job(Uuid::new_v4(), JobStatus::Pending);
    let job_clone = job.clone();

    repo.expect_create_job()
        .returning(move |_, _| Ok(job_clone.clone()));
    // Background task will call these -- just allow them
    repo.expect_update_status().returning(|_, _| Ok(()));
    repo.expect_save_assignments().returning(|_, _| Ok(()));

    let mut client = MockDataServiceClient::new();
    client
        .expect_get_resolved_members()
        .returning(|_| Ok(vec![]));

    let app = build_test_app(repo, client);

    let body = json!({
        "staff_group_id": job.staff_group_id,
        "period_begin_date": "2026-02-16"
    });

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/schedules")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::ACCEPTED);
}

#[tokio::test]
async fn get_status_returns_job() {
    let mut repo = MockJobRepository::new();
    let job_id = Uuid::new_v4();
    let job = make_job(job_id, JobStatus::Processing);

    repo.expect_find_by_id()
        .returning(move |_| Ok(Some(job.clone())));

    let app = build_test_app(repo, MockDataServiceClient::new());

    let res = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/schedules/{job_id}/status"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["success"].as_bool().unwrap());
}

#[tokio::test]
async fn get_status_not_found_returns_404() {
    let mut repo = MockJobRepository::new();
    repo.expect_find_by_id().returning(|_| Ok(None));

    let app = build_test_app(repo, MockDataServiceClient::new());

    let res = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/schedules/{}/status", Uuid::new_v4()))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn get_result_returns_schedule_result() {
    let mut repo = MockJobRepository::new();
    let job_id = Uuid::new_v4();
    let job = make_job(job_id, JobStatus::Completed);
    let staff_group_id = job.staff_group_id;
    let period_begin_date = job.period_begin_date;

    repo.expect_find_by_id()
        .returning(move |_| Ok(Some(job.clone())));

    let assignment = ShiftAssignment {
        id: Uuid::new_v4(),
        job_id,
        staff_id: Uuid::new_v4(),
        date: NaiveDate::from_ymd_opt(2026, 2, 16).unwrap(),
        shift_type: ShiftType::Morning,
    };
    let assignments = vec![assignment];
    repo.expect_get_assignments()
        .returning(move |_| Ok(assignments.clone()));

    let app = build_test_app(repo, MockDataServiceClient::new());

    let res = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/schedules/{job_id}/result"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["success"].as_bool().unwrap());

    let data = &json["data"];
    assert_eq!(data["schedule_id"], job_id.to_string());
    assert_eq!(data["staff_group_id"], staff_group_id.to_string());
    assert_eq!(data["period_begin_date"], period_begin_date.to_string());
    assert_eq!(data["assignments"].as_array().unwrap().len(), 1);
    assert_eq!(data["assignments"][0]["shift_type"], "MORNING");
}

#[tokio::test]
async fn submit_schedule_non_monday_returns_400() {
    let repo = MockJobRepository::new();
    let client = MockDataServiceClient::new();
    let app = build_test_app(repo, client);

    let body = json!({
        "staff_group_id": Uuid::new_v4(),
        "period_begin_date": "2026-02-17"
    });

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/schedules")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn get_result_not_completed_returns_400() {
    let mut repo = MockJobRepository::new();
    let job_id = Uuid::new_v4();
    let job = make_job(job_id, JobStatus::Processing);

    repo.expect_find_by_id()
        .returning(move |_| Ok(Some(job.clone())));

    let app = build_test_app(repo, MockDataServiceClient::new());

    let res = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/schedules/{job_id}/result"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn get_result_not_found_returns_404() {
    let mut repo = MockJobRepository::new();
    repo.expect_find_by_id().returning(|_| Ok(None));

    let app = build_test_app(repo, MockDataServiceClient::new());

    let res = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/schedules/{}/result", Uuid::new_v4()))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}
