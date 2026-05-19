//! Appearance section (= RFC v0.5 §5.2.2)。
//!
//! ## fields:
//! - Font size scale (Slider = base 14 ± offset、 range 10-20)
//! - Color mode (ComboBox = Light / Dark / System (Phase 4 defer))
//! - Accent color custom hex (TextInput + WCAG contrast check warning UI)
//! - Theme subsection (LabelWidget = Phase 3 で実装予定 note)
//!
//! ## R11 mitigation (= WCAG simple)
//! 完全 WCAG 2.2 spec は scope outside。本 file では simple relative
//! luminance ([`relative_luminance`]) + contrast ratio ([`contrast_ratio`]) の
//! helper のみ、 default accent (`#5A8BA8` 風藍) vs default surface (`#FFFFFF`)
//! の contrast を **静的に** 表示。dynamic 再計算 (= 入力反映 → 表示更新) は
//! reactive observer / shared widget handle 設計の別 dispatch dep、 本 wave で
//! 永続化 wire を land、 表示側更新は後続で対応。
//!
//! ## wave 3b — persistence wire (complete)
//! - Slider (`on_change`) と Color mode ComboBox (`on_select`) は前置き commit で
//!   `state.config` 更新 + debouncer 発火を配線済 ([`apply_font_scale_change`] /
//!   [`apply_color_mode_selection`])。
//! - TextInput accent_hex は本 commit で [`TextInputWidget::on_change`]
//!   (GUI_kit PR #155 framework gap 解消で利用解禁) push-style reactive bind で
//!   配線完了 ([`apply_accent_hex_change`])。`Rc<RefCell<TextInputWidget>>` wrap や
//!   `take_changed` 外部 poll は不要、`Box<dyn Widget>` move-by-value のまま
//!   builder chain 内で closure capture で state を共有する。初期値の
//!   `set_text(&initial_accent)` 呼出は TextArea parity (PR #155) により
//!   on_change を fire しないため、起動時の余計な debouncer.request() は
//!   発生しない。

use hayate_kit::widget::combo_box::ComboBoxWidget;
use hayate_kit::widget::form_layout::FormLayout;
use hayate_kit::widget::label::LabelWidget;
use hayate_kit::widget::layout::VStack;
use hayate_kit::widget::slider::SliderWidget;
use hayate_kit::widget::text_input_widget::TextInputWidget;
use hayate_kit::Widget;

use crate::lang::Strings;
use crate::persistence::ColorMode;
use crate::state::AppStateHandles;

/// HAYATE Original default accent (= `#5A8BA8` 風藍、 RFC v0.2 §3 design language)。
const DEFAULT_ACCENT_HEX: &str = "#5A8BA8";
/// HAYATE Original default surface-raised (= `#FFFFFF`)。
const DEFAULT_SURFACE_RGB: (u8, u8, u8) = (255, 255, 255);
/// HAYATE Original default accent-base RGB (= `#5A8BA8`)。
const DEFAULT_ACCENT_RGB: (u8, u8, u8) = (90, 139, 168);

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

    // ── Accent color hex ───────────────────────────────────────────────
    // 初期値を state.config からロード、入力反映は PR #155 framework gap fix で
    // 利用可能になった TextInputWidget::on_change push-style reactive bind で
    // 配線 (= apply_accent_hex_change → state.config.update + debouncer.request)。
    // 初期 set_text は TextArea parity で on_change を fire しないため、
    // 起動時に余計な save request が走らない。
    let initial_accent = state.config.get().appearance.accent_hex.clone();
    let mut accent_input = TextInputWidget::new()
        .with_placeholder(DEFAULT_ACCENT_HEX)
        .with_width(160.0)
        .on_change({
            let state = state.clone();
            move |text| apply_accent_hex_change(&state, text)
        });
    if !initial_accent.is_empty() {
        accent_input.set_text(&initial_accent);
    }

    let form = FormLayout::new()
        .row("Font size scale", font_scale)
        .row("Color mode", color_mode)
        .row("Accent color (hex)", accent_input);

    // WCAG simple contrast warning (= R11 mitigation 静的版)。
    // default accent 風藍 vs default surface 白 の contrast を計算して表示。
    // accent_hex 永続化 wire は完了済、 表示の dynamic 更新は reactive observer /
    // shared widget handle 設計の別 dispatch dep。
    let (r, g, b) = DEFAULT_ACCENT_RGB;
    let (sr, sg, sb) = DEFAULT_SURFACE_RGB;
    let ratio = contrast_ratio(relative_luminance(r, g, b), relative_luminance(sr, sg, sb));
    let aa_pass = ratio >= 4.5;
    let wcag_note = format!(
        "WCAG AA (>=4.5:1): {} vs surface = {} -- {} [reactive validation: dynamic re-calc 別 dispatch dep]",
        DEFAULT_ACCENT_HEX,
        format_ratio(ratio),
        if aa_pass {
            "PASS"
        } else {
            "FAIL: pick a darker / lighter accent"
        },
    );
    let wcag_label = LabelWidget::new(wcag_note, 12.0);

    // Theme サブセクション = Phase 3 hero feature defer の note。
    let theme_subheading = LabelWidget::new(strings.section_appearance_theme, 16.0);
    let theme_defer_note = LabelWidget::new(
        "Phase 3 で theme switcher (retro presets + HAYATE Original variants) を実装予定。",
        13.0,
    );

    let mut stack = VStack::new(16.0);
    stack = stack.add(Box::new(heading));
    stack = stack.add(Box::new(form));
    stack = stack.add(Box::new(wcag_label));
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

/// WCAG 2.2 §1.4.3 relative luminance (simple sRGB linearization)。
///
/// 完全 spec の gamma piecewise (= threshold 0.03928) は scope outside、本
/// helper は `(r * 0.299 + g * 0.587 + b * 0.114) / 255.0` の伝統的 luminance
/// 近似 ([ITU-R BT.601] coefficient)。reactive UI への組込前の sanity check
/// 用途のみで使用、 wave 3c で完全 WCAG 2.2 関数へ差替予定。
///
/// [ITU-R BT.601]: https://en.wikipedia.org/wiki/Relative_luminance
fn relative_luminance(r: u8, g: u8, b: u8) -> f32 {
    let rf = r as f32;
    let gf = g as f32;
    let bf = b as f32;
    (rf * 0.299 + gf * 0.587 + bf * 0.114) / 255.0
}

/// WCAG 2.2 §1.4.3 contrast ratio approximation (= `(L1 + 0.05) / (L2 + 0.05)`、
/// L1 >= L2)。
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
