use crate::common::{test_app, TestApp, PASSWORD};
use crate::integration::keys_test::create_team;
use actix_http::Request;
use actix_web::dev::{Service, ServiceResponse};
use chrono::{Duration, Utc};
use serde_json::json;

/// Writes one request straight into the log and the rollup, `days_ago` days
/// back, the way the usage writer would.
pub(crate) async fn spend<S>(
    app: &TestApp<S>,
    team_id: i64,
    key_id: i64,
    model: &str,
    cost_nanos: i64,
    days_ago: i64,
    status: i32,
) {
    let at = Utc::now() - Duration::days(days_ago);
    sqlx::query(
        "INSERT INTO request_logs (request_id, team_id, key_id, model_name, provider, status_code,
             prompt_tokens, completion_tokens, cost_nanos, latency_ms, stream, error, created_at)
         VALUES ($1, $2, $3, $4, 'openai', $5, 10, 20, $6, 100, FALSE, NULL, $7)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(team_id)
    .bind(key_id)
    .bind(model)
    .bind(status)
    .bind(cost_nanos)
    .bind(at)
    .execute(&app.db.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO usage_daily (day, team_id, key_id, model_name, requests, failed_requests,
             prompt_tokens, completion_tokens, cost_nanos, latency_ms)
         VALUES ($1, $2, $3, $4, 1, $5, 10, 20, $6, 100)
         ON CONFLICT (day, key_id, model_name) DO UPDATE SET
             requests = usage_daily.requests + 1,
             failed_requests = usage_daily.failed_requests + excluded.failed_requests,
             prompt_tokens = usage_daily.prompt_tokens + 10,
             completion_tokens = usage_daily.completion_tokens + 20,
             cost_nanos = usage_daily.cost_nanos + excluded.cost_nanos,
             latency_ms = usage_daily.latency_ms + 100",
    )
    .bind(at.format("%Y-%m-%d").to_string())
    .bind(team_id)
    .bind(key_id)
    .bind(model)
    .bind(i64::from(status >= 400))
    .bind(cost_nanos)
    .execute(&app.db.pool)
    .await
    .unwrap();
}

pub(crate) async fn key<S>(app: &mut TestApp<S>, team_id: i64, name: &str) -> i64
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let (_, key) = app
        .call(
            "POST",
            "/api/keys",
            Some(json!({"team_id": team_id, "name": name})),
        )
        .await;
    key["id"].as_i64().unwrap()
}

#[actix_web::test]
async fn usage_totals_and_breakdowns_for_an_admin() {
    let mut app = test_app().await;
    app.admin().await;
    let acme = create_team(&mut app, "Acme", &[]).await;
    let globex = create_team(&mut app, "Globex", &[]).await;
    let acme_key = key(&mut app, acme, "acme-backend").await;
    let globex_key = key(&mut app, globex, "globex-app").await;
    spend(&app, acme, acme_key, "gpt-4o", 3_000_000_000, 0, 200).await;
    spend(&app, acme, acme_key, "claude", 1_000_000_000, 1, 200).await;
    spend(&app, globex, globex_key, "gpt-4o", 500_000_000, 0, 500).await;
    spend(&app, globex, globex_key, "gpt-4o", 9_000_000_000, 60, 200).await; // outside 30 days

    let (status, usage) = app.call("GET", "/api/usage", None).await;
    assert_eq!(status, 200, "{usage}");
    assert_eq!(usage["totals"]["cost_usd"], 4.5);
    assert_eq!(usage["totals"]["requests"], 3);
    assert_eq!(usage["totals"]["failed_requests"], 1);
    assert_eq!(usage["totals"]["prompt_tokens"], 30);
    assert_eq!(usage["totals"]["cached_tokens"], 0);
    assert_eq!(
        usage["daily"].as_array().unwrap().len(),
        30,
        "every day of the range, zeros included"
    );
    assert_eq!(
        usage["daily"].as_array().unwrap().last().unwrap()["cost_usd"],
        3.5
    );

    assert_eq!(usage["by_team"][0]["team_name"], "Acme");
    assert_eq!(usage["by_team"][0]["cost_usd"], 4.0);
    assert_eq!(usage["by_team"][1]["team_name"], "Globex");
    assert_eq!(usage["by_key"][0]["key_name"], "acme-backend");
    assert!(usage["by_key"][0]["key_hint"]
        .as_str()
        .unwrap()
        .starts_with("sk-..."));
    assert_eq!(usage["by_model"][0]["model_name"], "gpt-4o");
    assert_eq!(usage["by_model"][0]["requests"], 2);
}

