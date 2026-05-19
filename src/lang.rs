//! Minimal i18n = Lang enum + Strings struct with ja/en const instances。
//!
//! Phase 1 step 4 (= RFC v0.2 §5.1 deliverable 4): runtime swap-able localized
//! string table、~20 string、ja/en の 2 言語、外部 i18n crate 依存なし。
//!
//! ## 規範
//! - [[feedback_new_apps_depend_on_gui_kit_only]]: hayate-kit のみ依存、
//!   外部 i18n crate (= fluent / gettext-rs / rust-i18n etc.) は不採用、
//!   minimal const lookup で sufficient
//! - [[feedback_root_cause_over_quick_fix]]: 将来 full i18n crate へ migrate
//!   可能な type-safe API (= 文字列 key ではなく struct field) を採用
//!
//! ## 拡張方針
//! - 新 string 追加: Strings struct に field 追加 + JA/EN 両 const に値追加
//! - 新言語追加: Lang enum に variant 追加 + Strings::XX const + strings()
//!   match arm 追加
//! - format! 系の動的部分は呼出側で formatting (= println!("{}", strings.foo))
//!
//! ## Lang detect protocol
//! 1. CLI --lang ja|en 明示指定が最優先
//! 2. 不在時 LANG env var 先頭 "ja" → Ja、その他 → En
//! 3. LANG 不在時 → En (= default)

/// Supported UI languages (= Phase 1 minimal subset)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// 日本語
    Ja,
    /// English
    En,
}

impl Lang {
    /// Resolve `Strings` table for this `Lang`.
    pub fn strings(self) -> &'static Strings {
        match self {
            Lang::Ja => &Strings::JA,
            Lang::En => &Strings::EN,
        }
    }

    /// Detect `Lang` from `LANG` env var (= `ja_JP.UTF-8` → `Ja`、その他 → `En`)。
    /// CLI override (= `--lang ja|en`) は呼出側で `Lang::from_cli` 経由で解決。
    pub fn detect() -> Self {
        let lang = std::env::var("LANG").unwrap_or_default();
        if lang.starts_with("ja") {
            Lang::Ja
        } else {
            Lang::En
        }
    }

    /// Parse CLI `--lang` argument (= `ja` or `en`、case-insensitive)。
    /// 不明値 or 不在時 `None` を返す = 呼出側で `detect()` fallback。
    pub fn from_cli(arg: Option<&str>) -> Option<Self> {
        match arg.map(str::to_ascii_lowercase).as_deref() {
            Some("ja") => Some(Lang::Ja),
            Some("en") => Some(Lang::En),
            _ => None,
        }
    }
}

/// Localized string table — Phase 1 minimal scope (~10 strings)。
///
/// 拡張時は本 struct に field 追加 + `JA` / `EN` 両 const にも値追加。
///
/// `#[allow(dead_code)]`: 一部 field (= `app_description` / `cli_*_desc`) は
/// 現状未使用 (= clap auto-doc は compile-time literal を要求、runtime resolved
/// string を inject 不可)、Phase 1 step 6 (settings panel skeleton extend) や
/// Phase 4 i18n full crate migrate 時に wire 化予定。
#[allow(dead_code)]
pub struct Strings {
    // App identity
    pub app_title: &'static str,
    pub app_description: &'static str,

    // Visual prototype placeholder (= Phase 1 step 3 wire の単行 label)
    pub placeholder_text: &'static str,

    // Safe boot mode messages (= Phase 1 step 7 --reset-config flag)
    pub reset_config_done: &'static str,
    pub reset_config_backup: &'static str,
    pub reset_config_not_found: &'static str,

    // CLI flag descriptions (= clap doc help、--reset-config / --lang)
    pub cli_reset_config_desc: &'static str,
    pub cli_lang_desc: &'static str,
}

impl Strings {
    /// 日本語 localized strings。
    pub const JA: Self = Self {
        app_title: "hayate-kit-settings — HAYATE Original プロトタイプ",
        app_description:
            "GUI_kit framework-native 設定パネル app (新世代 app 第 1 号、HAYATE Original aesthetic + theme switcher)",
        placeholder_text:
            "HAYATE Original ビジュアル プロトタイプ  /  風藍 accent #5A8BA8",
        reset_config_done: "設定をリセットしました",
        reset_config_backup: "バックアップを保存しました",
        reset_config_not_found: "設定ファイルが存在しません (既に default state)",
        cli_reset_config_desc:
            "Safe boot モード: 設定ファイルを .bak へ rename + GUI 起動せず exit (= R12 mitigation = bricked state 復旧 path)",
        cli_lang_desc: "UI 言語の指定 (ja | en、デフォルトは LANG 環境変数から検出)",
    };

    /// English localized strings。
    pub const EN: Self = Self {
        app_title: "hayate-kit-settings — HAYATE Original prototype",
        app_description:
            "GUI_kit framework-native settings panel app (新世代 app 第 1 号、HAYATE Original aesthetic + theme switcher)",
        placeholder_text:
            "HAYATE Original visual prototype  /  風藍 accent #5A8BA8",
        reset_config_done: "Reset config",
        reset_config_backup: "Backup saved",
        reset_config_not_found: "Config not found (already in default state)",
        cli_reset_config_desc:
            "Safe boot mode: rename config file to .bak and exit without GUI launch (= R12 mitigation = bricked state recovery path)",
        cli_lang_desc: "UI language (ja | en, default detected from LANG env var)",
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lang_from_cli_case_insensitive() {
        assert_eq!(Lang::from_cli(Some("ja")), Some(Lang::Ja));
        assert_eq!(Lang::from_cli(Some("JA")), Some(Lang::Ja));
        assert_eq!(Lang::from_cli(Some("en")), Some(Lang::En));
        assert_eq!(Lang::from_cli(Some("EN")), Some(Lang::En));
        assert_eq!(Lang::from_cli(Some("fr")), None);
        assert_eq!(Lang::from_cli(None), None);
    }

    #[test]
    fn strings_table_complete_for_both_langs() {
        // smoke: 両言語 const が compile される + 主要 string が空でない
        assert!(!Strings::JA.app_title.is_empty());
        assert!(!Strings::EN.app_title.is_empty());
        assert!(!Strings::JA.reset_config_done.is_empty());
        assert!(!Strings::EN.reset_config_done.is_empty());
    }

    #[test]
    fn lang_strings_dispatch() {
        // behavioral: Ja/En dispatch returns distinct string content
        assert_ne!(Lang::Ja.strings().app_title, Lang::En.strings().app_title);
        assert_ne!(
            Lang::Ja.strings().reset_config_done,
            Lang::En.strings().reset_config_done
        );
    }
}
