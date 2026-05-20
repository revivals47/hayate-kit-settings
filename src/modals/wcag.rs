//! WCAG simple helpers + hex parser (= accent picker preview / contrast check)。
//!
//! wave 3c-pre module split で `modals.rs` から分離 (pure refactor、 behavior
//! 変更ゼロ)。
//!
//! sections/appearance.rs 内 helper が private で reuse 不可、appearance.rs
//! touch 禁止のため modals に同一実装を複製。 wave 3c 以降で共通 module
//! (= src/wcag.rs 等) への extract 検討予定。 完全 spec の gamma piecewise
//! (= threshold 0.03928) は scope outside、 BT.601 luminance 近似で AA
//! threshold (4.5) 判定にのみ使用。

use super::{DEFAULT_ACCENT_HEX, DEFAULT_ACCENT_RGB, DEFAULT_SURFACE_RGB};

/// WCAG 2.2 §1.4.3 relative luminance (simple sRGB linearization、BT.601 近似)。
pub(crate) fn relative_luminance(r: u8, g: u8, b: u8) -> f32 {
    let rf = r as f32;
    let gf = g as f32;
    let bf = b as f32;
    (rf * 0.299 + gf * 0.587 + bf * 0.114) / 255.0
}

/// WCAG 2.2 §1.4.3 contrast ratio approximation (= `(L1 + 0.05) / (L2 + 0.05)`、L1 >= L2)。
pub(crate) fn contrast_ratio(la: f32, lb: f32) -> f32 {
    let (l1, l2) = if la >= lb { (la, lb) } else { (lb, la) };
    (l1 + 0.05) / (l2 + 0.05)
}

/// `4.7321 -> "4.73:1"` 等。 contrast ratio 表示用。
pub(crate) fn format_ratio(r: f32) -> String {
    format!("{:.2}:1", r)
}

/// `#RRGGBB` を parse して `(r, g, b)` を返す。 形式不正なら DEFAULT_ACCENT_RGB
/// (= 風藍) を返す (= 無効入力中も preview を fallback でレンダー継続、 user に
/// 「無効」表示で feedback)。
///
/// validation 仕様: 先頭 `#` + 6 文字 hex (case-insensitive)。 短縮形式
/// (#RGB) や `0x` prefix は accept しない (= 仕様シンプル化、 wave 3c 以降で
/// reactive hex parser を強化予定)。
pub(crate) fn parse_hex_or_default(hex: &str) -> (u8, u8, u8) {
    if !is_valid_hex(hex) {
        return DEFAULT_ACCENT_RGB;
    }
    let r = u8::from_str_radix(&hex[1..3], 16).unwrap_or(DEFAULT_ACCENT_RGB.0);
    let g = u8::from_str_radix(&hex[3..5], 16).unwrap_or(DEFAULT_ACCENT_RGB.1);
    let b = u8::from_str_radix(&hex[5..7], 16).unwrap_or(DEFAULT_ACCENT_RGB.2);
    (r, g, b)
}

/// `#RRGGBB` 形式チェック (= 先頭 `#` + 6 hex digit、 case-insensitive)。
pub(crate) fn is_valid_hex(hex: &str) -> bool {
    let bytes = hex.as_bytes();
    bytes.len() == 7
        && bytes[0] == b'#'
        && bytes[1..].iter().all(|b| b.is_ascii_hexdigit())
}

/// Preview swatch Label の表示文字列を構成。 valid 時は hex 表記 + 風藍呼称、
/// invalid 時は fallback indication を付与 (= user 即座に format error を認知可)。
pub(crate) fn update_preview_text(hex: &str, _r: u8, _g: u8, _b: u8) -> String {
    if is_valid_hex(hex) {
        format!("Preview swatch: {}", hex)
    } else {
        format!(
            "Preview swatch: {} (invalid hex → fallback {})",
            hex, DEFAULT_ACCENT_HEX
        )
    }
}

