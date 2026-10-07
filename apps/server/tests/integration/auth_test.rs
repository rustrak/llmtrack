use crate::common::{test_app, PASSWORD};
use serde_json::json;

#[actix_web::test]
async fn login_returns_the_user_and_me_answers_with_the_session() {
    let mut app = test_app().await;
    app.user("ada@example.com", "admin").await;

    let (status, body) = app
        .call(
            "POST",
            "/auth/login",
            Some(json!({"email": "Ada@Example.com ", "password": PASSWORD})),
        )
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["user"]["email"], "ada@example.com");
    assert_eq!(body["user"]["role"], "admin");
    assert!(body["user"].get("password_hash").is_none());

    let (status, me) = app.call("GET", "/auth/me", None).await;
    assert_eq!(status, 200);
    assert_eq!(me["user"]["email"], "ada@example.com");
}

#[actix_web::test]
async fn a_wrong_password_and_an_unknown_email_fail_the_same_way() {
    let mut app = test_app().await;
    app.user("ada@example.com", "member").await;

    let (wrong_status, wrong) = app
        .call(
            "POST",
            "/auth/login",
            Some(json!({"email": "ada@example.com", "password": "nope"})),
        )
        .await;
    let (unknown_status, unknown) = app
        .call(
            "POST",
            "/auth/login",
            Some(json!({"email": "bob@example.com", "password": "nope"})),
        )
        .await;

    assert_eq!(wrong_status, 401);
    assert_eq!(unknown_status, 401);
    assert_eq!(wrong["error"]["message"], unknown["error"]["message"]);
}

#[actix_web::test]
async fn a_disabled_account_cannot_log_in_or_use_an_open_session() {
    let mut app = test_app().await;
    let id = app.login_as("ada@example.com", "member").await;

    sqlx::query("UPDATE users SET is_active = FALSE WHERE id = $1")
        .bind(id)
        .execute(app.pool())
        .await
        .unwrap();

    assert_eq!(app.call("GET", "/auth/me", None).await.0, 401);
    let (status, _) = app
        .call(
            "POST",
            "/auth/login",
            Some(json!({"email": "ada@example.com", "password": PASSWORD})),
        )
        .await;
    assert_eq!(status, 401);
}

#[actix_web::test]
async fn me_without_a_session_is_401() {
    let mut app = test_app().await;
    assert_eq!(app.call("GET", "/auth/me", None).await.0, 401);
}

#[actix_web::test]
async fn logout_ends_the_session() {
    let mut app = test_app().await;
    app.login_as("ada@example.com", "member").await;

    assert_eq!(app.call("POST", "/auth/logout", None).await.0, 204);
    assert_eq!(app.call("GET", "/auth/me", None).await.0, 401);
}

#[actix_web::test]
async fn login_records_last_login() {
    let mut app = test_app().await;
    app.login_as("ada@example.com", "member").await;

    let (_, me) = app.call("GET", "/auth/me", None).await;
    assert!(me["user"]["last_login"].is_string());
}

#[actix_web::test]
async fn a_user_changes_their_own_password() {
    let mut app = test_app().await;
    app.login_as("ada@example.com", "member").await;

    let (status, _) = app
        .call(
            "POST",
            "/auth/me/password",
            Some(json!({"current_password": "wrong", "new_password": "a-new-password"})),
        )
        .await;
    assert_eq!(status, 401);

    let (status, _) = app
        .call(
            "POST",
            "/auth/me/password",
            Some(json!({"current_password": PASSWORD, "new_password": "a-new-password"})),
        )
        .await;
    assert_eq!(status, 204);

    app.logout_locally();
    let (status, _) = app
        .call(
            "POST",
            "/auth/login",
            Some(json!({"email": "ada@example.com", "password": "a-new-password"})),
        )
        .await;
    assert_eq!(status, 200);
}

#[actix_web::test]
async fn a_user_edits_their_profile_and_preferences() {
    let mut app = test_app().await;
    app.login_as("ada@example.com", "member").await;

    let (status, me) = app
        .call(
            "PATCH",
            "/auth/me",
            Some(json!({"name": "Ada Lovelace", "language": "es", "timezone": "Europe/Madrid", "currency": "EUR", "currency_rate": 0.86})),
        )
        .await;
    assert_eq!(status, 200, "{me}");
    assert_eq!(me["user"]["name"], "Ada Lovelace");
    assert_eq!(me["user"]["language"], "es");
    assert_eq!(me["user"]["timezone"], "Europe/Madrid");
    assert_eq!(me["user"]["currency"], "EUR");
    assert_eq!(me["user"]["currency_rate"], 0.86);

    // Absent fields stay, null clears.
    let (_, me) = app
        .call("PATCH", "/auth/me", Some(json!({"language": null})))
        .await;
    assert_eq!(me["user"]["language"], serde_json::Value::Null);
    assert_eq!(me["user"]["timezone"], "Europe/Madrid");

    let (_, again) = app.call("GET", "/auth/me", None).await;
    assert_eq!(again["user"]["name"], "Ada Lovelace");
}

#[actix_web::test]
async fn malformed_preferences_name_their_field() {
    let mut app = test_app().await;
    app.login_as("ada@example.com", "member").await;

    for (field, value) in [
        ("language", "not a tag!"),
        ("timezone", "Europe/../etc"),
        ("currency", "GBP"),
        ("name", &"x".repeat(101)),
    ] {
        let (status, body) = app
            .call("PATCH", "/auth/me", Some(json!({ field: value })))
            .await;
        assert_eq!(status, 400, "{field}");
        assert_eq!(body["error"]["fields"][0]["field"], field);
    }
    for rate in [json!(0), json!(-1.2)] {
        let (status, body) = app
            .call("PATCH", "/auth/me", Some(json!({ "currency_rate": rate })))
            .await;
        assert_eq!(status, 400, "{rate}");
        assert_eq!(body["error"]["fields"][0]["field"], "currency_rate");
    }
}
