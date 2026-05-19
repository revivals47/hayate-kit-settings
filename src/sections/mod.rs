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
}

impl SectionId {
    /// Default section displayed on first launch (= General)。
    pub const fn default() -> Self {
        SectionId::General
    }
}
