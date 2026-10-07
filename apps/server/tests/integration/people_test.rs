//! People: whoever spends through a team's keys without signing in to the
//! dashboard. Created on their own, then placed in at most one team, whose
//! spend they break down.

use crate::common::upstream::Upstream;
use crate::common::{test_app, TestApp, PASSWORD};
use crate::integration::keys_test::create_team;
use actix_http::Request;
use actix_web::dev::{Service, ServiceResponse};
use serde_json::{json, Value};

async fn person<S>(app: &mut TestApp<S>, team: Option<i64>, name: &str) -> i64
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let (status, body) = app
        .call(
            "POST",
            "/api/people",
            Some(json!({"name": name, "team_id": team})),
        )
        .await;
    assert_eq!(status, 201, "{body}");
    body["id"].as_i64().unwrap()
}

async fn login_as_member<S>(app: &mut TestApp<S>, team: i64, role: &str)
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    app.user("bob@example.com", "member").await;
    app.call(
        "POST",
        &format!("/api/teams/{team}/members"),
        Some(json!({"email": "bob@example.com", "role": role})),
    )
    .await;
    app.logout_locally();
    app.call(
        "POST",
        "/auth/login",
        Some(json!({"email": "bob@example.com", "password": PASSWORD})),
    )
    .await;
}

#[actix_web::test]
async fn people_are_created_alone_then_placed_in_a_team() {
    let mut app = test_app().await;
    app.admin().await;
    let team = create_team(&mut app, "Marketec360", &[]).await;
    let other = create_team(&mut app, "Acme", &[]).await;

    let (status, lucia) = app
        .call(
            "POST",
            "/api/people",
            Some(json!({"name": " Lucía ", "email": "lucia@marketec360.com"})),
        )
        .await;
    assert_eq!(status, 201, "{lucia}");
    assert_eq!(lucia["name"], "Lucía");
    assert_eq!(lucia["email"], "lucia@marketec360.com");
    assert_eq!(lucia["team_id"], Value::Null);
    let lucia_id = lucia["id"].as_i64().unwrap();

    let (_, unassigned) = app.call("GET", "/api/people?unassigned=true", None).await;
    assert_eq!(unassigned["total"], 1);

    let (status, placed) = app
        .call(
            "PATCH",
            &format!("/api/people/{lucia_id}"),
            Some(json!({"team_id": team, "name": "Lucía G."})),
        )
        .await;
    assert_eq!(status, 200, "{placed}");
    assert_eq!(placed["team_name"], "Marketec360");
    assert_eq!(placed["name"], "Lucía G.");

    person(&mut app, Some(team), "Pedro").await;
    let (status, body) = app
        .call(
            "PATCH",
            &format!("/api/people/{lucia_id}"),
            Some(json!({"name": "Pedro"})),
        )
        .await;
    assert_eq!(status, 400, "a name is unique within its team: {body}");
    assert_eq!(body["error"]["fields"][0]["field"], "name");

    let (status, key) = app
        .call(
            "POST",
            "/api/keys",
            Some(json!({"name": "lucia", "team_id": team, "person_id": lucia_id})),
        )
        .await;
    assert_eq!(status, 201, "{key}");
    assert_eq!(key["person_id"], lucia_id);
    assert_eq!(key["person_name"], "Lucía G.");
    let key_id = key["id"].as_i64().unwrap();

    for body in [
        json!({"name": "x", "team_id": other, "person_id": lucia_id}),
        json!({"name": "x", "person_id": lucia_id}),
    ] {
        let (status, error) = app.call("POST", "/api/keys", Some(body)).await;
        assert_eq!(status, 400, "only a key of the person's own team: {error}");
        assert_eq!(error["error"]["fields"][0]["field"], "person_id");
    }

    let (_, detail) = app.call("GET", &format!("/api/teams/{team}"), None).await;
    assert_eq!(detail["people"][0]["name"], "Lucía G.");
    assert_eq!(detail["people"][0]["key_count"], 1);

    // Leaving the team leaves their keys with the team, assigned to no one.
    let (status, left) = app
        .call(
            "PATCH",
            &format!("/api/people/{lucia_id}"),
            Some(json!({"team_id": null})),
        )
        .await;
    assert_eq!(status, 200, "{left}");
    assert_eq!(left["team_id"], Value::Null);
    let (_, key) = app.call("GET", &format!("/api/keys/{key_id}"), None).await;
    assert_eq!(key["person_id"], Value::Null);

    let (status, _) = app
        .call("DELETE", &format!("/api/people/{lucia_id}"), None)
        .await;
    assert_eq!(status, 204);
    let (_, all) = app.call("GET", "/api/people", None).await;
    assert_eq!(all["total"], 1, "only Pedro is left");
}

