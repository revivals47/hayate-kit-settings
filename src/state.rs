//! Reactive AppState handles (= Phase 2 wave 3a prerequisite scaffolding)。
//!
//! 各 section / modal / search が widget callback から共通参照する State 群
//! と persistence handle を 1 struct に集約。 wave 3a で signature を land、
//! actual callback wire (= on_change → State.update → debouncer.request) は
//! wave 3b で 3 worker 並走 dispatch で fill。
//!
//! ## 設計方針
//! - `State<Config>` 統一 hold: sub-field 編集は `.update(|c| c.section.field = ..)`
//!   pattern で全 section 統一、 persistence の serde round-trip と直接互換
//! - `State<SectionId>` / `State<String>` / `State<bool>` x 2: TreeView selection /
//!   Search query / 2 modal visibility を個別 State (= Clone で section 跨ぎ共有可)
//! - `Rc<RefCell<DebouncedSaver>>`: callback で borrow_mut してから request()
//! - `config_path`: DebouncedSaver poll loop で save flush 時に必要
//!
//! ## DTP reuse
//! 同じ AppStateHandles pattern (= 中央 State 群 + persistence handle) は DTP
//! app でも reuse 想定 ([[project_dtp_app_roadmap]] 整合)。 typography settings /
//! ruby / vertical writing 等の State も同様に追加し、 各 section / canvas が
//! 共通参照する pattern が universal。

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use hayate_kit::prelude::*;

use crate::persistence::{Config, DebouncedSaver};
use crate::sections::SectionId;

/// 粒度別 destructive reset の種別 (= どの範囲を default に戻すか)。
///
/// Advanced section の各 reset button が click 時に [`AppStateHandles::pending_reset`]
/// へ `Some(kind)` を set + `reset_confirm_visible.set(true)`、 reset confirm
/// modal が本値を読んで confirm message + apply action を分岐する
/// (= [`crate::modals::apply_reset_confirm`])。 reset 実行 / cancel で `None` に戻す。
///
/// ## variant scope
/// - [`ResetKind::All`]: 全 section を `Config::default()` に (= disk reset + .bak)
/// - [`ResetKind::Section`]: 指定 section の sub-config のみ default (= request 時の
///   `selected_section` snapshot)
/// - [`ResetKind::Field`]: 単一 field reset。 per-field focus tracking 不在のため
///   representative field = accent_hex (= 唯一 draft state を持つ最も顕著な
///   user-customizable field) に scope。 focus tracking land 時に generic 化する
///   localized change。
/// - [`ResetKind::CacheClear`]: cache flush。 実 cache layer 不在のため現状 no-op
///   + confirm 文言のみ (= 将来 cache 実装時に wire)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetKind {
    /// 全 section を default に。
    All,
    /// 指定 section のみ default に。
    Section(SectionId),
    /// 単一 field (= accent_hex) reset。
    Field,
    /// Cache flush (= 現状 no-op)。
    CacheClear,
}

