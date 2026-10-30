use actix_web::web;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_router};
use serde::{Deserialize, Serialize};

use super::{args, reply, ListArgs, LlmtrackMcp, NoArgs};
use crate::routes::{invitations, labels, users};

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ListUsersArgs {
    #[serde(flatten)]
    list: ListArgs,
    /// `admin` or `member`
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    /// `active` or `inactive`
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct InviteArgs {
    email: String,
    /// `admin` or `member` (default)
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct CreateLabelArgs {
    name: String,
    /// `gray`, `red`, `orange`, `amber`, `green`, `teal`, `blue`, `violet` or `pink`
    color: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

#[tool_router(router = user_tools, vis = "pub(super)")]
impl LlmtrackMcp {
    #[tool(
        description = "List dashboard users. Admins only.",
        annotations(read_only_hint = true)
    )]
    async fn list_users(&self, Parameters(a): Parameters<ListUsersArgs>) -> CallToolResult {
        reply(async {
            users::list(
                self.state(),
                self.admin()?,
                web::Query(args(&a)?),
                web::Query(args(&a)?),
            )
            .await
        })
        .await
    }

    #[tool(
        description = "List pending invitations. Admins only.",
        annotations(read_only_hint = true)
    )]
    async fn list_invitations(&self, Parameters(_): Parameters<NoArgs>) -> CallToolResult {
        reply(async { invitations::list(self.state(), self.admin()?).await }).await
    }

    #[tool(
        description = "Invite someone to the dashboard (48 h link, no email is sent: give them the link). Admins only.",
        annotations(destructive_hint = false)
    )]
    async fn create_invitation(&self, Parameters(a): Parameters<InviteArgs>) -> CallToolResult {
        reply(async {
            invitations::create(self.state(), self.admin()?, web::Json(args(&a)?)).await
        })
        .await
    }

    #[tool(
        description = "List the labels keys can carry (usage can be filtered by label).",
        annotations(read_only_hint = true)
    )]
    async fn list_labels(&self, Parameters(a): Parameters<ListArgs>) -> CallToolResult {
        reply(async { labels::list(self.state(), self.current(), web::Query(args(&a)?)).await })
            .await
    }

    #[tool(
        description = "Create a label for keys. Admins only.",
        annotations(destructive_hint = false)
    )]
    async fn create_label(&self, Parameters(a): Parameters<CreateLabelArgs>) -> CallToolResult {
        reply(async { labels::create(self.state(), self.admin()?, web::Json(args(&a)?)).await })
            .await
    }
}
