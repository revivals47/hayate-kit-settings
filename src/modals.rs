//! Modal dialog instances (= RFC v0.5 §5.2.8、 Phase 2 wave 2 stub)。
//!
//! wave 2 で 2 instance を fill 予定:
//! - `build_accent_picker(strings) -> Box<dyn Widget>` (= accent color hex 入力 +
//!   preview + WCAG contrast check warning UI)
//! - `build_reset_confirm(strings) -> Box<dyn Widget>` (= 全 section reset 等の
//!   destructive action gate)
//!
//! 既存 popup framework + `AlertDialog` widget を base、 settings-specific
//! styling のみ追加。 「framework 化」 (= 汎用化 / extension point / event 設計)
//! は Phase 4 へ defer 明示、 本 wave では 2 instance を land に scope 限定
//! (= YAGNI 回避)。
//!
//! ## wave 境界
//! wave 2 scope = 2 modal instance widget composition のみ。 modal の caller
//! 配線 (= 例: appearance.rs accent color row click → build_accent_picker show)
//! は wave 3 integration dep。
//!
//! ## DTP reuse
//! DTP app destructive action gate (= 例: ファイル削除確認 / 設定 reset / 編集
//! 取消) でも reuse 想定、 modal popup pattern は universal。

use hayate_kit::widget::label::LabelWidget;
use hayate_kit::widget::layout::VStack;
use hayate_kit::Widget;

#[allow(unused_imports)] // wave 2 で `Strings` field を使う、 stub では未使用
use crate::lang::Strings;

/// Accent color picker modal (= wave 2 fill 予定)。
///
/// wave 2 stub: VStack に「Coming soon: accent color picker」 label を 1 件のみ。
#[allow(dead_code)] // wave 2 で main.rs / appearance.rs から呼ばれる、 stub では未使用
pub fn build_accent_picker(_strings: &'static Strings) -> Box<dyn Widget> {
    let mut stack = VStack::new(8.0);
    stack = stack.add(Box::new(LabelWidget::new(
        "Coming soon: accent color picker modal (Phase 2 wave 2 fill 予定)",
        14.0,
    )));
    Box::new(stack)
}

/// Reset confirm modal (= wave 2 fill 予定)。
///
/// wave 2 stub: VStack に「Coming soon: reset confirm」 label を 1 件のみ。
#[allow(dead_code)] // wave 2 で main.rs / advanced.rs から呼ばれる、 stub では未使用
pub fn build_reset_confirm(_strings: &'static Strings) -> Box<dyn Widget> {
    let mut stack = VStack::new(8.0);
    stack = stack.add(Box::new(LabelWidget::new(
        "Coming soon: reset confirm modal (Phase 2 wave 2 fill 予定)",
        14.0,
    )));
    Box::new(stack)
}
