# `ddog` — Datadog CLI

A Rust CLI for querying Datadog from the terminal and from AI agents.

Designed to be readable by humans at a TTY **and** machine-parseable when
piped: the same command emits a colorized table in your shell and compact
NDJSON into a script, with no flags required.

**Status:** v0.1 — `logs` commands (`search`, `tail`, `get`, `aggregate`)
implemented. Metrics, monitors, events, and incidents planned for later
releases.

## Install

From this checkout:

```
cargo install --path crates/dd-cli --root ~/.local
```

(or any prefix on your `PATH`). Binary name is `ddog`.

From a release build without installing:

```
cargo build --release
./target/release/ddog --help
```

## Getting your Datadog credentials

The CLI talks to the Datadog v2 API, which requires **two** credentials,
issued on **two separate pages** in the Datadog UI:

| Credential         | Env var        | Where to get it                                                             | Identifies                          |
| ------------------ | -------------- | --------------------------------------------------------------------------- | ----------------------------------- |
| API key            | `DD_API_KEY`   | **Organization Settings → API Keys**                                        | Your Datadog **org** / tenant       |
| Application key    | `DD_APP_KEY`   | **Organization Settings → Application Keys**                                | Your **user** / scoped access level |

The two keys are not interchangeable. An API key alone is rejected by every
read endpoint; an Application key alone is rejected by every ingest endpoint.

### Step 1 — Identify your site

Datadog runs regional instances. The URL you log in at determines the `DD_SITE`
you need. Everything else (API keys, App keys, dashboards) is scoped to the
site — a key from `us1` will not work against `eu`.

| You log in at…                     | `DD_SITE` value           |
| ---------------------------------- | ------------------------- |
| `app.datadoghq.com`                | `datadoghq.com` (default) |
| `app.datadoghq.eu`                 | `datadoghq.eu`            |
| `us3.datadoghq.com`                | `us3.datadoghq.com`       |
| `us5.datadoghq.com`                | `us5.datadoghq.com`       |
| `ap1.datadoghq.com`                | `ap1.datadoghq.com`       |
| `app.ddog-gov.com`                 | `ddog-gov.com`            |

### Step 2 — Create an API key

1. Log in to Datadog.
2. Click your avatar (bottom-left) → **Organization Settings**.
3. Under **Access** in the left nav, click **API Keys**.
   Direct link (swap the subdomain for your site):
   `https://app.datadoghq.com/organization-settings/api-keys`
4. Click **New Key**. Give it a descriptive name (e.g. `ddog-cli-<your-name>`).
5. Click the key row to reveal the value, then **Copy Key**. This is a
   32-character hex string (or a UUID for legacy orgs). **Datadog shows the
   full key on demand but it is scoped to the org, so treat it as a secret.**

No extra permissions are needed beyond being an org member.

### Step 3 — Create an Application key

1. Still in **Organization Settings**, left nav → **Application Keys**
   (different page from API Keys).
   Direct link: `https://app.datadoghq.com/organization-settings/application-keys`
2. Click **New Key**.
3. **Scopes** (optional but recommended for least privilege):
   - `logs_read_data` — required for `ddog logs search`, `tail`, `get`.
   - `logs_read_index_data` — required for `ddog logs aggregate` and any
     query that touches a specific index.
   - `logs_read_archives` — only if you query `--storage online-archives`.
   - *(Leave unscoped to inherit your user's role permissions — simplest,
     fine for testing.)*
4. Click **Create Application Key** and **Copy Key** — a 40-character hex
   string. **This value is shown only at creation time.** If you lose it,
   delete the key and create a new one.

Permissions reference:
`https://docs.datadoghq.com/account_management/rbac/permissions/`

### Step 4 — Configure the CLI

Three ways to supply credentials, checked in this order (highest wins):

1. **CLI flags** — `--api-key`, `--app-key`, `--site`.
2. **Environment** — `DD_API_KEY`, `DD_APP_KEY`, `DD_SITE`, `DD_PROFILE`.
3. **Config file** — `~/.config/ddog/config.toml` (path overridable with
   `--config` / `DD_CONFIG`).

**Env approach (simple):**

```bash
export DD_API_KEY=<32-char hex from step 2>
export DD_APP_KEY=<40-char hex from step 3>
export DD_SITE=datadoghq.com   # or your region from step 1
ddog logs search 'status:error' --from now-15m
```

**Config file approach (multiple envs):**

```bash
mkdir -p ~/.config/ddog
cp docs/config.example.toml ~/.config/ddog/config.toml
chmod 600 ~/.config/ddog/config.toml
# then edit and fill in your keys
```

Example `~/.config/ddog/config.toml`:

```toml
default_site    = "datadoghq.com"
default_profile = "prod"

[profiles.prod]
api_key = "<api-key>"
app_key = "<app-key>"
site    = "datadoghq.com"

[profiles.staging]
api_key = "<api-key>"
app_key = "<app-key>"
site    = "datadoghq.eu"
```

Select a profile with `--profile staging` or `DD_PROFILE=staging`.

### Verify

```
ddog logs search '*' --from now-5m --max 1 -o json
```

- **Empty `[]`** → credentials work, your org just has no logs in the window.
- **`error: authentication failed`** → wrong keys, wrong site, or the App
  key was created in a different org than the API key. Regenerate both in
  the same org and retry.
- **`error: upstream error 403`** → the App key is missing a required scope.

## Usage

```
ddog logs search 'service:api status:error' --from now-1h
ddog logs search 'status:error' --from now-15m -o ndjson       # agent-friendly stream
ddog logs tail   'service:api status:error' --interval 5s
ddog logs get    AAAAxxxxx
ddog logs aggregate 'status:error' --group-by service --measure count --from now-1h
```

### Common flags

| Flag           | Purpose                                                     |
| -------------- | ----------------------------------------------------------- |
| `--from/--to`  | `now`, `now-15m`, `now-1h`, `now-7d`, or RFC-3339.          |
| `-n / --limit` | Page size (1–1000).                                         |
| `--max`        | Stop after N total events across pages (`0` = unlimited).   |
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

## License

MIT — see [`LICENSE`](LICENSE).
