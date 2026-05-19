//! IME section (= RFC v0.5 §5.2.4)。
//!
//! wave 1 = 3 ComboBox rows + fcitx5 known limitation note。
//!
//! ## fields (= 本 file で実装)
//! - IME backend ComboBox (= ibus / fcitx5、fcitx5 known limitation note 表示)
//! - Candidate window position ComboBox (= cursor anchor / widget bottom、
//!   track-ime-popup-x-anchor debt link)
//! - Preedit display style ComboBox (= inline / floating)
//!
//! ## DTP reuse note
//! IME section は DTP app (= 縦書き IME 含む future) でも reuse 想定、
//! [[project_dtp_app_roadmap]] forward path。本 file の wire pattern が
//! そのまま縦書き IME の backend 選択にも適用可能。
//!
//! ## wave 境界
//! wave 1 scope = widget composition のみ。reactive bind (= persistence layer
//! 経由 immediate save、IME runtime swap) は wave 2 dep、現状 on_select は
//! no-op placeholder。各 field label は hardcoded literal (= lang.rs touch
//! 禁止、wave 2/3 で lang.rs 拡張 → 統合)。

use hayate_kit::widget::combo_box::ComboBoxWidget;
use hayate_kit::widget::form_layout::FormLayout;
use hayate_kit::widget::label::LabelWidget;
use hayate_kit::widget::layout::VStack;
use hayate_kit::Widget;

use crate::lang::Strings;
use crate::state::AppStateHandles;

/// ## wave 3a signature 拡張
/// `_state: &AppStateHandles` を受け取るのは wave 3b で各 ComboBox on_select →
/// state.config.update → debouncer.request を配線するため。 wave 3a 時点では
/// 未使用 (`_` prefix で warning suppress)。
pub fn build(strings: &'static Strings, _state: &AppStateHandles) -> Box<dyn Widget> {
    let heading = LabelWidget::new(strings.section_ime, 18.0);

    let backend = ComboBoxWidget::new(vec!["ibus".to_string(), "fcitx5".to_string()]);
    let candidate_pos = ComboBoxWidget::new(vec![
        "Cursor anchor".to_string(),
        "Widget bottom".to_string(),
    ]);
    let preedit_style = ComboBoxWidget::new(vec!["Inline".to_string(), "Floating".to_string()]);

    let form = FormLayout::new()
        .row("IME backend", backend)
        .row("Candidate position", candidate_pos)
        .row("Preedit style", preedit_style);

    // fcitx5 known limitation note (= 既存 GUI_kit dogfood で verify 済の制約)
    let fcitx5_note = LabelWidget::new(
        "Note: fcitx5 = preedit display 一部制約あり (= track-ime-popup-x-anchor debt 参照)",
        12.0,
    );

    let mut stack = VStack::new(16.0);
    stack = stack.add(Box::new(heading));
    stack = stack.add(Box::new(form));
    stack = stack.add(Box::new(fcitx5_note));
    Box::new(stack)
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
        // smoke: build() returns Box<dyn Widget> without panic。
        // 内部構造は VStack { heading + FormLayout (3 ComboBox) + limitation note Label }。
    }
}
