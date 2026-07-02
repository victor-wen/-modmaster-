import { useStore } from "../store";
import { useTagSubscription } from "../hooks/useTagSubscription";
import { LayoutEngine } from "./LayoutEngine";
import type { DashboardWidget } from "../widgets/types";
import { registry } from "../widgets/registry";

interface DashboardRuntimeProps {
  widgets: DashboardWidget[];
  isEditing: boolean;
  onLayoutChange?: (widgets: DashboardWidget[]) => void;
}

export function DashboardRuntime({ widgets, isEditing, onLayoutChange }: DashboardRuntimeProps) {
  useTagSubscription();
  const tagValues = useStore((s) => s.tagValues);

  const available = registry.list();
  if (widgets.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center gap-4 py-20 text-slate-400">
        <p className="text-lg">No widgets yet</p>
        {!isEditing && <p className="text-sm">Enable edit mode to add widgets</p>}
        <div className="text-sm">
          Available widgets: {available.map((m) => m.name).join(", ")}
        </div>
      </div>
    );
  }

  return (
    <LayoutEngine
      widgets={widgets}
      values={tagValues}
      isEditing={isEditing}
      onLayoutChange={onLayoutChange}
    />
  );
}
