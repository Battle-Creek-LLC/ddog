use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::client::Client;
use crate::error::Result;

/// v2 "query timeseries data across multiple products". `from`/`to`/`interval`
/// are epoch **milliseconds**. Replaces the legacy v1 `/api/v1/query`, which
/// scoped application keys are not authorized for.
pub const TIMESERIES_PATH: &str = "api/v2/query/timeseries";

// ---- Request --------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct TimeseriesRequest {
    pub data: RequestData,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequestData {
    pub r#type: String,
    pub attributes: RequestAttributes,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequestAttributes {
    pub from: i64,
    pub to: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<i64>,
    pub queries: Vec<MetricQuery>,
    pub formulas: Vec<Formula>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricQuery {
    pub data_source: String,
    pub query: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Formula {
    pub formula: String,
}

// ---- Response -------------------------------------------------------------

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TimeseriesResponse {
    #[serde(default)]
    pub data: Option<ResponseData>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ResponseData {
    #[serde(default)]
    pub attributes: ResponseAttributes,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ResponseAttributes {
    /// One entry per returned series, in the same order as `values`.
    #[serde(default)]
    pub series: Vec<SeriesMeta>,
    /// Shared bucket timestamps (epoch milliseconds) for every series.
    #[serde(default)]
    pub times: Vec<i64>,
    /// `values[i][j]` is series `i` at `times[j]`; `null` where there's no data.
    #[serde(default)]
    pub values: Vec<Vec<Option<f64>>>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SeriesMeta {
    /// Tag values for the grouping, e.g. `["feed:positions"]`.
    #[serde(default)]
    pub group_tags: Vec<String>,
    #[serde(default)]
    pub query_index: Option<i64>,
    #[serde(default)]
    pub unit: Value,
}

impl Client {
    /// Query timeseries data for a single metric query string.
    ///
    /// `from_ms`/`to_ms`/`interval_ms` are epoch / duration **milliseconds**.
    /// `interval_ms` is optional; when `None`, Datadog picks a rollup (or the
    /// query's own `.rollup(...)` applies).
    pub async fn metrics_timeseries(
        &self,
        from_ms: i64,
        to_ms: i64,
        interval_ms: Option<i64>,
        query: &str,
    ) -> Result<TimeseriesResponse> {
        let req = TimeseriesRequest {
            data: RequestData {
                r#type: "timeseries_request".into(),
                attributes: RequestAttributes {
                    from: from_ms,
                    to: to_ms,
                    interval: interval_ms,
                    queries: vec![MetricQuery {
                        data_source: "metrics".into(),
                        query: query.into(),
                        name: "q1".into(),
                    }],
                    formulas: vec![Formula {
                        formula: "q1".into(),
                    }],
                },
            },
        };
        self.post_json(TIMESERIES_PATH, &req).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_serializes_to_v2_shape() {
        let req = TimeseriesRequest {
            data: RequestData {
                r#type: "timeseries_request".into(),
                attributes: RequestAttributes {
                    from: 1_747_000_000_000,
                    to: 1_747_864_000_000,
                    interval: Some(86_400_000),
                    queries: vec![MetricQuery {
                        data_source: "metrics".into(),
                        query: "sum:bridgeft.import.records{*} by {feed}".into(),
                        name: "q1".into(),
                    }],
                    formulas: vec![Formula {
                        formula: "q1".into(),
                    }],
                },
            },
        };
        let v: Value = serde_json::to_value(&req).unwrap();
        assert_eq!(v["data"]["type"], "timeseries_request");
        assert_eq!(v["data"]["attributes"]["from"], 1_747_000_000_000i64);
        assert_eq!(v["data"]["attributes"]["queries"][0]["data_source"], "metrics");
        assert_eq!(v["data"]["attributes"]["formulas"][0]["formula"], "q1");
    }

    #[test]
    fn parses_timeseries_response() {
        let raw = r#"{
            "data": {
                "type": "timeseries_response",
                "attributes": {
                    "series": [
                        {"group_tags": ["feed:positions"], "query_index": 0},
                        {"group_tags": ["feed:account_balances"], "query_index": 0}
                    ],
                    "times": [1747000000000, 1747086400000, 1747172800000],
                    "values": [
                        [2901, 2950, null],
                        [1483, 1490, 0]
                    ]
                }
            }
        }"#;
        let resp: TimeseriesResponse = serde_json::from_str(raw).unwrap();
        let attrs = resp.data.unwrap().attributes;
        assert_eq!(attrs.series.len(), 2);
        assert_eq!(attrs.series[0].group_tags, vec!["feed:positions"]);
        assert_eq!(attrs.times.len(), 3);
        assert_eq!(attrs.values[0][0], Some(2901.0));
        assert_eq!(attrs.values[0][2], None); // null preserved (≠ zero)
        assert_eq!(attrs.values[1][2], Some(0.0)); // real zero stays zero
    }
}
