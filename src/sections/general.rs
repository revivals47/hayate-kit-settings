//! General section (= RFC v0.5 §5.2.1)。
//!
//! wave 0 = "Coming soon" stub、 wave 1 worker dispatch で fill。
//!
//! ## planned fields (= wave 1 fill 予定):
//! - Language picker (Lang::Ja / Lang::En toggle)
//! - Startup behavior toggle (= 起動時 reset window pos / restore last state)
//! - Window remember position toggle
//! - 各 field の immediate save trigger (= persistence layer dep)

use hayate_kit::widget::form_layout::FormLayout;
use hayate_kit::widget::label::LabelWidget;
use hayate_kit::widget::layout::VStack;
use hayate_kit::Widget;

use crate::lang::Strings;

/// Build General section widget tree。
///
/// wave 0 stub = VStack { heading + "Coming soon" FormLayout row }。
pub fn build(strings: &'static Strings) -> Box<dyn Widget> {
    let heading = LabelWidget::new(strings.section_general, 18.0);
    let coming_soon = LabelWidget::new(strings.coming_soon, 14.0);
    let form = FormLayout::new().row(strings.section_general, coming_soon);

    let mut stack = VStack::new(16.0);
    stack = stack.add(Box::new(heading));
    stack = stack.add(Box::new(form));
    Box::new(stack)
}
