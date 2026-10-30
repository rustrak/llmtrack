use actix_web::web;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_router};
use serde::{Deserialize, Serialize};

use super::{args, reply, ListArgs, LlmtrackMcp};
use crate::routes::keys;

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ListKeysArgs {
    #[serde(flatten)]
    list: ListArgs,
    /// Only this team's keys
    #[serde(skip_serializing_if = "Option::is_none")]
    team_id: Option<i64>,
    /// `active`, `blocked` or `expired`
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct KeyArgs {
    /// Key ID
    key_id: i64,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct CreateKeyArgs {
    name: String,
    /// The team it bills to; none makes a personal key of yours
    #[serde(skip_serializing_if = "Option::is_none")]
    team_id: Option<i64>,
    /// One of the team's people who uses it
    #[serde(skip_serializing_if = "Option::is_none")]
    person_id: Option<i64>,
    /// Model IDs it may call (none: all the team may)
    #[serde(default)]
    models: Vec<i64>,
    /// Label IDs
    #[serde(default)]
    labels: Vec<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_budget_usd: Option<f64>,
    /// `30s`, `1h`, `1d`, `7d` or `1mo`
    #[serde(skip_serializing_if = "Option::is_none")]
    budget_duration: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rpm_limit: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tpm_limit: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_parallel_requests: Option<i64>,
    /// RFC 3339 time it stops working
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_at: Option<String>,
    /// Keep each request's prompt and reply (searchable later)
    #[serde(default)]
    log_bodies: bool,
    /// Days to keep them (none: until deleted)
    #[serde(skip_serializing_if = "Option::is_none")]
    body_retention_days: Option<i64>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BlockKeyArgs {
    key_id: i64,
    /// `true` blocks it, `false` lets it work again
    blocked: bool,
}

#[tool_router(router = key_tools, vis = "pub(super)")]
impl LlmtrackMcp {
    #[tool(
        description = "List the virtual keys you can see, with their spend, limits and status. Secrets are never shown.",
        annotations(read_only_hint = true)
    )]
    async fn list_keys(&self, Parameters(a): Parameters<ListKeysArgs>) -> CallToolResult {
        reply(async {
            keys::list(
                self.state(),
                self.current(),
                web::Query(args(&a)?),
                web::Query(args(&a)?),
            )
            .await
        })
        .await
    }

    #[tool(
        description = "Get one virtual key: owner, team, models, labels, limits and spend.",
        annotations(read_only_hint = true)
    )]
    async fn get_key(&self, Parameters(a): Parameters<KeyArgs>) -> CallToolResult {
        reply(keys::detail(
            self.state(),
            self.current(),
            web::Path::from(a.key_id),
        ))
        .await
    }

    #[tool(
        description = "Create a virtual key. The secret (`key`) is returned ONCE: hand it to the user, it cannot be read again.",
        annotations(destructive_hint = false)
    )]
    async fn create_key(&self, Parameters(a): Parameters<CreateKeyArgs>) -> CallToolResult {
        reply(async { keys::create(self.state(), self.current(), web::Json(args(&a)?)).await })
            .await
    }

    #[tool(
        description = "Block a key (its requests are refused) or unblock it. Reversible.",
        annotations(destructive_hint = false, idempotent_hint = true)
    )]
    async fn set_key_blocked(&self, Parameters(a): Parameters<BlockKeyArgs>) -> CallToolResult {
        reply(keys::set_blocked(
            self.state(),
            self.current(),
            web::Path::from(a.key_id),
            a.blocked,
        ))
        .await
    }
}
