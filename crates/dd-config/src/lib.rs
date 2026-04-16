use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

pub const DEFAULT_SITE: &str = "datadoghq.com";

#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub api_key: String,
    pub app_key: String,
    pub site: String,
    pub profile: Option<String>,
    pub source: ConfigSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigSource {
    EnvOnly,
    File,
    FileAndEnv,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct FileConfig {
    pub default_site: Option<String>,
    pub default_profile: Option<String>,
    #[serde(default)]
    pub profiles: BTreeMap<String, Profile>,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct Profile {
    pub api_key: Option<String>,
    pub app_key: Option<String>,
    pub site: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct Overrides {
    pub api_key: Option<String>,
    pub app_key: Option<String>,
    pub site: Option<String>,
    pub profile: Option<String>,
    pub config_path: Option<PathBuf>,
}

pub fn default_config_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("com", "ddog", "ddog")
        .map(|dirs| dirs.config_dir().join("config.toml"))
}

pub fn resolve(overrides: Overrides) -> Result<ResolvedConfig> {
    let env_api_key = std::env::var("DD_API_KEY").ok();
    let env_app_key = std::env::var("DD_APP_KEY").ok();
    let env_site = std::env::var("DD_SITE").ok();
    let env_profile = std::env::var("DD_PROFILE").ok();

    let config_path = overrides
        .config_path
        .clone()
        .or_else(|| std::env::var("DD_CONFIG").ok().map(PathBuf::from))
        .or_else(default_config_path);

    let (file_config, file_loaded) = match config_path.as_deref() {
        Some(path) if path.exists() => (load_file(path)?, true),
        _ => (FileConfig::default(), false),
    };

    let profile_name = overrides
        .profile
        .clone()
        .or(env_profile)
        .or_else(|| file_config.default_profile.clone());

    let profile = match profile_name.as_deref() {
        Some(name) => Some(
            file_config
                .profiles
                .get(name)
                .with_context(|| {
                    format!("profile '{name}' not found in config file")
                })?
                .clone(),
        ),
        None => None,
    };

    let api_key = overrides
        .api_key
        .or(env_api_key)
        .or_else(|| profile.as_ref().and_then(|p| p.api_key.clone()));
    let app_key = overrides
        .app_key
        .or(env_app_key)
        .or_else(|| profile.as_ref().and_then(|p| p.app_key.clone()));
    let site = overrides
        .site
        .or(env_site)
        .or_else(|| profile.as_ref().and_then(|p| p.site.clone()))
        .or(file_config.default_site)
        .unwrap_or_else(|| DEFAULT_SITE.to_string());

    let api_key = api_key
        .filter(|s| !s.is_empty())
        .context("DD_API_KEY is required (set --api-key, DD_API_KEY env, or profile)")?;
    let app_key = app_key
        .filter(|s| !s.is_empty())
        .context("DD_APP_KEY is required (set --app-key, DD_APP_KEY env, or profile)")?;

    let source = match (file_loaded, std::env::var("DD_API_KEY").is_ok()) {
        (true, true) => ConfigSource::FileAndEnv,
        (true, false) => ConfigSource::File,
        _ => ConfigSource::EnvOnly,
    };

    Ok(ResolvedConfig {
        api_key,
        app_key,
        site,
        profile: profile_name,
        source,
    })
}

fn load_file(path: &Path) -> Result<FileConfig> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("reading config file {}", path.display()))?;
    let parsed: FileConfig = toml::from_str(&raw)
        .with_context(|| format!("parsing config file {}", path.display()))?;
    for (name, _) in &parsed.profiles {
        if name.trim().is_empty() {
            bail!("profile names cannot be empty");
        }
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_profiles_toml() {
        let toml = r#"
            default_site = "datadoghq.eu"
            default_profile = "prod"

            [profiles.prod]
            api_key = "a"
            app_key = "b"

            [profiles.staging]
            api_key = "c"
            app_key = "d"
            site = "us5.datadoghq.com"
        "#;
        let parsed: FileConfig = toml::from_str(toml).unwrap();
        assert_eq!(parsed.default_site.as_deref(), Some("datadoghq.eu"));
        assert_eq!(parsed.default_profile.as_deref(), Some("prod"));
        assert_eq!(parsed.profiles.len(), 2);
        assert_eq!(
            parsed.profiles["staging"].site.as_deref(),
            Some("us5.datadoghq.com")
        );
    }
}
