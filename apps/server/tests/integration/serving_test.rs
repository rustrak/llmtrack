use crate::common::{test_app, TestDb, SECRET};
use actix_web::{test, web, App};
use llmtrack::app::{self, AppState};
use llmtrack::routes::dashboard::Dashboard;
use llmtrack::services::users;

#[actix_web::test]
async fn liveness_and_readiness() {
    let mut app = test_app().await;
    let (status, body) = app.call("GET", "/health", None).await;
    assert_eq!(status, 200);
    assert_eq!(body["status"], "ok");
    let (status, body) = app.call("GET", "/health/ready", None).await;
    assert_eq!(status, 200);
    assert_eq!(body["database"], "ok");
}

#[actix_web::test]
async fn the_dashboard_shell_answers_every_page_but_never_an_api_path() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("index.html"), "<html>llmtrack</html>").unwrap();
    std::fs::create_dir(dir.path().join("assets")).unwrap();
    std::fs::write(dir.path().join("assets/app-1.js"), "console.log(1)").unwrap();
    let dashboard = Dashboard::detect(dir.path()).expect("a build with index.html is detected");

    let db = TestDb::new().await;
    let state = web::Data::new(AppState::new(
        db.pool.clone(),
        SECRET.as_bytes(),
        std::time::Duration::from_secs(5),
    ));
    let service = test::init_service(
        App::new()
            .app_data(state)
            .wrap(app::session_middleware(
                actix_web::cookie::Key::from(SECRET.as_bytes()),
                false,
            ))
            .configure(app::configure)
            .configure(dashboard.configure()),
    )
    .await;

    for page in ["/", "/teams", "/models", "/keys/12"] {
        let response =
            test::call_service(&service, test::TestRequest::get().uri(page).to_request()).await;
        assert_eq!(response.status(), 200, "{page}");
        assert_eq!(
            test::read_body(response).await,
            "<html>llmtrack</html>",
            "{page}"
        );
    }

    let asset = test::call_service(
        &service,
        test::TestRequest::get()
            .uri("/assets/app-1.js")
            .to_request(),
    )
    .await;
    assert_eq!(asset.status(), 200);
    assert!(asset
        .headers()
        .get("cache-control")
        .unwrap()
        .to_str()
        .unwrap()
        .contains("immutable"));

    for missing in [
        "/assets/gone.js",
        "/api/nope",
        "/v1/nope",
        "/auth/nope",
        "/health/nope",
    ] {
        let response =
            test::call_service(&service, test::TestRequest::get().uri(missing).to_request()).await;
        assert_eq!(response.status(), 404, "{missing}");
        assert!(
            response
                .headers()
                .get("content-type")
                .unwrap()
                .to_str()
                .unwrap()
                .contains("json"),
            "{missing} must not answer with the page shell"
        );
    }
}

#[actix_web::test]
async fn no_build_means_no_dashboard() {
    let dir = tempfile::tempdir().unwrap();
    assert!(Dashboard::detect(dir.path()).is_none());
}

#[actix_web::test]
async fn the_superuser_is_created_once_and_only_into_an_empty_database() {
    let db = TestDb::new().await;

    users::ensure_superuser(&db.pool, Some("Root@Example.com:long-enough"))
        .await
        .unwrap();
    users::ensure_superuser(&db.pool, Some("other@example.com:long-enough"))
        .await
        .unwrap();
    users::ensure_superuser(&db.pool, None).await.unwrap();

    let all = users::list(
        &db.pool,
        &Default::default(),
        &users::UserListQuery {
            role: None,
            status: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(all.data.len(), 1);
    assert_eq!(all.data[0].user.email, "root@example.com");
    assert!(all.data[0].user.is_admin());
}

#[actix_web::test]
async fn a_malformed_superuser_spec_is_an_error() {
    let db = TestDb::new().await;
    assert!(users::ensure_superuser(&db.pool, Some("no-colon"))
        .await
        .is_err());
}
