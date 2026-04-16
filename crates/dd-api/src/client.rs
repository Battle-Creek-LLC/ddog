use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::de::DeserializeOwned;
use serde::Serialize;
use tracing::{debug, warn};
use url::Url;

use crate::error::{ApiError, Result};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_RETRIES: u32 = 3;

pub struct ClientBuilder {
    api_key: String,
    app_key: String,
    site: String,
    user_agent: String,
    timeout: Duration,
}

impl ClientBuilder {
    pub fn new(api_key: impl Into<String>, app_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            app_key: app_key.into(),
            site: "datadoghq.com".to_string(),
            user_agent: format!("ddog-cli/{}", env!("CARGO_PKG_VERSION")),
            timeout: DEFAULT_TIMEOUT,
        }
    }

    pub fn site(mut self, site: impl Into<String>) -> Self {
        self.site = site.into();
        self
    }

    pub fn user_agent(mut self, ua: impl Into<String>) -> Self {
        self.user_agent = ua.into();
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn build(self) -> Result<Client> {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("dd-api-key"),
            HeaderValue::from_str(&self.api_key)
                .map_err(|_| ApiError::Upstream { status: 0, body: "invalid API key header".into() })?,
        );
        headers.insert(
            HeaderName::from_static("dd-application-key"),
            HeaderValue::from_str(&self.app_key)
                .map_err(|_| ApiError::Upstream { status: 0, body: "invalid App key header".into() })?,
        );

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .user_agent(self.user_agent)
            .timeout(self.timeout)
            .build()?;

        let base = Url::parse(&format!("https://api.{}/", self.site))?;
        Ok(Client { http, base })
    }
}

pub struct Client {
    http: reqwest::Client,
    base: Url,
}

impl Client {
    pub fn base_url(&self) -> &Url {
        &self.base
    }

    fn url(&self, path: &str) -> Result<Url> {
        Ok(self.base.join(path.trim_start_matches('/'))?)
    }

    pub async fn post_json<B, R>(&self, path: &str, body: &B) -> Result<R>
    where
        B: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        let url = self.url(path)?;
        self.execute_with_retry(|| {
            self.http.post(url.clone()).json(body).send()
        })
        .await
    }

    pub async fn get_json<R>(&self, path: &str, query: &[(&str, String)]) -> Result<R>
    where
        R: DeserializeOwned,
    {
        let url = self.url(path)?;
        self.execute_with_retry(|| {
            let mut req = self.http.get(url.clone());
            if !query.is_empty() {
                req = req.query(query);
            }
            req.send()
        })
        .await
    }

    async fn execute_with_retry<F, Fut, R>(&self, mut send: F) -> Result<R>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = std::result::Result<reqwest::Response, reqwest::Error>>,
        R: DeserializeOwned,
    {
        let mut attempt = 0u32;
        loop {
            attempt += 1;
            let res = send().await;
            let resp = match res {
                Ok(r) => r,
                Err(e) => {
                    if attempt < MAX_RETRIES && (e.is_timeout() || e.is_connect()) {
                        let backoff = backoff_for(attempt);
                        warn!(attempt, ?backoff, error=%e, "transient network error; retrying");
                        tokio::time::sleep(backoff).await;
                        continue;
                    }
                    return Err(ApiError::from(e));
                }
            };

            let status = resp.status();
            if status.is_success() {
                let text = resp.text().await?;
                debug!(bytes = text.len(), "response body");
                return Ok(serde_json::from_str::<R>(&text)?);
            }

            let retry_after = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok());

            let code = status.as_u16();
            let body = resp.text().await.unwrap_or_default();

            match code {
                401 | 403 => return Err(ApiError::Auth),
                404 => return Err(ApiError::NotFound(body)),
                429 => {
                    if attempt < MAX_RETRIES {
                        let wait = retry_after.map(Duration::from_secs).unwrap_or(backoff_for(attempt));
                        warn!(attempt, ?wait, "rate limited; retrying");
                        tokio::time::sleep(wait).await;
                        continue;
                    }
                    return Err(ApiError::RateLimited {
                        retry_after_secs: retry_after.unwrap_or(0),
                    });
                }
                500..=599 => {
                    if attempt < MAX_RETRIES {
                        let wait = backoff_for(attempt);
                        warn!(attempt, status=code, ?wait, "upstream 5xx; retrying");
                        tokio::time::sleep(wait).await;
                        continue;
                    }
                    return Err(ApiError::Upstream { status: code, body });
                }
                _ => return Err(ApiError::Upstream { status: code, body }),
            }
        }
    }
}

fn backoff_for(attempt: u32) -> Duration {
    // 500ms, 1s, 2s
    Duration::from_millis(500u64 * 2u64.pow(attempt.saturating_sub(1)))
}
