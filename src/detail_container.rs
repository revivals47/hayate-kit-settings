//! Detail pane container with reactive rebuild on `SectionId` change
//! (= Phase 2 wave 3b、 worker3 dispatch、 dynamic rebuild path)。
//!
//! `SplitView` 右ペインに配置、 TreeView nav の選択変化 (= `state.selected_section`)
//! を `version` snapshot で polling 検出、 変化時に `build` closure を呼び直して
//! 内部 widget tree を atomic 入替する custom container。
//!
//! 設計判断の根拠 + Pattern A 採用理由 + 5 軸評価表 + DTP reuse 整合は
//! `docs/wave3b-design-rationale.md` (= 同 branch land 済 197 行 8 章)。
//! 本 file は同 doc の §「DetailContainerWidget 設計概要」 section の実装。
//!
//! ## rebuild timing
//! - `layout()` 冒頭 = repaint 前の widget tree 構造確定 timing
//! - `event()` 末尾 = event handler が `state.selected_section.set` を呼んだ
//!   場合の同 frame 反映、 次 paint で stale inner を見ない保証
//!
//! ## engine re-injection
//! `inject_engine` は App から widget tree に 1 回だけ呼ばれる初期化 hook、
//! rebuild 後の新 inner は engine 未注入状態で生成される。 そのため
//! `cached_engine` で保持して `rebuild_if_changed` 内で再注入する。
//!
//! ## DTP reuse
//! 「ある State<T> の値が変わったら 該当 sub-tree を rebuild」 という universal
//! primitive。 DTP app の per-paragraph reactive panel (= ruby annotation toggle
//! / vertical writing mode switch 等) に直接 reuse 想定
//! ([[project_dtp_app_roadmap]] handoff section I 整合)。

use std::cell::RefCell;
use std::rc::Rc;

use hayate_kit::{
    alloc_widget_id, Constraints, EventResponse, ItemRect, Renderer, Size, TextEngine, Widget,
    WidgetEvent, WidgetId,
};

use crate::sections::SectionId;
use crate::state::AppStateHandles;

/// Closure type for building the detail widget tree for a given section。
/// `'static` は `Box<dyn Fn>` の object-safety 要件、`Fn`(非 `FnMut`) なので
/// `rebuild_if_changed` 内の split borrow (= `&self.build` と `&mut self.inner`)
/// が成立する。
type BuildDetailFn = Box<dyn Fn(SectionId, &AppStateHandles) -> Box<dyn Widget>>;

/// Container widget that swaps its inner widget tree when
/// `state.selected_section` changes。
pub struct DetailContainerWidget {
    /// 直近 rebuild した時の section (= read-back test 用、 paint には未使用)。
    cached_section: SectionId,
    /// `state.selected_section.version()` snapshot、 polling-based change detection の比較値。
    cached_state_version: u64,
    /// 現在 section 用の widget tree。 rebuild 時に新 Box に差し替わる。
    inner: Box<dyn Widget>,
    /// `inject_engine` で受け取った engine の保持、 rebuild 後 inner への再注入用。
    cached_engine: Option<Rc<RefCell<TextEngine>>>,
    /// AppStateHandles clone (= 各 section / search / modal と共有)。
    state: AppStateHandles,
    /// section → widget tree 構築 closure (= main.rs 側で strings capture)。
    build: BuildDetailFn,
    /// Process-unique stable id (= `alloc_widget_id` 経由、 rebuild で変動しない)。
    id: WidgetId,
}

impl DetailContainerWidget {
    /// Construct a container with the initial section read from
    /// `state.selected_section`。
    ///
    /// `build` closure は section → `Box<dyn Widget>` を返す factory、
    /// caller 側で `strings: &'static Strings` を capture する形を想定
    /// (= `move |sid, state| build_detail(strings, sid, state)`)。
    pub fn new(
        state: AppStateHandles,
        build: impl Fn(SectionId, &AppStateHandles) -> Box<dyn Widget> + 'static,
    ) -> Self {
        let initial = *state.selected_section.get();
        let cached_state_version = state.selected_section.version();
        let inner = build(initial, &state);
        Self {
            cached_section: initial,
            cached_state_version,
            inner,
            cached_engine: None,
            state,
            build: Box::new(build),
            id: alloc_widget_id(),
        }
    }

    /// Rebuild `inner` if `state.selected_section.version()` has advanced
    /// since the last rebuild。 `version` 比較で「同 SectionId への再代入」
    /// (= no-op) も rebuild trigger するが、 wave 3b では問題視せず
    /// (= TreeView on_select callback は実際に違う path 押下時のみ発火)。
    fn rebuild_if_changed(&mut self) {
        let v = self.state.selected_section.version();
        if v == self.cached_state_version {
            return;
        }
        let sid = *self.state.selected_section.get();
        let new_inner = (self.build)(sid, &self.state);
        self.inner = new_inner;
        if let Some(eng) = self.cached_engine.as_ref() {
            self.inner.inject_engine(Rc::clone(eng));
        }
        self.cached_section = sid;
        self.cached_state_version = v;
    }

    /// Test-only read-back of the section currently held in `inner`。
    /// `assert_eq!(container.cached_section(), SectionId::Advanced)` 形式の
    /// 直接 read-back 検証を可能にする。
    #[cfg(test)]
    pub(crate) fn cached_section(&self) -> SectionId {
        self.cached_section
    }
}

impl Widget for DetailContainerWidget {
    fn layout(&mut self, constraints: &Constraints) -> Size {
        self.rebuild_if_changed();
        self.inner.layout(constraints)
    }