/// AppStateHandles — 各 section / modal / search が共有する State 群 + persistence
/// handle。 [`Clone`] を経由して widget callback closure が capture 可能。
///
/// `State<T>` は内部 `Rc` で同一 underlying value を共有、 [`Clone`] は cheap。
/// `Rc<RefCell<DebouncedSaver>>` 経由で全 callback が同じ debouncer instance を
/// 共有 (= 500ms quiet-period の真の意味での共通 timer)。
///
/// `dead_code` allow は wave 3a 時点で fields が未読出 (= main.rs / state.rs 内
/// 構築のみ) のため。 wave 3b で各 worker が widget callback wire で全 field を
/// 使用化する想定、 warning は wave 3b PR で自然解消。
#[allow(dead_code)]
#[derive(Clone)]
pub struct AppStateHandles {
    /// 全 section field の persisted state を保持する中央 State。
    /// sub-field 編集は `config.update(|c| c.section.field = ...)` pattern。
    pub config: State<Config>,
    /// TreeView selection / detail pane swap の binding source。
    pub selected_section: State<SectionId>,
    /// Search bar TextInput の current query。 filter_sections 評価に使用。
    pub search_query: State<String>,
    /// Accent color picker modal の show/hide flag (= wave 3b で AlertDialog or
    /// custom modal lifecycle に bind)。
    pub accent_picker_visible: State<bool>,
    /// Reset confirm modal の show/hide flag (= destructive action gate)。
    pub reset_confirm_visible: State<bool>,
    /// 現在 pending 中の reset 種別 (= 粒度別 destructive confirm)。
    /// Advanced section の各 reset button が click 時に `Some(kind)` を set +
    /// `reset_confirm_visible.set(true)`、 reset confirm modal が読んで
    /// confirm message + apply action を分岐。 reset 実行 / cancel で `None` に戻す。
    /// 初期値 `None` (= 未起動)、 modal が `None` を読んだ場合は安全側 fallback で
    /// `ResetKind::All` 相当の文言 / action にする ([`crate::modals::apply_reset_confirm`])。
    pub pending_reset: State<Option<ResetKind>>,
    /// Accent picker TextInput の draft 値 (= Apply 押下までは config 不反映)。
    /// `TextInput::on_change` callback で都度更新、 Apply button on_click closure が
    /// 本値を読み出して `apply_accent_picker(state, &draft)` を呼ぶ経路。 wave 3b
    /// で `Config::default().appearance.accent_hex` 由来の initial value を入れ、
    /// persistence load 直後は config の現値 (= 既存ユーザー設定 or default) と一致。
    pub draft_accent_hex: State<String>,
    /// 500ms quiet-period 共有 debouncer。 borrow_mut した上で request() を呼ぶ
    /// (= 連続 toggle で disk thrash を防ぐ)。
    pub debouncer: Rc<RefCell<DebouncedSaver>>,
    /// `~/.config/hayate-kit-settings/config.json` の絶対 path。 poll loop で
    /// `persistence::save(&*config.get(), &config_path)` 呼出に使用。
    pub config_path: PathBuf,
}

impl AppStateHandles {
    /// 初期 Config + config_path から AppStateHandles を構築。
    ///
    /// `runtime` は `App::with_reactive(scheduler.dirty_flag())` と同じ
    /// dirty flag を State 群が共有するための source。 各 State は
    /// `runtime.create_state(...)` 経由で作られ、 値変更で自動 repaint trigger。
    pub fn new(runtime: &ReactiveRuntime, initial_config: Config, config_path: PathBuf) -> Self {
        let initial_accent = initial_config.appearance.accent_hex.clone();
        Self {
            config: runtime.create_state(initial_config),
            selected_section: runtime.create_state(SectionId::default()),
            search_query: runtime.create_state(String::new()),
            accent_picker_visible: runtime.create_state(false),
            reset_confirm_visible: runtime.create_state(false),
            pending_reset: runtime.create_state(None),
            draft_accent_hex: runtime.create_state(initial_accent),
            debouncer: Rc::new(RefCell::new(DebouncedSaver::new())),
            config_path,
        }
    }
}

