//! Widgets showcase — Buttons group (Phase 3b 第4波)。
//!
//! ButtonWidget を normal / disabled / default で並べ、user が press/hover/focus
//! を実機評価できる場。disabled を normal と横並びにすることで「無効が通常と同じ
//! 見た目」(Gap B、modern skin で顕著) を視覚的に露呈させる。default ボタンは
//! .default_button() の skin-agnostic mark で、win95 では外周黒枠が出る。
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

    // 既定ボタン（win95-behavior G1 = 外周黒枠）。.default_button() で mark する
    // だけで、framework が skin ごとに解決する: win95 は黒枠の framed variant、
    // 他 skin は通常 button に fallback。app 側に skin 分岐は書かない
    // （widget は skin 非認知、AppTheme.button_default が解決を担う）。
    let default_action = ButtonWidget::new("OK").default_button();

    let form = FormLayout::new()
        .row("State (Gap B): normal vs disabled", state_row)
        .row("Interactive", interactive)
        .row("Default (win95 = black frame)", default_action);

    Box::new(form)
}
