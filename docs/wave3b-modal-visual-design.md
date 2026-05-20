# wave 3b modal visual wire — design draft

**Status (v0.2)**: implementation revision applied — see §10 for change log v0.1 → v0.2。
**Branch**: track2/phase2-wave3-modal-lifecycle
**Prerequisites**: GUI_kit-track-framework-extend land (= `TextInput::on_change`
callback method 追加 + hayate-kit から `Renderer` / `ItemRect` / `TextEngine` /
`alloc_widget_id` / `PopupId` re-export 追加) ← v0.1 表現、 v0.2 で
**PR #155 として land 完了** + 副次 finding により `alloc_widget_id` / `PopupId`
は worker2 case 不要に確定 (= §10 詳細)
**Author**: worker2 (= 並走 design draft、 boss1 推奨 iii)

> ⚠️ **READ FIRST**: §2 / §3 / §5 の `AlertDialogContainer` 名 + AlertDialog
> 採用設計 + `hayate_kit::Key::Escape` 参照は v0.1 時点の draft。 v0.2 実装で
> 採用された design は §10 revision history を参照 (= `ReactiveOverlayContainer`
> + OverlayContainer 利用 + 混在 → VStack 統一 + `xkbcommon::xkb::Keysym::Escape`)。
> 本 body §2-§9 は v0.1 として保存、 後世の re-discover 用に維持 (= workflow
> pattern 4 適用)。

## 1. 背景と目的

Phase 2 wave 3b track2 commit `baca8fc` で **logic-only subset** が land 済:
- 4 pub(crate) action helpers (apply/cancel × accent_picker/reset_confirm)
- button on_click closure → state mutate 配線
- 7 lifecycle unit test (= dispatch 4 項目を全 cover)

未着手 = **視覚 modal overlay**:
- AlertDialog show/hide を State<bool> で外部 control
- TextInput hex 入力 → 動的 preview swatch + WCAG ratio 再計算
- main.rs に modal overlay (= Z-order / focus capture / 外クリック dismiss)

framework gap (= `hayate_kit::Widget` trait method signature 内 `Renderer` /
`ItemRect` / `TextEngine` 未 re-export、 custom Widget impl 不可) が解消され
次第、 本 doc の design に従って follow-up commit を同 branch に積層し、
full scope 1 PR で raise する。

## 2. AlertDialogContainer custom Widget 設計

### 2.1 struct fields

```rust
use hayate_kit::widget::alert_dialog::AlertDialog;
use hayate_kit::{Constraints, EventResponse, ItemRect, Renderer, Size,
                  State, TextEngine, Widget, WidgetEvent, WidgetId};
use std::cell::RefCell;
use std::rc::Rc;

/// State<bool> 外部 control 経由で内部 AlertDialog の show/hide を駆動する
/// thin wrapper widget。 widget tree に Box::new() で投入後も visible 制御は
/// 外部 State<bool> を set/get するだけで完結 (= 直接 mutable handle 不要)。
pub(crate) struct AlertDialogContainer {
    /// Widget tree 上での stable identity (= focus / dirty tracking 用)。
    id: WidgetId,
    /// 内部 AlertDialog (= hayate-kit 既存 widget、 chrome + buttons を担当)。
    inner: AlertDialog,
    /// 外部 control source。 set/get で内部 show/hide を変動させる。
    visible_state: State<bool>,
    /// 前回 paint/event 時に観測した visible 値 (= 変化検出で show/hide forward)。
    last_observed_visible: bool,
    /// layout cache (= max_width / max_height 受領済の場合に再 layout を skip)。
    cached_size: Option<Size>,
    /// engine inject 経路 (= text 描画 prerequisite、 子 widget へも forward)。
    engine: Option<Rc<RefCell<TextEngine>>>,
    /// dirty flag (= 自分自身が再描画必要かを framework に通知)。
    is_dirty: bool,
}
```

### 2.2 impl Widget (= core method pseudocode)

