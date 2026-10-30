use actix_web::web;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_router};
use serde::{Deserialize, Serialize};

use super::{args, reply, ListArgs, LlmtrackMcp};
use crate::routes::teams;

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ListTeamsArgs {
    #[serde(flatten)]
    list: ListArgs,
    /// `all`: teams that may call every model; `restricted`: the others
    #[serde(skip_serializing_if = "Option::is_none")]
    models: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct TeamArgs {
    /// Team ID
    team_id: i64,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct CreateTeamArgs {
    name: String,
    /// Budget in USD per period (none: unlimited)
    #[serde(skip_serializing_if = "Option::is_none")]
    max_budget_usd: Option<f64>,
    /// Budget period: `30s`, `1h`, `1d`, `7d` or `1mo` (none: never resets)
    #[serde(skip_serializing_if = "Option::is_none")]
    budget_duration: Option<String>,
    /// Requests per minute
    #[serde(skip_serializing_if = "Option::is_none")]
    rpm_limit: Option<i64>,
    /// Tokens per minute
    #[serde(skip_serializing_if = "Option::is_none")]
    tpm_limit: Option<i64>,
    /// Requests in flight at once
    #[serde(skip_serializing_if = "Option::is_none")]
    max_parallel_requests: Option<i64>,
    /// May call every model
    #[serde(default)]
    all_models: bool,
    /// Model IDs the team may call, when not `all_models`
    #[serde(default)]
    models: Vec<i64>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct AddMemberArgs {
    team_id: i64,
    /// Email of an existing dashboard user
    email: String,
    /// `admin` (manages the team) or `member` (default)
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
}

#[tool_router(router = team_tools, vis = "pub(super)")]
impl LlmtrackMcp {
    #[tool(
        description = "List the teams you can see, with their spend, budget and limits.",
        annotations(read_only_hint = true)
    )]
    async fn list_teams(&self, Parameters(a): Parameters<ListTeamsArgs>) -> CallToolResult {
        reply(async {
            teams::list(
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
        description = "Get one team: its members, models, budget and limits.",
        annotations(read_only_hint = true)
    )]
    async fn get_team(&self, Parameters(a): Parameters<TeamArgs>) -> CallToolResult {
        reply(teams::get(
            self.state(),
            self.current(),
            web::Path::from(a.team_id),
        ))
        .await
    }

    #[tool(
        description = "Create a team. Admins only.",
        annotations(destructive_hint = false)
    )]
    async fn create_team(&self, Parameters(a): Parameters<CreateTeamArgs>) -> CallToolResult {
        reply(async { teams::create(self.state(), self.admin()?, web::Json(args(&a)?)).await })
            .await
    }

    #[tool(
        description = "Add a dashboard user to a team. Team admins and global admins.",
        annotations(destructive_hint = false)
    )]
    async fn add_team_member(&self, Parameters(a): Parameters<AddMemberArgs>) -> CallToolResult {
        reply(async {
            teams::add_member(
                self.state(),
                self.current(),
                web::Path::from(a.team_id),
                web::Json(args(&a)?),
            )
            .await
        })
        .await
    }
}
