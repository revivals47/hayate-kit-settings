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

use hayate_kit::prelude::*;
// AppTheme / Theme type は prelude の nested module 内 re-export で glob 非到達の
// ため crate root から明示 import (= app_theme_for / theme_for の戻り型)。
use hayate_kit::{AppTheme, Theme};
// 6 skin AppTheme preset。 app_theme_hayate_original は prelude 経由 reach 済、
// 残 5 件は full path で明示 import (= Phase 3a theme switcher の app_theme_for mapping)。
use hayate_kit::style::widget_theme_presets::app::{
    app_theme_mac_os9, app_theme_macos_big_sur, app_theme_win10, app_theme_win95, app_theme_xp_luna,
};
// 6 skin の base palette 定数 (= theme_for mapping、 ACTIVE_THEME に入る半分)。
// HAYATE_ORIGINAL は prelude 経由 reach 済、 残 5 件は AppTheme preset と同じ
// L2 style module から full path で明示 import。
use hayate_kit::style::theme::{
    MACOS9_THEME, MACOS_BIG_SUR_THEME, WIN10_THEME, WIN95_THEME, XP_LUNA_THEME,
};
// 6 skin の title bar theme (= titlebar_theme_for mapping)。ThemeBundle.titlebar_theme
// に載せて runtime swap で title bar も skin 連動切替 (Phase 3b skin-aware chrome)。
// TitleBarTheme 型は hayate_kit crate root から reach。
use hayate_kit::TitleBarTheme;
use hayate_kit::style::widget_theme_presets::titlebar::{
    titlebar_theme_hayate_original, titlebar_theme_mac_os9, titlebar_theme_macos_big_sur,
    titlebar_theme_win10, titlebar_theme_win95, titlebar_theme_xp_luna,
};

use crate::lang::Strings;
use crate::modals::wcag::{compute_wcag_status, parse_hex_or_default};
use crate::modals::LabelRef;
use crate::persistence::{ColorMode, ThemeId};
use crate::state::AppStateHandles;

/// HAYATE Original default accent (= `#5A8BA8` 風藍、 RFC v0.2 §3 design language)。
const DEFAULT_ACCENT_HEX: &str = "#5A8BA8";

// ── Color mode ComboBox labels (= source of truth、 callback + test 共有) ──

const COLOR_MODE_LIGHT: &str = "Light";
const COLOR_MODE_DARK: &str = "Dark";
const COLOR_MODE_SYSTEM: &str = "System (Phase 4 defer)";

/// Map a [`ThemeId`] to its GUI_kit [`AppTheme`] preset (= Phase 3a theme
/// switcher の中核 mapping)。 linux-gallery `chrome.rs` の `ThemeId::app_theme`
/// pattern を reuse、 ただし全 6 variant が実 preset を返す (= HayateOriginal も
/// `None` ではなく `app_theme_hayate_original()`)。
///
/// 起動時 (`main.rs` の `with_app_theme`) と runtime swap (= Theme ComboBox
/// `on_select` → `theme_handle.set(app_theme_for(id))`、 handle 依存部は
/// re-export land 後配線) の両方が本 fn を経由する。 `match` は exhaustive
/// (= `_` arm なし) なので、 将来 [`ThemeId`] に variant 追加時は本 fn が
/// compile error で漏れを検出する。
pub fn app_theme_for(id: ThemeId) -> AppTheme {
    match id {
        ThemeId::HayateOriginal => app_theme_hayate_original(),
        ThemeId::Win95 => app_theme_win95(),
        ThemeId::XpLuna => app_theme_xp_luna(),
        ThemeId::Win10 => app_theme_win10(),
        ThemeId::MacOs9 => app_theme_mac_os9(),
        ThemeId::MacOsBigSur => app_theme_macos_big_sur(),
    }
}

/// Map a [`ThemeId`] to its GUI_kit base [`Theme`] palette — the half that
/// installs into `ACTIVE_THEME` and is read by `active_theme()` widgets (and
/// the frame background). The per-skin twin of [`app_theme_for`];
/// [`theme_bundle_for`] pairs the two so a runtime swap moves the palette AND
/// the per-widget aggregate together. `match` is exhaustive (no `_` arm) so a
/// future [`ThemeId`] variant surfaces here as a compile error.
pub fn theme_for(id: ThemeId) -> &'static Theme {
    match id {
        ThemeId::HayateOriginal => &HAYATE_ORIGINAL,
        ThemeId::Win95 => &WIN95_THEME,
        ThemeId::XpLuna => &XP_LUNA_THEME,
        ThemeId::Win10 => &WIN10_THEME,
        ThemeId::MacOs9 => &MACOS9_THEME,
        ThemeId::MacOsBigSur => &MACOS_BIG_SUR_THEME,
    }
}

