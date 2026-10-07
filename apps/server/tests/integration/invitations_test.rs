//! Invitations, as Rustrak does them: an admin creates one, shares the link
//! by hand, and the invitee sets their own password.

use crate::common::{test_app, PASSWORD};
use serde_json::json;

#[actix_web::test]
async fn an_invited_user_sets_a_password_and_is_signed_in() {
    let mut app = test_app().await;
    app.admin().await;
    let (status, invite) = app
        .call(
            "POST",
            "/api/invitations",
            Some(json!({"email": " Bob@Example.com ", "role": "member"})),
        )
        .await;
    assert_eq!(status, 201, "{invite}");
    assert_eq!(invite["email"], "bob@example.com");
    assert_eq!(invite["status"], "pending");
    let token = invite["token"].as_str().unwrap().to_string();
    assert_eq!(token.len(), 40);
    let (_, list) = app.call("GET", "/api/invitations", None).await;
    assert_eq!(list[0]["token"], token.as_str());

    app.logout_locally();
    let (status, info) = app
        .call("GET", &format!("/auth/invitation/{token}"), None)
        .await;
    assert_eq!(status, 200);
    assert_eq!(info["email"], "bob@example.com");
    assert!(info.get("token").is_none());

    let (status, me) = app
        .call(
            "POST",
            "/auth/accept-invitation",
            Some(json!({"token": token, "password": PASSWORD})),
        )
        .await;
    assert_eq!(status, 201, "{me}");
    assert_eq!(me["user"]["email"], "bob@example.com");
    assert_eq!(me["user"]["role"], "member");
    let (status, _) = app.call("GET", "/auth/me", None).await;
    assert_eq!(status, 200, "accepting signs you in");

    let (status, _) = app
        .call(
            "POST",
            "/auth/accept-invitation",
            Some(json!({"token": token, "password": PASSWORD})),
        )
        .await;
    assert_eq!(status, 400, "an invitation works once");
    let (status, _) = app
        .call("GET", &format!("/auth/invitation/{token}"), None)
        .await;
    assert_eq!(status, 400);
}

#[actix_web::test]
async fn only_admins_invite() {
    let mut app = test_app().await;
    app.login_as("bob@example.com", "member").await;
    let (status, _) = app
        .call(
            "POST",
            "/api/invitations",
            Some(json!({"email": "c@example.com"})),
        )
        .await;
    assert_eq!(status, 403);
    assert_eq!(app.call("GET", "/api/invitations", None).await.0, 403);
}

#[actix_web::test]
async fn existing_users_and_pending_invitations_blame_the_email() {
    let mut app = test_app().await;
    app.admin().await;
    app.user("bob@example.com", "member").await;
    let (status, body) = app
        .call(
            "POST",
            "/api/invitations",
            Some(json!({"email": "BOB@example.com"})),
        )
        .await;
    assert_eq!(status, 409);
    assert_eq!(body["error"]["fields"][0]["field"], "email");

    app.call(
        "POST",
        "/api/invitations",
        Some(json!({"email": "carol@example.com"})),
    )
    .await;
    let (status, body) = app
        .call(
            "POST",
            "/api/invitations",
            Some(json!({"email": "carol@example.com"})),
        )
        .await;
    assert_eq!(status, 409);
    assert_eq!(body["error"]["fields"][0]["field"], "email");

    let (status, body) = app
        .call(
            "POST",
            "/api/invitations",
            Some(json!({"email": "nope", "role": "boss"})),
        )
        .await;
    assert_eq!(status, 400, "{body}");
}

#[actix_web::test]
async fn a_revoked_or_expired_invitation_cannot_be_used() {
    let mut app = test_app().await;
    app.admin().await;
    let (_, invite) = app
        .call(
            "POST",
            "/api/invitations",
            Some(json!({"email": "bob@example.com"})),
        )
        .await;
    let token = invite["token"].as_str().unwrap().to_string();
    assert_eq!(
        app.call("DELETE", &format!("/api/invitations/{token}"), None)
            .await
            .0,
        204
    );
    assert_eq!(
        app.call("DELETE", &format!("/api/invitations/{token}"), None)
            .await
            .0,
        404
    );
    let (status, _) = app
        .call(
            "POST",
            "/auth/accept-invitation",
            Some(json!({"token": token, "password": PASSWORD})),
        )
        .await;
    assert_eq!(status, 400);

    let (_, invite) = app
        .call(
            "POST",
            "/api/invitations",
            Some(json!({"email": "bob@example.com"})),
        )
        .await;
    let token = invite["token"].as_str().unwrap().to_string();
    sqlx::query("UPDATE invitations SET expires_at = $1 WHERE token = $2")
        .bind(chrono::Utc::now() - chrono::Duration::minutes(1))
        .bind(&token)
        .execute(app.pool())
        .await
        .unwrap();
    assert_eq!(
        app.call("GET", &format!("/auth/invitation/{token}"), None)
            .await
            .0,
        400
    );
    let (status, _) = app
        .call(
            "POST",
            "/auth/accept-invitation",
            Some(json!({"token": token, "password": PASSWORD})),
        )
        .await;
    assert_eq!(status, 400);
    assert_eq!(
        app.call("GET", "/auth/invitation/unknown", None).await.0,
        404
    );
}

#[actix_web::test]
async fn accepting_needs_a_valid_password() {
    let mut app = test_app().await;
    app.admin().await;
    let (_, invite) = app
        .call(
            "POST",
            "/api/invitations",
            Some(json!({"email": "bob@example.com"})),
        )
        .await;
    app.logout_locally();
    let (status, body) = app
        .call(
            "POST",
            "/auth/accept-invitation",
            Some(json!({"token": invite["token"], "password": ""})),
        )
        .await;
    assert_eq!(status, 400);
    assert_eq!(body["error"]["fields"][0]["field"], "password");
    let (status, _) = app
        .call(
            "GET",
            &format!("/auth/invitation/{}", invite["token"].as_str().unwrap()),
            None,
        )
        .await;
    assert_eq!(status, 200, "a failed attempt leaves it pending");
}
