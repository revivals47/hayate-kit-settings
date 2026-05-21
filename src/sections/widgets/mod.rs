//! Widgets section (= Phase 3b 第4波 — gallery 機能を settings に統合した
//! widget showcase)。
//!
//! 全 widget を state(rest / hover / press / focus / disabled)込みで縦に並べ、
//! user がテーマ切替しながら behavioral fidelity を実機評価する場。
//! **disabled を並べることで Gap B(無効が通常と同じ見た目)も視覚露呈**し、
//! user 確認 → 実装 → golden bless の loop が settings 内で完結する。
//!
//! ## widget group 別サブファイル（並走 worker の競合回避単位）
//! - [`buttons`]: ButtonWidget（normal / disabled 等、Gap B 可視化）
//! - [`selections`]: Checkbox / Radio / Switch
//! - [`inputs`]: Input / Slider / Dropdown / SpinButton
//! - [`feedback`]: Progress / Tooltip
//! - [`containers`]: Tab / GroupBox / ScrollBar
//!
//! ## 契約（各 group の build シグネチャ）
//! 各 group ファイルは既存 section と同形の
//! `pub fn build(strings: &'static Strings, state: &AppStateHandles) -> Box<dyn Widget>`
//! を公開する。 [`build`] coordinator が各 group の build() を呼び、 group 見出し
//! label を添えて VStack に縦積みする。 worker は自 group ファイルの build() 中身を
//! 埋めるだけでよく、 本 mod.rs と他 group ファイルは編集不要（= scaffolding 集約）。
//!
//! ## L2 厳守
//! hayate-kit 公開 API のみ使用（hayate_platform 直 dep 禁止、 settings 規範）。
//! テーマ切替は settings の `set_bundle` が tree 全体を自動再 theme するため
//! showcase 側に追加配線は不要。

pub mod buttons;
pub mod selections;
pub mod inputs;
pub mod feedback;
pub mod containers;

use hayate_kit::prelude::*;

use crate::lang::Strings;
use crate::state::AppStateHandles;

/// Build Widgets showcase section widget tree。
///
/// section 見出しに続けて、 各 widget group を「group 見出し label + group の
/// build() 結果」の順で VStack に縦積みする。 group の並び順 = buttons →
/// selections → inputs → feedback → containers。
pub fn build(strings: &'static Strings, state: &AppStateHandles) -> Box<dyn Widget> {
    let mut stack = VStack::new(16.0);
    stack = stack.add(Box::new(LabelWidget::new(strings.section_widgets, 18.0)));

    stack = stack.add(Box::new(LabelWidget::new("Buttons", 16.0)));
    stack = stack.add(buttons::build(strings, state));

    stack = stack.add(Box::new(LabelWidget::new("Selections", 16.0)));
    stack = stack.add(selections::build(strings, state));

    stack = stack.add(Box::new(LabelWidget::new("Inputs", 16.0)));
    stack = stack.add(inputs::build(strings, state));

    stack = stack.add(Box::new(LabelWidget::new("Feedback", 16.0)));
    stack = stack.add(feedback::build(strings, state));

    stack = stack.add(Box::new(LabelWidget::new("Containers", 16.0)));
    stack = stack.add(containers::build(strings, state));

    Box::new(stack)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Lang;

    #[test]
    fn build_returns_non_panicking_tree_for_both_languages() {
        let state = crate::state::for_testing();
        let _ja = build(Lang::Ja.strings(), &state);
        let _en = build(Lang::En.strings(), &state);
    }
}
