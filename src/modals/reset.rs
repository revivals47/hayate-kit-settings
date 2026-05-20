//! Reset confirm modal (= RFC v0.5 §5.2.8、 全 section reset 等の destructive
//! action gate)。
//!
//! wave 3c-pre module split で `modals.rs` から分離 (pure refactor、 behavior
//! 変更ゼロ)。 後続 track1 (= reset) の主担当領域。

use hayate_kit::widget::button::ButtonWidget;
use hayate_kit::widget::label::LabelWidget;
use hayate_kit::widget::layout::{HStack, VStack};
use hayate_kit::Widget;

use crate::lang::Strings;
use crate::persistence;
use crate::state::AppStateHandles;

// ── action helpers (= wave 3b core、 全 button on_click + 全 test で共有) ──

/// Reset Config to defaults + persist immediately + dismiss reset confirm modal。
///
/// `persistence::reset` が disk side `config.json` を `.bak` rename + new default
/// 書き出し、 同時に in-memory state.config も `Config::default()` で揃える。
/// debouncer は使わず即時 disk 書き込み (= destructive 確認後の hard reset、
/// debounce 経由で待つ semantics が不適切)。
pub(crate) fn apply_reset_confirm(state: &AppStateHandles) -> std::io::Result<()> {
    persistence::reset(&state.config_path)?;
    state.config.set(persistence::Config::default());
    state.reset_confirm_visible.set(false);
    // draft_accent_hex も default に同期 (= PR #8 codex 査読 real bug fix)。
    // reset 後の accent_picker reopen 時に旧 draft (= user 編集途中値) が見え
    // ないように、 config と一致する default value で reseed する。
    state.draft_accent_hex.set(persistence::Config::default().appearance.accent_hex);
    Ok(())
}

/// Dismiss reset confirm modal without performing reset (= state 不変)。
pub(crate) fn cancel_reset_confirm(state: &AppStateHandles) {
    state.reset_confirm_visible.set(false);
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
    let title = LabelWidget::new("Reset all settings?", 16.0);
    let message = LabelWidget::new(
        "全 settings を default に reset しますか? (この操作は取消不可)",
        14.0,
    );

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
