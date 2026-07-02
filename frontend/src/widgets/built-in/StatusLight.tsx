import type { WidgetRuntimeProps } from "../types";

export function StatusLight({ bindings, values, config }: WidgetRuntimeProps) {
  const tagId = bindings[0]?.tag_id;
  const v = tagId ? values[tagId] : undefined;
  const on = v?.value === true || v?.value === 1;
  return (
    <div className="flex h-full flex-col items-center justify-center">
      <h3 className="text-xs font-bold">{String(config.title || "")}</h3>
      <div className="flex items-center justify-center gap-2">
        <span
          className={`inline-block h-4 w-4 rounded-full ${on ? "bg-green-500" : "bg-gray-500"} shadow-lg`}
        />
        <span className="text-sm font-semibold">{on ? "ON" : "OFF"}</span>
      </div>
    </div>
  );
}
