//! Advanced section (= RFC v0.5 §5.2.5)。
//!
//! ## fields (= RFC v0.5 §5.2.5):
//! - Debug overlay toggle = `SwitchWidget`
//! - Log level dropdown = `ComboBoxWidget` (= trace / debug / info / warn / error)
//! - Cache clear button = `ButtonWidget`
//! - Reset to defaults = 3 `ButtonWidget` を `HStack` 横並び
//! - Safe boot mode hint = `LabelWidget`
//!
//! ## wave 3b — persistence wire 配線済
//! - Debug overlay Switch + Log level ComboBox は callback 経由で
//!   `state.config.advanced` を更新 + debouncer 発火。
//! - Reset to defaults の "Reset all" は [`AppStateHandles::reset_confirm_visible`]
//!   を `true` に倒し、 reset 実行は modal 側で確認後 [`crate::persistence::reset`]
//!   を呼ぶ contract (= destructive action gate)。
//! - cache clear / reset_section / reset_field の click は per-field reactive
//!   bind が wave 3c 以降の dispatch、本 wave では `apply_reset_all_request`
//!   と同 confirm-modal トリガーに合流させる (= destructive 直接実行は禁止)。

use hayate_kit::widget::button::ButtonWidget;
use hayate_kit::widget::combo_box::ComboBoxWidget;
use hayate_kit::widget::form_layout::FormLayout;
use hayate_kit::widget::label::LabelWidget;
use hayate_kit::widget::layout::{HStack, VStack};
use hayate_kit::widget::switch::SwitchWidget;
use hayate_kit::Widget;

use crate::lang::Strings;
use crate::persistence::LogLevel;
use crate::state::{AppStateHandles, ResetKind};

// ── ComboBox label literal (= source of truth、 callback と test で共有) ──

const LOG_TRACE: &str = "trace";
const LOG_DEBUG: &str = "debug";
const LOG_INFO: &str = "info";
const LOG_WARN: &str = "warn";
const LOG_ERROR: &str = "error";

pub fn build(strings: &'static Strings, state: &AppStateHandles) -> Box<dyn Widget> {
    let heading = LabelWidget::new(strings.section_advanced, 18.0);

    // Field 1: Debug overlay toggle
    let debug_overlay_initial = state.config.get().advanced.debug_overlay;
    let debug_overlay = SwitchWidget::new(debug_overlay_initial).on_toggle({
        let s = state.clone();
        move |checked| apply_debug_overlay_change(&s, checked)
    });

    // Field 2: Log level dropdown (5 levels)
    let log_level = ComboBoxWidget::new(vec![
        LOG_TRACE.to_string(),
        LOG_DEBUG.to_string(),
        LOG_INFO.to_string(),
        LOG_WARN.to_string(),
        LOG_ERROR.to_string(),
    ])
    .on_select({
        let s = state.clone();
        move |selected| apply_log_level_selection(&s, selected)
    });

    // Field 3: Cache clear button — 粒度別 confirm の CacheClear variant を起動
    // (= 実 cache layer 不在で apply は no-op、 confirm 文言で no-op を明示)。
    let cache_clear = ButtonWidget::new("Clear cache").on_click({
        let s = state.clone();
        move || request_reset(&s, ResetKind::CacheClear)
    });

    // Field 4: Reset to defaults — 3 button が個別の粒度で confirm modal を起動
    // (= destructive action は user 確認越し、 wave 3c で粒度別に分離)。
    let reset_all = ButtonWidget::new("Reset all").on_click({
        let s = state.clone();
        move || request_reset(&s, ResetKind::All)
    });
    let reset_section = ButtonWidget::new("Reset section").on_click({
        let s = state.clone();
        // 現在表示中の section を snapshot (= modal blocking 中は selection 不変)。
        move || request_reset(&s, ResetKind::Section(*s.selected_section.get()))
    });
    let reset_field = ButtonWidget::new("Reset field").on_click({
        let s = state.clone();
        move || request_reset(&s, ResetKind::Field)
    });
    let reset_row = HStack::new(8.0)
        .add(Box::new(reset_all))
        .add(Box::new(reset_section))
        .add(Box::new(reset_field));

    // Field 5: Safe boot mode hint (= R12 mitigation explicit)
    let safe_boot_hint = LabelWidget::new(
        "Tip: launch with `--reset-config` to start in safe-boot mode (resets all settings).",
        13.0,
    );

    let form = FormLayout::new()
        .row("Debug overlay", debug_overlay)
        .row("Log level", log_level)
        .row("Cache", cache_clear)
        .row("Reset to defaults", reset_row)
        .row("Safe boot", safe_boot_hint);

    let mut stack = VStack::new(16.0);
    stack = stack.add(Box::new(heading));
    stack = stack.add(Box::new(form));
    Box::new(stack)
}

// ── on_callback helpers (= 1:1 unit-test exercise 用に抽出) ─────────────

fn apply_debug_overlay_change(state: &AppStateHandles, checked: bool) {
    state.config.update(|c| c.advanced.debug_overlay = checked);
    state.debouncer.borrow().request();
}

fn apply_log_level_selection(state: &AppStateHandles, selected: &str) {
    let Some(value) = parse_log_level(selected) else {
        return;
    };
    state.config.update(|c| c.advanced.log_level = value);
    state.debouncer.borrow().request();
}

