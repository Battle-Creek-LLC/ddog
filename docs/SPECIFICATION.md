# `dd` — Datadog CLI

A Rust-based command-line tool for interacting with the Datadog API. Designed
for humans at the terminal **and** for AI agents that need structured output.

## Goals

1. **Agent-first ergonomics** — every command supports `--json` output; errors
   go to stderr with non-zero exit codes; no interactive prompts unless
   explicitly requested.
2. **Human-first defaults** — colorized, paginated, tail-able output when a TTY
   is detected.
3. **Surface area grows by subcommand** — v0 ships `logs`; `metrics`, `monitors`,
   `events`, `incidents` land later without reshuffling the top-level UX.
4. **Zero config to start** — reads `DD_API_KEY` / `DD_APP_KEY` / `DD_SITE` from
   environment. A `~/.config/dd/config.toml` is optional.

## Non-goals (v0)

- Ingesting logs / metrics (this is a read/query tool, not an agent forwarder).
- Managing dashboards or SLOs.
- Terraform-style declarative config.

---

## Top-level UX

```
dd [GLOBAL OPTIONS] <COMMAND> [ARGS]
```

### Global options

| Flag              | Env var       | Default          | Notes                                             |
| ----------------- | ------------- | ---------------- | ------------------------------------------------- |
| `--api-key`       | `DD_API_KEY`  | —                | Required. Datadog API key.                        |
| `--app-key`       | `DD_APP_KEY`  | —                | Required for most endpoints.                      |
| `--site`          | `DD_SITE`     | `datadoghq.com`  | e.g. `datadoghq.eu`, `us3.datadoghq.com`, `us5…`. |
| `--output`, `-o`  | `DD_OUTPUT`   | auto             | `text` \| `json` \| `ndjson` \| `table`.          |
| `--color`         | `NO_COLOR`    | auto             | `auto` \| `always` \| `never`.                    |
| `--quiet`, `-q`   | —             | false            | Suppress progress/status output.                  |
| `--verbose`, `-v` | —             | 0                | Repeatable; `-vv` enables debug, `-vvv` trace.    |
| `--config`        | `DD_CONFIG`   | `~/.config/dd/config.toml` | Override config path.                  |

### Exit codes

| Code | Meaning                                                       |
| ---- | ------------------------------------------------------------- |
| 0    | Success.                                                      |
| 1    | Generic error (usage, parsing).                               |
| 2    | Auth error (401 / 403).                                       |
| 3    | Not found (404).                                              |
| 4    | Rate limited (429). Includes `Retry-After` hint on stderr.    |
| 5    | Upstream 5xx after retries exhausted.                         |
| 6    | Network / TLS error.                                          |

### Output modes

- **`text`** — default when stdout is a TTY. Human-readable, colorized.
- **`json`** — single JSON document (object or array). Pretty-printed when TTY,
  compact otherwise. Default when stdout is *not* a TTY.
- **`ndjson`** — one JSON object per line. Preferred for streaming (`tail`,
  `search` with pagination).
- **`table`** — ASCII table of the most useful columns per command.

---

## `dd logs` — v0 surface

Wraps Datadog Logs API v2 (`/api/v2/logs/events`).

```
dd logs <SUBCOMMAND>

Subcommands:
  search      One-shot query over a time range.
  tail        Stream new logs matching a query (polling).
  get         Fetch a single log event by ID.
  aggregate   Run an aggregation query (group by, measures).
  facets      List available facets for the index.
```

### `dd logs search`

Run a single search against the Logs API.

```
dd logs search [OPTIONS] [QUERY]
```

**Arguments**

- `QUERY` — positional Datadog log query string
  (e.g. `service:api status:error`). If omitted, reads from stdin.

**Options**

| Flag              | Default     | Notes                                                       |
| ----------------- | ----------- | ----------------------------------------------------------- |
| `--from`          | `now-15m`   | Absolute ISO-8601 or relative (`now-1h`, `now-7d`).         |
| `--to`            | `now`       | Same format as `--from`.                                    |
| `--limit`, `-n`   | `100`       | Page size, max 1000.                                        |
| `--max`           | `1000`      | Stop paginating after N total events. `0` = unlimited.      |
| `--sort`          | `-timestamp`| `timestamp` ascending, `-timestamp` descending.             |
| `--index`         | `*`         | Comma-separated index names.                                |
| `--fields`        | —           | Comma-separated attributes to include (text/table output).  |
| `--storage`       | `indexes`   | `indexes` \| `online-archives` \| `flex`.                   |

**Example — human**

```
$ dd logs search 'service:api status:error' --from now-1h --limit 20
2026-04-16T18:04:12Z  ERROR  api        Failed to fetch holdings: timeout
2026-04-16T18:04:14Z  ERROR  api        Database connection lost
...
20 events (1.2s)
```

**Example — agent**

```
$ dd logs search 'service:api status:error' -o ndjson --from now-1h
{"id":"AAAA...","timestamp":"2026-04-16T18:04:12Z","service":"api","status":"error","message":"..."}
{"id":"BBBB...","timestamp":"2026-04-16T18:04:14Z","service":"api","status":"error","message":"..."}
```

### `dd logs tail`

Stream new matching logs. Polls the search endpoint at a configurable interval
(Datadog does not expose a live log stream over HTTP).

```
dd logs tail [OPTIONS] [QUERY]
```

| Flag              | Default    | Notes                                                |
| ----------------- | ---------- | ---------------------------------------------------- |
| `--interval`      | `5s`       | Poll interval. Minimum 2s.                           |
| `--since`         | `now`      | Start timestamp for the first poll.                  |
| `--fields`        | —          | Same as `search`.                                    |

