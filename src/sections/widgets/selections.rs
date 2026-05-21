//! Widgets showcase — Selections group (Phase 3b 第4波)。
//!
//! Checkbox / Radio / Switch を選択状態込みで並べ、user が click/hover/focus を
//! 実機評価できる場。group 見出し "Selections" は coordinator (mod.rs) が付ける
//! ため本ファイルでは付けない。
//!
//! FALLBACK 確定: CheckboxWidget / SwitchWidget / RadioGroupWidget には
//! `disabled()` / `set_enabled()` API が存在しない（不足 API-1/2/3、Gap B 同根）。
//! よって今回は **disabled 行を省く**（checked/unchecked + selected + off/on のみ）。
//! disabled 行はこれら 3 widget に `disabled()` を足す framework タスク完了後に
//! 追加する。

use hayate_kit::prelude::*;

use crate::lang::Strings;
use crate::state::AppStateHandles;

/// Build Selections group showcase。
///
/// `strings` / `state` は本 group では未使用（showcase は persistence 不要）。
/// engine は `with_engine` を呼ばず settings tree の `inject_engine` に依存する
/// （既存 section と同じ規範）。
pub fn build(_strings: &'static Strings, _state: &AppStateHandles) -> Box<dyn Widget> {
    // ── Checkbox: unchecked / checked ──
    // checkbox は label 内蔵 → VStack に直接（FormLayout だと二重 label になる）。
    // disabled 行は framework API 追加後（不足 API-1）。
    let mut cb_col = VStack::new(6.0);
    cb_col = cb_col.add(Box::new(CheckboxWidget::new("Checkbox: unchecked")));
    cb_col = cb_col.add(Box::new(CheckboxWidget::new("Checkbox: checked").checked(true)));

    // ── Radio group: selected = Medium ──
    // disabled group は framework API 追加後（不足 API-3）。
    let radio_label = LabelWidget::new("Radio group:", 13.0);
    let radio = RadioGroupWidget::new(&["Small", "Medium", "Large"]).with_selected(1);

    // ── Switch: off / on（label 内蔵なし → FormLayout.row で label を添える）──
    // disabled 行は framework API 追加後（不足 API-2）。
    let switch_form = FormLayout::new()
        .row("Switch: off", SwitchWidget::new(false))
        .row("Switch: on", SwitchWidget::new(true));

    let mut stack = VStack::new(12.0);
    stack = stack.add(Box::new(cb_col));
    stack = stack.add(Box::new(radio_label));
    stack = stack.add(Box::new(radio));
    stack = stack.add(Box::new(switch_form));
    Box::new(stack)
}
