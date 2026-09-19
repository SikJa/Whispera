import { invoke } from "@tauri-apps/api/core";
export const native = "__TAURI_INTERNALS__" in window;
export type Settings = { model: string; language: string; hotkey: string; color: string; pattern: "wave" | "stairs"; placement: "right" | "left" | "top" | "bottom"; autoCopy: boolean; autoPaste: boolean; soundTheme: string; sounds: boolean; recorderScale: number; trimSilence: boolean; watchdog: boolean };
export type Rule = { id: string; source: string; target: string; enabled: boolean };
export type Transcript = { id: string; timestamp: string; text: string };
export type Snapshot = { settings: Settings; rules: Rule[]; history: Transcript[]; keyConfigured: boolean; logs: string[]; native: boolean };
const defaults: Snapshot = { settings: { model: "whisper-large-v3-turbo", language: "es", hotkey: "|", color: "#9024DC", pattern: "wave", placement: "right", autoCopy: true, autoPaste: true, soundTheme: 'cristal', sounds: true, recorderScale: .85, trimSilence: true, watchdog: true }, rules: [], history: [], keyConfigured: false, logs: [], native: false };
let preview: Snapshot = structuredClone(defaults);
try { const stored = localStorage.getItem("whispera-v2-preview"); if (stored) preview = { ...defaults, ...JSON.parse(stored), native: false, keyConfigured: false }; } catch { /* Keep empty preview on invalid storage. */ }
const persist = () => { try { localStorage.setItem("whispera-v2-preview", JSON.stringify(preview)); } catch { /* Optional preview persistence. */ } };
export async function snapshot(): Promise<Snapshot> { if(native)return invoke("snapshot"); return structuredClone({...preview,settings:{...defaults.settings,...preview.settings}}); }
export async function saveSettings(settings: Settings) { if (native) await invoke("save_settings", { settings }); else { preview.settings = settings; persist(); } }
export async function saveRules(rules: Rule[]) {
  const seen = new Set<string>();
  for(const r of rules){const key=r.source.trim().toLowerCase(); if(!key||!r.target.trim())throw Error('Completá ambos campos.'); if(key===r.target.trim().toLowerCase())throw Error('La corrección debe cambiar la palabra.'); if(seen.has(key))throw Error('Ya existe una regla para esa palabra.'); seen.add(key);}
  if (native) await invoke("save_rules", { rules }); else { preview.rules = rules; persist(); }
}
export async function saveKey(key: string) { if (!native) throw Error("La clave solo se guarda desde la app nativa, nunca en esta vista previa."); await invoke("save_api_key", { key }); }
export async function transcribeFile() {
  if (!native) throw Error("La transcripción de archivos requiere la app nativa.");
  const { open } = await import("@tauri-apps/plugin-dialog");
  const path = await open({ multiple: false, filters: [{ name: "Audio", extensions: ["wav", "mp3", "m4a", "ogg", "webm", "flac"] }] });
  if (!path) return;
  return invoke<string>("transcribe_file", { path });
}
export async function importLegacy() {
  if (!native) throw Error("La importación requiere la app nativa.");
  const { open } = await import("@tauri-apps/plugin-dialog");
  const path = await open({ directory: true, multiple: false, title: "Seleccionar carpeta .local de Whispera anterior" });
  if (path) return invoke<string>("import_legacy", { path });
}
export async function importKey() {
  if (!native) throw Error("La importación requiere la app nativa.");
  const { open } = await import("@tauri-apps/plugin-dialog");
  const path = await open({ directory: true, multiple: false, title: "Seleccionar carpeta .local de Whispera anterior" });
  if (path) await invoke("import_groq_key", { path });
}
