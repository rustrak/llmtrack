//! Shared test harness: an isolated database per test, the real app wiring
//! on top of it, and a scripted fake provider to proxy to.

#![allow(dead_code)]

pub mod upstream;

use actix_http::Request;
use actix_web::dev::{Service, ServiceResponse};
use actix_web::{test, web, App};
use llmtrack::app::{self, AppState};
use llmtrack::db::DbPool;
use llmtrack::error::{AppError, AppResult};
use llmtrack::services::oauth::{
    AuthorizationCode, ClientMetadata, ClientMetadataSource, OAuthService,
};
use llmtrack::services::users;
use serde_json::{json, Value};

pub const SECRET: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
pub const PASSWORD: &str = "correct-horse";

/// The public URL the OAuth tests set, and the resource tokens are bound to.
pub const PUBLIC_URL: &str = "http://localhost:4000";
pub const MCP_URL: &str = "http://localhost:4000/mcp";
/// An OAuth client whose metadata [`FakeClients`] serves.
pub const CLIENT_ID: &str = "https://client.example/oauth/metadata.json";
pub const REDIRECT: &str = "https://client.example/callback";
/// A client whose document registers a script as its redirect.
pub const SCRIPT_CLIENT: &str = "https://evil.example/oauth/metadata.json";

/// Serves the test clients' metadata documents, and refuses every other
/// `client_id`: the real source fetches over HTTPS from the internet.
pub struct FakeClients;

#[async_trait::async_trait]
impl ClientMetadataSource for FakeClients {
    async fn fetch(&self, client_id: &str) -> AppResult<ClientMetadata> {
        let redirect = match client_id {
            CLIENT_ID => REDIRECT,
            SCRIPT_CLIENT => "javascript:alert(document.domain)//",
            _ => return Err(AppError::Validation("unknown client".into())),
        };
        Ok(serde_json::from_value(json!({
            "client_id": client_id,
            "client_name": "Example Assistant",
            "redirect_uris": [redirect],
            "token_endpoint_auth_method": "none"
        }))
        .unwrap())
    }
}

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
        .with_master_key(master_key.map(String::from))
        .with_clients(std::sync::Arc::new(FakeClients)),
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

    /// An MCP access token acting as `user_id`, as if they had approved
    /// [`CLIENT_ID`] (the flow itself is `oauth_test`'s).
    pub async fn mcp_token(&self, user_id: i64) -> String {
        let code = AuthorizationCode {
            user_id,
            client_id: CLIENT_ID.into(),
            client_name: "Example Assistant".into(),
            redirect_uri: REDIRECT.into(),
            resource: MCP_URL.into(),
            code_challenge: String::new(),
            expires_at: chrono::Utc::now(),
        };
        OAuthService::create_grant(self.pool(), &code)
            .await
            .unwrap()
            .access_token
    }

    /// Calls an MCP tool the way a stateless client does, returning the
    /// HTTP status and the JSON-RPC answer.
    pub async fn call_tool(
        &mut self,
        token: Option<&str>,
        name: &str,
        arguments: Value,
    ) -> (u16, Value) {
        let mut req = test::TestRequest::post()
            .uri("/mcp")
            .insert_header(("Content-Type", "application/json"))
            .insert_header(("Accept", "application/json, text/event-stream"))
            .insert_header(("MCP-Protocol-Version", "2026-07-28"))
            .insert_header(("Mcp-Method", "tools/call"))
            .insert_header(("Mcp-Name", name))
            .set_payload(
                rpc(
                    "tools/call",
                    json!({ "name": name, "arguments": arguments }),
                )
                .to_string(),
            );
        if let Some(token) = token {
            req = req.insert_header(("Authorization", format!("Bearer {token}")));
        }
        // The session cookie goes along, as a browser would send it.
        if let Some(cookie) = &self.cookie {
            req = req.insert_header(("Cookie", cookie.clone()));
        }
        let response = test::call_service(&self.service, req.to_request()).await;
        let status = response.status().as_u16();
        let bytes = test::read_body(response).await;
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    pub fn logout_locally(&mut self) {
        self.cookie = None;
    }
}

/// A JSON-RPC request as a `2026-07-28` client sends it: stateless, the
/// protocol version carried in `_meta`.
pub fn rpc(method: &str, mut params: Value) -> Value {
    params["_meta"] = json!({
        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
        "io.modelcontextprotocol/clientInfo": { "name": "test", "version": "1.0" },
        "io.modelcontextprotocol/clientCapabilities": {}
    });
    json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params })
}

/// The JSON a tool printed, out of the JSON-RPC envelope around it.
pub fn tool_output(body: &Value) -> Value {
    let text = body["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("no text content in {body}"));
    serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.into()))
}

/// Whether the tool answered with an error, and its text.
pub fn tool_error(body: &Value) -> Option<String> {
    (body["result"]["isError"] == true).then(|| {
        body["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    })
}
