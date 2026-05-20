//! IME section (= RFC v0.5 §5.2.4)。
//!
//! ## fields:
//! - IME backend ComboBox (= `ibus` / `fcitx5`、fcitx5 known limitation note 表示)
//! - Candidate window position ComboBox (= `Cursor anchor` / `Widget bottom`、
//!   track-ime-popup-x-anchor debt link)
//! - Preedit display style ComboBox (= `Inline` / `Floating`)
//!
//! ## DTP reuse note
//! IME section は DTP app (= 縦書き IME 含む future) でも reuse 想定、
//! [[project_dtp_app_roadmap]] forward path。本 file の wire pattern が
//! そのまま縦書き IME の backend 選択にも適用可能。
//!
//! ## wave 3b — persistence wire 配線済
//! 各 ComboBox の `on_select` → [`apply_*`] helper → `state.config` 更新 +
//! debouncer 発火。closure は widget の中に閉じ込められて呼び出せないため、
//! helper fn 抽出で unit test が 1:1 に exercise する設計。

use hayate_kit::prelude::*;

use crate::lang::Strings;
use crate::persistence::{CandidatePosition, ImeBackend, PreeditStyle};
use crate::state::AppStateHandles;

// ── ComboBox label literal (= source of truth for both UI + callback) ──

const BACKEND_IBUS: &str = "ibus";
const BACKEND_FCITX5: &str = "fcitx5";
const CAND_POS_CURSOR: &str = "Cursor anchor";
const CAND_POS_BOTTOM: &str = "Widget bottom";
const PREEDIT_INLINE: &str = "Inline";
const PREEDIT_FLOATING: &str = "Floating";

pub fn build(strings: &'static Strings, state: &AppStateHandles) -> Box<dyn Widget> {
    let heading = LabelWidget::new(strings.section_ime, 18.0);

    let backend = ComboBoxWidget::new(vec![BACKEND_IBUS.to_string(), BACKEND_FCITX5.to_string()])
        .on_select({
            let s = state.clone();
            move |selected| apply_backend_selection(&s, selected)
        });
    let candidate_pos = ComboBoxWidget::new(vec![
        CAND_POS_CURSOR.to_string(),
        CAND_POS_BOTTOM.to_string(),
    ])
    .on_select({
        let s = state.clone();
        move |selected| apply_candidate_position_selection(&s, selected)
    });
    let preedit_style =
        ComboBoxWidget::new(vec![PREEDIT_INLINE.to_string(), PREEDIT_FLOATING.to_string()])
            .on_select({
                let s = state.clone();
                move |selected| apply_preedit_style_selection(&s, selected)
            });

    let form = FormLayout::new()
        .row("IME backend", backend)
        .row("Candidate position", candidate_pos)
        .row("Preedit style", preedit_style);

    // fcitx5 known limitation note (= 既存 GUI_kit dogfood で verify 済の制約)
    let fcitx5_note = LabelWidget::new(
        "Note: fcitx5 = preedit display 一部制約あり (= track-ime-popup-x-anchor debt 参照)",
        12.0,
    );

    let mut stack = VStack::new(16.0);
    stack = stack.add(Box::new(heading));
    stack = stack.add(Box::new(form));
    stack = stack.add(Box::new(fcitx5_note));
    Box::new(stack)
}

// ── on_select helpers (= 1:1 unit-test exercise 用に抽出) ───────────────

/// 未知 label は 既存値を変更しないという保守的 fallback (= future ComboBox
/// 拡張で旧 enum に対応 variant が無いケース、 destructive 上書き回避)。
fn apply_backend_selection(state: &AppStateHandles, selected: &str) {
    let Some(value) = parse_backend(selected) else {
        return;
    };
    state.config.update(|c| c.ime.backend = value);
    state.debouncer.borrow().request();
}

fn apply_candidate_position_selection(state: &AppStateHandles, selected: &str) {
    let Some(value) = parse_candidate_position(selected) else {
        return;
    };
    state.config.update(|c| c.ime.candidate_position = value);
    state.debouncer.borrow().request();
}

fn apply_preedit_style_selection(state: &AppStateHandles, selected: &str) {
    let Some(value) = parse_preedit_style(selected) else {
        return;
    };
    state.config.update(|c| c.ime.preedit_style = value);
    state.debouncer.borrow().request();
}

fn parse_backend(label: &str) -> Option<ImeBackend> {
    match label {
        BACKEND_IBUS => Some(ImeBackend::Ibus),
        BACKEND_FCITX5 => Some(ImeBackend::Fcitx5),
        _ => None,
    }
}

fn parse_candidate_position(label: &str) -> Option<CandidatePosition> {
    match label {
        CAND_POS_CURSOR => Some(CandidatePosition::CursorAnchor),
        CAND_POS_BOTTOM => Some(CandidatePosition::WidgetBottom),
        _ => None,
    }
}

fn parse_preedit_style(label: &str) -> Option<PreeditStyle> {
    match label {
        PREEDIT_INLINE => Some(PreeditStyle::Inline),
        PREEDIT_FLOATING => Some(PreeditStyle::Floating),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Lang;

    #[test]
    fn build_returns_non_empty_widget_tree() {
        let strings = Lang::En.strings();
        let state = crate::state::for_testing();
        let _root = build(strings, &state);
    }

    #[test]
    fn backend_selection_propagates_and_requests_save() {
        let state = crate::state::for_testing();
        assert_eq!(state.config.get().ime.backend, ImeBackend::Ibus); // default
        apply_backend_selection(&state, BACKEND_FCITX5);
        assert_eq!(state.config.get().ime.backend, ImeBackend::Fcitx5);
        assert!(state.debouncer.borrow().has_pending());
    }

    #[test]
    fn backend_selection_unknown_label_is_no_op() {
        let state = crate::state::for_testing();
        state.config.update(|c| c.ime.backend = ImeBackend::Fcitx5);
        apply_backend_selection(&state, "zzz_future_backend");
        assert_eq!(state.config.get().ime.backend, ImeBackend::Fcitx5);
        assert!(!state.debouncer.borrow().has_pending());
    }

    #[test]
    fn candidate_position_selection_propagates() {
        let state = crate::state::for_testing();
        apply_candidate_position_selection(&state, CAND_POS_BOTTOM);
        assert_eq!(
            state.config.get().ime.candidate_position,
            CandidatePosition::WidgetBottom
        );
        assert!(state.debouncer.borrow().has_pending());
    }

    #[test]
    fn preedit_style_selection_propagates() {
        let state = crate::state::for_testing();
        apply_preedit_style_selection(&state, PREEDIT_FLOATING);
        assert_eq!(
            state.config.get().ime.preedit_style,
            PreeditStyle::Floating
        );
        assert!(state.debouncer.borrow().has_pending());
    }

    #[test]
    fn parse_helpers_round_trip_every_known_variant() {
        assert_eq!(parse_backend(BACKEND_IBUS), Some(ImeBackend::Ibus));
        assert_eq!(parse_backend(BACKEND_FCITX5), Some(ImeBackend::Fcitx5));
        assert_eq!(
            parse_candidate_position(CAND_POS_CURSOR),
            Some(CandidatePosition::CursorAnchor)
        );
        assert_eq!(
            parse_candidate_position(CAND_POS_BOTTOM),
            Some(CandidatePosition::WidgetBottom)
        );
        assert_eq!(parse_preedit_style(PREEDIT_INLINE), Some(PreeditStyle::Inline));
        assert_eq!(
            parse_preedit_style(PREEDIT_FLOATING),
            Some(PreeditStyle::Floating)
        );
    }
}
