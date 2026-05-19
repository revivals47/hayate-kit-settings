//! # hayate-kit-settings
//!
//! GUI_kit framework-native settings panel app (新世代 app 第 1 号)。
//! HAYATE Original aesthetic + theme switcher + accessibility-first design。
//!
//! ## 規範
//! - [[feedback_new_apps_depend_on_gui_kit_only]]: hayate-kit のみ依存、
//!   hayate-platform 直接参照禁止
//! - [[feedback_dogfood_legacy_new_apps_clean_slate]]: 新世代 app stance、
//!   既存 dogfood は legacy 資産
//!
//! ## 関連 RFC
//! `workspace/president-notes/hayate-kit-settings-rfc-v0.1.md` v0.2
//! (GUI_kit repo 内、Phase 0 spec)

// 規範整合性: `use hayate_kit::...` のみ、`use hayate_platform::...` 禁止
use hayate_kit::style::widget_theme_presets::app::app_theme_hayate_original;
use hayate_kit::style::widget_theme_presets::titlebar::titlebar_theme_hayate_original;
use hayate_kit::widget::label::LabelWidget;
use hayate_kit::{App, HAYATE_ORIGINAL};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // App builder chain = HAYATE Original 3-builder (= theme + titlebar + app theme)
    let app = App::new("hayate-kit-settings — HAYATE Original prototype", 720, 480)
        .with_theme(&HAYATE_ORIGINAL)
        .with_titlebar_theme(titlebar_theme_hayate_original())
        .with_app_theme(app_theme_hayate_original())
        .with_min_size(480, 320);

    // Phase 1 minimal placeholder = single-line Label。
    // explicit .with_color() で HAYATE_ORIGINAL.fg_primary を override
    // (= LabelWidget::new() default は HAYATE_DARK.fg_primary hardcoded = light grey、
    // warm light bg では invisible。framework limitation、別 debt 候補)。
    let placeholder = LabelWidget::new(
        "HAYATE Original visual prototype  /  風藍 accent #5A8BA8",
        14.0,
    )
    .with_color(42, 41, 37); // text-primary #2A2925

    app.run(Box::new(placeholder))
}
