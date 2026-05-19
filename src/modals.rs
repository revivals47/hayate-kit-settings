//! Modal dialog instances (= RFC v0.5 §5.2.8、 Phase 2 wave 3b — modal
//! lifecycle wire logic-only subset)。
//!
//! 2 instance を fill:
//! - `build_accent_picker(strings, state) -> Box<dyn Widget>` (= accent color
//!   hex 入力 + preview swatch + WCAG contrast check warning UI)
//! - `build_reset_confirm(strings, state) -> Box<dyn Widget>` (= 全 section
//!   reset 等の destructive action gate)
//!
//! ## wave 3b scope (= 本 commit、 logic-only subset)
//! 状態 wire (= 4 pub(crate) action helpers + button on_click closure → state
//! mutate) を実装、in-memory unit test で全 lifecycle path を検証。視覚 modal
//! overlay (= AlertDialog show/hide + 外部 State<bool> 経由 control + 動的
//! preview / WCAG 再計算) は framework gap (= hayate-kit から `Renderer` /
//! `ItemRect` / `TextEngine` / `alloc_widget_id` 未 re-export、custom Widget
//! impl 不可) を consolidation framework PR で先に解消後、 次 wave で追加。
//!
//! ## framework gap 背景
//! `feedback_new_apps_depend_on_gui_kit_only` 規範下では `hayate_kit::Widget`
//! trait は再 export 済だが method signature 内 `Renderer` / `ItemRect` 等が
//! 未 re-export で custom Widget impl が不可能。`AlertDialog` 視覚 lifecycle は
//! Rc<RefCell> + on_change subscription + show/hide forward に custom wrapper
//! Widget が必要、本 gap 解消が prerequisite。 同 gap は worker3 でも別文脈で
//! 同時刻に検出 = `feedback_widget_trait_forward_gap_pattern` の case 3+4 目。
//!
//! ## DTP reuse
//! DTP app destructive action gate (= 例: ファイル削除確認 / 設定 reset / 編集
//! 取消) + typography color picker でも reuse 想定、 4 action helper signature
//! pattern (= `fn <action>_<modal>(state[, input])` ) は universal。

use hayate_kit::widget::button::ButtonWidget;
use hayate_kit::widget::label::LabelWidget;
use hayate_kit::widget::layout::{HStack, VStack};
use hayate_kit::widget::text_input_widget::TextInputWidget;
use hayate_kit::Widget;

#[allow(unused_imports)] // wave 3 で `Strings` field を本格使用予定
use crate::lang::Strings;
use crate::persistence;
use crate::state::AppStateHandles;

/// HAYATE Original default accent (= `#5A8BA8` 風藍、 RFC v0.2 §3 design language)。
const DEFAULT_ACCENT_HEX: &str = "#5A8BA8";
/// HAYATE Original default accent-base RGB (= `#5A8BA8`)。
const DEFAULT_ACCENT_RGB: (u8, u8, u8) = (90, 139, 168);
/// HAYATE Original default surface-raised RGB (= `#FFFFFF`)。
const DEFAULT_SURFACE_RGB: (u8, u8, u8) = (255, 255, 255);

// ── action helpers (= wave 3b core、 全 button on_click + 全 test で共有) ──

/// Apply user-input accent hex into Config + request debounced save + dismiss
/// accent picker modal (= visible flag clear)。
///
/// `hex` は user-validated 文字列 (= `#RRGGBB` 形式想定、 validation は wave 3c
/// の reactive hex parser で実施予定、 本 wave は raw string 保存のみ)。
/// `state.config.update` で sub-field 編集、 全 section field の persistence
/// round-trip と互換。 debouncer は 500ms quiet-period で disk thrash を回避。
pub(crate) fn apply_accent_picker(state: &AppStateHandles, hex: &str) {
    state
        .config
        .update(|c| c.appearance.accent_hex = hex.to_string());
    state.debouncer.borrow().request();
    state.accent_picker_visible.set(false);
}

/// Dismiss accent picker modal without applying the draft hex (= state.config
/// 不変)。
pub(crate) fn cancel_accent_picker(state: &AppStateHandles) {
    state.accent_picker_visible.set(false);
}

/// Reset Config to defaults + persist immediately + dismiss reset confirm modal。
///
/// `persistence::reset` が disk side `config.json` を `.bak` rename + new default
/// 書き出し、 同時に in-memory state.config も `Config::default()` で揃える。
/// debouncer は使わず即時 disk 書き込み (= destructive 確認後の hard reset、
/// debounce 経由で待つ semantics が不適切)。
pub(crate) fn apply_reset_confirm(state: &AppStateHandles) -> std::io::Result<()> {
    persistence::reset(&state.config_path)?;
    state.config.set(persistence::Config::default());
    state.reset_confirm_visible.set(false);
    Ok(())
}

