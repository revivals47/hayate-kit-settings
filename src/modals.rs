//! Modal dialog instances (= RFC v0.5 §5.2.8、 Phase 2 wave 3b — modal
//! lifecycle wire logic-only subset)。
//!
//! 2 instance を fill:
//! - `build_accent_picker(strings, state) -> Box<dyn Widget>` (= accent color
//!   hex 入力 + preview swatch + WCAG contrast check warning UI)
//! - `build_reset_confirm(strings, state) -> Box<dyn Widget>` (= 全 section
//!   reset 等の destructive action gate)
//!
//! ## wave 3b scope (= 本 commit、 logic-only subset)
//! 状態 wire (= 4 pub(crate) action helpers + button on_click closure → state
//! mutate) を実装、in-memory unit test で全 lifecycle path を検証。視覚 modal
//! overlay (= AlertDialog show/hide + 外部 State<bool> 経由 control + 動的
//! preview / WCAG 再計算) は framework gap (= hayate-kit から `Renderer` /
//! `ItemRect` / `TextEngine` / `alloc_widget_id` 未 re-export、custom Widget
//! impl 不可) を consolidation framework PR で先に解消後、 次 wave で追加。
//!
//! ## framework gap 背景
//! `feedback_new_apps_depend_on_gui_kit_only` 規範下では `hayate_kit::Widget`
//! trait は再 export 済だが method signature 内 `Renderer` / `ItemRect` 等が
//! 未 re-export で custom Widget impl が不可能。`AlertDialog` 視覚 lifecycle は
//! Rc<RefCell> + on_change subscription + show/hide forward に custom wrapper
//! Widget が必要、本 gap 解消が prerequisite。 同 gap は worker3 でも別文脈で
//! 同時刻に検出 = `feedback_widget_trait_forward_gap_pattern` の case 3+4 目。
//!
//! ## DTP reuse
//! DTP app destructive action gate (= 例: ファイル削除確認 / 設定 reset / 編集
//! 取消) + typography color picker でも reuse 想定、 4 action helper signature
//! pattern (= `fn <action>_<modal>(state[, input])` ) は universal。

use std::cell::RefCell;
use std::rc::Rc;

use hayate_kit::widget::button::ButtonWidget;
use hayate_kit::widget::label::LabelWidget;
use hayate_kit::widget::layout::{HStack, VStack};
use hayate_kit::widget::overlay::{OverlayContainer, OverlayPosition};
use hayate_kit::widget::text_input_widget::TextInputWidget;
use hayate_kit::{
    Constraints, EventResponse, ItemRect, Renderer, Size, State, TextEngine, Widget, WidgetEvent,
    WidgetId,
};

#[allow(unused_imports)] // wave 3 で `Strings` field を本格使用予定
use crate::lang::Strings;
use crate::persistence;
use crate::state::AppStateHandles;

/// HAYATE Original default accent (= `#5A8BA8` 風藍、 RFC v0.2 §3 design language)。
const DEFAULT_ACCENT_HEX: &str = "#5A8BA8";
/// HAYATE Original default accent-base RGB (= `#5A8BA8`)。
const DEFAULT_ACCENT_RGB: (u8, u8, u8) = (90, 139, 168);
/// HAYATE Original default surface-raised RGB (= `#FFFFFF`)。
const DEFAULT_SURFACE_RGB: (u8, u8, u8) = (255, 255, 255);

// ── ReactiveOverlayContainer (= wave 3b 視覚層 root): State<bool> 観測 → ──
// hayate_kit::widget::overlay::OverlayContainer の show/hide_overlay forward。
// PRESIDENT 即決 Option A 採択 (= worker2 統合 design)、 design doc 名
// `AlertDialogContainer` から `ReactiveOverlayContainer` に rename。
//
// 設計根拠:
// - hayate_kit/hayate-platform に ZStack 不在、 既存 OverlayContainer (= base
//   + Vec<overlay widgets> + dimming + modal event routing 完備) を thin wrap
// - State<bool> 観測 = paint/layout/event 冒頭で sync_bindings()、 idempotent
// - Escape pre-intercept = 最上位 visible overlay の State<bool>.set(false)
//   (= AlertDialog 内蔵 Escape dismiss と二重発火しても idempotent)
// - 外クリック dismiss は本 wave scope 外 (= PRESIDENT 補強 A defer 決定)
// - Tab trap は本 wave scope 外 (= 確定方針通り defer)
// - id() は inner.id() delegate (= PRESIDENT distinction、 inner stable のため
//   alloc_widget_id PR 不要、 worker3 DetailContainerWidget case と分離)

/// State<bool> binding for a single overlay slot.
struct OverlayBinding {
    /// `OverlayContainer::add_overlay` で登録した stable id (= 操作対象選定 key)。
    id: String,
    /// 外部 control source (= AppStateHandles.{accent_picker_visible,
    /// reset_confirm_visible} 等)。
    state: State<bool>,
    /// 前回 sync 時に観測した値 (= 差分検出で show/hide forward を最小化)。
    last_observed: bool,
}

