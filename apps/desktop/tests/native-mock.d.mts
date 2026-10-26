export function installNativeMock(options?: {
  kind?: 'image'|'video'; width?: number; height?: number; scale?: number;
  setupComplete?: boolean; selectionPhase?: string; frozenImage?: number[];
  adjustable?: boolean; windowLabel?: string;
}): void;