/// Map a [`ThemeId`] to its GUI_kit [`TitleBarTheme`] — the per-skin title bar
/// (navy Win95, glossy XP, …). Carried in [`theme_bundle_for`] so a runtime
/// swap re-themes the title bar in lock-step (Phase 3b skin-aware chrome).
/// `match` is exhaustive so a future [`ThemeId`] surfaces here at compile time.
pub fn titlebar_theme_for(id: ThemeId) -> TitleBarTheme {
    match id {
        ThemeId::HayateOriginal => titlebar_theme_hayate_original(),
        ThemeId::Win95 => titlebar_theme_win95(),
        ThemeId::XpLuna => titlebar_theme_xp_luna(),
        ThemeId::Win10 => titlebar_theme_win10(),
        ThemeId::MacOs9 => titlebar_theme_mac_os9(),
        ThemeId::MacOsBigSur => titlebar_theme_macos_big_sur(),
    }
}

/// Pair a [`ThemeId`]'s base palette ([`theme_for`]), per-widget aggregate
/// ([`app_theme_for`]) and title bar ([`titlebar_theme_for`]) into a
/// [`ThemeBundle`] for a full runtime theme swap (`AppThemeHandle::set_bundle`).
/// Carrying all three halves is what makes a skin switch re-colour
/// `active_theme()` widgets + the frame background + the title bar together,
/// not just the injected `AppTheme` — the gap the deprecated
/// `AppThemeHandle::set` left.
pub fn theme_bundle_for(id: ThemeId) -> ThemeBundle {
    ThemeBundle {
        theme: theme_for(id).clone(),
        app_theme: Rc::new(app_theme_for(id)),
        titlebar_theme: Some(titlebar_theme_for(id)),
    }
}

/// Theme ComboBox の選択肢 (= `(ThemeId, 表示 label)` の単一 source of truth)。
/// 表示順 = HayateOriginal (signature) を先頭、 以降 retro/vendor。 label ⇔
/// ThemeId 双方向 map ([`theme_label_for`] / [`theme_id_from_label`]) の元。
const THEME_CHOICES: [(ThemeId, &str); 6] = [
    (ThemeId::HayateOriginal, "HAYATE Original"),
    (ThemeId::Win95, "Windows 95"),
    (ThemeId::XpLuna, "Windows XP"),
    (ThemeId::Win10, "Windows 10"),
    (ThemeId::MacOs9, "Mac OS 9"),
    (ThemeId::MacOsBigSur, "macOS"),
];

/// [`ThemeId`] → ComboBox 表示 label。 [`THEME_CHOICES`] を引く。
fn theme_label_for(id: ThemeId) -> &'static str {
    THEME_CHOICES
        .iter()
        .find(|(tid, _)| *tid == id)
        .map(|(_, label)| *label)
        .unwrap_or("HAYATE Original")
}

/// ComboBox 表示 label → [`ThemeId`]。 未知 label は `None` (= 保守的 fallback、
/// on_select 側で no-op、 destructive 上書き回避)。
fn theme_id_from_label(label: &str) -> Option<ThemeId> {
    THEME_CHOICES
        .iter()
        .find(|(_, l)| *l == label)
        .map(|(tid, _)| *tid)
}

/// Theme ComboBox の `on_select` を反映: (1) theme_handle 経由 runtime swap
/// (= `handle.set_bundle(theme_bundle_for(id))`、 production のみ Some)、
/// (2) config.theme_id 更新 + debouncer 発火 (= persist)。
///
/// `set_bundle` は base palette (`ACTIVE_THEME`) + per-widget `AppTheme` の
/// 両 half を 1 transaction で運ぶ (= 旧 `set()` は AppTheme のみ運び palette
/// 据置 = skin 切替で背景/`active_theme()` widget が変わらなかった gap の root-fix)。
///
/// handle が `None` (= test 構築) の場合は runtime swap を skip し config/debouncer
/// のみ更新 (= set_bundle は GUI runtime 経路で元々 unit test observe 不可、
/// persist 側のみ assert する設計)。
fn apply_theme_selection(state: &AppStateHandles, id: ThemeId) {
    if let Some(handle) = state.theme_handle.as_ref() {
        handle.set_bundle(theme_bundle_for(id));
    }
    state.config.update(|c| c.appearance.theme_id = id);
    state.debouncer.borrow().request();
}

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

    // ── Theme サブセクション = Phase 3a theme switcher (6 skin runtime swap) ──
    // 旧 defer note label を実 ComboBox に置換。 on_select → apply_theme_selection
    // が theme_handle.set(app_theme_for(id)) で runtime swap + config.update +
    // debouncer。 初期選択は persist された theme_id (= 起動時 app_theme_for と整合)。
    let theme_subheading = LabelWidget::new(strings.section_appearance_theme, 16.0);
    let theme_initial = state.config.get().appearance.theme_id;
    let mut theme_combo = ComboBoxWidget::new(
        THEME_CHOICES
            .iter()
            .map(|(_, label)| (*label).to_string())
            .collect(),
    )
    .on_select({
        let s = state.clone();
        move |selected| {
            if let Some(id) = theme_id_from_label(selected) {
                apply_theme_selection(&s, id);
            }
        }
    });
    theme_combo.set_text(theme_label_for(theme_initial));

    let mut stack = VStack::new(16.0);
    stack = stack.add(Box::new(heading));
    stack = stack.add(Box::new(form));
    stack = stack.add(wcag_widget);
    stack = stack.add(Box::new(theme_subheading));
    stack = stack.add(Box::new(theme_combo));
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

