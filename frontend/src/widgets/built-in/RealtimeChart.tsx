import { useEffect, useRef } from "react";
import { createChart, ColorType } from "lightweight-charts";
import type { UTCTimestamp } from "lightweight-charts";
import type { WidgetRuntimeProps } from "../types";

const COLORS = ["#3b82f6", "#ef4444", "#22c55e", "#f59e0b", "#8b5cf6", "#ec4899"];

export function RealtimeChart({ bindings, values }: WidgetRuntimeProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<ReturnType<typeof createChart> | null>(null);
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
    chartRef.current = chart;

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
      chartRef.current = null;
      chart.remove();
    };
  }, [bindings]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const ro = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const { width, height } = entry.contentRect;
        chartRef.current?.applyOptions({ width, height });
      }
    });
    ro.observe(container);
    return () => ro.disconnect();
  }, []);

  useEffect(() => {
    const seriesMap = seriesRef.current;
    for (const [tagId, series] of seriesMap) {
      const v = values[tagId];
      if (v && typeof v.value === "number") {
        series.update({ time: Math.floor(new Date(v.ts).getTime() / 1000) as UTCTimestamp, value: v.value });
      }
    }
  }, [values]);

  return <div ref={containerRef} className="h-full w-full" />;
}
