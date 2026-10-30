//! Filters that keep several values and leave others out, the same on the
//! usage report, the request log, the exported report and the MCP tools.

use serde_json::{json, Value};

use crate::common::{test_app, tool_output, PUBLIC_URL};
use crate::integration::keys_test::create_team;
use crate::integration::usage_test::{key, spend};

fn requests(report: &Value) -> i64 {
    report["totals"]["requests"].as_i64().unwrap()
}

#[actix_web::test]
async fn a_team_without_one_key_or_only_some_keys() {
    let mut app = test_app().await;
    let admin = app.admin().await;
    let acme = create_team(&mut app, "Acme", &[]).await;
    let globex = create_team(&mut app, "Globex", &[]).await;
    let (a1, a2, a3) = (
        key(&mut app, acme, "a1").await,
        key(&mut app, acme, "a2").await,
        key(&mut app, acme, "a3").await,
    );
    let g1 = key(&mut app, globex, "g1").await;
    // a1: 1 request, a2: 2, a3: 4, g1: 8; models split them.
    spend(&app, acme, a1, "gpt", 1_000, 0, 200).await;
    for _ in 0..2 {
        spend(&app, acme, a2, "claude", 1_000, 0, 200).await;
    }
    for _ in 0..4 {
        spend(&app, acme, a3, "gpt", 1_000, 0, 500).await;
    }
    for _ in 0..8 {
        spend(&app, globex, g1, "gpt", 1_000, 0, 200).await;
    }

    for (query, expected) in [
        (format!("team_id={acme}&exclude_key_id={a3}"), 3),
        (format!("key_id={a1},{a2},{g1}"), 11),
        (format!("key_id={a1}&key_id_unused=1"), 1),
        (format!("exclude_team_id={globex}"), 7),
        (format!("team_id={acme},{globex}&exclude_model=gpt"), 2),
        ("model=gpt,claude&exclude_model=claude".to_string(), 13),
    ] {
        let (status, report) = app.call("GET", &format!("/api/usage?{query}"), None).await;
        assert_eq!(status, 200, "{query}: {report}");
        assert_eq!(requests(&report), expected, "/api/usage?{query}");
        // The previous period, the series and the breakdowns follow suit.
        let by_key: i64 = report["by_key"]
            .as_array()
            .unwrap()
            .iter()
            .map(|k| k["requests"].as_i64().unwrap())
            .sum();
        assert_eq!(by_key, expected, "{query}");

        let (_, logs) = app.call("GET", &format!("/api/logs?{query}"), None).await;
        assert_eq!(logs["total"], expected, "/api/logs?{query}");
    }

    let (_, logs) = app
        .call(
            "GET",
            &format!("/api/logs?team_id={acme}&exclude_key_id={a1}&status=error"),
            None,
        )
        .await;
    assert_eq!(logs["total"], 4);

    let (status, _, _) = app
        .call_bytes(
            &format!("/api/usage/export?format=xlsx&team_id={acme}&exclude_key_id={a3}"),
            &[],
        )
        .await;
    assert_eq!(status, 200);

    // The same through MCP, as lists.
    app.call(
        "PATCH",
        "/api/settings",
        Some(json!({ "public_url": PUBLIC_URL })),
    )
    .await;
    app.call("PUT", "/api/settings/mcp", Some(json!({ "enabled": true })))
        .await;
    let token = app.mcp_token(admin).await;
    let (_, body) = app
        .call_tool(
            Some(&token),
            "get_usage",
            json!({"team_id": [acme], "exclude_key_id": [a3]}),
        )
        .await;
    assert_eq!(requests(&tool_output(&body)), 3, "{body}");
    let (_, body) = app
        .call_tool(
            Some(&token),
            "list_requests",
            json!({"key_id": [a1, a2, a3], "exclude_model": ["claude"]}),
        )
        .await;
    assert_eq!(tool_output(&body)["total"], 5, "{body}");
}

#[actix_web::test]
async fn tags_customers_and_labels_keep_and_leave_out() {
    let mut app = test_app().await;
    app.admin().await;
    let acme = create_team(&mut app, "Acme", &[]).await;
    let (k1, k2) = (
        key(&mut app, acme, "k1").await,
        key(&mut app, acme, "k2").await,
    );
    for _ in 0..3 {
        spend(&app, acme, k1, "gpt", 1_000, 0, 200).await;
    }
    spend(&app, acme, k2, "gpt", 1_000, 0, 200).await;
    // Tag and customer the three requests of k1 apart.
    let ids: Vec<i64> =
        sqlx::query_scalar("SELECT id FROM request_logs WHERE key_id = $1 ORDER BY id")
            .bind(k1)
            .fetch_all(app.pool())
            .await
            .unwrap();
    for (id, tags, customer) in [
        (ids[0], r#"["prod","eu"]"#, "alice"),
        (ids[1], r#"["prod"]"#, "bob"),
        (ids[2], r#"["dev"]"#, "carol"),
    ] {
        sqlx::query("UPDATE request_logs SET tags = $1, end_user = $2 WHERE id = $3")
            .bind(tags)
            .bind(customer)
            .bind(id)
            .execute(app.pool())
            .await
            .unwrap();
    }
    let (_, label) = app
        .call(
            "POST",
            "/api/labels",
            Some(json!({"name": "vip", "color": "blue"})),
        )
        .await;
    let label = label["id"].as_i64().unwrap();
    app.call(
        "PATCH",
        &format!("/api/keys/{k2}"),
        Some(json!({"labels": [label]})),
    )
    .await;

    for (query, expected) in [
        ("tag=prod".to_string(), 2),
        ("tag=eu,dev".to_string(), 2),
        ("exclude_tag=prod".to_string(), 2),
        ("tag=prod&exclude_tag=eu".to_string(), 1),
        ("end_user=alice,carol".to_string(), 2),
        ("exclude_end_user=alice".to_string(), 3),
        (format!("label_id={label}"), 1),
        (format!("exclude_label_id={label}"), 3),
    ] {
        let (status, logs) = app.call("GET", &format!("/api/logs?{query}"), None).await;
        assert_eq!(status, 200, "{logs}");
        assert_eq!(logs["total"], expected, "/api/logs?{query}");
    }
    let (_, report) = app
        .call("GET", &format!("/api/usage?exclude_label_id={label}"), None)
        .await;
    assert_eq!(requests(&report), 3);
}

#[actix_web::test]
async fn members_cannot_name_a_team_they_are_not_in() {
    let mut app = test_app().await;
    app.admin().await;
    let acme = create_team(&mut app, "Acme", &[]).await;
    let globex = create_team(&mut app, "Globex", &[]).await;
    app.user("bob@example.com", "member").await;
    app.call(
        "POST",
        &format!("/api/teams/{acme}/members"),
        Some(json!({"email": "bob@example.com"})),
    )
    .await;
    app.logout_locally();
    app.call(
        "POST",
        "/auth/login",
        Some(json!({"email": "bob@example.com", "password": crate::common::PASSWORD})),
    )
    .await;
    let (status, _) = app
        .call("GET", &format!("/api/usage?team_id={acme},{globex}"), None)
        .await;
    assert_eq!(
        status, 404,
        "every listed team is checked, and one not theirs does not exist"
    );
    let (status, _) = app
        .call("GET", &format!("/api/logs?team_id={acme}"), None)
        .await;
    assert_eq!(status, 200);
}
