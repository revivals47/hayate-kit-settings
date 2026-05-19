//! Advanced section (= RFC v0.5 §5.2.5)。
//!
//! wave 0 = "Coming soon" stub、 wave 1 worker dispatch で fill。
//!
//! ## planned fields (= wave 1 fill 予定):
//! - Debug overlay toggle (= FPS / hit-test rect / widget tree visualizer)
//! - Log level dropdown (= trace / debug / info / warn / error)
//! - Cache clear button
//! - Reset to defaults button (= 全 section / 現 section / per-field の 3 button)
//! - Safe boot mode hint (= `--reset-config` CLI flag 案内、 R12 mitigation)

use hayate_kit::widget::form_layout::FormLayout;
use hayate_kit::widget::label::LabelWidget;
use hayate_kit::widget::layout::VStack;
use hayate_kit::Widget;

use crate::lang::Strings;

pub fn build(strings: &'static Strings) -> Box<dyn Widget> {
    let heading = LabelWidget::new(strings.section_advanced, 18.0);
    let coming_soon = LabelWidget::new(strings.coming_soon, 14.0);
    let form = FormLayout::new().row(strings.section_advanced, coming_soon);

    let mut stack = VStack::new(16.0);
    stack = stack.add(Box::new(heading));
    stack = stack.add(Box::new(form));
    Box::new(stack)
}
