//! Appearance section (= RFC v0.5 §5.2.2)。
//!
//! wave 1 = Theme 以外 fields の widget composition (Font size scale / Color
//! mode / Accent color custom hex + WCAG simple)、 Theme サブセクションは
//! Phase 3 hero feature defer の note label のみ表示。
//!
//! ## fields:
//! - Font size scale (Slider = base 14 ± offset、 range 10-20)
//! - Color mode (ComboBox = Light / Dark / System (Phase 4 defer))
//! - Accent color custom hex (TextInput + WCAG contrast check warning UI)
//! - Theme subsection (LabelWidget = Phase 3 で実装予定 note)
//!
//! ## R11 mitigation (= WCAG simple)
//! 完全 WCAG 2.2 spec は scope outside。本 wave 1 では simple relative
//! luminance 計算 ([`relative_luminance`]) + contrast ratio ([`contrast_ratio`])
//! の helper のみ実装、 default accent (`#5A8BA8` 風藍) vs default surface
//! (`#FFFFFF`) の contrast を **静的に** 表示 (= reactive update は wave 2 で
//! `on_change` callback 経由で配線予定)。
//!
//! ## reactive wire status (= wave 1 scope)
//! 各 field の immediate save trigger / hex 入力毎の WCAG re-evaluation は
//! wave 2 dep (= persistence layer + reactive bind 経由)。本 wave では
//! default 値で静的に初期化のみ実施。

use hayate_kit::widget::combo_box::ComboBoxWidget;
use hayate_kit::widget::form_layout::FormLayout;
use hayate_kit::widget::label::LabelWidget;
use hayate_kit::widget::layout::VStack;
use hayate_kit::widget::slider::SliderWidget;
use hayate_kit::widget::text_input_widget::TextInputWidget;
use hayate_kit::Widget;

use crate::lang::Strings;
use crate::state::AppStateHandles;

/// HAYATE Original default accent (= `#5A8BA8` 風藍、 RFC v0.2 §3 design language)。
const DEFAULT_ACCENT_HEX: &str = "#5A8BA8";
/// HAYATE Original default surface-raised (= `#FFFFFF`)。
const DEFAULT_SURFACE_RGB: (u8, u8, u8) = (255, 255, 255);
/// HAYATE Original default accent-base RGB (= `#5A8BA8`)。
const DEFAULT_ACCENT_RGB: (u8, u8, u8) = (90, 139, 168);

