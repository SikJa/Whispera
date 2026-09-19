import { useEffect, useState, type ReactNode, type CSSProperties } from "react";
import { contrastInk } from "./palette";
import { Liquid } from "liquid-gooey";
import { Volume2, VolumeX, Pause, Play, Square } from "lucide-react";

export type Placement = "right" | "top" | "left" | "bottom";
function Swap({ active, a, b }: { active: boolean; a: ReactNode; b: ReactNode }) {
  return <span className="t-icon-swap" data-state={active ? "b" : "a"} aria-hidden="true"><span className="t-icon" data-icon="a">{a}</span><span className="t-icon" data-icon="b">{b}</span></span>;
}

export default function FolderControls({ color, customColor, paused, muted, open, placement, onMute, onPause, onStop }: {
  color: "black" | "white" | "blue"; paused: boolean; muted: boolean; open: boolean; placement: Placement;
  onMute: () => void; onPause: () => void; onStop: () => void;
  customColor?: string;
}) {
  const [moving, setMoving] = useState(false);
  useEffect(() => {
    setMoving(true);
    const timer = window.setTimeout(() => setMoving(false), 850);
    return () => window.clearTimeout(timer);
  }, [open, placement]);
  // Alpha must be applied AFTER the goo filter, or alpha contrast erases the gel.
  const fill = customColor ?? { black: "#292929", white: "#f5f5f5", blue: "#3a9ae8" }[color];
  // Start and finish inside the folder, beyond the edge absorption mask.
  const origin = { right: [240, 125], left: [30, 125], top: [134, 50], bottom: [134, 180] }[placement];
  const offset = (index: number) => ({ right: [136, (index - 1) * 70], left: [-137, (index - 1) * 70], top: [(index - 1) * 70, -286], bottom: [(index - 1) * 70, 120] })[placement];
  return <div className="folder-controls" data-color={customColor ? "custom" : color} data-open={open} data-placement={placement}
    style={customColor ? { "--custom-ink": contrastInk(customColor, .65), "--custom-ink-light": contrastInk(customColor, .65, 222), "--custom-edge": `color-mix(in srgb, ${customColor}, white 45%)` } as CSSProperties : undefined}>
    <Liquid key={placement} className="liquid-controls" blur={moving ? 10 : 0} contrast={18} fill={fill} filterPadding={40}
      shadow="0 7px 18px rgba(0,0,0,.20), inset 0 1px 2px rgba(255,255,255,.80)">
      {[
        { name: muted ? "Restaurar sonido" : "Mutear sonido", icon: <Swap active={muted} a={<Volume2 />} b={<VolumeX />} />, run: onMute, active: muted },
        { name: paused ? "Reanudar" : "Pausar", icon: <Swap active={paused} a={<Pause fill="currentColor" strokeWidth={1.5} />} b={<Play fill="currentColor" strokeWidth={1.5} />} />, run: onPause, active: paused },
        { name: "Detener grabación", icon: <Square size={18} fill="currentColor" strokeWidth={1.5} />, run: onStop, active: false },
      ].map((item, index) => <Liquid.Item key={index} x={open ? offset(index)[0] : 0} y={open ? offset(index)[1] : 0}
        transition={{ stiffness: 200, damping: 20, mass: 1 }} delay={open ? index * 45 : (2 - index) * 25} radius={18}
        style={{ position: "absolute", left: 150 + origin[0], top: 260 + origin[1] }}>
        <button className="glass-control satellite" title={item.name} aria-label={item.name} aria-pressed={index < 2 ? item.active : undefined}
          aria-hidden={!open} tabIndex={open ? 0 : -1} disabled={!open} onClick={item.run}>
          <span className="control-content" data-visible={open}>{item.icon}</span>
          <span className="control-tooltip" role="tooltip">{item.name}</span>
        </button>
      </Liquid.Item>)}
    </Liquid>
  </div>;
}
