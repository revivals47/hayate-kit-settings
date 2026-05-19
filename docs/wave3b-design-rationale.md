# Phase 2 wave 3b — TreeView selection + Search filter reactive bind design rationale

worker3 自律判断記録 (= boss1 dispatch 指示「dynamic rebuild vs Derived Widget Box の評価軸 + 採用根拠を PR description に明記」)。本書は最終 PR raise 時に PR description に同梱予定の評価表 + 採用根拠 + DTP roadmap 整合性論。framework PR (= `TextInput::on_change` + `Renderer` / `TextEngine` / `ItemRect` re-export、boss1 raise 中) land 後に custom Widget impl 着手時の reference として保持。

## 問題定義

wave 3b の核心要件は **2 つの reactive bind**:

1. **TreeView on_select → `state.selected_section.set(SectionId)`** 経由で TreeView nav 選択 → detail pane swap
2. **Search bar TextInput change → `state.search_query.set(query)` + `filter_sections(query).first()` で auto-select** で検索クエリ → 最初のマッチ section に自動遷移

(1) は `TreeViewWidget::on_select` builder method が既存で、closure 内で `state.selected_section.set(...)` を呼ぶだけ — framework PR と無関係に既存 API のみで配線可能。

(2) は `TextInputWidget` に `on_change` callback が無く、`take_changed()` / `text()` を poll する必要がある。poll site は widget tree 内 (= `event()` callback の後) しか無いため、`TextInputWidget` を内包する **custom wrapper Widget** (= `SearchBarWidget`) を起こす必要が発生。

更に、(1) で `state.selected_section` が変化したことを検出して detail pane (= `Box<dyn Widget>`) を rebuild する仕掛けも widget tree 内に必要 (= `DetailContainerWidget`)。

これにより wave 3b は **2 個の custom Widget impl** を要請する初の wave となり、framework gap (= `Renderer` / `ItemRect` / `TextEngine` が `hayate_kit` に re-export 漏れ) が露呈、PRESIDENT Path Z1 (= framework root-fix) に至った経緯がある。

framework PR land 後の選択肢は以下の **2 設計 pattern**:

- **Pattern A**: dynamic rebuild via cached-section comparison in `layout()` / `event()`
- **Pattern B**: Derived Widget Box (= `Derived<...>` で widget instance を tracking、State 変化で自動 recompute)

以下、worker3 が dispatch 内で「設計判断は worker3 自律」の指示に従い 5 評価軸で比較した結果を記録する。

## 5 評価軸での比較

| # | 評価軸 | Pattern A (dynamic rebuild) | Pattern B (Derived Widget Box) |
|---|---|---|---|
| 1 | parent ownership / inner widget lifecycle 制御 | container が `Box<dyn Widget>` を直接所有、SectionId 変化検出時に `build_detail` で新 Box 作成 + 旧 Box は scope 切れで drop = 明示的所有権 transfer、 lifecycle 明確 | `Derived<T>` は `T: 'static` 要求かつ `Clone` 暗黙 (= `Derived::get()` は `Ref<'_, T>` を返す = internal `RefCell` 経由)、`Box<dyn Widget>` は `Clone` 不可 = そのままでは `Derived<Box<dyn Widget>>` 構築不可、`Rc<RefCell<Box<dyn Widget>>>` 等の追加層必要、ownership 不透明化 |
| 2 | selected_section 変化時の re-layout cost | container が `version` を cache、変化検出時のみ `build_detail` 呼び (= 1 frame 1 回まで)、後続 frame は cached widget paint のみ = O(1) | `Derived::get()` は internal version check で `recompute` 呼出、 ただし `recompute` 内で `build_detail` 呼ぶ場合 cost は同等。`Derived` 経由でも結局 lazy rebuild、利点は薄い |
| 3 | reactive subscription source | container 自身が `selected_section: State<SectionId>` clone を hold、`event()` / `layout()` 内で `state.selected_section.get()` を直接 read、 version 比較 = 明示的 polling pattern、Effect/listener 登録不要 | `Derived::map(&state.selected_section, |&sid| build_detail(sid, ...))` で declarative、 ただし `build_detail` は `&'static Strings` + `&AppStateHandles` を closure capture 要、`Derived` ctor signature が制約。 さらに widget tree が `Rc<Derived<Box<dyn Widget>>>` を hold する仕掛けが必要で抽象化 cost が高い |
| 4 | test exercisability (= unit test で section 切替を simulate 可能) | `let mut c = DetailContainerWidget::new(...); c.layout(&Constraints::tight(100,100)); state.selected_section.set(Advanced); c.layout(&...); assert!(c.cached_section() == Advanced);` のように直接 read-back 可能、 cache 露出で test 書きやすい | `Derived` の `get()` は `Ref<'_, T>` で `T: PartialEq` 比較不可 (= `Box<dyn Widget>` is not `PartialEq`)、test は indirect proxy (= 副作用観察) で書く必要、 unit test cost が増す |
| 5 | DTP app 等の長期 north star との compatibility (= 縦書き text_core が将来 require する pattern) | DTP app の per-paragraph dynamic widget tree (= ruby annotation toggle / vertical-writing mode switch 等) は **同じ cached-comparison + manual rebuild pattern** で対応可、universal building block。container 内 `Vec<Box<dyn Widget>>` で多 panel も extend 自然 | Derived chain は declarative で美しいが、 DTP app の複数 State (= text content + style + ruby + layout) を AND した composite Derived は依存 graph が爆発、 debug 困難。Pattern A の明示 polling の方が long-term maintainability ◯ |

