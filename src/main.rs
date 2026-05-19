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
//! `workspace/president-notes/hayate-kit-settings-rfc-v0.1.md` v0.3
//! (GUI_kit repo 内、Phase 0 spec)

// 規範整合性: `use hayate_kit::...` のみ、`use hayate_platform::...` 禁止
use clap::Parser;
use hayate_kit::style::widget_theme_presets::app::app_theme_hayate_original;
use hayate_kit::style::widget_theme_presets::titlebar::titlebar_theme_hayate_original;
use hayate_kit::widget::label::LabelWidget;
use hayate_kit::{App, HAYATE_ORIGINAL};

/// Settings panel for hayate-kit, an embedded GUI framework.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Safe boot mode: delete saved config and exit (= recovery path、
    /// R12 mitigation for bricked state、RFC v0.2 §5.1 deliverable 7 mandatory)
    #[arg(long)]
    reset_config: bool,
}

/// Resolve XDG_CONFIG_HOME/hayate-kit-settings/config.json path with
/// `$HOME/.config` fallback (= XDG Base Directory Specification minimal impl)
fn config_path() -> std::path::PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").unwrap_or_default();
            std::path::PathBuf::from(home).join(".config")
        });
    base.join("hayate-kit-settings").join("config.json")
}

/// Safe boot mode action: rename existing config to .bak then report path,
/// exit without GUI launch (= R12 mitigation per RFC v0.2 §6.1)
fn reset_config() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = config_path();
    if cfg.exists() {
        let backup = cfg.with_extension("json.bak");
        std::fs::rename(&cfg, &backup)?;
        println!("Reset config: {}", cfg.display());
        println!("Backup saved: {}", backup.display());
    } else {
        println!("Config not found (already in default state): {}", cfg.display());
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Safe boot mode: reset + early exit (= bricked state recovery、GUI launch
    // 前に config 削除で次回起動時 default state 復帰)
    if cli.reset_config {
        return reset_config();
    }

    // Normal launch: App builder chain = HAYATE Original 3-builder
    let app = App::new("hayate-kit-settings — HAYATE Original prototype", 720, 480)
        .with_theme(&HAYATE_ORIGINAL)
        .with_titlebar_theme(titlebar_theme_hayate_original())
        .with_app_theme(app_theme_hayate_original())
        .with_min_size(480, 320);

    // Phase 1 minimal placeholder = single-line Label。
    // explicit .with_color() で HAYATE_ORIGINAL.fg_primary を override
    // (= LabelWidget::new() default は HAYATE_DARK.fg_primary hardcoded = light grey、
    // warm light bg では invisible。framework limitation、RFC v0.3 R13 debt 登録済)。
    let placeholder = LabelWidget::new(
        "HAYATE Original visual prototype  /  風藍 accent #5A8BA8",
        14.0,
    )
    .with_color(42, 41, 37); // text-primary #2A2925

    app.run(Box::new(placeholder))
}
