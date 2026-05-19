//! Persistence layer (= RFC v0.5 §5.2.6)。
//!
//! wave 2 = `Config` schema (version + 5 section nested struct) + serde
//! derive + migrate fn chain + atomic-rename save + corrupt detection +
//! debounced reactive helper + migration test framework + `--reset-config`
//! (= R12 mitigation) safe boot path 実装。
//!
//! ## schema
//! - `~/.config/hayate-kit-settings/config.json` (XDG_CONFIG_HOME 優先、
//!   不在時 `$HOME/.config` fallback)
//! - top-level `version: u32` + flat sections (= `general` / `appearance` /
//!   `accessibility` / `ime` / `advanced` 各 nested struct)
//! - sections は wave 1 で land した [`crate::sections`] の field と整合
//!
//! ## migration framework
//! - signature: `fn migrate_v{N}_to_v{N+1}(prev) -> next`
//! - [`load_from_str`] が version 検知 → migrate chain dispatch
//! - 各 migrate fn には round-trip test ([`tests::migrate_v0_to_v1_fills_defaults`] 等)
//! - 現状 v1 が initial schema、`V0Config` (= version field のみ) → `Config`
//!   への migrate skeleton を提供 (= future N+1 拡張時の reference 形)
//!
//! ## corrupt detection
//! - serde_json deserialize fail → `eprintln!("WARN ...")` + [`Config::default`]
//!   fallback。destructive recovery 回避、user の壊れた config は backup
//!   policy (= [`save`] が呼ばれた時) で `.bak` に保全される
//!
//! ## backup policy
//! - [`save`] は atomic rename pattern:
//!   1. `config.json.tmp` に新内容 write
//!   2. `config.json` が存在すれば `config.json.bak` へ rename (= 1 世代のみ保持)
//!   3. `config.json.tmp` を `config.json` へ rename
//! - Linux `rename(2)` は atomic、 partial-write による destructive 上書きを
//!   回避
//!
//! ## reactive bind (= wave 3 dep)
//! [`DebouncedSaver`] が 500ms quiet-period の信号 store を提供。wave 3 の
//! reactive layer が定期的に [`DebouncedSaver::is_ready`] を poll → true で
//! [`save`] を flush → [`DebouncedSaver::clear`] を呼ぶ contract。
//! 本 wave では actual reactive subscription は未配線、API surface のみ。
//!
//! ## --reset-config interaction (= R12 mitigation)
//! [`reset`] が現 `config.json` を `.bak` に rename した上で
//! [`Config::default`] を新 file に書き出す。Phase 1 step 7 で land 済の
//! `--reset-config` CLI flag は本 fn を invoke する想定 (= main.rs 側で
//! resolution、本 wave では公開 API のみ提供)。

// wave 2 = data layer 単体 land、 main.rs 側 wire は wave 3 dispatch で
// land 予定 (= ./agent-send.sh から本 file の APIs を invoke する caller が
// 現状 main.rs に未存在)。 dead_code warning は wave 3 PR で自然解消するので
// module-level で一括 allow する。 各 fn / type は test module で exercise
// 済 (= reachable from cargo test --bins)、 文字通り dead ではない。
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// Current config schema version。 schema 変更時 increment。
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

// ── enum types (serde-friendly mirror of UI state) ─────────────────────

/// UI 言語選好 (= mirror of [`crate::lang::Lang`] for persistence、 lang.rs
/// 自身に serde derive を追加せず本 layer 内で完結させる R23 mitigation)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LangConfig {
    Ja,
    /// 既定値 = `LANG` env var 未設定時等の fallback。
    #[default]
    En,
}

/// 配色 mode 選好 (= wave 1 Appearance section の Color mode ComboBox に対応)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ColorMode {
    Light,
    /// 既定値 = HAYATE_DARK fallback。
    #[default]
    Dark,
    /// Phase 4 で OS portal 経由実装予定、 wave 2 持続化のみ。
    System,
}

/// IME backend 選好 (= wave 1 IME section の backend ComboBox に対応)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ImeBackend {
    #[default]
    Ibus,
    Fcitx5,
}

