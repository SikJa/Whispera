<p align="center">
  <img src="apps/desktop/public/cristal/128x128.png" width="80" alt="Whispera" />
</p>

<h1 align="center">Whispera</h1>

<p align="center">
  Floating voice dictation for Windows.<br/>
  Press a shortcut, speak, and paste the transcription wherever you need it.
</p>

<p align="center">
  <a href="README.md">Español</a>&ensp;·&ensp;
  <a href="https://console.groq.com/keys">Get a Groq key</a>&ensp;·&ensp;
  <a href="docs/PRIVACY.md">Privacy</a>
</p>

---

<p align="center">
  <img src="docs/screenshots/recorder-active.png" width="360" alt="Floating recorder in action" />
  &emsp;
  <img src="docs/screenshots/settings-dark.png" width="420" alt="Settings panel" />
</p>

## What is it?

Whispera is a Windows desktop app that turns your voice into text using [Groq](https://groq.com).
It shows up as a **transparent floating folder** on top of any window. Record, transcribe, and auto-paste — all from a keyboard shortcut.

- 🎙️ **No local models** — uses the Groq API (Whisper Large V3 Turbo)
- 🔑 **Your own key** — no Whispera account, no server of ours
- 🪟 **Native on Windows** — Tauri + Rust + React, ~6 MB installer
- 🌐 **Spanish & English** — interface, installer and setup wizard are bilingual

## Features

**New: transcription while you speak.** Whispera processes long recordings in the background
using 60-second segments. Enabled by default; you can disable it in Settings.
In a three-minute test, post-stop waiting fell from **2.67 s to 0.92 s (65.5%)**.
This is a single measurement, not a speed or accuracy guarantee.
Read about the [implementation, safeguards and methodology](docs/INCREMENTAL-TRANSCRIPTION.md).
Cancelling cannot withdraw audio already sent to Groq.

| | |
|---|---|
| 🎨 **Customizable folder** | Color, scale, control placement, animations |
| ⏸️ **Pause & cancel** | With confirmation and system audio restore |
| 📂 **Transcribe files** | Open an audio file and transcribe it without recording |
| 📖 **Personal dictionary** | Names and terms the recognizer should respect |
| 📋 **History** | Every transcription, editable and copyable |
| 🔊 **Sounds** | Custom start/stop themes (cristal, marimba, pop…) |
| 💾 **Recovery** | Audio saved progressively, per-segment retries |
| 🚀 **Start with Windows** | Launches hidden in the system tray |

## Install

1. Download `Whispera_*_x64-setup.exe` from [Releases](../../releases)
2. Install for your user — no admin rights needed
3. First launch opens a **4-step setup wizard**:

| Step | What you do |
|:---:|---|
| 🛡️ | **Privacy** — Review how your audio and data are handled |
| 🔑 | **Groq** — Create your key at [console.groq.com/keys](https://console.groq.com/keys) and validate it |
| 🎤 | **Preferences** — Choose microphone, language, shortcut and auto-paste |
| ✅ | **Ready** — Whispera minimizes to the tray, ready to dictate |

4. Click a text field, press `Ctrl+Shift+Space`, speak, and press again

---

### 🔑 How to get your Groq key (step by step)

<details>
<summary>See guide with screenshots</summary>

<br/>

**Step 1 — Create a Groq account**

Go to [console.groq.com](https://console.groq.com) and sign up with Google or your email. It's free.

<img src="docs/groq-setup/01-login.png" width="520" alt="GroqCloud login page" />

<br/><br/>

**Step 2 — Go to API Keys**

Once logged in, click **API Keys** in the navigation menu.

<img src="docs/groq-setup/02-api-keys.png" width="520" alt="Groq API Keys page" />

<br/><br/>

**Step 3 — Create a new key**

Click the **Create API Key** button.

<img src="docs/groq-setup/03-create-key.png" width="520" alt="Create API Key button" />

<br/><br/>

**Step 4 — Name your key**

Type a name for your key (e.g. "Whispera") and confirm.

<img src="docs/groq-setup/04-name-key.png" width="520" alt="Name dialog for the API Key" />

<br/><br/>

**Step 5 — Copy the key**

Your key is shown only once. Copy it and paste it in Whispera. It starts with `gsk_`.

<img src="docs/groq-setup/05-copy-key.png" width="520" alt="Generated API key to copy" />

<br/><br/>

> ⚠️ **Important:** the key is not shown again. If you lose it, create a new one.

</details>

---

## What each user configures

| Setting | Required? | Details |
|---|:---:|---|
| **Groq API key** | Yes | Stored in Windows Credential Manager, never in files |
| **Microphone** | — | Uses the Windows default; check permissions |
| **Shortcut** | — | `Ctrl+Shift+Space` by default, customizable |
| **Audio language** | — | Spanish by default; English, Portuguese or auto |
| **Auto-paste** | — | Enabled by default; you can copy-only instead |
| **Start with Windows** | — | Optional, launches in the tray |
| **Dictionary** | — | Starts empty; add your words later |
| **Color & sounds** | — | Optional, several built-in themes |

## Privacy

- Audio and dictionary vocabulary are sent to **Groq** when transcribing (HTTPS)
- Your key is stored in **Windows Credential Manager**, not in text files
- Recordings, history and logs stay in `%APPDATA%\app.whispera.desktop`
- **No Whispera server** — no analytics, no telemetry, no account
- Local data is **not automatically deleted** (including cancelled recordings)

More → [docs/PRIVACY.md](docs/PRIVACY.md)

## Development

Requirements: Windows, Node.js 22+, Rust stable (MSVC), C++ Build Tools, WebView2.

```bash
cd apps/desktop
npm ci
npm run desktop        # dev with hot-reload
```

```bash
npm run tauri build -- --bundles nsis    # generate installer
cd src-tauri && cargo test --locked      # Rust tests
```

The installer is generated at `src-tauri/target/release/bundle/nsis/`.

## License

MIT — see [LICENSE](LICENSE).
Third-party attributions at [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
