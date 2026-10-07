use crate::common::test_app;
use serde_json::{json, Value};

fn gpt4o() -> Value {
    json!({
        "name": "gpt-4o",
        "provider": "openai",
        "upstream_model": "gpt-4o-2024-08-06",
        "api_key": "sk-openai-secret",
        "pricing": {"input": 2.5, "output": 10.0}
    })
}

#[actix_web::test]
async fn an_admin_registers_a_model_and_its_key_never_comes_back() {
    let mut app = test_app().await;
    app.admin().await;

    let (status, model) = app.call("POST", "/api/models", Some(gpt4o())).await;
    assert_eq!(status, 201, "{model}");
    assert_eq!(model["name"], "gpt-4o");
    assert_eq!(model["provider"], "openai");
    assert_eq!(model["api_base"], Value::Null);
    assert_eq!(model["effective_api_base"], "https://api.openai.com/v1");
    assert_eq!(model["has_api_key"], true);
    assert_eq!(model["pricing_source"], "custom");
    assert_eq!(model["pricing"]["input"], 2.5);
    assert_eq!(
        model["catalog_key"], "gpt-4o-2024-08-06",
        "found in the catalog by its provider model"
    );
    assert!(!model.to_string().contains("sk-openai-secret"));

    let stored: String = sqlx::query_scalar("SELECT api_key_encrypted FROM models")
        .fetch_one(app.pool())
        .await
        .unwrap();
    assert!(
        !stored.contains("sk-openai-secret"),
        "the key is encrypted at rest"
    );

    let (_, list) = app.call("GET", "/api/models", None).await;
    assert_eq!(list["data"].as_array().unwrap().len(), 1);
    assert!(!list.to_string().contains("sk-openai-secret"));
}

#[actix_web::test]
async fn one_name_can_have_several_deployments() {
    let mut app = test_app().await;
    app.admin().await;
    let (_, first) = app.call("POST", "/api/models", Some(gpt4o())).await;
    let mut second = gpt4o();
    second["weight"] = json!(3);
    let (status, second) = app.call("POST", "/api/models", Some(second)).await;
    assert_eq!(status, 201, "{second}");
    assert_ne!(first["id"], second["id"]);
    assert_eq!(second["weight"], 3);
    assert_eq!(first["weight"], json!(null));

    let mut bad = gpt4o();
    bad["weight"] = json!(-1);
    let (status, body) = app.call("POST", "/api/models", Some(bad)).await;
    assert_eq!(status, 400);
    assert_eq!(body["error"]["fields"][0]["field"], "weight");
}

#[actix_web::test]
async fn a_custom_provider_needs_an_api_base() {
    let mut app = test_app().await;
    app.admin().await;

    let mut model = gpt4o();
    model["provider"] = json!("openai_compatible");
    let (status, body) = app.call("POST", "/api/models", Some(model.clone())).await;
    assert_eq!(status, 400);
    assert_eq!(body["error"]["fields"][0]["field"], "api_base");

    model["api_base"] = json!("ftp://vllm.internal");
    assert_eq!(
        app.call("POST", "/api/models", Some(model.clone())).await.0,
        400
    );

    model["api_base"] = json!("http://vllm.internal:8000/v1/");
    let (status, created) = app.call("POST", "/api/models", Some(model)).await;
    assert_eq!(status, 201);
    assert_eq!(
        created["effective_api_base"],
        "http://vllm.internal:8000/v1"
    );
}

#[actix_web::test]
async fn unknown_providers_and_bad_names_are_refused() {
    let mut app = test_app().await;
    app.admin().await;

    let mut model = gpt4o();
    model["provider"] = json!("bedrock");
    let (_, body) = app.call("POST", "/api/models", Some(model)).await;
    assert_eq!(body["error"]["fields"][0]["field"], "provider");

    let mut model = gpt4o();
    model["name"] = json!("has spaces");
    let (_, body) = app.call("POST", "/api/models", Some(model)).await;
    assert_eq!(body["error"]["fields"][0]["field"], "name");
}

#[actix_web::test]
async fn updating_keeps_the_api_key_unless_told_otherwise() {
    let mut app = test_app().await;
    app.admin().await;
    let (_, model) = app.call("POST", "/api/models", Some(gpt4o())).await;
    let uri = format!("/api/models/{}", model["id"]);

    let (status, updated) = app
        .call(
            "PATCH",
            &uri,
            Some(json!({"pricing": {"input": 2.5, "output": 12.0}, "is_active": false})),
        )
        .await;
    assert_eq!(status, 200, "{updated}");
    assert_eq!(updated["pricing"]["output"], 12.0);
    assert_eq!(updated["is_active"], false);
    assert_eq!(updated["has_api_key"], true);

    let (_, cleared) = app
        .call("PATCH", &uri, Some(json!({"api_key": null})))
        .await;
    assert_eq!(cleared["has_api_key"], false);

    assert_eq!(app.call("DELETE", &uri, None).await.0, 204);
    assert_eq!(app.call("DELETE", &uri, None).await.0, 404);
}

