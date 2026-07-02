import { create } from "zustand";
import type { TagUpdate, DeviceState } from "../ipc/bindings";

interface TagValueMap { [tagId: string]: TagUpdate }

interface RuntimeStore {
  running: boolean;
  devices: DeviceState[];
  tagValues: TagValueMap;
  logs: string[];
  setRunning: (v: boolean) => void;
  setDevices: (d: DeviceState[]) => void;
  updateTagValues: (u: TagUpdate[]) => void;
  addLog: (msg: string) => void;
}

export const useStore = create<RuntimeStore>((set) => ({
  running: false,
  devices: [],
  tagValues: {},
  logs: [],
  setRunning: (v) => set({ running: v }),
  setDevices: (d) => set({ devices: d }),
  updateTagValues: (u) =>
    set((s) => {
      const next = { ...s.tagValues };
      for (const v of u) next[v.tag_id] = v;
      return { tagValues: next };
    }),
  addLog: (msg) =>
    set((s) => ({
      logs: [...s.logs.slice(-499), `[${new Date().toLocaleTimeString()}] ${msg}`],
    })),
}));
