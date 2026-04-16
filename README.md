# `ddog` — Datadog CLI

A Rust CLI for querying Datadog from the terminal and from AI agents.

**Status:** v0.1 — logs commands implemented. Metrics, monitors, events,
and incidents planned for later releases.

## Install

```
cargo install --path crates/dd-cli
```

Or run straight from a checkout:

```
cargo run -p dd-cli -- logs search 'status:error' --from now-1h
```

## Authenticating with Datadog

The CLI needs **two** credentials:

| Credential      | Env var       | What it's for                                         |
| --------------- | ------------- | ----------------------------------------------------- |
| API key         | `DD_API_KEY`  | Identifies the Datadog org. 32-char hex or UUID.      |
| Application key | `DD_APP_KEY`  | Scopes the user's access to query APIs. 40-char hex.  |

An **API key alone is not enough** — every read endpoint (logs search,
metrics query, monitors list…) also requires an Application key.

### Get an API key

1. Log in to Datadog.
2. Navigate to **Organization Settings → API Keys**
   (direct link: `https://app.datadoghq.com/organization-settings/api-keys` —
   swap the subdomain for EU/US3/US5/AP1 as needed).
3. Click **New Key**, name it (e.g. `ddog-cli-<your-name>`), copy the key.
4. **Permissions required:** none beyond org membership.

### Get an Application key

1. Same area of the UI: **Organization Settings → Application Keys**
   (`https://app.datadoghq.com/organization-settings/application-keys`).
2. Click **New Key**. Datadog will scope it to *your user*.
3. **Scopes required for `ddog logs`:**
   - `logs_read_data` (read log events)
   - `logs_read_index_data` (read index-level queries / aggregate)
4. Copy the key — Datadog shows it only once.

### Pick the right site

Your Datadog URL tells you the site:

| You log in at…                     | `DD_SITE` value           |
| ---------------------------------- | ------------------------- |
| `app.datadoghq.com`                | `datadoghq.com` (default) |
| `app.datadoghq.eu`                 | `datadoghq.eu`            |
| `us3.datadoghq.com`                | `us3.datadoghq.com`       |
| `us5.datadoghq.com`                | `us5.datadoghq.com`       |
| `ap1.datadoghq.com`                | `ap1.datadoghq.com`       |
| `app.ddog-gov.com`                 | `ddog-gov.com`            |

### Set your env

```bash
export DD_API_KEY=<your-api-key>
export DD_APP_KEY=<your-application-key>
export DD_SITE=datadoghq.com   # or your region
```

### Or use a config file with profiles

Default path: `~/.config/ddog/config.toml` (override with `--config` or
`DD_CONFIG`). Example (see `docs/config.example.toml` for a copy-paste template):

```toml
default_site = "datadoghq.com"
default_profile = "prod"

[profiles.prod]
api_key = "<api-key-goes-here>"
app_key = "<app-key-goes-here>"
site    = "datadoghq.com"

[profiles.staging]
api_key = "<api-key-goes-here>"
app_key = "<app-key-goes-here>"
site    = "datadoghq.eu"
```

Select a profile with `--profile staging` or `DD_PROFILE=staging`.

Precedence: **CLI flag > environment > profile > default**.

## Usage

```
ddog logs search 'service:api status:error' --from now-1h
ddog logs search 'status:error' --from now-15m -o ndjson         # agent-friendly stream
ddog logs tail   'service:api status:error' --interval 5s
ddog logs get    AAAAxxxxx
ddog logs aggregate 'status:error' --group-by service --measure count --from now-1h
```

### Common flags

| Flag           | Purpose                                                     |
| -------------- | ----------------------------------------------------------- |
| `--from/--to`  | `now`, `now-15m`, `now-1h`, `now-7d`, or RFC-3339.          |
| `-n / --limit` | Page size (1–1000).                                         |
| `--max`        | Stop after N total events (`0` = unlimited).                |
| `--index`      | Comma-separated index names.                                |
| `--fields`     | Attributes to include in text/table output.                 |
| `--storage`    | `indexes` \| `online-archives` \| `flex`.                   |
| `-o`           | `text` \| `json` \| `ndjson` \| `table`.                    |
| `-v` / `-vv`   | Verbose / debug (writes to stderr).                         |

### Output modes

- **text** — default when stdout is a TTY.
- **json** — default when piped; single document, pretty on TTY / compact otherwise.
- **ndjson** — one event per line; best for streaming to agents or `jq`.
- **table** — boxed ASCII table.

### Exit codes

`0` success · `1` usage/parse · `2` auth (401/403) · `3` not found · `4`
rate limited · `5` upstream 5xx · `6` network.

## Architecture

```
crates/
  dd-cli/     # Binary `ddog`. clap, output, TTY detection.
  dd-api/     # HTTP client. Auth, retries, pagination.
  dd-config/  # Env + config file + profile resolution.
```

Full spec: [`docs/SPECIFICATION.md`](docs/SPECIFICATION.md).

## Development

```
cargo build
cargo test
cargo run -p dd-cli -- logs search --help
```

## Roadmap

1. `ddog logs facets` (currently stubbed).
2. `ddog metrics query`.
3. `ddog monitors {list,get,mute}`.
4. `ddog events {list,post}`.
5. `ddog incidents {list,get}`.
6. Shell completions via `ddog completions <shell>`.