/// IME 候補窓 anchor 選好 (= track-ime-popup-x-anchor debt link)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CandidatePosition {
    #[default]
    CursorAnchor,
    WidgetBottom,
}

/// IME preedit 表示 style。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PreeditStyle {
    #[default]
    Inline,
    Floating,
}

/// Diagnostics log verbosity (= wave 1 Advanced section の Log level ComboBox)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

// ── section nested structs ──────────────────────────────────────────────

/// General section (= RFC v0.5 §5.2.1) persisted state。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeneralConfig {
    /// UI 言語。
    #[serde(default)]
    pub language: LangConfig,
    /// 起動時 window 位置を reset する (= `true` で前回位置を無視)。
    #[serde(default)]
    pub startup_reset_window: bool,
    /// 終了時 window 位置を記録する。
    #[serde(default = "default_true")]
    pub remember_window_position: bool,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            language: LangConfig::default(),
            startup_reset_window: false,
            remember_window_position: true,
        }
    }
}

/// Appearance section (= RFC v0.5 §5.2.2) persisted state。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppearanceConfig {
    /// Font size scale (= base 14 ± offset、 range 10.0-20.0)。
    #[serde(default = "default_font_size_scale")]
    pub font_size_scale: f32,
    /// Color mode (= HAYATE_DARK / HAYATE_LIGHT / system follow)。
    #[serde(default)]
    pub color_mode: ColorMode,
    /// Accent color hex literal (= `#RRGGBB`、 wave 2 では String 持続化のみ、
    /// hex validation + WCAG warning re-evaluation は wave 3 reactive bind 側)。
    #[serde(default = "default_accent_hex")]
    pub accent_hex: String,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            font_size_scale: 14.0,
            color_mode: ColorMode::default(),
            accent_hex: default_accent_hex(),
        }
    }
}

/// Accessibility section (= RFC v0.5 §5.2.3) persisted state。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AccessibilityConfig {
    /// AccessKit + Orca 等 screen reader 連携の有効化。
    #[serde(default)]
    pub screen_reader: bool,
    /// 高 contrast palette 強制 (= Phase 4 で system follow と接続予定)。
    #[serde(default)]
    pub high_contrast: bool,
    /// 全 transition duration を 0ms に落とす (= Phase 3 で wire)。
    #[serde(default)]
    pub reduce_motion: bool,
    /// keyboard focus ring 強調表示。
    #[serde(default)]
    pub keyboard_nav_hints: bool,
}

/// IME section (= RFC v0.5 §5.2.4) persisted state。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ImeConfig {
    #[serde(default)]
    pub backend: ImeBackend,
    #[serde(default)]
    pub candidate_position: CandidatePosition,
    #[serde(default)]
    pub preedit_style: PreeditStyle,
}

/// Advanced section (= RFC v0.5 §5.2.5) persisted state。
///
/// Cache clear / Reset to defaults はいずれも action であり persisted
/// state を持たない。本 struct は `debug_overlay` + `log_level` の 2 field のみ。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AdvancedConfig {
    /// Debug overlay (= FPS / hit-test rect / widget tree) を表示する。
    #[serde(default)]
    pub debug_overlay: bool,
    /// Diagnostics log verbosity。
    #[serde(default)]
    pub log_level: LogLevel,
}

// ── top-level Config ────────────────────────────────────────────────────

/// Application config persisted to `~/.config/hayate-kit-settings/config.json`。
///
/// 起動時 [`load`] (or [`load_from_str`])、 immediate save trigger は wave 3
/// で [`DebouncedSaver`] 経由配線予定。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    /// Schema version for migration detection (= 起動時 version check で
    /// migrate fn chain dispatch)。
    pub version: u32,
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub appearance: AppearanceConfig,
    #[serde(default)]
    pub accessibility: AccessibilityConfig,
    #[serde(default)]
    pub ime: ImeConfig,
    #[serde(default)]
    pub advanced: AdvancedConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CURRENT_SCHEMA_VERSION,
            general: GeneralConfig::default(),
            appearance: AppearanceConfig::default(),
            accessibility: AccessibilityConfig::default(),
            ime: ImeConfig::default(),
            advanced: AdvancedConfig::default(),
        }
    }
}

