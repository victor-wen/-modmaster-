import GridLayout from "react-grid-layout";
import "react-grid-layout/css/styles.css";
import type { DashboardWidget } from "../widgets/types";
import { registry } from "../widgets/registry";
import type { TagUpdate } from "../ipc/bindings";

interface LayoutEngineProps {
  widgets: DashboardWidget[];
  values: Record<string, TagUpdate>;
  isEditing: boolean;
  onLayoutChange?: (layout: DashboardWidget[]) => void;
}

export function LayoutEngine({ widgets, values, isEditing, onLayoutChange }: LayoutEngineProps) {
  const layout = widgets.map((w) => ({ i: w.id, ...w.layout }));
  const onGridChange = (newLayout: { i: string; x: number; y: number; w: number; h: number }[]) => {
    if (!onLayoutChange) return;
    const updated = widgets.map((w) => {
      const found = newLayout.find((l) => l.i === w.id);
      return found ? { ...w, layout: { x: found.x, y: found.y, w: found.w, h: found.h } } : w;
    });
    onLayoutChange(updated);
  };

  return (
    <GridLayout
      className="layout"
      layout={layout}
      cols={12}
      rowHeight={80}
      width={1200}
      isDraggable={isEditing}
      isResizable={isEditing}
      compactType="vertical"
      onLayoutChange={(l) => onGridChange(l)}
    >
      {widgets.map((w) => {
        const manifest = registry.get(w.widget_id);
        if (!manifest) return <div key={w.id} className="bg-red-900/30 p-2 text-xs text-red-300">Unknown widget: {w.widget_id}</div>;
        const Widget = manifest.runtime;
        const filteredValues: Record<string, TagUpdate> = {};
        for (const b of w.bindings) {
          if (values[b.tag_id]) filteredValues[b.tag_id] = values[b.tag_id];
        }
        return (
          <div key={w.id} className="rounded border border-slate-600 bg-slate-800/60 overflow-hidden">
            {isEditing && (
              <div className="flex items-center justify-between bg-slate-700 px-2 py-0.5 text-xs text-slate-300 cursor-grab drag-handle">
                <span>{manifest.name}</span>
                <span className="text-[10px] text-slate-500">{w.id}</span>
              </div>
            )}
            <div className="h-full w-full p-1">
              <Widget instanceId={w.id} config={w.config} bindings={w.bindings} values={filteredValues} />
            </div>
          </div>
        );
      })}
    </GridLayout>
  );
}
