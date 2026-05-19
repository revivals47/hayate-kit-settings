//! Search bar widget (= RFC v0.5 §5.2.7、 Phase 2 wave 2 stub)。
//!
//! wave 2 で fill 予定:
//! - `build(strings) -> Box<dyn Widget>` = text input + filter callback、
//!   matched section selection auto-trigger
//! - multi-language search (= JA + EN 両方 strings から検索)
//! - simple linear search (= 全 field ~50 件想定、 R17 mitigation = trie
//!   indexing は Phase 4 defer)
//!
//! ## wave 境界
//! wave 2 scope = Search bar widget composition + filter callback signature
//! 定義のみ。 actual section selection auto-trigger の reactive bind (= TreeView
//! の selection state 更新) は wave 3 integration dep。
//!
//! main.rs `build_sidebar` の VStack で TreeView の上に配置予定 (= wave 2 で
//! worker3 が build_sidebar 更新)。
//!
//! ## DTP reuse
//! DTP app preference dialog (= 大量 settings 横断検索) でも reuse 想定、
//! Search bar pattern は universal。

use hayate_kit::widget::label::LabelWidget;
use hayate_kit::Widget;

#[allow(unused_imports)] // wave 2 で `Strings` field を使う、 stub では未使用
use crate::lang::Strings;

/// Search bar widget (= wave 2 fill 予定)。
///
/// wave 2 stub: 「Coming soon: search bar」 label を 1 件のみ返却。
#[allow(dead_code)] // wave 2 で main.rs build_sidebar に integrate される、 stub では未使用
pub fn build(_strings: &'static Strings) -> Box<dyn Widget> {
    Box::new(LabelWidget::new(
        "Coming soon: search bar (Phase 2 wave 2 fill 予定)",
        12.0,
    ))
}
