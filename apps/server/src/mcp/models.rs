use actix_web::web;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_router};
use serde::{Deserialize, Serialize};

use super::{args, reply, ListArgs, LlmtrackMcp, NoArgs};
use crate::routes::models;

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ListModelsArgs {
    #[serde(flatten)]
    list: ListArgs,
    /// Only this provider's (see list_providers)
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<String>,
    /// `active` or `inactive`
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct CreateModelArgs {
    /// Public name applications call it by; several models may share one
    name: String,
    /// Provider id (see list_providers)
    provider: String,
    /// The provider's own model name
    upstream_model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    api_base: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    api_version: Option<String>,
    /// The provider API key; stored encrypted, never returned
    #[serde(skip_serializing_if = "Option::is_none")]
    api_key: Option<String>,
    /// Price-list entry; matched by provider and model when absent
    #[serde(skip_serializing_if = "Option::is_none")]
    catalog_key: Option<String>,
    /// Own rates in USD per million tokens, e.g. `{"input": 2.5, "output": 10}`
    #[serde(skip_serializing_if = "Option::is_none")]
    pricing: Option<serde_json::Value>,
    /// `catalog`, `custom` or `free`
    #[serde(skip_serializing_if = "Option::is_none")]
    pricing_mode: Option<String>,
    /// Share of traffic among models with the same name
    #[serde(skip_serializing_if = "Option::is_none")]
    weight: Option<i64>,
}

#[tool_router(router = model_tools, vis = "pub(super)")]
impl LlmtrackMcp {
    #[tool(
        description = "List the providers a model can use.",
        annotations(read_only_hint = true)
    )]
    async fn list_providers(&self, Parameters(_): Parameters<NoArgs>) -> CallToolResult {
        reply(async { Ok(models::list_providers(self.current()).await) }).await
    }

    #[tool(
        description = "List the models (deployments) the gateway routes to, with their prices.",
        annotations(read_only_hint = true)
    )]
    async fn list_models(&self, Parameters(a): Parameters<ListModelsArgs>) -> CallToolResult {
        reply(async {
            models::list(
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
        description = "Add a model the gateway can route to. Admins only.",
        annotations(destructive_hint = false)
    )]
    async fn create_model(&self, Parameters(a): Parameters<CreateModelArgs>) -> CallToolResult {
        reply(async { models::create(self.state(), self.admin()?, web::Json(args(&a)?)).await })
            .await
    }
}
