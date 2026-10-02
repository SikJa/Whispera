import type { CSSProperties, ReactNode } from 'react';

/** Preview decoration only. Video uses the capture-excluded selector window;
 * screenshot export draws the original pixels and marks on a separate canvas. */
export default function CaptureFrame({ width, height, color = '#ffffff', className = '', style, image, testId, children }: {
  width: number; height: number; color?: string; className?: string; style?: CSSProperties;
  image?: boolean; testId?: string; children?: ReactNode;
}) {
  const radius = Math.max(0, Math.min(14, width / 2, height / 2));
  const length = Math.max(1, 2 * (width + height) - 8 * radius + 2 * Math.PI * radius);
  const tail = Math.min(18, 100 * 110 / length);
  return <div className={`capture-frame ${className}`} data-image={image} data-testid={testId}
    style={{ width, height, borderColor: color, borderRadius: radius, '--capture-frame-color': color, ...style } as CSSProperties}>
    <svg className="capture-border-beam" aria-hidden="true" width="100%" height="100%">
      {[{ size: 1, opacity: .18, stroke: 5 }, { size: .65, opacity: .38, stroke: 4 }, { size: .28, opacity: 1, stroke: 3 }].map((part, i) =>
        <rect key={i} x="2" y="2" width={Math.max(0, width - 4)} height={Math.max(0, height - 4)} rx={Math.max(0, radius - 2)}
          pathLength="100" fill="none" stroke="currentColor" strokeWidth={part.stroke} strokeOpacity={part.opacity}
          strokeLinecap="round" strokeDasharray={`${tail * part.size} ${100 - tail * part.size}`} />)}
    </svg>
    {children}
  </div>;
}
