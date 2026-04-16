use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::client::Client;
use crate::error::{ApiError, Result};

pub const SEARCH_PATH: &str = "api/v2/logs/events/search";
pub const AGGREGATE_PATH: &str = "api/v2/logs/analytics/aggregate";

/// Storage tier for log queries.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum StorageTier {
    Indexes,
    OnlineArchives,
    Flex,
}

impl std::str::FromStr for StorageTier {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "indexes" => Ok(Self::Indexes),
            "online-archives" => Ok(Self::OnlineArchives),
            "flex" => Ok(Self::Flex),
            other => Err(format!(
                "invalid storage tier '{other}' (expected indexes|online-archives|flex)"
            )),
        }
    }
}

// ---- Search ---------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Default)]
pub struct SearchRequest {
    pub filter: SearchFilter,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<Page>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<SearchOptions>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct SearchFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indexes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "storage_tier")]
    pub storage_tier: Option<StorageTier>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Page {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct SearchOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchResponse {
    #[serde(default)]
    pub data: Vec<LogEvent>,
    #[serde(default)]
    pub meta: Option<Meta>,
    #[serde(default)]
    pub links: Option<Links>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LogEvent {
    pub id: String,
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    #[serde(default)]
    pub attributes: LogAttributes,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct LogAttributes {
    #[serde(default)]
    pub timestamp: Option<String>,
    #[serde(default)]
    pub service: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub attributes: Value,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct Meta {
    #[serde(default)]
    pub page: Option<MetaPage>,
    #[serde(default)]
    pub elapsed: Option<u64>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub warnings: Vec<Value>,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct MetaPage {
    #[serde(default)]
    pub after: Option<String>,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct Links {
    #[serde(default)]
    pub next: Option<String>,
}

impl Client {
    pub async fn logs_search(&self, req: &SearchRequest) -> Result<SearchResponse> {
        self.post_json(SEARCH_PATH, req).await
    }

    /// Fetch a single event by ID via the search endpoint. Datadog has no
    /// dedicated `GET /events/{id}` for logs v2; an ID filter is the canonical
    /// workaround.
    pub async fn logs_get(&self, id: &str, indexes: Option<Vec<String>>) -> Result<LogEvent> {
        let req = SearchRequest {
            filter: SearchFilter {
                query: Some(format!("@id:{id}")),
                from: Some("now-30d".into()),
                to: Some("now".into()),
                indexes,
                ..Default::default()
            },
            page: Some(Page { limit: Some(1), cursor: None }),
            sort: Some("-timestamp".into()),
            ..Default::default()
        };
        let resp: SearchResponse = self.logs_search(&req).await?;
        resp.data
            .into_iter()
            .next()
            .ok_or_else(|| ApiError::NotFound(format!("no log event with id {id}")))
    }
}

// ---- Aggregate ------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Default)]
pub struct AggregateRequest {
    pub filter: SearchFilter,
    pub compute: Vec<Compute>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub group_by: Vec<GroupBy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<SearchOptions>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Compute {
    pub aggregation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metric: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GroupBy {
    pub facet: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AggregateResponse {
    #[serde(default)]
    pub data: Option<AggregateData>,
    #[serde(default)]
    pub meta: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AggregateData {
    #[serde(default)]
    pub buckets: Vec<Bucket>,
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Bucket {
    #[serde(default)]
    pub by: Value,
    #[serde(default)]
    pub computes: Value,
}

impl Client {
    pub async fn logs_aggregate(&self, req: &AggregateRequest) -> Result<AggregateResponse> {
        self.post_json(AGGREGATE_PATH, req).await
    }
}