#[actix_web::test]
async fn usage_filters_by_team_and_date_range() {
    let mut app = test_app().await;
    app.admin().await;
    let acme = create_team(&mut app, "Acme", &[]).await;
    let globex = create_team(&mut app, "Globex", &[]).await;
    let acme_key = key(&mut app, acme, "a").await;
    let globex_key = key(&mut app, globex, "g").await;
    spend(&app, acme, acme_key, "gpt-4o", 1_000_000_000, 0, 200).await;
    spend(&app, acme, acme_key, "gpt-4o", 2_000_000_000, 10, 200).await;
    spend(&app, globex, globex_key, "gpt-4o", 5_000_000_000, 0, 200).await;

    let (_, usage) = app
        .call("GET", &format!("/api/usage?team_id={acme}"), None)
        .await;
    assert_eq!(usage["totals"]["cost_usd"], 3.0);

    let from = (Utc::now() - Duration::days(2)).format("%Y-%m-%d");
    let to = Utc::now().format("%Y-%m-%d");
    let (_, usage) = app
        .call(
            "GET",
            &format!("/api/usage?team_id={acme}&from={from}&to={to}"),
            None,
        )
        .await;
    assert_eq!(usage["totals"]["cost_usd"], 1.0);
    assert_eq!(usage["daily"].as_array().unwrap().len(), 3);

    let (status, _) = app
        .call("GET", &format!("/api/usage?from={to}&to={from}"), None)
        .await;
    assert_eq!(status, 400, "from after to");
}

#[actix_web::test]
async fn members_only_see_usage_of_their_teams() {
    let mut app = test_app().await;
    app.admin().await;
    app.user("bob@example.com", "member").await;
    let acme = create_team(&mut app, "Acme", &[]).await;
    let globex = create_team(&mut app, "Globex", &[]).await;
    app.call(
        "POST",
        &format!("/api/teams/{acme}/members"),
        Some(json!({"email": "bob@example.com"})),
    )
    .await;
    let acme_key = key(&mut app, acme, "a").await;
    let globex_key = key(&mut app, globex, "g").await;
    spend(&app, acme, acme_key, "gpt-4o", 1_000_000_000, 0, 200).await;
    spend(&app, globex, globex_key, "gpt-4o", 5_000_000_000, 0, 200).await;

    app.logout_locally();
    app.call(
        "POST",
        "/auth/login",
        Some(json!({"email": "bob@example.com", "password": PASSWORD})),
    )
    .await;

    let (_, usage) = app.call("GET", "/api/usage", None).await;
    assert_eq!(usage["totals"]["cost_usd"], 1.0);
    assert_eq!(usage["by_team"].as_array().unwrap().len(), 1);
    assert_eq!(
        app.call("GET", &format!("/api/usage?team_id={globex}"), None)
            .await
            .0,
        404
    );

    let (_, logs) = app.call("GET", "/api/logs", None).await;
    assert_eq!(logs["data"].as_array().unwrap().len(), 1);
    assert_eq!(
        app.call("GET", &format!("/api/logs?team_id={globex}"), None)
            .await
            .0,
        404
    );
}

#[actix_web::test]
async fn logs_page_newest_first_and_filter() {
    let mut app = test_app().await;
    app.admin().await;
    let acme = create_team(&mut app, "Acme", &[]).await;
    let acme_key = key(&mut app, acme, "backend").await;
    for cost in 1..=5 {
        spend(&app, acme, acme_key, "gpt-4o", cost, 0, 200).await;
    }
    spend(&app, acme, acme_key, "claude", 99, 0, 500).await;

    let (status, page) = app.call("GET", "/api/logs?per_page=2", None).await;
    assert_eq!(status, 200, "{page}");
    let rows = page["data"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["model_name"], "claude");
    assert_eq!(rows[0]["key_name"], "backend");
    assert_eq!(rows[0]["team_name"], "Acme");
    assert_eq!(rows[0]["total_tokens"], 30);
    assert_eq!(rows[0]["endpoint"], "chat/completions");
    assert_eq!(rows[0]["cached_tokens"], 0);
    assert_eq!(page["total"], 6);

    let (_, next) = app.call("GET", "/api/logs?per_page=2&page=2", None).await;
    assert_eq!(next["data"][0]["cost_usd"], 4e-9);

    let (_, errors) = app.call("GET", "/api/logs?status=error", None).await;
    assert_eq!(errors["data"].as_array().unwrap().len(), 1);
    let (_, by_model) = app.call("GET", "/api/logs?model=gpt-4o", None).await;
    assert_eq!(by_model["data"].as_array().unwrap().len(), 5);
    assert_eq!(by_model["total"], 5);
}