/// Dismiss reset confirm modal without performing reset (= state 不変)。
pub(crate) fn cancel_reset_confirm(state: &AppStateHandles) {
    state.reset_confirm_visible.set(false);
}

// ── widget composition (= wave 3b logic-only、 視覚 lifecycle は次 wave) ──

/// Accent color picker modal (= wave 3b logic-only subset)。
///
/// 構造: `VStack { hex TextInput + preview swatch Label + WCAG ratio Label +
/// HStack { Cancel ButtonWidget + Apply ButtonWidget } }`。
///
/// ## 配線
/// - Cancel button on_click: [`cancel_accent_picker`] で visible flag clear
/// - Apply button on_click: 現状 `DEFAULT_ACCENT_HEX` 固定で [`apply_accent_picker`]
///   を呼出 (= TextInput live text 読出は framework PR (= `TextInput::on_change`
///   callback + hayate_kit から `TextEngine` re-export) 完了後、 次 wave で
///   `state.draft_accent_hex: State<String>` 等を追加して動的 hex 反映予定)
/// - preview / WCAG label は default 値で静的構築 (= 動的再計算は同上理由 defer)
#[allow(dead_code)] // wave 3c 以降 main.rs から呼ばれる
pub fn build_accent_picker(_strings: &'static Strings, state: &AppStateHandles) -> Box<dyn Widget> {
    let hex_input = TextInputWidget::new()
        .with_placeholder(DEFAULT_ACCENT_HEX)
        .with_width(160.0);

    let preview = LabelWidget::new(
        format!("Preview swatch: {} (default 風藍)", DEFAULT_ACCENT_HEX),
        13.0,
    );

    let (r, g, b) = DEFAULT_ACCENT_RGB;
    let (sr, sg, sb) = DEFAULT_SURFACE_RGB;
    let ratio = contrast_ratio(relative_luminance(r, g, b), relative_luminance(sr, sg, sb));
    let aa_pass = ratio >= 4.5;
    let wcag_note = format!(
        "WCAG AA (>=4.5:1): {} vs surface = {} -- {} [dynamic re-eval: next wave]",
        DEFAULT_ACCENT_HEX,
        format_ratio(ratio),
        if aa_pass {
            "PASS"
        } else {
            "FAIL: pick a darker / lighter accent"
        },
    );
    let wcag_label = LabelWidget::new(wcag_note, 12.0);

    // Cancel + Apply buttons の closure に state clone を capture (= Rc 内部
    // 共有、 cheap clone)。 wave 3c では Apply に user-input hex を渡す経路を
    // framework PR 完了後に追加予定 (= 現状 DEFAULT_ACCENT_HEX 固定で placeholder)。
    let cancel_state = state.clone();
    let cancel_btn = ButtonWidget::new("Cancel").on_click(move || {
        cancel_accent_picker(&cancel_state);
    });
    let apply_state = state.clone();
    let apply_btn = ButtonWidget::new("Apply").on_click(move || {
        // FIXME(next wave): TextInput live text 読出は framework PR 完了後
        // (= TextInput::on_change callback + custom Widget impl 可) に追加。
        // 現状 default hex 固定で apply、 visible flag clear のみ実用化。
        apply_accent_picker(&apply_state, DEFAULT_ACCENT_HEX);
    });

    let mut buttons = HStack::new(12.0);
    buttons = buttons.add(Box::new(cancel_btn));
    buttons = buttons.add(Box::new(apply_btn));

    let mut stack = VStack::new(8.0);
    stack = stack.add(Box::new(hex_input));
    stack = stack.add(Box::new(preview));
    stack = stack.add(Box::new(wcag_label));
    stack = stack.add(Box::new(buttons));
    Box::new(stack)
}

