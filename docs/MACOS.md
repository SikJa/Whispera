# Whispera for macOS / Whispera para macOS

## Status / Estado

The macOS port is a **prerelease**, not a replacement for the stable Windows release.
It targets **macOS 13 or later**, with separate downloads for **Apple Silicon** and
**Intel**. The Cristal logo, React interface, Groq integration and local data model
are shared with Windows. Native integrations use AppKit, Accessibility,
ScreenCaptureKit, CoreAudio and Keychain.

El port para Mac es una **version de prueba**. No reemplaza la version estable de
Windows ni cambia su canal de actualizaciones. No hubo acceso a una Mac fisica:
compilar y ejecutar tests en GitHub no demuestra que el microfono, los monitores,
el foco y el arrastre funcionen en todas las aplicaciones reales.

## Install / Instalar

1. Download the DMG matching your Mac: `aarch64` for Apple Silicon or `x86_64` for Intel.
2. Open the DMG and move Whispera to **Applications** before launching it.
3. Open Whispera from Applications. It lives in the **menu bar**, not permanently in the Dock.
4. Enter your own Groq key. Keys are stored in **macOS Keychain**, not in the repo.
5. In **Settings > Application > macOS permissions**, allow microphone, screen capture
   and Accessibility as needed. The same permissions are available during onboarding.
6. Choose your shortcuts. `Super` is displayed as **Cmd** on Mac. Windows-only Print
   Screen hooks are not offered on Mac. Avoid macOS shortcuts already in use.

Descarga el DMG de tu arquitectura, arrastra Whispera a **Aplicaciones** y abrela
desde ahi. Configura tu clave de Groq, permisos y atajos. El inicio al iniciar sesion
es opcional y se administra desde Configuracion. No se importa ninguna clave del
desarrollador ni la configuracion privada de otra persona.

### Signing / Firma

These builds are **ad-hoc signed, not Apple-notarized**. No Apple Developer account
was available. Gatekeeper can block the first launch; review the source and release
checksums, then use the per-app **Open Anyway** approval in macOS Privacy & Security
only if you trust this release. Do not disable Gatekeeper globally. Managed Macs
may require their administrator's approval. Permission approval may need repeating
after an ad-hoc update.

El aviso de Apple no se puede eliminar con la firma del actualizador. Son dos firmas
distintas: los paquetes de actualizacion se verifican con la clave publica de
SikJa/Whispera, pero no tienen identidad Developer ID ni notarizacion de Apple.

## Feature Validation / Validacion

The port includes the following implementations. **Every native row still needs
manual validation on a real Mac**, including both Retina and mixed-DPI monitors.

| Feature / Funcion | macOS implementation | Required real-device check |
| --- | --- | --- |
| Groq dictation, dictionary, incremental transcription, recovery | Shared Rust engine and CPAL microphone input | Record a long dictation; interrupt and recover |
| Destination and automatic paste | Accessibility focus snapshot, foreground app restore, Cmd+V | Text fields in browser, editor and chat apps |
| Floating recorder and hit areas | Transparent AppKit window and pointer hit testing | Drag, resize, switch Spaces, click through empty areas |
| Shortcuts and menu bar | Tauri global shortcuts and status item | System shortcut conflicts and input layouts |
| Start/end/completed sounds and system mute | Native audio helpers | Correct output device and restoring volume |
| Screenshots and annotations | ScreenCaptureKit with overlay filtering | Open menus, multiple displays, rounded PNG export |
| Video, microphone/system sound, replay and trimming | ScreenCaptureKit/CoreAudio with bundled FFmpeg | A/V sync, pause, stop, recovery and resource use |
| Clipboard history and file/image/video drag | NSPasteboard and native AppKit drag | Finder, browser uploads and chat composers |
| Image editor, backgrounds and composition | Shared React/canvas editor | Export, preview and trackpad interactions |
| Login startup | Per-user LaunchAgent through Tauri autostart | Logout/login and disabling in System Settings |
| Updates and profile protection | Signed preview feed, SQLite backup and Keychain | Upgrade between two preview builds without data loss |

Capturing protected/DRM windows is not guaranteed. Secure text fields can reject
Accessibility or paste. A destination app may reject a file type or multi-file
drop; supplying an MP4 plus transcript does not force a chat app to accept both.
Replay requires a working hardware H.264 encoder; it stays disabled when no compatible
encoder is available, avoiding continuous software encoding in the background.

## Build / Compilar

Requires a Mac with Xcode command-line tools, Rust stable and Node.js 22.

```sh
cd apps/desktop
npm ci
npm ci --ignore-scripts --prefix vendor/edge-drop
npm run build
cd src-tauri
cargo test --locked -- --test-threads=1
cd ..
npm run tauri build -- --bundles app,dmg --ci
```

For updater archives, provide `TAURI_SIGNING_PRIVATE_KEY` and its password securely
outside source control. GitHub CI injects the existing repository secret. Apple
Developer signing/notarization is a separate future configuration, not required
for compilation. The `macos-preview` workflow builds native ARM64 and Intel jobs,
checks Mach-O architectures, FFmpeg, bundle metadata and ad-hoc signatures.

Local headless UI tests mute audio and mock native IPC. Rust unit tests use fixtures;
permission prompts, actual recording, native dragging and automatic paste must not
be simulated as successful hardware tests.
The macOS jobs also run the shared interface in headless WebKit and start the
bundled app for a short startup smoke check on the runner. These checks exercise
the browser engine, dynamic linking and app initialization, not real-device recording.

## References / Referencias

- [Tauri macOS signing](https://v2.tauri.app/distribute/sign/macos/)
- [Tauri application bundles](https://v2.tauri.app/distribute/macos-application-bundle/)
- [Apple ScreenCaptureKit](https://developer.apple.com/documentation/screencapturekit/capturing-screen-content-in-macos)
- [Apple notarization](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)
