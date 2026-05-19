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
mod modals;
mod persistence;
mod search;
mod sections;
mod state;

// 規範整合性: `use hayate_kit::...` のみ、`use hayate_platform::...` 禁止
use clap::Parser;
use hayate_kit::style::widget_theme_presets::app::app_theme_hayate_original;
use hayate_kit::style::widget_theme_presets::titlebar::titlebar_theme_hayate_original;
use hayate_kit::widget::default_chrome::build_systemlike;
use hayate_kit::widget::layout::VStack;
use hayate_kit::widget::split_view::{SplitOrientation, SplitViewWidget};
use hayate_kit::widget::tree_view::{TreeNode, TreeViewWidget};
use hayate_kit::{App, Decorations, ReactiveRuntime, Widget, WindowPolicy, HAYATE_ORIGINAL};

use crate::lang::{Lang, Strings};
use crate::state::AppStateHandles;

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

/// Sidebar = Search bar (上) + TreeView nav (下) を VStack で積む構成
/// (= Phase 2 wave 2 batch 3 で拡張、 RFC v0.5 §5.2.7 search bar integration)。
///
/// TreeView nav = 5 root sections + Appearance sub-tree (3 sub-nodes)。
/// select callback / search filter → selection 更新 reactive bind は wave 3b
/// dispatch dep、 本 wave 3a では widget composition + state handle 渡しのみ。
fn build_sidebar(strings: &'static Strings, state: &AppStateHandles) -> Box<dyn Widget> {
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
    let tree = TreeViewWidget::new(nodes);

    let mut stack = VStack::new(8.0);
    stack = stack.add(search::build(strings, state));
    stack = stack.add(Box::new(tree));
    Box::new(stack)
}

/// Detail pane = selected section の widget tree (= Phase 2 wave 3a で state 配線)。
///
/// 各 section build() に state handle を渡す signature。 wave 3a 時点では state は
/// section 側で未使用、 wave 3b dispatch で各 worker が widget callback wire を
/// 配線する際に使用化される。
///
/// 各 section impl は src/sections/{general,appearance,accessibility,ime,
/// advanced}.rs に分離。 R13 systemic fix 完遂後は LabelWidget 内 hardcoded
/// HAYATE_DARK 問題解消、 caller .with_color() override は不要 (= 各 section
/// module で active_theme() 経由)。
fn build_detail(
    strings: &'static Strings,
    section: sections::SectionId,
    state: &AppStateHandles,
) -> Box<dyn Widget> {
    match section {
        sections::SectionId::General => sections::general::build(strings, state),
        sections::SectionId::Appearance => sections::appearance::build(strings, state),
        sections::SectionId::Accessibility => sections::accessibility::build(strings, state),
        sections::SectionId::Ime => sections::ime::build(strings, state),
        sections::SectionId::Advanced => sections::advanced::build(strings, state),
    }
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

    // ── Phase 2 wave 3a: ReactiveRuntime + persistence load + AppStateHandles ──
    //
    // 起動時 persistence load (= 不在 / 不正 / 旧 schema いずれも Config::default
    // へ recoverable fallback、 io::Error のみ伝播)。 file 不在ケースは
    // persistence::load 内で NotFound → Config::default 扱い。
    let cfg_path = config_path();
    let initial_config = match persistence::load(&cfg_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "WARN: hayate-kit-settings: failed to read config from {}: {e}; using defaults",
                cfg_path.display()
            );
            persistence::Config::default()
        }
    };

    // ReactiveRuntime と Scheduler を構築、 dirty_flag を App::with_reactive と
    // AppStateHandles の両方が共有する形で配線 (= State<T> 変更 → 自動 repaint trigger)
    let runtime = ReactiveRuntime::new();
    let dirty_flag = runtime.scheduler().dirty_flag();
    let app_state = AppStateHandles::new(&runtime, initial_config, cfg_path);

    // Normal launch: App builder chain = HAYATE Original 全 opt-in hard-baked
    // (= RFC v0.2 §1.3 「全 opt-in pattern hard-baked」規範整合)
    // Decorations::SystemLike + build_systemlike で close/min/max button 標準装備
    let titlebar = titlebar_theme_hayate_original();
    let policy = WindowPolicy::default();
    let app = App::new(strings.app_title, 800, 540)
        .with_theme(&HAYATE_ORIGINAL)
        .with_titlebar_theme(titlebar.clone())
        .with_app_theme(app_theme_hayate_original())
        .with_window_policy(policy.clone())
        .with_min_size(560, 400)
        .with_reactive(dirty_flag);

    // Decorations::SystemLike opt-in (= R12 + RFC v0.2 §1.3 hard-baked、
    // close/min/max button 標準装備、 default = Decorations::Borderless から
    // 明示 opt-in する規範)
    let window_action = app.window_action();
    let current_title = app.current_title();
    let chrome = build_systemlike(
        strings.app_title,
        &window_action,
        &policy,
        Some(&titlebar),
        &current_title,
    );
    let app = app.with_decorations(Decorations::SystemLike(chrome));

    // Phase 1 step 6 skeleton: SplitView { Sidebar (TreeView nav) + Detail pane }
    // HStack ではなく SplitView を使用する理由 (= R13 fix 後 visual re-verify で発覚):
    // TreeView::layout() は constraints.max_width を常に claim する design、
    // HStack::add(child=flex 0) は unbounded constraint を渡すため TreeView が
    // infinity 占有 → detail pane 0px。 SplitView は ratio + min_sizes 経路で
    // proportional split を保証、 sidebar と detail 両方 visible に。
    let sidebar = build_sidebar(strings, &app_state);
    let detail = build_detail(strings, sections::SectionId::default(), &app_state);
    let root = SplitViewWidget::new(sidebar, detail, SplitOrientation::Horizontal)
        .with_ratio(0.3) // sidebar = 30%、 detail = 70%
        .with_min_sizes(180.0, 400.0); // sidebar 最低 180px、 detail 最低 400px

    app.run(Box::new(root))
}
