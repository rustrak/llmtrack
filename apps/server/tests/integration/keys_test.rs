use crate::common::{test_app, TestApp, PASSWORD};
use actix_http::Request;
use actix_web::dev::{Service, ServiceResponse};
use serde_json::{json, Value};

pub async fn create_model<S>(app: &mut TestApp<S>, name: &str) -> i64
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let (_, body) = app
        .call(
            "POST",
            "/api/models",
            Some(json!({"name": name, "provider": "openai", "upstream_model": name})),
        )
        .await;
    body["id"].as_i64().unwrap()
}

pub async fn create_team<S>(app: &mut TestApp<S>, name: &str, models: &[i64]) -> i64
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let (status, body) = app
        .call(
            "POST",
            "/api/teams",
            Some(json!({"name": name, "models": models, "all_models": models.is_empty()})),
        )
        .await;
    assert_eq!(status, 201, "{body}");
    body["id"].as_i64().unwrap()
}

async fn relogin<S>(app: &mut TestApp<S>, email: &str)
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    app.logout_locally();
    let (status, _) = app
        .call(
            "POST",
            "/auth/login",
            Some(json!({"email": email, "password": PASSWORD})),
        )
        .await;
    assert_eq!(status, 200);
}

#[actix_web::test]
async fn creating_a_key_shows_it_once_and_stores_only_its_hash() {
    let mut app = test_app().await;
    app.admin().await;
    let team = create_team(&mut app, "Acme", &[]).await;

    let (status, created) = app
        .call(
            "POST",
            "/api/keys",
            Some(json!({"team_id": team, "name": "backend", "max_budget_usd": 20})),
        )
        .await;
    assert_eq!(status, 201, "{created}");
    let raw = created["key"].as_str().unwrap().to_string();
    assert!(raw.starts_with("sk-"));
    assert_eq!(
        created["key_hint"],
        format!("sk-...{}", &raw[raw.len() - 4..])
    );
    assert_eq!(created["team_name"], "Acme");
    assert_eq!(created["max_budget_usd"], 20.0);
    assert_eq!(created["created_by_email"], "admin@example.com");

    let stored: Vec<String> = sqlx::query_scalar("SELECT key_hash FROM api_keys")
        .fetch_all(app.pool())
        .await
        .unwrap();
    assert_eq!(stored.len(), 1);
    assert_ne!(stored[0], raw);

    let (_, list) = app.call("GET", "/api/keys", None).await;
    assert_eq!(list["data"].as_array().unwrap().len(), 1);
    assert!(
        list["data"][0].get("key").is_none(),
        "the raw key is never listed"
    );
}

#[actix_web::test]
async fn a_key_can_only_allow_models_its_team_allows() {
    let mut app = test_app().await;
    app.admin().await;
    let gpt = create_model(&mut app, "gpt-4o").await;
    let claude = create_model(&mut app, "claude").await;
    let team = create_team(&mut app, "Acme", &[gpt]).await;

    let (status, body) = app
        .call(
            "POST",
            "/api/keys",
            Some(json!({"team_id": team, "name": "k", "models": [claude]})),
        )
        .await;
    assert_eq!(status, 400);
    assert_eq!(body["error"]["fields"][0]["field"], "models");

    let (status, body) = app
        .call(
            "POST",
            "/api/keys",
            Some(json!({"team_id": team, "name": "k", "models": [gpt]})),
        )
        .await;
    assert_eq!(status, 201);
    assert_eq!(body["models"][0]["name"], "gpt-4o");
}

#[actix_web::test]
async fn revoking_a_key_hides_it_from_the_list() {
    let mut app = test_app().await;
    app.admin().await;
    let team = create_team(&mut app, "Acme", &[]).await;
    let (_, key) = app
        .call(
            "POST",
            "/api/keys",
            Some(json!({"team_id": team, "name": "k"})),
        )
        .await;

    assert_eq!(
        app.call("DELETE", &format!("/api/keys/{}", key["id"]), None)
            .await
            .0,
        204
    );
    let (_, list) = app.call("GET", "/api/keys", None).await;
    assert!(list["data"].as_array().unwrap().is_empty());
    assert_eq!(
        app.call("DELETE", &format!("/api/keys/{}", key["id"]), None)
            .await
            .0,
        404
    );
}

