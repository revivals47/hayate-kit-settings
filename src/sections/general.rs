//! General section (= RFC v0.5 §5.2.1)。
//!
//! ## fields:
//! - Language picker (ComboBox = `LangConfig::Ja` / `LangConfig::En`)
//! - Startup behavior toggle (Switch = 起動時 reset window pos / restore last state)
//! - Window remember position toggle (Switch)
//!
//! ## wave 3b — persistence wire 配線済
//! 各 widget の on_toggle / on_select callback が
//! [`apply_*`] helper 経由で `state.config` を更新し
//! [`DebouncedSaver::request`](crate::persistence::DebouncedSaver::request) を発火。
//! callback closure は移動後 widget の内部に閉じ込められて外部から呼べないため、
//! helper fn 抽出により unit test で 1:1 に exercise する設計
//! (= wave 3 reactive bind 配線時の挙動を closure 越しではなく helper 経由で検証)。

use hayate_kit::prelude::*;

use crate::lang::Strings;
use crate::persistence::LangConfig;
use crate::state::AppStateHandles;

/// 日本語 picker label literal。callback の `&str` 比較と test 双方で参照する
/// 唯一の source of truth。
const LANG_JA_LABEL: &str = "日本語";
/// English picker label literal。
const LANG_EN_LABEL: &str = "English";

/// Build General section widget tree。
///
/// 構造 = `VStack { heading + FormLayout { 3 rows } }`。
/// HAYATE Original aesthetic は `active_theme()` default 経由で自動適用
/// (= R13 systemic fix 済、caller `.with_color(...)` override 不要)。
pub fn build(strings: &'static Strings, state: &AppStateHandles) -> Box<dyn Widget> {
    let heading = LabelWidget::new(strings.section_general, 18.0);

    // ── Language picker ────────────────────────────────────────────────
    // 初期 value は state.config から復元 (= persistence::load() で読まれた値)
    let language_picker = ComboBoxWidget::new(vec![
        LANG_JA_LABEL.to_string(),
        LANG_EN_LABEL.to_string(),
    ])
    .on_select({
        let s = state.clone();
        move |selected| apply_language_selection(&s, selected)
    });

    // ── Startup behavior ───────────────────────────────────────────────
    let startup_initial = state.config.get().general.startup_reset_window;
    let startup_reset = SwitchWidget::new(startup_initial).on_toggle({
        let s = state.clone();
        move |checked| apply_startup_reset_change(&s, checked)
    });

    // ── Remember window position ───────────────────────────────────────
    let remember_initial = state.config.get().general.remember_window_position;
    let remember_position = SwitchWidget::new(remember_initial).on_toggle({
        let s = state.clone();
        move |checked| apply_remember_position_change(&s, checked)
    });

    let form = FormLayout::new()
        .row("Language", language_picker)
        .row("Reset window position on startup", startup_reset)
        .row("Remember window position", remember_position);

    let mut stack = VStack::new(16.0);
    stack = stack.add(Box::new(heading));
    stack = stack.add(Box::new(form));
    Box::new(stack)
}

// ── on_callback helpers (= 1:1 unit-test exercise 用に抽出) ─────────────

/// Language picker の選択を [`AppStateHandles::config`] に反映 + debouncer 発火。
///
/// 未知 label (= `Strings` 拡張で picker に新 variant が追加された未来) は
/// `En` に fallback (= 既存 default と整合、 destructive 回避)。
fn apply_language_selection(state: &AppStateHandles, selected: &str) {
    let lang = match selected {
        LANG_JA_LABEL => LangConfig::Ja,
        LANG_EN_LABEL => LangConfig::En,
        _ => LangConfig::En,
    };
    state.config.update(|c| c.general.language = lang);
    state.debouncer.borrow().request();
}

/// Startup-reset-window switch の toggle を反映 + debouncer 発火。
fn apply_startup_reset_change(state: &AppStateHandles, checked: bool) {
    state
        .config
        .update(|c| c.general.startup_reset_window = checked);
    state.debouncer.borrow().request();
}

/// Remember-window-position switch の toggle を反映 + debouncer 発火。
fn apply_remember_position_change(state: &AppStateHandles, checked: bool) {
    state
        .config
        .update(|c| c.general.remember_window_position = checked);
    state.debouncer.borrow().request();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Lang;

    #[test]
    fn build_returns_non_panicking_tree_for_both_languages() {
        let state = crate::state::for_testing();
        let _ja = build(Lang::Ja.strings(), &state);
        let _en = build(Lang::En.strings(), &state);
    }

    #[test]
    fn language_selection_ja_updates_config_and_requests_save() {
        let state = crate::state::for_testing();
        assert_eq!(state.config.get().general.language, LangConfig::En); // default
        apply_language_selection(&state, LANG_JA_LABEL);
        assert_eq!(state.config.get().general.language, LangConfig::Ja);
        assert!(state.debouncer.borrow().has_pending());
    }

    #[test]
    fn language_selection_en_reverts_back_to_en() {
        let state = crate::state::for_testing();
        state.config.update(|c| c.general.language = LangConfig::Ja);
        apply_language_selection(&state, LANG_EN_LABEL);
        assert_eq!(state.config.get().general.language, LangConfig::En);
        assert!(state.debouncer.borrow().has_pending());
    }

    #[test]
    fn language_selection_unknown_label_falls_back_to_en() {
        let state = crate::state::for_testing();
        state.config.update(|c| c.general.language = LangConfig::Ja);
        apply_language_selection(&state, "Zzz future locale");
        assert_eq!(state.config.get().general.language, LangConfig::En);
    }

    #[test]
    fn startup_reset_toggle_propagates_and_requests_save() {
        let state = crate::state::for_testing();
        assert!(!state.config.get().general.startup_reset_window); // default false
        apply_startup_reset_change(&state, true);
        assert!(state.config.get().general.startup_reset_window);
        assert!(state.debouncer.borrow().has_pending());
        apply_startup_reset_change(&state, false);
        assert!(!state.config.get().general.startup_reset_window);
    }

    #[test]
    fn remember_position_toggle_propagates_and_requests_save() {
        let state = crate::state::for_testing();
        assert!(state.config.get().general.remember_window_position); // default true
        apply_remember_position_change(&state, false);
        assert!(!state.config.get().general.remember_window_position);
        assert!(state.debouncer.borrow().has_pending());
    }
}