#[actix_web::test]
async fn members_can_list_models_but_not_change_them() {
    let mut app = test_app().await;
    app.admin().await;
    app.call("POST", "/api/models", Some(gpt4o())).await;

    app.login_as("bob@example.com", "member").await;
    assert_eq!(app.call("GET", "/api/models", None).await.0, 200);
    assert_eq!(app.call("POST", "/api/models", Some(gpt4o())).await.0, 403);
}

#[actix_web::test]
async fn the_provider_catalogue_is_public_to_signed_in_users() {
    let mut app = test_app().await;
    app.login_as("bob@example.com", "member").await;

    let (status, providers) = app.call("GET", "/api/providers", None).await;
    assert_eq!(status, 200);
    assert!(providers
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["id"] == "anthropic" && p["wire"] == "anthropic"));
}

#[actix_web::test]
async fn a_model_without_its_own_rates_is_priced_by_the_catalog() {
    let mut app = test_app().await;
    app.admin().await;

    let (status, model) = app
        .call(
            "POST",
            "/api/models",
            Some(json!({"name": "sonnet", "provider": "anthropic", "upstream_model": "claude-sonnet-4-5"})),
        )
        .await;
    assert_eq!(status, 201, "{model}");
    assert_eq!(model["pricing_source"], "catalog");
    assert_eq!(model["catalog_key"], "claude-sonnet-4-5");
    assert_eq!(model["pricing"]["input"], 3.0);
    assert_eq!(model["pricing"]["cache_read"], 0.3);
    assert_eq!(model["pricing"]["above"]["threshold_tokens"], 200_000);
    assert_eq!(model["custom_pricing"], serde_json::Value::Null);

    // Own rates win; null goes back to the catalog.
    let uri = format!("/api/models/{}", model["id"]);
    let (_, custom) = app
        .call(
            "PATCH",
            &uri,
            Some(json!({"pricing": {"input": 1.0, "output": 2.0, "cache_read": 0.1}})),
        )
        .await;
    assert_eq!(custom["pricing_source"], "custom");
    assert_eq!(custom["pricing"]["cache_read"], 0.1);
    let (_, back) = app
        .call("PATCH", &uri, Some(json!({"pricing": null})))
        .await;
    assert_eq!(back["pricing_source"], "catalog");
}

#[actix_web::test]
async fn a_model_the_catalog_does_not_know_is_unpriced_until_given_a_key_or_rates() {
    let mut app = test_app().await;
    app.admin().await;

    let (_, model) = app
        .call(
            "POST",
            "/api/models",
            Some(json!({"name": "local", "provider": "openai_compatible", "upstream_model": "my-finetune",
                        "api_base": "http://vllm:8000/v1"})),
        )
        .await;
    assert_eq!(model["pricing_source"], "none");
    assert_eq!(model["pricing"], serde_json::Value::Null);

    let uri = format!("/api/models/{}", model["id"]);
    let (status, body) = app
        .call("PATCH", &uri, Some(json!({"catalog_key": "no-such-entry"})))
        .await;
    assert_eq!(status, 400);
    assert_eq!(body["error"]["fields"][0]["field"], "catalog_key");

    let (status, priced) = app
        .call("PATCH", &uri, Some(json!({"catalog_key": "gpt-4o-mini"})))
        .await;
    assert_eq!(status, 200, "{priced}");
    assert_eq!(priced["pricing_source"], "catalog");
}

#[actix_web::test]
async fn a_model_can_have_no_price_on_purpose() {
    let mut app = test_app().await;
    app.admin().await;
    let (_, model) = app
        .call(
            "POST",
            "/api/models",
            Some(json!({"name": "sonnet", "provider": "anthropic", "upstream_model": "claude-sonnet-4-5"})),
        )
        .await;
    assert_eq!(model["pricing_mode"], "catalog");
    assert_eq!(model["pricing_source"], "catalog", "matched automatically");
    let uri = format!("/api/models/{}", model["id"]);

    let (status, free) = app
        .call("PATCH", &uri, Some(json!({"pricing_mode": "free"})))
        .await;
    assert_eq!(status, 200, "{free}");
    assert_eq!(free["pricing_mode"], "free");
    assert_eq!(free["pricing_source"], "free", "not the 'unpriced' warning");
    assert_eq!(free["pricing"], Value::Null);

    let (_, still) = app
        .call(
            "PATCH",
            &uri,
            Some(json!({"upstream_model": "claude-haiku-4-5"})),
        )
        .await;
    assert_eq!(
        still["pricing_source"], "free",
        "a new provider model does not re-price it"
    );

    let (_, back) = app
        .call("PATCH", &uri, Some(json!({"pricing_mode": "catalog"})))
        .await;
    assert_eq!(back["pricing_source"], "catalog");
    assert_eq!(back["catalog_key"], "claude-haiku-4-5");
}

