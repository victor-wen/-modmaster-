import type { WidgetRuntimeProps } from "../types";

const BAR_COLORS = ["#3b82f6", "#ef4444", "#22c55e", "#f59e0b", "#8b5cf6", "#ec4899"];

export function BarChart({ bindings, values, config }: WidgetRuntimeProps) {
  const maxVal = Math.max(1, ...bindings.map((b) => {
    const v = values[b.tag_id];
    return typeof v?.value === "number" ? Math.abs(v.value) : 0;
  }));

  return (
    <div className="flex h-full flex-col">
      <h3 className="text-xs font-bold px-2">{String(config.title || "")}</h3>
      <div className="flex flex-1 items-end gap-1 px-2 pt-4">
        {bindings.map((b, i) => {
          const v = values[b.tag_id];
          const val = typeof v?.value === "number" ? v.value : 0;
          const pct = (Math.abs(val) / maxVal) * 100;
          return (
            <div key={b.tag_id} className="flex flex-1 flex-col items-center justify-end h-full">
              <span className="mb-1 text-[10px] text-slate-400">{val}</span>
              <div
                className="w-full rounded-t transition-all duration-200"
                style={{ height: `${pct}%`, backgroundColor: BAR_COLORS[i % BAR_COLORS.length] }}
              />
              <span className="mt-1 truncate text-[10px] text-slate-500 w-full text-center">{b.tag_id}</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