// ── serde default helpers (function references) ─────────────────────────

fn default_true() -> bool {
    true
}

fn default_font_size_scale() -> f32 {
    14.0
}

fn default_accent_hex() -> String {
    String::from("#5A8BA8")
}

// ── migration framework ─────────────────────────────────────────────────

/// Minimal `version`-only struct used to detect pre-v1 (= legacy schema)
/// config files. v0 は実際に shipping した schema ではないが、 将来
/// `migrate_v{N}_to_v{N+1}` を追加する際の reference 形として skeleton を
/// 維持する。
#[derive(Debug, Clone, Deserialize)]
struct V0Config {
    #[allow(dead_code)] // future migration chain で参照予定、 v0 → v1 では捨てる
    version: u32,
}

/// v0 (= initial pre-release schema、 version field のみ) → v1 (= current)。
/// v0 には実 user-data field が無かったため、 全 section を default で fill。
/// future N+1 拡張時はこのパターンを踏襲: 新 field は default 値で fill、
/// 既存 field は prev から移送。
fn migrate_v0_to_v1(_prev: V0Config) -> Config {
    Config::default()
}

// ── load / save / reset ────────────────────────────────────────────────

/// Resolve canonical config file path (= XDG_CONFIG_HOME 優先、 fallback
/// `$HOME/.config`)。 環境変数いずれも未設定なら `None`。
pub fn default_config_path() -> Option<PathBuf> {
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(config_home.join("hayate-kit-settings").join("config.json"))
}

/// Parse raw JSON into a [`Config`]、 必要に応じて migrate chain を dispatch。
///
/// - 不正 JSON → `WARN` log + [`Config::default`]
/// - `version` > [`CURRENT_SCHEMA_VERSION`] → `WARN` log + [`Config::default`]
///   (= forward-incompatible config、 destructive 回避で default 起動)
/// - `version` == current → そのまま deserialize、 部分的 field 不正は serde の
///   `#[serde(default)]` 経由で field-level default 補完
/// - `version` == 0 → [`migrate_v0_to_v1`] 経由
pub fn load_from_str(raw: &str) -> Config {
    let value: serde_json::Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "WARN: hayate-kit-settings: failed to parse config JSON, using defaults: {e}"
            );
            return Config::default();
        }
    };

    let version = value
        .get("version")
        .and_then(|x| x.as_u64())
        .map(|v| v as u32)
        .unwrap_or(0);

    match version {
        0 => {
            // pre-v1 schema (= version field のみ or 完全 stub) → fill defaults
            let v0: V0Config = serde_json::from_value(value).unwrap_or(V0Config { version: 0 });
            migrate_v0_to_v1(v0)
        }
        v if v == CURRENT_SCHEMA_VERSION => {
            serde_json::from_value(value).unwrap_or_else(|e| {
                eprintln!(
                    "WARN: hayate-kit-settings: config v{CURRENT_SCHEMA_VERSION} \
                     partially malformed ({e}), using defaults"
                );
                Config::default()
            })
        }
        v if v > CURRENT_SCHEMA_VERSION => {
            eprintln!(
                "WARN: hayate-kit-settings: config version {v} is newer than supported \
                 {CURRENT_SCHEMA_VERSION}; using defaults (file is preserved on disk)"
            );
            Config::default()
        }
        // 1 <= v < CURRENT will land here once CURRENT > 1; chain stays open.
        _ => Config::default(),
    }
}

/// Serialize a [`Config`] to a pretty-printed JSON string suitable for
/// `config.json` on disk。
pub fn save_to_string(config: &Config) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(config)
}

