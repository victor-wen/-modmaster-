import type { WidgetRuntimeProps } from "../types";

export function Gauge({ bindings, values }: WidgetRuntimeProps) {
  const tagId = bindings[0]?.tag_id;
  const v = tagId ? values[tagId] : undefined;
  const raw = typeof v?.value === "number" ? v.value : 0;
  const pct = Math.min(100, Math.max(0, raw));
  const r = 40;
  const circ = 2 * Math.PI * r;
  const offset = circ * (1 - pct / 100);

  return (
    <div className="flex h-full flex-col items-center justify-center">
      <svg viewBox="0 0 100 100" className="w-24 h-24" aria-label="Gauge">
        <title>Gauge</title>
        <circle cx="50" cy="50" r={r} fill="none" stroke="#334155" strokeWidth="8" />
        <circle
          cx="50" cy="50" r={r} fill="none" stroke="#3b82f6" strokeWidth="8"
          strokeDasharray={circ} strokeDashoffset={offset}
          strokeLinecap="round" transform="rotate(-90 50 50)"
          className="transition-all duration-300"
        />
        <text x="50" y="50" textAnchor="middle" dominantBaseline="central" className="fill-white text-sm font-bold">
          {pct.toFixed(0)}%
        </text>
      </svg>
      {v?.unit && <span className="mt-1 text-xs text-slate-400">{v.unit}</span>}
    </div>
  );
}
