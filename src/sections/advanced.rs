//! Advanced section (= RFC v0.5 §5.2.5)。
//!
//! wave 0 = "Coming soon" stub、 wave 1 で 5 field を実 widget で fill。
//!
//! ## fields (= RFC v0.5 §5.2.5):
//! - Debug overlay toggle = `SwitchWidget` (= FPS / hit-test rect / widget tree)
//! - Log level dropdown = `ComboBoxWidget` (= trace / debug / info / warn / error)
//! - Cache clear button = `ButtonWidget`
//! - Reset to defaults = 3 `ButtonWidget` を `HStack` 横並び (= 全 section /
//!   現 section / per-field の 3 button)
//! - Safe boot mode hint = `LabelWidget` (= `--reset-config` CLI flag 案内、
//!   R12 mitigation = bricked state recovery path explicit display)
//!
//! ## wave 1 scope notes
//! - click callback の actual logic (= cache clear / reset / debug overlay
//!   toggle 反映) は wave 2/3 で reactive bind 配線時に注入予定。本 wave 1
//!   では UI placement と placeholder closure (`|_| {}` / `|| {}`) のみ。
//! - HAYATE Original aesthetic は `active_theme()` 経由の default で reach
//!   済 (= GUI_kit R13 systemic fix land 後)。caller-side `.with_color()`
//!   override は不要。

use hayate_kit::widget::button::ButtonWidget;
use hayate_kit::widget::combo_box::ComboBoxWidget;
use hayate_kit::widget::form_layout::FormLayout;
use hayate_kit::widget::label::LabelWidget;
use hayate_kit::widget::layout::{HStack, VStack};
use hayate_kit::widget::switch::SwitchWidget;
use hayate_kit::Widget;

use crate::lang::Strings;
use crate::state::AppStateHandles;

/// Build Advanced section widget tree (= wave 1 fill)。
///
/// ## wave 3a signature 拡張
/// `_state: &AppStateHandles` を受け取るのは wave 3b で debug_overlay on_toggle /
/// log_level on_select → state.config.update、 cache_clear / reset_* on_click →
/// state.reset_confirm_visible.set(true) を配線するため。 wave 3a 時点では
/// 未使用 (`_` prefix で warning suppress)。
pub fn build(strings: &'static Strings, _state: &AppStateHandles) -> Box<dyn Widget> {
    let heading = LabelWidget::new(strings.section_advanced, 18.0);

    // Field 1: Debug overlay toggle (= FPS / hit-test rect / widget tree)
    // wave 1 placeholder = OFF default、callback は wave 2/3 で配線
    let debug_overlay = SwitchWidget::new(false).on_toggle(|_checked| {
        // wave 2/3: debug overlay enable/disable 反映 (= reactive state)
    });

    // Field 2: Log level (= trace / debug / info / warn / error 5 段階)
    // wave 1 placeholder = items 列挙のみ、on_select callback は wave 2/3
    let log_level = ComboBoxWidget::new(vec![
        "trace".to_string(),
        "debug".to_string(),
        "info".to_string(),
        "warn".to_string(),
        "error".to_string(),
    ])
    .on_select(|_level| {
        // wave 2/3: log level 設定反映 (= reactive state + log filter)
    });

    // Field 3: Cache clear button
    // wave 1 placeholder = UI placement のみ、actual clear logic は wave 2/3
    let cache_clear = ButtonWidget::new("Clear cache").on_click(|| {
        // wave 2/3: cache 実 clear (= filesystem / in-memory cache flush)
    });

    // Field 4: Reset to defaults — 3 button を HStack 横並び
    // (= 全 section / 現 section / per-field、Reset の粒度 3 種)
    let reset_all = ButtonWidget::new("Reset all").on_click(|| {
        // wave 2/3: 全 section reset (= persistence layer に default re-write)
    });
    let reset_section = ButtonWidget::new("Reset section").on_click(|| {
        // wave 2/3: 現 section reset
    });
    let reset_field = ButtonWidget::new("Reset field").on_click(|| {
        // wave 2/3: per-field reset (= focused field 単位)
    });
    let reset_row = HStack::new(8.0)
        .add(Box::new(reset_all))
        .add(Box::new(reset_section))
        .add(Box::new(reset_field));

    // Field 5: Safe boot mode hint (= R12 mitigation explicit)
    // hardcoded literal: --reset-config CLI flag 案内
    // (= settings が bricked になった際の recovery path、 RFC v0.5 §5.2.5)
    let safe_boot_hint = LabelWidget::new(
        "Tip: launch with `--reset-config` to start in safe-boot mode (resets all settings).",
        13.0,
    );

    let form = FormLayout::new()
        .row("Debug overlay", debug_overlay)
        .row("Log level", log_level)
        .row("Cache", cache_clear)
        .row("Reset to defaults", reset_row)
        .row("Safe boot", safe_boot_hint);

    let mut stack = VStack::new(16.0);
    stack = stack.add(Box::new(heading));
    stack = stack.add(Box::new(form));
    Box::new(stack)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Lang;

    #[test]
    fn build_smoke_does_not_panic() {
        // smoke: build(strings) が panic せず Box<dyn Widget> を返すこと。
        // 5 field 全 construct + FormLayout::row 5 回 + VStack ネストの
        // structural integrity check (= wave 1 placement のみ、event/paint は
        // wave 2/3 reactive bind 後の visual smoke で verify)。
        let strings = Lang::En.strings();
        let state = crate::state::for_testing();
        let _w = build(strings, &state);
    }
}
