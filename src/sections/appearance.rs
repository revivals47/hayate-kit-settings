//! Appearance section (= RFC v0.5 §5.2.2)。
//!
//! ## fields:
//! - Font size scale (Slider = base 14 ± offset、 range 10-20)
//! - Color mode (ComboBox = Light / Dark / System (Phase 4 defer))
//! - Accent color custom hex (TextInput + WCAG contrast check warning UI)
//! - Theme subsection (LabelWidget = Phase 3 で実装予定 note)
//!
//! ## R11 mitigation (= WCAG simple)
//! 完全 WCAG 2.2 spec は scope outside。WCAG helper は [`crate::modals::wcag`]
//! ([`compute_wcag_status`] / [`parse_hex_or_default`]) を再利用 (= wave 3c
//! track2 で sections / modals 間の WCAG helper 重複を解消、 BT.601 luminance
//! 近似 + AA threshold 4.5:1 判定)。
//!
//! ## wave 3b — persistence wire (complete)
//! - Slider (`on_change`) と Color mode ComboBox (`on_select`) は前置き commit で
//!   `state.config` 更新 + debouncer 発火を配線済 ([`apply_font_scale_change`] /
//!   [`apply_color_mode_selection`])。
//! - TextInput accent_hex は [`TextInputWidget::on_change`]
//!   (GUI_kit PR #155 framework gap 解消で利用解禁) push-style reactive bind で
//!   配線完了 ([`apply_accent_hex_change`])。
//!
//! ## wave 3c track2 — accent color 完成 (trigger + dynamic WCAG)
//! - **trigger**: accent color row に `Pick...` button を追加、 on_click で
//!   `state.accent_picker_visible.set(true)` ([`open_accent_picker`])。 これで
//!   従来 dormant だった accent picker modal (main.rs `ReactiveOverlayContainer`
//!   配下) が起動可能に。
//! - **dynamic WCAG**: WCAG label を [`crate::modals::LabelRef`] 経由 shared
//!   mutate handle で構築、 TextInput `on_change` ごとに [`compute_wcag_status`]
//!   で再計算して label 更新 ([`refresh_wcag_label`])。 従来 static (= default
//!   風藍固定) だった表示が入力 hex に追従。

use std::cell::RefCell;
use std::rc::Rc;

use hayate_kit::widget::button::ButtonWidget;
use hayate_kit::widget::combo_box::ComboBoxWidget;
use hayate_kit::widget::form_layout::FormLayout;
use hayate_kit::widget::label::LabelWidget;
use hayate_kit::widget::layout::{HStack, VStack};
use hayate_kit::widget::slider::SliderWidget;
use hayate_kit::widget::text_input_widget::TextInputWidget;
use hayate_kit::Widget;

use crate::lang::Strings;
use crate::modals::wcag::{compute_wcag_status, parse_hex_or_default};
use crate::modals::LabelRef;
use crate::persistence::ColorMode;
use crate::state::AppStateHandles;

/// HAYATE Original default accent (= `#5A8BA8` 風藍、 RFC v0.2 §3 design language)。
const DEFAULT_ACCENT_HEX: &str = "#5A8BA8";

// ── Color mode ComboBox labels (= source of truth、 callback + test 共有) ──

const COLOR_MODE_LIGHT: &str = "Light";
const COLOR_MODE_DARK: &str = "Dark";
const COLOR_MODE_SYSTEM: &str = "System (Phase 4 defer)";