```rust
impl Widget for AlertDialogContainer {
    fn id(&self) -> WidgetId { self.id }

    fn layout(&mut self, constraints: &Constraints) -> Size {
        // visible が false なら zero-size (= 親 layout に影響しない)、
        // true なら inner.layout 委譲。 layout 結果は cached_size に保存。
        if !*self.visible_state.get() {
            return Size::zero();
        }
        let s = self.inner.layout(constraints);
        self.cached_size = Some(s);
        s
    }

    fn paint(&mut self, renderer: &mut Renderer, rect: ItemRect) {
        // visible 観測値が前回と差分あれば inner.show()/hide() を forward。
        // ここで synchronizing する理由 (= update() でも同等可能だが):
        //  - paint は常に呼ばれる確定 timing
        //  - update() は dt 受領のため動かない時 dirty 再計算 race を回避
        let visible_now = *self.visible_state.get();
        if visible_now != self.last_observed_visible {
            if visible_now { self.inner.show(); } else { self.inner.hide(); }
            self.last_observed_visible = visible_now;
            self.is_dirty = true;
        }
        if visible_now {
            self.inner.paint(renderer, rect);
        }
        // else: paint なし (= 透過、 直下 widget が見える)
    }

    fn event(&mut self, event: &WidgetEvent) -> EventResponse {
        // visible 時のみ event 食う (= modal なので背後 widget へ届けない)。
        // 内部 AlertDialog の on_result が button 押下時に発火、
        // 自分自身が visible_state.set(false) で dismiss するのではなく、
        // build_*_confirm 側の on_click callback (= 既 land の logic-only
        // subset) が State 更新 → 次 paint で last_observed_visible 差分検出
        // → hide() forward の 1-frame遅延 chain で閉じる。
        if !*self.visible_state.get() {
            return EventResponse::Ignored;
        }
        // Escape / outside-click dismiss (= modal UX 標準) は外部 main.rs 側で
        // 別 widget を被せて handle するか、 ここで `WidgetEvent::KeyPress(Escape)`
        // 検出時 visible_state.set(false) するか選択 (= 推奨後者、 close 経路
        // が外部 State 経由で一貫)。
        if let WidgetEvent::KeyPress { key, .. } = event {
            if *key == hayate_kit::Key::Escape {
                self.visible_state.set(false);
                return EventResponse::Handled;
            }
        }
        self.inner.event(event)
    }

    fn paint_overlay(&mut self, renderer: &mut Renderer) {
        // AlertDialog 自体が modal overlay (= dim backdrop + center panel) を
        // paint_overlay で描く実装。 visible 時のみ forward。
        if !*self.visible_state.get() { return; }
        self.inner.paint_overlay(renderer);
    }

    fn dirty(&self) -> bool {
        self.is_dirty || self.inner.dirty()
    }

    fn clear_dirty(&mut self) {
        self.is_dirty = false;
        self.inner.clear_dirty();
    }

    fn inject_engine(&mut self, engine: Rc<RefCell<TextEngine>>) {
        self.engine = Some(Rc::clone(&engine));
        self.inner.inject_engine(engine);
    }
}
```

### 2.3 構築 helper (= modals.rs に追加)

```rust
/// AlertDialog をベースに visible_state 経由 control 可能な container を構築。
/// inner AlertDialog の title / message / buttons + on_result callback は
/// caller が事前に builder pattern で完成させた状態で渡す。
pub(crate) fn alert_dialog_container(
    inner: AlertDialog,
    visible_state: State<bool>,
) -> AlertDialogContainer {
    AlertDialogContainer {
        id: hayate_kit::widget::widget_id::alloc_widget_id(),
        inner,
        visible_state,
        last_observed_visible: false,
        cached_size: None,
        engine: None,
        is_dirty: true,
    }
}
```

`alloc_widget_id` は framework PR で hayate-kit から re-export される
(= `hayate_kit::widget::widget_id::alloc_widget_id` 経路を想定)。