#[actix_web::test]
async fn the_pricing_mode_is_set_at_creation_and_checked() {
    let mut app = test_app().await;
    app.admin().await;
    let (status, model) = app
        .call(
            "POST",
            "/api/models",
            Some(json!({"name": "sonnet", "provider": "anthropic",
                        "upstream_model": "claude-sonnet-4-5", "pricing_mode": "free"})),
        )
        .await;
    assert_eq!(status, 201, "{model}");
    assert_eq!(model["pricing_source"], "free");
    let uri = format!("/api/models/{}", model["id"]);

    let (status, body) = app
        .call("PATCH", &uri, Some(json!({"pricing_mode": "cheap"})))
        .await;
    assert_eq!(status, 400);
    assert_eq!(body["error"]["fields"][0]["field"], "pricing_mode");
    let (status, body) = app
        .call("PATCH", &uri, Some(json!({"pricing_mode": "custom"})))
        .await;
    assert_eq!(status, 400, "own rates need rates: {body}");
    assert_eq!(body["error"]["fields"][0]["field"], "pricing");

    let (_, custom) = app
        .call(
            "PATCH",
            &uri,
            Some(json!({"pricing": {"input": 1.0, "output": 2.0}})),
        )
        .await;
    assert_eq!(
        custom["pricing_mode"], "custom",
        "rates switch to own rates"
    );
    let (_, free) = app
        .call("PATCH", &uri, Some(json!({"pricing_mode": "free"})))
        .await;
    assert_eq!(
        free["custom_pricing"]["input"], 1.0,
        "the rates stay for later"
    );
    let (_, custom) = app
        .call("PATCH", &uri, Some(json!({"pricing_mode": "custom"})))
        .await;
    assert_eq!(custom["pricing"]["input"], 1.0);
}

#[actix_web::test]
async fn a_free_model_bills_nothing() {
    let upstream = crate::common::upstream::Upstream::start().await;
    let mut app = test_app().await;
    app.admin().await;
    let (_, model) = app
        .call(
            "POST",
            "/api/models",
            Some(
                json!({"name": "gpt", "provider": "openai", "upstream_model": "gpt-4o",
                        "api_base": upstream.base, "pricing_mode": "free"}),
            ),
        )
        .await;
    assert_eq!(
        model["catalog_key"], "gpt-4o",
        "the list knows it, but it is free here"
    );
    let (_, key) = app
        .call("POST", "/api/keys", Some(json!({"name": "k"})))
        .await;
    let auth = format!("Bearer {}", key["key"].as_str().unwrap());
    let (status, _) = app
        .call_with(
            "POST",
            "/v1/chat/completions",
            Some(json!({"model": "gpt", "messages": [{"role": "user", "content": "hi"}]})),
            &[("Authorization", &auth)],
        )
        .await;
    assert_eq!(status, 200);
    app.state.gateway.flush().await;
    let (_, logs) = app.call("GET", "/api/logs", None).await;
    assert_eq!(logs["data"][0]["cost_usd"], 0.0);
    assert_eq!(logs["data"][0]["prompt_tokens"], 10, "tokens still count");
}

#[actix_web::test]
async fn bad_rates_name_their_path() {
    let mut app = test_app().await;
    app.admin().await;
    let mut model = gpt4o();
    model["pricing"] = json!({"input": -1.0, "output": 1.0});
    let (status, body) = app.call("POST", "/api/models", Some(model)).await;
    assert_eq!(status, 400);
    assert_eq!(body["error"]["fields"][0]["field"], "pricing.input");
}

