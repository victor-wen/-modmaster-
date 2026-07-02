import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { UnlistenFn } from "@tauri-apps/api/event";
import type { Project, Device, TagUpdate, DeviceState, RuntimeStatus, Sample, LogEntry } from "./bindings";
export interface IpcClient {
  newProject(name: string): Promise<Project>;
  openProject(path: string): Promise<Project>;
  saveProject(p: Project): Promise<void>;
  listDevices(): Promise<Device[]>;
  startRuntime(): Promise<void>;
  stopRuntime(): Promise<void>;
  runtimeStatus(): Promise<RuntimeStatus>;
  writeTag(tagId: string, value: string): Promise<void>;
  queryTrend(tagIds: string[], fromMs: number, toMs: number, maxPoints: number): Promise<Sample[]>;
  onTagUpdate(cb: (u: TagUpdate[]) => void): Promise<UnlistenFn>;
  onDeviceState(cb: (s: DeviceState) => void): Promise<UnlistenFn>;
  onLog(cb: (e: LogEntry) => void): Promise<UnlistenFn>;
}
export function createClient(): IpcClient {
  return {
    newProject: (n) => invoke("new_project", { name: n }),
    openProject: (p) => invoke("open_project", { path: p }),
    saveProject: (p) => invoke("save_project", { project: p }),
    listDevices: () => invoke("list_devices"),
    startRuntime: () => invoke("start_runtime"),
    stopRuntime: () => invoke("stop_runtime"),
    runtimeStatus: () => invoke("runtime_status"),
    writeTag: (id, v) => invoke("write_tag", { tagId: id, value: v }),
    queryTrend: (ids, f, t, m) => invoke("query_trend", { tagIds: ids, fromMs: f, toMs: t, maxPoints: m }),
    onTagUpdate: (cb) => listen<TagUpdate[]>("tag-update", (e) => cb(e.payload)),
    onDeviceState: (cb) => listen<DeviceState>("device-state", (e) => cb(e.payload)),
    onLog: (cb) => listen<LogEntry>("log", (e) => cb(e.payload)),
  };
}
