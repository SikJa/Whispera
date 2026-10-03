# Whispera library / Biblioteca

The Edge Drop renderer is adapted inside Whispera's existing Tauri process.
Dropshelf is not part of the integration. No second Electron application runs.
Source, pinned revision and Apache-2.0 license are in apps/desktop/vendor/edge-drop.

## Workflow / Flujo

- Alt+C (configurable) or tray Library opens the shelf, initially on the left.
- Copying supported text, links, images or file lists populates local SQLite history.
- Whispera captures and finished screen videos are also registered directly.
- Filters: All, Text, Links, Images, Videos, Files. No color/emoji filters or count badge.
- Search, pin, delete, clear unpinned, stack compatible items and ungroup are preserved.
- Quick Preview opens beside the shelf. Supported videos use native WebView playback.
- Copy uses Windows clipboard formats. Paste restores the destination captured at opening.
- Images/videos/files use Windows OLE drag via drag-rs; destination apps must accept files.
- Text/links use HTML text drag. Native WebView-to-external-app compatibility needs manual validation.
- Retention defaults to 48 hours and 250 unpinned items. Pins are excluded from expiration.
- Clearing/deleting library items does NOT delete original external files.
- Owned clipboard PNGs without history references are cleaned internally hourly.
- Incognito pauses collection, including direct capture ingestion.
- Settings support side/top placement, panel geometry, history retention, sounds,
  reduced motion, shortcut changes and Whispera startup. Updates link to Whispera;
  automatic download/install is not configured. Only current-monitor targeting is exposed.

## Privacy / Privacidad

History is local and currently not encrypted. Global capture uses sequence-number
polling; extremely fast successive copies can be missed. Only the latest readable
clipboard payload can be recovered, not earlier Windows clipboard history.
Collection respects ExcludeClipboardContentFromMonitorProcessing and skips recognizable
Groq/OpenAI project API keys. This is NOT a guarantee that every password or secret is detected.
Use Incognito for sensitive work. Clipboard content is never sent to Groq by this feature.
Files copied from Explorer are referenced, not duplicated; deleting the source makes
that historical file unavailable. Playback depends on the WebView's installed codecs.

## Verification

verify-edge-preview.mjs: original renderer interactions, image grouping, split and pin,
headless with local screenshot fixtures. No native clipboard access.
verify-native-library.mjs: production bundle, native bridge mocked with synthetic data,
filters, subitem copy, file-drag IPC, persistence refresh and clipped geometry.
Rust unit tests: retention, deduplication, incognito and group/path boundaries.
No visible Windows capture tests are run. Passing these tests does not prove real
OLE drop into a specific browser, focus restoration, or compositor behavior.

## Next CleanShot-inspired features (not implemented here)

Quick Access Overlay after capture, floating pinned screenshots, background/padding
presets, and combining several images on a single export canvas.
Reference: https://cleanshot.com/features
