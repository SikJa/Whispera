# Unified Whispera

This working copy integrates the MIT-licensed capture/editor/video features from
[kazu00001/Whispera-K](https://github.com/kazu00001/Whispera-K) with the local
Whispera dictation engine, including incremental transcription and native retention.

## Local Compatibility

- App identifier: `app.whispera.desktop.preview`, matching the daily installation.
- Groq credential: existing Windows Credential Manager entry, never copied to a file.
- Existing dictation settings, dictionary, history and recordings remain in place.
- Capture preferences and recent capture references are imported once from
  `app.whispera.desktop`, in a transaction without modifying the source database.
- Conflicting shortcuts prevent migration; they do not prevent dictation startup.
- Both installations and their data remain available for rollback.
- The canonical executable remains `Whispera2.exe` to preserve startup and Windows
  transparency-tool exclusions.

## Checks (2026-10-02)

- TypeScript/Vite production build passed.
- Rust: 43 passed, 2 explicit opt-in tests skipped after the retention fix.
- Five browser verification suites passed using mocked IPC: capture, editor,
  history, hotkeys and setup.
- Isolated native capture test passed once: 640x400 PNG, zero pixel error;
  640x400 MP4 with annotations, mean RGB error 1.8483, controls excluded.
- Later native runs could not acquire the required Windows foreground focus.
  External-field paste and shortcut-field acquisition are not verified by those
  runs. No user audio was uploaded to Groq.
- Fresh cloud dictation and screen-video audio modes remain to be validated.

## Activation

Activated on 2026-10-02 after restart authorization, at the existing daily
executable path, with FFmpeg resources and the existing startup shortcut.
The old executable and both SQLite databases were backed up before replacement.
Capture preferences imported successfully; original dictation settings remain.

The first restart of the previous app invoked its existing 48-hour retention,
removing 117 expired audio sessions and 115 old transcript rows. All 115 texts
were restored from the pre-restart SQLite backup with their original timestamps.
Their IDs are protected by `unified_preserved_history`, so the next hourly cleanup
does not silently remove restored historical text. New audio retains the prior
48-hour cleanup policy. The deleted expired audio files were not recovered.
The native hidden-start check passed after the final replacement.

The old capture installation should not run alongside the unified app: it can
compete for the capture/video shortcuts. Do not uninstall or delete its data.

## Startup Audit (2026-10-02)

- Startup is now a single native `HKCU/Run/Whispera` entry targeting the canonical
  executable with `--autostart`, enabled in Windows StartupApproved.
- The former `Whispera 2.lnk` was moved to the startup backup, not deleted.
- Reinvoking `--autostart` keeps one running instance and no visible windows.
- No other Whispera startup shortcut, scheduled task or machine-wide Run entry
  was found. Unrelated applications were left untouched.
- Dictation preferences and all 13 dictionary entries match the original backup.
- All 115 historical texts and original dates remain unchanged; one new
  transcription completed in the unified app (116 entries at audit time).
- SQLite quick_check passed. Both existing credential entries are present.
- Capture/video preferences were retained; FFmpeg starts successfully.
- Paste, audio capture, floating-window handling, sounds and system-audio restore
  source match the previous app, ignoring CRLF/LF differences.
- A real Windows sign-in and end-to-end insertion into an external text field
  have not been independently tested in this audit.

## Linea Capture Controls (2026-10-02)

- Selected concept 01 is implemented in the native capture toolbar: 42px rail,
  no internal scrolling, compact mode and separate contextual panels.
- Panels expand the protected native host only while open, center on the
  triggering control, clamp to the selected monitor and keep the rail stationary.
  Closing restores the narrow host. The shared Adobe color picker is reused.
- Rectangle, ellipse, triangle, diamond, hexagon and star are real annotation
  tools, with aspect constraints, selection/movement, history and PNG/MP4 output.
- More contains undo, redo, delete selection and clear; it does not duplicate
  existing drawing tools. The video HUD uses time, pause/resume, stop and cancel.
- Six browser suites pass with mocked IPC; dedicated shape tests check actual
  exported PNG pixels. All three concepts also pass desktop/mobile previews.
- Isolated native QA passes panel bounds, stable rail position, shrinking the
  closed host, the custom palette and Rust tool acceptance. A real MP4 contains
  a star annotation; native pause/resume/stop controls work. No audio was sent.
- The older generic native capture script timed out when selecting an arrow via
  synthetic CDP click; the targeted native test selects the tool through IPC,
  draws through pointer events and exercises video controls through DOM clicks.
- Dictation, Groq, credentials, shortcuts, dictionary and startup are unchanged.
- Installed final release after restart authorization. Installed/release hashes
  match; one canonical process runs with no visible startup window. Settings,
  dictionary, capture preferences and all 119 transcripts match the pre-update
  backup. Final backup: `.local/desktop-backups/unified-20261002-172923` in the
  original `C:/Coding/Whispera` installation. Rust: 43 passed, 2 opt-in skipped.
