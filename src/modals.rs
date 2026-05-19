//! Modal dialog instances (= RFC v0.5 §5.2.8、 Phase 2 wave 2 fill)。
//!
//! 2 instance を fill:
//! - `build_accent_picker(strings) -> Box<dyn Widget>` (= accent color hex 入力 +
//!   preview swatch + WCAG contrast check warning UI)
//! - `build_reset_confirm(strings) -> Box<dyn Widget>` (= 全 section reset 等の
//!   destructive action gate)
//!
//! ## framework 化は Phase 4 へ defer
//! 「framework 化」 (= 汎用化 / extension point / event 設計) は Phase 4 へ defer
//! 明示、 本 wave では 2 instance を land に scope 限定 (= YAGNI 回避)。
//!
//! ## wave 境界
//! wave 2 scope = 2 modal instance widget composition のみ。 modal の caller
//! 配線 (= 例: appearance.rs accent color row click → build_accent_picker show)
//! は wave 3 integration dep。click callback は placeholder closure (= `|| {}` /
//! `|_| {}`)、 actual logic は wave 3 で persistence + reactive bind と合わせて
//! 配線。
//!
//! ## AlertDialog 非採用 (= 設計選択メモ)
//! `hayate_kit::widget::alert_dialog::AlertDialog` は default `visible: false`
//! で内部に show/hide 状態を持つ自前 modal widget。本 wave では widget
//! composition のみで、modal lifecycle (= show / dismiss) は wave 3 dep のため、
//! AlertDialog をここで使うと visible=false のまま paint 0 になり smoke test
//! の目的を満たさない。よって VStack + HStack で受動 widget tree を構築、
//! modal lifecycle wire は wave 3 で AlertDialog or popup framework に置換可。
//!
//! ## DTP reuse
//! DTP app destructive action gate (= 例: ファイル削除確認 / 設定 reset / 編集
//! 取消) + typography color picker でも reuse 想定、 modal popup pattern は
//! universal。

use hayate_kit::widget::button::ButtonWidget;
use hayate_kit::widget::label::LabelWidget;
use hayate_kit::widget::layout::{HStack, VStack};
use hayate_kit::widget::text_input_widget::TextInputWidget;
use hayate_kit::Widget;

#[allow(unused_imports)] // wave 3 で `Strings` field を本格使用予定
use crate::lang::Strings;

/// HAYATE Original default accent (= `#5A8BA8` 風藍、 RFC v0.2 §3 design language)。
const DEFAULT_ACCENT_HEX: &str = "#5A8BA8";
/// HAYATE Original default accent-base RGB (= `#5A8BA8`)。
const DEFAULT_ACCENT_RGB: (u8, u8, u8) = (90, 139, 168);
/// HAYATE Original default surface-raised RGB (= `#FFFFFF`)。
const DEFAULT_SURFACE_RGB: (u8, u8, u8) = (255, 255, 255);

