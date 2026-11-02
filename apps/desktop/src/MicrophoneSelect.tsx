import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { native } from "./client";

// Devices are re-read on focus so a headset plugged in later appears without reopening.
export function MicrophoneSelect({ value, onChange }: { value: string; onChange: (value: string) => void }) {
  const [devices, setDevices] = useState<string[]>([]);
  const load = () => { if (native) void invoke<string[]>("audio_input_devices").then(list => setDevices(Array.isArray(list) ? list : [])).catch(() => setDevices([])); };
  useEffect(load, []);
  const missing = Boolean(value) && devices.length > 0 && !devices.includes(value);
  return <div className="form-row">
    <label htmlFor="microphone">Micrófono<span>{missing ? "No está conectado: se usa el predeterminado." : "Se usa al dictar."}</span></label>
    <select id="microphone" value={value} onFocus={load} onChange={e => onChange(e.target.value)}>
      <option value="">Predeterminado del sistema</option>
      {missing && <option value={value}>{value}</option>}
      {devices.map(device => <option key={device} value={device}>{device}</option>)}
    </select>
  </div>;
}
