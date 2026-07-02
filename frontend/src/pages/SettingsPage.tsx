import { useState } from "react";
import { createClient } from "../ipc/client";

const client = createClient();

export default function SettingsPage() {
  const [projectName, setProjectName] = useState("My Project");
  const [status, setStatus] = useState("");

  const handleSave = async () => {
    try {
      await client.saveProject({
        name: projectName,
        version: 1,
        runtime: { default_poll_interval_ms: 1000 },
        storage: { history_sampling_ms: 1000, trend_max_points: 2000 },
      });
      setStatus("Project saved");
    } catch (e) {
      setStatus(`Save failed: ${e}`);
    }
  };

  const handleOpen = async () => {
    const path = prompt("Enter project directory path:");
    if (!path) return;
    try {
      const p = await client.openProject(path);
      setProjectName(p.name);
      setStatus(`Opened "${p.name}"`);
    } catch (e) {
      setStatus(`Open failed: ${e}`);
    }
  };

  const handleNew = async () => {
    if (!projectName.trim()) return;
    try {
      await client.newProject(projectName.trim());
      setStatus(`Created "${projectName}"`);
    } catch (e) {
      setStatus(`Create failed: ${e}`);
    }
  };

  return (
    <div className="flex h-full flex-col gap-4">
      <h2 className="text-lg font-semibold">Settings</h2>
      <div className="max-w-md rounded border border-slate-600 bg-slate-800/50 p-4">
        <div className="flex flex-col gap-3">
          <div>
            <label htmlFor="project-name" className="mb-1 block text-xs text-slate-400">Project Name</label>
            <input id="project-name"
              className="w-full rounded border border-slate-600 bg-slate-800 px-3 py-1.5 text-sm text-white"
              value={projectName}
              onChange={(e) => setProjectName(e.target.value)}
            />
          </div>
          <div className="flex gap-2">
            <button type="button" onClick={handleSave}
              className="rounded bg-blue-600 px-4 py-1.5 text-sm text-white hover:bg-blue-500">
              Save Project
            </button>
            <button type="button" onClick={handleOpen}
              className="rounded bg-slate-600 px-4 py-1.5 text-sm text-white hover:bg-slate-500">
              Open Project
            </button>
            <button type="button" onClick={handleNew}
              className="rounded bg-green-700 px-4 py-1.5 text-sm text-white hover:bg-green-600">
              New Project
            </button>
          </div>
          {status && <p className="text-sm text-slate-400">{status}</p>}
        </div>
      </div>
    </div>
  );
}