## 3. State<bool> 観測 mechanism — 2 案検討、 採用 = paint-time poll

### 案 A (採用): paint-time poll
- paint() で `*self.visible_state.get()` を read、 前回値と差分検出
- pros: 単純、 reactive runtime への額外 subscription 登録不要、 self-contained
- cons: visible 変化から実反映まで 1 frame 遅延 (= 60fps で ~16ms、 modal UX
  上 imperceptible)

### 案 B (不採用): Effect callback
- ReactiveRuntime::create_effect で visible_state 観測、 callback で
  inner.show()/hide()
- cons: Effect callback 内から `&mut AlertDialog` 不可能 (= widget tree owner)、
  Rc<RefCell<AlertDialog>> 必要で循環参照 risk

→ **paint-time poll で十分**、 案 A 採用。

## 4. 動的 preview swatch + WCAG ratio 再計算

### 4.1 TextInput::on_change 受領経路 (framework PR で追加予定)

```rust
let on_change_state = state.clone();
let preview_label_handle: Rc<RefCell<LabelWidget>> = Rc::new(RefCell::new(...));
let wcag_label_handle: Rc<RefCell<LabelWidget>> = Rc::new(RefCell::new(...));
let preview_clone = Rc::clone(&preview_label_handle);
let wcag_clone = Rc::clone(&wcag_label_handle);

let hex_input = TextInputWidget::new()
    .with_placeholder(DEFAULT_ACCENT_HEX)
    .with_width(160.0)
    .on_change(move |new_text: &str| {  // ← framework PR で追加 method
        // hex validation (= #RRGGBB 形式チェック、 invalid 時は default 扱い)
        let (r, g, b) = parse_hex_or_default(new_text);
        // preview swatch label 更新
        preview_clone.borrow_mut().set_text(&format!(
            "Preview swatch: {} ({})", new_text,
            if is_valid_hex(new_text) { "valid" } else { "invalid → fallback" }
        ));
        // WCAG ratio 再計算 + label 更新
        let ratio = contrast_ratio(
            relative_luminance(r, g, b),
            relative_luminance(255, 255, 255),
        );
        wcag_clone.borrow_mut().set_text(&format!(
            "WCAG AA (>=4.5:1): {} vs surface = {} -- {}",
            new_text,
            format_ratio(ratio),
            if ratio >= 4.5 { "PASS" } else { "FAIL" },
        ));
        // draft hex を state.draft_accent_hex に保存 (= Apply ボタンで読出)
        on_change_state.draft_accent_hex.set(new_text.to_string());
    });
```

### 4.2 prerequisite struct field 追加 (state.rs)

```rust
pub struct AppStateHandles {
    // ... existing fields ...
    /// accent picker hex 入力の draft 値 (= Apply 押下まで config 不反映)、
    /// TextInput on_change で都度更新、 Apply 押下時に config に commit。
    pub draft_accent_hex: State<String>,
}
```

initial value = `String::from("#5A8BA8")` (= DEFAULT_ACCENT_HEX)。

### 4.3 Apply button on_click closure 再配線

```rust
let apply_state = state.clone();
let apply_btn = ButtonWidget::new("Apply").on_click(move || {
    let draft = apply_state.draft_accent_hex.get().clone();
    apply_accent_picker(&apply_state, &draft);
});
```

(= 既 land logic-only subset の `DEFAULT_ACCENT_HEX` 固定 placeholder を
draft 経由 dynamic 読出に置換)

### 4.4 Rc<RefCell<LabelWidget>> 共有経路

LabelWidget が widget tree に Box::new() で投入された後も `set_text` で外部
mutate 可能なよう、 build_accent_picker 内で `Rc::new(RefCell::new(label))`
で作って、 一方を closure capture、 もう一方を Box 化して tree 投入する。

