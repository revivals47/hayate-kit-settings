//! Accessibility section (= RFC v0.5 §5.2.3)。
//!
//! ## fields:
//! - Screen reader integration toggle (= AccessKit + Orca on/off)
//! - High contrast mode toggle (= Phase 4 で system follow と合わせて wire 予定、
//!   現状 UI のみで behavior は no-op)
//! - Reduce motion toggle (= 全 transition 150ms → 0ms へ落とす、Phase 3 で wire)
//! - Keyboard navigation hint visualization toggle (= focus ring 強調)
//!
//! ## DTP reuse
//! 全 a11y toggle は DTP app でも reuse 想定 (= 同 wire pattern、
//! [[project_dtp_app_roadmap]] 整合)。
//!
//! ## wave 3b — persistence wire 配線済
//! 各 Switch の `on_toggle` → [`apply_*`] helper → `state.config` 更新 +
//! debouncer 発火。closure は widget の中に閉じ込められて呼び出せないため、
//! helper fn 抽出で unit test が 1:1 に exercise する設計。

use hayate_kit::prelude::*;

use crate::lang::Strings;
use crate::state::AppStateHandles;

pub fn build(strings: &'static Strings, state: &AppStateHandles) -> Box<dyn Widget> {
    let heading = LabelWidget::new(strings.section_accessibility, 18.0);

    // 初期値は state.config から復元、 toggle で書き戻し + debouncer 発火
    let cfg = state.config.get();
    let screen_reader_initial = cfg.accessibility.screen_reader;
    let high_contrast_initial = cfg.accessibility.high_contrast;
    let reduce_motion_initial = cfg.accessibility.reduce_motion;
    let keyboard_hint_initial = cfg.accessibility.keyboard_nav_hints;
    drop(cfg);

    let screen_reader = SwitchWidget::new(screen_reader_initial).on_toggle({
        let s = state.clone();
        move |checked| apply_screen_reader_change(&s, checked)
    });
    let high_contrast = SwitchWidget::new(high_contrast_initial).on_toggle({
        let s = state.clone();
        move |checked| apply_high_contrast_change(&s, checked)
    });
    let reduce_motion = SwitchWidget::new(reduce_motion_initial).on_toggle({
        let s = state.clone();
        move |checked| apply_reduce_motion_change(&s, checked)
    });
    let keyboard_hint = SwitchWidget::new(keyboard_hint_initial).on_toggle({
        let s = state.clone();
        move |checked| apply_keyboard_hint_change(&s, checked)
    });

    let form = FormLayout::new()
        .row("Screen reader", screen_reader)
        .row("High contrast", high_contrast)
        .row("Reduce motion", reduce_motion)
        .row("Keyboard nav hint", keyboard_hint);

    // Phase 4 defer note (= High contrast は system follow と合わせて wire 予定)
    let defer_note = LabelWidget::new(
        "Note: High contrast は Phase 4 で system follow と合わせて実装予定",
        12.0,
    );

    let mut stack = VStack::new(16.0);
    stack = stack.add(Box::new(heading));
    stack = stack.add(Box::new(form));
    stack = stack.add(Box::new(defer_note));
    Box::new(stack)
}

// ── on_toggle helpers (= 1:1 unit-test exercise 用に抽出) ───────────────

fn apply_screen_reader_change(state: &AppStateHandles, checked: bool) {
    state
        .config
        .update(|c| c.accessibility.screen_reader = checked);
    state.debouncer.borrow().request();
}

fn apply_high_contrast_change(state: &AppStateHandles, checked: bool) {
    state
        .config
        .update(|c| c.accessibility.high_contrast = checked);
    state.debouncer.borrow().request();
}

fn apply_reduce_motion_change(state: &AppStateHandles, checked: bool) {
    state
        .config
        .update(|c| c.accessibility.reduce_motion = checked);
    state.debouncer.borrow().request();
}

fn apply_keyboard_hint_change(state: &AppStateHandles, checked: bool) {
    state
        .config
        .update(|c| c.accessibility.keyboard_nav_hints = checked);
    state.debouncer.borrow().request();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Lang;

    #[test]
    fn build_returns_non_empty_widget_tree() {
        let strings = Lang::En.strings();
        let state = crate::state::for_testing();
        let _root = build(strings, &state);
    }

    #[test]
    fn screen_reader_toggle_propagates_and_requests_save() {
        let state = crate::state::for_testing();
        assert!(!state.config.get().accessibility.screen_reader);
        apply_screen_reader_change(&state, true);
        assert!(state.config.get().accessibility.screen_reader);
        assert!(state.debouncer.borrow().has_pending());
    }

    #[test]
    fn high_contrast_toggle_propagates_and_requests_save() {
        let state = crate::state::for_testing();
        apply_high_contrast_change(&state, true);
        assert!(state.config.get().accessibility.high_contrast);
        assert!(state.debouncer.borrow().has_pending());
    }

    #[test]
    fn reduce_motion_toggle_propagates_and_requests_save() {
        let state = crate::state::for_testing();
        apply_reduce_motion_change(&state, true);
        assert!(state.config.get().accessibility.reduce_motion);
        assert!(state.debouncer.borrow().has_pending());
    }

    #[test]
    fn keyboard_hint_toggle_propagates_and_requests_save() {
        let state = crate::state::for_testing();
        apply_keyboard_hint_change(&state, true);
        assert!(state.config.get().accessibility.keyboard_nav_hints);
        assert!(state.debouncer.borrow().has_pending());
    }

    #[test]
    fn toggles_are_independent_across_fields() {
        // 各 helper が他 field に副作用を出さないことを smoke。
        let state = crate::state::for_testing();
        apply_screen_reader_change(&state, true);
        let cfg = state.config.get();
        assert!(cfg.accessibility.screen_reader);
        assert!(!cfg.accessibility.high_contrast);
        assert!(!cfg.accessibility.reduce_motion);
        assert!(!cfg.accessibility.keyboard_nav_hints);
    }
}