#[actix_web::test]
async fn team_admins_manage_their_people_and_members_only_see_them() {
    let mut app = test_app().await;
    app.admin().await;
    let team = create_team(&mut app, "Marketec360", &[]).await;
    let acme = create_team(&mut app, "Acme", &[]).await;
    let lucia = person(&mut app, Some(team), "Lucía").await;
    let ana = person(&mut app, Some(acme), "Ana").await;
    let loose = person(&mut app, None, "Suelta").await;

    login_as_member(&mut app, team, "member").await;
    let (_, mine) = app.call("GET", "/api/people", None).await;
    assert_eq!(mine["total"], 1, "only their team's people: {mine}");
    let (status, _) = app
        .call(
            "POST",
            "/api/people",
            Some(json!({"name": "Pedro", "team_id": team})),
        )
        .await;
    assert_eq!(status, 403);
    let (status, _) = app
        .call("DELETE", &format!("/api/people/{lucia}"), None)
        .await;
    assert_eq!(status, 403);

    // A team admin adds to their team, but never touches another team's
    // people or anyone without a team.
    sqlx::query("UPDATE team_members SET role = 'admin'")
        .execute(&app.db.pool)
        .await
        .unwrap();
    let (status, body) = app
        .call(
            "POST",
            "/api/people",
            Some(json!({"name": "Pedro", "team_id": team})),
        )
        .await;
    assert_eq!(status, 201, "{body}");
    for (id, change) in [
        (ana, json!({"name": "x"})),
        (loose, json!({"team_id": team})),
        (lucia, json!({"team_id": acme})),
    ] {
        let (status, body) = app
            .call("PATCH", &format!("/api/people/{id}"), Some(change))
            .await;
        assert!(matches!(status, 403 | 404), "{status} {body}");
    }
    let (status, _) = app
        .call("POST", "/api/people", Some(json!({"name": "Sin equipo"})))
        .await;
    assert_eq!(status, 403, "only global admins keep people without a team");
}

#[actix_web::test]
async fn spend_breaks_down_by_person() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    app.admin().await;
    let (status, body) = app
        .call(
            "POST",
            "/api/models",
            Some(json!({"name": "gpt", "provider": "openai_compatible",
                "upstream_model": "gpt", "api_base": upstream.base,
                "pricing": {"input": 2.5, "output": 10.0}})),
        )
        .await;
    assert_eq!(status, 201, "{body}");
    let team = create_team(&mut app, "Marketec360", &[]).await;
    let lucia_id = person(&mut app, Some(team), "Lucía").await;

    for person in [Some(lucia_id), None] {
        let (_, key) = app
            .call(
                "POST",
                "/api/keys",
                Some(json!({"name": format!("{person:?}"), "team_id": team, "person_id": person})),
            )
            .await;
        let auth = format!("Bearer {}", key["key"].as_str().unwrap());
        let (status, body) = app
            .call_with(
                "POST",
                "/v1/chat/completions",
                Some(json!({"model": "gpt", "messages": [{"role": "user", "content": "hi"}]})),
                &[("Authorization", auth.as_str())],
            )
            .await;
        assert_eq!(status, 200, "{body}");
    }
    app.state.gateway.flush().await;

    let (status, usage) = app
        .call("GET", &format!("/api/usage?team_id={team}"), None)
        .await;
    assert_eq!(status, 200, "{usage}");
    let people = usage["by_person"].as_array().unwrap();
    assert_eq!(
        people.len(),
        1,
        "the unassigned key is not a person: {usage}"
    );
    assert_eq!(people[0]["person_id"], lucia_id);
    assert_eq!(people[0]["person_name"], "Lucía");
    assert_eq!(people[0]["team_name"], "Marketec360");
    assert_eq!(people[0]["requests"], 1);
    let cost = people[0]["cost_usd"].as_f64().unwrap();
    assert!(cost > 0.0);
    assert!(usage["totals"]["cost_usd"].as_f64().unwrap() > cost);

    let (_, detail) = app.call("GET", &format!("/api/teams/{team}"), None).await;
    assert_eq!(detail["people"][0]["spend_usd"], cost);

    let (status, grouped) = app
        .call(
            "GET",
            &format!("/api/usage?team_id={team}&group_by=person"),
            None,
        )
        .await;
    assert_eq!(status, 200, "{grouped}");
    let mut keys: Vec<(&str, &Value)> = grouped["series"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (p["key"].as_str().unwrap(), &p["label"]))
        .collect();
    keys.sort_by_key(|(key, _)| *key);
    let lucia_key = lucia_id.to_string();
    assert_eq!(
        keys,
        [("", &Value::Null), (lucia_key.as_str(), &json!("Lucía"))],
        "the unassigned key shows as no one"
    );
}

#[actix_web::test]
async fn the_list_searches_and_filters_by_team() {
    let mut app = test_app().await;
    app.admin().await;
    let marketec = create_team(&mut app, "Marketec360", &[]).await;
    let acme = create_team(&mut app, "Acme", &[]).await;
    for (team, name) in [
        (Some(marketec), "Lucía"),
        (Some(marketec), "Pedro"),
        (Some(acme), "Ana"),
        (None, "Bea"),
    ] {
        person(&mut app, team, name).await;
    }
    let names = |body: &Value| -> Vec<String> {
        body["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                format!(
                    "{} @ {}",
                    p["name"].as_str().unwrap(),
                    p["team_name"].as_str().unwrap_or("-")
                )
            })
            .collect()
    };

    let (status, all) = app.call("GET", "/api/people", None).await;
    assert_eq!(status, 200, "{all}");
    assert_eq!(
        names(&all),
        [
            "Ana @ Acme",
            "Bea @ -",
            "Lucía @ Marketec360",
            "Pedro @ Marketec360"
        ]
    );
    let (_, one_team) = app
        .call(
            "GET",
            &format!("/api/people?team_id={marketec}&q=ped"),
            None,
        )
        .await;
    assert_eq!(names(&one_team), ["Pedro @ Marketec360"]);
}
