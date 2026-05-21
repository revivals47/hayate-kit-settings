//! Widgets showcase — Feedback group (Phase 3b 第4波)。
//!
//! Progress（determinate 2 段 + indeterminate）と Tooltip（Button を hover で
//! 説明表示）を label 付き row で並べる。group 見出し "Feedback" は coordinator
//! (mod.rs) が付けるため本ファイルでは付けない。
//! NOTE (disabled): Progress / Tooltip とも disabled() API を持たない（hayate-kit
//! 未提供、第3波 Gap B と同根）。今回は normal 状態のみ。

use hayate_kit::prelude::*;
// TooltipWidget は prelude 未収録のため full path（prelude 追加は cleanliness の
// 別 PR、phase2 では GUI_kit 非編集で settings のみで完結させる）。
use hayate_kit::widget::tooltip::TooltipWidget;

use crate::lang::Strings;
use crate::state::AppStateHandles;

/// Build Feedback group showcase。
///
/// `strings` / `state` は本 group では未使用（showcase は persistence 不要）。
pub fn build(_strings: &'static Strings, _state: &AppStateHandles) -> Box<dyn Widget> {
    // Progress — determinate 25% / 60% + indeterminate（gallery progress_bar.rs 準拠）。
    let progress_25 = ProgressBarWidget::new(0.25);
    let progress_60 = ProgressBarWidget::new(0.60);
    let progress_busy = ProgressBarWidget::indeterminate();

    // Tooltip — Button を包み hover で説明表示（gallery tooltip.rs 準拠）。
    let tip_button = Box::new(ButtonWidget::new("Hover over me"));
    let tooltip = TooltipWidget::new(tip_button, "This is a tooltip");

    let form = FormLayout::new()
        .row("Progress 25%", progress_25)
        .row("Progress 60%", progress_60)
        .row("Progress (indeterminate)", progress_busy)
        .row("Tooltip", tooltip);

    Box::new(form)
}