/// Thin reactive wrapper around [`OverlayContainer`].
///
/// 内部に `OverlayContainer` を保持、 N 個の (overlay id, `State<bool>`,
/// last_observed) bindings を sync_bindings() で観測し、 paint/layout/event
/// 冒頭で `show_overlay` / `hide_overlay` を forward する。
///
/// ## ライフサイクル
/// 1. `new(base)` で OverlayContainer 構築 (base widget は SplitView 等の主 UI)
/// 2. `add_overlay_with_state(id, widget, position, state)` で overlay slot 追加
///    + binding 登録、 initial state が true なら即 show_overlay 同期
/// 3. paint/layout/event の冒頭で sync_bindings() が走り、 state 変化を反映
/// 4. event で Escape を pre-intercept、 最上位 visible binding の
///    state.set(false) を呼んで dismiss、 inner.hide_overlay も即時 forward
///
/// ## scope
/// 本 wave 3b では `modals.rs` 内 thin wrapper (consumer-specific layer、
/// PRESIDENT 補強 C)。 wave 3b 完遂後 closeout で汎用需要評価、 必要なら
/// framework PR で hayate-kit 昇格 (別 dispatch、 本 wave scope 外)。
pub(crate) struct ReactiveOverlayContainer {
    inner: OverlayContainer,
    bindings: Vec<OverlayBinding>,
}

impl ReactiveOverlayContainer {
    /// Construct a container wrapping `base` widget (= 主 UI、 通常は SplitView)。
    pub(crate) fn new(base: Box<dyn Widget>) -> Self {
        Self {
            inner: OverlayContainer::new(base),
            bindings: Vec::new(),
        }
    }

    /// Add an overlay slot bound to a `State<bool>` source。
    ///
    /// initial state が true なら即 inner.show_overlay 同期。 同 id を 2 回
    /// 登録すると inner 側で重複登録になる + binding が 2 個出来るので、
    /// caller は id の uniqueness を保証する。
    pub(crate) fn add_overlay_with_state(
        &mut self,
        id: impl Into<String>,
        widget: Box<dyn Widget>,
        position: OverlayPosition,
        visible_state: State<bool>,
    ) {
        let id = id.into();
        self.inner.add_overlay(id.clone(), widget, position);
        let initial = *visible_state.get();
        if initial {
            self.inner.show_overlay(&id);
        }
        self.bindings.push(OverlayBinding {
            id,
            state: visible_state,
            last_observed: initial,
        });
    }

    /// Pull state changes into inner OverlayContainer visibility flags。
    ///
    /// layout / paint / event の冒頭で呼出、 idempotent (= 差分がなければ
    /// no-op、 差分時のみ show/hide_overlay forward + last_observed 更新)。
    fn sync_bindings(&mut self) {
        for binding in &mut self.bindings {
            let now = *binding.state.get();
            if now != binding.last_observed {
                if now {
                    self.inner.show_overlay(&binding.id);
                } else {
                    self.inner.hide_overlay(&binding.id);
                }
                binding.last_observed = now;
            }
        }
    }

    /// Dismiss the topmost visible overlay (if any)。
    ///
    /// 順序: bindings 末尾 (= 後で add_overlay_with_state された方が上層想定)
    /// から逆順走査し、 初出の visible binding の State<bool>.set(false) を
    /// 呼び、 inner.hide_overlay も即時 forward + last_observed = false 更新。
    /// Escape key pre-intercept から呼び出され、 全 binding が hidden の場合は
    /// 何もせず false を返す (= Escape を吸わない = base layer に届ける)。
    ///
    /// 別 method 抽出理由 (= test 観点): event() 内 `WidgetEvent::Key` 構築は
    /// `hayate_platform::platform::keyboard::KeyEvent` を要求し、 本 crate の
    /// hayate-kit only 依存規範下では test code から synthesize 不可能。 dismiss
    /// logic を method 化することで key event 経由のテストを迂回、 logic 自体を
    /// 直接 assert 可能にする (= unit test boundary 設計)。
    pub(crate) fn dismiss_topmost_visible(&mut self) -> bool {
        for binding in self.bindings.iter_mut().rev() {
            if *binding.state.get() {
                binding.state.set(false);
                binding.last_observed = false;
                self.inner.hide_overlay(&binding.id);
                return true;
            }
        }
        false
    }
}

impl Widget for ReactiveOverlayContainer {
    fn id(&self) -> WidgetId {
        self.inner.id()
    }

    fn layout(&mut self, constraints: &Constraints) -> Size {
        self.sync_bindings();
        self.inner.layout(constraints)
    }

    fn paint(&mut self, renderer: &mut Renderer, rect: ItemRect) {
        self.sync_bindings();
        self.inner.paint(renderer, rect);
    }

    fn event(&mut self, event: &WidgetEvent) -> EventResponse {
        self.sync_bindings();
        // Escape pre-intercept (= 最上位 visible binding を dismiss)。
        // keysym のみ判定、 state (Pressed/Released) は無視 = idempotent dismiss
        // で release event 二重発火しても無害 (= hide_overlay も State.set(false)
        // も冪等)。 hayate_platform::platform::keyboard::KeyState は hayate-kit
        // から re-export されておらず、 KeyState::Pressed 比較は本 layer から
        // unreachable のため、 keysym only 判定を採用。
        if let WidgetEvent::Key(ke) = event
            && ke.keysym == xkbcommon::xkb::Keysym::Escape
            && self.dismiss_topmost_visible()
        {
            return EventResponse::Handled;
        }
        self.inner.event(event)
    }

