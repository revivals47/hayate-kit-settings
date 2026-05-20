//! Reactive overlay container + LabelRef shim (= modal 視覚層 root)。
//!
//! wave 3c-pre module split で `modals.rs` から分離 (pure refactor、 behavior
//! 変更ゼロ)。 [`ReactiveOverlayContainer`] は `State<bool>` 観測 →
//! `hayate_kit::widget::overlay::OverlayContainer` の show/hide forward、
//! [`LabelRef`] は `Rc<RefCell<LabelWidget>>` を Widget tree 投入 + 外部
//! mutate 両立する thin wrapper。

use std::cell::RefCell;
use std::rc::Rc;

use hayate_kit::prelude::widget_impl::*;

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
// - 外クリック dismiss = A2 で実装済 (= PointerPress pre-intercept、 wave 3b/3c
//   defer から framework method point_outside_visible_overlays land 後に配線)
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
    /// Enter key 押下時の Ok 系 button click と同等 action (= e.g. accent picker の
    /// Apply、 reset confirm の Reset)。 None なら Enter は単純 dismiss と等価。
    /// closure 自体が state.<modal>_visible.set(false) を呼ぶ責務を負う (= 既存
    /// apply_accent_picker / apply_reset_confirm は内部で set(false) するので、
    /// 同 helper を直接 wrap する形で渡せば自動 dismiss)。
    on_enter: Option<Box<dyn FnMut()>>,
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
/// 本 wave 3b では `modals` 内 thin wrapper (consumer-specific layer、
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

    /// Add an overlay slot bound to a `State<bool>` source (= Enter handler なし)。
    ///
    /// initial state が true なら即 inner.show_overlay 同期。 同 id を 2 回
    /// 登録すると inner 側で重複登録になる + binding が 2 個出来るので、
    /// caller は id の uniqueness を保証する。
    ///
    /// no-Enter variant: production の 2 modal (accent / reset) は
    /// [`Self::add_overlay_with_state_and_enter`] のみ使用するが、 Enter action
    /// を持たない overlay (= 単純 info dialog 等の dismiss-only modal) 用に保持。
    /// `reactive_overlay_accept_without_on_enter_falls_back_to_dismiss` test が
    /// no-Enter fallback path を網羅する正当な API variant。 premature capability
    /// 除去を避けるため allow(dead_code) で保持。
    #[allow(dead_code)]
    pub(crate) fn add_overlay_with_state(
        &mut self,
        id: impl Into<String>,
        widget: Box<dyn Widget>,
        position: OverlayPosition,
        visible_state: State<bool>,
    ) {
        self.add_overlay_internal(id.into(), widget, position, visible_state, None);
    }

    /// Add an overlay slot with both a `State<bool>` visibility source and an
    /// Enter-key action closure (= Ok 系 button click と同等 result 担当)。
    ///
    /// `on_enter` は Enter / KP_Enter キー押下時に最上位 visible binding として
    /// 起動された場合に 1 回呼ばれ、 通常は内部で `apply_*` action helper を
    /// 呼ぶ。 既存 helper (= `apply_accent_picker` / `apply_reset_confirm`) は
    /// 自身で `state.<modal>_visible.set(false)` を呼ぶため、 closure が helper
    /// を呼べば自動 dismiss + action という二重 result が成立する。
    pub(crate) fn add_overlay_with_state_and_enter(
        &mut self,
        id: impl Into<String>,
        widget: Box<dyn Widget>,
        position: OverlayPosition,
        visible_state: State<bool>,
        on_enter: impl FnMut() + 'static,
    ) {
        self.add_overlay_internal(
            id.into(),
            widget,
            position,
            visible_state,
            Some(Box::new(on_enter)),
        );
    }

    fn add_overlay_internal(
        &mut self,
        id: String,
        widget: Box<dyn Widget>,
        position: OverlayPosition,
        visible_state: State<bool>,
        on_enter: Option<Box<dyn FnMut()>>,
    ) {
        self.inner.add_overlay(id.clone(), widget, position);
        let initial = *visible_state.get();
        if initial {
            self.inner.show_overlay(&id);
        }
        self.bindings.push(OverlayBinding {
            id,
            state: visible_state,
            last_observed: initial,
            on_enter,
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
    /// 別 method 抽出理由: event() が key / pointer pre-intercept から呼び出す
    /// 共通 dismiss logic の再利用単位 (= Escape handler と A2 外クリック handler が共有)。
    /// 加えて last-visible priority / fallback / no-op といった fine-grained logic
    /// を直接 assert する unit test boundary としても機能する。
    ///
    /// (履歴: wave 3b 時点では `WidgetEvent::Key` の payload 型 `KeyEvent` が
    /// hayate-kit re-export 漏れで test code から synthesize 不可能だったため、
    /// 本 method 直呼びが event() を test する唯一の手段だった。 wave 3c GUI_kit
    /// PR #158 = feedback_widget_trait_forward_gap_pattern case 8 で
    /// `hayate_kit::{KeyEvent, KeyState, Modifiers}` re-export が land し、 event()
    /// 自体を `WidgetEvent::Key` 構築経由で end-to-end test 可能に。 本 method の
    /// 直接 test は今や event() end-to-end test と相補的な fine-grained 層。)
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

    /// Accept (= Enter key) the topmost visible overlay (if any)。
    ///
    /// Escape (= dismiss_topmost_visible) と対称構造。 binding に on_enter
    /// closure が登録されていればそれを起動 (= Ok 系 button click と同等 action、
    /// 例: apply_accent_picker / apply_reset_confirm)、 closure 自身が
    /// state.set(false) を呼ぶ責務を持つ。 on_enter 未登録なら単純 dismiss
    /// (= state.set(false) + inner.hide_overlay) に fallback。
    ///
    /// 返り値: visible binding が見つかって 1 件処理した場合 true、 全 hidden
    /// なら false (= Enter を吸わない、 base layer に届く)。
    pub(crate) fn accept_topmost_visible(&mut self) -> bool {
        for binding in self.bindings.iter_mut().rev() {
            if *binding.state.get() {
                if let Some(cb) = binding.on_enter.as_mut() {
                    cb();
                    // on_enter closure が state.set(false) を呼ぶ前提だが、
                    // 安全側で last_observed を即時 false 同期 (= 次 sync
                    // が no-op になる、 closure が set(false) 呼ばない場合は
                    // 次 frame の sync で逆方向 show が起きる = 設計通り)。
                    binding.last_observed = false;
                    self.inner.hide_overlay(&binding.id);
                } else {
                    binding.state.set(false);
                    binding.last_observed = false;
                    self.inner.hide_overlay(&binding.id);
                }
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
        // Key pre-intercept (= Escape / Return / KP_Enter)。
        // 現実装は keysym のみ判定し KeyState (Pressed/Released) で filter しない。
        // dismiss / accept は idempotent (= release 等で重複発火しても
        // hide_overlay / state.set(false) / on_enter closure は冪等 or 自己責務)
        // なので Pressed 限定 filter を設けていない (= 設計選択であって制約ではない。
        // KeyState は case 8 = GUI_kit PR #158 で re-export 済、 必要なら
        // `ke.state` で filter 可能だが本 path では不要)。
        if let WidgetEvent::Key(ke) = event {
            let ks = ke.keysym;
            if ks == xkbcommon::xkb::Keysym::Escape && self.dismiss_topmost_visible() {
                return EventResponse::Handled;
            }
            if (ks == xkbcommon::xkb::Keysym::Return || ks == xkbcommon::xkb::Keysym::KP_Enter)
                && self.accept_topmost_visible()
            {
                return EventResponse::Handled;
            }
        }
        // Pointer pre-intercept (= 外クリック dismiss、 A2)。
        // visible overlay がある状態で press 座標が全 visible overlay の外
        // (= dimmed backdrop) なら最上位 overlay を dismiss する。
        // 座標 (x, y) は WidgetEvent の widget-local。 ReactiveOverlayContainer は
        // SplitView を base に wrap した root 近傍 widget で translate を挟まない
        // ため container-local = inner OverlayContainer の overlay rect と同 space
        // (= point_outside_visible_overlays の前提座標系、
        // feedback_coord_system_platform_layer_invariant 整合)。
        // has_visible_overlay() で gate してから判定 (= overlay 皆無時は
        // point_outside_* が trivial true を返すため、 gate なしだと backdrop なし
        // でも press を吸ってしまう)。 button は問わない (= 左右中いずれの press でも
        // backdrop なら dismiss、 modal の慣例)。
        if let WidgetEvent::PointerPress { x, y, .. } = event {
            if self.inner.has_visible_overlay()
                && self.inner.point_outside_visible_overlays(*x, *y)
                && self.dismiss_topmost_visible()
            {
                return EventResponse::Handled;
            }
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

    // ── popup 系 3 callback の明示 forward (= dropdown regression root-fix) ──
    //
    // Widget trait の popup 系 callback の default impl は `children_mut()` へ
    // forward するが、 ReactiveOverlayContainer は `children_mut()` を override
    // せず inner を露出しないため、 default では inner (= OverlayContainer →
    // base SplitView 内の ComboBox) に届かず popup_request が root 不達 = 全
    // ComboBox dropdown が開かない regression を生む (= wave 3b PR #8 で
    // feedback_widget_trait_forward_gap_pattern case 1 が再発)。
    //
    // paint/event/layout/inject_engine と同じく self.inner へ明示 forward する
    // ことで inner ComboBox の popup callback を root まで貫通させる。 型は
    // hayate_kit::prelude::widget_impl 経由 reach (= GUI_kit PR #163 case 10
    // root-fix で re-export 済)。
    fn popup_request(&mut self) -> Option<PopupRequest> {
        self.inner.popup_request()
    }

    fn paint_popup(&mut self, renderer: &mut Renderer, id: PopupId) {
        self.inner.paint_popup(renderer, id);
    }

    fn on_popup_dismissed(&mut self, token: PopupWidgetToken) {
        self.inner.on_popup_dismissed(token);
    }

    // ── overlay-pass + theme + drag-finished forward (= dropdown regression
    // 完遂、 Step 2) ──
    //
    // popup_request 系と同根の gap: これら 4 callback の Widget trait default は
    // `children_mut()` 経由 forward だが、 ReactiveOverlayContainer は inner を
    // 露出せず children_mut() を override しないため inner (= OverlayContainer →
    // base SplitView 内の ComboBox) に届かない。 paint_overlay / event_overlay の
    // 不達が ComboBox dropdown を描画不可・操作不可にしていた user blocker の
    // 真因 (= popup_request 取違えの PR #17 では不足だった機構)。 inject_theme は
    // Phase 3a runtime theme swap が content 不達の latent bug、 on_drag_finished は
    // broadcast family の漏れ。 GUI_kit OverlayContainer ADD 4 (PR #164) と完全
    // symmetric に self.inner へ明示 forward する。 型は prelude::widget_impl 経由
    // reach (Renderer/WidgetEvent/EventResponse + AppTheme + DragOutcome = case 11
    // PR #165 で re-export 済)。
    //
    // DEFER 2 (on_drag_event / on_drag_start = bounds-aware routing、 broadcast で
    // ない) は OverlayContainer と同方針で本 PR 非対象 (= DnD-through-overlay 需要
    // 発生時の focused follow-up)。 N/A (children_mut) も同様 override せず明示
    // forward 方針。
    //
    // paint_overlay / event_overlay は overlay visibility に依存するため、
    // paint / event / layout と同じく冒頭で sync_bindings() を呼び inner の
    // overlay 可視状態を最新化してから forward する。 inject_theme /
    // on_drag_finished は visibility 非依存 broadcast ゆえ pure pass-through。
    fn paint_overlay(&mut self, renderer: &mut Renderer) {
        self.sync_bindings();
        self.inner.paint_overlay(renderer);
    }

    fn event_overlay(&mut self, event: &WidgetEvent) -> EventResponse {
        self.sync_bindings();
        self.inner.event_overlay(event)
    }

    fn inject_theme(&mut self, theme: Rc<AppTheme>) {
        self.inner.inject_theme(theme);
    }

    fn on_drag_finished(&mut self, outcome: DragOutcome) {
        self.inner.on_drag_finished(outcome);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::for_testing;

    /// Synthesize a `WidgetEvent::Key` for the given keysym (= Pressed, no mods)。
    ///
    /// `KeyEvent` / `KeyState` / `Modifiers` は GUI_kit PR #158
    /// (= feedback_widget_trait_forward_gap_pattern case 8) の re-export で
    /// hayate-kit only 依存規範下でも構築可能になった。 これにより event() の
    /// Key pre-intercept path (Escape → dismiss / Enter → accept) を method
    /// 直呼び迂回なしに end-to-end test できる。 production event() は keysym
    /// のみ判定し KeyState を見ない (overlay.rs event() の comment 参照) ため
    /// state は Pressed 固定で十分。
    fn key_event(keysym: xkbcommon::xkb::Keysym) -> WidgetEvent {
        WidgetEvent::Key(KeyEvent {
            key: 0,
            keysym,
            utf8: None,
            modifiers: Modifiers::default(),
            state: KeyState::Pressed,
        })
    }

    /// Synthesize a left-button `WidgetEvent::PointerPress` at container-local
    /// (x, y) (= A2 外クリック dismiss の event() pre-intercept を駆動)。
    fn pointer_press(x: f32, y: f32) -> WidgetEvent {
        WidgetEvent::PointerPress {
            x,
            y,
            button: 0x110, // left
            modifiers: Modifiers::default(),
        }
    }

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

        /// 任意 size の stub (= 外クリック hit-test test で base > overlay の
        /// サイズ差を作り、 内側/外側を区別可能にするため)。
        fn with_size(raw_id: u64, w: f32, h: f32) -> Self {
            Self {
                id: WidgetId(raw_id),
                size: Size::new(w, h),
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

    /// Shared call-count flags for [`ForwardProbe`]'s symmetric-4 callbacks。
    #[derive(Clone, Default)]
    struct ForwardFlags {
        paint_overlay: Rc<std::cell::Cell<u32>>,
        event_overlay: Rc<std::cell::Cell<u32>>,
        theme: Rc<std::cell::Cell<u32>>,
        drag_finished: Rc<std::cell::Cell<u32>>,
    }

    /// Probe base widget recording the symmetric-4 forwards (= overlay-pass +
    /// theme + drag-finished) via shared [`ForwardFlags`]。 ReactiveOverlayContainer
    /// が self.inner → base へ forward することを assert する (= Step 2 の gap fix)。
    struct ForwardProbe {
        id: WidgetId,
        flags: ForwardFlags,
    }

    impl ForwardProbe {
        fn new(raw_id: u64) -> (Self, ForwardFlags) {
            let flags = ForwardFlags::default();
            let probe = Self {
                id: WidgetId(raw_id),
                flags: flags.clone(),
            };
            (probe, flags)
        }
    }

    impl Widget for ForwardProbe {
        fn id(&self) -> WidgetId {
            self.id
        }
        fn layout(&mut self, _c: &Constraints) -> Size {
            Size::new(100.0, 50.0)
        }
        fn paint(&mut self, _r: &mut Renderer, _rect: ItemRect) {}
        fn paint_overlay(&mut self, _r: &mut Renderer) {
            self.flags.paint_overlay.set(self.flags.paint_overlay.get() + 1);
        }
        fn event_overlay(&mut self, _e: &WidgetEvent) -> EventResponse {
            self.flags.event_overlay.set(self.flags.event_overlay.get() + 1);
            EventResponse::Ignored
        }
        fn inject_theme(&mut self, _t: Rc<AppTheme>) {
            self.flags.theme.set(self.flags.theme.get() + 1);
        }
        fn on_drag_finished(&mut self, _o: DragOutcome) {
            self.flags.drag_finished.set(self.flags.drag_finished.get() + 1);
        }
    }

    /// Step 2 regression: ReactiveOverlayContainer の symmetric-4 forward が
    /// self.inner 経由で base に到達する (= paint_overlay/event_overlay は dropdown
    /// 描画・操作 blocker、 inject_theme は theme swap content 反映、 on_drag_finished
    /// は broadcast)。 OverlayContainer ADD 4 (PR #164) と完全 symmetric。
    #[test]
    fn reactive_overlay_forwards_symmetric_four_to_base() {
        let (base, flags) = ForwardProbe::new(1);
        let mut roc = ReactiveOverlayContainer::new(Box::new(base));

        // paint_overlay → inner → base
        let mut buf = vec![0u8; 64 * 64 * 4];
        let mut r = Renderer::Cpu {
            canvas: &mut buf,
            stride: 64 * 4,
            width: 64,
            height: 64,
        };
        roc.paint_overlay(&mut r);
        assert_eq!(flags.paint_overlay.get(), 1, "paint_overlay reached base (dropdown 描画)");

        // event_overlay → inner → base (no visible overlay → falls to base)
        roc.event_overlay(&WidgetEvent::PointerMove { x: 1.0, y: 1.0 });
        assert_eq!(flags.event_overlay.get(), 1, "event_overlay reached base (dropdown 操作)");

        // inject_theme → inner → base (= Phase 3a theme swap が content 反映)
        roc.inject_theme(Rc::new(AppTheme::default()));
        assert_eq!(flags.theme.get(), 1, "inject_theme reached base (theme swap)");

        // on_drag_finished → inner → base broadcast
        roc.on_drag_finished(DragOutcome::Cancelled);
        assert_eq!(flags.drag_finished.get(), 1, "on_drag_finished reached base");
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

    /// accept_topmost_visible() = on_enter closure が registered なら invoke
    /// (= closure が state.set(false) と Ok 系 action を担当)。 全 hidden なら false。
    #[test]
    fn reactive_overlay_accept_invokes_on_enter_closure() {
        use std::cell::Cell;
        use std::rc::Rc as StdRc;

        let state = for_testing();
        state.accent_picker_visible.set(true);
        let mut roc = ReactiveOverlayContainer::new(Box::new(TestStub::new(1)));

        let invocation_count = StdRc::new(Cell::new(0));
        let count_clone = StdRc::clone(&invocation_count);
        let state_for_closure = state.clone();
        roc.add_overlay_with_state_and_enter(
            "accent",
            Box::new(TestStub::new(2)),
            OverlayPosition::Center,
            state.accent_picker_visible.clone(),
            move || {
                count_clone.set(count_clone.get() + 1);
                // closure 自身が state.set(false) を呼ぶ責務 (= 実 usage では
                // apply_accent_picker / apply_reset_confirm が同等の責務を持つ)。
                state_for_closure.accent_picker_visible.set(false);
            },
        );

        let accepted = roc.accept_topmost_visible();
        assert!(accepted, "visible binding 有なら true");
        assert_eq!(invocation_count.get(), 1, "on_enter closure が 1 回呼ばれた");
        assert!(!*state.accent_picker_visible.get(), "state が dismiss された");
    }

    /// accept_topmost_visible() = on_enter 不在 binding は dismiss fallback。
    #[test]
    fn reactive_overlay_accept_without_on_enter_falls_back_to_dismiss() {
        let state = for_testing();
        state.accent_picker_visible.set(true);
        let mut roc = ReactiveOverlayContainer::new(Box::new(TestStub::new(1)));
        roc.add_overlay_with_state(
            "accent",
            Box::new(TestStub::new(2)),
            OverlayPosition::Center,
            state.accent_picker_visible.clone(),
        );

        let accepted = roc.accept_topmost_visible();
        assert!(accepted, "visible binding 有なら true");
        assert!(
            !*state.accent_picker_visible.get(),
            "on_enter 未登録時は fallback で state.set(false)"
        );
    }

    /// accept_topmost_visible() = 全 hidden なら no-op + false。
    #[test]
    fn reactive_overlay_accept_noop_when_all_hidden() {
        let state = for_testing();
        let mut roc = ReactiveOverlayContainer::new(Box::new(TestStub::new(1)));
        roc.add_overlay_with_state_and_enter(
            "accent",
            Box::new(TestStub::new(2)),
            OverlayPosition::Center,
            state.accent_picker_visible.clone(),
            || panic!("hidden 状態で on_enter は呼ばれない"),
        );

        let accepted = roc.accept_topmost_visible();
        assert!(!accepted, "全 hidden 時は false");
    }

    /// accept_topmost_visible() = 末尾 (= 後 add) 優先で 1 件のみ処理。
    #[test]
    fn reactive_overlay_accept_picks_last_visible_only() {
        use std::cell::Cell;
        use std::rc::Rc as StdRc;

        let state = for_testing();
        state.accent_picker_visible.set(true);
        state.reset_confirm_visible.set(true);
        let mut roc = ReactiveOverlayContainer::new(Box::new(TestStub::new(1)));

        let accent_fired = StdRc::new(Cell::new(false));
        let accent_fired_c = StdRc::clone(&accent_fired);
        let accent_state = state.clone();
        roc.add_overlay_with_state_and_enter(
            "accent",
            Box::new(TestStub::new(2)),
            OverlayPosition::Center,
            state.accent_picker_visible.clone(),
            move || {
                accent_fired_c.set(true);
                accent_state.accent_picker_visible.set(false);
            },
        );
        let reset_fired = StdRc::new(Cell::new(false));
        let reset_fired_c = StdRc::clone(&reset_fired);
        let reset_state = state.clone();
        roc.add_overlay_with_state_and_enter(
            "reset",
            Box::new(TestStub::new(3)),
            OverlayPosition::Center,
            state.reset_confirm_visible.clone(),
            move || {
                reset_fired_c.set(true);
                reset_state.reset_confirm_visible.set(false);
            },
        );

        let accepted = roc.accept_topmost_visible();
        assert!(accepted);
        assert!(!accent_fired.get(), "accent (= 下層) は触らない");
        assert!(reset_fired.get(), "reset (= 末尾) のみ accept");
    }

    // ── event() Key pre-intercept end-to-end tests (= wave 3c item 4) ──
    //
    // 上記 dismiss_topmost_visible / accept_topmost_visible の直呼び test は
    // method logic を fine-grained に検証する。 以下は GUI_kit PR #158 (case 8
    // re-export) で初めて可能になった、 `WidgetEvent::Key` 構築 → event() への
    // dispatch という end-to-end path の検証。 keysym → method routing
    // (Escape→dismiss / Return・KP_Enter→accept) + 全 hidden 時 pass-through を
    // cover する。

    /// event() に Escape Key を渡すと最上位 visible overlay が dismiss され、
    /// EventResponse::Handled を返す (= pre-intercept 成立)。
    #[test]
    fn event_escape_key_dismisses_topmost_visible_end_to_end() {
        let state = for_testing();
        state.accent_picker_visible.set(true);
        let mut roc = make_container_with_two_overlays(
            state.accent_picker_visible.clone(),
            state.reset_confirm_visible.clone(),
        );

        let resp = roc.event(&key_event(xkbcommon::xkb::Keysym::Escape));
        assert_eq!(
            resp,
            EventResponse::Handled,
            "visible overlay 上での Escape は event() で Handled"
        );
        assert!(
            !*state.accent_picker_visible.get(),
            "Escape が event() 経由で accent を dismiss した"
        );
    }

    /// event() に Return Key を渡すと最上位 visible binding の on_enter closure が
    /// 起動され、 Handled を返す (= accept pre-intercept 成立)。
    #[test]
    fn event_return_key_accepts_topmost_visible_end_to_end() {
        use std::cell::Cell;
        use std::rc::Rc as StdRc;

        let state = for_testing();
        state.accent_picker_visible.set(true);
        let mut roc = ReactiveOverlayContainer::new(Box::new(TestStub::new(1)));

        let fired = StdRc::new(Cell::new(false));
        let fired_c = StdRc::clone(&fired);
        let state_c = state.clone();
        roc.add_overlay_with_state_and_enter(
            "accent",
            Box::new(TestStub::new(2)),
            OverlayPosition::Center,
            state.accent_picker_visible.clone(),
            move || {
                fired_c.set(true);
                state_c.accent_picker_visible.set(false);
            },
        );

        let resp = roc.event(&key_event(xkbcommon::xkb::Keysym::Return));
        assert_eq!(resp, EventResponse::Handled, "Return は event() で Handled");
        assert!(fired.get(), "Return が on_enter closure を event() 経由で起動");
        assert!(
            !*state.accent_picker_visible.get(),
            "on_enter closure が state を dismiss"
        );
    }

    /// KP_Enter (テンキー Enter) も Return と同様に accept path を叩く。
    #[test]
    fn event_kp_enter_key_accepts_topmost_visible_end_to_end() {
        use std::cell::Cell;
        use std::rc::Rc as StdRc;

        let state = for_testing();
        state.accent_picker_visible.set(true);
        let mut roc = ReactiveOverlayContainer::new(Box::new(TestStub::new(1)));

        let fired = StdRc::new(Cell::new(false));
        let fired_c = StdRc::clone(&fired);
        let state_c = state.clone();
        roc.add_overlay_with_state_and_enter(
            "accent",
            Box::new(TestStub::new(2)),
            OverlayPosition::Center,
            state.accent_picker_visible.clone(),
            move || {
                fired_c.set(true);
                state_c.accent_picker_visible.set(false);
            },
        );

        let resp = roc.event(&key_event(xkbcommon::xkb::Keysym::KP_Enter));
        assert_eq!(resp, EventResponse::Handled, "KP_Enter も Handled");
        assert!(fired.get(), "KP_Enter が on_enter closure を起動");
    }

    /// 全 overlay hidden 時、 Escape は pre-intercept で吸われず (= dismiss 不発)、
    /// state は不変のまま inner へ pass-through する (= base layer に届く)。
    #[test]
    fn event_escape_when_all_hidden_passes_through_without_state_change() {
        let state = for_testing();
        let mut roc = make_container_with_two_overlays(
            state.accent_picker_visible.clone(),
            state.reset_confirm_visible.clone(),
        );

        let resp = roc.event(&key_event(xkbcommon::xkb::Keysym::Escape));
        // dismiss_topmost_visible が false を返すため pre-intercept は Handled を
        // 返さず inner.event() に委譲される (= Escape を吸わない)。
        assert_ne!(
            resp,
            EventResponse::Handled,
            "全 hidden 時の Escape は pre-intercept で Handled にならない"
        );
        assert!(!*state.accent_picker_visible.get(), "state 不変");
        assert!(!*state.reset_confirm_visible.get(), "state 不変");
    }

    /// 非 Escape/Enter の Key (= 例: 文字 'a') は pre-intercept 対象外で、
    /// visible overlay があっても dismiss/accept しない。
    #[test]
    fn event_unrelated_key_does_not_trigger_pre_intercept() {
        let state = for_testing();
        state.accent_picker_visible.set(true);
        let mut roc = make_container_with_two_overlays(
            state.accent_picker_visible.clone(),
            state.reset_confirm_visible.clone(),
        );

        let _ = roc.event(&key_event(xkbcommon::xkb::Keysym::a));
        assert!(
            *state.accent_picker_visible.get(),
            "無関係 key では accent は dismiss されず visible のまま"
        );
    }

    // ── PointerPress 外クリック dismiss end-to-end tests (= A2 Step 2) ──
    //
    // 座標系実証: ReactiveOverlayContainer.event() に渡る PointerPress(x, y) を
    // container-local 座標として inner.point_outside_visible_overlays に通し、
    // 既知 click 位置で内側/外側判定が正しく dismiss に結びつくことを確認する。
    // base 800×600 + Center overlay 100×50 → overlay rect = (350,275,100,50)。
    // (x,y) が container-local でなければこの内側/外側判定は破綻するため、 本
    // test 群が座標系整合の unit-level 実証となる (= 実機 click の visual smoke は
    // 別途、 但し座標が parent-translated なら本 test が落ちて検出できる)。

    /// base 800×600 + Center overlay 100×50 (= rect (350,275,100,50)) を visible
    /// 状態で layout 済にした container を返す helper。
    fn laid_out_single_centered_overlay(visible: State<bool>) -> ReactiveOverlayContainer {
        let mut roc = ReactiveOverlayContainer::new(Box::new(TestStub::with_size(1, 800.0, 600.0)));
        roc.add_overlay_with_state(
            "dlg",
            Box::new(TestStub::with_size(2, 100.0, 50.0)),
            OverlayPosition::Center,
            visible,
        );
        // layout で last_size=800×600 + overlay cached_size=100×50 を確定
        // (= overlay_rect 計算の前提)。
        let _ = roc.layout(&Constraints::tight(800.0, 600.0));
        roc
    }

    /// PointerPress が overlay 外 (= dimmed backdrop) なら dismiss + Handled。
    #[test]
    fn event_pointer_press_outside_visible_overlay_dismisses() {
        let state = for_testing();
        state.accent_picker_visible.set(true);
        let mut roc = laid_out_single_centered_overlay(state.accent_picker_visible.clone());

        // (10, 10) は rect (350,275,100,50) の外 = backdrop
        let resp = roc.event(&pointer_press(10.0, 10.0));
        assert_eq!(
            resp,
            EventResponse::Handled,
            "backdrop click は event() で Handled"
        );
        assert!(
            !*state.accent_picker_visible.get(),
            "backdrop click が overlay を dismiss した"
        );
    }

    /// PointerPress が overlay 内側なら dismiss しない (= modal 内の操作)。
    #[test]
    fn event_pointer_press_inside_visible_overlay_does_not_dismiss() {
        let state = for_testing();
        state.accent_picker_visible.set(true);
        let mut roc = laid_out_single_centered_overlay(state.accent_picker_visible.clone());

        // (400, 300) は rect (350,275,100,50) の内側。
        // 内側 click は外クリック pre-intercept を発火させず dismiss しない。
        // (EventResponse は inner OverlayContainer の modal routing が内側 click を
        // 消費して Handled を返しうる = 背後 base への透過を block する正しい modal
        // 挙動。 ここで検証すべき不変は「dismiss されないこと」= state 維持。)
        let _resp = roc.event(&pointer_press(400.0, 300.0));
        assert!(
            *state.accent_picker_visible.get(),
            "overlay 内側 click では dismiss されず visible のまま"
        );
    }

    /// visible overlay 皆無時は PointerPress を pre-intercept しない (= gate)。
    /// point_outside_* は overlay 皆無時 trivial true を返すため、 gate なしだと
    /// backdrop が存在しないのに press を吸ってしまう。 has_visible_overlay() gate
    /// がそれを防ぐことを確認。
    #[test]
    fn event_pointer_press_when_no_overlay_visible_passes_through() {
        let state = for_testing();
        // overlay を add するが visible にしない
        let mut roc = laid_out_single_centered_overlay(state.accent_picker_visible.clone());
        assert!(!roc.inner.has_visible_overlay(), "baseline: visible 皆無");

        let resp = roc.event(&pointer_press(10.0, 10.0));
        assert_ne!(
            resp,
            EventResponse::Handled,
            "visible overlay 皆無時の PointerPress は pre-intercept で吸われない"
        );
        assert!(
            !*state.accent_picker_visible.get(),
            "state 不変 (= 元から hidden)"
        );
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

    // ── popup callback forward regression guard (= dropdown 全滅 root-fix) ──
    //
    // PopupRequest は PopupConfig (= move-only、 hayate-kit 未 re-export) を要し
    // hayate_kit only 依存規範下で `Some(..)` を構築できないため、 戻り値ではなく
    // 「inner child の callback が **呼ばれたか**」を probe で記録して forward 成立を
    // 機械的に証明する。 fix 前 (= default children_mut() forward、 ReactiveOverlay-
    // Container は children_mut 未 override で空) なら probe に届かず flag が false の
    // まま = test fail。 fix 後 (= self.inner へ明示 forward) なら届いて flag が true。

    /// inner child に置いて popup callback の到達を記録する probe widget。
    struct PopupProbe {
        id: WidgetId,
        popup_requested: Rc<std::cell::Cell<bool>>,
        dismissed_token: Rc<RefCell<Option<PopupWidgetToken>>>,
    }

    impl PopupProbe {
        // 戻り型は probe 用の 3-tuple (= widget + 2 観測 handle)。 test helper
        // ゆえ type alias 化せず allow (= pre-existing、 Step 2 で --all-targets
        // 緑化のため scoped allow を付与)。
        #[allow(clippy::type_complexity)]
        fn new(
            raw_id: u64,
        ) -> (
            Self,
            Rc<std::cell::Cell<bool>>,
            Rc<RefCell<Option<PopupWidgetToken>>>,
        ) {
            let requested = Rc::new(std::cell::Cell::new(false));
            let dismissed = Rc::new(RefCell::new(None));
            let probe = Self {
                id: WidgetId(raw_id),
                popup_requested: Rc::clone(&requested),
                dismissed_token: Rc::clone(&dismissed),
            };
            (probe, requested, dismissed)
        }
    }

    impl Widget for PopupProbe {
        fn id(&self) -> WidgetId {
            self.id
        }
        fn layout(&mut self, _c: &Constraints) -> Size {
            Size::new(10.0, 10.0)
        }
        fn paint(&mut self, _r: &mut Renderer, _rect: ItemRect) {}
        fn popup_request(&mut self) -> Option<PopupRequest> {
            // 呼ばれたことを記録 (= forward 到達の証明)。 PopupRequest は構築不能
            // のため戻り値は None で良い (= test は call 到達のみを assert)。
            self.popup_requested.set(true);
            None
        }
        fn on_popup_dismissed(&mut self, token: PopupWidgetToken) {
            *self.dismissed_token.borrow_mut() = Some(token);
        }
    }

    /// `ReactiveOverlayContainer::popup_request()` が inner (= OverlayContainer
    /// → base) の child まで forward されること。 fix 前は default children_mut()
    /// forward (空) で probe に届かず regression。
    #[test]
    fn popup_request_forwards_through_to_inner_child() {
        let (probe, requested, _) = PopupProbe::new(99);
        let mut roc = ReactiveOverlayContainer::new(Box::new(probe));
        let _ = roc.popup_request();
        assert!(
            requested.get(),
            "popup_request must reach inner child (= dropdown 全滅 regression guard)"
        );
    }

    /// `ReactiveOverlayContainer::on_popup_dismissed()` が inner child まで
    /// forward され、 同一 token が届くこと。
    #[test]
    fn on_popup_dismissed_forwards_through_to_inner_child() {
        let (probe, _, dismissed) = PopupProbe::new(98);
        let mut roc = ReactiveOverlayContainer::new(Box::new(probe));
        let token = PopupWidgetToken(42);
        roc.on_popup_dismissed(token);
        assert_eq!(
            *dismissed.borrow(),
            Some(token),
            "on_popup_dismissed must reach inner child with same token"
        );
    }
}
