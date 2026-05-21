//! Widgets showcase — Containers group (Phase 3b 第4波)。
//!
//! Tab / GroupBox / ScrollBar を label 付き row で並べ、user がテーマ切替しながら
//! rest / hover / press / focus を実機評価する場。group 見出し "Containers" は
//! coordinator (mod.rs) が付けるため本ファイルでは付けない。
//!
//! NOTE (disabled): これら container widget も `disabled()` / `set_enabled()` API を
//! 持たない（hayate-kit 未提供、第3波 Gap B と同根の別 framework タスク）。今回は
//! normal 状態のみ。
//!
//! `ScrollBar` / `ScrollBarOrientation` は prelude 未収録のため full path で参照
//! （prelude 追加は cleanliness の別 PR、phase2 では GUI_kit 非編集で settings のみで
//! 完結させる方針。feedback.rs の `TooltipWidget` full path と同方針）。

use hayate_kit::prelude::*;
use hayate_kit::widget::scroll_bar::{ScrollBar, ScrollBarOrientation};

use crate::lang::Strings;
use crate::state::AppStateHandles;

/// Build Containers group showcase。
///
/// `strings` / `state` は本 group では未使用（showcase は persistence 不要、
/// callback は no-op で評価は視覚/触感）。engine は `with_engine` を呼ばず
/// settings tree の `inject_engine` に依存する（既存 section と同じ規範）。
pub fn build(_strings: &'static Strings, _state: &AppStateHandles) -> Box<dyn Widget> {
    // ── Tab view: 3 tab、初期選択 0。各 tab content は簡単な label。──
    // テーマ別に tab bar / 選択 tab の strip・bevel・色を評価（win95 = 立体 tab、
    // win10 = flat underline 等）。on_change は showcase なので no-op。
    let tabs = TabViewWidget::new()
        .add_tab(TabEntry::new(
            "General",
            Box::new(LabelWidget::new("General tab content", 13.0)),
        ))
        .add_tab(TabEntry::new(
            "Details",
            Box::new(LabelWidget::new("Details tab content", 13.0)),
        ))
        .add_tab(TabEntry::new(
            "About",
            Box::new(LabelWidget::new("About tab content", 13.0)),
        ))
        .on_change(|_i| {});

    // ── Group box: label 付き枠が内容を囲う。win95 は etched 2-line frame、
    // modern は flat 1-line frame をテーマ別に評価。中身は checkbox + label。──
    let mut gb_inner = VStack::new(6.0);
    gb_inner = gb_inner.add(Box::new(CheckboxWidget::new("Enable feature")));
    gb_inner = gb_inner.add(Box::new(LabelWidget::new("Grouped content", 13.0)));
    let group = GroupBoxWidget::new("Group box", Box::new(gb_inner));

    // ── Scroll bar: 横向き ScrollBar。content > viewport で thumb を表示。──
    // FormLayout row は幅を与える → 横向きが自然（縦向きは row 高さに潰れて thumb が
    // 見えない）。win95 テーマでは dither track + 両端 arrow cap + raised thumb bevel が
    // 出る。content/viewport は静的値で thumb 比率 = viewport/content ≈ 1/3。
    let mut hbar = ScrollBar::new(ScrollBarOrientation::Horizontal);
    hbar.set_content_size(300.0);
    hbar.set_viewport_size(100.0);

    let form = FormLayout::new()
        .row("Tab view", tabs)
        .row("Group box", group)
        .row("Scroll bar (horizontal)", hbar);

    Box::new(form)
}
