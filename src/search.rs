//! Search bar widget (= RFC v0.5 §5.2.7、 Phase 2 wave 2 fill)。
//!
//! ## wave 2 scope (= 本 commit)
//! - [`build(strings) -> Box<dyn Widget>`]: `TextInputWidget` ベースの search bar
//!   (placeholder = "Search settings...")
//! - [`filter_sections(query, strings) -> Vec<SectionId>`]: JA + EN 両 strings
//!   table を linear search、 query が section label に含まれる SectionId を
//!   返却。empty query = 全 section 返却。 R17 mitigation = trie indexing は
//!   Phase 4 defer、 全 field ~50 件想定で linear で sufficient。
//!
//! ## wave 境界
//! wave 2 = Search bar widget composition + filter callback signature 定義。
//! wave 3b = `TextInputWidget::on_change` push-style reactive bind で text 変更
//! を [`AppStateHandles::search_query`] へ反映 ([`apply_search_query_change`])。
//! TreeView selection 更新側 (= `filter_sections` 呼出 → `selected_section` set)
//! は consumer 側 (main.rs / sidebar layout) の reactive observer dep。
//!
//! main.rs `build_sidebar` (= wave 2 で worker3 が更新) で `build(strings)` の
//! return を VStack で TreeView の上に配置。
//!
//! ## DTP reuse
//! DTP app preference dialog (= 大量 settings 横断検索) でも reuse 想定、
//! Search bar pattern + linear `filter_*` fn signature は universal。

use hayate_kit::prelude::*;

use crate::lang::{Lang, Strings};
use crate::sections::SectionId;
use crate::state::AppStateHandles;

/// Search bar widget build (= Phase 2 wave 2 fill + wave 3b reactive wire)。
///
/// `TextInputWidget` を placeholder = "Search settings..." + width 240px で
/// 構成。 width は sidebar (= SplitView 30% 比率、 540 wide 既定で sidebar
/// ~240px) に合わせ、 上限を控えめに設定。
///
/// HAYATE Original aesthetic は `active_theme()` 経由 default で reach 済
/// (= GUI_kit R13 systemic fix land 後)、caller-side `.theme()` override 不要。
///
/// ## wave 3b on_change wire
/// `.on_change(|t| apply_search_query_change(&state, t))` で push-style reactive
/// bind。text 変更で [`AppStateHandles::search_query`] が即時更新され、
/// consumer 側 (= main.rs / sidebar layout) が `search_query` を observe して
/// [`filter_sections`] 呼出 → TreeView selection 更新を駆動する想定。
/// search query は永続化対象外のため debouncer は触らない。
pub fn build(_strings: &'static Strings, state: &AppStateHandles) -> Box<dyn Widget> {
    Box::new(
        TextInputWidget::new()
            .with_placeholder("Search settings...")
            .with_width(240.0)
            .on_change({
                let state = state.clone();
                move |text| apply_search_query_change(&state, text)
            }),
    )
}

/// Search bar `on_change` の反映: 入力 text を [`AppStateHandles::search_query`]
/// へ set。search_query は永続化対象外 (= UI live filter state) のため
/// debouncer.request() は呼ばない。
fn apply_search_query_change(state: &AppStateHandles, text: &str) {
    state.search_query.set(text.to_owned());
}

