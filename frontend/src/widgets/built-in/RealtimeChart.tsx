import { useEffect, useRef } from "react";
import { createChart, ColorType } from "lightweight-charts";
import type { WidgetRuntimeProps } from "../types";

const COLORS = ["#3b82f6", "#ef4444", "#22c55e", "#f59e0b", "#8b5cf6", "#ec4899"];

export function RealtimeChart({ bindings, values }: WidgetRuntimeProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const seriesRef = useRef<Map<string, ReturnType<ReturnType<typeof createChart>["addLineSeries"]>>>(new Map());

  useEffect(() => {
    if (!containerRef.current) return;
    const chart = createChart(containerRef.current, {
      layout: { background: { type: ColorType.Solid, color: "transparent" }, textColor: "#94a3b8" },
      grid: { vertLines: { color: "#334155" }, horzLines: { color: "#334155" } },
      width: containerRef.current.clientWidth,
      height: containerRef.current.clientHeight,
      crosshair: { mode: 0 },
    });
    chart.timeScale().fitContent();

    const seriesMap = seriesRef.current;
    bindings.forEach((b, i) => {
      const series = chart.addLineSeries({
        color: COLORS[i % COLORS.length],
        lineWidth: 2,
        priceFormat: { type: "custom", formatter: (v: number) => v.toFixed(2) },
      });
      seriesMap.set(b.tag_id, series);
    });

    return () => {
      seriesMap.clear();
      chart.remove();
    };
  }, [bindings]);

  useEffect(() => {
    const seriesMap = seriesRef.current;
    for (const [tagId, series] of seriesMap) {
      const v = values[tagId];
      if (v && typeof v.value === "number") {
        series.update({ time: v.ts.slice(5, 19).replace("T", " ") as any, value: v.value });
      }
    }
  }, [values]);

  return <div ref={containerRef} className="h-full w-full" />;
}