問題: `Box<dyn Widget>` と `Rc<RefCell<LabelWidget>>` の型 mismatch。
→ Widget shim wrapper を 1 つ書く (= `LabelRef(Rc<RefCell<LabelWidget>>)`)、
   `impl Widget for LabelRef` で全 method を borrow().method() / borrow_mut().method() 経由 forward。

framework PR で `Renderer` / `ItemRect` 等 re-export 完了後にこの shim を
modals.rs ローカルで 30 行ほどで実装可能。

## 5. main.rs modal overlay 配置

### 5.1 Z-order

現状 main.rs の root = `SplitViewWidget { sidebar, detail }`。 modal を
重ねるには root を ZStack (or 同等 layout widget) に変更:

```rust
let split = SplitViewWidget::new(sidebar, detail, SplitOrientation::Horizontal)
    .with_ratio(0.3)
    .with_min_sizes(180.0, 400.0);

let accent_modal = alert_dialog_container(
    accent_picker_alert_dialog(strings, &app_state),  // 既存 build 結果を AlertDialog ベースに reshape
    app_state.accent_picker_visible.clone(),
);
let reset_modal = alert_dialog_container(
    reset_confirm_alert_dialog(strings, &app_state),
    app_state.reset_confirm_visible.clone(),
);

let root = ZStack::new()
    .add(Box::new(split))
    .add(Box::new(accent_modal))
    .add(Box::new(reset_modal));
app.run(Box::new(root))
```

ZStack 不在の場合は HStack を流用 or paint_overlay 経路で済ます (= AlertDialog
自身が paint_overlay で dim backdrop + center panel を描く想定)。

### 5.2 focus capture

modal visible 時、 背後 widget へ event を届けないのは AlertDialogContainer の
event() で `if !visible { Ignored } else { ... }` の早期 return + visible 時は
`Handled` 返却 (= 親側 dispatch で兄弟 widget に伝播しない) で実現。 ZStack
が「最上位 child が Handled なら兄弟 skip」の dispatch 規約を持つ前提。

### 5.3 外クリック dismiss

AlertDialog 既存実装で実装されていない場合、 AlertDialogContainer の event()
で `PointerPress { x, y }` を受領、 inner.last_rect で hit-test、 外側なら
`visible_state.set(false)` で dismiss。 (Escape key dismiss と同 path)

## 6. test exercisability

### 6.1 visible toggle → paint 呼出可能性

```rust
#[test]
fn alert_dialog_container_shows_on_state_true() {
    let runtime = ReactiveRuntime::new();
    let state = runtime.create_state(false);
    let inner = AlertDialog::new("title", "msg")
        .with_kind(DialogKind::Question);
    let mut container = alert_dialog_container(inner, state.clone());

    // baseline: hidden = layout zero-size、 paint no-op
    let mut buf = vec![0u8; 800 * 600 * 4];
    let mut renderer = cpu_renderer(&mut buf, 800, 600);
    let size = container.layout(&Constraints::tight(800.0, 600.0));
    assert_eq!(size, Size::zero());

    // act: state.set(true) → 次 paint で show() forward + paint 実行
    state.set(true);
    let size2 = container.layout(&Constraints::tight(800.0, 600.0));
    assert!(size2.width > 0.0 && size2.height > 0.0);
    container.paint(&mut renderer, ItemRect::new(0.0, 0.0, 800.0, 600.0));
    // inner.visible() == true を 観測可能なら assert
}
```

### 6.2 既 land 7 test との関係

既 land logic-only subset の 7 test は visual 部分に依存せず、 state.set/get
+ pub(crate) helper invoke で完結。 視覚 wire 追加後も全 7 test は無修正で
green 維持 (= 視覚 layer は logic layer の上に積層、 break しない設計)。

新規追加 test (= 視覚 wire):
- alert_dialog_container_shows_on_state_true (= ↑)
- alert_dialog_container_hides_on_state_false (= 逆経路)
- container_escape_key_dismisses (= Escape → visible false)
- container_outside_click_dismisses (= 外クリック → visible false)
- preview_label_updates_on_hex_input (= on_change → preview text 変化)
- wcag_label_updates_on_hex_input (= on_change → WCAG ratio 再計算)
- apply_uses_draft_hex_not_default (= Apply → draft_accent_hex 経由で commit)