/// Reset 系 button の click を粒度別に confirm modal へ流す (= wave 3c)。
///
/// `pending_reset` に `kind` を set + `reset_confirm_visible.set(true)` の 2 操作
/// のみ。 destructive 直接実行は禁止、 必ず user 確認 gate (= reset confirm modal)
/// を通す。 実 reset は modal の Reset 押下 / Enter accept → `apply_reset_confirm`
/// → `apply_reset(state, kind)` 経路でのみ発火。
fn request_reset(state: &AppStateHandles, kind: ResetKind) {
    state.pending_reset.set(Some(kind));
    state.reset_confirm_visible.set(true);
}

fn parse_log_level(label: &str) -> Option<LogLevel> {
    match label {
        LOG_TRACE => Some(LogLevel::Trace),
        LOG_DEBUG => Some(LogLevel::Debug),
        LOG_INFO => Some(LogLevel::Info),
        LOG_WARN => Some(LogLevel::Warn),
        LOG_ERROR => Some(LogLevel::Error),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Lang;
    use crate::sections::SectionId;

    #[test]
    fn build_smoke_does_not_panic() {
        let strings = Lang::En.strings();
        let state = crate::state::for_testing();
        let _w = build(strings, &state);
    }

    #[test]
    fn debug_overlay_toggle_propagates_and_requests_save() {
        let state = crate::state::for_testing();
        assert!(!state.config.get().advanced.debug_overlay);
        apply_debug_overlay_change(&state, true);
        assert!(state.config.get().advanced.debug_overlay);
        assert!(state.debouncer.borrow().has_pending());
    }

    #[test]
    fn log_level_selection_propagates_for_each_variant() {
        let state = crate::state::for_testing();
        // default は Info
        assert_eq!(state.config.get().advanced.log_level, LogLevel::Info);

        apply_log_level_selection(&state, LOG_TRACE);
        assert_eq!(state.config.get().advanced.log_level, LogLevel::Trace);
        apply_log_level_selection(&state, LOG_DEBUG);
        assert_eq!(state.config.get().advanced.log_level, LogLevel::Debug);
        apply_log_level_selection(&state, LOG_INFO);
        assert_eq!(state.config.get().advanced.log_level, LogLevel::Info);
        apply_log_level_selection(&state, LOG_WARN);
        assert_eq!(state.config.get().advanced.log_level, LogLevel::Warn);
        apply_log_level_selection(&state, LOG_ERROR);
        assert_eq!(state.config.get().advanced.log_level, LogLevel::Error);

        assert!(state.debouncer.borrow().has_pending());
    }

    #[test]
    fn log_level_unknown_label_is_no_op() {
        let state = crate::state::for_testing();
        state.config.update(|c| c.advanced.log_level = LogLevel::Warn);
        apply_log_level_selection(&state, "fatal"); // unknown
        assert_eq!(state.config.get().advanced.log_level, LogLevel::Warn);
        assert!(!state.debouncer.borrow().has_pending());
    }

    #[test]
    fn request_reset_opens_confirm_modal_without_mutating_config() {
        // destructive action は user 確認 modal を経由するため、 button click
        // 単独で config を書き換えない (= reset modal commit path で初めて
        // `apply_reset` を呼ぶ contract)。
        let state = crate::state::for_testing();
        let pre = state.config.get().clone();
        assert!(!*state.reset_confirm_visible.get());
        request_reset(&state, ResetKind::All);
        assert!(*state.reset_confirm_visible.get());
        assert_eq!(*state.config.get(), pre); // 不変
        assert!(!state.debouncer.borrow().has_pending()); // save も発火しない
    }

    #[test]
    fn request_reset_sets_pending_kind_per_button() {
        // 各 button が個別の ResetKind を pending_reset に set する。
        let state = crate::state::for_testing();

        request_reset(&state, ResetKind::All);
        assert_eq!(*state.pending_reset.get(), Some(ResetKind::All));

        request_reset(&state, ResetKind::Section(SectionId::Advanced));
        assert_eq!(
            *state.pending_reset.get(),
            Some(ResetKind::Section(SectionId::Advanced))
        );

        request_reset(&state, ResetKind::Field);
        assert_eq!(*state.pending_reset.get(), Some(ResetKind::Field));

        request_reset(&state, ResetKind::CacheClear);
        assert_eq!(*state.pending_reset.get(), Some(ResetKind::CacheClear));

        // いずれも config は不変 (= request は確認 gate を開くのみ)
        assert!(!state.debouncer.borrow().has_pending());
    }

    #[test]
    fn reset_section_button_snapshots_selected_section() {
        // Reset section は request 時の selected_section を snapshot する設計。
        // selected_section を Appearance にしてから request → Section(Appearance)。
        let state = crate::state::for_testing();
        state.selected_section.set(SectionId::Appearance);
        request_reset(&state, ResetKind::Section(*state.selected_section.get()));
        assert_eq!(
            *state.pending_reset.get(),
            Some(ResetKind::Section(SectionId::Appearance))
        );
    }

    #[test]
    fn parse_log_level_round_trip_every_variant() {
        assert_eq!(parse_log_level(LOG_TRACE), Some(LogLevel::Trace));
        assert_eq!(parse_log_level(LOG_DEBUG), Some(LogLevel::Debug));
        assert_eq!(parse_log_level(LOG_INFO), Some(LogLevel::Info));
        assert_eq!(parse_log_level(LOG_WARN), Some(LogLevel::Warn));
        assert_eq!(parse_log_level(LOG_ERROR), Some(LogLevel::Error));
        assert_eq!(parse_log_level("unknown"), None);
    }
}
