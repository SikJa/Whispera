export type BaseColor = "black" | "white" | "blue";
export type Selection = { base: BaseColor; hex?: string };
export const presets: { name: string; hex: string; base: BaseColor; custom?: boolean }[] = [
  { name: "Negro", hex: "#000000", base: "black" },
  { name: "Blanco", hex: "#F3F3F3", base: "white" },
  { name: "Azul", hex: "#50B1FD", base: "blue" },
  { name: "Rojo", hex: "#F51C28", base: "black", custom: true },
  { name: "Bordó", hex: "#800020", base: "black", custom: true },
  { name: "Morado", hex: "#9024DC", base: "black", custom: true },
  { name: "Dorado", hex: "#E5B322", base: "black", custom: true },
];
export const validHex = (value: unknown): value is string => typeof value === "string" && /^#[\da-f]{6}$/i.test(value);
export function loadSelection(): Selection {
  try {
    const value = JSON.parse(localStorage.getItem("whispera-folder-demo-color") || "null");
    if (value && ["black", "white", "blue"].includes(value.base) && (value.hex === undefined || validHex(value.hex))) return value;
  } catch { /* Storage may be unavailable; the demo still works. */ }
  return { base: "black" };
}
export function paletteLabel(selection: Selection) {
  return presets.find(p => p.custom ? p.hex.toLowerCase() === selection.hex?.toLowerCase() : !selection.hex && p.base === selection.base)?.name ?? "Personalizado";
}
export function selectionHex(selection: Selection) { return selection.hex ?? presets.find(p => p.base === selection.base && !p.custom)!.hex; }
export function contrastInk(hex: string, alpha = 1, background = 17) {
  const rgb = [1, 3, 5].map(start => (parseInt(hex.slice(start, start + 2), 16) * alpha + background * (1 - alpha)) / 255);
  const linear = rgb.map(c => c <= .04045 ? c / 12.92 : ((c + .055) / 1.055) ** 2.4);
  return linear[0] * .2126 + linear[1] * .7152 + linear[2] * .0722 > .179 ? "#171717" : "#ffffff";
}