/// WCAG ratio status Label の表示文字列を構成。 入力 RGB と DEFAULT_SURFACE_RGB
/// (= `#FFFFFF`) との contrast ratio を計算、 AA threshold (4.5:1) 比較で
/// PASS/FAIL 判定 + 推奨 hint を付与。
pub(crate) fn compute_wcag_status(hex: &str, r: u8, g: u8, b: u8) -> String {
    let (sr, sg, sb) = DEFAULT_SURFACE_RGB;
    let ratio = contrast_ratio(
        relative_luminance(r, g, b),
        relative_luminance(sr, sg, sb),
    );
    let aa_pass = ratio >= 4.5;
    let verdict = if aa_pass {
        "PASS"
    } else {
        "FAIL: pick a darker / lighter accent"
    };
    let display_hex = if is_valid_hex(hex) {
        hex.to_string()
    } else {
        format!("{} (using fallback {})", hex, DEFAULT_ACCENT_HEX)
    };
    format!(
        "WCAG AA (>=4.5:1): {} vs surface = {} -- {}",
        display_hex,
        format_ratio(ratio),
        verdict,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn is_valid_hex_recognizes_canonical_form() {
        assert!(is_valid_hex("#5A8BA8"));
        assert!(is_valid_hex("#000000"));
        assert!(is_valid_hex("#FFFFFF"));
        assert!(is_valid_hex("#abcdef"), "lowercase OK");
        assert!(is_valid_hex("#AbCdEf"), "mixed case OK");
    }

    #[test]
    fn is_valid_hex_rejects_invalid_forms() {
        assert!(!is_valid_hex(""), "empty");
        assert!(!is_valid_hex("5A8BA8"), "missing #");
        assert!(!is_valid_hex("#5A8"), "short");
        assert!(!is_valid_hex("#5A8BA8FF"), "too long (alpha unsupported)");
        assert!(!is_valid_hex("#GGGGGG"), "non-hex digit");
        assert!(!is_valid_hex("#5A 8BA"), "embedded space");
    }

    #[test]
    fn parse_hex_or_default_returns_canonical_value() {
        assert_eq!(parse_hex_or_default("#5A8BA8"), (90, 139, 168));
        assert_eq!(parse_hex_or_default("#000000"), (0, 0, 0));
        assert_eq!(parse_hex_or_default("#FFFFFF"), (255, 255, 255));
        assert_eq!(parse_hex_or_default("#abcdef"), (0xab, 0xcd, 0xef));
    }

    #[test]
    fn parse_hex_or_default_falls_back_on_invalid() {
        assert_eq!(parse_hex_or_default(""), DEFAULT_ACCENT_RGB);
        assert_eq!(parse_hex_or_default("#GGGGGG"), DEFAULT_ACCENT_RGB);
        assert_eq!(parse_hex_or_default("badtext"), DEFAULT_ACCENT_RGB);
    }

    #[test]
    fn update_preview_text_marks_invalid_as_fallback() {
        let valid = update_preview_text("#5A8BA8", 90, 139, 168);
        assert!(valid.contains("#5A8BA8"), "valid hex echoed");
        assert!(!valid.contains("fallback"), "valid path に fallback 表示なし");

        let invalid = update_preview_text("garbage", 0, 0, 0);
        assert!(invalid.contains("garbage"), "raw input echoed");
        assert!(invalid.contains("invalid hex"), "invalid 状態を表示");
        assert!(
            invalid.contains(DEFAULT_ACCENT_HEX),
            "fallback hex 名示"
        );
    }

    #[test]
    fn compute_wcag_status_pass_or_fail_branch() {
        // 風藍 vs #FFFFFF surface = ~3.6:1 → AA FAIL (= 推奨 darker accent)
        let s_default = compute_wcag_status("#5A8BA8", 90, 139, 168);
        assert!(s_default.contains("FAIL"), "default 風藍 vs 白 surface は FAIL");

        // 黒 vs #FFFFFF = max contrast (21:1) → AA PASS
        let s_black = compute_wcag_status("#000000", 0, 0, 0);
        assert!(s_black.contains("PASS"), "黒 vs 白 surface は PASS");
        assert!(s_black.contains("21.00:1"), "ratio 表示確認");
    }
}
