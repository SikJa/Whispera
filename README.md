# Whispera

[Español](README.es.md) | English

Floating voice dictation for Windows, built with Rust, Tauri and React.
Press a global shortcut, speak, and paste the transcription back into your application.
Bring your own Groq API key. No local AI model, Python runtime or Whispera account is required.

## Distribution status

Private beta preparation. This repository is intentionally private until the owner approves publication.
Windows x64 is the only target tested for this release. Installer and initial setup support English and Spanish;
the existing main settings interface is currently Spanish. This is not a fully translated application yet.
Installers are unsigned: Windows may display an unknown-publisher/SmartScreen warning.
Do not disable security protections. Only use artifacts from this repository and verify SHA-256 checksums.

## Install and configure

1. Download the NSIS `Whispera_*_x64-setup.exe` from an approved release.
2. Install for your Windows user. WebView2 is required; the installer can download it if missing.
3. The first launch opens setup. Create your own key at https://console.groq.com/keys and validate it in the app.
4. Check your Windows default microphone and microphone permissions. Choose audio language and shortcut.
5. Choose automatic paste and optional startup with Windows. Finish setup to continue in the tray.
6. Focus a text field, press `Control+Shift+Space` (default), speak and press again.

Later launches stay in the tray. Open Settings from the tray to revisit setup.
Pasting restores a native window/control, not a specific browser tab. Avoid switching fields while dictating.
Provider quotas, terms and availability apply; free access is not guaranteed by Whispera.

## Features

- Transparent draggable recorder, adjustable scale, glass folder animation, custom colors.
- Pause, system sound mute/restore, confirmed cancellation, start/stop sounds.
- Groq Whisper Large V3 Turbo or Large V3; multilingual recognition.
- Personal dictionary hints and post-transcription corrections; editable rules and duplicate validation.
- History, dedicated audio import, copy and plain-text editing.
- Durable recording files, retryable transcription segments and pending-audio recovery.
- Opt-in Windows startup, manual restart and limited idle-interface watchdog.

## Privacy

Audio and dictionary hints are transmitted to Groq when transcribing. The setup key check queries Groq's
model list without sending audio. Keys are stored under `Whispera.Desktop` in Windows Credential Manager.
Recordings, transcripts, dictionary and logs remain in `%APPDATA%\app.whispera.desktop`.
Local data is not encrypted by Whispera and is not automatically deleted, including cancelled recordings.
There is no Whispera analytics server. See [Privacy / Privacidad](docs/PRIVACY.md).

## Develop

Windows, Node.js 22+, Rust stable MSVC, Microsoft C++ Build Tools/Windows SDK and WebView2 are required.

```powershell
cd apps/desktop
npm ci
npm run desktop
```

```powershell
npm run build
cd src-tauri
cargo test --locked
cd ..
npm run tauri build -- --bundles nsis
```

The installer is generated under `src-tauri/target/release/bundle/nsis` (or `CARGO_TARGET_DIR`).
Never commit credentials or personal recordings. [Contributing](CONTRIBUTING.md), [Security](SECURITY.md),
[release checklist](docs/RELEASE.md), [third-party notices](THIRD_PARTY_NOTICES.md).

## License

MIT for Whispera-owned code. Third-party licenses and required notices remain applicable.
