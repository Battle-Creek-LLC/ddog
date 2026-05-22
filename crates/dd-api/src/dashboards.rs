use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::client::Client;
use crate::error::Result;

pub const DASHBOARD_PATH: &str = "api/v1/dashboard";

/// One row of `GET /api/v1/dashboard`'s `dashboards` array.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DashboardSummary {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub author_handle: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DashboardListResponse {
    #[serde(default)]
    pub dashboards: Vec<DashboardSummary>,
}

impl Client {
    /// Fetch a single dashboard definition. Returns the raw JSON so callers can
    /// read widget queries (`widgets[].definition.requests[].q`) without us
    /// modelling Datadog's large, evolving widget schema.
    pub async fn dashboard_get(&self, id: &str) -> Result<Value> {
        self.get_json(&format!("{DASHBOARD_PATH}/{id}"), &[]).await
    }

    /// List all dashboards. The v1 endpoint has no server-side title search, so
    /// title filtering is done client-side by the caller.
    pub async fn dashboards_list(&self) -> Result<DashboardListResponse> {
        self.get_json(DASHBOARD_PATH, &[]).await
    }
}