### 比較サマリー

|| Pattern A | Pattern B |
|---|---|---|
| 軸 1 ownership | ◎ | △ |
| 軸 2 cost | ○ | ○ |
| 軸 3 subscription | ○ | △ (抽象化 cost) |
| 軸 4 testability | ◎ | △ |
| 軸 5 DTP 整合 | ◎ | △ |

**Pattern A 採用、5 軸中 4 軸で優位、cost 軸のみ同等**。

## 採用根拠 (= worker3 自律判断、 PRESIDENT validation 推奨)

Pattern A (dynamic rebuild via cached comparison) を採用する具体根拠 3 点:

1. **framework PR の re-export だけで実装可能** — `Renderer` / `ItemRect` / `TextEngine` を `hayate_kit` 経由で得れば custom `impl Widget` が書ける。Pattern B のように `Derived<Box<dyn Widget>>` 構築のための `Rc<RefCell<Box<dyn Widget>>>` 追加抽象が不要 = framework PR scope を最小化、 boss1 の framework PR diff も最小に保てる ([[feedback_root_cause_over_quick_fix]] と整合、最小限の root-fix で同じ目的達成)
2. **test 容易性** — `DetailContainerWidget::cached_section()` getter を pub(crate) で expose、`assert_eq!(container.cached_section(), SectionId::Advanced)` のような直接 read-back test を書ける。Pattern B の indirect proxy 検証 (= renderer mock + paint side-effect 観察) は test infra cost が桁違いに大きく、 wave 3b unit test scope を逸脱
3. **DTP app north star 整合性** — DTP app は per-paragraph dynamic widget tree が要件 (例: ruby annotation toggle / vertical-writing mode switch / per-style sub-tree)、これらは全て「ある State の値が変わったら、 該当 sub-tree を rebuild」 という同 pattern。Pattern A を本 wave 3b で確立すれば DTP app に **そのまま reuse 可能な container widget primitive** として再利用できる ([[project_dtp_app_roadmap]] handoff section I 整合)

## SearchBarWidget 設計概要

