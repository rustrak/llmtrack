use actix_web::web;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_router};
use serde::{Deserialize, Serialize};

use super::{args, reply, ListArgs, LlmtrackMcp};
use crate::routes::people;

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ListPeopleArgs {
    #[serde(flatten)]
    list: ListArgs,
    #[serde(skip_serializing_if = "Option::is_none")]
    team_id: Option<i64>,
    /// Only people without a team
    #[serde(default)]
    unassigned: bool,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct CreatePersonArgs {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    team_id: Option<i64>,
}

#[tool_router(router = people_tools, vis = "pub(super)")]
impl LlmtrackMcp {
    #[tool(
        description = "List people: who spends through a team's keys without a dashboard account.",
        annotations(read_only_hint = true)
    )]
    async fn list_people(&self, Parameters(a): Parameters<ListPeopleArgs>) -> CallToolResult {
        reply(async {
            people::list(
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
        description = "Create a person, optionally in a team.",
        annotations(destructive_hint = false)
    )]
    async fn create_person(&self, Parameters(a): Parameters<CreatePersonArgs>) -> CallToolResult {
        reply(async { people::create(self.state(), self.current(), web::Json(args(&a)?)).await })
            .await
    }
}
