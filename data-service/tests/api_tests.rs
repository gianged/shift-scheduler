use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
    routing::{delete, get, patch, post},
};
use chrono::Utc;
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;
use uuid::Uuid;

use data_service::{
    api::{
        handler::{group, membership, staff},
        state::DataServiceAppState,
    },
    domain::{
        group::MockGroupRepository, membership::MockMembershipRepository,
        staff::MockStaffRepository,
    },
    error::DataServiceError,
};
use shared::types::{Staff, StaffGroup, StaffStatus};

fn build_test_app(
    mock_staff: MockStaffRepository,
    mock_group: MockGroupRepository,
    mock_membership: MockMembershipRepository,
) -> Router {
    let state = Arc::new(DataServiceAppState {
        staff_repo: Arc::new(mock_staff),
        group_repo: Arc::new(mock_group),
        membership_repo: Arc::new(mock_membership),
    });

    Router::new()
        .route("/api/v1/staff", get(staff::find_all).post(staff::create))
        .route("/api/v1/staff/batch", post(staff::batch_create))
        .route(
            "/api/v1/staff/{id}",
            get(staff::find_by_id)
                .put(staff::update)
                .delete(staff::delete),
        )
        .route("/api/v1/staff/{id}/deactivate", patch(staff::deactivate))
        .route("/api/v1/groups", get(group::find_all).post(group::create))
        .route("/api/v1/groups/batch", post(group::batch_create))
        .route(
            "/api/v1/groups/{id}",
            get(group::find_by_id)
                .put(group::update)
                .delete(group::delete),
        )
        .route(
            "/api/v1/groups/{group_id}/members",
            get(membership::get_group_members).post(membership::add_member),
        )
        .route(
            "/api/v1/groups/{group_id}/members/{staff_id}",
            delete(membership::remove_member),
        )
        .route(
            "/api/v1/memberships/batch",
            post(membership::batch_add_members),
        )
        .route(
            "/api/v1/groups/{group_id}/resolved-members",
            get(membership::resolve_members),
        )
        .route(
            "/api/v1/staff/{id}/groups",
            get(membership::get_staff_groups),
        )
        .with_state(state)
}

fn make_staff(id: Uuid) -> Staff {
    let now = Utc::now();
    Staff {
        id,
        name: "Alice".to_string(),
        email: format!("alice-{id}@example.com"),
        position: "Nurse".to_string(),
        status: StaffStatus::Active,
        created_at: now,
        updated_at: now,
    }
}

fn make_group(id: Uuid) -> StaffGroup {
    let now = Utc::now();
    StaffGroup {
        id,
        name: "Ward A".to_string(),
        parent_group_id: None,
        created_at: now,
        updated_at: now,
    }
}

#[tokio::test]
async fn create_staff_returns_ok() {
    let mut mock_staff = MockStaffRepository::new();
    let staff_id = Uuid::new_v4();
    let staff = make_staff(staff_id);

    mock_staff
        .expect_create()
        .returning(move |_| Ok(staff.clone()));

    let app = build_test_app(
        mock_staff,
        MockGroupRepository::new(),
        MockMembershipRepository::new(),
    );

    let body = json!({
        "name": "Alice",
        "email": "alice@example.com",
        "position": "Nurse"
    });

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/staff")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["success"].as_bool().unwrap());
    assert_eq!(json["data"]["name"], "Alice");
}