#[actix_web::test]
async fn test_connect_calls_the_provider_with_the_unsaved_settings() {
    let upstream = crate::common::upstream::Upstream::start().await;
    let mut app = test_app().await;
    app.admin().await;

    let (status, ok) = app
        .call(
            "POST",
            "/api/models/test",
            Some(
                json!({"provider": "openai_compatible", "upstream_model": "gpt-4o",
                        "api_base": upstream.base, "api_key": "sk-try"}),
            ),
        )
        .await;
    assert_eq!(status, 200, "{ok}");
    assert_eq!(ok["ok"], true);
    assert!(ok["latency_ms"].as_u64().is_some());
    let sent = upstream.last();
    assert_eq!(sent.authorization.as_deref(), Some("Bearer sk-try"));
    assert_eq!(sent.body["max_tokens"], 1);

    let (_, failed) = app
        .call(
            "POST",
            "/api/models/test",
            Some(json!({"provider": "openai_compatible", "upstream_model": "bad-key", "api_base": upstream.base})),
        )
        .await;
    assert_eq!(failed["ok"], false);
    assert_eq!(failed["status"], 401);
    assert!(failed["message"]
        .as_str()
        .unwrap()
        .contains("Incorrect API key"));

    let (_, unreachable) = app
        .call(
            "POST",
            "/api/models/test",
            Some(json!({"provider": "openai_compatible", "upstream_model": "x", "api_base": "http://127.0.0.1:9/v1"})),
        )
        .await;
    assert_eq!(unreachable["ok"], false);
    assert_eq!(unreachable["status"], serde_json::Value::Null);
}

#[actix_web::test]
async fn test_connect_reuses_the_stored_key_of_a_model_being_edited() {
    let upstream = crate::common::upstream::Upstream::start().await;
    let mut app = test_app().await;
    app.admin().await;
    let mut model = gpt4o();
    model["provider"] = json!("openai_compatible");
    model["api_base"] = json!(upstream.base);
    let (_, created) = app.call("POST", "/api/models", Some(model)).await;

    let (_, ok) = app
        .call(
            "POST",
            "/api/models/test",
            Some(
                json!({"model_id": created["id"], "provider": "openai_compatible",
                        "upstream_model": "gpt-4o", "api_base": upstream.base}),
            ),
        )
        .await;
    assert_eq!(ok["ok"], true, "{ok}");
    assert_eq!(
        upstream.last().authorization.as_deref(),
        Some("Bearer sk-openai-secret")
    );
}

#[actix_web::test]
async fn members_cannot_test_connections() {
    let mut app = test_app().await;
    app.login_as("bob@example.com", "member").await;
    let (status, _) = app
        .call(
            "POST",
            "/api/models/test",
            Some(json!({"provider": "openai", "upstream_model": "gpt-4o"})),
        )
        .await;
    assert_eq!(status, 403);
}

#[actix_web::test]
async fn the_model_list_pages_sorts_searches_and_filters() {
    let mut app = test_app().await;
    app.admin().await;
    let mut ids = Vec::new();
    for (name, provider, upstream) in [
        ("gpt-4o", "openai", "gpt-4o"),
        ("sonnet", "anthropic", "claude-sonnet-4-5"),
        ("gpt-4o", "openai", "gpt-4o-2024-11-20"),
    ] {
        let (status, body) = app
            .call(
                "POST",
                "/api/models",
                Some(json!({"name": name, "provider": provider, "upstream_model": upstream})),
            )
            .await;
        assert_eq!(status, 201, "{body}");
        ids.push(body["id"].as_i64().unwrap());
    }
    app.call(
        "PATCH",
        &format!("/api/models/{}", ids[2]),
        Some(json!({"is_active": false})),
    )
    .await;

    let (_, all) = app.call("GET", "/api/models", None).await;
    assert_eq!(names(&all["data"]), ["gpt-4o", "gpt-4o", "sonnet"]);
    assert_eq!(all["data"][0]["deployments"], 2, "a name's deployments");
    assert_eq!(all["data"][2]["deployments"], 1);
    let (_, second) = app.call("GET", "/api/models?per_page=2&page=2", None).await;
    assert_eq!(names(&second["data"]), ["sonnet"]);
    assert_eq!(second["total"], 3);

    let (_, anthropic) = app
        .call("GET", "/api/models?provider=anthropic", None)
        .await;
    assert_eq!(names(&anthropic["data"]), ["sonnet"]);
    let (_, inactive) = app.call("GET", "/api/models?status=inactive", None).await;
    assert_eq!(inactive["data"][0]["id"], ids[2]);
    assert_eq!(inactive["total"], 1);
    let (_, found) = app.call("GET", "/api/models?q=CLAUDE", None).await;
    assert_eq!(
        names(&found["data"]),
        ["sonnet"],
        "matches the upstream model"
    );
    let (_, desc) = app.call("GET", "/api/models?sort=-name", None).await;
    assert_eq!(names(&desc["data"]), ["sonnet", "gpt-4o", "gpt-4o"]);
    assert_eq!(
        app.call("GET", "/api/models?sort=api_key", None).await.0,
        400
    );
}

fn names(list: &Value) -> Vec<&str> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|m| m["name"].as_str().unwrap())
        .collect()
}
