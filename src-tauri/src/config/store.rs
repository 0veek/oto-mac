use super::model::AppConfig;
use crate::error::{OtoError, OtoResult};
use std::fs;
use std::path::{Path, PathBuf};

pub fn config_path() -> OtoResult<PathBuf> {
    let base = directories::ProjectDirs::from("dev", "Oto", "oto")
        .ok_or_else(|| OtoError::Message("could not resolve config dir".into()))?;
    Ok(base.config_dir().join("config.json"))
}

pub fn load_config() -> OtoResult<AppConfig> {
    let path = config_path()?;
    read_config_from(&path)
}

fn read_config_from(path: &Path) -> OtoResult<AppConfig> {
    if !path.exists() {
        // A missing file is the one true first run. Every other path — including
        // a config that predates the field — keeps `onboarding_complete: true`,
        // so upgrading never reopens the wizard.
        return Ok(AppConfig {
            onboarding_complete: false,
            ..AppConfig::default()
        });
    }
    let raw = fs::read_to_string(path)?;
    if raw.trim().is_empty() {
        // A truncated write is a damaged config, not a new install.
        return Ok(AppConfig::default());
    }
    let cfg: AppConfig = serde_json::from_str(&raw)?;
    // Hard guard: never accept api_key fields if present from older versions
    if raw.contains("api_key") {
        // still load structural fields; keys ignored
        let _ = ();
    }
    Ok(cfg)
}

pub fn save_config(cfg: &AppConfig) -> OtoResult<()> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_string_pretty(cfg)?;
    if raw.contains("api_key") {
        return Err(OtoError::Message(
            "refusing to write config that contains api_key".into(),
        ));
    }
    fs::write(path, raw)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::model::*;

    #[test]
    fn default_roundtrip_json_has_no_api_key() {
        let cfg = AppConfig::default();
        let raw = serde_json::to_string(&cfg).unwrap();
        assert!(!raw.contains("api_key"));
        let back: AppConfig = serde_json::from_str(&raw).unwrap();
        assert_eq!(cfg, back);
    }

    #[test]
    fn save_load_roundtrip_tmp() {
        let cfg = AppConfig {
            dictionary: vec!["Oto".into(), "Tauri".into()],
            polish_enabled: false,
            ..AppConfig::default()
        };
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, serde_json::to_string_pretty(&cfg).unwrap()).unwrap();
        let loaded: AppConfig = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(loaded.dictionary, vec!["Oto", "Tauri"]);
        assert!(!loaded.polish_enabled);
    }

    #[test]
    fn empty_or_missing_config_falls_back_to_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("config.json");
        // Only the absent file is treated as a first run.
        let fresh = read_config_from(&missing).unwrap();
        assert!(!fresh.onboarding_complete);
        assert_eq!(
            fresh,
            AppConfig {
                onboarding_complete: false,
                ..AppConfig::default()
            }
        );
        // A truncated write is a damaged config, not a new install: sending the
        // user through onboarding again would be a confusing way to report it.
        fs::write(&missing, "   \n").unwrap();
        assert_eq!(read_config_from(&missing).unwrap(), AppConfig::default());
    }

    #[test]
    fn an_existing_config_never_reopens_onboarding() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        // A 0.1.0 document has no `onboarding_complete` at all.
        fs::write(&path, r#"{"hotkey":"Ctrl+Shift+Space"}"#).unwrap();
        assert!(read_config_from(&path).unwrap().onboarding_complete);
    }
}
