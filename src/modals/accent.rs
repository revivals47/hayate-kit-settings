//! Accent color picker modal (= RFC v0.5 §5.2.8、 accent hex 入力 + preview
//! swatch + WCAG contrast check warning UI)。
//!
//! wave 3c-pre module split で `modals.rs` から分離 (pure refactor、 behavior
//! 変更ゼロ)。 後続 track2 (= accent) の主担当領域。

use std::rc::Rc;

use hayate_kit::widget::button::ButtonWidget;
use hayate_kit::widget::label::LabelWidget;
use hayate_kit::widget::layout::{HStack, VStack};
use hayate_kit::widget::text_input_widget::TextInputWidget;
use hayate_kit::Widget;

use crate::lang::Strings;
use crate::state::AppStateHandles;

use super::overlay::LabelRef;
use super::wcag::{compute_wcag_status, parse_hex_or_default, update_preview_text};
use super::DEFAULT_ACCENT_HEX;

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
///
/// 末尾で `draft_accent_hex` を `state.config.appearance.accent_hex` に re-seed
/// する。 これにより user が cancel した編集分は次回 modal reopen 時に表示
/// されず、 stale draft で誤って Enter accept される race が解消される
/// (= PR #8 codex 査読 real bug fix、 inline TextInput 経由 accent_hex 変更後
/// に modal 経由再編集する path も同 re-seed で current 値 reflect)。
pub(crate) fn cancel_accent_picker(state: &AppStateHandles) {
    state.accent_picker_visible.set(false);
    let current = state.config.get().appearance.accent_hex.clone();
    state.draft_accent_hex.set(current);
}

// ── widget composition (= wave 3b 視覚 wire 完成、 動的 preview/WCAG 配線済) ──

