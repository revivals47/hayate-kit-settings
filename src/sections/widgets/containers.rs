//! Widgets showcase — Containers group (Phase 3b 第4波)。
//!
//! Phase 1 scaffolding stub（worker3 担当、 実 containers 実装は Phase 2）。
//! Tab / GroupBox / ScrollBar を並べる。 本ファイルの build() 中身のみ
//! 埋めればよい（mod.rs / 他 group は非編集）。

use hayate_kit::prelude::*;

use crate::lang::Strings;
use crate::state::AppStateHandles;

/// Build Containers group showcase。 Phase 1 stub（= 単一 placeholder label）。
pub fn build(_strings: &'static Strings, _state: &AppStateHandles) -> Box<dyn Widget> {
    Box::new(LabelWidget::new("Containers: (stub — Phase 2 で実装)", 12.0))
}
