//! Reset confirm modal (= RFC v0.5 §5.2.8、 全 section reset 等の destructive
//! action gate)。
//!
//! wave 3c-pre module split で `modals.rs` から分離 (pure refactor、 behavior
//! 変更ゼロ)。 後続 track1 (= reset) の主担当領域。

use std::cell::RefCell;
use std::rc::Rc;

use hayate_kit::widget::button::ButtonWidget;
use hayate_kit::widget::label::LabelWidget;
use hayate_kit::widget::layout::{HStack, VStack};
use hayate_kit::{Constraints, ItemRect, Renderer, Size, State, TextEngine, Widget, WidgetId};

use crate::lang::Strings;
use crate::persistence;
use crate::sections::SectionId;
use crate::state::{AppStateHandles, ResetKind};

// ── action helpers (= wave 3b core + wave 3c 粒度別、 全 button on_click + 全 test で共有) ──

/// Apply a reset of the requested granularity + persist + dismiss modal。
///
/// 粒度別 destructive reset の中核。 [`ResetKind`] で何を default に戻すか分岐:
/// - [`ResetKind::All`]: `persistence::reset` で disk `config.json` を `.bak`
///   rename + new default 書き出し + in-memory `Config::default()` 同期 (= 旧
///   `apply_reset_confirm` の hard reset 動作を保全)
/// - [`ResetKind::Section`]: 指定 section の sub-config struct のみ default に
///   差し替え + `persistence::save` で disk 反映 (.bak は save の atomic rename で
///   自動作成)
/// - [`ResetKind::Field`]: accent_hex 単一 field を default に + save
/// - [`ResetKind::CacheClear`]: 実 cache layer 不在のため no-op (= 将来 cache
///   実装時に flush wire)
///
/// 末尾で `reset_confirm_visible` / `pending_reset` を clear (= modal を閉じて
/// pending 状態を解除)。 accent_hex に触れる variant (All / Field /
/// Section(Appearance)) では `draft_accent_hex` も config 現値に reseed
/// (= reopen 時に旧 draft が見えない、 PR #8 codex 査読 real bug fix の踏襲)。
///
/// debouncer は使わず即時 disk 書き込み (= destructive 確認後の hard reset、
/// debounce 経由で待つ semantics が不適切)。
pub(crate) fn apply_reset(state: &AppStateHandles, kind: ResetKind) -> std::io::Result<()> {
    match kind {
        ResetKind::All => {
            persistence::reset(&state.config_path)?;
            state.config.set(persistence::Config::default());
            state
                .draft_accent_hex
                .set(persistence::Config::default().appearance.accent_hex);
        }
        ResetKind::Section(sid) => {
            state.config.update(|c| reset_section_in_config(c, sid));
            persistence::save(&state.config.get(), &state.config_path)?;
            // Appearance section reset 時のみ accent draft を reseed
            // (= 他 section reset は accent_hex に触れない)。
            if sid == SectionId::Appearance {
                let current = state.config.get().appearance.accent_hex.clone();
                state.draft_accent_hex.set(current);
            }
        }
        ResetKind::Field => {
            let default_accent = persistence::Config::default().appearance.accent_hex;
            state
                .config
                .update(|c| c.appearance.accent_hex = default_accent.clone());
            persistence::save(&state.config.get(), &state.config_path)?;
            state.draft_accent_hex.set(default_accent);
        }
        ResetKind::CacheClear => {
            // 実 cache layer 不在のため no-op (= 将来 cache 実装時に flush wire)。
            // config / draft いずれも触れず、 modal dismiss のみ実施。
        }
    }
    state.reset_confirm_visible.set(false);
    state.pending_reset.set(None);
    Ok(())
}

/// Reset confirm modal の Reset 押下 / Enter accept から呼ばれる entry point。
///
/// `pending_reset` を読んで [`apply_reset`] に dispatch する thin wrapper。
/// signature を `(state) -> io::Result<()>` のまま維持することで、 main.rs の
/// on_enter closure / reset button on_click は無変更で粒度別 reset に対応する
/// (= scope discipline、 main.rs touch 不要)。 `pending_reset` が `None` の場合
/// (= 直接 modal を開いた等の安全側経路) は `ResetKind::All` に fallback し、
/// 従来の hard reset 動作を保全。
pub(crate) fn apply_reset_confirm(state: &AppStateHandles) -> std::io::Result<()> {
    let kind = (*state.pending_reset.get()).unwrap_or(ResetKind::All);
    apply_reset(state, kind)
}