```text
SearchBarWidget {
    inner: TextInputWidget,             // owned by value (concrete type)、
                                        // take_changed() / text() に直接 access
    state: AppStateHandles,             // clone, for search_query.set / selected_section.set
    strings: &'static Strings,          // filter_sections への引数
}

impl Widget for SearchBarWidget {
    fn layout(c) -> Size { self.inner.layout(c) }
    fn paint(r, rect) { self.inner.paint(r, rect) }
    fn paint_overlay(r) { self.inner.paint_overlay(r) }  // IME candidate 等の overlay 維持
    fn event(e) -> EventResponse {
        let resp = self.inner.event(e);
        if self.inner.take_changed() {                   // poll change flag
            let text = self.inner.text().to_string();
            self.state.search_query.set(text.clone());   // reactive: query state
            let matches = filter_sections(&text, self.strings);
            if let Some(&first) = matches.first() {
                self.state.selected_section.set(first);  // reactive: auto-select first match
            }
        }
        resp
    }
    fn event_overlay(e) { self.inner.event_overlay(e) }
    fn inject_engine(eng) { self.inner.inject_engine(eng) }
    // inject_theme は override せず default forward (= children_mut() 経由 / empty slice)、
    // TextInputWidget は active_theme() fallback で HAYATE_ORIGINAL を直接 read 可
    fn focusable() -> bool { self.inner.focusable() }
    fn id() -> WidgetId { self.inner.id() }
    fn dirty() -> bool { self.inner.dirty() }
    fn clear_dirty() { self.inner.clear_dirty() }
    fn update(dt) { self.inner.update(dt) }
    fn accessible() { self.inner.accessible() }
    fn last_rect() { self.inner.last_rect() }
}
```

### framework PR `TextInput::on_change` callback 採用時の simplification

PRESIDENT Z1 の framework PR で `TextInput::on_change(|text|)` builder が追加されれば、 上記 `event()` 内の poll は不要、 `TextInputWidget::new().on_change(move |text| { state.search_query.set(...); ... })` で declarative bind 可能。 その場合 SearchBarWidget は wrapper 不要、 `search::build()` 内で直接 builder chain で完結。 framework PR diff が `on_change` callback 入りなら本 doc の Pattern A custom wrapper は SearchBarWidget portion のみ不要化、 DetailContainerWidget portion は引き続き Pattern A (= TextInputWidget と異なり TreeView は既に on_select callback 持ち、 detail pane の cached comparison は別問題で残る)。

worker3 は framework PR diff を確認後に SearchBarWidget vs `on_change` simplification の最終判断する。

## DetailContainerWidget 設計概要

```text
DetailContainerWidget {
    cached_section: SectionId,                    // 直近 rebuild した時の section
    cached_state_version: u64,                    // selected_section.version() snapshot
    inner: Box<dyn Widget>,                       // 現 section の widget tree
    cached_engine: Option<Rc<RefCell<TextEngine>>>,  // inject_engine 後の re-inject 用
    strings: &'static Strings,
    state: AppStateHandles,
}

impl DetailContainerWidget {
    pub fn new(strings, state) -> Self {
        let initial = SectionId::default();
        let inner = build_detail(strings, initial, &state);
        Self {
            cached_section: initial,
            cached_state_version: state.selected_section.version(),
            inner, cached_engine: None,
            strings, state,
        }
    }
    fn rebuild_if_changed(&mut self) {
        let v = self.state.selected_section.version();
        if v != self.cached_state_version {
            let sid = *self.state.selected_section.get();
            self.inner = build_detail(self.strings, sid, &self.state);
            if let Some(eng) = &self.cached_engine {
                self.inner.inject_engine(Rc::clone(eng));     // re-inject engine
            }
            self.cached_section = sid;
            self.cached_state_version = v;
        }
    }
    #[cfg(test)] pub(crate) fn cached_section(&self) -> SectionId { self.cached_section }
}

impl Widget for DetailContainerWidget {
    fn layout(c) -> Size { self.rebuild_if_changed(); self.inner.layout(c) }
    fn paint(r, rect) { self.inner.paint(r, rect) }
    fn paint_overlay(r) { self.inner.paint_overlay(r) }
    fn event(e) { let r = self.inner.event(e); self.rebuild_if_changed(); r }
    fn inject_engine(eng) {
        self.cached_engine = Some(Rc::clone(&eng));
        self.inner.inject_engine(eng);
    }
    // 他 method は default forward via children_mut() ... ただし children_mut は
    // &mut [Box<dyn Widget>] なので self.inner を slice 化する必要、
    // 実装時 std::slice::from_mut(&mut self.inner) を試行
}
```

### `rebuild_if_changed` の呼出 timing

