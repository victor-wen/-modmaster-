import { useStore } from "../store";
export function RuntimeStatusBar() {
  const running = useStore((s) => s.running);
  const devices = useStore((s) => s.devices);
  const online = devices.filter((d) => d.online).length;
  return (
    <div className="flex items-center gap-4 bg-slate-700 px-4 py-1 text-xs text-white">
      <span className={running ? "text-green-400" : "text-red-400"}>
        {running ? "Running" : "Stopped"}
      </span>
      <span>Devices: {online}/{devices.length} online</span>
    </div>
  );
}
