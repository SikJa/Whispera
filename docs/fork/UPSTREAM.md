# Upstream integration — 0.3.0-readaloud.11

Integrated `SikJa/Whispera` through **87253eb9af45ba0aec632a969bcc52745ea4e4a6** (Windows release 0.2.38), from the previous integration point **8266b3075a5eebdeb4f9e120c25e22c772c37b6d**. This includes 26 upstream commits. The fork baseline before this merge was **28e530af60c9ebdfa928e1c84cd9933f1bf5d4aa**.

## Adaptations

- Preserve installed profile identity, local reading, automatic reading-language selection, MP3, AI quota connections, settings search and visible installed version.
- Move the reader controls and AI usage link into the new custom tray. Reader state uses phase/export metadata only; it does not expose selected text or start a synthesis helper to open the menu. Tray actions keep their own window authorization; general reader IPC remains restricted to Settings.
- Keep exact-original-field validation when pasting. Retain upstream UWP target discovery and Antigravity-specific video path handling, without the fallback to a different active field.
- Decode imported audio to a temporary local WAV when Local is selected. Local mode rejects cloud transcription requests before reading credentials or contacting Groq, including video transcript paths. To transcribe videos in Groq the user must explicitly select Groq and opt into video transcription.
- Install the 1.2 MB speech detector only following an explicit local-model installation. Startup may reuse an installed detector, without downloading one. Model downloads retain upstream pinned revisions, byte lengths and SHA-256 verification.
- Carry the per-video audio selection into the extracted capture view and monitor/fullscreen starts. Audio defaults remain unchanged, and failed preference loading blocks recording instead of silently choosing no audio.
- Retain opt-in defaults for global clipboard collection, video transcription and transcript attachments. Keep the recording consent check while adopting upstream per-file transcript job leases.
- Preserve the fork's package version and license notices. Add the upstream Vulkan SDK build step together with the Windows reader and quota helpers; keep author-only release/macOS publication restrictions and manual installation.
- Resolve 14 conflicting files. Ordinary native tests run sequentially once, matching the upstream fixtures' shared native initialization. Optional device/network fixtures remain ignored. Focus UI checks on changed capture, tray and engine paths plus the fork's reader, MP3, quota and search features.

Build results and installer hashes are recorded with the downloaded installer. This integration is validated using synthetic inputs and mocked UI IPC; real microphone/GPU quality, physical monitors and Windows application focus still require a user-led device check. No private recordings, selected text or account settings are used in the fixtures.

## Earlier integration

### 0.3.0-readaloud.9

Integrated repository: https://github.com/SikJa/Whispera

Integrated branch: `main`, through **8266b3075a5eebdeb4f9e120c25e22c772c37b6d**.
Previous common upstream base: `e62de979f8c2a396d46776f73c5816a79176eda8`.
The merge keeps both histories, including our reading, MP3, quota and recording controls.

## Included commits

- `2b5b354` — transcription latency and durable video segment timestamps.
- `ade0e26` — avoid redundant screenshot encoding and duplicate copy exports.
- `d7ea007` — preserve clipboard contention retries in the image publisher.
- `94993ea` — capture layers, video audio continuity, native drag and themes.
- `8001137` — image saving, custom backgrounds and background-only transcription filtering.
- `2f9a0ee` — read duration data independently of the shelf transition.
- `fd77070` — preserve original images and destinations during Save As.
- `8266b30` — keep the live color palette anchored while choosing colors.

## Fork adaptations

- Keep our package version, identity, whatlang dependency and bundled notices.
- Combine the app-theme imports with ReaderPanel and AIUsagePanel; preserve the reading and quota routes.
- Register the new upstream frontend suites before applying the fork's suite filter.
- Explicitly opt the three new ordinary transcription test fixtures, and the optional synthetic network fixture, into `transcribeVideo`. Fixtures that check transcript packaging also opt into `videoTranscriptAttachment`. Production defaults remain off.
- Keep global clipboard collection, video transcription and transcript attachments opt-in. Existing explicit preferences remain respected.
- Keep upstream installer checks/installations disabled. The macOS publication jobs remain restricted to SikJa's repository.
- Run ordinary native tests once for the broad upstream integration. Desktop capture, microphone, private clipboard and network fixtures remain opt-in/ignored. Frontend validation selects the changed areas and the existing fork features.

The legacy `upstream_updates.rs` checker tracks Whispera-K independently; its integrated commit is not changed to a SikJa commit. This document records the integration point for SikJa/Whispera.

## Validation and distribution

The distribution is a Windows x64 preview built by the fork's Windows workflow, with updater artifact creation disabled. It is installed manually, after successful validation, and is not merged into the upstream repository automatically.

Frontend type checking passed locally. The local clipboard renderer dependency cache contained a malformed Vite file, so the complete production build and regressions use clean dependencies on Windows CI. Build results, source/compiled revisions and checksums accompany the installer.

No personal recordings or clipboard contents are used to validate this integration. New timing/upload behavior is exercised by synthetic/offline fixtures; physical audio devices and native desktop interactions still require an explicit manual check.