Deduplicates across polls using event IDs. Exits on SIGINT with a summary line.

### `dd logs get`

```
dd logs get <EVENT_ID> [--index <name>]
```

Returns the full event document. Primary use: an agent finds an ID via `search`
and fetches the full payload via `get`.

### `dd logs aggregate`

Wraps `POST /api/v2/logs/analytics/aggregate`. Accepts either flags for the
common case or `--spec <file>.json` for the full API schema.

```
dd logs aggregate [OPTIONS] [QUERY]

  --group-by <facet>      Repeatable. e.g. --group-by service --group-by status
  --measure <agg:facet>   Repeatable. e.g. --measure count, --measure avg:@duration
  --from, --to            As above.
```

Output: table by default, JSON for agents.

### `dd logs facets`

Lists facets available on the index, optionally filtered by prefix. Mostly a
discovery aid for agents building queries.

---

## `dd metrics` — timeseries

Wraps the v2 multi-product query API. `query` (timeseries) ships first; `scalar`
and multi-query formulas come later.

### `dd metrics query`

```
dd metrics query [OPTIONS] [QUERY]
```

`POST /api/v2/query/timeseries` with a single `metrics` query. The legacy v1
`GET /api/v1/query` is intentionally avoided — scoped Application keys are not
authorized for it (it returns `403`), whereas the v2 endpoint honors the
`timeseries_query` scope.

| Flag          | Default   | Notes                                                       |
| ------------- | --------- | ----------------------------------------------------------- |
| `--from`      | `now-1d`  | Absolute RFC-3339 or relative (`now-1h`, `now-7d`). Sent as epoch ms. |
| `--to`        | `now`     | Same format as `--from`.                                    |
| `--interval`  | —         | Bucket duration (`1d`, `1h`, `15m`). Omitted ⇒ Datadog auto-rollup. |

The column-oriented response (`series[]` + shared `times[]` + `values[][]`) is
flattened to one record per (series, bucket): `{scope, timestamp, timestamp_ms,
value}`. `value` is `null` for an empty bucket — deliberately distinct from `0`.

**Example — agent**

```
$ dd metrics query 'sum:bridgeft.import.records{*} by {feed}.rollup(sum, 86400)' \
    --from now-7d --interval 1d -o ndjson
{"scope":"feed:positions","timestamp":"2026-05-21T00:00:00Z","timestamp_ms":1747785600000,"value":2901.0}
```

---

## Configuration file

Optional `~/.config/dd/config.toml`. Environment wins over config; CLI flags
win over environment.

```toml
default_site = "datadoghq.com"
default_output = "text"

[profiles.prod]
api_key = "…"
app_key = "…"
site = "datadoghq.com"

[profiles.staging]
api_key = "…"
app_key = "…"
site = "datadoghq.eu"
```

Select with `--profile prod` or `DD_PROFILE=prod`.

---

## Architecture

```
crates/
  dd-cli/        # Binary. clap parsing, output formatting, TTY detection.
  dd-api/        # HTTP client. Typed request/response, retries, pagination.
  dd-config/     # Config file + env + flag merging.
```

**Key dependencies**

| Crate                | Purpose                                   |
| -------------------- | ----------------------------------------- |
| `clap` (v4, derive)  | CLI parsing.                              |
| `reqwest` (rustls)   | HTTP.                                     |
| `tokio`              | Async runtime.                            |
| `serde` / `serde_json` | (De)serialization.                      |
| `time` or `chrono`   | Timestamp parsing (`now-1h` etc).         |
| `tracing` + `tracing-subscriber` | Structured logging with `-v`.    |
| `anyhow` / `thiserror` | Error plumbing.                         |
| `comfy-table`        | Table output.                             |
| `is-terminal`        | TTY detection for output mode default.    |

**HTTP behavior**

- Timeout: 30s per request (configurable).
- Retries: exponential backoff on 429 and 5xx, max 3 attempts, honors
  `Retry-After`.
- Pagination: auto-follows cursor for `search` until `--max` reached.
- User-Agent: `dd-cli/<version> (+https://…)`.

---

## Testing strategy

- Unit tests for config resolution and time parsing.
- Integration tests against a mock HTTP server (`wiremock` crate) covering:
  happy path, 401, 404, 429 with retry, paginated cursor, ndjson streaming.
- Snapshot tests (`insta`) for human-readable output.
- No tests hit real Datadog in CI.

---

## Roadmap after v0

1. `dd metrics query` — timeseries via `/api/v2/query/timeseries` ✅ shipped;
   `/api/v2/query/scalar` and multi-query formulas next.
2. `dd monitors {list,get,mute,unmute}`.
3. `dd events {list,post}`.
4. `dd incidents {list,get}`.
5. Shell completions (`dd completions <shell>`).
6. Publish to crates.io + Homebrew tap.

---

## Open questions for review

1. **Binary name** — `dd` collides with the Unix `dd(1)` utility. Alternative:
   `ddog`, `datadog`, or `dd-cli`. Suggest `ddog` if we want PATH safety;
   `dd` is punchier if we don't mind requiring users to alias if they use
   raw `dd(1)` a lot.
2. **Async runtime** — Tokio vs keeping it sync via `ureq`. Async gives us
   `tail` + concurrent pagination; sync gives a smaller binary and simpler
   build. Recommend Tokio.
3. **Output default for TTY** — text or table? Text is easier to grep; table
   is prettier. Recommend text, table opt-in via `-o table`.
4. **Config directory** — `~/.config/dd/` vs `~/.datadog/`. XDG-compliant
   wins.