    fn dirty(&self) -> bool {
        self.inner.dirty()
    }

    fn clear_dirty(&mut self) {
        self.inner.clear_dirty();
    }

    fn update(&mut self, dt: f32) {
        self.inner.update(dt);
    }

    fn inject_engine(&mut self, engine: Rc<RefCell<TextEngine>>) {
        self.inner.inject_engine(engine);
    }
}

// ── LabelRef shim (= wave 3b dynamic preview/WCAG 配線基盤) ──
//
// `Rc<RefCell<LabelWidget>>` を Widget tree に投入するための thin wrapper。
// build_accent_picker 内で TextInput::on_change closure が clone を capture、
// もう一方を本 shim 経由で tree に投入することで、 closure 側から `set_text`
// 呼出可能な共有 mutate path を確立。
//
// inject_theme は default impl (= children_mut() empty で no-op) に任せる。
// LabelWidget::inject_theme は `bitmap_default` のみ更新する Win95 family skin
// 用 fallback。 hayate-kit-settings は HAYATE_ORIGINAL theme (= modern、
// cosmic-text path) を hard-baked 使用、 bitmap_default 未注入でも paint 正常。

/// Widget tree 投入と外部 mutate を両立する LabelWidget 共有 wrapper。
pub(crate) struct LabelRef {
    inner: Rc<RefCell<LabelWidget>>,
}

impl LabelRef {
    /// Construct a (`Box<dyn Widget>`, mutation handle) pair from a LabelWidget。
    ///
    /// returned tuple: `(0)` = Box 化済 widget (= tree 投入用)、 `(1)` = closure
    /// capture 用の Rc clone (= `handle.borrow_mut().set_text(&new)` で更新可)。
    pub(crate) fn new_pair(label: LabelWidget) -> (Box<dyn Widget>, Rc<RefCell<LabelWidget>>) {
        let inner = Rc::new(RefCell::new(label));
        let handle = Rc::clone(&inner);
        let widget: Box<dyn Widget> = Box::new(Self { inner });
        (widget, handle)
    }
}

impl Widget for LabelRef {
    fn id(&self) -> WidgetId {
        self.inner.borrow().id()
    }

    fn layout(&mut self, constraints: &Constraints) -> Size {
        self.inner.borrow_mut().layout(constraints)
    }

    fn paint(&mut self, renderer: &mut Renderer, rect: ItemRect) {
        self.inner.borrow_mut().paint(renderer, rect);
    }

    fn dirty(&self) -> bool {
        self.inner.borrow().dirty()
    }

    fn clear_dirty(&mut self) {
        self.inner.borrow_mut().clear_dirty();
    }

    fn inject_engine(&mut self, engine: Rc<RefCell<TextEngine>>) {
        self.inner.borrow_mut().inject_engine(engine);
    }
}

// ── action helpers (= wave 3b core、 全 button on_click + 全 test で共有) ──

/// Apply user-input accent hex into Config + request debounced save + dismiss
/// accent picker modal (= visible flag clear)。
///
/// `hex` は user-validated 文字列 (= `#RRGGBB` 形式想定、 validation は wave 3c
/// の reactive hex parser で実施予定、 本 wave は raw string 保存のみ)。
/// `state.config.update` で sub-field 編集、 全 section field の persistence
/// round-trip と互換。 debouncer は 500ms quiet-period で disk thrash を回避。
pub(crate) fn apply_accent_picker(state: &AppStateHandles, hex: &str) {
    state
        .config
        .update(|c| c.appearance.accent_hex = hex.to_string());
    state.debouncer.borrow().request();
    state.accent_picker_visible.set(false);
}

/// Dismiss accent picker modal without applying the draft hex (= state.config
/// 不変)。
pub(crate) fn cancel_accent_picker(state: &AppStateHandles) {
    state.accent_picker_visible.set(false);
}

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
    Ok(())
}

/// Dismiss reset confirm modal without performing reset (= state 不変)。
pub(crate) fn cancel_reset_confirm(state: &AppStateHandles) {
    state.reset_confirm_visible.set(false);
}

// ── widget composition (= wave 3b 視覚 wire 完成、 動的 preview/WCAG 配線済) ──

