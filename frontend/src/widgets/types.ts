import type { ComponentType } from "react";
import type { TagUpdate } from "../ipc/bindings";

export interface WidgetManifest {
  id: string;
  name: string;
  category: "indicator" | "chart" | "table" | "control";
  configSchema: Record<string, unknown>;
  dataBinding: { minSources: number; maxSources: number; sourceRoleNames: string[] };
  defaultProps: { w: number; h: number };
  runtime: ComponentType<WidgetRuntimeProps>;
  editor?: ComponentType<WidgetEditorProps>;
}

export interface WidgetRuntimeProps {
  instanceId: string;
  config: Record<string, unknown>;
  bindings: TagBinding[];
  values: Record<string, TagUpdate>;
}

export interface WidgetEditorProps {
  config: Record<string, unknown>;
  bindings: TagBinding[];
  onChange: (config: Record<string, unknown>, bindings: TagBinding[]) => void;
}

export interface TagBinding {
  role: number;
  tag_id: string;
}

export interface DashboardWidget {
  id: string;
  widget_id: string;
  layout: { x: number; y: number; w: number; h: number };
  config: Record<string, unknown>;
  bindings: TagBinding[];
}

export interface Dashboard {
  name: string;
  widgets: DashboardWidget[];
}
