//! Shared test harness: an isolated database per test, the real app wiring
//! on top of it, and a scripted fake provider to proxy to.

#![allow(dead_code)]

pub mod upstream;

use actix_http::Request;
use actix_web::dev::{Service, ServiceResponse};
use actix_web::{test, web, App};
use llmtrack::app::{self, AppState};
use llmtrack::db::DbPool;
use llmtrack::services::users;
use serde_json::Value;

pub const SECRET: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
pub const PASSWORD: &str = "correct-horse";

#[cfg(feature = "sqlite")]
pub struct TestDb {
    pub pool: DbPool,
    _dir: tempfile::TempDir,
}

#[cfg(feature = "sqlite")]
impl TestDb {
    /// Migrates one template per test binary, then copies it per test: every
    /// test gets its own database without paying for migrations each time.
    pub async fn new() -> Self {
        use std::sync::Arc;
        static TEMPLATE: tokio::sync::OnceCell<Arc<tempfile::TempDir>> =
            tokio::sync::OnceCell::const_new();
        let template = TEMPLATE
            .get_or_init(|| async {
                let dir = tempfile::tempdir().unwrap();
                let url = format!("sqlite://{}", dir.path().join("t.db").display());
                let pool = llmtrack::db::create_pool(&url, 1).await.unwrap();
                llmtrack::db::run_migrations(&pool).await.unwrap();
                pool.close().await;
                Arc::new(dir)
            })
            .await;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.db");
        std::fs::copy(template.path().join("t.db"), &path).unwrap();
        let pool = llmtrack::db::create_pool(&format!("sqlite://{}", path.display()), 1)
            .await
            .unwrap();
        TestDb { pool, _dir: dir }
    }
}

#[cfg(feature = "postgres")]
pub struct TestDb {
    pub pool: DbPool,
    _container: testcontainers::ContainerAsync<testcontainers_modules::postgres::Postgres>,
}

#[cfg(feature = "postgres")]
impl TestDb {
    pub async fn new() -> Self {
        use testcontainers::{runners::AsyncRunner, ImageExt};
        let container = testcontainers_modules::postgres::Postgres::default()
            .with_tag("16-alpine")
            .start()
            .await
            .expect("Docker must be running for the postgres suite");
        let url = format!(
            "postgres://postgres:postgres@{}:{}/postgres",
            container.get_host().await.unwrap(),
            container.get_host_port_ipv4(5432).await.unwrap()
        );
        let pool = llmtrack::db::create_pool(&url, 5).await.unwrap();
        llmtrack::db::run_migrations(&pool).await.unwrap();
        TestDb {
            pool,
            _container: container,
        }
    }
}

/// The application under test plus the database behind it. `cookie` holds the
/// session of whoever logged in last.
pub struct TestApp<S> {
    pub service: S,
    pub state: web::Data<AppState>,
    pub db: TestDb,
    pub cookie: Option<String>,
}

pub async fn test_app(
) -> TestApp<impl Service<Request, Response = ServiceResponse, Error = actix_web::Error>> {
    test_app_with(None).await
}

/// The app with the master key set.
pub async fn test_app_with(
    master_key: Option<&str>,
) -> TestApp<impl Service<Request, Response = ServiceResponse, Error = actix_web::Error>> {
    let db = TestDb::new().await;
    let state = web::Data::new(
        AppState::new(
            db.pool.clone(),
            SECRET.as_bytes(),
            std::time::Duration::from_secs(5),
        )
        .with_master_key(master_key.map(String::from)),
    );
    let key = actix_web::cookie::Key::from(SECRET.as_bytes());
    let service = test::init_service(
        App::new()
            .app_data(state.clone())
            .wrap(app::session_middleware(key, false))
            .default_service(web::to(llmtrack::routes::not_found))
            .configure(app::configure),
    )
    .await;
    TestApp {
        service,
        state,
        db,
        cookie: None,
    }
}

