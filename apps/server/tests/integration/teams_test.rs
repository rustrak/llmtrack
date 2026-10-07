use crate::common::test_app;
use serde_json::{json, Value};

async fn model<S>(app: &mut crate::common::TestApp<S>, name: &str) -> i64
where
    S: actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
{
    let (status, body) = app
        .call(
            "POST",
            "/api/models",
            Some(json!({"name": name, "provider": "openai", "upstream_model": name})),
        )
        .await;
    assert_eq!(status, 201, "{body}");
    body["id"].as_i64().unwrap()
}

fn names(models: &Value) -> Vec<String> {
    models
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["name"].as_str().unwrap().to_string())
        .collect()
}

#[actix_web::test]
async fn an_admin_creates_a_team_with_a_budget_and_model_allowlist() {
    let mut app = test_app().await;
    app.admin().await;
    let gpt = model(&mut app, "gpt-4o").await;
    model(&mut app, "claude").await;

    let (status, team) = app
        .call(
            "POST",
            "/api/teams",
            Some(json!({"name": "Acme", "max_budget_usd": 100.5, "models": [gpt]})),
        )
        .await;
    assert_eq!(status, 201, "{team}");
    assert_eq!(team["name"], "Acme");
    assert_eq!(team["max_budget_usd"], 100.5);
    assert_eq!(team["spend_usd"], 0.0);
    assert_eq!(names(&team["models"]), ["gpt-4o"]);

    let (_, teams) = app.call("GET", "/api/teams", None).await;
    assert_eq!(teams["data"].as_array().unwrap().len(), 1);
    assert_eq!(teams["data"][0]["key_count"], 0);
}

#[actix_web::test]
async fn team_names_are_unique_and_required() {
    let mut app = test_app().await;
    app.admin().await;
    app.call("POST", "/api/teams", Some(json!({"name": "Acme"})))
        .await;

    let (status, body) = app
        .call("POST", "/api/teams", Some(json!({"name": "Acme"})))
        .await;
    assert_eq!(status, 409);
    assert_eq!(body["error"]["fields"][0]["field"], "name");

    let (status, _) = app
        .call("POST", "/api/teams", Some(json!({"name": "  "})))
        .await;
    assert_eq!(status, 400);
}

#[actix_web::test]
async fn an_unknown_model_in_the_allowlist_is_refused() {
    let mut app = test_app().await;
    app.admin().await;

    let (status, body) = app
        .call(
            "POST",
            "/api/teams",
            Some(json!({"name": "Acme", "models": [999]})),
        )
        .await;
    assert_eq!(status, 400);
    assert_eq!(body["error"]["fields"][0]["field"], "models");
}

#[actix_web::test]
async fn updating_a_team_can_clear_its_budget_and_replace_its_models() {
    let mut app = test_app().await;
    app.admin().await;
    let gpt = model(&mut app, "gpt-4o").await;
    let claude = model(&mut app, "claude").await;
    let (_, team) = app
        .call(
            "POST",
            "/api/teams",
            Some(json!({"name": "Acme", "max_budget_usd": 5, "models": [gpt]})),
        )
        .await;
    let uri = format!("/api/teams/{}", team["id"]);

    let (status, updated) = app
        .call(
            "PATCH",
            &uri,
            Some(json!({"max_budget_usd": null, "models": [claude]})),
        )
        .await;
    assert_eq!(status, 200, "{updated}");
    assert_eq!(updated["max_budget_usd"], Value::Null);
    assert_eq!(names(&updated["models"]), ["claude"]);
    assert_eq!(updated["name"], "Acme");

    assert_eq!(app.call("DELETE", &uri, None).await.0, 204);
    assert_eq!(app.call("GET", &uri, None).await.0, 404);
}