/// Dismiss reset confirm modal without performing reset (= config 不変)。
/// `pending_reset` も `None` に戻す (= 次回 modal が stale kind を読まない)。
pub(crate) fn cancel_reset_confirm(state: &AppStateHandles) {
    state.reset_confirm_visible.set(false);
    state.pending_reset.set(None);
}

/// 指定 section の sub-config struct を `Default` で差し替える。
/// `Config` の section 別 nested struct がそれぞれ `Default` を実装している
/// 前提 (= persistence layer の schema と整合)。
fn reset_section_in_config(config: &mut persistence::Config, sid: SectionId) {
    use persistence::{
        AccessibilityConfig, AdvancedConfig, AppearanceConfig, GeneralConfig, ImeConfig,
    };
    match sid {
        SectionId::General => config.general = GeneralConfig::default(),
        SectionId::Appearance => config.appearance = AppearanceConfig::default(),
        SectionId::Accessibility => config.accessibility = AccessibilityConfig::default(),
        SectionId::Ime => config.ime = ImeConfig::default(),
        SectionId::Advanced => config.advanced = AdvancedConfig::default(),
    }
}

/// `pending_reset` の値から confirm modal の表示 message を構成する (= 何が
/// reset されるかを user に明示)。 `None` は安全側 fallback message。
pub(crate) fn reset_confirm_message(kind: Option<ResetKind>) -> String {
    match kind {
        Some(ResetKind::All) => {
            "Reset ALL settings to default? (この操作は取消不可)".to_string()
        }
        Some(ResetKind::Section(sid)) => format!(
            "Reset the {} section to default? (この操作は取消不可)",
            section_label(sid)
        ),
        Some(ResetKind::Field) => format!(
            "Reset accent color to default ({})? (この操作は取消不可)",
            persistence::Config::default().appearance.accent_hex
        ),
        Some(ResetKind::CacheClear) => {
            "Clear cache? (現状 cache layer 未実装のため no-op)".to_string()
        }
        None => "Reset settings? (この操作は取消不可)".to_string(),
    }
}

/// SectionId → 人間可読 section 名 (= confirm message 用)。
fn section_label(sid: SectionId) -> &'static str {
    match sid {
        SectionId::General => "General",
        SectionId::Appearance => "Appearance",
        SectionId::Accessibility => "Accessibility",
        SectionId::Ime => "IME",
        SectionId::Advanced => "Advanced",
    }
}

// ── ReactiveResetMessage (= pending_reset 観測 → 動的 confirm message) ──
//
// reset confirm modal は build_reset_confirm で 1 回構築され、 reset_confirm_visible
// で show/hide される。 message は pending_reset (= どの button が押されたか) に
// 応じて動的に変える必要があるため、 ReactiveOverlayContainer の sync pattern を
// 踏襲し、 layout/paint 冒頭で pending_reset を観測 → 差分時のみ set_text する
// thin reactive label wrapper。

/// `pending_reset` を観測して confirm message を動的更新する LabelWidget wrapper。
struct ReactiveResetMessage {
    pending: State<Option<ResetKind>>,
    inner: LabelWidget,
    last_observed: Option<ResetKind>,
}

impl ReactiveResetMessage {
    fn new(pending: State<Option<ResetKind>>) -> Self {
        let initial = *pending.get();
        let inner = LabelWidget::new(reset_confirm_message(initial), 14.0);
        Self {
            pending,
            inner,
            last_observed: initial,
        }
    }

    /// layout / paint 冒頭で呼出、 idempotent (= 差分時のみ set_text)。
    fn sync(&mut self) {
        let now = *self.pending.get();
        if now != self.last_observed {
            self.inner.set_text(&reset_confirm_message(now));
            self.last_observed = now;
        }
    }
}

impl Widget for ReactiveResetMessage {
    fn id(&self) -> WidgetId {
        self.inner.id()
    }

    fn layout(&mut self, constraints: &Constraints) -> Size {
        self.sync();
        self.inner.layout(constraints)
    }

