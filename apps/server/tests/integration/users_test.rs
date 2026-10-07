use crate::common::test_app;
use serde_json::json;

#[actix_web::test]
async fn an_admin_creates_lists_updates_and_deletes_users() {
    let mut app = test_app().await;
    app.admin().await;

    let (status, created) = app
        .call(
            "POST",
            "/api/users",
            Some(json!({"email": "Bob@Example.com", "password": "long-enough"})),
        )
        .await;
    assert_eq!(status, 201);
    assert_eq!(created["email"], "bob@example.com");
    assert_eq!(created["role"], "member");
    let id = created["id"].as_i64().unwrap();

    let (_, list) = app.call("GET", "/api/users", None).await;
    assert_eq!(list["data"].as_array().unwrap().len(), 2);

    let (status, updated) = app
        .call(
            "PATCH",
            &format!("/api/users/{id}"),
            Some(json!({"role": "admin", "is_active": false})),
        )
        .await;
    assert_eq!(status, 200);
    assert_eq!(updated["role"], "admin");
    assert_eq!(updated["is_active"], false);

    assert_eq!(
        app.call("DELETE", &format!("/api/users/{id}"), None)
            .await
            .0,
        204
    );
    assert_eq!(
        app.call("DELETE", &format!("/api/users/{id}"), None)
            .await
            .0,
        404
    );
}

#[actix_web::test]
async fn a_duplicate_email_is_a_409_on_the_email_field() {
    let mut app = test_app().await;
    app.admin().await;

    let (status, body) = app
        .call(
            "POST",
            "/api/users",
            Some(json!({"email": "admin@example.com", "password": "long-enough"})),
        )
        .await;
    assert_eq!(status, 409);
    assert_eq!(body["error"]["fields"][0]["field"], "email");
    assert_eq!(body["error"]["fields"][0]["code"], "already_exists");
}

#[actix_web::test]
async fn invalid_input_names_the_field() {
    let mut app = test_app().await;
    app.admin().await;

    let (status, body) = app
        .call(
            "POST",
            "/api/users",
            Some(json!({"email": "nope", "password": "long-enough"})),
        )
        .await;
    assert_eq!(status, 400);
    assert_eq!(body["error"]["fields"][0]["field"], "email");

    let (status, body) = app
        .call(
            "POST",
            "/api/users",
            Some(json!({"email": "a@b.co", "password": "short"})),
        )
        .await;
    assert_eq!(status, 400);
    assert_eq!(body["error"]["fields"][0]["field"], "password");
}

#[actix_web::test]
async fn an_admin_cannot_lock_themselves_out() {
    let mut app = test_app().await;
    let me = app.admin().await;

    let (status, _) = app
        .call(
            "PATCH",
            &format!("/api/users/{me}"),
            Some(json!({"role": "member"})),
        )
        .await;
    assert_eq!(status, 403);
    assert_eq!(
        app.call("DELETE", &format!("/api/users/{me}"), None)
            .await
            .0,
        403
    );
}

#[actix_web::test]
async fn a_member_cannot_manage_users() {
    let mut app = test_app().await;
    app.login_as("bob@example.com", "member").await;

    assert_eq!(app.call("GET", "/api/users", None).await.0, 403);
    let (status, _) = app
        .call(
            "POST",
            "/api/users",
            Some(json!({"email": "c@d.co", "password": "long-enough"})),
        )
        .await;
    assert_eq!(status, 403);
}

#[actix_web::test]
async fn the_api_needs_a_session() {
    let mut app = test_app().await;
    assert_eq!(app.call("GET", "/api/users", None).await.0, 401);
}

#[actix_web::test]
async fn a_malformed_body_is_a_json_400() {
    let mut app = test_app().await;
    app.admin().await;

    let (status, body) = app
        .call("POST", "/api/users", Some(json!({"email": 42})))
        .await;
    assert_eq!(status, 400);
    assert_eq!(body["error"]["type"], "ValidationError");
}

#[actix_web::test]
async fn the_user_list_pages_sorts_searches_and_filters() {
    let mut app = test_app().await;
    app.admin().await;
    app.user("carol@example.com", "member").await;
    let bob = app.user("bob@example.com", "member").await;
    app.call(
        "PATCH",
        &format!("/api/users/{bob}"),
        Some(json!({"is_active": false})),
    )
    .await;
    let emails = |list: &serde_json::Value| -> Vec<String> {
        list["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|u| u["email"].as_str().unwrap().to_string())
            .collect()
    };

    let (status, page) = app.call("GET", "/api/users?per_page=2", None).await;
    assert_eq!(status, 200, "{page}");
    assert_eq!(emails(&page), ["admin@example.com", "bob@example.com"]);
    assert_eq!(page["total"], 3);
    assert!(page["data"][0].get("password_hash").is_none());
    let (_, members) = app.call("GET", "/api/users?role=member", None).await;
    assert_eq!(emails(&members), ["bob@example.com", "carol@example.com"]);
    let (_, inactive) = app.call("GET", "/api/users?status=inactive", None).await;
    assert_eq!(emails(&inactive), ["bob@example.com"]);
    let (_, found) = app.call("GET", "/api/users?q=CAROL", None).await;
    assert_eq!(emails(&found), ["carol@example.com"]);
    let (_, desc) = app.call("GET", "/api/users?sort=-email", None).await;
    assert_eq!(emails(&desc)[0], "carol@example.com");
    assert_eq!(
        app.call("GET", "/api/users?sort=password_hash", None)
            .await
            .0,
        400
    );
    assert_eq!(app.call("GET", "/api/users?role=owner", None).await.0, 400);
}
