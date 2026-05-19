//! Appearance section (= RFC v0.5 §5.2.2)。
//!
//! wave 0 = "Coming soon" stub、 wave 1 worker dispatch で fill。
//!
//! ## planned fields (= wave 1 fill 予定、 Theme は Phase 3 hero feature defer):
//! - Font size scale (= a11y、 modular scale base 14 ± offset)
//! - Color mode (= light / dark / system follow、 Phase 4 で system follow)
//! - Accent color custom hex 入力 + WCAG contrast check + warning UI (R11)

use hayate_kit::widget::form_layout::FormLayout;
use hayate_kit::widget::label::LabelWidget;
use hayate_kit::widget::layout::VStack;
use hayate_kit::Widget;

use crate::lang::Strings;

pub fn build(strings: &'static Strings) -> Box<dyn Widget> {
    let heading = LabelWidget::new(strings.section_appearance, 18.0);
    let coming_soon = LabelWidget::new(strings.coming_soon, 14.0);
    let form = FormLayout::new().row(strings.section_appearance, coming_soon);

    let mut stack = VStack::new(16.0);
    stack = stack.add(Box::new(heading));
    stack = stack.add(Box::new(form));
    Box::new(stack)
}
