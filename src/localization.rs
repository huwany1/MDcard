// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[serde(rename = "zh-CN")]
    SimplifiedChinese,
    #[serde(rename = "zh-TW")]
    TraditionalChinese,
    #[serde(rename = "en")]
    English,
    #[serde(rename = "ko")]
    Korean,
    #[serde(rename = "ja")]
    Japanese,
    #[serde(rename = "ru")]
    Russian,
}
impl Language {
    pub const ALL: [Self; 6] = [
        Self::SimplifiedChinese,
        Self::TraditionalChinese,
        Self::English,
        Self::Korean,
        Self::Japanese,
        Self::Russian,
    ];
    pub fn code(self) -> &'static str {
        match self {
            Self::SimplifiedChinese => "zh-CN",
            Self::TraditionalChinese => "zh-TW",
            Self::English => "en",
            Self::Korean => "ko",
            Self::Japanese => "ja",
            Self::Russian => "ru",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::SimplifiedChinese => "简体中文",
            Self::TraditionalChinese => "繁體中文",
            Self::English => "English",
            Self::Korean => "한국어",
            Self::Japanese => "日本語",
            Self::Russian => "Русский",
        }
    }
    pub fn from_tag(tag: &str) -> Option<Self> {
        let tag = tag
            .trim()
            .split('.')
            .next()?
            .replace('_', "-")
            .to_ascii_lowercase();
        let parts: Vec<_> = tag.split('-').collect();
        match parts.first().copied()? {
            "zh" => Some(
                if parts.iter().any(|p| ["tw", "hk", "mo", "hant"].contains(p)) {
                    Self::TraditionalChinese
                } else {
                    Self::SimplifiedChinese
                },
            ),
            "en" => Some(Self::English),
            "ko" => Some(Self::Korean),
            "ja" => Some(Self::Japanese),
            "ru" => Some(Self::Russian),
            _ => None,
        }
    }
    pub fn text(self, key: &'static str) -> &'static str {
        catalogs()
            .get(self.code())
            .and_then(|c| c.get(key))
            .map(String::as_str)
            .unwrap_or(key)
    }
    pub fn format(self, key: &'static str, values: &[(&str, &str)]) -> String {
        let mut result = self.text(key).to_owned();
        for (name, value) in values {
            result = result.replace(&format!("{{{name}}}"), value);
        }
        result
    }
    pub fn welcome(self) -> &'static str {
        match self {
            Self::SimplifiedChinese => include_str!("../locales/welcome-zh-CN.md"),
            Self::TraditionalChinese => include_str!("../locales/welcome-zh-TW.md"),
            Self::English => include_str!("../locales/welcome-en.md"),
            Self::Korean => include_str!("../locales/welcome-ko.md"),
            Self::Japanese => include_str!("../locales/welcome-ja.md"),
            Self::Russian => include_str!("../locales/welcome-ru.md"),
        }
    }
}
type Catalogs = BTreeMap<String, BTreeMap<String, String>>;
fn catalogs() -> &'static Catalogs {
    static CATALOGS: OnceLock<Catalogs> = OnceLock::new();
    CATALOGS.get_or_init(|| {
        serde_json::from_str(include_str!("../locales/ui.json"))
            .expect("validated translation catalog")
    })
}
pub fn system_language() -> Language {
    // Explicit override is useful for isolated desktop verification and remote sessions.
    if let Ok(tag) = std::env::var("MDCARD_SYSTEM_LOCALE") {
        return Language::from_tag(&tag).unwrap_or(Language::English);
    }
    #[cfg(target_os = "windows")]
    {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetUserDefaultLocaleName(name: *mut u16, count: i32) -> i32;
        }
        let mut name = [0u16; 85];
        // SAFETY: buffer is writable and its capacity is passed to the Windows API.
        let len = unsafe { GetUserDefaultLocaleName(name.as_mut_ptr(), name.len() as i32) };
        if len > 1 {
            return Language::from_tag(&String::from_utf16_lossy(&name[..len as usize - 1]))
                .unwrap_or(Language::English);
        }
    }
    #[cfg(not(target_os = "windows"))]
    for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(tag) = std::env::var(key)
            && !tag.is_empty()
        {
            return Language::from_tag(&tag).unwrap_or(Language::English);
        }
    }
    Language::English
}

pub fn install_fonts(ctx: &eframe::egui::Context, language: Language) {
    use eframe::egui::{FontData, FontDefinitions, FontFamily};
    let mut fonts = FontDefinitions::default();
    for (name, data) in [
        (
            "noto-sc",
            include_bytes!("../assets/MDcardDiagramSans-Regular.ttf").as_slice(),
        ),
        (
            "noto-tc",
            include_bytes!("../assets/MDcardUISansTC-Regular.ttf").as_slice(),
        ),
        (
            "noto-jp",
            include_bytes!("../assets/MDcardUISansJP-Regular.ttf").as_slice(),
        ),
        (
            "noto-kr",
            include_bytes!("../assets/MDcardDiagramSansKR-Regular.ttf").as_slice(),
        ),
        (
            "noto-emoji",
            include_bytes!("../assets/NotoEmoji.ttf").as_slice(),
        ),
    ] {
        fonts
            .font_data
            .insert(name.into(), FontData::from_static(data).into());
    }
    let first = match language {
        Language::TraditionalChinese => "noto-tc",
        Language::Japanese => "noto-jp",
        Language::Korean => "noto-kr",
        _ => "noto-sc",
    };
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        let list = fonts.families.get_mut(&family).unwrap();
        list.push(first.into());
        for name in ["noto-sc", "noto-tc", "noto-jp", "noto-kr", "noto-emoji"] {
            if name != first {
                list.push(name.into());
            }
        }
    }
    ctx.set_fonts(fonts);
}

