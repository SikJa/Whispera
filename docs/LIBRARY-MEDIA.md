# Library media tools

Reference: https://cleanshot.com/features (Quick Access Overlay, Floating Screenshots, Background tool, Combine multiple images).

## Implemented

- Video tiles use a cached first frame and duration. Quick Preview plays supported browser codecs from the authorized local asset protocol, with controls below the image rather than the native dark gradient over it.
- Captures enter the library silently. Quick-access and floating-reference windows have been removed entirely, including native opening modes and renderer actions. The old `quickAccessOverlay` value is ignored.
- Edit image is offered only on images, image collections and file groups containing PNG/JPEG/WebP images. Videos, text, links and other files do not show the pencil. Copy, pin, preview, delete and native file drag remain available in the shelf.
- Composition editor: up to ten images, free positioning, aspect-preserving resizing, layer ordering/removal, auto balance, padding, corner radius, shadow, transparent/solid/custom-color backgrounds, aspect ratios.
- Background editor: twelve original soft mesh-gradient presets, blurred capture background, library image wallpaper, collapsible settings panel and optional automatic balancing. Background preview and export share the same raster renderer. No CleanShot wallpaper assets were copied.
- Editor controls use custom compact sliders, a keyboard-accessible aspect-ratio menu and an animated balance switch. Dropdown and toggle motion follows transitions-dev tokens, including reduced-motion behavior. Escape dismisses the ratio menu without closing the editor.
- Export creates a real PNG in managed `compositions`, records it in the library and copies the bitmap to the Windows clipboard. Source images are unchanged.
- Native drag uses an actual bounded image/video-frame thumbnail instead of the app icon. Prestage warms the disk cache. Actual drop acceptance depends on the destination application.
- Transitions use the `transitions-dev` panel/resize easing and reduced-motion guards. The original Edge Drop shelf layout remains intact.

## Boundaries

No CleanShot code/assets were copied. These are local Windows implementations of the referenced workflows, not a promise of exact macOS compositor behavior.

Video format support depends on WebView2 codecs; MP4 H.264 was exercised headlessly. MKV/AVI or unsupported codecs may require opening the original in another player. Video background editing/export, arbitrary background photos, annotation editing in this new canvas, reusable project files, OCR, click-through lock mode and cloud upload are not part of this change.

Captured files retain their existing retention policy. Managed compositions are not user-selected source files and are safe to retain separately.

## Main configuration and video speech

- Main Configuration now includes Portapapeles: global clipboard capture, pause history, capacity, automatic expiration, clear unpinned on restart, microphone transcription and transcript attachments.
- Portapapeles opens on Biblioteca, with native-backed search, type filters, copy/pin/delete/reveal and direct image editing. Configuracion retains unsaved settings while switching tabs. File drag remains in the existing shelf opened with Abrir estante or its shortcut.
- Atajos includes the library shortcut (existing value preserved, default Alt+C). All four shortcuts are validated and saved together with registration rollback on failure.
- New screen recordings with Microphone or Both selected prepare microphone-only mono 16 kHz WAV segments, bounded to five minutes each. System audio is never used for speech transcription. No microphone is enabled implicitly when recording without audio or system-only.
- After recording completes, a background worker calls the existing Groq implementation using the current key, model, language and dictionary. Completed segment text is checkpointed locally. Interrupted work resumes after restart; provider errors are visible in Quick Preview and can be retried without losing the MP4 or source speech.
- Quick Preview shows the transcript, Copy text and retry on provider errors. Video tiles show pending/ready/error status.
- Copy/paste and native file drag can include the MP4 plus its generated `.transcript.txt` attachment. Clipboard copy also advertises Unicode text. Destination applications choose the formats they accept: this does NOT guarantee automatic caption insertion or acceptance of two attachments in every app.
- Existing videos are not retroactively uploaded. No old private recording was uploaded during verification. The Groq path was verified with one public MInDS-14 audio fixture (32 words, 1124 ms for request, persistence and TXT packaging); real paste into third-party apps still requires user-side acceptance testing.
- The initial video copy remains immediate. When transcription completes, its clipboard package is upgraded only if the clipboard still contains that same single video; unrelated copied content is not replaced.

## Verification

- `npm run build`: desktop and library production bundles.
- `cargo test`: Rust tests, excluding the explicitly opt-in desktop capture and Groq benchmark.
- `verify-native-library.mjs`: filters, video thumbnail/duration, history actions and IPC connections, with native APIs mocked.
- `verify-library-settings.mjs`: production settings, persistence contract, fourth shortcut and no capture-window invocation, entirely headless with native IPC mocked.
- Rust video-transcript tests: real FFmpeg PCM-to-WAV conversion with synthetic silence, ordered pause segments, preparation errors, transcript attachment readiness and opt-out, no live Groq requests or Windows UI.
- Explicit opt-in `public_groq_video_transcript_pipeline`: real Groq request on the public fixture, persisted transcript and matching TXT attachment. Does not record desktop/audio or mutate the user's clipboard.
- Position regression: left/right/top, horizontal offset, no truncated labels, recovery from a rejected settings save and OS reduced-motion preference. All headless.
- Configuration opening now restores a minimized/externally hidden native window, hides the library that could obscure it, and uses the same opening path from the tray and recorder. This native foreground action is not exercised by headless tests.
- `verify-library-media.mjs`: two-image move/resize/composition, exported PNG color pixels and alpha, portrait framing, close IPC, and absence of floating-overlay controls.
- Browser verification uses `tests/silent-browser.mjs`: headless Chromium with `--mute-audio`, muted media playback and disconnected Web Audio speaker output. No audible or visible native tests are permitted during the user's stream.
- Strict TypeScript check for the new media renderer, composition engine and video tile.

No visible native Windows capture, drag/drop or popup tests were run. Windows focus/compositor behavior and drag into real destination apps still need user-side confirmation after installation.

## 2026-10-03 stability review

- Re-copying an existing item refreshes its timestamp and ordering without losing its identity or pinned state.
- Expired unpinned entries no longer consume the history capacity before being removed.
- Failed video segments preserve completed text; retry skips previously completed audio. A preparation failure cannot become a misleading ready result.
- Transcript sidecar disk/database work happens before opening the Windows clipboard, reducing its lock duration.
- Verified: 9 library unit tests, 4 offline transcript unit tests, 3 media unit tests; silent browser suites for shelf/video playback and controls, composition export, and main library/settings. Live Groq, native microphone capture and third-party drop acceptance were not exercised in this revision.
