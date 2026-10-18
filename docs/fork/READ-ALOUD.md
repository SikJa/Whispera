# Local reading preview — 0.3.0-readaloud.3

This fork adds Windows text-to-speech independently of Groq transcription.

- Choose **Use reading without Groq** in setup, then **Lectura**.
- Paste or type up to 50,000 UTF-16 text units; choose an installed Windows desktop voice and speed, then Read, Pause/Continue or Stop.
- Open a transcript in History to listen to its current text, including edits.
- The reader does not save text or send it to a provider. Only voice/speed preferences are stored locally. Existing transcript history and an explicitly enabled clipboard history retain their own behavior.
- An installed Spanish desktop voice is required for Spanish narration. Windows may offer other narrator voices that are not exposed to the legacy desktop synthesis API; availability is shown in the list.
- Dictation/screen recording/replay stops the reader. A running capture prevents starting playback. Closing the reader panel stops playback.
- The local reader is currently Windows-only. macOS shows an unavailable state.
- Select text in a compatible Windows application and press **Ctrl+Alt+R** to read it; press again to stop. Change it in **Atajos → Lectura en voz alta**. Selection uses UI Automation: no simulated Copy, clipboard reads or clipboard changes. Password fields, empty selections, unsupported apps and selections over the limit are rejected. The shortcut uses the voice/speed most recently chosen in Lectura.
- Right-click the Whispera icon next to the Windows clock to **Pausar lectura**, **Continuar lectura** or **Detener lectura**. These controls also work for selection-shortcut playback with Settings hidden. Pause preserves the current position; Stop ends playback. Controls disable after completion or Stop, and do not start a reader when idle.
- This is not a screen reader, OCR or PDF importer. Apps must expose their selection through Windows accessibility. No full-document or clipboard fallback is used.

## Privacy changes

New profiles start with global clipboard collection off and video transcription/attachments off. Existing explicit settings are preserved. Video recropping checks the transcription preference before sending audio to Groq. Upstream auto-update checks/installations are disabled so the original app cannot overwrite this experimental fork.

The earlier audit's other retention/backups/dependency issues are not all fixed by this branch. Do not interpret local reading as making every Whispera function offline.

## Building and validation

Use the existing Node 22+/Rust MSVC/WebView2 build instructions on Windows. `build.rs` compiles `native/LocalReader.cs` with the existing Windows .NET Framework compiler and System.Speech. The result stays under ignored `src-tauri/target/reader` and is bundled as `bin/WhisperaReader.exe`. No binary is committed and no speech service/key is added.

`node tests/reader-protocol.mjs src-tauri/target/reader/WhisperaReader.exe` exercises the protocol silently. Native Rust input tests cover bounds; the browser fixture covers controls and reading edited history with mocked IPC. Full audible voice quality requires a manual test with non-sensitive text; automated checks intentionally do not play sound.

The Windows CI workflow builds an unsigned experimental installer. A successful workflow is required before treating the installer as validated. No release is automatically published from this fork. The upstream macOS release workflow is restricted to the original repository; this preview delivers a Windows installer only.