/// Build Appearance section widget tree。
///
/// wave 1 構造 = `VStack { heading + FormLayout { 3 rows } + WCAG note +
/// Theme subheading + Theme defer note }`。 Theme サブセクションは Phase 3
/// hero feature defer のため標準 LabelWidget で hardcoded note のみ表示
/// (= 実 theme switcher widget は Phase 3 land)。
/// ## wave 3a signature 拡張
/// `_state: &AppStateHandles` を受け取るのは wave 3b で Slider on_change /
/// ComboBox on_select / TextInput take_changed poll → state.config.update →
/// debouncer.request、 さらに hex 入力 → WCAG ratio dynamic 更新を配線するため。
/// wave 3a 時点では未使用 (`_` prefix で warning suppress)。
pub fn build(strings: &'static Strings, _state: &AppStateHandles) -> Box<dyn Widget> {
    let heading = LabelWidget::new(strings.section_appearance, 18.0);

    // Font size scale — 10.0 から 20.0、 default 14.0 (= HAYATE Original baseline)。
    // wave 2 で `on_change` callback + persistence 連動。
    let font_scale = SliderWidget::new(10.0, 20.0, 14.0);

    // Color mode picker — Light / Dark / System (Phase 4 defer note 同行表示)。
    // wave 2 で actual theme apply 連動。
    let color_mode = ComboBoxWidget::new(vec![
        "Light".to_string(),
        "Dark".to_string(),
        "System (Phase 4 defer)".to_string(),
    ]);

    // Accent color hex 入力 — default `#5A8BA8` を placeholder で提示。
    // wave 2 で `on_change` callback + R11 WCAG warning re-evaluation 連動。
    let accent_input = TextInputWidget::new()
        .with_placeholder(DEFAULT_ACCENT_HEX)
        .with_width(160.0);

    let form = FormLayout::new()
        .row("Font size scale", font_scale)
        .row("Color mode", color_mode)
        .row("Accent color (hex)", accent_input);

    // WCAG simple contrast warning (= R11 mitigation 静的版)。
    // default accent 風藍 vs default surface 白 の contrast を計算して表示。
    // wave 2 で hex 入力に reactive bind した dynamic 版 へ移行予定。
    let (r, g, b) = DEFAULT_ACCENT_RGB;
    let (sr, sg, sb) = DEFAULT_SURFACE_RGB;
    let ratio = contrast_ratio(relative_luminance(r, g, b), relative_luminance(sr, sg, sb));
    let aa_pass = ratio >= 4.5;
    let wcag_note = format!(
        "WCAG AA (>=4.5:1): {} vs surface = {} -- {} [reactive validation: wave 2]",
        DEFAULT_ACCENT_HEX,
        format_ratio(ratio),
        if aa_pass {
            "PASS"
        } else {
            "FAIL: pick a darker / lighter accent"
        },
    );
    // `LabelWidget::new` takes `impl Into<String>`、 String を直接 pass 可。
    let wcag_label = LabelWidget::new(wcag_note, 12.0);

    // Theme サブセクション = Phase 3 hero feature defer の note。
    // strings.section_appearance_theme は wave 0 で既存 field、 i18n 整合。
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

/// WCAG 2.2 §1.4.3 relative luminance (simple sRGB linearization)。
///
/// 完全 spec の gamma piecewise (= threshold 0.03928) は scope outside、本
/// helper は `(r * 0.299 + g * 0.587 + b * 0.114) / 255.0` の伝統的 luminance
/// 近似 ([ITU-R BT.601] coefficient)。reactive UI への組込前の sanity check
/// 用途のみで使用、 wave 2 で完全 WCAG 2.2 関数へ差替予定。
///
/// [ITU-R BT.601]: https://en.wikipedia.org/wiki/Relative_luminance
fn relative_luminance(r: u8, g: u8, b: u8) -> f32 {
    let rf = r as f32;
    let gf = g as f32;
    let bf = b as f32;
    (rf * 0.299 + gf * 0.587 + bf * 0.114) / 255.0
}

/// WCAG 2.2 §1.4.3 contrast ratio approximation (= `(L1 + 0.05) / (L2 + 0.05)`、
/// L1 >= L2)。 上記 [`relative_luminance`] と同様 simple 近似で、 完全 spec
/// の luminance 関数とは小数点以下で誤差あり (= 数 % オーダー)、本 wave 1
/// では AA threshold (4.5) 大幅超過 / 不足の判定にのみ使用、完全 borderline
/// case は wave 2 で完全 spec 採用後再評価。
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
        // BT.601 coefficient sum = 0.299 + 0.587 + 0.114 = 1.0 で white は 1.0、
        // black は 0.0、 monotonic in between。
        assert!((relative_luminance(0, 0, 0) - 0.0).abs() < 0.001);
        assert!((relative_luminance(255, 255, 255) - 1.0).abs() < 0.001);
    }

    #[test]
    fn contrast_ratio_white_on_black_is_max() {
        // 完全 spec では 21:1、 simple 近似でも (1.05/0.05) = 21.0。
        let l_w = relative_luminance(255, 255, 255);
        let l_k = relative_luminance(0, 0, 0);
        let r = contrast_ratio(l_w, l_k);
        assert!((r - 21.0).abs() < 0.001);
    }

    #[test]
    fn contrast_ratio_is_symmetric() {
        // L1/L2 順序非依存 (= 内部で大小比較してから比) を smoke。
        let la = relative_luminance(90, 139, 168); // accent
        let lb = relative_luminance(255, 255, 255); // surface
        let r1 = contrast_ratio(la, lb);
        let r2 = contrast_ratio(lb, la);
        assert!((r1 - r2).abs() < 0.001);
    }

    #[test]
    fn default_accent_vs_surface_returns_finite_ratio() {
        // wave 1 静的 WCAG note の入力で panic / NaN / Inf が起きないこと smoke。
        let (r, g, b) = DEFAULT_ACCENT_RGB;
        let (sr, sg, sb) = DEFAULT_SURFACE_RGB;
        let ratio =
            contrast_ratio(relative_luminance(r, g, b), relative_luminance(sr, sg, sb));
        assert!(ratio.is_finite());
        assert!(ratio > 1.0); // any non-identical colors give >1
    }
}