期待 net 結果: 既存 54 passed + 新規 7-10 視覚 test = 61-64 passed 想定。

## 7. follow-up commit 順序

framework PR (= GUI_kit-track-framework-extend) land trigger 受信後、 本
branch (track2/phase2-wave3-modal-lifecycle) 上で以下順次 commit:

1. `chore(deps): bump hayate-kit to framework PR sha`
   (= Cargo.toml 内 path dep 切替 or upstream main update reflect)
2. `feat(state): add draft_accent_hex State<String> for accent picker live preview`
   (= state.rs に field 追加 + new() initial + tests)
3. `feat(modal): AlertDialogContainer custom Widget — State<bool> external control`
   (= modals.rs に struct + impl Widget + alert_dialog_container helper +
   LabelRef shim + 視覚 test)
4. `feat(modal): dynamic preview + WCAG ratio re-eval on TextInput on_change`
   (= modals.rs build_accent_picker 内で Rc<RefCell<LabelWidget>> 経由配線、
   on_change callback で再計算、 関連 test)
5. `feat(main): mount accent_picker + reset_confirm modals in root ZStack`
   (= main.rs root を SplitView 単体 → ZStack + 2 modal に変更、 layout test)
6. `docs: wave3b-modal-visual-design.md 同梱` (= 本 doc を docs/ に commit)
7. PR raise (= full scope: logic-only subset commit baca8fc + 1〜6 の visual
   layer + design doc を 1 PR で codex 査読)

各 commit は cargo check -j1 / cargo test --bins -j1 / cargo clippy
--all-targets -j1 全 green を維持。

## 8. risk + 不確実性

- **framework PR scope drift**: TextInput::on_change callback signature が
  想定 (`impl FnMut(&str) + 'static`) と異なる場合、 4.1 closure を再書き換え
- **ZStack 不在**: 既存 hayate-kit に ZStack widget が無い場合、 paint_overlay
  経路のみで modal を実装するか別 widget (= OverlayContainer 等) を framework
  PR で同時追加するか boss1 相談
- **AlertDialog 内蔵 dismiss 経路と外部 State 競合**: AlertDialog 自身が
  on_result callback 内で hide() を呼ぶ実装の場合、 last_observed_visible が
  外部 State と乖離する potential。 解決 = on_result callback で外部 State も
  set(false) する規約を modals.rs side で enforce
- **focus capture 仕様未確定**: modal visible 時、 textInput focus が背後の
  search bar / detail pane field と競合しないこと要 testing。 framework PR
  で focus stack push/pop API が増える可能性

## 9. 完了基準 (= 最終 PR raise 時)

> v0.1 元基準。 v0.2 で実装 path が確定した完了基準は §10.4 を参照。

- [ ] commit `baca8fc` (= 本 logic-only subset) preserved
- [ ] framework PR land + Cargo.toml bump
- [ ] state.rs: draft_accent_hex 追加 + tests
- [ ] modals.rs: AlertDialogContainer + LabelRef shim + 動的 preview/WCAG
- [ ] main.rs: ZStack or paint_overlay 経路で 2 modal mount
- [ ] docs/wave3b-modal-visual-design.md 同梱 commit
- [ ] cargo check -j1 / cargo test --bins -j1 (= 61-64 passed 想定) /
      cargo clippy --all-targets -j1 全 green
- [ ] PR raise + boss1 ack

## 10. Revision history (v0.1 → v0.2) — implementation 着手で確定した補正

本章は v0.1 doc の主要 design 判断点で、 worker2 実装着手前後の grep + framework
PR #155 land + PRESIDENT 即決 trigger 経由で revise された点を網羅。 後世が
「なぜ v0.1 通りに実装されなかったか」を re-discover 可能化する責務 (=
workflow pattern 4 適用、 [[feedback_president_dispatch_pace]] + [[feedback_root_cause_over_quick_fix]] 整合)。

