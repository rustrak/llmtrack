//! Labels: defined by admins, assigned to keys, and a filter on usage.

use crate::common::{test_app, TestApp};
use crate::integration::keys_test::create_team;
use crate::integration::usage_test::{key, spend};
use actix_http::Request;
use actix_web::dev::{Service, ServiceResponse};
use serde_json::{json, Value};

pub(crate) async fn create_label<S>(app: &mut TestApp<S>, name: &str, color: &str) -> i64
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let (status, body) = app
        .call(
            "POST",
            "/api/labels",
            Some(json!({"name": name, "color": color})),
        )
        .await;
    assert_eq!(status, 201, "{body}");
    body["id"].as_i64().unwrap()
}

fn names(page: &Value) -> Vec<&str> {
    page["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["name"].as_str().unwrap())
        .collect()
}

#[actix_web::test]
async fn an_admin_creates_lists_updates_and_deletes_labels() {
    let mut app = test_app().await;
    app.admin().await;

    let (status, created) = app
        .call(
            "POST",
            "/api/labels",
            Some(json!({"name": " Production ", "color": "green",
                        "description": "Live traffic"})),
        )
        .await;
    assert_eq!(status, 201, "{created}");
    assert_eq!(created["name"], "Production");
    assert_eq!(created["color"], "green");
    assert_eq!(created["description"], "Live traffic");
    assert_eq!(created["key_count"], 0);
    let id = created["id"].as_i64().unwrap();
    create_label(&mut app, "CRM", "blue").await;

    let (status, body) = app
        .call(
            "POST",
            "/api/labels",
            Some(json!({"name": "production", "color": "red"})),
        )
        .await;
    assert_eq!(status, 409, "names are unique whatever their case: {body}");
    assert_eq!(body["error"]["fields"][0]["field"], "name");
    let (status, body) = app
        .call(
            "POST",
            "/api/labels",
            Some(json!({"name": "x", "color": "chartreuse"})),
        )
        .await;
    assert_eq!(status, 400, "{body}");
    assert_eq!(body["error"]["fields"][0]["field"], "color");

    let (_, page) = app.call("GET", "/api/labels", None).await;
    assert_eq!(names(&page), ["CRM", "Production"]);
    let (_, page) = app.call("GET", "/api/labels?q=live", None).await;
    assert_eq!(
        names(&page),
        ["Production"],
        "the description is searched too"
    );

    let (status, updated) = app
        .call(
            "PATCH",
            &format!("/api/labels/{id}"),
            Some(json!({"name": "Prod", "color": "red", "description": null})),
        )
        .await;
    assert_eq!(status, 200, "{updated}");
    assert_eq!(updated["name"], "Prod");
    assert_eq!(updated["color"], "red");
    assert_eq!(updated["description"], Value::Null);

    let (status, _) = app.call("DELETE", &format!("/api/labels/{id}"), None).await;
    assert_eq!(status, 204);
    let (_, page) = app.call("GET", "/api/labels", None).await;
    assert_eq!(names(&page), ["CRM"]);
}

#[actix_web::test]
async fn members_see_labels_but_only_admins_manage_them() {
    let mut app = test_app().await;
    app.admin().await;
    let id = create_label(&mut app, "CRM", "blue").await;

    app.login_as("bob@example.com", "member").await;
    let (status, page) = app.call("GET", "/api/labels", None).await;
    assert_eq!(status, 200);
    assert_eq!(names(&page), ["CRM"]);
    let (status, _) = app
        .call(
            "POST",
            "/api/labels",
            Some(json!({"name": "Mine", "color": "red"})),
        )
        .await;
    assert_eq!(status, 403);
    let (status, _) = app
        .call(
            "PATCH",
            &format!("/api/labels/{id}"),
            Some(json!({"name": "x"})),
        )
        .await;
    assert_eq!(status, 403);
    let (status, _) = app.call("DELETE", &format!("/api/labels/{id}"), None).await;
    assert_eq!(status, 403);
}

#[actix_web::test]
async fn keys_carry_several_labels() {
    let mut app = test_app().await;
    app.admin().await;
    let team = create_team(&mut app, "Acme", &[]).await;
    let prod = create_label(&mut app, "Production", "green").await;
    let crm = create_label(&mut app, "CRM", "blue").await;

    let (status, created) = app
        .call(
            "POST",
            "/api/keys",
            Some(json!({"team_id": team, "name": "crm", "labels": [prod, crm, prod]})),
        )
        .await;
    assert_eq!(status, 201, "{created}");
    assert_eq!(
        created["labels"],
        json!([{"id": crm, "name": "CRM", "color": "blue"},
               {"id": prod, "name": "Production", "color": "green"}])
    );
    let id = created["id"].as_i64().unwrap();
    let (_, page) = app.call("GET", "/api/labels", None).await;
    assert_eq!(page["data"][0]["key_count"], 1);

    let (_, updated) = app
        .call(
            "PATCH",
            &format!("/api/keys/{id}"),
            Some(json!({"labels": [prod]})),
        )
        .await;
    assert_eq!(updated["labels"][0]["id"], prod);
    assert_eq!(updated["labels"].as_array().unwrap().len(), 1);
    let (_, untouched) = app
        .call(
            "PATCH",
            &format!("/api/keys/{id}"),
            Some(json!({"name": "crm-2"})),
        )
        .await;
    assert_eq!(
        untouched["labels"].as_array().unwrap().len(),
        1,
        "kept when absent"
    );

    let (status, body) = app
        .call(
            "PATCH",
            &format!("/api/keys/{id}"),
            Some(json!({"labels": [9999]})),
        )
        .await;
    assert_eq!(status, 400, "{body}");
    assert_eq!(body["error"]["fields"][0]["field"], "labels");

    app.call("DELETE", &format!("/api/labels/{prod}"), None)
        .await;
    let (_, listed) = app.call("GET", "/api/keys", None).await;
    assert_eq!(
        listed["data"][0]["labels"],
        json!([]),
        "a deleted label leaves its keys"
    );
}

#[actix_web::test]
async fn usage_filters_by_the_labels_keys_carry_now() {
    let mut app = test_app().await;
    app.admin().await;
    let team = create_team(&mut app, "Acme", &[]).await;
    let crm = key(&mut app, team, "crm").await;
    let other = key(&mut app, team, "other").await;
    spend(&app, team, crm, "gpt-4o", 3_000_000_000, 2, 200).await;
    spend(&app, team, crm, "claude", 1_000_000_000, 0, 200).await;
    spend(&app, team, other, "gpt-4o", 5_000_000_000, 0, 200).await;

    // Labelled after the spend: the filter still counts it.
    let label = create_label(&mut app, "CRM", "blue").await;
    app.call(
        "PATCH",
        &format!("/api/keys/{crm}"),
        Some(json!({"labels": [label]})),
    )
    .await;

    let (status, usage) = app
        .call("GET", &format!("/api/usage?label_id={label}"), None)
        .await;
    assert_eq!(status, 200, "{usage}");
    assert_eq!(usage["totals"]["requests"], 2);
    assert_eq!(usage["totals"]["cost_usd"], 4.0);
    assert_eq!(usage["by_key"].as_array().unwrap().len(), 1);
    assert_eq!(usage["by_model"].as_array().unwrap().len(), 2);

    let (_, both) = app
        .call(
            "GET",
            &format!("/api/usage?label_id={label}&model=claude"),
            None,
        )
        .await;
    assert_eq!(both["totals"]["requests"], 1);

    let (status, _) = app
        .call(
            "GET",
            &format!("/api/usage/export?format=xlsx&label_id={label}&sections=summary,requests"),
            None,
        )
        .await;
    assert_eq!(status, 200);
}
