import { useState, useRef, useEffect } from "react";
import { createChart, ColorType } from "lightweight-charts";
import type { UTCTimestamp } from "lightweight-charts";
import { createClient } from "../ipc/client";
import type { Sample } from "../ipc/bindings";

const client = createClient();

export default function TrendPage() {
  const [tagId, setTagId] = useState("");
  const [hoursBack, setHoursBack] = useState(1);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const chartContainerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<ReturnType<typeof createChart> | null>(null);
  const seriesRef = useRef<ReturnType<ReturnType<typeof createChart>["addLineSeries"]> | null>(null);

  useEffect(() => {
    if (!chartContainerRef.current) return;
    const chart = createChart(chartContainerRef.current, {
      layout: { background: { type: ColorType.Solid, color: "#1e293b" }, textColor: "#94a3b8" },
      grid: { vertLines: { color: "#334155" }, horzLines: { color: "#334155" } },
      width: chartContainerRef.current.clientWidth,
      height: 400,
      crosshair: { mode: 0 },
    });
    chart.timeScale().fitContent();
    const series = chart.addLineSeries({ color: "#3b82f6", lineWidth: 2 });
    chartRef.current = chart;
    seriesRef.current = series;

    const ro = new ResizeObserver((entries) => {
      for (const e of entries) {
        chart.applyOptions({ width: e.contentRect.width });
      }
    });
    ro.observe(chartContainerRef.current);

    return () => {
      ro.disconnect();
      chart.remove();
      chartRef.current = null;
      seriesRef.current = null;
    };
  }, []);

  const handleQuery = async () => {
    if (!tagId.trim()) return;
    setLoading(true);
    setError("");
    try {
      const now = Date.now();
      const fromMs = now - hoursBack * 3600 * 1000;
      const data: Sample[] = await client.queryTrend([tagId.trim()], fromMs, now, 2000);
      const series = seriesRef.current;
      if (!series) return;
      const points = data
        .filter((s) => typeof s.value === "number")
        .map((s) => ({
          time: Math.floor(new Date(s.ts).getTime() / 1000) as UTCTimestamp,
          value: s.value as number,
        }));
      series.setData(points);
      chartRef.current?.timeScale().fitContent();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="flex h-full flex-col gap-4">
      <h2 className="text-lg font-semibold">Trend</h2>
      <div className="flex items-end gap-3">
        <div className="flex flex-col">
          <label htmlFor="tag-id" className="mb-1 text-xs text-slate-400">Tag ID</label>
          <input id="tag-id"
            className="rounded border border-slate-600 bg-slate-800 px-3 py-1.5 text-sm text-white"
            value={tagId}
            onChange={(e) => setTagId(e.target.value)}
            placeholder="dev_1/tag_temp"
          />
        </div>
        <div className="flex flex-col">
          <label htmlFor="hours-back" className="mb-1 text-xs text-slate-400">Hours Back</label>
          <input id="hours-back"
            type="number"
            className="w-24 rounded border border-slate-600 bg-slate-800 px-3 py-1.5 text-sm text-white"
            value={hoursBack}
            min={0.1}
            step={0.5}
            onChange={(e) => setHoursBack(parseFloat(e.target.value) || 1)}
          />
        </div>
        <button type="button"
          onClick={handleQuery}
          disabled={loading}
          className="rounded bg-blue-600 px-4 py-1.5 text-sm text-white hover:bg-blue-500 disabled:opacity-50"
        >
          {loading ? "Loading..." : "Query"}
        </button>
      </div>
      {error && <div className="rounded bg-red-900/30 px-3 py-2 text-sm text-red-300">{error}</div>}
      <div ref={chartContainerRef} className="flex-1 rounded border border-slate-600" />
    </div>
  );
}