/// Reset confirm modal (= wave 3b logic-only subset)。
///
/// 構造: `VStack { confirm message Label + HStack { Cancel ButtonWidget + Reset
/// ButtonWidget } }`。
///
/// ## 配線
/// - Cancel button on_click: [`cancel_reset_confirm`] で visible flag clear
/// - Reset button on_click: [`apply_reset_confirm`] で `persistence::reset` +
///   state.config = default + visible flag clear。 io::Error は `eprintln!`
///   で WARN 出力 + 黙って続行 (= user-visible error toast は別 widget 追加
///   必要のため次 wave defer、 disk 書込失敗時も visible flag clear で modal
///   は閉じる方針)。
#[allow(dead_code)] // wave 3c 以降 main.rs から呼ばれる
pub fn build_reset_confirm(_strings: &'static Strings, state: &AppStateHandles) -> Box<dyn Widget> {
    let message = LabelWidget::new(
        "全 settings を default に reset しますか? (この操作は取消不可)",
        14.0,
    );

    let cancel_state = state.clone();
    let cancel_btn = ButtonWidget::new("Cancel").on_click(move || {
        cancel_reset_confirm(&cancel_state);
    });
    let reset_state = state.clone();
    let reset_btn = ButtonWidget::new("Reset").on_click(move || {
        if let Err(e) = apply_reset_confirm(&reset_state) {
            eprintln!("WARN: hayate-kit-settings: reset failed: {e}");
            // disk 失敗時も visible flag は clear して modal を閉じる
            // (= 次 wave で error toast 表示と引き換え予定)
            reset_state.reset_confirm_visible.set(false);
        }
    });

    let mut buttons = HStack::new(12.0);
    buttons = buttons.add(Box::new(cancel_btn));
    buttons = buttons.add(Box::new(reset_btn));

    let mut stack = VStack::new(16.0);
    stack = stack.add(Box::new(message));
    stack = stack.add(Box::new(buttons));
    Box::new(stack)
}

// ── WCAG helper local duplicate (= sections/appearance.rs 内 helper が private
// で reuse 不可、appearance.rs touch 禁止のため modals.rs に同一実装を複製。
// wave 3c 以降で共通 module (= src/wcag.rs 等) への extract 検討予定。
// 完全 spec の gamma piecewise (= threshold 0.03928) は scope outside、
// BT.601 luminance 近似で AA threshold (4.5) 判定にのみ使用) ──

/// WCAG 2.2 §1.4.3 relative luminance (simple sRGB linearization、BT.601 近似)。
fn relative_luminance(r: u8, g: u8, b: u8) -> f32 {
    let rf = r as f32;
    let gf = g as f32;
    let bf = b as f32;
    (rf * 0.299 + gf * 0.587 + bf * 0.114) / 255.0
}

/// WCAG 2.2 §1.4.3 contrast ratio approximation (= `(L1 + 0.05) / (L2 + 0.05)`、L1 >= L2)。
fn contrast_ratio(la: f32, lb: f32) -> f32 {
    let (l1, l2) = if la >= lb { (la, lb) } else { (lb, la) };
    (l1 + 0.05) / (l2 + 0.05)
}