- `layout()` 冒頭 = repaint 前の widget tree 構造確定 timing、 layout 計算前に最新 inner を確保
- `event()` 末尾 = event 内で state.selected_section.set が起きた場合の同 frame 反映、 次 paint で stale inner を見ない保証

`update(dt)` / `paint(r, rect)` 冒頭でも rebuild check したいが、`paint` は immutable borrow 期待 = `&mut self` の rebuild は呼べる。一方 `layout` + `event` 両 hook で覆えば実用上 sufficient。

### engine re-injection の理由

`inject_engine` は App から widget tree に 1 回だけ呼ばれる初期化 hook。 rebuild 後の新 inner は engine 未注入状態で生成されるため、 cached engine を保持して `rebuild_if_changed` 内で再 `inject_engine` 呼出する。

`inject_theme` は同様に rebuild 後の inner に到達しないが、 R13 systemic fix 後の widget 群は `active_theme()` 経由 default で HAYATE_ORIGINAL に reach 可能、 明示 inject 不要 (= caller `.theme()` override も section 側で行っていない)。

## Test 計画

framework PR land 後の wave 3b 完成時に追加予定の unit test:

- `sections::tests::from_tree_path_*` (= 既 land、 6 test)
- `search::tests::search_bar_widget_text_change_triggers_state_updates` (= TextInputWidget の `set_text` で simulate → SearchBarWidget の event 経由 polling → state.search_query が新 query 反映 + state.selected_section が first match に変化)
- `search::tests::search_bar_widget_empty_query_does_not_change_selection` (= empty query で filter_sections 全件返却、 first match = General は保持 / 元のままで何も変化なし behavior 確認)
- `main::tests::detail_container_rebuild_on_section_change` (= DetailContainerWidget の cached_section が state.selected_section.set 経由で同期する read-back 検証)
- `main::tests::detail_container_engine_reinjection` (= rebuild 後 inner が engine 持つことを accessible() side-effect 等で観察、 spike test として PR 中議論)

## 残課題 / 開発時 risk

- **TextInputWidget の `inject_theme` 未到達** — wave 3b 開始時点で TextInputWidget は default impl で受け取れず、 active_theme() fallback。 visual で HAYATE_ORIGINAL の input theme が反映されない場合 (= R13 fix が input.rs に未到達)、 visual smoke で発覚、 codex 査読時に framework root-fix で対応提案。 暫定 risk と把握、 wave 3b scope outside
- **children_mut() forward の限界** — `inner: TextInputWidget` (concrete) は `children_mut()` 経由 forward 不可、 全 callback (paint_overlay / event_overlay / inject_engine / accessible / focusable / id / dirty / clear_dirty / update / last_rect) を明示 forward 必要。 boilerplate 増、 ただし single-inner wrapper なので manageable
- **PopupId 等の hayate_platform 内型** — `paint_popup(id: PopupId)` の forward 等で PopupId が必要、 framework PR で `hayate_kit::PopupId` 公開推奨。 boss1 framework PR diff 確認時に確認、 不足あれば second framework PR or fallback (= paint_popup 未 override、 default no-op)

## DTP roadmap reuse 整合 ([[project_dtp_app_roadmap]] handoff section I)

本 wave 3b の Pattern A 確立で得られる primitive:

1. **`DetailContainerWidget` 一般化** — 「ある State<T> の値変化で内部 widget tree を rebuild する container」 という universal primitive、 DTP app の per-paragraph reactive panel に直接 reuse 可
2. **`SearchBarWidget` 一般化** — 「TextInput の poll-based change → State<T> set」 wrapper、 DTP app の typography search / glyph search 等に同 pattern reuse 可
3. **`SectionId::from_tree_path` 一般化** — hierarchical TreeView path → enum mapping、 DTP app の per-style hierarchical nav に同 pattern reuse 可

これら 3 primitive は wave 3b 終了後に `feedback_dtp_reactive_primitives_reuse` として memory 化を検討 (= worker3 standby 時に PRESIDENT 上申)。

---

worker3 自律設計判断 record。最終 PR raise 時に PR description に同梱予定 (= 全文 or 要約形)。
