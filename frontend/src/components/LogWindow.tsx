import { useRef, useEffect } from "react";
import { useStore } from "../store";
export function LogWindow() {
  const logs = useStore((s) => s.logs);
  const endRef = useRef<HTMLDivElement>(null);
  useEffect(() => { endRef.current?.scrollIntoView({ behavior: "smooth" }); }, [logs]);
  return (
    <div className="h-40 overflow-y-auto bg-black font-mono text-xs text-green-400 p-2">
      {logs.map((l, i) => <div key={i}>{l}</div>)}
      <div ref={endRef} />
    </div>
  );
}