#[actix_web::test]
async fn logs_sort_and_search() {
    let mut app = test_app().await;
    app.admin().await;
    let acme = create_team(&mut app, "Acme", &[]).await;
    let acme_key = key(&mut app, acme, "backend").await;
    for cost in [3, 1, 2] {
        spend(&app, acme, acme_key, "gpt-4o", cost, 0, 200).await;
    }
    spend(&app, acme, acme_key, "claude-sonnet", 9, 0, 200).await;

    let (_, cheap) = app.call("GET", "/api/logs?sort=cost", None).await;
    assert_eq!(cheap["data"][0]["cost_usd"], 1e-9);
    let (_, dear) = app.call("GET", "/api/logs?sort=-cost", None).await;
    assert_eq!(dear["data"][0]["model_name"], "claude-sonnet");
    let (_, found) = app.call("GET", "/api/logs?q=CLAUDE", None).await;
    assert_eq!(found["total"], 1);
    assert_eq!(app.call("GET", "/api/logs?sort=error", None).await.0, 400);
}

#[actix_web::test]
async fn the_report_compares_groups_and_times() {
    let mut app = test_app().await;
    app.admin().await;
    let acme = create_team(&mut app, "Acme", &[]).await;
    let k = key(&mut app, acme, "backend").await;
    spend(&app, acme, k, "gpt-4o", 5, 0, 200).await;
    spend(&app, acme, k, "claude", 3, 1, 500).await;
    spend(&app, acme, k, "gpt-4o", 7, 2, 200).await;
    let today = Utc::now().date_naive();
    let window = format!("from={}&to={today}", today - Duration::days(1));

    let (status, report) = app.call("GET", &format!("/api/usage?{window}"), None).await;
    assert_eq!(status, 200, "{report}");
    assert_eq!(report["totals"]["cost_usd"], 8e-9);
    assert_eq!(
        report["totals"]["latency_ms"], 200,
        "summed, for an average"
    );
    assert_eq!(
        report["previous"]["cost_usd"], 7e-9,
        "the equal window just before"
    );

    let series = report["series"].as_array().unwrap();
    assert_eq!(series.len(), 2, "one point per day and model: {report}");
    assert!(series
        .iter()
        .any(|p| p["key"] == "claude" && p["label"] == "claude" && p["requests"] == 1));

    let (_, by_team) = app
        .call("GET", &format!("/api/usage?{window}&group_by=team"), None)
        .await;
    let point = &by_team["series"][0];
    assert_eq!(point["key"], acme.to_string());
    assert_eq!(point["label"], "Acme");

    let (_, claude) = app
        .call("GET", &format!("/api/usage?{window}&model=claude"), None)
        .await;
    assert_eq!(claude["totals"]["cost_usd"], 3e-9);
    assert_eq!(claude["totals"]["failed_requests"], 1);
    assert_eq!(claude["by_model"].as_array().unwrap().len(), 1);

    let (status, _) = app
        .call("GET", &format!("/api/usage?{window}&group_by=secret"), None)
        .await;
    assert_eq!(status, 400);
}

#[actix_web::test]
async fn the_report_splits_billing_and_filters_by_person() {
    let mut app = test_app().await;
    let admin = app.admin().await;
    let acme = create_team(&mut app, "Acme", &[]).await;
    let k = key(&mut app, acme, "backend").await;
    let (_, lucia) = app
        .call(
            "POST",
            "/api/people",
            Some(json!({"name": "Lucía", "team_id": acme})),
        )
        .await;
    let lucia = lucia["id"].as_i64().unwrap();
    let (_, own) = app
        .call("POST", "/api/keys", Some(json!({"name": "mine"})))
        .await;
    let own = own["id"].as_i64().unwrap();
    let today = Utc::now().format("%Y-%m-%d").to_string();
    for (team, key_id, person, user, cost) in [
        (Some(acme), k, Some(lucia), None, 5_i64),
        (Some(acme), k, None, None, 3),
        (None, own, None, Some(admin), 2),
    ] {
        sqlx::query(
            "INSERT INTO usage_daily (day, team_id, key_id, model_name, requests, cost_nanos,
                 person_id, user_id)
             VALUES ($1, $2, $3, $4, 1, $5, $6, $7)",
        )
        .bind(&today)
        .bind(team)
        .bind(key_id)
        .bind(format!("m{cost}"))
        .bind(cost)
        .bind(person)
        .bind(user)
        .execute(app.pool())
        .await
        .unwrap();
    }

    let (_, billing) = app.call("GET", "/api/usage?group_by=team", None).await;
    let keys: Vec<&str> = billing["series"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["key"].as_str().unwrap())
        .collect();
    assert!(keys.contains(&acme.to_string().as_str()), "{billing}");
    assert!(
        keys.contains(&format!("user:{admin}").as_str()),
        "personal keys bill to their user: {billing}"
    );

    let (status, theirs) = app
        .call(
            "GET",
            &format!("/api/usage?team_id={acme}&person_id={lucia}"),
            None,
        )
        .await;
    assert_eq!(status, 200, "{theirs}");
    assert_eq!(theirs["totals"]["cost_usd"], 5e-9);
}