### 10.1 Prerequisite framework PR land (= 副次 finding 反映)

**v0.1**: prerequisite = TextInput::on_change + hayate-kit 経由 `Renderer` /
`ItemRect` / `TextEngine` / `alloc_widget_id` / `PopupId` re-export 計 5 件

**v0.2 確定**:
- PR #155 land 済 = `TextInputWidget::on_change` builder + `Renderer` /
  `TextEngine` / `ItemRect` re-export 3 件追加 ✅
- `alloc_widget_id` 不要化 (= worker2 case は wrapper widget の id() を
  `inner.id()` delegate で代替、 stable inner ゆえ alloc 不要)
- `PopupId` 不要化 (= ReactiveOverlayContainer は popup emitter ではない、
  Widget trait default impl 経由で OverlayContainer に forward 委譲)

**revision 根拠**: worker3 dispatch との distinction (= PRESIDENT 補強で明文化)
= worker3 DetailContainerWidget は inner rebuild が前提で inner.id() 不安定、
別途 framework PR #156 で alloc_widget_id 解禁が必要。 worker2 は inner stable
ゆえ delegate で済む。

### 10.2 AlertDialogContainer → ReactiveOverlayContainer rename + 設計大幅 simplify

**v0.1 §2**: `AlertDialogContainer` = AlertDialog を内部に持つ thin wrapper、
visible_state 観測 + show/hide forward + paint_overlay forward を担当

**v0.2 確定**: `ReactiveOverlayContainer` = `hayate_kit::widget::overlay::OverlayContainer`
を内部に持つ thin wrapper、 N 個の (overlay_id, State<bool>, last_observed,
on_enter) bindings 経由で `show_overlay` / `hide_overlay` を forward

**revision 根拠** (PRESIDENT 即決 Option A):
- v0.1 §3 で議論された ZStack 不在問題は `hayate_kit::widget::overlay::OverlayContainer`
  既存利用で完全解消 (= base widget + Vec<overlay widgets> + dimming + modal
  event routing 既実装、 695 行)
- paint_overlay 経路 / 親 paint 末尾 後段 描画 の 2 候補多択も解消 (= 第 3 の
  解 = OverlayContainer 採用)
- N modal 集約 (1 wrapper で複数 modal) → app builder の組み合わせ柔軟性向上
- framework PR 不要、 wrapper code ~210 行に圧縮

### 10.3 reset_confirm AlertDialog 採用 → VStack 統一 (v0.1 → v0.1.5 → v0.2)

**v0.1**: reset_confirm 設計言及なし (= accent_picker のみ詳述)

**v0.1.5** (= PRESIDENT 即決 Option A item 5 中間採択): reset_confirm =
AlertDialog 直接利用 (= title + message + Cancel/Reset buttons で perfect fit、
DialogKind::Question + on_result callback で State<bool> sync)

**v0.2 確定** (= PRESIDENT 即決 Option β): reset_confirm = VStack 統一
(= accent_picker と同 pattern: title Label + message Label + HStack { Cancel + Reset })

**revision 根拠** (= worker2 Step 5 着手前 grep で 2 件 coordination 問題発見):
1. **dual-visibility-source coordination 不可**:
   - AlertDialog は内部 self.visible flag を保持 (alert_dialog.rs:104) + show/hide/visible (line 174/180/185)
   - ReactiveOverlayContainer は OverlayContainer.entry.visible を State<bool> で駆動
   - 2 visibility source 同期手段なし: `Box<dyn Widget>` 経由で AlertDialog の
     `show()` inherent method は trait object 化により消滅、 外部から reach 不可
   - 解消には StateDrivenAlertDialog wrapper widget 追加が必要 = consumer 側
     band-aid、 [[feedback_platform_principle]] 緊張