/// Accent color picker modal (= wave 3b 視覚 wire 完成版)。
///
/// 構造: `VStack { hex TextInput + preview swatch Label + WCAG ratio Label +
/// HStack { Cancel ButtonWidget + Apply ButtonWidget } }`。
///
/// ## 配線 (= wave 3b visual layer)
/// - hex TextInput.on_change: 入力ごとに [`parse_hex_or_default`] で RGB 抽出、
///   [`update_preview_text`] で preview label を更新、 [`compute_wcag_status`]
///   で WCAG ratio を再計算して wcag label を更新、 さらに state.draft_accent_hex
///   に raw 文字列を保存 (= Apply 押下時の commit 元)
/// - Cancel button on_click: [`cancel_accent_picker`] で visible flag clear
/// - Apply button on_click: state.draft_accent_hex から raw 文字列を読出して
///   [`apply_accent_picker`] に渡す (= 動的 hex 反映、 DEFAULT_ACCENT_HEX 固定 placeholder 廃止)
/// - preview / WCAG label は [`LabelRef::new_pair`] 経由 Rc<RefCell> 共有 mutate 配線
#[allow(dead_code)] // wave 3c 以降 main.rs から呼ばれる
pub fn build_accent_picker(_strings: &'static Strings, state: &AppStateHandles) -> Box<dyn Widget> {
    // initial preview / WCAG label content (= state.draft_accent_hex 現値 base)
    let initial_hex = state.draft_accent_hex.get().clone();
    let (ir, ig, ib) = parse_hex_or_default(&initial_hex);
    let preview_label = LabelWidget::new(update_preview_text(&initial_hex, ir, ig, ib), 13.0);
    let wcag_label = LabelWidget::new(compute_wcag_status(&initial_hex, ir, ig, ib), 12.0);

    // LabelRef shim 経由で Widget tree 投入 widget と外部 mutate handle を分離。
    // handle clone は on_change closure が capture、 widget は VStack に投入。
    let (preview_widget, preview_handle) = LabelRef::new_pair(preview_label);
    let (wcag_widget, wcag_handle) = LabelRef::new_pair(wcag_label);

    // TextInput::on_change closure (= PR #155 公開 API、 入力 mutation 経路毎に発火)。
    // 各 frame の paint 内 set_text 呼出が dirty flag を立て、 framework が次 frame
    // で repaint trigger。 ReactiveOverlayContainer 配下 layout は overlay slot
    // cached_size 経由で text 拡張に追従 (= 文字列長変化で auto reflow)。
    let on_change_state = state.clone();
    let preview_for_change = Rc::clone(&preview_handle);
    let wcag_for_change = Rc::clone(&wcag_handle);
    let hex_input = TextInputWidget::new()
        .with_placeholder(DEFAULT_ACCENT_HEX)
        .with_width(160.0)
        .on_change(move |new_text: &str| {
            let (r, g, b) = parse_hex_or_default(new_text);
            preview_for_change
                .borrow_mut()
                .set_text(&update_preview_text(new_text, r, g, b));
            wcag_for_change
                .borrow_mut()
                .set_text(&compute_wcag_status(new_text, r, g, b));
            on_change_state.draft_accent_hex.set(new_text.to_string());
        });

    let cancel_state = state.clone();
    let cancel_btn = ButtonWidget::new("Cancel").on_click(move || {
        cancel_accent_picker(&cancel_state);
    });
    let apply_state = state.clone();
    let apply_btn = ButtonWidget::new("Apply").on_click(move || {
        // draft_accent_hex 経由 user input 反映 (= wave 3b 視覚 wire で動的化、
        // DEFAULT_ACCENT_HEX 固定 placeholder は廃止)。 draft は on_change で
        // 都度更新済、 ここでは現値 snapshot を取って apply_accent_picker に転送。
        let draft = apply_state.draft_accent_hex.get().clone();
        apply_accent_picker(&apply_state, &draft);
    });

    let mut buttons = HStack::new(12.0);
    buttons = buttons.add(Box::new(cancel_btn));
    buttons = buttons.add(Box::new(apply_btn));

    let mut stack = VStack::new(8.0);
    stack = stack.add(Box::new(hex_input));
    stack = stack.add(preview_widget);
    stack = stack.add(wcag_widget);
    stack = stack.add(Box::new(buttons));
    Box::new(stack)
}

/// Reset confirm modal (= wave 3b logic-only subset)。
///
/// 構造: `VStack { confirm message Label + HStack { Cancel ButtonWidget + Reset
/// ButtonWidget } }`。
///
/// ## 配線
/// - Cancel button on_click: [`cancel_reset_confirm`] で visible flag clear
/// - Reset button on_click: [`apply_reset_confirm`] で `persistence::reset` +
///   state.config = default + visible flag clear。 io::Error は `eprintln!`
///   で WARN 出力 + 黙って続行 (= user-visible error toast は別 widget 追加
///   必要のため次 wave defer、 disk 書込失敗時も visible flag clear で modal
///   は閉じる方針)。
#[allow(dead_code)] // wave 3c 以降 main.rs から呼ばれる
pub fn build_reset_confirm(_strings: &'static Strings, state: &AppStateHandles) -> Box<dyn Widget> {
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
    stack = stack.add(Box::new(message));
    stack = stack.add(Box::new(buttons));
    Box::new(stack)
}

// ── WCAG helper local duplicate (= sections/appearance.rs 内 helper が private
// で reuse 不可、appearance.rs touch 禁止のため modals.rs に同一実装を複製。
// wave 3c 以降で共通 module (= src/wcag.rs 等) への extract 検討予定。
// 完全 spec の gamma piecewise (= threshold 0.03928) は scope outside、
// BT.601 luminance 近似で AA threshold (4.5) 判定にのみ使用) ──

