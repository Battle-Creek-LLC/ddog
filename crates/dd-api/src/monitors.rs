use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::client::Client;
use crate::error::Result;

pub const MONITOR_PATH: &str = "api/v1/monitor";

/// A monitor as returned by `GET /api/v1/monitor` and `/monitor/{id}`.
///
/// Only the fields we render are typed; the rest of the (large) payload is kept
/// in `extra` so `-o json` stays lossless.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Monitor {
    pub id: i64,
    #[serde(default)]
    pub name: Option<String>,
    /// `OK` | `Alert` | `Warn` | `No Data` | `Skipped` | `Ignored` | …
    #[serde(default)]
    pub overall_state: Option<String>,
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(flatten)]
    pub extra: Value,
}

/// Filters for `monitors_list`, each mapping to a `GET /api/v1/monitor` query
/// param.
#[derive(Debug, Clone, Default)]
pub struct MonitorListFilter {
    /// Substring match on the monitor name (`name`).
    pub name: Option<String>,
    /// Scope tags, e.g. `service:nodejs-worker` (`tags`).
    pub tags: Option<String>,
    /// Tags set on the monitor object itself (`monitor_tags`).
    pub monitor_tags: Option<String>,
}

impl Client {
    pub async fn monitors_list(&self, filter: &MonitorListFilter) -> Result<Vec<Monitor>> {
        let mut query: Vec<(&str, String)> = Vec::new();
        if let Some(name) = &filter.name {
            query.push(("name", name.clone()));
        }
        if let Some(tags) = &filter.tags {
            query.push(("tags", tags.clone()));
        }
        if let Some(monitor_tags) = &filter.monitor_tags {
            query.push(("monitor_tags", monitor_tags.clone()));
        }
        self.get_json(MONITOR_PATH, &query).await
    }

    pub async fn monitor_get(&self, id: i64) -> Result<Monitor> {
        self.get_json(&format!("{MONITOR_PATH}/{id}"), &[]).await
    }
}
