import type { WidgetRuntimeProps } from "../types";

export function NumericTable({ bindings, values }: WidgetRuntimeProps) {
  const rows = bindings.map((b) => {
    const v = values[b.tag_id];
    return (
      <tr key={b.tag_id} className="border-b border-slate-600">
        <td className="px-2 py-1 text-sm">{b.tag_id}</td>
        <td className="px-2 py-1 text-right font-mono">
          {v ? String(v.value) : "—"}
        </td>
        <td className="px-2 py-1 text-xs text-slate-400">
          {v?.unit ?? ""}
        </td>
      </tr>
    );
  });

  return (
    <div className="h-full overflow-auto">
      <table className="w-full text-xs">
        <thead>
          <tr className="bg-slate-700 text-left text-slate-300">
            <th className="px-2 py-1">Tag</th>
            <th className="px-2 py-1 text-right">Value</th>
            <th className="px-2 py-1">Unit</th>
          </tr>
        </thead>
        <tbody>{rows}</tbody>
      </table>
    </div>
  );
}