/// `Pick...` button on_click action: accent picker modal を起動する。
///
/// 起動前に `draft_accent_hex` を現 config 値で re-seed する (= stale draft
/// 防止、 codex PR #11 finding)。 Pick... trigger で dormant modal が初めて
/// 実運用到達可能になったため、 インライン Accent color (hex) 編集後 or 過去
/// modal cancel 後に open すると stale draft が表示される bug を補完。
/// [`crate::modals::cancel_accent_picker`] の cancel/reset 後 re-seed と同
/// family の invariant を trigger 経路でも保証する。
///
/// その後 `accent_picker_visible.set(true)` で main.rs `ReactiveOverlayContainer`
/// の binding が次 frame sync で overlay を show する。
fn open_accent_picker(state: &AppStateHandles) {
    let current = state.config.get().appearance.accent_hex.clone();
    state.draft_accent_hex.set(current);
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

    // ── Phase 3a: ThemeId <-> AppTheme mapping ─────────────────────────

    /// app_theme_for は 6 variant 全てで panic せず AppTheme を返す + 正しい
    /// preset に map する (= HayateOriginal → app_theme_hayate_original 等を
    /// button.bg field で identity 確認)。 match exhaustive なので variant 漏れは
    /// compile-time で別途保証。
    #[test]
    fn app_theme_for_maps_each_variant_to_its_preset() {
        // HayateOriginal / Win95 を代表 identity 確認 (= 正しい preset へ map)。
        assert_eq!(
            app_theme_for(ThemeId::HayateOriginal).button.bg,
            app_theme_hayate_original().button.bg,
            "HayateOriginal → app_theme_hayate_original"
        );
        assert_eq!(
            app_theme_for(ThemeId::Win95).button.bg,
            app_theme_win95().button.bg,
            "Win95 → app_theme_win95"
        );
        // 全 6 variant smoke (= panic なく construct)。
        for id in [
            ThemeId::HayateOriginal,
            ThemeId::Win95,
            ThemeId::XpLuna,
            ThemeId::Win10,
            ThemeId::MacOs9,
            ThemeId::MacOsBigSur,
        ] {
            let _theme = app_theme_for(id);
        }
    }

    /// 異なる ThemeId は識別可能な AppTheme を返す (= mapping が定数ではない)。
    /// HayateOriginal (白 button bg) vs Win95 (灰 button bg) を button.bg で対比。
    #[test]
    fn app_theme_for_distinct_themes_differ() {
        let hayate = app_theme_for(ThemeId::HayateOriginal);
        let win95 = app_theme_for(ThemeId::Win95);
        assert_ne!(
            hayate.button.bg, win95.button.bg,
            "HayateOriginal と Win95 は異なる button bg (= 同一 preset 返却バグ検出)"
        );
    }

    /// theme_for は skin ごとに異なる base palette を返す (= 同一 palette 返却 /
    /// 取り違えバグ検出)。base palette は ACTIVE_THEME に入り背景/active_theme()
    /// widget の色を決めるので、 swap で実際に変わることが Step 2 の核。
    #[test]
    fn theme_for_maps_distinct_palettes() {
        assert_ne!(
            theme_for(ThemeId::Win95).bg_primary,
            theme_for(ThemeId::Win10).bg_primary,
            "Win95 と Win10 は異なる base palette bg_primary"
        );
        assert_ne!(
            theme_for(ThemeId::HayateOriginal).bg_primary,
            theme_for(ThemeId::MacOs9).bg_primary,
            "HayateOriginal と MacOs9 は異なる base palette bg_primary"
        );
    }

    /// theme_bundle_for は base palette ([`theme_for`]) と per-widget AppTheme
    /// ([`app_theme_for`]) を pair にする (= set_bundle が両 half を一緒に運ぶ
    /// Step 2 の核、 旧 set() の palette 据置 gap の root-fix)。
    #[test]
    fn theme_bundle_for_pairs_palette_and_app_theme() {
        for id in [ThemeId::Win95, ThemeId::MacOsBigSur] {
            let bundle = theme_bundle_for(id);
            assert_eq!(
                bundle.theme.bg_primary,
                theme_for(id).bg_primary,
                "bundle.theme = theme_for(id) の base palette"
            );
            assert_eq!(
                bundle.app_theme.button.bg,
                app_theme_for(id).button.bg,
                "bundle.app_theme = app_theme_for(id) の AppTheme"
            );
        }
    }

    /// label ⇔ ThemeId 双方向 map が全 6 variant で round-trip する。
    #[test]
    fn theme_label_id_round_trip_all_variants() {
        for (id, label) in THEME_CHOICES {
            assert_eq!(theme_label_for(id), label, "id → label");
            assert_eq!(theme_id_from_label(label), Some(id), "label → id");
        }
    }

    /// 未知 label は None (= on_select で no-op、 destructive 上書き回避)。
    #[test]
    fn theme_id_from_unknown_label_is_none() {
        assert_eq!(theme_id_from_label("Sepia (future)"), None);
        assert_eq!(theme_id_from_label(""), None);
    }

    /// Theme ComboBox on_select 相当: apply_theme_selection が config.theme_id を
    /// 更新 + debouncer 発火する (= persist 側、 handle.set は None path で skip)。
    /// handle.set → runtime swap は GUI runtime 経路ゆえ unit test では観測不可、
    /// visual smoke (user) で確認。
    #[test]
    fn apply_theme_selection_updates_config_and_requests_save() {
        let state = crate::state::for_testing();
        // for_testing は theme_handle = None (= App 不在)、 handle.set は skip され
        // config/debouncer のみ更新される path を assert。
        assert!(state.theme_handle.is_none(), "for_testing は handle None");
        assert_eq!(
            state.config.get().appearance.theme_id,
            ThemeId::HayateOriginal,
            "initial = default"
        );
        apply_theme_selection(&state, ThemeId::Win95);
        assert_eq!(
            state.config.get().appearance.theme_id,
            ThemeId::Win95,
            "config.theme_id 反映"
        );
        assert!(
            state.debouncer.borrow().has_pending(),
            "debouncer 発火 (= persist 予約)"
        );
    }

    /// apply_theme_selection は他の appearance config field を touch しない
    /// (= theme_id のみ更新、 accent_hex / font_size_scale 不変)。
    #[test]
    fn apply_theme_selection_leaves_other_appearance_fields_intact() {
        let state = crate::state::for_testing();
        let accent_before = state.config.get().appearance.accent_hex.clone();
        let font_before = state.config.get().appearance.font_size_scale;
        apply_theme_selection(&state, ThemeId::MacOs9);
        assert_eq!(state.config.get().appearance.accent_hex, accent_before);
        assert!(
            (state.config.get().appearance.font_size_scale - font_before).abs() < f32::EPSILON
        );
        assert_eq!(state.config.get().appearance.theme_id, ThemeId::MacOs9);
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
    /// (= modal 起動 + draft re-seed のみ、 config commit は modal 内 Apply path
    /// の責務、 debouncer も Apply まで発火しない)。
    #[test]
    fn open_accent_picker_does_not_touch_config_or_debouncer() {
        let state = crate::state::for_testing();
        let snapshot = state.config.get().clone();
        open_accent_picker(&state);
        assert_eq!(*state.config.get(), snapshot, "config 不変");
        assert!(!state.debouncer.borrow().has_pending(), "save request なし");
    }

    /// codex PR #11 finding regression: open 時に draft_accent_hex が現 config
    /// 値で re-seed される (= stale draft 防止)。 config を non-default に seed
    /// してから draft を別値に汚した状態で open → draft が config 値へ復元
    /// されることを fixation (= 誤って set(true) のみで draft 据置の実装は fail)。
    #[test]
    fn open_accent_picker_reseeds_draft_to_current_config() {
        let state = crate::state::for_testing();

        // config を non-default 値に seed (= "current config" を明示化)
        let current_config_hex = String::from("#FF0000");
        state
            .config
            .update(|c| c.appearance.accent_hex = current_config_hex.clone());

        // draft を stale 値に汚す (= 過去 inline 編集 / modal cancel 漏れ simulate)
        let stale_draft = String::from("#00FF00");
        state.draft_accent_hex.set(stale_draft.clone());
        assert_ne!(
            *state.draft_accent_hex.get(),
            current_config_hex,
            "test fixation 前提: open 前は draft が config と乖離 (= stale)"
        );

        // open → draft が現 config 値に re-seed
        open_accent_picker(&state);
        assert_eq!(
            *state.draft_accent_hex.get(),
            current_config_hex,
            "open で draft が現 config 値 ({}) に re-seed (= stale draft 防止、 \
             set(true) のみで draft 据置の実装は本 assert で fail)",
            current_config_hex
        );
        assert!(*state.accent_picker_visible.get(), "modal visible = true");
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