/// Accent color picker modal (= wave 3b 視覚 wire 完成版)。
///
/// 構造: `VStack { hex TextInput + preview swatch Label + WCAG ratio Label +
/// HStack { Cancel ButtonWidget + Apply ButtonWidget } }`。
///
/// ## 配線 (= wave 3b visual layer)
/// - hex TextInput.on_change: 入力ごとに [`parse_hex_or_default`] で RGB 抽出、
///   [`update_preview_text`] で preview label を更新、 [`compute_wcag_status`]
///   で WCAG ratio を再計算して wcag label を更新、 さらに state.draft_accent_hex
///   に raw 文字列を保存 (= Apply 押下時の commit 元)
/// - Cancel button on_click: [`cancel_accent_picker`] で visible flag clear
/// - Apply button on_click: state.draft_accent_hex から raw 文字列を読出して
///   [`apply_accent_picker`] に渡す (= 動的 hex 反映、 DEFAULT_ACCENT_HEX 固定 placeholder 廃止)
/// - preview / WCAG label は [`LabelRef::new_pair`] 経由 Rc<RefCell> 共有 mutate 配線
#[allow(dead_code)] // wave 3c 以降 main.rs から呼ばれる
pub fn build_accent_picker(_strings: &'static Strings, state: &AppStateHandles) -> Box<dyn Widget> {
    // initial preview / WCAG label content (= state.draft_accent_hex 現値 base)
    let initial_hex = state.draft_accent_hex.get().clone();
    let (ir, ig, ib) = parse_hex_or_default(&initial_hex);
    let preview_label = LabelWidget::new(update_preview_text(&initial_hex, ir, ig, ib), 13.0);
    let wcag_label = LabelWidget::new(compute_wcag_status(&initial_hex, ir, ig, ib), 12.0);

    // LabelRef shim 経由で Widget tree 投入 widget と外部 mutate handle を分離。
    // handle clone は on_change closure が capture、 widget は VStack に投入。
    let (preview_widget, preview_handle) = LabelRef::new_pair(preview_label);
    let (wcag_widget, wcag_handle) = LabelRef::new_pair(wcag_label);

    // TextInput::on_change closure (= PR #155 公開 API、 入力 mutation 経路毎に発火)。
    // 各 frame の paint 内 set_text 呼出が dirty flag を立て、 framework が次 frame
    // で repaint trigger。 ReactiveOverlayContainer 配下 layout は overlay slot
    // cached_size 経由で text 拡張に追従 (= 文字列長変化で auto reflow)。
    let on_change_state = state.clone();
    let preview_for_change = Rc::clone(&preview_handle);
    let wcag_for_change = Rc::clone(&wcag_handle);
    let hex_input = TextInputWidget::new()
        .with_placeholder(DEFAULT_ACCENT_HEX)
        .with_width(160.0)
        .on_change(move |new_text: &str| {
            let (r, g, b) = parse_hex_or_default(new_text);
            preview_for_change
                .borrow_mut()
                .set_text(&update_preview_text(new_text, r, g, b));
            wcag_for_change
                .borrow_mut()
                .set_text(&compute_wcag_status(new_text, r, g, b));
            on_change_state.draft_accent_hex.set(new_text.to_string());
        });

    let cancel_state = state.clone();
    let cancel_btn = ButtonWidget::new("Cancel").on_click(move || {
        cancel_accent_picker(&cancel_state);
    });
    let apply_state = state.clone();
    let apply_btn = ButtonWidget::new("Apply").on_click(move || {
        // draft_accent_hex 経由 user input 反映 (= wave 3b 視覚 wire で動的化、
        // DEFAULT_ACCENT_HEX 固定 placeholder は廃止)。 draft は on_change で
        // 都度更新済、 ここでは現値 snapshot を取って apply_accent_picker に転送。
        let draft = apply_state.draft_accent_hex.get().clone();
        apply_accent_picker(&apply_state, &draft);
    });

    let mut buttons = HStack::new(12.0);
    buttons = buttons.add(Box::new(cancel_btn));
    buttons = buttons.add(Box::new(apply_btn));

    let mut stack = VStack::new(8.0);
    stack = stack.add(Box::new(hex_input));
    stack = stack.add(preview_widget);
    stack = stack.add(wcag_widget);
    stack = stack.add(Box::new(buttons));
    Box::new(stack)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Lang;
    use crate::persistence::Config;
    use crate::state::for_testing;

    #[test]
    fn build_accent_picker_smoke() {
        let strings = Lang::En.strings();
        let state = for_testing();
        let _root = build_accent_picker(strings, &state);
    }

    // State<bool> visible toggle 観測 (= 外部 set/get で reactive flag を制御可)
    #[test]
    fn accent_picker_visible_toggle_via_state() {
        let state = for_testing();
        assert!(!*state.accent_picker_visible.get(), "initial = false");
        state.accent_picker_visible.set(true);
        assert!(*state.accent_picker_visible.get(), "set(true) reflected");
        state.accent_picker_visible.set(false);
        assert!(!*state.accent_picker_visible.get(), "set(false) reflected");
    }

    // Apply 押下 → state.config に hex 反映 + debouncer pending + visible clear
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

    // Cancel 押下 → state.config 不変 + debouncer 不変 + visible clear
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

    /// Cancel 押下後、 user 編集 draft は config 現値に re-seed される。
    /// 次回 modal reopen 時に stale draft で誤 Enter accept される race を防ぐ。
    ///
    /// 第二 round codex 強化反映 (= PR #8 review): 「current config に戻す」
    /// vs 「Config::default() に戻す」を判別可能化するため、 config を
    /// non-default 値 (例 "#FF0000") に seed してから draft を別値 ("#00FF00") に
    /// edit して cancel、 assert は **"#FF0000"** (= current config 値、 NOT
    /// default "#5A8BA8") を期待。 これにより 実装が誤って Config::default の
    /// accent_hex で re-seed すると test fail する fixation 強化。
    #[test]
    fn cancel_after_draft_edit_reseeds_draft_to_config() {
        let state = for_testing();
        let default_hex = Config::default().appearance.accent_hex.clone();

        // step 1: config を non-default 値に seed (= "current config" を明示化)
        let current_config_hex = String::from("#FF0000");
        assert_ne!(
            current_config_hex, default_hex,
            "test fixation 前提: seed 値は Config::default と異なる"
        );
        state
            .config
            .update(|c| c.appearance.accent_hex = current_config_hex.clone());

        // step 2: draft を更に別値に edit (= TextInput::on_change で draft のみ更新 simulate)
        state.accent_picker_visible.set(true);
        let edited_draft = String::from("#00FF00");
        assert_ne!(
            edited_draft, current_config_hex,
            "test fixation 前提: draft 編集値は config 値と異なる"
        );
        assert_ne!(
            edited_draft, default_hex,
            "test fixation 前提: draft 編集値は Config::default 値とも異なる"
        );
        state.draft_accent_hex.set(edited_draft.clone());
        assert_eq!(*state.draft_accent_hex.get(), edited_draft, "draft 編集反映");
        assert_eq!(
            state.config.get().appearance.accent_hex,
            current_config_hex,
            "config は draft 編集中も touch されない"
        );

        // step 3: Cancel 押下相当
        cancel_accent_picker(&state);

        // step 4: draft が **current config 値** に re-seed (= NOT Config::default 値)
        assert_eq!(
            *state.draft_accent_hex.get(),
            current_config_hex,
            "Cancel で draft が current config 値 ({}) に re-seed されること \
             (= 誤って Config::default 値 ({}) で re-seed する実装は本 assert で fail)",
            current_config_hex,
            default_hex
        );
        assert_ne!(
            *state.draft_accent_hex.get(),
            default_hex,
            "明示否定: draft は Config::default 値に戻らない (= current config 値が source of truth)"
        );
        assert!(!*state.accent_picker_visible.get(), "modal 閉じる");
    }

    // Visible flag は Apply / Cancel いずれの path でも必ず false に落ちる
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

    /// build_accent_picker 構築後、 draft_accent_hex を直接 set すると後続の
    /// Apply path で commit される (= closure capture と State<String> 共有確認)。
    #[test]
    fn build_accent_picker_apply_uses_draft_hex() {
        let state = for_testing();
        let _root = build_accent_picker(Lang::En.strings(), &state);
        // 直接 draft を仕込んでから apply (= on_change closure simulation)
        state.draft_accent_hex.set(String::from("#3D6884"));
        // Apply button on_click closure を直接呼ぶ手段はないので、
        // 代替 = apply_accent_picker(state, &draft) を直接呼出して同等性を verify
        let draft_snapshot = state.draft_accent_hex.get().clone();
        apply_accent_picker(&state, &draft_snapshot);
        assert_eq!(state.config.get().appearance.accent_hex, "#3D6884");
    }

    /// build_accent_picker 構築直後の draft_accent_hex は state.config 由来の
    /// initial value と一致 (= AppStateHandles::new の draft_accent_hex 同期と整合)。
    #[test]
    fn build_accent_picker_draft_matches_state_initial() {
        let state = for_testing();
        // state.draft_accent_hex の initial は Config::default().appearance.accent_hex
        // = "#5A8BA8"。 build_accent_picker 自体は draft を mutate しない (=
        // initial read のみ)、 構築後も draft は initial 維持。
        let _root = build_accent_picker(Lang::En.strings(), &state);
        assert_eq!(*state.draft_accent_hex.get(), "#5A8BA8");
    }
}
