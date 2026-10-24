// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
use serde::{Deserialize, Serialize};
use std::{
    io::{self, Write},
    path::PathBuf,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Appearance {
    #[default]
    Light,
    Dark,
    System,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    /// None follows the OS language; older settings remain compatible.
    #[serde(deserialize_with = "deserialize_language")]
    pub language: Option<crate::localization::Language>,
    pub appearance: Appearance,
    pub accent: usize,
    pub gpu_acceleration: bool,
    pub reduce_motion: bool,
    pub font_size: f32,
    /// None selects wrapping automatically according to document size.
    pub source_wrap: Option<bool>,
}
fn deserialize_language<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<crate::localization::Language>, D::Error> {
    let tag = Option::<String>::deserialize(deserializer)?;
    Ok(tag
        .as_deref()
        .and_then(crate::localization::Language::from_tag))
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            language: None,
            appearance: Appearance::Light,
            accent: 0,
            gpu_acceleration: true,
            reduce_motion: false,
            font_size: 15.0,
            source_wrap: None,
        }
    }
}
pub fn path() -> PathBuf {
    if let Some(path) = std::env::var_os("MDCARD_SETTINGS_PATH") {
        return path.into();
    }
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(not(target_os = "windows"))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")));
    base.unwrap_or_else(|| PathBuf::from("."))
        .join("MDcard")
        .join("settings.json")
}
pub fn load() -> Result<Preferences, String> {
    let bytes = match std::fs::read(path()) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Preferences::default()),
        Err(e) => return Err(format!("Could not read settings: {e}")),
    };
    let mut p: Preferences = serde_json::from_slice(&bytes)
        .map_err(|e| format!("Settings are invalid; defaults are active. {e}"))?;
    p.accent = p.accent.min(5);
    if !p.font_size.is_finite() {
        p.font_size = 15.0;
    }
    p.font_size = p.font_size.clamp(12.0, 23.0);
    Ok(p)
}
pub fn save(p: &Preferences) -> io::Result<()> {
    save_to(&path(), p)
}
fn save_to(path: &std::path::Path, p: &Preferences) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    std::fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(&serde_json::to_vec_pretty(p)?)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|e| e.error)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_roundtrip_and_failed_save_preserves_previous() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        let p = Preferences {
            gpu_acceleration: false,
            appearance: Appearance::Dark,
            accent: 5,
            ..Default::default()
        };
        save_to(&file, &p).unwrap();
        assert_eq!(
            serde_json::from_slice::<Preferences>(&std::fs::read(&file).unwrap()).unwrap(),
            p
        );
        assert!(save_to(&file.join("impossible"), &Preferences::default()).is_err());
        assert_eq!(
            serde_json::from_slice::<Preferences>(&std::fs::read(&file).unwrap()).unwrap(),
            p
        );
    }
    #[test]
    fn old_settings_follow_system_and_all_language_choices_persist() {
        let old = r#"{"appearance":"Dark","accent":4,"gpu_acceleration":false,"reduce_motion":true,"font_size":23.0}"#;
        let p: Preferences = serde_json::from_str(old).unwrap();
        assert_eq!(p.language, None);
        assert_eq!(p.appearance, Appearance::Dark);
        assert_eq!(p.accent, 4);
        let future: Preferences = serde_json::from_str(
            r#"{"language":"de-DE","appearance":"Dark","accent":4,"gpu_acceleration":false}"#,
        )
        .unwrap();
        assert_eq!(future.language, None);
        assert_eq!(future.appearance, Appearance::Dark);
        assert!(!future.gpu_acceleration);
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        for language in crate::localization::Language::ALL {
            let p = Preferences {
                language: Some(language),
                ..p.clone()
            };
            save_to(&file, &p).unwrap();
            let restored: Preferences =
                serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
            assert_eq!(restored, p);
            assert!(!restored.gpu_acceleration);
        }
    }
}