/// Build Appearance section widget tree。
pub fn build(strings: &'static Strings, state: &AppStateHandles) -> Box<dyn Widget> {
    let heading = LabelWidget::new(strings.section_appearance, 18.0);

    // ── Font size scale ────────────────────────────────────────────────
    let font_initial = state.config.get().appearance.font_size_scale;
    let font_scale = SliderWidget::new(10.0, 20.0, font_initial).on_change({
        let s = state.clone();
        move |v| apply_font_scale_change(&s, v)
    });

    // ── Color mode ─────────────────────────────────────────────────────
    let color_mode = ComboBoxWidget::new(vec![
        COLOR_MODE_LIGHT.to_string(),
        COLOR_MODE_DARK.to_string(),
        COLOR_MODE_SYSTEM.to_string(),
    ])
    .on_select({
        let s = state.clone();
        move |selected| apply_color_mode_selection(&s, selected)
    });

    // ── WCAG label (= dynamic、 LabelRef shared mutate handle 経由) ───────
    // 初期 content は state.config の現 accent_hex base、 TextInput on_change で
    // refresh_wcag_label が再計算して set_text。 widget は VStack 投入、 handle は
    // on_change closure が capture (= modals::accent と同 LabelRef pattern)。
    let initial_accent = state.config.get().appearance.accent_hex.clone();
    let (ir, ig, ib) = parse_hex_or_default(&initial_accent);
    let wcag_label = LabelWidget::new(compute_wcag_status(&initial_accent, ir, ig, ib), 12.0);
    let (wcag_widget, wcag_handle) = LabelRef::new_pair(wcag_label);

    // ── Accent color hex + Pick... trigger ──────────────────────────────
    // TextInput on_change: (1) apply_accent_hex_change で config 更新 + debouncer、
    // (2) refresh_wcag_label で WCAG label を dynamic 再計算。
    // Pick... button: accent picker modal を起動 (= 従来 dormant の trigger)。
    let mut accent_input = TextInputWidget::new()
        .with_placeholder(DEFAULT_ACCENT_HEX)
        .with_width(160.0)
        .on_change({
            let state = state.clone();
            let wcag_handle = Rc::clone(&wcag_handle);
            move |text| {
                apply_accent_hex_change(&state, text);
                refresh_wcag_label(&wcag_handle, text);
            }
        });
    if !initial_accent.is_empty() {
        accent_input.set_text(&initial_accent);
    }

    let pick_btn = ButtonWidget::new("Pick...").on_click({
        let s = state.clone();
        move || open_accent_picker(&s)
    });
    let mut accent_row = HStack::new(8.0);
    accent_row = accent_row.add(Box::new(accent_input));
    accent_row = accent_row.add(Box::new(pick_btn));

    let form = FormLayout::new()
        .row("Font size scale", font_scale)
        .row("Color mode", color_mode)
        .row("Accent color (hex)", accent_row);

    // Theme サブセクション = Phase 3 hero feature defer の note。
    let theme_subheading = LabelWidget::new(strings.section_appearance_theme, 16.0);
    let theme_defer_note = LabelWidget::new(
        "Phase 3 で theme switcher (retro presets + HAYATE Original variants) を実装予定。",
        13.0,
    );

    let mut stack = VStack::new(16.0);
    stack = stack.add(Box::new(heading));
    stack = stack.add(Box::new(form));
    stack = stack.add(wcag_widget);
    stack = stack.add(Box::new(theme_subheading));
    stack = stack.add(Box::new(theme_defer_note));
    Box::new(stack)
}

// ── on_callback helpers (= 1:1 unit-test exercise 用に抽出) ─────────────

/// Font size scale Slider の `on_change` を反映 + debouncer 発火。
///
/// SliderWidget の `with_step` を呼んでいないため連続値で来る (= drag 中に
/// 細かく callback 発火)。 debounced save と組合せて disk thrash を回避する
/// contract、 本 helper 単体では毎呼び出しで request() を叩く。
fn apply_font_scale_change(state: &AppStateHandles, value: f32) {
    state
        .config
        .update(|c| c.appearance.font_size_scale = value);
    state.debouncer.borrow().request();
}

/// Color mode ComboBox の `on_select` を反映 + debouncer 発火。
/// 未知 label は no-op (= 保守的 fallback、 destructive 上書き回避)。
fn apply_color_mode_selection(state: &AppStateHandles, selected: &str) {
    let Some(value) = parse_color_mode(selected) else {
        return;
    };
    state.config.update(|c| c.appearance.color_mode = value);
    state.debouncer.borrow().request();
}

/// Accent color hex TextInput の `on_change` を反映 + debouncer 発火。
///
/// 文字 1 字編集ごとに発火する想定 (= IME commit / key event Changed / Cut
/// 各経路で fire)。 RequestPaste と set_text は fire しない契約 (PR #155
/// codex 査読確定) のため、起動時の初期 set_text + Ctrl+V 要求では本 helper は
/// 呼ばれない。disk thrash 回避は `debouncer.request()` の 500ms quiet-period に
/// 委ねる。modal 経由の hex 反映 path (= `modals::apply_accent_picker`) は
/// 同等の config commit + debouncer 起動 + draft re-seed 統合を提供する。 本
/// helper は inline TextInput 用 (= modal scope 外) で sections 内 private
/// duplicate として維持 (= 既存 WCAG helper duplicate と同形 pattern)。
fn apply_accent_hex_change(state: &AppStateHandles, text: &str) {
    state
        .config
        .update(|c| c.appearance.accent_hex = text.to_owned());
    state.debouncer.borrow().request();
}