    fn paint(&mut self, renderer: &mut Renderer, rect: ItemRect) {
        self.sync();
        self.inner.paint(renderer, rect);
    }

    fn dirty(&self) -> bool {
        self.inner.dirty()
    }

    fn clear_dirty(&mut self) {
        self.inner.clear_dirty();
    }

    fn inject_engine(&mut self, engine: Rc<RefCell<TextEngine>>) {
        self.inner.inject_engine(engine);
    }
}

// ── widget composition (= wave 3b 視覚 wire 完成) ──

/// Reset confirm modal (= wave 3b 視覚 wire 完成版、 PRESIDENT Option β 採択)。
///
/// 構造: `VStack { title Label + message Label + HStack { Cancel + Reset } }`。
/// accent_picker と同 pattern (= VStack 統一、 AlertDialog 不使用) で 2 modal
/// の visual + 構造 cohesion を確保 (= Option A revision v0.1 → v0.2)。
///
/// ## 配線
/// - Cancel button on_click: [`cancel_reset_confirm`] で visible flag clear
/// - Reset button on_click: [`apply_reset_confirm`] で `persistence::reset` +
///   state.config = default + visible flag clear。 io::Error は `eprintln!`
///   で WARN 出力 + 黙って続行 (= user-visible error toast は別 widget 追加
///   必要のため次 wave defer、 disk 書込失敗時も visible flag clear で modal
///   は閉じる方針)。
/// - Escape dismiss: ReactiveOverlayContainer.event() の pre-intercept 経由
///   (= dismiss_topmost_visible → state.reset_confirm_visible.set(false))。
///   AlertDialog 内蔵 dismiss は本 modal では不使用 (= VStack 化のため)。
/// - Enter accept: ReactiveOverlayContainer.event() の Return/KP_Enter
///   pre-intercept 経由 (= main.rs 側で on_enter closure = apply_reset_confirm
///   登録、 Ok 系 button click と同等 result)。
#[allow(dead_code)] // wave 3c 以降 main.rs から呼ばれる
pub fn build_reset_confirm(_strings: &'static Strings, state: &AppStateHandles) -> Box<dyn Widget> {
    // title は generic (= 粒度は message 側で明示)、 message は pending_reset を
    // 観測して動的更新する ReactiveResetMessage で構成。
    let title = LabelWidget::new("Confirm reset", 16.0);
    let message = ReactiveResetMessage::new(state.pending_reset.clone());

    let cancel_state = state.clone();
    let cancel_btn = ButtonWidget::new("Cancel").on_click(move || {
        cancel_reset_confirm(&cancel_state);
    });
    let reset_state = state.clone();
    let reset_btn = ButtonWidget::new("Reset").on_click(move || {
        if let Err(e) = apply_reset_confirm(&reset_state) {
            eprintln!("WARN: hayate-kit-settings: reset failed: {e}");
            // disk 失敗時も visible flag は clear して modal を閉じる
            // (= 次 wave で error toast 表示と引き換え予定)
            reset_state.reset_confirm_visible.set(false);
        }
    });

    let mut buttons = HStack::new(12.0);
    buttons = buttons.add(Box::new(cancel_btn));
    buttons = buttons.add(Box::new(reset_btn));

    let mut stack = VStack::new(16.0);
    stack = stack.add(Box::new(title));
    stack = stack.add(Box::new(message));
    stack = stack.add(Box::new(buttons));
    Box::new(stack)
}

#[cfg(test)]
mod granular_tests {
    use super::*;
    use crate::persistence::{Config, LogLevel};
    use crate::state::{for_testing, AppStateHandles, ResetKind};
    use hayate_kit::ReactiveRuntime;
    use std::path::PathBuf;

    fn isolated_state(label: &str) -> (AppStateHandles, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "hayate-kit-settings-granular-test-{}-{}",
            std::process::id(),
            label
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("config.json");
        let runtime = ReactiveRuntime::new();
        let state = AppStateHandles::new(&runtime, Config::default(), path.clone());
        (state, path)
    }

