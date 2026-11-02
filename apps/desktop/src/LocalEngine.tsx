import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Check, Cloud, Cpu, Download, LoaderCircle, Trash2, X } from "lucide-react";
import { native, type Settings } from "./client";
import "./local-engine.css";

type LocalModel = { id: string; name: string; description: string; bytes: number; installed: boolean };
type Install = { phase: "" | "downloading" | "verifying" | "error"; model: string; downloaded: number; total: number; error: string };
type Status = { models: LocalModel[]; install: Install; device: string; loaded: string; error: string };

const size = (bytes: number) => bytes >= 1e9 ? `${(bytes / 1e9).toFixed(1)} GB` : `${Math.round(bytes / 1e6)} MB`;

// Engine choice plus the on-device model catalog: install, progress, remove.
export function LocalEngine({ settings, patch, onError }: { settings: Settings; patch: (value: Partial<Settings>) => void; onError: (message: string) => void }) {
  const [status, setStatus] = useState<Status>();
  const engine = settings.transcriptionEngine ?? "groq";
  const selected = settings.localModel ?? "parakeet-v3";
  const busy = status?.install.phase === "downloading" || status?.install.phase === "verifying";
  const refresh = () => { if (native) void invoke<Status>("local_models").then(setStatus).catch(() => {}); };
  useEffect(refresh, []);
  // Poll only while a download runs or the model is still loading.
  useEffect(() => {
    if (!busy && !(engine === "local" && status && !status.loaded && !status.error && status.models.some(m => m.id === selected && m.installed))) return;
    const id = setInterval(refresh, 400);
    return () => clearInterval(id);
  }, [busy, engine, selected, status?.loaded]);
  const install = (id: string) => {
    void invoke("local_model_install", { id }).then(() => { refresh(); patch({ localModel: id }); }).catch(e => { refresh(); if (!String(e).includes("cancelada")) onError(String(e)); });
    setTimeout(refresh, 100);
  };
  const remove = (id: string) => void invoke("local_model_remove", { id }).then(refresh).catch(e => onError(String(e)));
  const ready = status?.models.find(m => m.id === selected)?.installed ?? false;
  return <div className="local-engine">
    <div className="engine-choice" role="radiogroup" aria-label="Motor de transcripción">
      <button role="radio" aria-checked={engine === "groq"} onClick={() => patch({ transcriptionEngine: "groq" })}><Cloud size={18} /><span><strong>Groq</strong><small>En la nube · necesita clave e internet</small></span></button>
      <button role="radio" aria-checked={engine === "local"} onClick={() => patch({ transcriptionEngine: "local" })}><Cpu size={18} /><span><strong>Local</strong><small>En tu PC · sin internet, el audio no sale</small></span></button>
    </div>
    {engine === "local" && <>
      {!ready && <p className="local-hint">Instalá un modelo para dictar en local. La instalación incluye un detector de voz de 1,2 MB. El modo local no envía audio a Groq.</p>}
      {status?.loaded && <p className="local-device"><i />Listo en {status.device}</p>}
      {status?.error && <p className="local-model-error">{status.error}</p>}
      {!status?.loaded && !status?.error && ready && <p className="local-device" data-loading><LoaderCircle size={13} className="update-spinner" />Cargando el modelo…</p>}
      <div className="local-models">
        {(status?.models ?? []).map(model => {
          const current = status?.install.model === model.id ? status.install : undefined;
          const downloading = current && (current.phase === "downloading" || current.phase === "verifying");
          const percent = current && current.total ? Math.floor(current.downloaded / current.total * 100) : 0;
          return <div className="local-model" key={model.id} data-selected={model.installed && selected === model.id}>
            <button className="local-model-pick" disabled={!model.installed} aria-pressed={selected === model.id} onClick={() => patch({ localModel: model.id })}>
              <span className="local-model-radio">{model.installed && selected === model.id && <Check size={12} />}</span>
              <span><strong>{model.name}</strong><small>{model.description}</small></span>
            </button>
            <span className="local-model-size">{size(model.bytes)}</span>
            {downloading ? <button className="local-model-action" onClick={() => void invoke("local_model_cancel")} aria-label={`Cancelar descarga de ${model.name}`}><X size={15} />{current.phase === "verifying" ? "Verificando" : `${percent}%`}</button>
              : model.installed ? <button className="local-model-action" data-variant="ghost" disabled={busy} onClick={() => remove(model.id)} aria-label={`Borrar ${model.name}`}><Trash2 size={15} /></button>
              : <button className="local-model-action" disabled={busy || !native} onClick={() => install(model.id)}><Download size={15} />Instalar</button>}
            {downloading && <progress max={100} value={current.phase === "verifying" ? 100 : percent} aria-label={`Descarga de ${model.name}`} />}
            {current?.phase === "error" && <p className="local-model-error">{current.error}</p>}
          </div>;
        })}
      </div>
      <p className="local-hint">En Windows necesita un controlador compatible con Vulkan. El diccionario personal se sigue aplicando. Los archivos importados se convierten y transcriben en tu equipo. Para transcribir videos con Groq, elegí Groq y activá esa opción en Portapapeles.</p>
    </>}
  </div>;
}