2. **double-dimming 視覚問題**:
   - OverlayContainer.paint(): DIM_COLOR = [0,0,0,128] = alpha 128 dimming (overlay.rs:269-276)
   - AlertDialog.paint(): parent_w/parent_h 全域に alpha=128 dimming (alert_dialog.rs:262-263)
   - 重ね合わせ合成 alpha ≈ 192 (= 1 - (1-128/255)² ≈ 0.748)
   - accent_picker (= VStack content) と reset_confirm (= AlertDialog) で
     dimming alpha が非対称 → UI 一貫性損失

**v0.2 採用の effect**:
- 機能 lose ゼロ: AlertDialog 内蔵 Escape/Enter dismiss は ReactiveOverlayContainer
  の Escape pre-intercept + Enter accept で代替済 (= §10.5 詳細)
- 視覚一貫: 2 modal 共 alpha 128 dimming で対称
- 構造 cohesion: 2 modal が VStack { title + content + HStack { buttons } } 同 pattern
- 実装コスト最小: wrapper widget 不要、 build_reset_confirm 軽微 refactor (= title Label 追加のみ)
- ETA 維持: 元 120 min 想定通り

**棄却 option**:
- Option α (StateDrivenAlertDialog wrapper 追加): consumer 側 band-aid、 double-dimming 残置
- Option γ (OverlayContainer has_dimming flag 追加 framework PR): wave 3b scope 外、 ETA 延長

### 10.4 完了基準 v0.2 (= §9 v0.1 基準の置換)

- [x] commit `baca8fc` (= logic-only subset) preserved → HEAD lineage 維持
- [x] framework PR #155 land 済 (= path dep ゆえ Cargo.toml bump 不要)
- [x] state.rs: draft_accent_hex 追加 + 3 tests (Step 2)
- [x] modals.rs: **ReactiveOverlayContainer** + LabelRef shim + 8 tests (Step 3)
- [x] modals.rs: 動的 preview + WCAG ratio on TextInput::on_change + 8 tests (Step 4)
- [x] modals.rs: build_reset_confirm VStack 統一 + ReactiveOverlayContainer Enter accept + 4 tests (Step 5)
- [x] main.rs: ReactiveOverlayContainer { OverlayContainer { base: SplitView, overlays: [accent, reset] } } root mount (Step 5)
- [x] docs/wave3b-modal-visual-design.md revision v0.1 → v0.2 embed (Step 6 = 本 commit)
- [ ] cargo test --package hayate-kit-settings -j1 全 green (= 54 baseline + 23 視覚層 = 77 passed 想定)
- [ ] cargo clippy --all-targets -j1 全 green
- [ ] PR raise + codex 査読 + PRESIDENT 直接 merge

### 10.5 採用 dismiss / accept 経路 (= §2.2 + §5.2 + §5.3 の v0.2 置換)

- **Escape dismiss** = ReactiveOverlayContainer.event() 内 keysym `xkbcommon::xkb::Keysym::Escape`
  pre-intercept → 最上位 visible binding の State<bool>.set(false) + inner.hide_overlay
  - v0.1 doc 内 `hayate_kit::Key::Escape` 言及は誤り (= 該当 path 不存在、 grep 確定)
  - 採用 path: `WidgetEvent::Key(KeyEvent { keysym, .. })` で `keysym: xkbcommon::xkb::Keysym`
    field access、 hayate-kit 経由再 export 不在のため `xkbcommon` を Cargo.toml に
    直 dep 追加 (= third-party crate ゆえ feedback_new_apps_depend_on_gui_kit_only 規範違反なし)
- **Enter accept** = `Keysym::Return` / `Keysym::KP_Enter` pre-intercept →
  最上位 visible binding に on_enter closure registered なら invoke (= Ok 系
  button click と同等 action: accent は apply_accent_picker、 reset は apply_reset_confirm)、
  closure 不在なら dismiss fallback
