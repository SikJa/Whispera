import { useState } from "react";
import { Button, Dialog, DialogTrigger, Popover, Heading, ColorPicker, ColorArea, ColorThumb, ColorSlider, SliderTrack, ColorField, Input, Label } from "react-aria-components";
import { ChevronDown, X, Check } from "lucide-react";
import { presets, paletteLabel, selectionHex, type Selection } from "./palette";
import "./palette.css";

export function PaletteContents({ value, onChange, title = "Color de carpeta", close }: { value: Selection; onChange: (value: Selection) => void; title?: string; close: () => void }) {
  const [tab, setTab] = useState<"presets" | "custom">("presets");
  return <>
          <div className="palette-heading"><Heading slot="title">{title}</Heading><Button className="palette-close" aria-label="Cerrar paleta" onPress={close}><X size={17} /></Button></div>
          <div className="segmented palette-tabs" aria-label="Tipo de color"><button aria-pressed={tab === "presets"} onClick={() => setTab("presets")}>Predefinidos</button><button aria-pressed={tab === "custom"} onClick={() => setTab("custom")}>Personalizado</button></div>
          {tab === "presets" ? <div className="palette-presets">{presets.map(p => {
            const selected = paletteLabel(value) === p.name;
            return <button key={p.name} aria-label={p.name} aria-pressed={selected} onClick={() => onChange({ base: p.base, ...(p.custom ? { hex: p.hex } : {}) })}>
              <span className="preset-color" style={{ background: p.hex }} /> <span>{p.name}</span>{selected && <Check size={15} />}
            </button>;
          })}</div> : <ColorPicker value={selectionHex(value)} onChange={color => onChange({ base: "black", hex: color.toString("hex") })}>
            <ColorArea className="palette-area" colorSpace="hsb" xChannel="saturation" yChannel="brightness" aria-label="Saturación y brillo"><ColorThumb className="palette-thumb" /></ColorArea>
            <ColorSlider className="palette-hue" colorSpace="hsb" channel="hue" aria-label="Tono"><SliderTrack className="palette-track"><ColorThumb className="palette-thumb" /></SliderTrack></ColorSlider>
            <ColorField className="palette-field" aria-label="Código HEX"><Label>HEX</Label><Input spellCheck={false} /></ColorField>
          </ColorPicker>}
        </>;
}
export default function PalettePanel({ value, onChange, title = "Color de carpeta" }: { value: Selection; onChange: (value: Selection) => void; title?: string }) {
  return <DialogTrigger>
    <Button className="palette-trigger" aria-label="Elegir color"><i style={{ background: selectionHex(value) }} />{paletteLabel(value)}<ChevronDown size={15} /></Button>
    <Popover className="palette-popover" placement="top start" offset={12} containerPadding={12}>
      <Dialog className="palette-dialog">{({ close }) => <PaletteContents value={value} onChange={onChange} title={title} close={close}/>}</Dialog>
    </Popover>
  </DialogTrigger>;
}
