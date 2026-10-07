use crate::common::upstream::Upstream;
use crate::common::{test_app, PASSWORD};
use serde_json::json;

#[actix_web::test]
async fn admins_read_and_change_the_global_settings() {
    let mut app = test_app().await;
    app.admin().await;

    let (status, settings) = app.call("GET", "/api/settings", None).await;
    assert_eq!(status, 200);
    assert_eq!(settings["public_url"], serde_json::Value::Null);
    assert!(settings["price_catalog_url"]
        .as_str()
        .unwrap()
        .contains("model_prices"));

    let (status, updated) = app
        .call(
            "PATCH",
            "/api/settings",
            Some(json!({"public_url": "https://llm.acme.dev/"})),
        )
        .await;
    assert_eq!(status, 200, "{updated}");
    assert_eq!(updated["public_url"], "https://llm.acme.dev");

    let (_, cleared) = app
        .call("PATCH", "/api/settings", Some(json!({"public_url": null})))
        .await;
    assert_eq!(cleared["public_url"], serde_json::Value::Null);
}

#[actix_web::test]
async fn invalid_settings_name_their_field() {
    let mut app = test_app().await;
    app.admin().await;
    for (field, value) in [
        ("public_url", json!("ftp://nope")),
        ("price_catalog_url", json!("not a url")),
    ] {
        let (status, body) = app
            .call("PATCH", "/api/settings", Some(json!({ field: value })))
            .await;
        assert_eq!(status, 400, "{field}");
        assert_eq!(body["error"]["fields"][0]["field"], field);
    }
}

#[actix_web::test]
async fn members_see_the_public_url_but_cannot_change_settings() {
    let mut app = test_app().await;
    app.admin().await;
    app.call(
        "PATCH",
        "/api/settings",
        Some(json!({"public_url": "https://llm.acme.dev"})),
    )
    .await;
    app.user("bob@example.com", "member").await;
    app.logout_locally();
    app.call(
        "POST",
        "/auth/login",
        Some(json!({"email": "bob@example.com", "password": PASSWORD})),
    )
    .await;

    let (status, settings) = app.call("GET", "/api/settings", None).await;
    assert_eq!(status, 200);
    assert_eq!(settings["public_url"], "https://llm.acme.dev");
    assert!(
        settings.get("price_catalog_url").is_none(),
        "admin-only fields are left out"
    );
    assert_eq!(
        app.call("PATCH", "/api/settings", Some(json!({"public_url": null})))
            .await
            .0,
        403
    );
}

#[actix_web::test]
async fn the_catalog_is_searchable_and_reports_where_it_came_from() {
    let mut app = test_app().await;
    app.admin().await;

    let (status, status_body) = app.call("GET", "/api/catalog/status", None).await;
    assert_eq!(status, 200);
    assert_eq!(status_body["source"], "builtin");
    assert!(status_body["entries"].as_u64().unwrap() > 1000);

    let (status, hits) = app
        .call("GET", "/api/catalog?q=sonnet%204-5&limit=5", None)
        .await;
    assert_eq!(status, 200);
    let hits = hits.as_array().unwrap();
    assert!(!hits.is_empty() && hits.len() <= 5);
    assert!(hits[0]["key"].as_str().unwrap().contains("sonnet"));
    assert!(hits[0]["pricing"]["input"].as_f64().unwrap() > 0.0);
}

#[actix_web::test]
async fn syncing_the_catalog_reprices_catalog_models_and_survives_a_restart() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    app.admin().await;
    let root = upstream.base.trim_end_matches("/v1").to_string();
    app.call(
        "PATCH",
        "/api/settings",
        Some(json!({"price_catalog_url": format!("{root}/catalog.json")})),
    )
    .await;
    let (_, model) = app
        .call(
            "POST",
            "/api/models",
            Some(json!({"name": "4o", "provider": "openai", "upstream_model": "gpt-4o"})),
        )
        .await;
    assert_eq!(model["pricing"]["input"], 2.5);

    let (status, synced) = app.call("POST", "/api/catalog/sync", None).await;
    assert_eq!(status, 200, "{synced}");
    assert_eq!(synced["source"], "synced");
    assert_eq!(synced["entries"], 601);

    let (_, models) = app.call("GET", "/api/models", None).await;
    assert_eq!(
        models["data"][0]["pricing"]["input"], 5.0,
        "repriced from the new list"
    );

    // A new process loads the stored list instead of the built-in one.
    llmtrack::services::catalog::load_stored(&app.state)
        .await
        .unwrap();
    assert_eq!(app.state.gateway.catalog().len(), 601);
}

#[actix_web::test]
async fn a_sync_that_fails_keeps_the_current_catalog() {
    let mut app = test_app().await;
    app.admin().await;
    app.call(
        "PATCH",
        "/api/settings",
        Some(json!({"price_catalog_url": "http://127.0.0.1:9/none.json"})),
    )
    .await;
    let (status, _) = app.call("POST", "/api/catalog/sync", None).await;
    assert_eq!(status, 502);
    let (_, status_body) = app.call("GET", "/api/catalog/status", None).await;
    assert_eq!(status_body["source"], "builtin");
}

#[actix_web::test]
async fn the_catalog_lists_a_providers_models_with_the_id_to_send() {
    let mut app = test_app().await;
    app.admin().await;

    let (status, models) = app
        .call("GET", "/api/catalog?provider=anthropic&limit=500", None)
        .await;
    assert_eq!(status, 200);
    let models = models.as_array().unwrap();
    assert!(models.len() > 10);
    assert!(models.iter().all(|m| m["provider"] == "anthropic"));
    let sonnet = models
        .iter()
        .find(|m| m["key"] == "claude-sonnet-4-5")
        .expect("listed");
    assert_eq!(sonnet["model"], "claude-sonnet-4-5");

    let (_, gemini) = app
        .call("GET", "/api/catalog?provider=gemini&q=2.5-pro", None)
        .await;
    let pro = gemini
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["key"] == "gemini/gemini-2.5-pro")
        .expect("listed");
    assert_eq!(
        pro["model"], "gemini-2.5-pro",
        "the prefix is the list's, not the provider's"
    );

    let (_, custom) = app
        .call("GET", "/api/catalog?provider=openai_compatible", None)
        .await;
    assert_eq!(
        custom.as_array().unwrap().len(),
        0,
        "nothing to list for an unknown server"
    );
}
