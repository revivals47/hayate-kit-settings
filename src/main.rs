//! # hayate-kit-settings
//!
//! GUI_kit framework-native settings panel app (新世代 app 第 1 号)。
//! HAYATE original aesthetic + theme switcher + accessibility-first design。
//!
//! ## 規範
//! - [[feedback_new_apps_depend_on_gui_kit_only]]: hayate-kit のみ依存、
//!   hayate-platform 直接参照禁止
//! - [[feedback_dogfood_legacy_new_apps_clean_slate]]: 新世代 app stance、
//!   既存 dogfood は legacy 資産
//!
//! ## 関連 RFC
//! - `workspace/president-notes/hayate-kit-settings-rfc-v0.1.md` v0.2
//!   (GUI_kit repo 内、Phase 0 spec)
//!
//! ## Phase 1 status
//! - ✅ hayate-kit public API extension (= GUI_kit d64cd87)
//! - ✅ repo init (= 本 commit)
//! - ⏸ HAYATE Original theme implementation
//! - ⏸ minimal i18n + icon set
//! - ⏸ settings panel skeleton (App builder chain + HStack root)
//! - ⏸ safe boot mode --reset-config CLI flag
//! - ⏸ cargo test + Orca + IME smoke verify

// 規範整合性早期 verify: `use hayate_kit::...` のみ、`use hayate_platform::...` 禁止
#[allow(unused_imports)]
use hayate_kit::App;

fn main() {
    // Phase 1 placeholder: skeleton 起動確認のみ、本体は incremental 拡張
    println!("hayate-kit-settings v0.1.0 — Phase 1 skeleton (= repo init 段階)");
    println!("hayate-kit re-export check: App type imported from hayate_kit ✓");
    println!("Next: HAYATE Original theme + settings skeleton + safe boot mode");
}
