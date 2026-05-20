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

mod detail_container;
mod lang;
mod modals;
mod persistence;
mod search;
mod sections;
mod state;

// 規範整合性: `use hayate_kit::...` のみ、`use hayate_platform::...` 禁止
use clap::Parser;
use hayate_kit::prelude::*;

use crate::detail_container::DetailContainerWidget;
use crate::lang::{Lang, Strings};
use crate::modals::{
    apply_accent_picker, apply_reset_confirm, build_accent_picker, build_reset_confirm,
    ReactiveOverlayContainer,
};
use crate::sections::SectionId;
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

    // TreeView nav 選択 → state.selected_section.set で reactive 配線
    // (= wave 3b worker3 dispatch、 DetailContainerWidget が version polling で
    // 検出 → 該当 section の widget tree に rebuild)。
    // 空 path / 範囲外 path は **現在の selection を維持** (= panic 回避 + UX:
    // 異常 path 押下で detail pane が default = General に強制リセットされる
    // 不自然さを排除、 codex PR #9 finding 3 反映)。
    let nav_state = state.clone();
    let tree = TreeViewWidget::new(nodes).on_select(move |path| {
        let current = *nav_state.selected_section.get();
        let sid = SectionId::from_tree_path(&path).unwrap_or(current);
        nav_state.selected_section.set(sid);
    });

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
    // Phase 3a: persist された theme_id を move 前に取得 (= ThemeId は Copy)、
    // 起動時 initial theme 解決に使用。 initial_config は直後 AppStateHandles へ move。
    let initial_theme_id = initial_config.appearance.theme_id;
    let app_state = AppStateHandles::new(&runtime, initial_config, cfg_path);

    // Normal launch: App builder chain = HAYATE Original 全 opt-in hard-baked
    // (= RFC v0.2 §1.3 「全 opt-in pattern hard-baked」規範整合)
    // Decorations::SystemLike + build_systemlike で close/min/max button 標準装備
    // Phase 3a: 起動時 initial theme = 永続化された theme_id から解決
    // (= app_theme_for mapping)。 旧 config (theme_id 不在) は serde(default) で
    // HayateOriginal に補完されるため、 従来 hard-coded app_theme_hayate_original()
    // と同一 default 挙動を維持しつつ、 persist された skin を起動時に復元する。
    let titlebar = titlebar_theme_hayate_original();
    let policy = WindowPolicy::default();
    let initial_theme = crate::sections::appearance::app_theme_for(initial_theme_id);
    let app = App::new(strings.app_title, 800, 540)
        .with_theme(&HAYATE_ORIGINAL)
        .with_titlebar_theme(titlebar.clone())
        .with_app_theme(initial_theme)
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
    //
    // Detail pane は DetailContainerWidget で wrap、 state.selected_section の
    // 変化を polling 検出して section 用 widget tree を rebuild
    // (= wave 3b worker3 dispatch、 design doc Pattern A = dynamic rebuild)。
    // SplitView 全体は track2 land の ReactiveOverlayContainer の base として渡す
    // ことで 2 worker (= track2 modal lifecycle / track3 reactive detail pane)
    // の root structure を直交 nest で統合 (= step 4 reconciliation)。
    let sidebar = build_sidebar(strings, &app_state);
    let detail: Box<dyn Widget> = Box::new(DetailContainerWidget::new(
        app_state.clone(),
        move |sid, s| build_detail(strings, sid, s),
    ));
    let split = SplitViewWidget::new(sidebar, detail, SplitOrientation::Horizontal)
        .with_ratio(0.3) // sidebar = 30%、 detail = 70%
        .with_min_sizes(180.0, 400.0); // sidebar 最低 180px、 detail 最低 400px

    // Phase 2 wave 3b track2: ReactiveOverlayContainer { OverlayContainer {
    // base: SplitView, overlays: [accent, reset] } } で root を組み立て、
    // State<bool> 駆動 visibility + Escape/Enter pre-intercept + dimming を
    // framework 側 (= hayate_kit::widget::overlay::OverlayContainer) に委譲。
    //
    // PRESIDENT Option β 採択 (= AlertDialog 不使用、 2 modal を VStack 統一)。
    // double-dimming 解消 + visibility coordination 単一 source (= State<bool>) で
    // 一貫性確保 (= [[feedback_platform_principle]] 整合)。
    //
    // overlay 登録順 = bindings 末尾優先で accept/dismiss 走査するため、 末尾に
    // 入れた reset_confirm が Escape/Enter 競合時の優先 dismiss/accept 対象。
    // 通常 UX 上 2 modal 同時 visible にはならない設計だが、 安全側 fallback。
    let accent_overlay = build_accent_picker(strings, &app_state);
    let reset_overlay = build_reset_confirm(strings, &app_state);

    let mut root = ReactiveOverlayContainer::new(Box::new(split));

    // accent picker on_enter = Apply 押下と同等 (= draft_accent_hex 経由 commit)。
    // apply_accent_picker は内部で state.accent_picker_visible.set(false) を呼ぶ
    // ため、 closure が dismiss + action 両 result を提供する。
    let accent_enter_state = app_state.clone();
    root.add_overlay_with_state_and_enter(
        "accent",
        accent_overlay,
        OverlayPosition::Center,
        app_state.accent_picker_visible.clone(),
        move || {
            let draft = accent_enter_state.draft_accent_hex.get().clone();
            apply_accent_picker(&accent_enter_state, &draft);
        },
    );

    // reset confirm on_enter = Reset 押下と同等 (= persistence::reset + default
    // 復元)。 disk 失敗時は WARN + visible flag clear (= build_reset_confirm 内
    // Reset button on_click と同 fallback)。
    let reset_enter_state = app_state.clone();
    root.add_overlay_with_state_and_enter(
        "reset",
        reset_overlay,
        OverlayPosition::Center,
        app_state.reset_confirm_visible.clone(),
        move || {
            if let Err(e) = apply_reset_confirm(&reset_enter_state) {
                eprintln!("WARN: hayate-kit-settings: reset failed: {e}");
                reset_enter_state.reset_confirm_visible.set(false);
            }
        },
    );

    app.run(Box::new(root))
}