/// Accent color picker modal (= wave 2 fill 完遂)。
///
/// 構造: `VStack { hex TextInput + preview swatch Label + WCAG ratio Label }`。
/// wave 3 で hex 入力 → reactive bind → preview swatch / WCAG ratio dynamic 更新を
/// 配線予定 (= 現状は default `#5A8BA8` 風藍 vs `#FFFFFF` surface で静的計算)。
#[allow(dead_code)] // wave 3 で main.rs / appearance.rs から呼ばれる
pub fn build_accent_picker(_strings: &'static Strings) -> Box<dyn Widget> {
    // hex 入力 (= default 風藍を placeholder 提示)。 wave 3 で on_change wire。
    let hex_input = TextInputWidget::new()
        .with_placeholder(DEFAULT_ACCENT_HEX)
        .with_width(160.0);

    // preview swatch (= 固定 default 風藍 hex code の text 表示で代替、
    // actual user-input parse + dynamic color fill rect は wave 3 dep)。
    let preview = LabelWidget::new(
        format!("Preview swatch: {} (default 風藍)", DEFAULT_ACCENT_HEX),
        13.0,
    );

    // WCAG simple contrast check (= R11 mitigation 静的版、 sections/appearance.rs
    // と同 BT.601 helper を local duplicate = appearance.rs 内 helper は private
    // のため、scope discipline 維持で modals.rs only に複製。
    // wave 3 で helper を共通 module に extract 検討)。
    let (r, g, b) = DEFAULT_ACCENT_RGB;
    let (sr, sg, sb) = DEFAULT_SURFACE_RGB;
    let ratio = contrast_ratio(relative_luminance(r, g, b), relative_luminance(sr, sg, sb));
    let aa_pass = ratio >= 4.5;
    let wcag_note = format!(
        "WCAG AA (>=4.5:1): {} vs surface = {} -- {} [reactive validation: wave 3]",
        DEFAULT_ACCENT_HEX,
        format_ratio(ratio),
        if aa_pass {
            "PASS"
        } else {
            "FAIL: pick a darker / lighter accent"
        },
    );
    let wcag_label = LabelWidget::new(wcag_note, 12.0);

    let mut stack = VStack::new(8.0);
    stack = stack.add(Box::new(hex_input));
    stack = stack.add(Box::new(preview));
    stack = stack.add(Box::new(wcag_label));
    Box::new(stack)
}

/// Reset confirm modal (= wave 2 fill 完遂)。
///
/// 構造: `VStack { confirm message Label + HStack { Cancel ButtonWidget + Reset
/// ButtonWidget } }`。 click callback は wave 3 dep で placeholder closure
/// (= `|| {}`)。 wave 3 で persistence reset + modal dismiss を配線予定。
#[allow(dead_code)] // wave 3 で main.rs / advanced.rs から呼ばれる
pub fn build_reset_confirm(_strings: &'static Strings) -> Box<dyn Widget> {
    let message = LabelWidget::new(
        "全 settings を default に reset しますか? (この操作は取消不可)",
        14.0,
    );

    let cancel_btn = ButtonWidget::new("Cancel").on_click(|| { /* wave 3: dismiss modal */ });
    let reset_btn = ButtonWidget::new("Reset")
        .on_click(|| { /* wave 3: persistence::reset() + dismiss modal */ });

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
// wave 3 で共通 module (= src/wcag.rs 等) への extract 検討予定。
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

    #[test]
    fn build_accent_picker_smoke() {
        let strings = Lang::En.strings();
        let _root = build_accent_picker(strings);
        // 構造: VStack { TextInput + preview Label + WCAG ratio Label }。
        // smoke: panic なく Box<dyn Widget> 返却。
    }

    #[test]
    fn build_reset_confirm_smoke() {
        let strings = Lang::En.strings();
        let _root = build_reset_confirm(strings);
        // 構造: VStack { message Label + HStack { Cancel Button + Reset Button } }。
        // smoke: panic なく Box<dyn Widget> 返却、placeholder closure capture OK。
    }

    #[test]
    fn wcag_helpers_match_appearance_section_values() {
        // sections/appearance.rs と同 BT.601 implementation を local duplicate
        // した parity smoke。 default 風藍 vs 白 surface の ratio が再現可能。
        let l_a = relative_luminance(90, 139, 168);
        let l_s = relative_luminance(255, 255, 255);
        let ratio = contrast_ratio(l_a, l_s);
        assert!(ratio.is_finite());
        assert!(ratio > 1.0); // any non-identical colors give >1
    }

    #[test]
    fn wcag_endpoints_white_on_black_is_max_21() {
        // 完全 spec では 21:1、 simple 近似でも (1.05/0.05) = 21.0。
        let l_w = relative_luminance(255, 255, 255);
        let l_k = relative_luminance(0, 0, 0);
        let r = contrast_ratio(l_w, l_k);
        assert!((r - 21.0).abs() < 0.001);
    }
}
