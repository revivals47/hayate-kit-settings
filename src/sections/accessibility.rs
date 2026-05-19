//! Accessibility section (= RFC v0.5 §5.2.3)。
//!
//! wave 1 = 4 toggle rows + Phase 4 defer note。
//!
//! ## fields (= 本 file で実装)
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
//! ## wave 境界
//! wave 1 scope = widget composition のみ。reactive bind (= persistence layer
//! 経由 immediate save) は wave 2 dep、現状 toggle callback は no-op
//! placeholder。各 field label は hardcoded literal (= lang.rs touch 禁止、
//! wave 2/3 で lang.rs 拡張 → 統合)。

use hayate_kit::widget::form_layout::FormLayout;
use hayate_kit::widget::label::LabelWidget;
use hayate_kit::widget::layout::VStack;
use hayate_kit::widget::switch::SwitchWidget;
use hayate_kit::Widget;

use crate::lang::Strings;

pub fn build(strings: &'static Strings) -> Box<dyn Widget> {
    let heading = LabelWidget::new(strings.section_accessibility, 18.0);

    let screen_reader = SwitchWidget::new(false);
    let high_contrast = SwitchWidget::new(false);
    let reduce_motion = SwitchWidget::new(false);
    let keyboard_hint = SwitchWidget::new(false);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Lang;

    #[test]
    fn build_returns_non_empty_widget_tree() {
        let strings = Lang::En.strings();
        let _root = build(strings);
        // smoke: build() returns Box<dyn Widget> without panic。
        // 内部構造は VStack { heading + FormLayout (4 Switch) + defer note Label }。
    }
}