    fn cleanup(path: &std::path::Path) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
    }

    // ── confirm message 文言 (= 粒度別、 何が reset されるか明示) ──

    #[test]
    fn message_distinguishes_each_reset_kind() {
        let all = reset_confirm_message(Some(ResetKind::All));
        assert!(all.contains("ALL"), "all = 全体明示");

        let sec = reset_confirm_message(Some(ResetKind::Section(SectionId::Advanced)));
        assert!(sec.contains("Advanced"), "section 名明示");
        assert!(sec.contains("section"));

        let field = reset_confirm_message(Some(ResetKind::Field));
        assert!(field.contains("accent"), "field = accent 明示");

        let cache = reset_confirm_message(Some(ResetKind::CacheClear));
        assert!(cache.contains("cache") || cache.contains("Cache"), "cache 明示");
        assert!(cache.contains("no-op"), "未実装 no-op を明示");

        // 4 message は互いに distinct
        assert_ne!(all, sec);
        assert_ne!(sec, field);
        assert_ne!(field, cache);
        assert_ne!(all, field);
    }

    #[test]
    fn message_section_label_covers_every_section() {
        for sid in [
            SectionId::General,
            SectionId::Appearance,
            SectionId::Accessibility,
            SectionId::Ime,
            SectionId::Advanced,
        ] {
            let msg = reset_confirm_message(Some(ResetKind::Section(sid)));
            assert!(msg.contains(section_label(sid)), "section 名が message に含まれる");
        }
    }

    #[test]
    fn message_none_falls_back_to_generic() {
        let msg = reset_confirm_message(None);
        assert!(msg.contains("Reset"), "fallback も Reset 明示");
    }

    // ── apply_reset 粒度別 action ──

    #[test]
    fn apply_reset_all_restores_full_default_and_clears_pending() {
        let (state, path) = isolated_state("all");
        state.config.update(|c| {
            c.appearance.accent_hex = String::from("#FFCC00");
            c.advanced.log_level = LogLevel::Trace;
            c.general.startup_reset_window = true;
        });
        persistence::save(&state.config.get(), &path).expect("seed");
        state.pending_reset.set(Some(ResetKind::All));
        state.reset_confirm_visible.set(true);

        apply_reset(&state, ResetKind::All).expect("reset all");

        assert_eq!(*state.config.get(), Config::default(), "全 default 復元");
        assert!(!*state.reset_confirm_visible.get(), "modal dismiss");
        assert!(state.pending_reset.get().is_none(), "pending clear");
        cleanup(&path);
    }

    #[test]
    fn apply_reset_section_resets_only_target_section() {
        let (state, path) = isolated_state("section");
        // 2 section を non-default に seed
        state.config.update(|c| {
            c.advanced.log_level = LogLevel::Trace; // Advanced (= reset 対象)
            c.general.startup_reset_window = true; // General (= 維持されるべき)
        });
        persistence::save(&state.config.get(), &path).expect("seed");
        state.pending_reset.set(Some(ResetKind::Section(SectionId::Advanced)));
        state.reset_confirm_visible.set(true);

        apply_reset(&state, ResetKind::Section(SectionId::Advanced)).expect("reset section");

        // Advanced は default、 General は維持
        assert_eq!(
            state.config.get().advanced.log_level,
            LogLevel::Info,
            "Advanced は default (Info) に reset"
        );
        assert!(
            state.config.get().general.startup_reset_window,
            "General section は touch されない (= 粒度別 reset の core property)"
        );
        assert!(!*state.reset_confirm_visible.get());
        assert!(state.pending_reset.get().is_none());

        // disk にも反映 (= save 経由)
        let on_disk = persistence::load(&path).expect("load");
        assert_eq!(on_disk.advanced.log_level, LogLevel::Info);
        assert!(on_disk.general.startup_reset_window, "General disk 維持");
        cleanup(&path);
    }

    #[test]
    fn apply_reset_section_appearance_reseeds_draft() {
        let (state, path) = isolated_state("section-appearance");
        state.config.update(|c| c.appearance.accent_hex = String::from("#FFCC00"));
        state.draft_accent_hex.set(String::from("#ABCDEF"));
        persistence::save(&state.config.get(), &path).expect("seed");

        apply_reset(&state, ResetKind::Section(SectionId::Appearance)).expect("reset");

        let default_accent = Config::default().appearance.accent_hex;
        assert_eq!(state.config.get().appearance.accent_hex, default_accent);
        assert_eq!(
            *state.draft_accent_hex.get(),
            default_accent,
            "Appearance reset は draft も reseed"
        );
        cleanup(&path);
    }

    #[test]
    fn apply_reset_field_resets_only_accent_hex() {
        let (state, path) = isolated_state("field");
        state.config.update(|c| {
            c.appearance.accent_hex = String::from("#FFCC00");
            c.appearance.font_size_scale = 99.0; // 同 section 別 field (= 維持)
            c.advanced.log_level = LogLevel::Trace; // 別 section (= 維持)
        });
        state.draft_accent_hex.set(String::from("#ABCDEF"));
        persistence::save(&state.config.get(), &path).expect("seed");

        apply_reset(&state, ResetKind::Field).expect("reset field");

        let default_accent = Config::default().appearance.accent_hex;
        assert_eq!(
            state.config.get().appearance.accent_hex,
            default_accent,
            "accent_hex のみ default"
        );
        assert_eq!(
            state.config.get().appearance.font_size_scale,
            99.0,
            "同 section 別 field (font_size_scale) は維持 (= field 粒度の core property)"
        );
        assert_eq!(
            state.config.get().advanced.log_level,
            LogLevel::Trace,
            "別 section は維持"
        );
        assert_eq!(*state.draft_accent_hex.get(), default_accent, "draft reseed");
        cleanup(&path);
    }

    #[test]
    fn apply_reset_cache_clear_is_noop_on_config() {
        let (state, path) = isolated_state("cache");
        state.config.update(|c| {
            c.advanced.log_level = LogLevel::Trace;
            c.appearance.accent_hex = String::from("#FFCC00");
        });
        let snapshot = state.config.get().clone();
        state.pending_reset.set(Some(ResetKind::CacheClear));
        state.reset_confirm_visible.set(true);

        apply_reset(&state, ResetKind::CacheClear).expect("cache clear");

        assert_eq!(
            *state.config.get(),
            snapshot,
            "CacheClear は config を一切変更しない (= 現状 no-op)"
        );
        assert!(!*state.reset_confirm_visible.get(), "modal は dismiss");
        assert!(state.pending_reset.get().is_none(), "pending clear");
        cleanup(&path);
    }

    // ── apply_reset_confirm が pending_reset を読んで dispatch ──

    #[test]
    fn apply_reset_confirm_dispatches_via_pending_reset_section() {
        let (state, path) = isolated_state("dispatch-section");
        state.config.update(|c| {
            c.advanced.log_level = LogLevel::Trace;
            c.general.startup_reset_window = true;
        });
        persistence::save(&state.config.get(), &path).expect("seed");
        // pending_reset = Section(Advanced) → apply_reset_confirm は Advanced のみ reset
        state.pending_reset.set(Some(ResetKind::Section(SectionId::Advanced)));
        state.reset_confirm_visible.set(true);

        apply_reset_confirm(&state).expect("dispatch");

        assert_eq!(state.config.get().advanced.log_level, LogLevel::Info);
        assert!(
            state.config.get().general.startup_reset_window,
            "Section dispatch は他 section を touch しない"
        );
        cleanup(&path);
    }

    #[test]
    fn apply_reset_confirm_none_falls_back_to_all() {
        let (state, path) = isolated_state("dispatch-none");
        state.config.update(|c| c.advanced.log_level = LogLevel::Trace);
        persistence::save(&state.config.get(), &path).expect("seed");
        // pending_reset = None → apply_reset_confirm は All に fallback (= 従来動作保全)
        assert!(state.pending_reset.get().is_none());
        state.reset_confirm_visible.set(true);

        apply_reset_confirm(&state).expect("fallback");

        assert_eq!(*state.config.get(), Config::default(), "None → All fallback");
        cleanup(&path);
    }

    #[test]
    fn cancel_clears_pending_reset() {
        let state = for_testing();
        state.pending_reset.set(Some(ResetKind::Field));
        state.reset_confirm_visible.set(true);
        cancel_reset_confirm(&state);
        assert!(state.pending_reset.get().is_none(), "cancel で pending clear");
        assert!(!*state.reset_confirm_visible.get(), "modal dismiss");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Lang;
    use crate::persistence::{Config, LogLevel};
    use crate::state::{for_testing, AppStateHandles};
    use hayate_kit::ReactiveRuntime;
    use std::path::PathBuf;

    /// Test isolation helper: per-test unique config_path で disk I/O 衝突を回避。
    fn isolated_state(label: &str) -> (AppStateHandles, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "hayate-kit-settings-modal-test-{}-{}",
            std::process::id(),
            label
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("config.json");
        let runtime = ReactiveRuntime::new();
        let state = AppStateHandles::new(&runtime, Config::default(), path.clone());
        (state, path)
    }

    fn cleanup_state(path: &std::path::Path) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
    }

    #[test]
    fn build_reset_confirm_smoke() {
        let strings = Lang::En.strings();
        let state = for_testing();
        let _root = build_reset_confirm(strings, &state);
    }

    #[test]
    fn reset_confirm_visible_toggle_via_state() {
        let state = for_testing();
        assert!(!*state.reset_confirm_visible.get(), "initial = false");
        state.reset_confirm_visible.set(true);
        assert!(*state.reset_confirm_visible.get(), "set(true) reflected");
        state.reset_confirm_visible.set(false);
        assert!(!*state.reset_confirm_visible.get(), "set(false) reflected");
    }

    // Reset 押下 → config default 復元 + disk reset + .bak 作成 + visible clear
    #[test]
    fn apply_reset_confirm_restores_defaults_and_dismisses() {
        let (state, path) = isolated_state("apply-reset");

        // seed: 非 default な config を disk + state 両方へ
        state.config.update(|c| {
            c.appearance.accent_hex = String::from("#FFCC00");
            c.advanced.log_level = LogLevel::Trace;
        });
        persistence::save(&state.config.get(), &path).expect("seed save");
        state.reset_confirm_visible.set(true);

        // act: Reset 押下相当
        apply_reset_confirm(&state).expect("reset succeeds");

        // verify: in-memory state = default
        assert_eq!(*state.config.get(), Config::default(), "in-memory default");
        assert!(
            !*state.reset_confirm_visible.get(),
            "Reset must dismiss modal"
        );

        // verify: disk file = default、 .bak = pre-reset seed
        let bak = {
            let mut s = path.as_os_str().to_owned();
            s.push(".bak");
            PathBuf::from(s)
        };
        assert!(bak.exists(), ".bak must hold pre-reset seed");
        let on_disk = persistence::load(&path).expect("load post-reset");
        assert_eq!(on_disk, Config::default(), "disk default");

        cleanup_state(&path);
    }

    // Cancel reset → state.config 不変 + visible clear (= destructive 回避)
    #[test]
    fn cancel_reset_confirm_leaves_config_untouched() {
        let state = for_testing();
        state.reset_confirm_visible.set(true);
        state.config.update(|c| {
            c.advanced.debug_overlay = true;
            c.appearance.accent_hex = String::from("#ABC123");
        });
        let snapshot = state.config.get().clone();
        cancel_reset_confirm(&state);
        assert_eq!(
            *state.config.get(),
            snapshot,
            "Cancel reset must not mutate config"
        );
        assert!(
            !*state.reset_confirm_visible.get(),
            "Cancel reset must dismiss modal"
        );
    }

    /// Reset confirm 適用後、 draft_accent_hex は Config::default の accent_hex
    /// に re-seed される。 reset 後に accent_picker reopen 時、 旧 user 編集
    /// draft が見えないことを保証。
    #[test]
    fn reset_clears_draft_to_default() {
        let (state, path) = isolated_state("reset-draft");

        // seed: config + draft 両方を non-default に
        state.config.update(|c| {
            c.appearance.accent_hex = String::from("#FFCC00");
        });
        state.draft_accent_hex.set(String::from("#ABCDEF"));
        state.reset_confirm_visible.set(true);

        // act: Reset 押下相当
        apply_reset_confirm(&state).expect("reset succeeds");

        // verify: draft が Config::default の accent_hex に re-seed
        let default_accent = Config::default().appearance.accent_hex;
        assert_eq!(
            *state.draft_accent_hex.get(),
            default_accent,
            "Reset で draft が default に re-seed"
        );
        // verify: config も default 復元 (= 既存 path、 regression check)
        assert_eq!(state.config.get().appearance.accent_hex, default_accent);
        assert!(!*state.reset_confirm_visible.get());

        cleanup_state(&path);
    }
}