/// Filter sections by query (= linear search over JA + EN strings)。
///
/// `query` が `strings` 内の section label (JA + EN 両言語、 大文字小文字
/// 無視 ASCII 比較) のいずれかに含まれる SectionId を返却。empty query は
/// 全 section を default 順 (General / Appearance / Accessibility / Ime /
/// Advanced) で返却。
///
/// ## 実装方針
/// - JA + EN 両言語マッチ: ユーザーが入力言語に関わらず検索ヒットするよう
///   `Lang::Ja.strings()` + `Lang::En.strings()` 両 table をマッチ対象に
/// - case-insensitive (ASCII): `to_ascii_lowercase` で正規化、日本語は
///   そのまま部分一致 (= unicode case folding は Phase 4 i18n 拡張で検討)
/// - linear scan (= R17 mitigation): 5 section + Appearance sub 3 = ~8 field、
///   trie / inverted index は overengineering、 Phase 4 defer
/// - reactive bind は wave 3 dep: 本 fn 単体で SectionId 列挙のみ、 TreeView
///   selection 更新 / detail pane swap は caller 側 (= wave 3)
///
/// ## DTP reuse
/// 同じ signature pattern (= `fn filter_<domain>(query, table) -> Vec<Id>`) は
/// DTP app preference dialog の検索でも 100% reuse 可能。
#[allow(dead_code)] // wave 3 で main.rs reactive bind 配線時に caller 化
pub fn filter_sections(query: &str, _strings: &Strings) -> Vec<SectionId> {
    let q = query.trim();
    let all = [
        SectionId::General,
        SectionId::Appearance,
        SectionId::Accessibility,
        SectionId::Ime,
        SectionId::Advanced,
        SectionId::Widgets,
    ];
    if q.is_empty() {
        return all.to_vec();
    }
    let q_lower = q.to_ascii_lowercase();
    let ja = Lang::Ja.strings();
    let en = Lang::En.strings();
    all.iter()
        .copied()
        .filter(|sid| {
            let (ja_label, en_label) = label_pair(*sid, ja, en);
            ja_label.contains(q)
                || en_label.to_ascii_lowercase().contains(&q_lower)
        })
        .collect()
}

/// Helper: SectionId → (JA label, EN label) で両言語 label を返却。
/// `filter_sections` の linear scan 内部のみ使用。
fn label_pair(sid: SectionId, ja: &Strings, en: &Strings) -> (&'static str, &'static str) {
    match sid {
        SectionId::General => (ja.section_general, en.section_general),
        SectionId::Appearance => (ja.section_appearance, en.section_appearance),
        SectionId::Accessibility => (ja.section_accessibility, en.section_accessibility),
        SectionId::Ime => (ja.section_ime, en.section_ime),
        SectionId::Advanced => (ja.section_advanced, en.section_advanced),
        SectionId::Widgets => (ja.section_widgets, en.section_widgets),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_smoke_does_not_panic() {
        // smoke: build(strings) が panic せず Box<dyn Widget> を返却。
        let strings = Lang::En.strings();
        let state = crate::state::for_testing();
        let _w = build(strings, &state);
    }

    #[test]
    fn filter_empty_query_returns_all() {
        // empty query = 全 6 section、 default 順 (General/Appearance/
        // Accessibility/Ime/Advanced/Widgets)。
        let strings = Lang::En.strings();
        let r = filter_sections("", strings);
        assert_eq!(r.len(), 6);
        assert_eq!(r[0], SectionId::General);
        assert_eq!(r[4], SectionId::Advanced);
        assert_eq!(r[5], SectionId::Widgets);
    }

    #[test]
    fn filter_whitespace_only_returns_all() {
        // whitespace-only query は trim 後 empty 扱い = 全 section。
        let strings = Lang::En.strings();
        let r = filter_sections("   ", strings);
        assert_eq!(r.len(), 6);
    }

    #[test]
    fn filter_en_substring_case_insensitive() {
        // EN substring + case-insensitive: "APPEAR" → Appearance。
        let strings = Lang::En.strings();
        let r = filter_sections("APPEAR", strings);
        assert_eq!(r, vec![SectionId::Appearance]);
    }

    #[test]
    fn filter_ja_substring_match() {
        // JA substring match: "詳細" → 詳細設定 = Advanced。
        let strings = Lang::En.strings();
        let r = filter_sections("詳細", strings);
        assert_eq!(r, vec![SectionId::Advanced]);
    }

    #[test]
    fn filter_no_match_returns_empty() {
        // 該当なし → empty Vec。
        let strings = Lang::En.strings();
        let r = filter_sections("zzzzzz_no_match", strings);
        assert!(r.is_empty());
    }

    // ── wave 3b on_change wire ─────────────────────────────────────────

    #[test]
    fn search_query_change_propagates_to_state() {
        let state = crate::state::for_testing();
        assert!(state.search_query.get().is_empty());
        apply_search_query_change(&state, "appearance");
        assert_eq!(*state.search_query.get(), "appearance");
        // Search query は永続化対象外、 debouncer は触らないことを assert。
        assert!(!state.debouncer.borrow().has_pending());
    }
}
