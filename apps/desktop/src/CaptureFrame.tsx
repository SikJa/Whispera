import type { CSSProperties, ReactNode } from 'react';

/** Preview decoration only. Video uses the capture-excluded selector window;
 * screenshot export draws the original pixels and marks on a separate canvas. */
export default function CaptureFrame({ width, height, color = '#ffffff', className = '', style, image, testId, children }: {
  width: number; height: number; color?: string; className?: string; style?: CSSProperties;
  image?: boolean; testId?: string; children?: ReactNode;
}) {
  const radius = Math.max(0, Math.min(14, width / 2, height / 2));
  const pathWidth=Math.max(0,width-2),pathHeight=Math.max(0,height-2),pathRadius=Math.max(0,radius-1);
  // A two-pixel CSS border is centered one pixel from its outer edge.
  // The shine follows that exact centerline, rather than a second inset contour.
  const length = Math.max(1, 2 * (pathWidth + pathHeight) - 8 * pathRadius + 2 * Math.PI * pathRadius);
  const tail = Math.min(18, 100 * 110 / length);
  return <div className={`capture-frame ${className}`} data-image={image} data-testid={testId}
    style={{ width, height, borderColor: color, borderRadius: radius, '--capture-frame-color': color, ...style } as CSSProperties}>
    <svg className="capture-border-beam" aria-hidden="true" width="100%" height="100%">
      {[{ size: 1, opacity: .28, stroke: 4 }, { size: .65, opacity: .50, stroke: 3 }, { size: .28, opacity: 1, stroke: 2 }].map((part, i) =>
        <rect key={i} x="1" y="1" width={pathWidth} height={pathHeight} rx={pathRadius}
          pathLength="100" fill="none" stroke="currentColor" strokeWidth={part.stroke} strokeOpacity={part.opacity}
          strokeLinecap="round" strokeDasharray={`${tail * part.size} ${100 - tail * part.size}`} />)}
    </svg>
    {children}
  </div>;
}
