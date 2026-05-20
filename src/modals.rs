//! Modal dialog instances (= RFC v0.5 §5.2.8) — coordinator module。
//!
//! wave 3c-pre で 1 file (1328 行) を機能別 submodule に split (pure refactor、
//! behavior 変更ゼロ)。 wave 3c で track1=reset / track2=accent が別 file を
//! 編集することで modals.rs 単一 file への同時 touch conflict を解消する
//! prerequisite。
//!
//! ## submodule 構成
//! - [`overlay`]: [`ReactiveOverlayContainer`] (= State<bool> 観測 → OverlayContainer
//!   show/hide forward) + `LabelRef` (= Rc<RefCell<LabelWidget>> 共有 mutate shim)
//! - [`wcag`]: relative luminance / contrast ratio / hex parser / preview /
//!   WCAG status helpers (= accent picker preview + contrast check)
//! - [`accent`]: [`build_accent_picker`] + [`apply_accent_picker`] /
//!   [`cancel_accent_picker`] (= accent color hex 入力 modal、 後続 track2 領域)
//! - [`reset`]: [`build_reset_confirm`] + [`apply_reset_confirm`] /
//!   [`cancel_reset_confirm`] (= destructive action gate、 後続 track1 領域)
//!
//! ## public path 保全
//! `pub use` re-export で `crate::modals::{build_accent_picker, apply_accent_picker,
//! build_reset_confirm, apply_reset_confirm, ReactiveOverlayContainer, ...}` の
//! 既存 import path を維持 (= main.rs / sections 側の import 無変更)。
//!
//! ## DTP reuse
//! DTP app destructive action gate (= 例: ファイル削除確認 / 設定 reset / 編集
//! 取消) + typography color picker でも reuse 想定、 4 action helper signature
//! pattern (= `fn <action>_<modal>(state[, input])` ) は universal。

mod accent;
mod overlay;
mod reset;
mod wcag;

// ── 共有定数 (= submodule が `use super::` で参照) ──

/// HAYATE Original default accent (= `#5A8BA8` 風藍、 RFC v0.2 §3 design language)。
const DEFAULT_ACCENT_HEX: &str = "#5A8BA8";
/// HAYATE Original default accent-base RGB (= `#5A8BA8`)。
const DEFAULT_ACCENT_RGB: (u8, u8, u8) = (90, 139, 168);
/// HAYATE Original default surface-raised RGB (= `#FFFFFF`)。
const DEFAULT_SURFACE_RGB: (u8, u8, u8) = (255, 255, 255);

// ── public path 保全 re-export (= 既存 import を壊さない) ──

pub(crate) use overlay::ReactiveOverlayContainer;

pub use accent::build_accent_picker;
pub(crate) use accent::apply_accent_picker;

pub use reset::build_reset_confirm;
pub(crate) use reset::apply_reset_confirm;

// `cancel_*` は現状 build_* の on_click closure 内 (= 各 submodule 内) でのみ
// 使用、 main.rs 等の外部 caller は未存在。 wave 3c で track1=reset /
// track2=accent が Escape / Cancel ボタンを main.rs 側から明示 wire する際に
// `crate::modals::cancel_*` path で reach する想定のため、 public path を
// 先行保全して re-export を維持する (= add_overlay_with_state と同じ premature
// capability 除去回避方針)。 現時点で外部未使用のため allow(unused_imports)。
#[allow(unused_imports)]
pub(crate) use accent::cancel_accent_picker;
#[allow(unused_imports)]
pub(crate) use reset::cancel_reset_confirm;
