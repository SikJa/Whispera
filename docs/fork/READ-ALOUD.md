# Local reading preview — 0.3.0-readaloud.10

This fork adds Windows text-to-speech independently of Groq transcription.

- Choose **Use reading without Groq** in setup, then **Lectura**.
- Paste or type up to 50,000 UTF-16 text units; choose an installed Windows desktop voice and speed, then Read, Pause/Continue or Stop.
- Open a transcript in History to listen to its current text, including edits.
- The reader does not save text or send it to a provider. Only voice/speed preferences are stored locally. The latest successfully spoken text and its voice/speed are retained in memory until Whispera exits, so shortcut selections can be exported explicitly. Existing transcript history and an explicitly enabled clipboard history retain their own behavior.
- An installed Spanish desktop voice is required for Spanish narration. Windows may offer other narrator voices that are not exposed to the legacy desktop synthesis API; availability is shown in the list.
- Dictation/screen recording/replay stops the reader. A running capture prevents starting playback. Closing the reader panel stops playback.
- The local reader is currently Windows-only. macOS shows an unavailable state.
- Select text in a compatible Windows application and press **Ctrl+Alt+R** to read it; press again to stop. Change it in **Atajos → Lectura en voz alta**. Selection uses UI Automation: no simulated Copy, clipboard reads or clipboard changes. Password fields, empty selections, unsupported apps and selections over the limit are rejected. The shortcut uses the voice/speed most recently chosen in Lectura.
- Right-click the Whispera icon next to the Windows clock to **Pausar lectura**, **Continuar lectura** or **Detener lectura**. These controls also work for selection-shortcut playback with Settings hidden. Pause preserves the current position; Stop ends playback. Controls disable after completion or Stop, and do not start a reader when idle.
- This is not a screen reader, OCR or PDF importer. Apps must expose their selection through Windows accessibility. No full-document or clipboard fallback is used.

## Windows Terminal with WSL, Claude or Codex

