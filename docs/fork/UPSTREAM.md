# Upstream integration — 0.3.0-readaloud.9

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
