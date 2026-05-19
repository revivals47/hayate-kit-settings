//! IME section (= RFC v0.5 §5.2.4)。
//!
//! wave 0 = "Coming soon" stub、 wave 1 worker dispatch で fill。
//!
//! ## planned fields (= wave 1 fill 予定):
//! - IME backend dropdown (= ibus / fcitx5、 fcitx5 known limitation note)
//! - Candidate window position toggle (= cursor anchor / widget bottom、
//!   track-ime-popup-x-anchor debt link)
//! - Preedit display style toggle (= inline / floating)
//!
//! ## DTP reuse note
//! IME section は DTP app (= 縦書き IME 含む future) でも reuse 想定、
//! [[project_dtp_app_roadmap]] forward path。

use hayate_kit::widget::form_layout::FormLayout;
use hayate_kit::widget::label::LabelWidget;
use hayate_kit::widget::layout::VStack;
use hayate_kit::Widget;

use crate::lang::Strings;

pub fn build(strings: &'static Strings) -> Box<dyn Widget> {
    let heading = LabelWidget::new(strings.section_ime, 18.0);
    let coming_soon = LabelWidget::new(strings.coming_soon, 14.0);
    let form = FormLayout::new().row(strings.section_ime, coming_soon);

    let mut stack = VStack::new(16.0);
    stack = stack.add(Box::new(heading));
    stack = stack.add(Box::new(form));
    Box::new(stack)
}