#[actix_web::test]
async fn updating_a_key_changes_its_limits() {
    let mut app = test_app().await;
    app.admin().await;
    let team = create_team(&mut app, "Acme", &[]).await;
    let (_, key) = app
        .call(
            "POST",
            "/api/keys",
            Some(json!({"team_id": team, "name": "k", "max_budget_usd": 1})),
        )
        .await;

    let (status, updated) = app
        .call(
            "PATCH",
            &format!("/api/keys/{}", key["id"]),
            Some(json!({"name": "renamed", "max_budget_usd": null, "expires_at": "2099-01-01T00:00:00Z"})),
        )
        .await;
    assert_eq!(status, 200, "{updated}");
    assert_eq!(updated["name"], "renamed");
    assert_eq!(updated["max_budget_usd"], Value::Null);
    assert!(updated["expires_at"]
        .as_str()
        .unwrap()
        .starts_with("2099-01-01"));
}

#[actix_web::test]
async fn an_expiry_in_the_past_is_refused() {
    let mut app = test_app().await;
    app.admin().await;
    let team = create_team(&mut app, "Acme", &[]).await;

    let (status, body) = app
        .call(
            "POST",
            "/api/keys",
            Some(json!({"team_id": team, "name": "k", "expires_at": "2000-01-01T00:00:00Z"})),
        )
        .await;
    assert_eq!(status, 400);
    assert_eq!(body["error"]["fields"][0]["field"], "expires_at");
}

#[actix_web::test]
async fn members_create_keys_in_their_teams_and_manage_only_their_own() {
    let mut app = test_app().await;
    app.admin().await;
    app.user("bob@example.com", "member").await;
    app.user("carol@example.com", "member").await;
    let acme = create_team(&mut app, "Acme", &[]).await;
    let other = create_team(&mut app, "Other", &[]).await;
    for email in ["bob@example.com", "carol@example.com"] {
        app.call(
            "POST",
            &format!("/api/teams/{acme}/members"),
            Some(json!({"email": email})),
        )
        .await;
    }
    let (_, admins_key) = app
        .call(
            "POST",
            "/api/keys",
            Some(json!({"team_id": other, "name": "hidden"})),
        )
        .await;

    relogin(&mut app, "bob@example.com").await;
    let (status, bobs_key) = app
        .call(
            "POST",
            "/api/keys",
            Some(json!({"team_id": acme, "name": "bob"})),
        )
        .await;
    assert_eq!(status, 201);
    let (status, _) = app
        .call(
            "POST",
            "/api/keys",
            Some(json!({"team_id": other, "name": "nope"})),
        )
        .await;
    assert_eq!(status, 404);
    let (_, list) = app.call("GET", "/api/keys", None).await;
    assert_eq!(
        list["data"].as_array().unwrap().len(),
        1,
        "only Acme's keys are visible"
    );
    assert_eq!(
        app.call("DELETE", &format!("/api/keys/{}", admins_key["id"]), None)
            .await
            .0,
        404
    );

    relogin(&mut app, "carol@example.com").await;
    assert_eq!(
        app.call("DELETE", &format!("/api/keys/{}", bobs_key["id"]), None)
            .await
            .0,
        403,
        "a plain member cannot revoke a teammate's key"
    );
}

#[actix_web::test]
async fn the_list_filters_by_team() {
    let mut app = test_app().await;
    app.admin().await;
    let acme = create_team(&mut app, "Acme", &[]).await;
    let other = create_team(&mut app, "Other", &[]).await;
    for team in [acme, other] {
        app.call(
            "POST",
            "/api/keys",
            Some(json!({"team_id": team, "name": "k"})),
        )
        .await;
    }

    let (_, list) = app
        .call("GET", &format!("/api/keys?team_id={acme}"), None)
        .await;
    assert_eq!(list["data"].as_array().unwrap().len(), 1);
    assert_eq!(list["data"][0]["team_id"], acme);
}

