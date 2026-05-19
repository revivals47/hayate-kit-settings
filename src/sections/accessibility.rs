//! Accessibility section (= RFC v0.5 §5.2.3)。
//!
//! wave 0 = "Coming soon" stub、 wave 1 worker dispatch で fill。
//!
//! ## planned fields (= wave 1 fill 予定):
//! - Screen reader integration toggle (AccessKit + Orca on/off)
//! - High contrast mode toggle (= Phase 4 へ defer 想定)
//! - Reduce motion toggle (= 全 transition 150ms → 0ms)
//! - Keyboard navigation hint visualization toggle (focus ring 強調)

use hayate_kit::widget::form_layout::FormLayout;
use hayate_kit::widget::label::LabelWidget;
use hayate_kit::widget::layout::VStack;
use hayate_kit::Widget;

use crate::lang::Strings;

pub fn build(strings: &'static Strings) -> Box<dyn Widget> {
    let heading = LabelWidget::new(strings.section_accessibility, 18.0);
    let coming_soon = LabelWidget::new(strings.coming_soon, 14.0);
    let form = FormLayout::new().row(strings.section_accessibility, coming_soon);

    let mut stack = VStack::new(16.0);
    stack = stack.add(Box::new(heading));
    stack = stack.add(Box::new(form));
    Box::new(stack)
}