/// Test-only constructor for sections / modals / search smoke tests。
/// `cargo test` gated、 production binary には含まれない。
#[cfg(test)]
pub(crate) fn for_testing() -> AppStateHandles {
    let runtime = ReactiveRuntime::new();
    AppStateHandles::new(
        &runtime,
        Config::default(),
        PathBuf::from("/tmp/hayate-kit-settings-test/config.json"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_construct_smoke() {
        let runtime = ReactiveRuntime::new();
        let config = Config::default();
        let path = PathBuf::from("/tmp/hayate-kit-settings-test/config.json");
        let h = AppStateHandles::new(&runtime, config, path.clone());

        // initial values check
        assert_eq!(*h.selected_section.get(), SectionId::default());
        assert!(h.search_query.get().is_empty());
        assert!(!*h.accent_picker_visible.get());
        assert!(!*h.reset_confirm_visible.get());
        assert!(h.pending_reset.get().is_none());
        assert!(!h.debouncer.borrow().has_pending());
        assert_eq!(h.config_path, path);
    }

    #[test]
    fn pending_reset_initial_none_and_shared_across_clones() {
        let runtime = ReactiveRuntime::new();
        let h1 = AppStateHandles::new(
            &runtime,
            Config::default(),
            PathBuf::from("/tmp/hayate-kit-settings-test/pending-reset.json"),
        );
        assert!(h1.pending_reset.get().is_none(), "initial = None");
        let h2 = h1.clone();
        h2.pending_reset.set(Some(ResetKind::Section(SectionId::Advanced)));
        assert_eq!(
            *h1.pending_reset.get(),
            Some(ResetKind::Section(SectionId::Advanced)),
            "clone 経由 set が共有 Rc で観測可"
        );
    }

    #[test]
    fn draft_accent_hex_initial_matches_config_accent() {
        // 起動時 persistence load 由来の Config を State 化すると、 draft_accent_hex
        // initial は config の現値 (= default or persisted user value) と一致する。
        let runtime = ReactiveRuntime::new();
        let config = Config::default();
        let expected = config.appearance.accent_hex.clone();
        let h = AppStateHandles::new(
            &runtime,
            config,
            PathBuf::from("/tmp/hayate-kit-settings-test/draft.json"),
        );
        assert_eq!(*h.draft_accent_hex.get(), expected);
    }

    #[test]
    fn draft_accent_hex_initial_reflects_persisted_value() {
        // persistence load 結果が non-default の場合、 draft はその値で start。
        let runtime = ReactiveRuntime::new();
        let mut config = Config::default();
        config.appearance.accent_hex = String::from("#ABCDEF");
        let h = AppStateHandles::new(
            &runtime,
            config,
            PathBuf::from("/tmp/hayate-kit-settings-test/persisted.json"),
        );
        assert_eq!(*h.draft_accent_hex.get(), "#ABCDEF");
    }

    #[test]
    fn draft_accent_hex_shared_across_clones() {
        // TextInput::on_change closure が clone を capture して set すると、
        // Apply button on_click closure 側 clone でも観測可 (= 同 Rc 内部値)。
        let runtime = ReactiveRuntime::new();
        let h1 = AppStateHandles::new(
            &runtime,
            Config::default(),
            PathBuf::from("/tmp/hayate-kit-settings-test/shared.json"),
        );
        let h2 = h1.clone();
        h2.draft_accent_hex.set(String::from("#FF0000"));
        assert_eq!(*h1.draft_accent_hex.get(), "#FF0000");
    }

    #[test]
    fn clone_shares_underlying_state() {
        let runtime = ReactiveRuntime::new();
        let h1 = AppStateHandles::new(
            &runtime,
            Config::default(),
            PathBuf::from("/tmp/x/config.json"),
        );
        let h2 = h1.clone();
        // h2 経由 update が h1 でも観測可能 (= 同 Rc 内部値)
        h2.selected_section.set(SectionId::Advanced);
        assert_eq!(*h1.selected_section.get(), SectionId::Advanced);
    }

    #[test]
    fn config_update_propagates_through_state() {
        let runtime = ReactiveRuntime::new();
        let h = AppStateHandles::new(
            &runtime,
            Config::default(),
            PathBuf::from("/tmp/y/config.json"),
        );
        // update sub-field via state-level mutation
        h.config.update(|c| {
            c.general.startup_reset_window = true;
        });
        assert!(h.config.get().general.startup_reset_window);
    }

    #[test]
    fn debouncer_request_shared_across_clones() {
        let runtime = ReactiveRuntime::new();
        let h1 = AppStateHandles::new(
            &runtime,
            Config::default(),
            PathBuf::from("/tmp/z/config.json"),
        );
        let h2 = h1.clone();
        h2.debouncer.borrow().request();
        assert!(h1.debouncer.borrow().has_pending());
    }
}