#[actix_web::test]
async fn members_only_see_their_own_teams() {
    let mut app = test_app().await;
    app.admin().await;
    let bob = app.user("bob@example.com", "member").await;
    let (_, acme) = app
        .call("POST", "/api/teams", Some(json!({"name": "Acme"})))
        .await;
    let (_, other) = app
        .call("POST", "/api/teams", Some(json!({"name": "Other"})))
        .await;
    let (status, _) = app
        .call(
            "POST",
            &format!("/api/teams/{}/members", acme["id"]),
            Some(json!({"email": "bob@example.com"})),
        )
        .await;
    assert_eq!(status, 201);

    app.logout_locally();
    let (status, _) = app
        .call(
            "POST",
            "/auth/login",
            Some(json!({"email": "bob@example.com", "password": crate::common::PASSWORD})),
        )
        .await;
    assert_eq!(status, 200);

    let (_, teams) = app.call("GET", "/api/teams", None).await;
    assert_eq!(names(&teams["data"]), ["Acme"]);
    assert_eq!(teams["data"][0]["my_role"], "member");

    let (status, detail) = app
        .call("GET", &format!("/api/teams/{}", acme["id"]), None)
        .await;
    assert_eq!(status, 200);
    assert_eq!(detail["members"][0]["user_id"], bob);
    assert_eq!(
        app.call("GET", &format!("/api/teams/{}", other["id"]), None)
            .await
            .0,
        404
    );

    // Members cannot reshape the team or invite people.
    assert_eq!(
        app.call("POST", "/api/teams", Some(json!({"name": "Mine"})))
            .await
            .0,
        403
    );
    let (status, _) = app
        .call(
            "PATCH",
            &format!("/api/teams/{}", acme["id"]),
            Some(json!({"name": "Renamed"})),
        )
        .await;
    assert_eq!(status, 403);
    let (status, _) = app
        .call(
            "POST",
            &format!("/api/teams/{}/members", acme["id"]),
            Some(json!({"email": "admin@example.com"})),
        )
        .await;
    assert_eq!(status, 403);
}

#[actix_web::test]
async fn a_team_admin_manages_members() {
    let mut app = test_app().await;
    app.admin().await;
    app.user("lead@example.com", "member").await;
    let carol = app.user("carol@example.com", "member").await;
    let (_, team) = app
        .call("POST", "/api/teams", Some(json!({"name": "Acme"})))
        .await;
    let members = format!("/api/teams/{}/members", team["id"]);
    app.call(
        "POST",
        &members,
        Some(json!({"email": "lead@example.com", "role": "admin"})),
    )
    .await;

    app.logout_locally();
    app.call(
        "POST",
        "/auth/login",
        Some(json!({"email": "lead@example.com", "password": crate::common::PASSWORD})),
    )
    .await;

    let (status, _) = app
        .call(
            "POST",
            &members,
            Some(json!({"email": "carol@example.com"})),
        )
        .await;
    assert_eq!(status, 201);
    let (status, _) = app
        .call(
            "POST",
            &members,
            Some(json!({"email": "carol@example.com"})),
        )
        .await;
    assert_eq!(status, 409);
    let (status, _) = app
        .call(
            "POST",
            &members,
            Some(json!({"email": "ghost@example.com"})),
        )
        .await;
    assert_eq!(status, 404);

    let (status, member) = app
        .call(
            "PATCH",
            &format!("{members}/{carol}"),
            Some(json!({"role": "admin"})),
        )
        .await;
    assert_eq!(status, 200);
    assert_eq!(member["role"], "admin");

    assert_eq!(
        app.call("DELETE", &format!("{members}/{carol}"), None)
            .await
            .0,
        204
    );
    let (_, detail) = app
        .call("GET", &format!("/api/teams/{}", team["id"]), None)
        .await;
    assert_eq!(detail["members"].as_array().unwrap().len(), 1);
}

#[actix_web::test]
async fn the_team_list_pages_sorts_searches_and_filters() {
    let mut app = test_app().await;
    app.admin().await;
    for (name, all_models) in [("Bravo", true), ("alpha", false), ("Charlie", true)] {
        let (status, body) = app
            .call(
                "POST",
                "/api/teams",
                Some(json!({"name": name, "all_models": all_models})),
            )
            .await;
        assert_eq!(status, 201, "{body}");
    }

    let (_, page) = app.call("GET", "/api/teams?per_page=2", None).await;
    assert_eq!(
        names(&page["data"]),
        ["alpha", "Bravo"],
        "by name, any case"
    );
    assert_eq!(page["total"], 3);
    let (_, desc) = app.call("GET", "/api/teams?sort=-name", None).await;
    assert_eq!(names(&desc["data"]), ["Charlie", "Bravo", "alpha"]);
    let (_, found) = app.call("GET", "/api/teams?q=BR", None).await;
    assert_eq!(names(&found["data"]), ["Bravo"]);
    let (_, restricted) = app.call("GET", "/api/teams?models=restricted", None).await;
    assert_eq!(names(&restricted["data"]), ["alpha"]);
    assert_eq!(app.call("GET", "/api/teams?sort=secret", None).await.0, 400);
    assert_eq!(app.call("GET", "/api/teams?models=some", None).await.0, 400);
}
