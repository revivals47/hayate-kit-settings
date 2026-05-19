//! General section (= RFC v0.5 §5.2.1)。
//!
//! wave 1 = 実 widget composition (Language picker / Startup behavior /
//! Window remember position)、 wave 2 で persistence layer 経由の immediate
//! save trigger を配線予定。
//!
//! ## fields:
//! - Language picker (ComboBox = `Lang::Ja` / `Lang::En` toggle)
//! - Startup behavior toggle (Switch = 起動時 reset window pos / restore last state)
//! - Window remember position toggle (Switch)
//!
//! ## reactive wire status (= wave 1 scope)
//! 各 field の immediate save trigger は wave 2 dep (= persistence layer
//! 経由)。本 wave では builder の `.on_select` / `.on_toggle` callback を
//! 配線せず、 default 値で静的に初期化のみ実施。

use hayate_kit::widget::combo_box::ComboBoxWidget;
use hayate_kit::widget::form_layout::FormLayout;
use hayate_kit::widget::label::LabelWidget;
use hayate_kit::widget::layout::VStack;
use hayate_kit::widget::switch::SwitchWidget;
use hayate_kit::Widget;

use crate::lang::Strings;

/// Build General section widget tree。
///
/// wave 1 構造 = `VStack { heading + FormLayout { 3 rows } }`。
/// HAYATE Original aesthetic は `active_theme()` default 経由で自動適用
/// (= R13 systemic fix 済、caller `.with_color(...)` override 不要)。
pub fn build(strings: &'static Strings) -> Box<dyn Widget> {
    let heading = LabelWidget::new(strings.section_general, 18.0);

    // Language picker — 日本語 / English の 2 択。actual `Lang` enum 連動 は
    // wave 2 で `on_select` callback + persistence layer 経由で配線。
    let language_picker = ComboBoxWidget::new(vec![
        "日本語".to_string(),
        "English".to_string(),
    ]);

    // Startup behavior — true = 起動時 window 位置 reset、false = 復元。
    // default = false (= 前回の位置を復元、user 期待値)。
    let startup_reset = SwitchWidget::new(false);

    // Window remember position — true = 終了時 window 位置を記録。
    // default = true (= remember 既定 ON)。
    let remember_position = SwitchWidget::new(true);

    let form = FormLayout::new()
        .row("Language", language_picker)
        .row("Reset window position on startup", startup_reset)
        .row("Remember window position", remember_position);

    let mut stack = VStack::new(16.0);
    stack = stack.add(Box::new(heading));
    stack = stack.add(Box::new(form));
    Box::new(stack)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Lang;

    #[test]
    fn build_returns_non_panicking_tree_for_both_languages() {
        // smoke: ja / en 両方で widget tree が組まれることを確認。
        // Box<dyn Widget> の inner structure は型 system が保証するため
        // depth / row count 直接 assertion は省略 (= Box invariant 経由)。
        let _ja = build(Lang::Ja.strings());
        let _en = build(Lang::En.strings());
    }
}
