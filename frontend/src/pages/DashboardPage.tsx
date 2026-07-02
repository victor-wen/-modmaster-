import { useState, useCallback } from "react";
import { DashboardRuntime } from "../dashboard/DashboardRuntime";
import { registry } from "../widgets/registry";
import type { DashboardWidget } from "../widgets/types";
import { LogWindow } from "../components/LogWindow";

export default function DashboardPage() {
  const [isEditing, setIsEditing] = useState(false);
  const [widgets, setWidgets] = useState<DashboardWidget[]>([]);
  const [showPicker, setShowPicker] = useState(false);

  const addWidget = useCallback((widgetId: string) => {
    const manifest = registry.get(widgetId);
    if (!manifest) return;
    const id = `w_${Date.now()}_${Math.random().toString(36).slice(2, 6)}`;
    const w: DashboardWidget = {
      id,
      widget_id: widgetId,
      layout: { x: 0, y: Infinity, w: manifest.defaultProps.w, h: manifest.defaultProps.h },
      config: {},
      bindings: [],
    };
    setWidgets((prev) => [...prev, w]);
    setShowPicker(false);
  }, []);

  return (
    <div className="flex h-full flex-col gap-4">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold">Dashboard</h2>
        <div className="flex gap-2">
          <button type="button"
            onClick={() => setIsEditing((v) => !v)}
            className={`rounded px-3 py-1 text-sm ${isEditing ? "bg-blue-600 text-white" : "bg-slate-700 text-slate-200 hover:bg-slate-600"}`}
          >
            {isEditing ? "Done Editing" : "Edit"}
          </button>
          {isEditing && (
            <button type="button"
              onClick={() => setShowPicker(true)}
              className="rounded bg-green-700 px-3 py-1 text-sm text-white hover:bg-green-600"
            >
              + Add Widget
            </button>
          )}
        </div>
      </div>

      {showPicker && (
        <div className="rounded border border-slate-600 bg-slate-800 p-4">
          <h3 className="mb-2 text-sm font-semibold text-slate-300">Select Widget</h3>
          <div className="flex flex-wrap gap-2">
            {registry.list().map((m) => (
              <button type="button"
                key={m.id}
                onClick={() => addWidget(m.id)}
                className="rounded bg-slate-700 px-3 py-2 text-sm text-slate-200 hover:bg-slate-600"
              >
                {m.name}
              </button>
            ))}
          </div>
        </div>
      )}

      <div className="flex-1 overflow-auto">
        <DashboardRuntime
          widgets={widgets}
          isEditing={isEditing}
          onLayoutChange={setWidgets}
        />
      </div>

      <LogWindow />
    </div>
  );
}
