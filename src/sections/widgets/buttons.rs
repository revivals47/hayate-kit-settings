//! Widgets showcase — Buttons group (Phase 3b 第4波)。
//!
//! Phase 1 scaffolding stub。 worker が ButtonWidget を normal / disabled /
//! (default) で並べて Gap B（無効が通常と同じ見た目）を可視化する。
//! 本ファイルの build() 中身のみ埋めればよい（mod.rs / 他 group は非編集）。

use hayate_kit::prelude::*;

use crate::lang::Strings;
use crate::state::AppStateHandles;

/// Build Buttons group showcase。 Phase 1 stub（= 単一 placeholder label）。
pub fn build(_strings: &'static Strings, _state: &AppStateHandles) -> Box<dyn Widget> {
    Box::new(LabelWidget::new("Buttons: (stub — Phase 2 で実装)", 12.0))
}