    fn paint(&mut self, renderer: &mut Renderer, rect: ItemRect) {
        self.inner.paint(renderer, rect);
    }

    fn event(&mut self, event: &WidgetEvent) -> EventResponse {
        let resp = self.inner.event(event);
        // event handler が state.selected_section.set を呼んだ場合、 同 frame の
        // 次 paint で stale inner を見ないよう即座に反映する。
        self.rebuild_if_changed();
        resp
    }

    fn inject_engine(&mut self, engine: Rc<RefCell<TextEngine>>) {
        // rebuild 後の新 inner にも engine を注入できるよう保持。
        self.cached_engine = Some(Rc::clone(&engine));
        self.inner.inject_engine(engine);
    }

    fn id(&self) -> WidgetId {
        self.id
    }

    /// 単一 inner を slice 1 件として expose、 default impl の paint_overlay /
    /// event_overlay / inject_theme / popup_request / on_popup_dismissed /
    /// on_drag_finished / paint_popup forward を inner に到達させる。
    fn children(&self) -> &[Box<dyn Widget>] {
        std::slice::from_ref(&self.inner)
    }

    fn children_mut(&mut self) -> &mut [Box<dyn Widget>] {
        std::slice::from_mut(&mut self.inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hayate_kit::widget::label::LabelWidget;

    /// Test helper: section → label widget の最小 stub closure factory。
    /// 本物の sections::*::build を呼ばず unit test を section-impl 依存から
    /// 切り離す (= sections regression と本 container test 独立性確保)。
    fn stub_build() -> impl Fn(SectionId, &AppStateHandles) -> Box<dyn Widget> + 'static {
        |sid, _state| {
            let name = match sid {
                SectionId::General => "general",
                SectionId::Appearance => "appearance",
                SectionId::Accessibility => "accessibility",
                SectionId::Ime => "ime",
                SectionId::Advanced => "advanced",
            };
            Box::new(LabelWidget::new(name, 14.0))
        }
    }

    /// `state.selected_section.set(new_sid)` → 次回 `layout` 呼出で
    /// `cached_section` が新 SectionId に更新されることを確認。
    /// 「TreeView on_select callback が state を更新 → 次 frame で detail pane
    /// が新 section の widget tree に切り替わる」 という reactive 反応の
    /// unit-level 固定。
    #[test]
    fn rebuild_on_section_change_reflects_new_section() {
        let state = crate::state::for_testing();
        let mut container = DetailContainerWidget::new(state.clone(), stub_build());

        // 初期 section は SectionId::default() = General
        assert_eq!(container.cached_section(), SectionId::default());

        // selected_section を Advanced に切り替え → layout 経由で rebuild
        state.selected_section.set(SectionId::Advanced);
        let _ = container.layout(&Constraints::unbounded());
        assert_eq!(container.cached_section(), SectionId::Advanced);

        // 別 section (Ime) に切り替え → 再度 rebuild
        state.selected_section.set(SectionId::Ime);
        let _ = container.layout(&Constraints::unbounded());
        assert_eq!(container.cached_section(), SectionId::Ime);
    }

    /// version 未変動の連続 layout で rebuild 回数が増えないことを確認
    /// (= cache hit path)。 副作用観察として「build closure の呼出回数」を
    /// counter で測定。
    #[test]
    fn no_rebuild_when_version_unchanged() {
        use std::cell::Cell;

        let state = crate::state::for_testing();
        let build_count = Rc::new(Cell::new(0u32));
        let bc = Rc::clone(&build_count);
        let mut container = DetailContainerWidget::new(state.clone(), move |_sid, _s| {
            bc.set(bc.get() + 1);
            Box::new(LabelWidget::new("test", 14.0))
        });

        // new() 内で 1 回 build される
        assert_eq!(build_count.get(), 1);

        // version 未変動の連続 layout → 追加 build なし
        let _ = container.layout(&Constraints::unbounded());
        let _ = container.layout(&Constraints::unbounded());
        let _ = container.layout(&Constraints::unbounded());
        assert_eq!(build_count.get(), 1);

        // selected_section.set 後の layout で +1、 さらに layout で増えない
        state.selected_section.set(SectionId::Advanced);
        let _ = container.layout(&Constraints::unbounded());
        assert_eq!(build_count.get(), 2);
        let _ = container.layout(&Constraints::unbounded());
        assert_eq!(build_count.get(), 2);
    }

    /// 同 SectionId への .set でも version は increment するため rebuild 発火
    /// (= State<T>::set の挙動)。 wave 3b では実害なし (TreeView on_select は
    /// 異 path 押下時のみ発火、 重複 set は practically 不発生)、
    /// 仕様明文化目的の test。
    #[test]
    fn same_section_set_triggers_rebuild_per_state_semantics() {
        use std::cell::Cell;

        let state = crate::state::for_testing();
        let build_count = Rc::new(Cell::new(0u32));
        let bc = Rc::clone(&build_count);
        let mut container = DetailContainerWidget::new(state.clone(), move |_sid, _s| {
            bc.set(bc.get() + 1);
            Box::new(LabelWidget::new("x", 14.0))
        });
        assert_eq!(build_count.get(), 1);

        // 初期と同じ General で .set → version++ → rebuild 発火
        state.selected_section.set(SectionId::General);
        let _ = container.layout(&Constraints::unbounded());
        assert_eq!(build_count.get(), 2);
    }
}