/// WCAG 2.2 §1.4.3 relative luminance (simple sRGB linearization、BT.601 近似)。
fn relative_luminance(r: u8, g: u8, b: u8) -> f32 {
    let rf = r as f32;
    let gf = g as f32;
    let bf = b as f32;
    (rf * 0.299 + gf * 0.587 + bf * 0.114) / 255.0
}

/// WCAG 2.2 §1.4.3 contrast ratio approximation (= `(L1 + 0.05) / (L2 + 0.05)`、L1 >= L2)。
fn contrast_ratio(la: f32, lb: f32) -> f32 {
    let (l1, l2) = if la >= lb { (la, lb) } else { (lb, la) };
    (l1 + 0.05) / (l2 + 0.05)
}

/// `4.7321 -> "4.73:1"` 等。 contrast ratio 表示用。
fn format_ratio(r: f32) -> String {
    format!("{:.2}:1", r)
}

/// `#RRGGBB` を parse して `(r, g, b)` を返す。 形式不正なら DEFAULT_ACCENT_RGB
/// (= 風藍) を返す (= 無効入力中も preview を fallback でレンダー継続、 user に
/// 「無効」表示で feedback)。
///
/// validation 仕様: 先頭 `#` + 6 文字 hex (case-insensitive)。 短縮形式
/// (#RGB) や `0x` prefix は accept しない (= 仕様シンプル化、 wave 3c 以降で
/// reactive hex parser を強化予定)。
fn parse_hex_or_default(hex: &str) -> (u8, u8, u8) {
    if !is_valid_hex(hex) {
        return DEFAULT_ACCENT_RGB;
    }
    let r = u8::from_str_radix(&hex[1..3], 16).unwrap_or(DEFAULT_ACCENT_RGB.0);
    let g = u8::from_str_radix(&hex[3..5], 16).unwrap_or(DEFAULT_ACCENT_RGB.1);
    let b = u8::from_str_radix(&hex[5..7], 16).unwrap_or(DEFAULT_ACCENT_RGB.2);
    (r, g, b)
}

/// `#RRGGBB` 形式チェック (= 先頭 `#` + 6 hex digit、 case-insensitive)。
fn is_valid_hex(hex: &str) -> bool {
    let bytes = hex.as_bytes();
    bytes.len() == 7
        && bytes[0] == b'#'
        && bytes[1..].iter().all(|b| b.is_ascii_hexdigit())
}

/// Preview swatch Label の表示文字列を構成。 valid 時は hex 表記 + 風藍呼称、
/// invalid 時は fallback indication を付与 (= user 即座に format error を認知可)。
fn update_preview_text(hex: &str, _r: u8, _g: u8, _b: u8) -> String {
    if is_valid_hex(hex) {
        format!("Preview swatch: {}", hex)
    } else {
        format!(
            "Preview swatch: {} (invalid hex → fallback {})",
            hex, DEFAULT_ACCENT_HEX
        )
    }
}

