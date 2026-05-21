//! Widgets showcase — Inputs group (Phase 3b 第4波)。
//!
//! TextInput / Slider / Dropdown / SpinButton を label 付き row で並べ、user が
//! テーマ切替しながら rest / hover / press / focus を実機評価する場。
//! group 見出し "Inputs" は coordinator (mod.rs) が付けるため本ファイルでは付けない。
//! NOTE (disabled): これら 4 widget は disabled() API を持たない（hayate-kit 未提供、
//! 第3波 Gap B と同根の別 framework タスク）。今回は normal 状態のみ。

use hayate_kit::prelude::*;

use crate::lang::Strings;
use crate::state::AppStateHandles;

/// Build Inputs group showcase。
///
/// `strings` / `state` は本 group では未使用（showcase は persistence 不要、
/// callback は no-op で評価は視覚/触感）。
pub fn build(_strings: &'static Strings, _state: &AppStateHandles) -> Box<dyn Widget> {
    // Text input — placeholder + 固定幅（gallery text_input.rs 準拠）。
    let text_input = TextInputWidget::new()
        .with_placeholder("Type here...")
        .with_width(240.0);

    // Slider — continuous（0–100, value 30）と stepped（0–10, step 1）で挙動差を見せる。
    let slider = SliderWidget::new(0.0, 100.0, 30.0).on_change(|_v| {});
    let slider_step = SliderWidget::new(0.0, 10.0, 5.0).with_step(1.0);

    // Dropdown — 4 項目、初期選択 0。
    let dropdown = DropdownWidget::new(vec![
        "Apple".to_string(),
        "Orange".to_string(),
        "Grape".to_string(),
        "Strawberry".to_string(),
    ])
    .with_selected(0)
    .on_select(|_i, _s| {});

    // SpinButton — 0–100, value 10, step 1。
    let spin = SpinButtonWidget::new(0.0, 100.0, 10.0, 1.0).on_change(|_v| {});

    let form = FormLayout::new()
        .row("Text input", text_input)
        .row("Slider (continuous)", slider)
        .row("Slider (stepped)", slider_step)
        .row("Dropdown", dropdown)
        .row("Spin button", spin);

    Box::new(form)
}