- **外クリック dismiss** = **A2 で実装済** (wave 3b/3c で連続 defer の後)。
  GUI_kit framework method `OverlayContainer::point_outside_visible_overlays`
  (= PR #159、 overlay rect hit-test) land → hayate-kit-settings 側 event() に
  `PointerPress` pre-intercept 配線 (= has_visible_overlay() gate →
  point_outside_visible_overlays(x,y) で backdrop click 判定 → dismiss_topmost_visible)。
  座標 (x,y) は widget-local = container-local (root 近傍 wrap、 translate なし)。
- **focus capture / Tab trap** = wave 3b 範囲 **外** (= 確定方針)、 worker2
  既明記、 closeout phase で再評価予定。

### 10.6 副次 finding (= 実装着手で発覚した framework gap、 framework PR 不要)

- `hayate_kit::widget::widget_id::alloc_widget_id` 不在 (= hayate-platform 側のみ
  実装、 hayate-kit 経由 re-export なし)
  - 回避策: wrapper widget の id() は inner.id() delegate で代替、 framework PR 不要
- `hayate_platform::platform::keyboard::{KeyEvent, KeyState, Modifiers}` 再 export 不在
  - 影響: unit test 内 `WidgetEvent::Key` 構築不可 (= KeyEvent struct field access 要求)
  - 回避策: dismiss_topmost_visible / accept_topmost_visible を pub(crate) fn
    抽出、 key event 経路を経由せず直接 logic 呼出で test 可能化
  - 残課題: 真の event-driven integration test は workspace level (= 統合
    test crate) で実施推奨、 wave 3b scope 外
- `hayate_platform::widget_themes::app::AppTheme` 再 export 不在
  - 影響: LabelRef shim の inject_theme 完全 forward 不能
  - 回避策: inject_theme default impl (= children_mut() empty で no-op) で十分
    (= hayate-kit-settings は HAYATE_ORIGINAL theme + cosmic-text path 使用、
    bitmap_default 未注入でも paint 正常)

## 11. accent_picker modal trigger 配線状態 (= wave 3c dispatch 予定 dormant infrastructure)

wave 3b で land した modal infrastructure (= AlertDialogContainer 置換の
ReactiveOverlayContainer + accent_picker / reset_confirm overlay 配置 + Escape /
Enter / on_enter closure + OverlayContainer 経由 visibility + dimming) は
**production code path から trigger されない dormant state** で land する。

- **accent_picker modal trigger** (= `state.accent_picker_visible.set(true)`):
  本 wave 3b では production code 内に呼出 site なし。 既存 inline TextInput
  accent_hex 編集 path (= `sections/appearance.rs:84-89` 周辺) で wave 3b 基本
  編集 path は充足するため、 modal trigger は visual color picker (= swatch
  preview + WCAG live re-eval) 用途で別 dispatch (= wave 3c) で配線予定。
- **reset_confirm modal trigger** (= `state.reset_confirm_visible.set(true)`):
  同じく wave 3c dispatch 予定。 wave 3b では advanced section の reset button
  callback wire を含めず、 modal 表示 trigger 未配線。
- **test coverage 確保**: trigger 未配線でも infrastructure 自体は cargo test
  77 + 2 (= 79) で全 path 検証済。 wave 3c で trigger 配線時に既存 test 不変
  維持で integration verify 可能 (= contract preserved)。
- **rationale**: wave 3b dispatch ETA 圧縮 (= ETA 元 120 min 維持)、 trigger
  配線 + visual picker UX 整備 + section button 周辺 affordance は wave 3c で
  集中 dispatch、 wave 3b は infrastructure land + 動的 preview/WCAG/Escape/Enter
  validation までを scope 確定。
- **後世への trail**: 「なぜ modal infrastructure が land 済なのに使えないか」
  と疑問を持った後世 reader は、 本 §11 + git log で wave 3c dispatch 履歴を
  参照のこと (= dormant 期間中の re-discover 可能化)。