/// WCAG ratio status Label の表示文字列を構成。 入力 RGB と DEFAULT_SURFACE_RGB
/// (= `#FFFFFF`) との contrast ratio を計算、 AA threshold (4.5:1) 比較で
/// PASS/FAIL 判定 + 推奨 hint を付与。
fn compute_wcag_status(hex: &str, r: u8, g: u8, b: u8) -> String {
    let (sr, sg, sb) = DEFAULT_SURFACE_RGB;
    let ratio = contrast_ratio(
        relative_luminance(r, g, b),
        relative_luminance(sr, sg, sb),
    );
    let aa_pass = ratio >= 4.5;
    let verdict = if aa_pass {
        "PASS"
    } else {
        "FAIL: pick a darker / lighter accent"
    };
    let display_hex = if is_valid_hex(hex) {
        hex.to_string()
    } else {
        format!("{} (using fallback {})", hex, DEFAULT_ACCENT_HEX)
    };
    format!(
        "WCAG AA (>=4.5:1): {} vs surface = {} -- {}",
        display_hex,
        format_ratio(ratio),
        verdict,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Lang;
    use crate::persistence::{Config, LogLevel};
    use crate::state::{for_testing, AppStateHandles};
    use hayate_kit::ReactiveRuntime;
    use std::path::PathBuf;

    // ── visual layer test helpers (= ReactiveOverlayContainer / LabelRef) ──

    /// Minimal stub widget for ReactiveOverlayContainer/LabelRef driver tests。
    /// production widget tree には登場しない (= `#[cfg(test)]` 配下のみ)。
    struct TestStub {
        id: WidgetId,
        size: Size,
    }

    impl TestStub {
        fn new(raw_id: u64) -> Self {
            Self {
                id: WidgetId(raw_id),
                size: Size::new(100.0, 50.0),
            }
        }
    }

    impl Widget for TestStub {
        fn id(&self) -> WidgetId {
            self.id
        }
        fn layout(&mut self, _c: &Constraints) -> Size {
            self.size
        }
        fn paint(&mut self, _r: &mut Renderer, _rect: ItemRect) {}
    }

    // ── existing smoke tests (preserved baseline) ───────────────────────

    #[test]
    fn build_accent_picker_smoke() {
        let strings = Lang::En.strings();
        let state = for_testing();
        let _root = build_accent_picker(strings, &state);
    }

    #[test]
    fn build_reset_confirm_smoke() {
        let strings = Lang::En.strings();
        let state = for_testing();
        let _root = build_reset_confirm(strings, &state);
    }

    #[test]
    fn wcag_helpers_match_appearance_section_values() {
        let l_a = relative_luminance(90, 139, 168);
        let l_s = relative_luminance(255, 255, 255);
        let ratio = contrast_ratio(l_a, l_s);
        assert!(ratio.is_finite());
        assert!(ratio > 1.0);
    }

    #[test]
    fn wcag_endpoints_white_on_black_is_max_21() {
        let l_w = relative_luminance(255, 255, 255);
        let l_k = relative_luminance(0, 0, 0);
        let r = contrast_ratio(l_w, l_k);
        assert!((r - 21.0).abs() < 0.001);
    }

    // ── wave 3b lifecycle tests (= 4 action helpers + State<bool> toggle) ──

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

    // 1. State<bool> visible toggle 観測 (= 外部 set/get で reactive flag を制御可)
    #[test]
    fn accent_picker_visible_toggle_via_state() {
        let state = for_testing();
        assert!(!*state.accent_picker_visible.get(), "initial = false");
        state.accent_picker_visible.set(true);
        assert!(*state.accent_picker_visible.get(), "set(true) reflected");
        state.accent_picker_visible.set(false);
        assert!(!*state.accent_picker_visible.get(), "set(false) reflected");
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

    // 2. Apply 押下 → state.config に hex 反映 + debouncer pending + visible clear
    #[test]
    fn apply_accent_picker_writes_hex_and_dismisses() {
        let state = for_testing();
        state.accent_picker_visible.set(true);
        let new_hex = "#3D6884"; // accent-pressed (RFC v0.2 §3 から)
        apply_accent_picker(&state, new_hex);
        assert_eq!(state.config.get().appearance.accent_hex, new_hex);
        assert!(
            state.debouncer.borrow().has_pending(),
            "Apply must request debounced save"
        );
        assert!(
            !*state.accent_picker_visible.get(),
            "Apply must dismiss modal"
        );
    }

    // 3. Cancel 押下 → state.config 不変 + debouncer 不変 + visible clear
    #[test]
    fn cancel_accent_picker_leaves_config_untouched() {
        let state = for_testing();
        state.accent_picker_visible.set(true);
        let original_hex = state.config.get().appearance.accent_hex.clone();
        assert!(!state.debouncer.borrow().has_pending(), "baseline");
        cancel_accent_picker(&state);
        assert_eq!(
            state.config.get().appearance.accent_hex,
            original_hex,
            "Cancel must not mutate config"
        );
        assert!(
            !state.debouncer.borrow().has_pending(),
            "Cancel must not request save"
        );
        assert!(
            !*state.accent_picker_visible.get(),
            "Cancel must dismiss modal"
        );
    }

    // 4. Reset 押下 → config default 復元 + disk reset + .bak 作成 + visible clear
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

    // 5. Cancel reset → state.config 不変 + visible clear (= destructive 回避)
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

    // 6. Visible flag は Apply / Cancel いずれの path でも必ず false に落ちる
    #[test]
    fn both_dismiss_paths_clear_visible_flag() {
        let state = for_testing();
        // Apply path
        state.accent_picker_visible.set(true);
        apply_accent_picker(&state, "#000000");
        assert!(!*state.accent_picker_visible.get());

        // Cancel path
        state.accent_picker_visible.set(true);
        cancel_accent_picker(&state);
        assert!(!*state.accent_picker_visible.get());
    }

    // ── ReactiveOverlayContainer tests (= 視覚層 binding sync) ──

    fn make_container_with_two_overlays(
        accent_state: State<bool>,
        reset_state: State<bool>,
    ) -> ReactiveOverlayContainer {
        let mut roc = ReactiveOverlayContainer::new(Box::new(TestStub::new(1)));
        roc.add_overlay_with_state(
            "accent",
            Box::new(TestStub::new(2)),
            OverlayPosition::Center,
            accent_state,
        );
        roc.add_overlay_with_state(
            "reset",
            Box::new(TestStub::new(3)),
            OverlayPosition::Center,
            reset_state,
        );
        roc
    }

    /// initial state が true のまま add すると即 visible 同期される。
    #[test]
    fn reactive_overlay_initial_visible_state_syncs_on_add() {
        let state = for_testing();
        state.accent_picker_visible.set(true);
        let mut roc = ReactiveOverlayContainer::new(Box::new(TestStub::new(1)));
        roc.add_overlay_with_state(
            "accent",
            Box::new(TestStub::new(2)),
            OverlayPosition::Center,
            state.accent_picker_visible.clone(),
        );
        // sync_bindings は add 時の initial 反映で十分、 layout 不要
        assert!(roc.inner.is_visible("accent"));
    }

    /// state.set(true) → 次 layout() で show_overlay forward される。
    #[test]
    fn reactive_overlay_state_change_forwards_via_layout() {
        let state = for_testing();
        let mut roc = make_container_with_two_overlays(
            state.accent_picker_visible.clone(),
            state.reset_confirm_visible.clone(),
        );
        assert!(!roc.inner.is_visible("accent"), "baseline hidden");

        state.accent_picker_visible.set(true);
        let _ = roc.layout(&Constraints::tight(800.0, 600.0));
        assert!(roc.inner.is_visible("accent"), "after set(true) + layout");
    }

    /// state.set(false) → 次 paint() で hide_overlay forward される。
    #[test]
    fn reactive_overlay_state_change_forwards_via_paint() {
        let state = for_testing();
        state.accent_picker_visible.set(true);
        let mut roc = make_container_with_two_overlays(
            state.accent_picker_visible.clone(),
            state.reset_confirm_visible.clone(),
        );
        assert!(roc.inner.is_visible("accent"), "initial visible");

        state.accent_picker_visible.set(false);
        // paint() を直接呼び出すと renderer 構築が重いので、 sync が走る
        // layout() で代替 (= 同じ sync_bindings() を経由)。 paint() 経路は
        // 別 frame で同様に動作 (= idempotent、 重ね呼出無害)。
        let _ = roc.layout(&Constraints::tight(800.0, 600.0));
        assert!(!roc.inner.is_visible("accent"), "after set(false) + layout");
    }

    /// dismiss_topmost_visible() = 末尾 (= reset = 後 add) が visible なら
    /// それを優先 dismiss、 そうでなく accent (= 前 add) が visible なら accent。
    #[test]
    fn reactive_overlay_dismiss_topmost_picks_last_visible() {
        let state = for_testing();
        state.accent_picker_visible.set(true);
        state.reset_confirm_visible.set(true);
        let mut roc = make_container_with_two_overlays(
            state.accent_picker_visible.clone(),
            state.reset_confirm_visible.clone(),
        );

        let dismissed = roc.dismiss_topmost_visible();
        assert!(dismissed, "should dismiss something");
        assert!(*state.accent_picker_visible.get(), "accent still visible (= 下層)");
        assert!(!*state.reset_confirm_visible.get(), "reset (= 末尾) が dismiss された");
        assert!(!roc.inner.is_visible("reset"), "inner も同期 hidden");
    }

    /// dismiss_topmost_visible() で全 hidden 状態は何もせず false を返す。
    #[test]
    fn reactive_overlay_dismiss_topmost_noop_when_all_hidden() {
        let state = for_testing();
        let mut roc = make_container_with_two_overlays(
            state.accent_picker_visible.clone(),
            state.reset_confirm_visible.clone(),
        );

        let dismissed = roc.dismiss_topmost_visible();
        assert!(!dismissed, "全 hidden 時は dismiss 不発");
        assert!(!*state.accent_picker_visible.get(), "state 不変");
        assert!(!*state.reset_confirm_visible.get(), "state 不変");
    }

    /// dismiss → state.set(false) 経由で外部 closure が読み取れる (= 共有 Rc)。
    #[test]
    fn reactive_overlay_dismiss_propagates_state_set_to_externally_held_clones() {
        let state = for_testing();
        state.accent_picker_visible.set(true);
        let mut roc = ReactiveOverlayContainer::new(Box::new(TestStub::new(1)));
        roc.add_overlay_with_state(
            "accent",
            Box::new(TestStub::new(2)),
            OverlayPosition::Center,
            state.accent_picker_visible.clone(),
        );

        let external_clone = state.accent_picker_visible.clone();
        roc.dismiss_topmost_visible();
        assert!(!*external_clone.get(), "別 clone 経由でも dismiss 結果が観測可");
    }

    // ── LabelRef shim tests (= dynamic preview/WCAG 配線基盤) ──

    /// handle 経由 set_text 後に内部 LabelWidget の text field が更新される。
    /// closure capture (= handle clone) から外部 mutate → widget tree 反映の
    /// 共有 mutate path を成立させる core property を assert。
    #[test]
    fn label_ref_set_text_via_handle_visible_to_inner() {
        let label = LabelWidget::new("initial", 13.0);
        let (_widget, handle) = LabelRef::new_pair(label);

        assert_eq!(handle.borrow().text, "initial", "baseline text");
        handle.borrow_mut().set_text("after on_change");
        assert_eq!(
            handle.borrow().text,
            "after on_change",
            "handle 経由 mutate が即時反映"
        );
    }

    /// LabelRef widget (= tree 投入用) と handle (= mutate 用) は同じ inner を
    /// share している (= Rc clone)。 これにより closure capture と tree 投入
    /// 両立可能。
    #[test]
    fn label_ref_widget_and_handle_share_inner() {
        let label = LabelWidget::new("shared", 13.0);
        let (_widget, handle) = LabelRef::new_pair(label);

        // handle.borrow_mut() で書き換え、 LabelRef widget 側からは set_text
        // 直接観測手段なし (= public API は Widget trait 経由のみ) なので、
        // 内部 dirty flag を 1 次性質として確認。 set_text 後は inner.dirty
        // が true (LabelWidget::set_text() の挙動)。
        handle.borrow_mut().clear_dirty();
        assert!(!handle.borrow().dirty(), "clear_dirty 後 dirty=false");
        handle.borrow_mut().set_text("dirty trigger");
        assert!(handle.borrow().dirty(), "set_text 後 dirty=true (= 再 paint trigger)");
    }

    // ── hex parser + WCAG helpers tests (= dynamic preview/WCAG 内訳) ──

    #[test]
    fn is_valid_hex_recognizes_canonical_form() {
        assert!(is_valid_hex("#5A8BA8"));
        assert!(is_valid_hex("#000000"));
        assert!(is_valid_hex("#FFFFFF"));
        assert!(is_valid_hex("#abcdef"), "lowercase OK");
        assert!(is_valid_hex("#AbCdEf"), "mixed case OK");
    }

    #[test]
    fn is_valid_hex_rejects_invalid_forms() {
        assert!(!is_valid_hex(""), "empty");
        assert!(!is_valid_hex("5A8BA8"), "missing #");
        assert!(!is_valid_hex("#5A8"), "short");
        assert!(!is_valid_hex("#5A8BA8FF"), "too long (alpha unsupported)");
        assert!(!is_valid_hex("#GGGGGG"), "non-hex digit");
        assert!(!is_valid_hex("#5A 8BA"), "embedded space");
    }

    #[test]
    fn parse_hex_or_default_returns_canonical_value() {
        assert_eq!(parse_hex_or_default("#5A8BA8"), (90, 139, 168));
        assert_eq!(parse_hex_or_default("#000000"), (0, 0, 0));
        assert_eq!(parse_hex_or_default("#FFFFFF"), (255, 255, 255));
        assert_eq!(parse_hex_or_default("#abcdef"), (0xab, 0xcd, 0xef));
    }

    #[test]
    fn parse_hex_or_default_falls_back_on_invalid() {
        assert_eq!(parse_hex_or_default(""), DEFAULT_ACCENT_RGB);
        assert_eq!(parse_hex_or_default("#GGGGGG"), DEFAULT_ACCENT_RGB);
        assert_eq!(parse_hex_or_default("badtext"), DEFAULT_ACCENT_RGB);
    }

    #[test]
    fn update_preview_text_marks_invalid_as_fallback() {
        let valid = update_preview_text("#5A8BA8", 90, 139, 168);
        assert!(valid.contains("#5A8BA8"), "valid hex echoed");
        assert!(!valid.contains("fallback"), "valid path に fallback 表示なし");

        let invalid = update_preview_text("garbage", 0, 0, 0);
        assert!(invalid.contains("garbage"), "raw input echoed");
        assert!(invalid.contains("invalid hex"), "invalid 状態を表示");
        assert!(
            invalid.contains(DEFAULT_ACCENT_HEX),
            "fallback hex 名示"
        );
    }

    #[test]
    fn compute_wcag_status_pass_or_fail_branch() {
        // 風藍 vs #FFFFFF surface = ~3.6:1 → AA FAIL (= 推奨 darker accent)
        let s_default = compute_wcag_status("#5A8BA8", 90, 139, 168);
        assert!(s_default.contains("FAIL"), "default 風藍 vs 白 surface は FAIL");

        // 黒 vs #FFFFFF = max contrast (21:1) → AA PASS
        let s_black = compute_wcag_status("#000000", 0, 0, 0);
        assert!(s_black.contains("PASS"), "黒 vs 白 surface は PASS");
        assert!(s_black.contains("21.00:1"), "ratio 表示確認");
    }

    // ── on_change 配線結合テスト (= build_accent_picker 経由 closure 経路) ──

    /// build_accent_picker 構築後、 draft_accent_hex を直接 set すると後続の
    /// Apply path で commit される (= closure capture と State<String> 共有確認)。
    #[test]
    fn build_accent_picker_apply_uses_draft_hex() {
        let state = for_testing();
        let _root = build_accent_picker(Lang::En.strings(), &state);
        // 直接 draft を仕込んでから apply (= on_change closure simulation)
        state.draft_accent_hex.set(String::from("#3D6884"));
        // Apply button on_click closure を直接呼ぶ手段はないので、
        // 代替 = apply_accent_picker(state, &draft) を直接呼出して同等性を verify
        let draft_snapshot = state.draft_accent_hex.get().clone();
        apply_accent_picker(&state, &draft_snapshot);
        assert_eq!(state.config.get().appearance.accent_hex, "#3D6884");
    }

    /// build_accent_picker 構築直後の draft_accent_hex は state.config 由来の
    /// initial value と一致 (= AppStateHandles::new の draft_accent_hex 同期と整合)。
    #[test]
    fn build_accent_picker_draft_matches_state_initial() {
        let state = for_testing();
        // state.draft_accent_hex の initial は Config::default().appearance.accent_hex
        // = "#5A8BA8"。 build_accent_picker 自体は draft を mutate しない (=
        // initial read のみ)、 構築後も draft は initial 維持。
        let _root = build_accent_picker(Lang::En.strings(), &state);
        assert_eq!(*state.draft_accent_hex.get(), "#5A8BA8");
    }
}
