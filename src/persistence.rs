//! Persistence layer (= RFC v0.5 §5.2.6)。
//!
//! wave 0 = `Config` struct stub + 空 default、 wave 2 worker dispatch で
//! 各 field + serde + migration framework + atomic rename + reactive bind を fill。
//!
//! ## spec (= RFC v0.5 §5.2.6 nail down):
//! - schema: `~/.config/hayate-kit-settings/config.json` with `version: u32`
//!   field at top + flat sections (= general / appearance / accessibility /
//!   ime / advanced 各 nested struct)
//! - migrate fn signature: `fn migrate_v{N}_to_v{N+1}(prev) -> current`、
//!   chain で N step skip 対応
//! - corrupt detection: serde deserialize fail → log warn + fallback to
//!   default、 destructive recovery 回避
//! - backup policy: 上書き前 atomic rename (= config.json → config.json.bak、
//!   1 世代のみ保持)
//! - reactive bind: config change → save trigger は debounced (= 500ms
//!   quiet period 後 save)
//! - migration test framework: 各 migrate fn に round-trip test
//! - safe boot interaction: `--reset-config` (= R12 mitigation) は current
//!   config を .bak rename + 起動時 default 生成 path
//!
//! ## 関連
//! - [[track-ime-popup-x-anchor]] etc. debt は本 Config 内 IME section で
//!   user-toggleable settings として実装 (= wave 1 IME section impl と整合)

/// Current config schema version。 schema 変更時 increment。
#[allow(dead_code)] // wave 2 で persistence layer impl 時に使用
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Application config persisted to `~/.config/hayate-kit-settings/config.json`。
///
/// wave 0 stub = version field のみ、 wave 2 で各 section nested struct + serde
/// + migration framework fill。
#[allow(dead_code)] // wave 2 で persistence layer impl 時に construct
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// Schema version for migration detection (= 起動時 version check で
    /// migrate fn chain dispatch)。
    pub version: u32,
    // wave 2 で各 section nested struct 追加予定:
    // pub general: GeneralConfig,
    // pub appearance: AppearanceConfig,
    // pub accessibility: AccessibilityConfig,
    // pub ime: ImeConfig,
    // pub advanced: AdvancedConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CURRENT_SCHEMA_VERSION,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_current_schema_version() {
        assert_eq!(Config::default().version, CURRENT_SCHEMA_VERSION);
    }
}
