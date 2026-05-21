//! Widgets showcase — Buttons group (Phase 3b 第4波)。
//!
//! ButtonWidget を normal / disabled で並べ、user が press/hover/focus を実機
//! 評価できる場。disabled を normal と横並びにすることで「無効が通常と同じ
//! 見た目」(Gap B、modern skin で顕著) を視覚的に露呈させる。
//! group 見出し "Buttons" は coordinator (mod.rs) が付けるため本ファイルでは
//! 付けない。

use hayate_kit::prelude::*;

use crate::lang::Strings;
use crate::state::AppStateHandles;

/// Build Buttons group showcase。
///
/// `strings` / `state` は本 group では未使用（showcase は persistence 不要）。
pub fn build(_strings: &'static Strings, _state: &AppStateHandles) -> Box<dyn Widget> {
    // normal と disabled を横並び（Gap B: 無効が通常と区別つくか user が実機確認）。
    // ButtonWidget は disabled() を持つので両状態を出せる。
    let normal = ButtonWidget::new("Normal");
    let disabled = ButtonWidget::new("Disabled").disabled();
    let mut state_row = HStack::new(8.0);
    state_row = state_row.add(Box::new(normal));
    state_row = state_row.add(Box::new(disabled));

    // press/hover/focus を触って評価する単体（disabled と挙動を比較）。
    let interactive = ButtonWidget::new("Press / Hover / Focus");

    // NOTE (default button): ButtonWidget に既定ボタン styling API が無い
    // （不足 API-4 / wave-3 win95-behavior G1 = 既定ボタン外周黒枠）。実装後に
    // .row("Default", ButtonWidget::new("OK").default()) を追加する。今回は省略。
    let form = FormLayout::new()
        .row("State (Gap B): normal vs disabled", state_row)
        .row("Interactive", interactive);

    Box::new(form)
}
