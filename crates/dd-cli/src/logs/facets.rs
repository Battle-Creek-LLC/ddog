use clap::Args;

use crate::cli::Ctx;

#[derive(Debug, Args)]
pub struct FacetsArgs {
    /// Optional prefix filter.
    pub prefix: Option<String>,
}

pub async fn run(_ctx: Ctx, _args: FacetsArgs) -> anyhow::Result<()> {
    // Datadog does not expose a public `facets` endpoint for logs v2. A proper
    // implementation would need to either scrape a search result for observed
    // attribute keys or read the index configuration via the v1 indexes API.
    // Tracked for the next release; see docs/SPECIFICATION.md roadmap.
    anyhow::bail!("`ddog logs facets` is not yet implemented (planned for v0.2)")
}
