# hayate-kit-settings

GUI_kit framework-native settings panel app (新世代 app 第 1 号)。

## 概要

[GUI_kit](https://github.com/revivals47/GUI_kit) を base にした settings panel app。HAYATE original aesthetic で実装、theme switcher で 既存 retro theme preset (Win95 / macOS 9 / Win XP / etc.) も live switch 可能。

## 規範

本 app は **`hayate-kit` (GUI_kit) のみに依存**、framework (`hayate-platform`) 直接依存しない。L2 facade pattern 徹底 + GUI_kit driven feedback loop 強化が architectural stance。

詳細は [GUI_kit repo の RFC](https://github.com/revivals47/GUI_kit/blob/main/workspace/president-notes/hayate-kit-settings-rfc-v0.1.md) (v0.2) 参照。

## 関連

- GUI_kit upstream: https://github.com/revivals47/GUI_kit
- 既存 dogfood (legacy): hayate-notepad / hayate-freecell / hayate-solitaire 等

## Status

Phase 1 skeleton (= repo init 段階)、本体実装は incremental。

## License

Dual-licensed under MIT or Apache-2.0、GUI_kit upstream と同。
