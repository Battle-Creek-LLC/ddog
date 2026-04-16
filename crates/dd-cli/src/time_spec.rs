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
}