impl<S> TestApp<S>
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    pub fn pool(&self) -> &DbPool {
        &self.db.pool
    }

    /// Creates a user straight in the database.
    pub async fn user(&self, email: &str, role: &str) -> i64 {
        users::create(self.pool(), email, PASSWORD, role)
            .await
            .unwrap()
            .id
    }

    /// Creates a user and keeps their session cookie for later requests.
    pub async fn login_as(&mut self, email: &str, role: &str) -> i64 {
        let id = self.user(email, role).await;
        let (status, _) = self
            .call(
                "POST",
                "/auth/login",
                Some(serde_json::json!({"email": email, "password": PASSWORD})),
            )
            .await;
        assert_eq!(status, 200, "login as {email} failed");
        id
    }

    pub async fn admin(&mut self) -> i64 {
        self.login_as("admin@example.com", "admin").await
    }

    /// Sends a request with the stored session cookie and returns status and
    /// JSON body (Null when the body is empty or not JSON). A Set-Cookie on the
    /// response replaces the stored cookie.
    pub async fn call(&mut self, method: &str, uri: &str, body: Option<Value>) -> (u16, Value) {
        self.call_with(method, uri, body, &[]).await
    }

    pub async fn call_with(
        &mut self,
        method: &str,
        uri: &str,
        body: Option<Value>,
        headers: &[(&str, &str)],
    ) -> (u16, Value) {
        let (status, _, body) = self.call_full(method, uri, body, headers).await;
        (status, body)
    }

    /// Like [`Self::call_with`], also returning the response headers.
    pub async fn call_full(
        &mut self,
        method: &str,
        uri: &str,
        body: Option<Value>,
        headers: &[(&str, &str)],
    ) -> (u16, actix_web::http::header::HeaderMap, Value) {
        let mut req = test::TestRequest::default()
            .method(method.parse().unwrap())
            .uri(uri);
        if let Some(cookie) = &self.cookie {
            req = req.insert_header(("Cookie", cookie.clone()));
        }
        for (name, value) in headers {
            req = req.insert_header((*name, *value));
        }
        if let Some(body) = body {
            req = req.set_json(body);
        }
        let response = test::call_service(&self.service, req.to_request()).await;
        if let Some(set_cookie) = response.headers().get("set-cookie") {
            let pair = set_cookie
                .to_str()
                .unwrap()
                .split(';')
                .next()
                .unwrap()
                .to_string();
            self.cookie = Some(pair);
        }
        let status = response.status().as_u16();
        let response_headers = response.headers().clone();
        let bytes = test::read_body(response).await;
        (
            status,
            response_headers,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    /// A GET with the session cookie, returning status, headers and raw bytes
    /// (for file downloads).
    pub async fn call_bytes(
        &mut self,
        uri: &str,
        headers: &[(&str, &str)],
    ) -> (u16, actix_web::http::header::HeaderMap, Vec<u8>) {
        let mut req = test::TestRequest::get().uri(uri);
        if let Some(cookie) = &self.cookie {
            req = req.insert_header(("Cookie", cookie.clone()));
        }
        for (name, value) in headers {
            req = req.insert_header((*name, *value));
        }
        let response = test::call_service(&self.service, req.to_request()).await;
        let status = response.status().as_u16();
        let response_headers = response.headers().clone();
        let bytes = test::read_body(response).await;
        (status, response_headers, bytes.to_vec())
    }

    /// Sends a raw request (for streaming bodies) and returns status and text.
    pub async fn call_text(
        &mut self,
        uri: &str,
        body: Value,
        headers: &[(&str, &str)],
    ) -> (u16, String) {
        let mut req = test::TestRequest::post().uri(uri).set_json(body);
        if let Some(cookie) = &self.cookie {
            req = req.insert_header(("Cookie", cookie.clone()));
        }
        for (name, value) in headers {
            req = req.insert_header((*name, *value));
        }
        let response = test::call_service(&self.service, req.to_request()).await;
        let status = response.status().as_u16();
        let bytes = test::read_body(response).await;
        (status, String::from_utf8(bytes.to_vec()).unwrap())
    }

    pub fn logout_locally(&mut self) {
        self.cookie = None;
    }
}