pub fn refresh_welcome(
    doc: &mut crate::document::Document,
    previous: Language,
    next: Language,
) -> bool {
    if doc.path.is_none() && !doc.dirty() && doc.text == previous.welcome() {
        *doc = crate::document::Document::from_text(next.welcome());
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn locale_regions_scripts_and_unsupported_fallback() {
        for (tag, expected) in [
            ("zh_CN.UTF-8", Language::SimplifiedChinese),
            ("zh-Hans-SG", Language::SimplifiedChinese),
            ("zh-Hant", Language::TraditionalChinese),
            ("zh_HK", Language::TraditionalChinese),
            ("zh-MO", Language::TraditionalChinese),
            ("zh-TW", Language::TraditionalChinese),
            ("en_GB.UTF-8", Language::English),
            ("ko_KR", Language::Korean),
            ("ja-JP", Language::Japanese),
            ("ru_RU.UTF-8", Language::Russian),
        ] {
            assert_eq!(Language::from_tag(tag), Some(expected), "{tag}");
        }
        assert_eq!(Language::from_tag("de_DE"), None);
        assert_eq!(Language::from_tag("C.UTF-8"), None);
    }
    fn placeholders(value: &str) -> Vec<String> {
        let mut result = Vec::new();
        let mut rest = value;
        while let Some((_, tail)) = rest.split_once('{') {
            let (name, tail) = tail.split_once('}').expect("unclosed placeholder");
            result.push(name.to_owned());
            rest = tail;
        }
        result.sort();
        result
    }
    #[test]
    fn all_catalogs_complete_with_identical_placeholders_and_real_gfm_welcome() {
        use pulldown_cmark::{Event, Options, Parser, Tag};
        let source = &catalogs()["en"];
        assert_eq!(catalogs().len(), 6);
        assert!(source.len() >= 80);
        for language in Language::ALL {
            let target = &catalogs()[language.code()];
            assert_eq!(
                source.keys().collect::<Vec<_>>(),
                target.keys().collect::<Vec<_>>()
            );
            for (key, value) in target {
                assert!(!value.trim().is_empty(), "{language:?}: {key}");
                assert_eq!(
                    placeholders(key),
                    placeholders(value),
                    "{language:?}: {key}"
                );
            }
            let events: Vec<_> = Parser::new_ext(language.welcome(), Options::all()).collect();
            assert!(
                events
                    .iter()
                    .any(|e| matches!(e, Event::Start(Tag::Table(_))))
            );
            assert!(
                events
                    .iter()
                    .any(|e| matches!(e, Event::TaskListMarker(true)))
            );
            assert!(
                events
                    .iter()
                    .any(|e| matches!(e, Event::Start(Tag::CodeBlock(_))))
            );
            assert!(
                events
                    .iter()
                    .any(|e| matches!(e, Event::Start(Tag::Link { .. })))
            );
            assert!(
                !language
                    .text("# A new idea\n\nStart writing…")
                    .contains("Start writing")
                    || language == Language::English
            );
        }
    }
    #[test]
    fn bundled_fonts_cover_all_ui_welcome_and_language_names_without_system_fonts() {
        use eframe::egui::{Context, FontId, RawInput};
        for language in Language::ALL {
            let ctx = Context::default();
            install_fonts(&ctx, language);
            ctx.begin_pass(RawInput::default());
            let text = catalogs()[language.code()]
                .values()
                .cloned()
                .chain(Language::ALL.map(|l| l.name().to_string()))
                .collect::<Vec<_>>()
                .join("")
                + language.welcome();
            let chars: std::collections::BTreeSet<_> =
                text.chars().filter(|c| !c.is_control()).collect();
            ctx.fonts_mut(|fonts| {
                for font in [FontId::proportional(15.0), FontId::monospace(23.0)] {
                    for c in &chars {
                        assert!(
                            fonts.has_glyph(&font, *c),
                            "{language:?} {font:?}: missing {c:?}"
                        );
                    }
                }
            });
            let _ = ctx.end_pass();
        }
    }
    #[test]
    fn switching_language_preserves_user_edits_file_backed_documents_and_dirty_state() {
        use crate::document::Document;
        let mut doc = Document::from_text(Language::English.welcome());
        assert!(refresh_welcome(
            &mut doc,
            Language::English,
            Language::Korean
        ));
        assert_eq!(doc.text, Language::Korean.welcome());
        assert!(!doc.dirty());
        doc.text.push_str("\nMy unsaved work");
        let edited = doc.text.clone();
        assert!(!refresh_welcome(
            &mut doc,
            Language::Korean,
            Language::Russian
        ));
        assert_eq!(doc.text, edited);
        assert!(doc.dirty());
        let dir = tempfile::tempdir().unwrap();
        doc.save_to(&dir.path().join("한글_日本語_русский_繁體.md"))
            .unwrap();
        assert!(!refresh_welcome(
            &mut doc,
            Language::Korean,
            Language::Japanese
        ));
        assert_eq!(doc.text, edited);
        assert!(!doc.dirty());
        let mut empty = Document::default();
        assert!(!refresh_welcome(
            &mut empty,
            Language::English,
            Language::Japanese
        ));
        assert!(empty.text.is_empty());
    }
}
