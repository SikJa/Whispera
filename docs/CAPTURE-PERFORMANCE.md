# Capture Performance

## Capture Controls (2026-10-02)

- Selection no longer displays pixel dimensions.
- Collapsed toolbars are centered using their compact height, not the expanded
  height. Video HUD stays horizontally centered on the region (monitor-edge
  clamping still applies), with symmetric padding and a camera action.
- `screen_video_snapshot` captures the current region including live ink, copies
  a rounded PNG and adds it to capture history without selecting a new region,
  stopping/pausing the video or replacing its editor context.
- Voice and video no longer mutually reject start actions. Each keeps its own
  recorder, gate, pause, cancellation and output. A device/backend may still
  refuse concurrent hardware streams; this was not tested with real devices.
- Verification for this change is headless Playwright with mocked IPC, Rust unit
  tests, and compilation. No interactive/native capture tests were run. The new
  `verify-capture-controls.mjs` is part of `npm test`.

## Popup Capture And Background Video Finalization

Still-image selection now snapshots each monitor before hiding/activating any
Whispera window. The selector displays that frozen PNG; image export crops the
same source instead of taking another live screenshot after the selection. This
preserves focus-sensitive popups even when their original window dismisses them.
Snapshots are session-local and released on capture/cancel. Video stays live.

Stop sends the capture worker its stop signal, hides the selector/ink/tools/HUD,
then waits for MP4 finalization in a blocking background task. Recovery files and
final clipboard/history behavior are unchanged.

Native QA `verify-frozen-menu.mjs` checks a colored popup that closes on blur,
forces blur if Windows denies automated focus transfer, and verifies its original
pixels are captured. It also verifies every capture window is hidden before the
stop command finishes, while the resulting MP4 remains nonempty.

### Lightshot Comparison (2026-10-02)

`scripts/benchmark-selection.ps1` measures synthetic PrintScreen key submission
to a visible full-screen HWND on the same Windows desktop. It does not measure
compositor presentation, human-perceived readiness, file export or copying.
Eight samples are taken; the first is excluded from the repeated-open median.

| Application | Repeated-open median (7 samples) |
| --- | ---: |
| Lightshot 5.5.0.7 | 153.57 ms |
| Whispera, frozen selector (installed release) | 195.99 ms |

Lightshot was 42.42 ms faster under this narrow metric. Whispera's first opening
after a fresh process launch took 739.89 ms. Lightshot was already warmed from an
earlier aborted run, so its first sample is not a comparable cold-start result.
The older live selector's 68.55 ms median is not an equivalent feature comparison:
it did not freeze popups and took its actual image after selection. Screen/layout
and concurrent load can affect these small samples. Lightshot was started only
for the benchmark and stopped afterwards; Whispera's shortcuts were unchanged.

## Rounded Image Exports (2026-10-02)

Image copy/save and immediate-copy paths apply the same 14 CSS-pixel radius as
CaptureFrame, scaled to physical pixels and clamped for small selections. The PNG
corners are transparent with 4x4 antialiasing; interior pixels and dimensions are
preserved. The editor canvas previews the same rounding. Video is unchanged.
Failed automatic copies reopen the original unmasked image, preventing a second
alpha mask on retry. `verify-rounded-capture.mjs` checks the actual PNG files from
both native copy paths under the isolated QA identifier.

## Changes (2026-10-02)

- One-shot screenshots and blur samples use Windows GDI in-process, followed by
  lossless PNG encoding in memory. No capture subprocess or temporary image is
  required on the normal path. The previous FFmpeg path remains a fallback.
- Native GDI handles are released on both success and error. Dimensions are
  checked before allocation; desktop BGRA becomes opaque RGBA without rescaling.
- DWM is synchronized before capture. Previous image-editor ink is temporarily
  excluded from image capture so close animations cannot contaminate a new image.
  Its original display affinity is restored. Video blur sampling never changes
  ink affinity, keeping live annotations visible in recorded video.
- Video PCM serialization reuses a scratch allocation. Encoded sample bytes and
  immediate writes remain unchanged. Pause/finalization still preserves PCM and
  intermediate video until the final MP4 has been verified.
- The video counter publishes at most every 250ms instead of every 25ms; final
  timestamps and state transitions are still immediate. This reduces periodic
  counter locking, not the 30 FPS capture/encoding rate.
- Video codec, CRF 18, resolution, audio format and remux behavior are unchanged.

## Local Measurement

`apps/desktop/benchmark-capture.mjs` runs only against isolated
`app.whispera.desktop.unifiedqa`. It creates a synthetic 640x400 fixture in the
QA app. It does not capture another application's contents or send audio.
Two image warmups precede seven timed samples; video has three two-second trials
without audio. Both app variants use debug builds with the same FFmpeg binary.

| Metric | Baseline | Optimized |
| --- | ---: | ---: |
| Image editor ready, median | 317.01ms | 169.96ms |
| Image mean RGB error, worst repeated capture | 0.944 | 0.000 |
| Video start, median | 135.28ms | 143.47ms |
| Video stop, median | 219.21ms | 169.66ms |

Image improvement is about 46.4% in this fixture. Video measurements varied
between runs and have only three samples; no repeatable video speedup or overall
CPU improvement is claimed. GDI objects remained at 58 before/after seven repeated
image captures, with no growth in this check.
Results are local measurements, not guarantees across screen sizes or systems.

Raw reports: `.local/benchmark-baseline.json` and
`.local/benchmark-optimized.json` (not intended for distribution).

## API Reference

- [Microsoft: Capturing an Image](https://learn.microsoft.com/en-us/windows/win32/gdi/capturing-an-image)
- [Microsoft: GetDIBits](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-getdibits)

## Verification

Rust tests cover channel conversion, size bounds and byte-identical PCM scratch
reuse. Browser suites cover editor/export/undo/redo and all existing settings.
Native tests cover repeated screenshot pixel accuracy, panel bounds, a real
annotated MP4, blur samples, pause, resume and stop. Hardware microphone/system
audio smoke tests remain opt-in; the optimization does not change those formats.

## Activation

Installed after restart authorization on 2026-10-02. Release and installed hashes
match; only the canonical process runs, with no visible startup window. Original
settings, dictionary, capture preferences (including PrintScreen) and history
match the pre-update SQLite backup. Backup directory:
`C:/Coding/Whispera/.local/desktop-backups/unified-20261002-184455`.
Final Rust suite: 46 passed, 2 opt-in skipped; six browser suites passed.
# Concurrent Still Images (2026-10-02)

Still-image selection and editing no longer acquire the dictation gate or reject
an active/paused/processing voice session. Starting voice dictation is also
permitted while a still-image editor is open. Voice/video overlap is now also
allowed (hardware verification limits above). Reused selectors reassert z-order after
showing, including over Whispera Settings; Settings is not hidden or excluded.

`verify-concurrent-capture.mjs` uses the isolated `unifiedqa` identifier. It opens
the real Settings surface, triggers the global screenshot shortcut while idle
and while the microphone records, compares captured Settings pixels, checks
session continuity and advancing time, captures while paused, then cancels the
test audio without submitting it to Groq. Its historical voice/video rejection
assertion predates the later concurrent-recording support and is not a current
release acceptance test. This is not a speed comparison against Lightshot.