#[tokio::test]
async fn find_all_staff_returns_list() {
    let mut mock_staff = MockStaffRepository::new();
    let staff = vec![make_staff(Uuid::new_v4()), make_staff(Uuid::new_v4())];

    mock_staff
        .expect_find_all()
        .returning(move || Ok(staff.clone()));

    let app = build_test_app(
        mock_staff,
        MockGroupRepository::new(),
        MockMembershipRepository::new(),
    );

    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/staff")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["success"].as_bool().unwrap());
    assert_eq!(json["data"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn find_staff_not_found_returns_404() {
    let mut mock_staff = MockStaffRepository::new();
    mock_staff.expect_find_by_id().returning(|_| Ok(None));

    let app = build_test_app(
        mock_staff,
        MockGroupRepository::new(),
        MockMembershipRepository::new(),
    );

    let res = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/staff/{}", Uuid::new_v4()))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn create_group_returns_ok() {
    let mut mock_group = MockGroupRepository::new();
    let group = make_group(Uuid::new_v4());

    mock_group
        .expect_create()
        .returning(move |_| Ok(group.clone()));

    let app = build_test_app(
        MockStaffRepository::new(),
        mock_group,
        MockMembershipRepository::new(),
    );

    let body = json!({
        "name": "Ward A"
    });

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/groups")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["success"].as_bool().unwrap());
    assert_eq!(json["data"]["name"], "Ward A");
}

#[tokio::test]
async fn resolve_members_returns_nested() {
    let mut mock_membership = MockMembershipRepository::new();
    let staff = vec![make_staff(Uuid::new_v4())];

    mock_membership
        .expect_resolve_members()
        .returning(move |_| Ok(staff.clone()));

    let app = build_test_app(
        MockStaffRepository::new(),
        MockGroupRepository::new(),
        mock_membership,
    );

    let group_id = Uuid::new_v4();
    let res = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/groups/{group_id}/resolved-members"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["success"].as_bool().unwrap());
    assert_eq!(json["data"].as_array().unwrap().len(), 1);
}

// -- Staff update / delete / deactivate tests --

#[tokio::test]
async fn update_staff_returns_updated() {
    let mut mock_staff = MockStaffRepository::new();
    let staff_id = Uuid::new_v4();
    let mut updated = make_staff(staff_id);
    updated.name = "Alice Updated".to_string();

    mock_staff
        .expect_update()
        .returning(move |_, _| Ok(updated.clone()));

    let app = build_test_app(
        mock_staff,
        MockGroupRepository::new(),
        MockMembershipRepository::new(),
    );

    let body = json!({ "name": "Alice Updated" });
    let res = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/staff/{staff_id}"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["success"].as_bool().unwrap());
    assert_eq!(json["data"]["name"], "Alice Updated");
}

#[tokio::test]
async fn update_staff_not_found_returns_404() {
    let mut mock_staff = MockStaffRepository::new();
    mock_staff
        .expect_update()
        .returning(|_, _| Err(DataServiceError::NotFound("Staff not found".into())));

    let app = build_test_app(
        mock_staff,
        MockGroupRepository::new(),
        MockMembershipRepository::new(),
    );

    let body = json!({ "name": "Ghost" });
    let res = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/staff/{}", Uuid::new_v4()))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_staff_returns_ok() {
    let mut mock_staff = MockStaffRepository::new();
    mock_staff.expect_delete().returning(|_| Ok(()));

    let app = build_test_app(
        mock_staff,
        MockGroupRepository::new(),
        MockMembershipRepository::new(),
    );

    let res = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/staff/{}", Uuid::new_v4()))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn delete_staff_not_found_returns_404() {
    let mut mock_staff = MockStaffRepository::new();
    mock_staff
        .expect_delete()
        .returning(|_| Err(DataServiceError::NotFound("Staff not found".into())));

    let app = build_test_app(
        mock_staff,
        MockGroupRepository::new(),
        MockMembershipRepository::new(),
    );

    let res = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/staff/{}", Uuid::new_v4()))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn deactivate_staff_returns_ok() {
    let mut mock_staff = MockStaffRepository::new();
    mock_staff.expect_deactivate().returning(|_| Ok(()));

    let app = build_test_app(
        mock_staff,
        MockGroupRepository::new(),
        MockMembershipRepository::new(),
    );

    let res = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/staff/{}/deactivate", Uuid::new_v4()))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn create_staff_duplicate_email_returns_409() {
    let mut mock_staff = MockStaffRepository::new();
    mock_staff
        .expect_create()
        .returning(|_| Err(DataServiceError::Conflict("Email already exists".into())));

    let app = build_test_app(
        mock_staff,
        MockGroupRepository::new(),
        MockMembershipRepository::new(),
    );

    let body = json!({
        "name": "Duplicate",
        "email": "dup@example.com",
        "position": "Staff"
    });

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/staff")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn batch_create_staff_returns_list() {
    let mut mock_staff = MockStaffRepository::new();
    let staff = vec![make_staff(Uuid::new_v4()), make_staff(Uuid::new_v4())];

    mock_staff
        .expect_batch_create()
        .returning(move |_| Ok(staff.clone()));

    let app = build_test_app(
        mock_staff,
        MockGroupRepository::new(),
        MockMembershipRepository::new(),
    );

    let body = json!([
        { "name": "A", "email": "a@example.com", "position": "Staff" },
        { "name": "B", "email": "b@example.com", "position": "Staff" }
    ]);

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/staff/batch")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["success"].as_bool().unwrap());
    assert_eq!(json["data"].as_array().unwrap().len(), 2);
}

// -- Group update / delete / batch tests --

#[tokio::test]
async fn update_group_returns_updated() {
    let mut mock_group = MockGroupRepository::new();
    let group_id = Uuid::new_v4();
    let mut updated = make_group(group_id);
    updated.name = "Ward B".to_string();

    mock_group
        .expect_update()
        .returning(move |_, _| Ok(updated.clone()));

    let app = build_test_app(
        MockStaffRepository::new(),
        mock_group,
        MockMembershipRepository::new(),
    );

    let body = json!({ "name": "Ward B" });
    let res = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/groups/{group_id}"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["success"].as_bool().unwrap());
    assert_eq!(json["data"]["name"], "Ward B");
}

#[tokio::test]
async fn delete_group_returns_ok() {
    let mut mock_group = MockGroupRepository::new();
    mock_group.expect_delete().returning(|_| Ok(()));

    let app = build_test_app(
        MockStaffRepository::new(),
        mock_group,
        MockMembershipRepository::new(),
    );

    let res = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/groups/{}", Uuid::new_v4()))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn find_group_not_found_returns_404() {
    let mut mock_group = MockGroupRepository::new();
    mock_group.expect_find_by_id().returning(|_| Ok(None));

    let app = build_test_app(
        MockStaffRepository::new(),
        mock_group,
        MockMembershipRepository::new(),
    );

    let res = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/groups/{}", Uuid::new_v4()))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn batch_create_groups_returns_list() {
    let mut mock_group = MockGroupRepository::new();
    let groups = vec![make_group(Uuid::new_v4()), make_group(Uuid::new_v4())];

    mock_group
        .expect_batch_create()
        .returning(move |_| Ok(groups.clone()));

    let app = build_test_app(
        MockStaffRepository::new(),
        mock_group,
        MockMembershipRepository::new(),
    );

    let body = json!([
        { "name": "Ward A" },
        { "name": "Ward B" }
    ]);

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/groups/batch")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["success"].as_bool().unwrap());
    assert_eq!(json["data"].as_array().unwrap().len(), 2);
}

// -- Membership tests --

#[tokio::test]
async fn add_member_returns_ok() {
    let mut mock_membership = MockMembershipRepository::new();
    mock_membership
        .expect_add_staff_to_group()
        .returning(|_, _| Ok(()));

    let app = build_test_app(
        MockStaffRepository::new(),
        MockGroupRepository::new(),
        mock_membership,
    );

    let group_id = Uuid::new_v4();
    let body = json!({ "staff_id": Uuid::new_v4(), "group_id": group_id });

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/groups/{group_id}/members"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn remove_member_returns_ok() {
    let mut mock_membership = MockMembershipRepository::new();
    mock_membership
        .expect_remove_staff_from_group()
        .returning(|_, _| Ok(()));

    let app = build_test_app(
        MockStaffRepository::new(),
        MockGroupRepository::new(),
        mock_membership,
    );

    let group_id = Uuid::new_v4();
    let staff_id = Uuid::new_v4();

    let res = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/groups/{group_id}/members/{staff_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn get_group_members_returns_list() {
    let mut mock_membership = MockMembershipRepository::new();
    let staff = vec![make_staff(Uuid::new_v4()), make_staff(Uuid::new_v4())];

    mock_membership
        .expect_get_group_members()
        .returning(move |_| Ok(staff.clone()));

    let app = build_test_app(
        MockStaffRepository::new(),
        MockGroupRepository::new(),
        mock_membership,
    );

    let group_id = Uuid::new_v4();
    let res = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/groups/{group_id}/members"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["success"].as_bool().unwrap());
    assert_eq!(json["data"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn get_staff_groups_returns_list() {
    let mut mock_membership = MockMembershipRepository::new();
    let groups = vec![make_group(Uuid::new_v4())];

    mock_membership
        .expect_get_staff_groups()
        .returning(move |_| Ok(groups.clone()));

    let app = build_test_app(
        MockStaffRepository::new(),
        MockGroupRepository::new(),
        mock_membership,
    );

    let staff_id = Uuid::new_v4();
    let res = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/staff/{staff_id}/groups"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["success"].as_bool().unwrap());
    assert_eq!(json["data"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn batch_add_members_returns_ok() {
    let mut mock_membership = MockMembershipRepository::new();
    mock_membership
        .expect_batch_add_members()
        .returning(|_| Ok(()));

    let app = build_test_app(
        MockStaffRepository::new(),
        MockGroupRepository::new(),
        mock_membership,
    );

    let body = json!([
        { "staff_id": Uuid::new_v4(), "group_id": Uuid::new_v4() },
        { "staff_id": Uuid::new_v4(), "group_id": Uuid::new_v4() }
    ]);

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/memberships/batch")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
}
