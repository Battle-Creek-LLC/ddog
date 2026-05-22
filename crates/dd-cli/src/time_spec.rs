//! Parse time expressions like `now`, `now-15m`, `now-1h`, or ISO-8601.
//!
//! The Datadog API itself accepts the `now-<N><unit>` shorthand directly, so
//! we pass the raw string through when it matches that pattern. For absolute
//! timestamps we validate that they parse as RFC-3339 and re-emit in UTC.

use anyhow::{bail, Context, Result};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

pub fn normalize(input: &str) -> Result<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        bail!("time value cannot be empty");
    }

    if trimmed == "now" || is_relative(trimmed) {
        return Ok(trimmed.to_string());
    }

    // Absolute: must parse as RFC-3339.
    let parsed = OffsetDateTime::parse(trimmed, &Rfc3339)
        .with_context(|| format!("expected 'now', 'now-<N><unit>', or RFC-3339 timestamp; got '{trimmed}'"))?;
    Ok(parsed.format(&Rfc3339)?)
}

/// Resolve a time expression to a Unix epoch in **seconds**.
///
/// The logs API takes the `now-<N><unit>` shorthand verbatim, so [`normalize`]
/// passes it through. The v2 metrics timeseries endpoint instead wants a
/// numeric epoch (in milliseconds; the caller scales this up), so here we
/// actually resolve `now` / `now-<N><unit>` against the current time, and parse
/// RFC-3339 timestamps to their epoch.
pub fn to_epoch_secs(input: &str) -> Result<i64> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        bail!("time value cannot be empty");
    }

    let now = OffsetDateTime::now_utc().unix_timestamp();
    if trimmed == "now" {
        return Ok(now);
    }

    if let Some(rest) = trimmed.strip_prefix("now-") {
        let (num, unit) = rest.split_at(rest.len().saturating_sub(1));
        let n: i64 = num
            .parse()
            .map_err(|_| anyhow::anyhow!("invalid relative time '{trimmed}'"))?;
        let secs_per = match unit {
            "s" => 1,
            "m" => 60,
            "h" => 3_600,
            "d" => 86_400,
            "w" => 604_800,
            _ => bail!("invalid relative time unit in '{trimmed}' (expected s|m|h|d|w)"),
        };
        return Ok(now - n * secs_per);
    }

    let parsed = OffsetDateTime::parse(trimmed, &Rfc3339).with_context(|| {
        format!("expected 'now', 'now-<N><unit>', or RFC-3339 timestamp; got '{trimmed}'")
    })?;
    Ok(parsed.unix_timestamp())
}

/// Parse a bare duration like `15m`, `1h`, `1d`, `1w` into seconds.
pub fn duration_secs(input: &str) -> Result<i64> {
    let s = input.trim();
    if s.is_empty() {
        bail!("duration cannot be empty");
    }
    let (num, unit) = s.split_at(s.len().saturating_sub(1));
    let n: i64 = num
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid duration '{s}'"))?;
    let secs_per = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 3_600,
        "d" => 86_400,
        "w" => 604_800,
        _ => bail!("invalid duration unit in '{s}' (expected s|m|h|d|w)"),
    };
    Ok(n * secs_per)
}

fn is_relative(s: &str) -> bool {
    // now-15m, now-1h, now-2d, now-30s, now-1w (Datadog accepts these)
    let Some(rest) = s.strip_prefix("now-") else {
        return false;
    };
    if rest.is_empty() {
        return false;
    }
    let (num, unit) = rest.split_at(rest.len() - 1);
    if !matches!(unit, "s" | "m" | "h" | "d" | "w") {
        return false;
    }
    num.chars().all(|c| c.is_ascii_digit()) && !num.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_passes_through() {
        assert_eq!(normalize("now").unwrap(), "now");
        assert_eq!(normalize("now-15m").unwrap(), "now-15m");
        assert_eq!(normalize("now-1h").unwrap(), "now-1h");
        assert_eq!(normalize("now-7d").unwrap(), "now-7d");
    }

    #[test]
    fn absolute_parses_rfc3339() {
        let got = normalize("2026-04-16T18:04:12Z").unwrap();
        assert!(got.starts_with("2026-04-16"));
    }

    #[test]
    fn rejects_garbage() {
        assert!(normalize("yesterday").is_err());
        assert!(normalize("").is_err());
        assert!(normalize("now-").is_err());
        assert!(normalize("now-1x").is_err());
    }

    #[test]
    fn epoch_resolves_relative() {
        let now = to_epoch_secs("now").unwrap();
        let an_hour_ago = to_epoch_secs("now-1h").unwrap();
        assert!((now - an_hour_ago - 3_600).abs() <= 2);
        let two_days_ago = to_epoch_secs("now-2d").unwrap();
        assert!((now - two_days_ago - 172_800).abs() <= 2);
    }

    #[test]
    fn epoch_parses_absolute() {
        // 2021-01-01T00:00:00Z == 1_609_459_200
        assert_eq!(to_epoch_secs("2021-01-01T00:00:00Z").unwrap(), 1_609_459_200);
    }

    #[test]
    fn epoch_rejects_garbage() {
        assert!(to_epoch_secs("tomorrow").is_err());
        assert!(to_epoch_secs("now-1x").is_err());
        assert!(to_epoch_secs("now-").is_err());
        assert!(to_epoch_secs("").is_err());
    }

    #[test]
    fn duration_parses_units() {
        assert_eq!(duration_secs("15m").unwrap(), 900);
        assert_eq!(duration_secs("1h").unwrap(), 3_600);
        assert_eq!(duration_secs("1d").unwrap(), 86_400);
        assert_eq!(duration_secs("1w").unwrap(), 604_800);
    }

    #[test]
    fn duration_rejects_garbage() {
        assert!(duration_secs("").is_err());
        assert!(duration_secs("1y").is_err());
        assert!(duration_secs("abc").is_err());
    }
}
