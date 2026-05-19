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

mod lang;

// 規範整合性: `use hayate_kit::...` のみ、`use hayate_platform::...` 禁止
use clap::Parser;
use hayate_kit::style::widget_theme_presets::app::app_theme_hayate_original;
use hayate_kit::style::widget_theme_presets::titlebar::titlebar_theme_hayate_original;
use hayate_kit::widget::form_layout::FormLayout;
use hayate_kit::widget::label::LabelWidget;
use hayate_kit::widget::split_view::{SplitOrientation, SplitViewWidget};
use hayate_kit::widget::tree_view::{TreeNode, TreeViewWidget};
use hayate_kit::{App, Widget, HAYATE_ORIGINAL};

use crate::lang::{Lang, Strings};

/// Settings panel for hayate-kit, an embedded GUI framework.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Safe boot mode: delete saved config and exit (= recovery path、
    /// R12 mitigation for bricked state、RFC v0.2 §5.1 deliverable 7 mandatory)
    #[arg(long)]
    reset_config: bool,

    /// UI language override (ja | en、case-insensitive)。
    /// Unset → LANG env var auto-detect (= ja_JP.UTF-8 → ja、その他 → en)
    #[arg(long)]
    lang: Option<String>,
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
fn reset_config(lang: Lang) -> Result<(), Box<dyn std::error::Error>> {
    let strings = lang.strings();
    let cfg = config_path();
    if cfg.exists() {
        let backup = cfg.with_extension("json.bak");
        std::fs::rename(&cfg, &backup)?;
        println!("{}: {}", strings.reset_config_done, cfg.display());
        println!("{}: {}", strings.reset_config_backup, backup.display());
    } else {
        println!("{}: {}", strings.reset_config_not_found, cfg.display());
    }
    Ok(())
}

/// Sidebar TreeView nav = 5 root sections + Appearance sub-tree (3 sub-nodes)。
/// Phase 1 skeleton stub = navigation のみ、 select callback は未配線 (= Phase 2 で
/// section selected を reactive bind して detail pane swap)。
fn build_sidebar(strings: &'static Strings) -> TreeViewWidget {
    let nodes = vec![
        TreeNode::new(strings.section_general),
        TreeNode::new(strings.section_appearance).with_children(vec![
            TreeNode::new(strings.section_appearance_theme),
            TreeNode::new(strings.section_appearance_font),
            TreeNode::new(strings.section_appearance_color),
        ]),
        TreeNode::new(strings.section_accessibility),
        TreeNode::new(strings.section_ime),
        TreeNode::new(strings.section_advanced),
    ];
    TreeViewWidget::new(nodes)
}

/// Detail pane = "General" section の placeholder FormLayout (= Phase 1 stub)。
/// Phase 2 で section selected の reactive bind + 各 section の form 内容を
/// 順次 implementation。
fn build_detail(strings: &'static Strings) -> Box<dyn Widget> {
    // FormLayout に "Coming soon" placeholder field を 1 row。
    // label widget は HAYATE_DARK fg hardcoded 既知 limitation ([[R13]])、
    // explicit .with_color() で text-primary override (= 風韻 warm dark)
    let coming_soon = LabelWidget::new(strings.coming_soon, 14.0).with_color(42, 41, 37);
    let heading = LabelWidget::new(strings.section_general, 18.0).with_color(42, 41, 37);
    let form = FormLayout::new().row(strings.section_general, coming_soon);

    // VStack { heading + form } で section heading の上に form
    use hayate_kit::widget::layout::VStack;
    let mut stack = VStack::new(16.0); // space-md per RFC §3.4
    stack = stack.add(Box::new(heading));
    stack = stack.add(Box::new(form));
    Box::new(stack)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Lang resolution: CLI override が最優先、不在時 LANG env 自動検出 fallback
    let lang = Lang::from_cli(cli.lang.as_deref()).unwrap_or_else(Lang::detect);
    let strings = lang.strings();

    // Safe boot mode: reset + early exit
    if cli.reset_config {
        return reset_config(lang);
    }

    // Normal launch: App builder chain = HAYATE Original 3-builder
    let app = App::new(strings.app_title, 800, 540)
        .with_theme(&HAYATE_ORIGINAL)
        .with_titlebar_theme(titlebar_theme_hayate_original())
        .with_app_theme(app_theme_hayate_original())
        .with_min_size(560, 400);

    // Phase 1 step 6 skeleton: SplitView { Sidebar (TreeView nav) + Detail pane }
    // HStack ではなく SplitView を使用する理由 (= R13 fix 後 visual re-verify で発覚):
    // TreeView::layout() は constraints.max_width を常に claim する design、
    // HStack::add(child=flex 0) は unbounded constraint を渡すため TreeView が
    // infinity 占有 → detail pane 0px。 SplitView は ratio + min_sizes 経路で
    // proportional split を保証、 sidebar と detail 両方 visible に。
    let sidebar = build_sidebar(strings);
    let detail = build_detail(strings);
    let root = SplitViewWidget::new(Box::new(sidebar), detail, SplitOrientation::Horizontal)
        .with_ratio(0.3) // sidebar = 30%、 detail = 70%
        .with_min_sizes(180.0, 400.0); // sidebar 最低 180px、 detail 最低 400px

    app.run(Box::new(root))
}