fn parse_color_mode(label: &str) -> Option<ColorMode> {
    match label {
        COLOR_MODE_LIGHT => Some(ColorMode::Light),
        COLOR_MODE_DARK => Some(ColorMode::Dark),
        COLOR_MODE_SYSTEM => Some(ColorMode::System),
        _ => None,
    }
}

/// `Pick...` button on_click action: accent picker modal を起動する
/// (= `state.accent_picker_visible.set(true)`)。 main.rs `ReactiveOverlayContainer`
/// の binding が次 frame sync で overlay を show する。 従来 trigger 皆無で
/// dormant だった modal をユーザー操作で起動可能にする wave 3c の核。
fn open_accent_picker(state: &AppStateHandles) {
    state.accent_picker_visible.set(true);
}

/// WCAG label を現入力 hex で再計算して set_text (= TextInput on_change ごとに
/// 呼ばれる dynamic 更新)。 [`crate::modals::wcag`] の helper を再利用して
/// 重複を排し、 inline accent row と modal で同一 WCAG 評価ロジックを共有する。
fn refresh_wcag_label(wcag_handle: &Rc<RefCell<LabelWidget>>, hex: &str) {
    let (r, g, b) = parse_hex_or_default(hex);
    wcag_handle
        .borrow_mut()
        .set_text(&compute_wcag_status(hex, r, g, b));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Lang;
    // wave 3c dedup: WCAG 低レベル helper は modals::wcag を canonical 化。
    // 既存 appearance WCAG-math test は同 helper を参照する形で維持。
    use crate::modals::wcag::{contrast_ratio, relative_luminance};

    /// HAYATE Original default accent-base RGB (= `#5A8BA8`、 test fixation 用)。
    const DEFAULT_ACCENT_RGB: (u8, u8, u8) = (90, 139, 168);
    /// HAYATE Original default surface-raised RGB (= `#FFFFFF`、 test fixation 用)。
    const DEFAULT_SURFACE_RGB: (u8, u8, u8) = (255, 255, 255);

    #[test]
    fn build_returns_non_panicking_tree_for_both_languages() {
        let state = crate::state::for_testing();
        let _ja = build(Lang::Ja.strings(), &state);
        let _en = build(Lang::En.strings(), &state);
    }

    #[test]
    fn luminance_black_and_white_endpoints() {
        assert!((relative_luminance(0, 0, 0) - 0.0).abs() < 0.001);
        assert!((relative_luminance(255, 255, 255) - 1.0).abs() < 0.001);
    }

    #[test]
    fn contrast_ratio_white_on_black_is_max() {
        let l_w = relative_luminance(255, 255, 255);
        let l_k = relative_luminance(0, 0, 0);
        let r = contrast_ratio(l_w, l_k);
        assert!((r - 21.0).abs() < 0.001);
    }

    #[test]
    fn contrast_ratio_is_symmetric() {
        let la = relative_luminance(90, 139, 168);
        let lb = relative_luminance(255, 255, 255);
        let r1 = contrast_ratio(la, lb);
        let r2 = contrast_ratio(lb, la);
        assert!((r1 - r2).abs() < 0.001);
    }

    #[test]
    fn default_accent_vs_surface_returns_finite_ratio() {
        let (r, g, b) = DEFAULT_ACCENT_RGB;
        let (sr, sg, sb) = DEFAULT_SURFACE_RGB;
        let ratio =
            contrast_ratio(relative_luminance(r, g, b), relative_luminance(sr, sg, sb));
        assert!(ratio.is_finite());
        assert!(ratio > 1.0);
    }

    // ── wave 3c track2: trigger + dynamic WCAG ─────────────────────────

    /// Pick... button action = accent picker modal trigger。
    /// open_accent_picker で accent_picker_visible が false → true に遷移。
    #[test]
    fn open_accent_picker_sets_visible_true() {
        let state = crate::state::for_testing();
        assert!(!*state.accent_picker_visible.get(), "initial = false (dormant)");
        open_accent_picker(&state);
        assert!(
            *state.accent_picker_visible.get(),
            "Pick... trigger で modal visible = true"
        );
    }

    /// open_accent_picker は config / debouncer を一切 touch しない
    /// (= 純粋に modal 起動のみ、 hex 反映は modal 内 Apply path の責務)。
    #[test]
    fn open_accent_picker_does_not_touch_config_or_debouncer() {
        let state = crate::state::for_testing();
        let snapshot = state.config.get().clone();
        open_accent_picker(&state);
        assert_eq!(*state.config.get(), snapshot, "config 不変");
        assert!(!state.debouncer.borrow().has_pending(), "save request なし");
    }

    /// dynamic WCAG: refresh_wcag_label が入力 hex で label text を再計算更新。
    /// 黒 (#000000) vs 白 surface = 21:1 PASS、 風藍 (#5A8BA8) = ~3.6 FAIL。
    #[test]
    fn refresh_wcag_label_updates_text_for_input_hex() {
        let label = LabelWidget::new("initial", 12.0);
        let (_widget, handle) = LabelRef::new_pair(label);

        refresh_wcag_label(&handle, "#000000");
        assert!(
            handle.borrow().text.contains("PASS"),
            "黒 vs 白 surface = 21:1 → AA PASS"
        );
        assert!(handle.borrow().text.contains("#000000"), "入力 hex を echo");

        refresh_wcag_label(&handle, DEFAULT_ACCENT_HEX);
        assert!(
            handle.borrow().text.contains("FAIL"),
            "風藍 vs 白 surface = ~3.6:1 → AA FAIL"
        );
    }

    /// dynamic WCAG: invalid hex 入力でも panic せず fallback 表示。
    #[test]
    fn refresh_wcag_label_handles_invalid_hex_gracefully() {
        let label = LabelWidget::new("initial", 12.0);
        let (_widget, handle) = LabelRef::new_pair(label);
        refresh_wcag_label(&handle, "garbage");
        // parse_hex_or_default が風藍 fallback → FAIL、 raw 入力を echo
        let text = handle.borrow().text.clone();
        assert!(text.contains("garbage"), "raw 入力 echo (= user feedback)");
        assert!(
            text.contains("fallback") || text.contains("FAIL"),
            "invalid 時 fallback 表示 or FAIL 判定"
        );
    }

    // ── wave 3b callback wires ─────────────────────────────────────────

    #[test]
    fn font_scale_change_propagates_and_requests_save() {
        let state = crate::state::for_testing();
        assert!((state.config.get().appearance.font_size_scale - 14.0).abs() < f32::EPSILON);
        apply_font_scale_change(&state, 17.5);
        assert!((state.config.get().appearance.font_size_scale - 17.5).abs() < f32::EPSILON);
        assert!(state.debouncer.borrow().has_pending());
    }

    #[test]
    fn color_mode_selection_each_known_variant_propagates() {
        let state = crate::state::for_testing();
        apply_color_mode_selection(&state, COLOR_MODE_LIGHT);
        assert_eq!(state.config.get().appearance.color_mode, ColorMode::Light);
        apply_color_mode_selection(&state, COLOR_MODE_DARK);
        assert_eq!(state.config.get().appearance.color_mode, ColorMode::Dark);
        apply_color_mode_selection(&state, COLOR_MODE_SYSTEM);
        assert_eq!(state.config.get().appearance.color_mode, ColorMode::System);
        assert!(state.debouncer.borrow().has_pending());
    }

    #[test]
    fn color_mode_unknown_label_is_no_op() {
        let state = crate::state::for_testing();
        state
            .config
            .update(|c| c.appearance.color_mode = ColorMode::Light);
        apply_color_mode_selection(&state, "Sepia (future)");
        assert_eq!(state.config.get().appearance.color_mode, ColorMode::Light);
        assert!(!state.debouncer.borrow().has_pending());
    }

    #[test]
    fn parse_color_mode_round_trip_every_variant() {
        assert_eq!(parse_color_mode(COLOR_MODE_LIGHT), Some(ColorMode::Light));
        assert_eq!(parse_color_mode(COLOR_MODE_DARK), Some(ColorMode::Dark));
        assert_eq!(parse_color_mode(COLOR_MODE_SYSTEM), Some(ColorMode::System));
        assert_eq!(parse_color_mode("Sepia"), None);
    }

    #[test]
    fn accent_hex_change_propagates_and_requests_save() {
        let state = crate::state::for_testing();
        apply_accent_hex_change(&state, "#3D6884");
        assert_eq!(state.config.get().appearance.accent_hex, "#3D6884");
        assert!(state.debouncer.borrow().has_pending());
    }
}