When an application inside Windows Terminal handles mouse input, dragging may
interact with that application instead of making a native terminal selection.
Hold **Shift** while dragging, release Shift, then press the configured reading
shortcut while Windows Terminal remains active. Microsoft documents this behavior
in [Windows Terminal selection](https://learn.microsoft.com/en-us/windows/terminal/selection).
This procedure was confirmed to work in the reported WSL session (issue #3).

No new capture mechanism is needed for that case. Version 0.3.0-readaloud.10 keeps
the original focused-control selection path and adds an explanatory Shift hint
only when the active window is Windows Terminal and accessibility reports an
empty/unavailable selection. Successful reads, other applications, password and
size rejections, foreground checks and the two-second timeout are preserved.
There is no broader window search, simulated Copy, clipboard access, WSL command,
new permission or service. The workaround also works on the previous version.

For troubleshooting, select a complete sentence and temporarily choose a manual
voice in Lectura. Automatic language detection can independently reject short
fragments or code with an uncertain language; it does not mean selection failed.

## Automatic language and voice

Select **Automático · detectar idioma** under Voz. This is the default for a new
profile; an existing manually selected voice and speed are preserved on upgrade.
Choose Automatic explicitly to enable detection for an existing profile. Before
reading, a local preview shows the detected language and compatible installed voice.
No text is sent to Groq or any other service, and no subscription quota is used.

The same native selector handles the editor, History, Ctrl+Alt+R selections, and
new MP3 exports. It detects one predominant language for the entire text; it does
not change voices between sentences. It ignores Windows' interface language and
does not infer your preferred regional accent. Mixed-language text may be classified
by its predominant language and is less predictable than a single-language passage.

Expand **Voces preferidas por idioma** to choose the accent/voice for each available
language. Switching from a manual voice to Automatic initially preserves that voice
as the preference for its language if no preference exists. If a preferred voice
is removed, another installed voice of the same language is used. A manual choice
always overrides detection, including for very short or mixed-language text.

The detector is whatlang 0.18.0, pinned in Cargo.lock with compiled-in language
models (70 supported languages). Windows voices provide ISO language codes, with
aliases normalized for Mandarin, Persian, Norwegian and Filipino. Detection is
never restricted to installed voice languages: a detected French passage without
a French voice is reported as missing, not guessed as English or Spanish.

Insufficiently reliable detection, fewer than 12 alphabetic characters, or fewer
than three words in Latin script require a manual voice. Missing compatible voices
also prevent automatic playback/export. The panel explains how to proceed; the
shortcut opens the existing reader error notice. Installing a new Windows desktop
voice may require restarting Whispera so its speech helper discovers that voice.
Unsupported detector languages can still be read with an explicitly chosen voice.

Only voice IDs, speed, and per-language voice preferences are persisted. Preferences
load from the native store before the UI can save them, preserving existing manual
settings. Local previews are debounced, and late results for an older text or voice
configuration cannot enable playback or replace the current preview.

Once playback begins, its actual resolved voice is stored with the latest reading
in memory. Saving that reading from the tray or while it is playing/paused keeps
that exact voice even if the editor, automatic detection, or preferences change.
There is no new inference service, login, runtime model download or background
document scan. Dependency attribution is bundled in the licenses directory.

## Save a reading as MP3

Choose **Guardar como MP3** in Lectura or an open transcript, then select a filename
and destination in the Windows save dialog. No need to play the text first or wait
for playback to finish. While speaking/paused, the export uses the complete current
reading and its original voice/speed, even if the editor has since changed. Otherwise,
it exports the editor's text and chosen voice/speed. If the editor is empty, it can
export the latest successful reading still in memory.

For a selection read through Ctrl+Alt+R, right-click Whispera's tray icon and choose
**Guardar lectura como MP3…**. This always uses the latest successful reading,
including one that has already finished or been stopped. It is disabled before any
reading has started. **Cancelar guardado MP3** cancels an active export; the reading
panel also shows a cancel button. Closing the save dialog cancels without exporting.

Export regenerates the entire narration locally, using a separate Windows speech
process and Whispera's already bundled FFmpeg MP3 encoder (128 kbps). It does not
record the speakers, include pauses/interruption sounds, call Groq/AI services, or
use subscription quota. Playback's pause/resume/stop controls remain independent.
The audio is saved only at the explicitly chosen destination and is not added to
history or uploaded. No new runtime/service/dependency is installed.

A temporary staging folder beside the destination contains the intermediate WAV
and MP3, and is removed on normal completion, failure, or cancellation. The final
MP3 replaces the selected filename only after successful encoding; a failed export
leaves an existing destination intact. Windows asks before replacing an existing
file. Synthesis/encoding have time limits and cancellation kills only this export's
own processes. An interrupted app/computer can leave its staging folder behind.

## Privacy changes

New profiles start with global clipboard collection off and video transcription/attachments off. Existing explicit settings are preserved. Video recropping checks the transcription preference before sending audio to Groq. Upstream auto-update checks/installations are disabled so the original app cannot overwrite this experimental fork.

The earlier audit's other retention/backups/dependency issues are not all fixed by this branch. Do not interpret local reading as making every Whispera function offline.

## Building and validation

Use the existing Node 22+/Rust MSVC/WebView2 build instructions on Windows. `build.rs` compiles `native/LocalReader.cs` with the existing Windows .NET Framework compiler and System.Speech. The result stays under ignored `src-tauri/target/reader` and is bundled as `bin/WhisperaReader.exe`. No binary is committed and no speech service/key is added.

`node tests/reader-protocol.mjs src-tauri/target/reader/WhisperaReader.exe` exercises the protocol silently. `node tests/reader-mp3.mjs src-tauri/target/reader/WhisperaReader.exe node_modules/ffmpeg-static/ffmpeg.exe` synthesizes a temporary WAV and verifies an actual decodable MP3 without playing it. Native Rust tests cover bounds, staging cleanup and cancellation of a stalled process/pipe; the browser fixture covers export, cancellation, save errors, controls and reading edited history with mocked IPC. Full audible voice quality requires a manual test with non-sensitive text; automated checks intentionally do not play sound.

The Windows CI workflow builds an unsigned experimental installer. A successful workflow is required before treating the installer as validated. No release is automatically published from this fork. The upstream macOS release workflow is restricted to the original repository; this preview delivers a Windows installer only.

## Buscar opciones y consultar la versión

En Configuración, **Buscar opciones** localiza secciones y ajustes por nombre o términos habituales (voz, micrófono, atajo, saldo, versión). No distingue mayúsculas ni tildes. El resultado muestra su ubicación; al seleccionarlo abre la sección, también la pestaña de ajustes del portapapeles cuando corresponde, y resalta la opción durante unos segundos.

Se puede abrir el buscador con **Ctrl+K** (⌘K en macOS), recorrer los resultados con flechas, elegir con Enter y cerrar con Escape. Este atajo solo actúa dentro de Configuración y no interfiere con los campos donde se graban atajos. La búsqueda usa únicamente un catálogo local de opciones; no consulta ni guarda contenido del portapapeles, transcripciones o datos de cuentas.

**Configuración → Aplicación** muestra la versión instalada obtenida del motor nativo. También aparece en Actualizaciones. La vista del navegador se identifica como vista previa.