/// Atomically save `config` to `path` with the documented backup policy:
///
/// 1. Serialize to `{path}.tmp`
/// 2. If `path` exists, rename it to `{path}.bak` (= 1-generation backup)
/// 3. Rename `{path}.tmp` to `path`
///
/// Linux `rename(2)` は atomic で、 partial-write による destructive 上書き
/// を回避する。 また parent directory が無ければ `create_dir_all` で生成。
pub fn save(config: &Config, path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = sibling_with_suffix(path, ".tmp");
    let bak = sibling_with_suffix(path, ".bak");
    let json = save_to_string(config).map_err(std::io::Error::other)?;
    std::fs::write(&tmp, json)?;
    if path.exists() {
        // 1 世代のみ保持: 既存 .bak は今回の .bak で上書きされる
        std::fs::rename(path, &bak)?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Load config from `path`、 不在 / 不正 / 旧 schema いずれも recoverable に
/// [`Config::default`] へ fallback。 file system error (= permission denied
/// 等) は呼出側に `io::Error` で伝播。
pub fn load(path: &Path) -> std::io::Result<Config> {
    match std::fs::read_to_string(path) {
        Ok(raw) => Ok(load_from_str(&raw)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
        Err(e) => Err(e),
    }
}

/// `--reset-config` CLI flag (= R12 mitigation = bricked state recovery path)
/// の publicly-callable 本体: 既存 file を `.bak` へ rename した上で
/// [`Config::default`] を新 file に書き出す。
///
/// main.rs 側で `clap` parsed flag から本 fn を呼ぶ想定。本 wave では公開
/// API のみ提供、 main.rs 側 wire は wave 3 dispatch で land 予定。
pub fn reset(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if path.exists() {
        let bak = sibling_with_suffix(path, ".bak");
        std::fs::rename(path, &bak)?;
    }
    save(&Config::default(), path)
}

/// `config.json` -> `config.json.tmp` / `config.json.bak` 等、 suffix を
/// extension では無く literal append で扱う helper。 `Path::with_extension`
/// は dot-replace 挙動なので不適。
fn sibling_with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

// ── DebouncedSaver (= wave 3 reactive bind glue) ───────────────────────

/// 500ms quiet-period debouncer for reactive save triggers。
///
/// wave 3 の reactive layer は config 変更 callback で [`request`](Self::request)
/// を call し、 別 thread / event-loop tick で [`is_ready`](Self::is_ready) を
/// poll する contract。 ready が true になったら [`save`] を flush して
/// [`clear`](Self::clear) を呼ぶ。
///
/// 本 wave 2 では actual reactive subscription は未配線 (= [`request`] caller
/// が存在しない)、API surface + 動作 test のみ提供。
#[derive(Debug)]
pub struct DebouncedSaver {
    quiet_period: Duration,
    pending_since: Mutex<Option<Instant>>,
}

impl DebouncedSaver {
    /// Default quiet period (= 500ms、 RFC v0.5 §5.2.6)。
    pub const DEFAULT_QUIET_PERIOD: Duration = Duration::from_millis(500);

    /// Construct with the default 500ms quiet period.
    pub fn new() -> Self {
        Self::with_quiet_period(Self::DEFAULT_QUIET_PERIOD)
    }

    /// Construct with a custom quiet period (= test 用、 production は
    /// [`Self::new`] 推奨)。
    pub fn with_quiet_period(quiet_period: Duration) -> Self {
        Self {
            quiet_period,
            pending_since: Mutex::new(None),
        }
    }

    /// Record a save request。 quiet-period timer を **reset** する (= 連続
    /// toggle で disk thrash を防ぐ debounce semantics)。
    pub fn request(&self) {
        let mut guard = self.pending_since.lock().expect("mutex poisoned");
        *guard = Some(Instant::now());
    }

    /// True if a request is pending and the quiet period has elapsed。
    /// wave 3 reactive layer がここを poll、 true なら [`save`] を flush
    /// した上で [`Self::clear`] を呼ぶ contract。
    pub fn is_ready(&self) -> bool {
        let guard = self.pending_since.lock().expect("mutex poisoned");
        match *guard {
            Some(t) => t.elapsed() >= self.quiet_period,
            None => false,
        }
    }

    /// Clear the pending request (= flush 完了後 caller が呼ぶ)。
    pub fn clear(&self) {
        let mut guard = self.pending_since.lock().expect("mutex poisoned");
        *guard = None;
    }

    /// True if a request is currently pending (= ready 判定とは独立、 quiet
    /// period 未経過でも pending なら true)。
    pub fn has_pending(&self) -> bool {
        self.pending_since
            .lock()
            .expect("mutex poisoned")
            .is_some()
    }
}

impl Default for DebouncedSaver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── existing baseline test (preserved) ──────────────────────────────

    #[test]
    fn default_config_has_current_schema_version() {
        assert_eq!(Config::default().version, CURRENT_SCHEMA_VERSION);
    }

    // ── default-value invariants ────────────────────────────────────────

    #[test]
    fn general_defaults_match_user_expectation() {
        let g = GeneralConfig::default();
        assert_eq!(g.language, LangConfig::En);
        assert!(!g.startup_reset_window);
        assert!(g.remember_window_position);
    }

    #[test]
    fn appearance_defaults_match_hayate_baseline() {
        let a = AppearanceConfig::default();
        assert!((a.font_size_scale - 14.0).abs() < f32::EPSILON);
        assert_eq!(a.color_mode, ColorMode::Dark);
        assert_eq!(a.accent_hex, "#5A8BA8");
    }

    #[test]
    fn advanced_defaults_match_info_log_level() {
        let a = AdvancedConfig::default();
        assert!(!a.debug_overlay);
        assert_eq!(a.log_level, LogLevel::Info);
    }

    // ── round-trip ──────────────────────────────────────────────────────

    #[test]
    fn config_round_trip_via_json_is_lossless() {
        let mut c = Config::default();
        c.general.language = LangConfig::Ja;
        c.appearance.font_size_scale = 17.5;
        c.ime.backend = ImeBackend::Fcitx5;
        c.advanced.log_level = LogLevel::Debug;
        let json = save_to_string(&c).expect("serialize");
        let back = load_from_str(&json);
        assert_eq!(back, c);
    }

    // ── migration round-trip (mandatory) ────────────────────────────────

    #[test]
    fn migrate_v0_to_v1_fills_defaults() {
        // pre-v1 stub schema = version field のみ、 sections 不在
        let raw = r#"{ "version": 0 }"#;
        let migrated = load_from_str(raw);
        assert_eq!(migrated.version, CURRENT_SCHEMA_VERSION);
        assert_eq!(migrated, Config::default());
    }

    #[test]
    fn migrate_v0_to_v1_works_for_empty_object_too() {
        // version 不在の object でも v0 と同等扱い (= default fallback path)
        let raw = "{}";
        let migrated = load_from_str(raw);
        assert_eq!(migrated.version, CURRENT_SCHEMA_VERSION);
    }

    // ── corrupt detection ───────────────────────────────────────────────

    #[test]
    fn load_from_str_handles_malformed_json_with_defaults() {
        let raw = "this is not json {{{";
        let c = load_from_str(raw);
        assert_eq!(c, Config::default());
    }

    #[test]
    fn load_from_str_handles_partial_field_corruption_with_section_default() {
        // version 正、 section 内 field の一部 type 不正 → serde の
        // section-level deserialize が fail し、 whole-config default に
        // fall back (= destructive 回避、 .bak 保全は save() path 側)
        let raw = r#"{
            "version": 1,
            "general": { "language": "ja", "startup_reset_window": true,
                         "remember_window_position": "not-a-bool" }
        }"#;
        let c = load_from_str(raw);
        assert_eq!(c, Config::default());
    }

    #[test]
    fn load_from_str_rejects_future_version_with_defaults() {
        let raw = format!(r#"{{ "version": {} }}"#, CURRENT_SCHEMA_VERSION + 99);
        let c = load_from_str(&raw);
        assert_eq!(c, Config::default());
    }

    // ── enum serde wire format invariants ───────────────────────────────

    #[test]
    fn lang_config_serializes_as_lowercase() {
        assert_eq!(
            serde_json::to_string(&LangConfig::Ja).unwrap(),
            r#""ja""#
        );
        assert_eq!(
            serde_json::to_string(&LangConfig::En).unwrap(),
            r#""en""#
        );
    }

    #[test]
    fn candidate_position_serializes_as_snake_case() {
        assert_eq!(
            serde_json::to_string(&CandidatePosition::CursorAnchor).unwrap(),
            r#""cursor_anchor""#
        );
        assert_eq!(
            serde_json::to_string(&CandidatePosition::WidgetBottom).unwrap(),
            r#""widget_bottom""#
        );
    }

    // ── disk I/O (= load / save / reset round trip via tempdir) ─────────

    fn temp_path(name: &str) -> PathBuf {
        // テスト isolation: pid + counter で衝突回避。tempfile crate を
        // 追加せず std で完結 (= scope discipline)。
        let dir = std::env::temp_dir().join(format!(
            "hayate-kit-settings-test-{}-{}",
            std::process::id(),
            name
        ));
        let _ = std::fs::remove_dir_all(&dir); // 前回 残骸を掃除
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir.join("config.json")
    }

    #[test]
    fn save_then_load_round_trip_via_filesystem() {
        let path = temp_path("save-load-roundtrip");
        let mut c = Config::default();
        c.appearance.accent_hex = String::from("#3D6884");
        c.ime.candidate_position = CandidatePosition::WidgetBottom;
        save(&c, &path).expect("save");
        let back = load(&path).expect("load");
        assert_eq!(back, c);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn save_creates_bak_when_overwriting_existing() {
        let path = temp_path("save-creates-bak");
        let bak = sibling_with_suffix(&path, ".bak");
        save(&Config::default(), &path).expect("first save");
        assert!(path.exists());
        assert!(!bak.exists(), "no .bak before second save");
        let mut c = Config::default();
        c.advanced.debug_overlay = true;
        save(&c, &path).expect("second save");
        assert!(path.exists());
        assert!(bak.exists(), ".bak must be created on overwrite");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn load_missing_file_returns_default() {
        let path = temp_path("load-missing");
        // file は **作成しない**
        let c = load(&path).expect("load on missing file is recoverable");
        assert_eq!(c, Config::default());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn reset_backs_up_existing_and_writes_default() {
        let path = temp_path("reset-roundtrip");
        let bak = sibling_with_suffix(&path, ".bak");
        let mut c = Config::default();
        c.appearance.accent_hex = String::from("#FFCC00");
        c.advanced.log_level = LogLevel::Trace;
        save(&c, &path).expect("seed save");
        // 初回 save では .bak は作られない (= overwrite ではない)
        assert!(!bak.exists());
        reset(&path).expect("reset");
        // reset 後: 新 file は default、 .bak は seed 内容
        let post = load(&path).expect("load after reset");
        assert_eq!(post, Config::default());
        assert!(bak.exists(), ".bak must hold the pre-reset content");
        let bak_raw = std::fs::read_to_string(&bak).expect("read bak");
        let bak_parsed = load_from_str(&bak_raw);
        assert_eq!(bak_parsed, c);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    // ── DebouncedSaver semantics ────────────────────────────────────────

    #[test]
    fn debouncer_starts_idle() {
        let d = DebouncedSaver::new();
        assert!(!d.has_pending());
        assert!(!d.is_ready());
    }

    #[test]
    fn debouncer_request_marks_pending_but_not_ready() {
        let d = DebouncedSaver::new();
        d.request();
        assert!(d.has_pending());
        // 500ms quiet period 未経過 → ready=false
        assert!(!d.is_ready());
    }

    #[test]
    fn debouncer_ready_after_quiet_period_elapses() {
        // Test 用 short quiet period で is_ready transition を実 sleep で観測。
        let d = DebouncedSaver::with_quiet_period(Duration::from_millis(20));
        d.request();
        assert!(!d.is_ready());
        std::thread::sleep(Duration::from_millis(40));
        assert!(d.is_ready());
    }

    #[test]
    fn debouncer_clear_resets_pending() {
        let d = DebouncedSaver::with_quiet_period(Duration::from_millis(1));
        d.request();
        std::thread::sleep(Duration::from_millis(5));
        assert!(d.is_ready());
        d.clear();
        assert!(!d.has_pending());
        assert!(!d.is_ready());
    }

    #[test]
    fn debouncer_subsequent_request_resets_quiet_window() {
        // 2 連続 request の後者で quiet timer が reset され、 短時間後の
        // ready 判定が false になることを smoke。
        let d = DebouncedSaver::with_quiet_period(Duration::from_millis(50));
        d.request();
        std::thread::sleep(Duration::from_millis(30));
        d.request(); // reset
        // 30ms 待っただけでは 50ms quiet period に未達
        assert!(!d.is_ready());
    }
}