/// `4.7321 -> "4.73:1"` 等。 contrast ratio 表示用。
fn format_ratio(r: f32) -> String {
    format!("{:.2}:1", r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Lang;
    use crate::persistence::{Config, LogLevel};
    use crate::state::{for_testing, AppStateHandles};
    use hayate_kit::ReactiveRuntime;
    use std::path::PathBuf;

    // ── existing smoke tests (preserved baseline) ───────────────────────

    #[test]
    fn build_accent_picker_smoke() {
        let strings = Lang::En.strings();
        let state = for_testing();
        let _root = build_accent_picker(strings, &state);
    }

    #[test]
    fn build_reset_confirm_smoke() {
        let strings = Lang::En.strings();
        let state = for_testing();
        let _root = build_reset_confirm(strings, &state);
    }

    #[test]
    fn wcag_helpers_match_appearance_section_values() {
        let l_a = relative_luminance(90, 139, 168);
        let l_s = relative_luminance(255, 255, 255);
        let ratio = contrast_ratio(l_a, l_s);
        assert!(ratio.is_finite());
        assert!(ratio > 1.0);
    }

    #[test]
    fn wcag_endpoints_white_on_black_is_max_21() {
        let l_w = relative_luminance(255, 255, 255);
        let l_k = relative_luminance(0, 0, 0);
        let r = contrast_ratio(l_w, l_k);
        assert!((r - 21.0).abs() < 0.001);
    }

    // ── wave 3b lifecycle tests (= 4 action helpers + State<bool> toggle) ──

    /// Test isolation helper: per-test unique config_path で disk I/O 衝突を回避。
    fn isolated_state(label: &str) -> (AppStateHandles, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "hayate-kit-settings-modal-test-{}-{}",
            std::process::id(),
            label
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("config.json");
        let runtime = ReactiveRuntime::new();
        let state = AppStateHandles::new(&runtime, Config::default(), path.clone());
        (state, path)
    }

    fn cleanup_state(path: &std::path::Path) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
    }

    // 1. State<bool> visible toggle 観測 (= 外部 set/get で reactive flag を制御可)
    #[test]
    fn accent_picker_visible_toggle_via_state() {
        let state = for_testing();
        assert!(!*state.accent_picker_visible.get(), "initial = false");
        state.accent_picker_visible.set(true);
        assert!(*state.accent_picker_visible.get(), "set(true) reflected");
        state.accent_picker_visible.set(false);
        assert!(!*state.accent_picker_visible.get(), "set(false) reflected");
    }

    #[test]
    fn reset_confirm_visible_toggle_via_state() {
        let state = for_testing();
        assert!(!*state.reset_confirm_visible.get(), "initial = false");
        state.reset_confirm_visible.set(true);
        assert!(*state.reset_confirm_visible.get(), "set(true) reflected");
        state.reset_confirm_visible.set(false);
        assert!(!*state.reset_confirm_visible.get(), "set(false) reflected");
    }

    // 2. Apply 押下 → state.config に hex 反映 + debouncer pending + visible clear
    #[test]
    fn apply_accent_picker_writes_hex_and_dismisses() {
        let state = for_testing();
        state.accent_picker_visible.set(true);
        let new_hex = "#3D6884"; // accent-pressed (RFC v0.2 §3 から)
        apply_accent_picker(&state, new_hex);
        assert_eq!(state.config.get().appearance.accent_hex, new_hex);
        assert!(
            state.debouncer.borrow().has_pending(),
            "Apply must request debounced save"
        );
        assert!(
            !*state.accent_picker_visible.get(),
            "Apply must dismiss modal"
        );
    }

    // 3. Cancel 押下 → state.config 不変 + debouncer 不変 + visible clear
    #[test]
    fn cancel_accent_picker_leaves_config_untouched() {
        let state = for_testing();
        state.accent_picker_visible.set(true);
        let original_hex = state.config.get().appearance.accent_hex.clone();
        assert!(!state.debouncer.borrow().has_pending(), "baseline");
        cancel_accent_picker(&state);
        assert_eq!(
            state.config.get().appearance.accent_hex,
            original_hex,
            "Cancel must not mutate config"
        );
        assert!(
            !state.debouncer.borrow().has_pending(),
            "Cancel must not request save"
        );
        assert!(
            !*state.accent_picker_visible.get(),
            "Cancel must dismiss modal"
        );
    }

    // 4. Reset 押下 → config default 復元 + disk reset + .bak 作成 + visible clear
    #[test]
    fn apply_reset_confirm_restores_defaults_and_dismisses() {
        let (state, path) = isolated_state("apply-reset");

        // seed: 非 default な config を disk + state 両方へ
        state.config.update(|c| {
            c.appearance.accent_hex = String::from("#FFCC00");
            c.advanced.log_level = LogLevel::Trace;
        });
        persistence::save(&state.config.get(), &path).expect("seed save");
        state.reset_confirm_visible.set(true);

        // act: Reset 押下相当
        apply_reset_confirm(&state).expect("reset succeeds");

        // verify: in-memory state = default
        assert_eq!(*state.config.get(), Config::default(), "in-memory default");
        assert!(
            !*state.reset_confirm_visible.get(),
            "Reset must dismiss modal"
        );

        // verify: disk file = default、 .bak = pre-reset seed
        let bak = {
            let mut s = path.as_os_str().to_owned();
            s.push(".bak");
            PathBuf::from(s)
        };
        assert!(bak.exists(), ".bak must hold pre-reset seed");
        let on_disk = persistence::load(&path).expect("load post-reset");
        assert_eq!(on_disk, Config::default(), "disk default");

        cleanup_state(&path);
    }

    // 5. Cancel reset → state.config 不変 + visible clear (= destructive 回避)
    #[test]
    fn cancel_reset_confirm_leaves_config_untouched() {
        let state = for_testing();
        state.reset_confirm_visible.set(true);
        state.config.update(|c| {
            c.advanced.debug_overlay = true;
            c.appearance.accent_hex = String::from("#ABC123");
        });
        let snapshot = state.config.get().clone();
        cancel_reset_confirm(&state);
        assert_eq!(
            *state.config.get(),
            snapshot,
            "Cancel reset must not mutate config"
        );
        assert!(
            !*state.reset_confirm_visible.get(),
            "Cancel reset must dismiss modal"
        );
    }

    // 6. Visible flag は Apply / Cancel いずれの path でも必ず false に落ちる
    #[test]
    fn both_dismiss_paths_clear_visible_flag() {
        let state = for_testing();
        // Apply path
        state.accent_picker_visible.set(true);
        apply_accent_picker(&state, "#000000");
        assert!(!*state.accent_picker_visible.get());

        // Cancel path
        state.accent_picker_visible.set(true);
        cancel_accent_picker(&state);
        assert!(!*state.accent_picker_visible.get());
    }
}
