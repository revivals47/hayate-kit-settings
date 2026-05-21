//! Settings panel sections module (= Phase 2 wave 0 scaffolding)。
//!
//! 各 section は `pub fn build(strings: &'static Strings) -> Box<dyn Widget>`
//! signature で detail pane に配置可能な widget tree を返す。 wave 0 では
//! 全 section が "Coming soon" placeholder を返す stub、 wave 1 worker
//! dispatch で各 section の form 内容を fill。
//!
//! ## section list (= RFC v0.5 §5.2)
//! - [`general`]: Language / Startup behavior / Window position
//! - [`appearance`]: Font size / Color mode / Accent color (= Theme は Phase 3)
//! - [`accessibility`]: Screen reader / High contrast / Reduce motion / Keyboard hint
//! - [`ime`]: IME backend / Candidate window position / Preedit display style
//! - [`advanced`]: Debug overlay / Log level / Cache clear / Reset / Safe boot hint
//!
//! ## SectionId (= TreeView nav selection state binding 用)
//! 各 section に対応する enum variant、 wave 1+ で reactive selection state
//! 配線時に使用 (= Rc<RefCell<SectionId>> or reactive State<SectionId>)。

pub mod general;
pub mod appearance;
pub mod accessibility;
pub mod ime;
pub mod advanced;
pub mod widgets;

/// Section identifier — TreeView nav selection と detail pane build dispatch
/// の binding に使用 (= wave 2/3 reactive bind 配線時)。
///
/// `#[allow(dead_code)]` on variants: wave 0 では `SectionId::default()` (= General)
/// のみ construct、 Appearance/Accessibility/Ime/Advanced variants は wave 2/3
/// で TreeView on_select callback で reactive bind 時 construct 予定。
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionId {
    General,
    Appearance,
    Accessibility,
    Ime,
    Advanced,
    /// Widgets showcase (= Phase 3b 第4波、 全 widget を state 込みで並べ
    /// behavioral 評価する場)。 sidebar root index 5。
    Widgets,
}

impl SectionId {
    /// Default section displayed on first launch (= General)。
    pub const fn default() -> Self {
        SectionId::General
    }

    /// Dev / screenshot hook: `HAYATE_SETTINGS_SECTION=<id>` overrides the
    /// section shown on launch (`general` / `appearance` / `accessibility` /
    /// `ime` / `advanced` / `widgets`). Unset or unrecognized falls back to
    /// [`default`]. Lets headless captures (`HAYATE_SCREENSHOT`) target any
    /// section without a pointer click.
    pub fn initial_from_env() -> Self {
        match std::env::var("HAYATE_SETTINGS_SECTION").ok().as_deref() {
            Some("general") => SectionId::General,
            Some("appearance") => SectionId::Appearance,
            Some("accessibility") => SectionId::Accessibility,
            Some("ime") => SectionId::Ime,
            Some("advanced") => SectionId::Advanced,
            Some("widgets") => SectionId::Widgets,
            _ => SectionId::default(),
        }
    }

    /// Map a `TreeView` selection path (= `Vec<usize>` of nested indices) to
    /// the corresponding `SectionId`。
    ///
    /// Sidebar TreeView node layout (= main.rs `build_sidebar`):
    /// - `[0]` = General
    /// - `[1]` = Appearance (root) — falls through to `Appearance` itself
    /// - `[1, 0]` = Appearance → Theme sub-node → `Appearance`
    /// - `[1, 1]` = Appearance → Font sub-node → `Appearance`
    /// - `[1, 2]` = Appearance → Color sub-node → `Appearance`
    /// - `[2]` = Accessibility
    /// - `[3]` = IME
    /// - `[4]` = Advanced
    /// - `[5]` = Widgets
    ///
    /// Empty / out-of-range paths return `None` so caller can fall back
    /// (e.g. keep current selection rather than panic). All Appearance sub-
    /// nodes resolve to `SectionId::Appearance` since the detail pane builds
    /// a unified Appearance page in wave 3b (= sub-node granularity is
    /// reserved for wave 4 scroll-to-anchor).
    ///
    /// ## DTP reuse
    /// Identical path → enum mapping pattern (= `match path.first()` +
    /// guarded sub-index) is reusable for DTP app preference dialog
    /// (= per-style / per-paragraph hierarchical settings)。
    #[allow(dead_code)] // wave 3b で main.rs TreeView on_select callback で使用化
    pub fn from_tree_path(path: &[usize]) -> Option<Self> {
        match path.first()? {
            0 => Some(SectionId::General),
            1 => Some(SectionId::Appearance), // root + any sub-node
            2 => Some(SectionId::Accessibility),
            3 => Some(SectionId::Ime),
            4 => Some(SectionId::Advanced),
            5 => Some(SectionId::Widgets),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_tree_path_root_indices() {
        assert_eq!(SectionId::from_tree_path(&[0]), Some(SectionId::General));
        assert_eq!(SectionId::from_tree_path(&[1]), Some(SectionId::Appearance));
        assert_eq!(SectionId::from_tree_path(&[2]), Some(SectionId::Accessibility));
        assert_eq!(SectionId::from_tree_path(&[3]), Some(SectionId::Ime));
        assert_eq!(SectionId::from_tree_path(&[4]), Some(SectionId::Advanced));
        assert_eq!(SectionId::from_tree_path(&[5]), Some(SectionId::Widgets));
    }

    #[test]
    fn from_tree_path_appearance_sub_nodes_all_resolve_to_appearance() {
        assert_eq!(SectionId::from_tree_path(&[1, 0]), Some(SectionId::Appearance));
        assert_eq!(SectionId::from_tree_path(&[1, 1]), Some(SectionId::Appearance));
        assert_eq!(SectionId::from_tree_path(&[1, 2]), Some(SectionId::Appearance));
    }

    #[test]
    fn from_tree_path_empty_returns_none() {
        assert_eq!(SectionId::from_tree_path(&[]), None);
    }

    #[test]
    fn from_tree_path_out_of_range_returns_none() {
        // 6 root sections (indices 0..=5); 6 and beyond are invalid
        assert_eq!(SectionId::from_tree_path(&[6]), None);
        assert_eq!(SectionId::from_tree_path(&[99]), None);
    }

    #[test]
    fn from_tree_path_non_appearance_root_with_subpath_still_resolves() {
        // Defensive: only Appearance root has sub-nodes in current layout,
        // but if a non-Appearance root somehow receives a sub-path index
        // (e.g. via TreeView API misuse), still resolve to that root's
        // SectionId rather than None — the first index is authoritative.
        assert_eq!(SectionId::from_tree_path(&[0, 7]), Some(SectionId::General));
        assert_eq!(SectionId::from_tree_path(&[4, 99]), Some(SectionId::Advanced));
    }

    #[test]
    fn default_is_general() {
        assert_eq!(SectionId::default(), SectionId::General);
    }
}