async fn create_keys<S>(app: &mut TestApp<S>, team: i64, names: &[&str]) -> Vec<i64>
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let mut ids = Vec::new();
    for name in names {
        let (status, body) = app
            .call(
                "POST",
                "/api/keys",
                Some(json!({"team_id": team, "name": name})),
            )
            .await;
        assert_eq!(status, 201, "{body}");
        ids.push(body["id"].as_i64().unwrap());
    }
    ids
}

fn names(list: &Value) -> Vec<&str> {
    list["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k["name"].as_str().unwrap())
        .collect()
}

#[actix_web::test]
async fn the_list_is_paged_with_a_total() {
    let mut app = test_app().await;
    app.admin().await;
    let team = create_team(&mut app, "Acme", &[]).await;
    create_keys(&mut app, team, &["a", "b", "c"]).await;

    let (status, first) = app
        .call("GET", "/api/keys?per_page=2&sort=name", None)
        .await;
    assert_eq!(status, 200, "{first}");
    assert_eq!(names(&first), ["a", "b"]);
    assert_eq!(first["total"], 3);
    assert_eq!(first["page"], 1);
    assert_eq!(first["per_page"], 2);

    let (_, second) = app
        .call("GET", "/api/keys?per_page=2&page=2&sort=name", None)
        .await;
    assert_eq!(names(&second), ["c"]);
    assert_eq!(second["total"], 3);
}

#[actix_web::test]
async fn the_list_sorts_by_a_listed_field_only() {
    let mut app = test_app().await;
    app.admin().await;
    let team = create_team(&mut app, "Acme", &[]).await;
    create_keys(&mut app, team, &["b", "a", "c"]).await;

    let (_, asc) = app.call("GET", "/api/keys?sort=name", None).await;
    assert_eq!(names(&asc), ["a", "b", "c"]);
    let (_, desc) = app.call("GET", "/api/keys?sort=-name", None).await;
    assert_eq!(names(&desc), ["c", "b", "a"]);
    let (_, newest) = app.call("GET", "/api/keys", None).await;
    assert_eq!(names(&newest), ["c", "a", "b"], "newest first by default");

    let (status, body) = app.call("GET", "/api/keys?sort=key_hash", None).await;
    assert_eq!(status, 400, "{body}");
}

#[actix_web::test]
async fn the_list_searches_names_and_owners_ignoring_case() {
    let mut app = test_app().await;
    app.admin().await;
    let team = create_team(&mut app, "Acme", &[]).await;
    create_keys(&mut app, team, &["Prod API", "staging", "100%_off"]).await;

    let (_, found) = app.call("GET", "/api/keys?q=prod", None).await;
    assert_eq!(names(&found), ["Prod API"]);
    assert_eq!(found["total"], 1);

    let (_, literal) = app.call("GET", "/api/keys?q=%25_", None).await;
    assert_eq!(names(&literal), ["100%_off"], "% and _ match themselves");
}

#[actix_web::test]
async fn the_list_filters_by_status() {
    let mut app = test_app().await;
    app.admin().await;
    let team = create_team(&mut app, "Acme", &[]).await;
    let ids = create_keys(&mut app, team, &["live", "stopped"]).await;
    app.call("POST", &format!("/api/keys/{}/block", ids[1]), None)
        .await;

    let (_, blocked) = app.call("GET", "/api/keys?status=blocked", None).await;
    assert_eq!(names(&blocked), ["stopped"]);
    let (_, active) = app.call("GET", "/api/keys?status=active", None).await;
    assert_eq!(names(&active), ["live"]);
    let (status, _) = app.call("GET", "/api/keys?status=nope", None).await;
    assert_eq!(status, 400);
}
