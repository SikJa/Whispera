// Liquid background for the settings sidebar. The panel is a column of
// spring-coupled nodes along its free (right) edge with a surface-wave term,
// drawn as one SVG path under a goo filter so detached droplets merge back.
// Physics runs in fixed steps with seeded randomness: identical at 60/120/144 Hz.
// The loop sleeps whenever the liquid is at rest, so an idle window costs nothing.
import { useEffect, useId, useRef, type RefObject } from "react";

const STEP = 1 / 120;
const NODES = 28;
const RADIUS = 22;
const REACH = 70; // px from the edge where the cursor starts to pull the liquid

type Drop = { x: number; y: number; vx: number; vy: number; r: number };
type Bubble = { x: number; y: number; r: number; speed: number; phase: number };

function seeded(seed: number) {
  return () => {
    seed |= 0; seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

// Catmull-Rom through the edge nodes, as cubic Béziers.
function edge(points: [number, number][]) {
  let d = "";
  for (let i = 0; i < points.length - 1; i++) {
    const p0 = points[Math.max(0, i - 1)], p1 = points[i], p2 = points[i + 1], p3 = points[Math.min(points.length - 1, i + 2)];
    d += `C${(p1[0] + (p2[0] - p0[0]) / 6).toFixed(2)},${(p1[1] + (p2[1] - p0[1]) / 6).toFixed(2)} ${(p2[0] - (p3[0] - p1[0]) / 6).toFixed(2)},${(p2[1] - (p3[1] - p1[1]) / 6).toFixed(2)} ${p2[0].toFixed(2)},${p2[1].toFixed(2)}`;
  }
  return d;
}

export default function LiquidPanel({ target, expanded, onSettled, onEmptied }: { target: RefObject<HTMLElement | null>; expanded: boolean; onSettled: () => void; onEmptied?: () => void }) {
  const svg = useRef<SVGSVGElement>(null);
  const body = useRef<SVGPathElement>(null);
  const gloss = useRef<SVGPathElement>(null);
  const drops = useRef<SVGGElement>(null);
  const bubbles = useRef<SVGGElement>(null);
  const stream = useRef<SVGRectElement>(null);
  const filter = useId().replace(/:/g, "");
  const wake = useRef<() => void>(() => {});
  const settled = useRef(onSettled); settled.current = onSettled;
  const emptied = useRef(onEmptied); emptied.current = onEmptied;

  useEffect(() => {
    const host = svg.current, aside = target.current;
    if (!host || !aside) return;
    const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
    const random = seeded(0x57a1);
    const x = new Float64Array(NODES), v = new Float64Array(NODES), release = new Float64Array(NODES);
    let width = aside.offsetWidth, height = aside.offsetHeight, left = aside.offsetLeft, top = aside.offsetTop;
    x.fill(width);
    let previous = x[0], lastWidth = width, clock = 0, frame = 0, last = 0, acc = 0, rest = 0;
    let pointer: { x: number; y: number } | null = null;
    // Intro: a stream pours in and the level rises from the bottom.
    let level = reduce ? 0 : 1, intro = !reduce, introTime = 0;
    const floaters: Bubble[] = [];
    const flying: Drop[] = [];
    let slosh = 0;

    const measure = () => { width = aside.offsetWidth; height = aside.offsetHeight; left = aside.offsetLeft; top = aside.offsetTop; };
    const resize = new ResizeObserver(() => { measure(); start(); });
    resize.observe(aside);

    const step = () => {
      clock += STEP;
      measure();
      if (width !== lastWidth) {
        // A width change travels down from the toggle as a wave front.
        const growing = width > lastWidth;
        if (Math.abs(width - previous) > 30 || clock - release[NODES - 1] > .4) {
          for (let i = 0; i < NODES; i++) release[i] = clock + i * .012;
          previous = lastWidth;
          if (growing && !reduce) for (let n = 0; n < 5; n++) flying.push({ x: lastWidth, y: 40 + random() * (height - 80), vx: 220 + random() * 260, vy: (random() - .5) * 60, r: 4 + random() * 5 });
        }
        lastWidth = width;
      }
      for (let i = 0; i < NODES; i++) {
        const goal = clock >= release[i] ? width : previous;
        const above = i > 0 ? x[i - 1] : x[i], below = i < NODES - 1 ? x[i + 1] : x[i];
        let force = 170 * (goal - x[i]) + 420 * (above + below - 2 * x[i]) - 9 * v[i];
        if (pointer) {
          const ny = RADIUS + (i / (NODES - 1)) * (height - 2 * RADIUS);
          const dy = Math.abs(pointer.y - ny), dx = pointer.x - x[i];
          if (dy < 110 && dx > -20 && dx < REACH) force += 330 * Math.min(22, dx + 10) * (1 - dy / 110) ** 2;
        }
        v[i] += force * STEP;
      }
      for (let i = 0; i < NODES; i++) x[i] += v[i] * STEP;
      for (let i = flying.length - 1; i >= 0; i--) {
        const d = flying[i];
        const node = Math.min(NODES - 1, Math.max(0, Math.round(((d.y - RADIUS) / Math.max(1, height - 2 * RADIUS)) * (NODES - 1))));
        d.vx += -90 * (d.x - x[node]) * STEP - d.vx * 3.2 * STEP;
        d.vy -= d.vy * 2 * STEP;
        d.x += d.vx * STEP; d.y += d.vy * STEP; d.r -= STEP * 2.2;
        if (d.r < 1.5) flying.splice(i, 1);
      }
      if (intro) {
        introTime += STEP;
        level = Math.max(0, 1 - Math.max(0, introTime - .25) / 1.05);
        slosh = Math.max(0, 1 - introTime / 2.2);
        if (random() < .5 && floaters.length < 22) floaters.push({ x: 8 + random() * Math.max(10, width - 16), y: height - 6, r: .8 + random() * 2, speed: 40 + random() * 70, phase: random() * 6.28 });
        if (introTime > 1.75) { intro = false; floaters.length = 0; settled.current(); }
      }
      for (let i = floaters.length - 1; i >= 0; i--) {
        const b = floaters[i];
        b.y -= b.speed * STEP; b.phase += STEP * 5;
        if (b.y < height * level + 8) floaters.splice(i, 1);
      }
    };

    const draw = () => {
      host.style.left = `${left}px`; host.style.top = `${top}px`;
      host.setAttribute("width", String(width + 160)); host.setAttribute("height", String(height));
      const span = height - 2 * RADIUS;
      const surface = height * level;
      const points: [number, number][] = [];
      for (let i = 0; i < NODES; i++) points.push([Math.max(RADIUS * 1.2, x[i]), RADIUS + (i / (NODES - 1)) * span]);
      let d: string;
      if (intro && surface > 2) {
        // While filling, the top is a sloshing surface at the current level.
        const wave = (px: number) => surface + Math.sin(px * .09 + clock * 9) * 7 * slosh + Math.sin(px * .045 - clock * 5) * 4 * slosh;
        const right = points[0][0];
        d = `M0,${Math.min(height - RADIUS, wave(0))}`;
        for (let px = 6; px <= right; px += 6) d += `L${px},${Math.min(height - RADIUS, wave(px))}`;
        d += `L${right},${height - RADIUS}Q${right},${height} ${right - RADIUS},${height}L${RADIUS},${height}Q0,${height} 0,${height - RADIUS}Z`;
      } else {
        const [fx, fy] = points[0], [lx, ly] = points[NODES - 1];
        d = `M${RADIUS},0L${fx - RADIUS},0Q${fx},0 ${fx},${fy}` + edge(points) +
          `Q${lx},${height} ${lx - RADIUS},${height}L${RADIUS},${height}Q0,${height} 0,${ly}L0,${RADIUS}Q0,0 ${RADIUS},0Z`;
      }
      body.current?.setAttribute("d", d);
      gloss.current?.setAttribute("d", d);
      if (stream.current) {
        const pouring = intro && introTime < 1.35;
        const thin = Math.max(0, 1 - Math.max(0, introTime - 1.05) / .3) * 7;
        stream.current.setAttribute("x", String(points[0][0] / 2 - thin / 2));
        stream.current.setAttribute("width", String(pouring ? thin : 0));
        stream.current.setAttribute("height", String(pouring ? Math.max(0, surface + 70) : 0));
      }
      if (drops.current) drops.current.innerHTML = flying.map(f => `<circle cx="${f.x.toFixed(1)}" cy="${f.y.toFixed(1)}" r="${f.r.toFixed(1)}"/>`).join("");
      if (bubbles.current) bubbles.current.innerHTML = floaters.map(b => `<circle cx="${(b.x + Math.sin(b.phase) * 2).toFixed(1)}" cy="${b.y.toFixed(1)}" r="${b.r.toFixed(1)}"/>`).join("");
    };

    const energy = () => {
      let e = 0;
      for (let i = 0; i < NODES; i++) e += Math.abs(v[i]) + Math.abs(x[i] - width);
      return e + flying.length * 50;
    };
    const tick = (now: number) => {
      frame = 0;
      acc += Math.min(.1, (now - last) / 1000); last = now;
      while (acc >= STEP) { step(); acc -= STEP; }
      draw();
      rest = energy() < 2 && !intro && !pointer ? rest + 1 : 0;
      if (rest < 20) frame = requestAnimationFrame(tick);
    };
    function start() { if (!frame) { last = performance.now(); rest = 0; frame = requestAnimationFrame(tick); } }
    wake.current = start;

    const move = (e: PointerEvent) => {
      const box = aside.getBoundingClientRect();
      const px = e.clientX - box.left, py = e.clientY - box.top;
      const near = py > 0 && py < box.height && px > box.width - 30 && px < box.width + REACH;
      pointer = near && !reduce ? { x: px, y: py } : null;
      if (near) start();
    };
    const leave = () => { pointer = null; };
    // Settings windows are hidden, not closed: pour again every time one is shown.
    const replay = () => {
      if (document.hidden || reduce) return;
      level = 1; intro = true; introTime = 0; slosh = 0;
      floaters.length = 0; flying.length = 0;
      emptied.current?.(); start();
    };
    window.addEventListener("pointermove", move);
    document.addEventListener("pointerleave", leave);
    document.addEventListener("visibilitychange", replay);
    if (reduce) settled.current();
    draw(); start();
    return () => {
      cancelAnimationFrame(frame); resize.disconnect();
      window.removeEventListener("pointermove", move); document.removeEventListener("pointerleave", leave);
      document.removeEventListener("visibilitychange", replay);
    };
  }, [target]);

  useEffect(() => { wake.current(); }, [expanded]);

  return <svg ref={svg} className="liquid-panel" aria-hidden="true" style={{ overflow: "visible" }}>
    <defs>
      <filter id={filter} x="-20%" y="-10%" width="140%" height="120%">
        <feGaussianBlur in="SourceGraphic" stdDeviation="5" result="blur" />
        <feColorMatrix in="blur" mode="matrix" values="1 0 0 0 0  0 1 0 0 0  0 0 1 0 0  0 0 0 22 -9" />
      </filter>
      <linearGradient id={`${filter}-sheen`} x1="0" y1="0" x2="1" y2="1">
        <stop offset="0" stopColor="var(--liquid-sheen)" />
        <stop offset=".45" stopColor="transparent" />
      </linearGradient>
    </defs>
    <g filter={`url(#${filter})`} className="liquid-body">
      <rect ref={stream} y="-60" width="0" height="0" rx="3" />
      <path ref={body} />
      <g ref={drops} />
    </g>
    <path ref={gloss} className="liquid-gloss" fill={`url(#${filter}-sheen)`} />
    <g ref={bubbles} className="liquid-bubbles" />
  </svg>;
}
